//! Delivery-tracking repository for outbound email.
//!
//! Two distinct lifecycles share this one table:
//!
//! - **OTP** (Phase 0, unchanged): inline-only. The OTP code a resend would
//!   need is never persisted (see `crate::email_delivery`'s module doc for
//!   why), so retry is bounded and inline, happening entirely within the
//!   original request. These rows never populate `related_entity_type`,
//!   `related_entity_id`, or `claimed_until`, and nothing ever claims them —
//!   this module's claim/lease functions only ever select rows a
//!   transactional-email INSERT created.
//! - **Transactional** (Phase 1B): asynchronous and durable. A business
//!   handler inserts a *thin* row (`enqueue_transactional`, no rendered
//!   content — see that function's own doc comment for why) inside its own
//!   business transaction, atomically with the event it's about. A
//!   background worker (`services::email_dispatch_worker`) later claims due
//!   rows with a lease (`claimed_until`) mirroring `notification_repo`'s own
//!   `SELECT ... FOR UPDATE SKIP LOCKED` claim idiom, resolves fresh display
//!   content from the referenced entity, renders, and sends — with bounded
//!   retries and exponential backoff (`next_attempt_at`) across ticks, not
//!   inline sleeps.
//!
//! Mirrors `notification_repo`'s dedup-by-unique-constraint idiom for both
//! lifecycles alike.
//!
//! Unlike `notification_repo::insert_if_new`, this uses a plain
//! `INSERT ... ON CONFLICT (dedup_key) DO NOTHING` rather than a caught
//! `23505`. That's safe here specifically because `email_outbox`'s RLS
//! policies grant the two writer roles (`hellodoctor_auth_worker`,
//! `hellodoctor_system_worker`) unrestricted `USING (true)` visibility — the
//! problem `notification_repo` worked around (an owner-scoped `SELECT`
//! policy making `ON CONFLICT`'s conflict-resolution check unable to see a
//! row it would conflict with) does not exist for this table.
//!
//! Post-PITR-restore behavior differs by lifecycle. For OTP (nothing but
//! `expire_stale` ever transitions a row after creation — see
//! `docs/EMAIL_OUTBOX_RECOVERY_POLICY.md`), a restored row is never replayed
//! regardless of its status. For transactional rows, a restore to a point
//! before a row reached `SENT_TO_PROVIDER` genuinely can cause the worker to
//! legitimately re-claim and re-send it — there is no way to make "provider
//! accepted the email" and "database durably recorded that" a single atomic
//! operation. This is the same at-least-once (never exactly-once) delivery
//! guarantee any outbox pattern gives when the external side-effect and the
//! database commit cannot share a transaction; a duplicate business-event
//! email (e.g. a second "payment confirmed" after an operator-initiated PITR
//! restore) is an accepted, documented consequence, not a bug this module
//! tries to eliminate.

use chrono::{DateTime, Duration, Utc};
use sqlx::PgExecutor;
use uuid::Uuid;

/// Generous backstop for the periodic `expire_stale` reconciliation sweep —
/// far longer than any real retry campaign should take (`MAX_ATTEMPTS`'s own
/// exponential backoff, capped at 3600s/attempt, exhausts its budget in well
/// under a day), so this only ever catches a row that somehow never got
/// touched by any worker tick at all, not a row still legitimately retrying.
pub fn transactional_email_expiry() -> DateTime<Utc> {
    Utc::now() + Duration::hours(24 * 7)
}

/// Creates a new outbox row for a send attempt that is about to begin.
/// `dedup_key` is expected to already be unique per logical send attempt
/// (e.g. `otp-email:{realm}:{challenge_token}` — the challenge token is
/// itself a fresh UUID per request, so a conflict here is a defensive guard
/// against an accidental double-call, not a resend-suppression mechanism).
/// Returns `true` only if a new row was actually inserted.
pub async fn insert_if_new<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    email_type: &str,
    recipient_email: &str,
    dedup_key: &str,
    expires_at: DateTime<Utc>,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO email_outbox (id, email_type, recipient_email, dedup_key, expires_at) \
         VALUES ($1,$2,$3,$4,$5) ON CONFLICT (dedup_key) DO NOTHING",
    )
    .bind(id)
    .bind(email_type)
    .bind(recipient_email)
    .bind(dedup_key)
    .bind(expires_at)
    .execute(executor)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// Phase 1B: inserts a *thin* transactional-email intent — `email_type`,
/// `recipient_email`, `dedup_key`, and a reference to the business entity
/// the content should be rendered from (`related_entity_type`/
/// `related_entity_id`), but never rendered subject/body content. Intended
/// to be called with `&mut *tx` on the business transaction that is about to
/// commit the event this email is about — its own atomicity comes entirely
/// from running on that same connection, not from anything in this
/// function. Content is deliberately resolved and rendered later, at
/// dispatch time, by `services::email_dispatch_worker` (reading fresh data
/// via the system-worker pool) rather than captured now, for two reasons:
/// the business transaction's own role (e.g. `hellodoctor_payment_worker`)
/// is deliberately not granted broad read access to doctor/patient profile
/// tables it has no other reason to touch, and rendering at dispatch time
/// means a row created just before a second, superseding event (see
/// `supersede_pending`) never needs to race to capture "the latest" data —
/// whichever row is still live when the worker gets to it reads whatever is
/// true then. Returns `true` only if a new row was actually inserted (the
/// same idempotent-enqueue guarantee `insert_if_new` gives OTP).
#[allow(clippy::too_many_arguments)]
pub async fn enqueue_transactional<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    email_type: &str,
    recipient_email: &str,
    dedup_key: &str,
    related_entity_type: &str,
    related_entity_id: Uuid,
    expires_at: DateTime<Utc>,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO email_outbox \
            (id, email_type, recipient_email, dedup_key, related_entity_type, related_entity_id, expires_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7) ON CONFLICT (dedup_key) DO NOTHING",
    )
    .bind(id)
    .bind(email_type)
    .bind(recipient_email)
    .bind(dedup_key)
    .bind(related_entity_type)
    .bind(related_entity_id)
    .bind(expires_at)
    .execute(executor)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// Marks any still-pending (`CREATED`, i.e. not yet sent/failed/expired) row
/// for the same business entity and one of `email_types` as `SUPERSEDED` —
/// the worker's claim query only ever selects `CREATED` rows, so a
/// superseded row is simply never picked up again. Used when a later event
/// for the same entity makes an earlier queued email stale (e.g. a second
/// reschedule before the first reschedule email ever sent): run inside the
/// same transaction as the new event, immediately alongside
/// `enqueue_transactional`'s own insert for that new event, so "the old
/// email is superseded" and "the new email now exists" commit together.
/// `except_dedup_key` excludes the row this same call is about to insert (or
/// just inserted), so a call can never supersede itself.
pub async fn supersede_pending<'e, E: PgExecutor<'e>>(
    executor: E,
    email_types: &[&str],
    related_entity_type: &str,
    related_entity_id: Uuid,
    except_dedup_key: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE email_outbox SET delivery_status='SUPERSEDED', updated_at=now() \
         WHERE delivery_status='CREATED' AND related_entity_type=$1 AND related_entity_id=$2 \
           AND email_type = ANY($3) AND dedup_key <> $4",
    )
    .bind(related_entity_type)
    .bind(related_entity_id)
    .bind(email_types)
    .bind(except_dedup_key)
    .execute(executor)
    .await?;
    Ok(result.rows_affected())
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ClaimedTransactionalEmail {
    pub id: Uuid,
    pub email_type: String,
    pub recipient_email: String,
    pub related_entity_type: String,
    pub related_entity_id: Uuid,
    pub delivery_attempts: i32,
}

/// Claims up to `limit` due transactional rows — `CREATED`, `next_attempt_at`
/// reached, and either never claimed or whose lease (`claimed_until`) has
/// expired (stale-lease recovery: a worker that crashed mid-send leaves its
/// claim to expire rather than blocking the row forever). One atomic
/// statement: the `FOR UPDATE SKIP LOCKED` subquery lets concurrent worker
/// instances each claim a disjoint set with no explicit transaction or
/// advisory lock needed, mirroring `notification_repo`'s own claim idiom.
/// Sets a fresh lease (`now() + lease_seconds`) on every claimed row in the
/// same statement, so a row is never returned to two callers at once even
/// under real concurrency.
pub async fn claim_due_transactional(
    pool: &sqlx::PgPool,
    lease_seconds: i64,
    limit: i64,
) -> Result<Vec<ClaimedTransactionalEmail>, sqlx::Error> {
    sqlx::query_as(
        "WITH due AS ( \
            SELECT id FROM email_outbox \
             WHERE delivery_status='CREATED' AND next_attempt_at<=clock_timestamp() \
               AND (claimed_until IS NULL OR claimed_until<=clock_timestamp()) \
               AND related_entity_type IS NOT NULL \
             ORDER BY next_attempt_at \
             LIMIT $2 \
             FOR UPDATE SKIP LOCKED \
         ) \
         UPDATE email_outbox SET claimed_until = clock_timestamp() + make_interval(secs => $1) \
          WHERE id IN (SELECT id FROM due) \
          RETURNING id, email_type, recipient_email, related_entity_type, related_entity_id, delivery_attempts",
    )
    .bind(lease_seconds)
    .bind(limit)
    .fetch_all(pool)
    .await
}

/// Same claim as `claim_due_transactional`, scoped to one specific row —
/// used by the post-commit "try to send this one right now" fast path (see
/// `services::email_dispatch_worker::dispatch_now`). If the periodic batch
/// worker has already claimed (or already finished) this row concurrently,
/// `SKIP LOCKED`/the `CREATED`+due filter simply yields nothing here, so the
/// two paths can never double-process the same row.
pub async fn claim_specific_transactional(
    pool: &sqlx::PgPool,
    id: Uuid,
    lease_seconds: i64,
) -> Result<Option<ClaimedTransactionalEmail>, sqlx::Error> {
    sqlx::query_as(
        "WITH due AS ( \
            SELECT id FROM email_outbox \
             WHERE id=$2 AND delivery_status='CREATED' AND next_attempt_at<=clock_timestamp() \
               AND (claimed_until IS NULL OR claimed_until<=clock_timestamp()) \
               AND related_entity_type IS NOT NULL \
             FOR UPDATE SKIP LOCKED \
         ) \
         UPDATE email_outbox SET claimed_until = clock_timestamp() + make_interval(secs => $1) \
          WHERE id IN (SELECT id FROM due) \
          RETURNING id, email_type, recipient_email, related_entity_type, related_entity_id, delivery_attempts",
    )
    .bind(lease_seconds)
    .bind(id)
    .fetch_optional(pool)
    .await
}

/// Successful terminal outcome for a claimed row.
pub async fn mark_transactional_sent<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    provider_message_id: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE email_outbox SET delivery_status='SENT_TO_PROVIDER', provider_message_id=$2, \
            claimed_until=NULL, last_delivery_error=NULL, updated_at=now() WHERE id=$1",
    )
    .bind(id)
    .bind(provider_message_id)
    .execute(executor)
    .await?;
    Ok(())
}

/// A `Retryable` outcome, or a resolution failure the worker chooses to
/// retry (e.g. a transient DB error while rendering), under its own attempt
/// budget: releases the lease, bumps the attempt counter, and schedules the
/// next attempt at `next_attempt_at` (exponential backoff is the caller's
/// responsibility to compute, exactly like `notification_scheduler`'s own
/// push-delivery backoff). Row stays `CREATED` — still claimable once due.
pub async fn mark_transactional_retry<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    reason: &str,
    next_attempt_at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE email_outbox SET delivery_attempts=delivery_attempts+1, \
            last_delivery_error=$2, claimed_until=NULL, next_attempt_at=$3, updated_at=now() \
         WHERE id=$1",
    )
    .bind(id)
    .bind(reason.chars().take(500).collect::<String>())
    .bind(next_attempt_at)
    .execute(executor)
    .await?;
    Ok(())
}

/// A `Permanent` provider outcome, or the retry budget exhausted — terminal,
/// never claimed again.
pub async fn mark_transactional_permanent_failed<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    reason: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE email_outbox SET delivery_status='FAILED', delivery_attempts=delivery_attempts+1, \
            last_delivery_error=$2, claimed_until=NULL, updated_at=now() WHERE id=$1",
    )
    .bind(id)
    .bind(reason.chars().take(500).collect::<String>())
    .execute(executor)
    .await?;
    Ok(())
}

/// Records one attempt's outcome without necessarily closing the row out —
/// called after every provider call, including ones that will be retried.
pub async fn record_attempt<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    attempts: i32,
    last_error: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE email_outbox SET delivery_attempts=$2, last_delivery_error=$3, updated_at=now() \
         WHERE id=$1",
    )
    .bind(id)
    .bind(attempts)
    .bind(last_error.map(|e| e.chars().take(500).collect::<String>()))
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn mark_sent<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    provider_message_id: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE email_outbox SET delivery_status='SENT_TO_PROVIDER', provider_message_id=$2, \
            last_delivery_error=NULL, updated_at=now() WHERE id=$1",
    )
    .bind(id)
    .bind(provider_message_id)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn mark_failed<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    reason: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE email_outbox SET delivery_status='FAILED', last_delivery_error=$2, updated_at=now() \
         WHERE id=$1",
    )
    .bind(id)
    .bind(reason.chars().take(500).collect::<String>())
    .execute(executor)
    .await?;
    Ok(())
}

/// Periodic reconciliation only — closes out rows whose underlying OTP
/// challenge expired before the request that created them ever reached a
/// terminal status (e.g. the process crashed mid-send). Never retried: the
/// code itself is gone, and the user's original OTP request has already
/// timed out from their perspective. Returns the number of rows closed.
pub async fn expire_stale<'e, E: PgExecutor<'e>>(executor: E) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE email_outbox SET delivery_status='EXPIRED', updated_at=now() \
         WHERE delivery_status='CREATED' AND expires_at<=clock_timestamp()",
    )
    .execute(executor)
    .await?;
    Ok(result.rows_affected())
}

#[derive(Debug, sqlx::FromRow)]
pub struct EmailOutboxRow {
    pub id: Uuid,
    pub email_type: String,
    pub recipient_email: String,
    pub delivery_status: String,
    pub delivery_attempts: i32,
    pub last_delivery_error: Option<String>,
    pub provider_message_id: Option<String>,
    pub related_entity_type: Option<String>,
    pub related_entity_id: Option<Uuid>,
    pub claimed_until: Option<DateTime<Utc>>,
    pub next_attempt_at: DateTime<Utc>,
}

/// Test/observability helper — not used by any delivery-path code.
pub async fn find_by_dedup_key<'e, E: PgExecutor<'e>>(
    executor: E,
    dedup_key: &str,
) -> Result<Option<EmailOutboxRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, email_type, recipient_email, delivery_status, delivery_attempts, \
                last_delivery_error, provider_message_id, related_entity_type, \
                related_entity_id, claimed_until, next_attempt_at \
         FROM email_outbox WHERE dedup_key=$1",
    )
    .bind(dedup_key)
    .fetch_optional(executor)
    .await
}
