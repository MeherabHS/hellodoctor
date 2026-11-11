use crate::domain::ids::UserId;
use crate::domain::models::Notification;
use chrono::{DateTime, Utc};
use sqlx::{Acquire, PgExecutor, Postgres};
use uuid::Uuid;

/// Creates a notification unless a row with the same `dedup_key` already
/// exists — this is the single mechanism every caller (admin verify/reject,
/// consultation-completion earning credit, the daily-summary/T-15-reminder
/// scheduler) relies on for "exactly one notification per real event,"
/// including safely across a retried request or a re-run scheduler tick.
/// Returns `true` only if a new row was actually inserted.
///
/// Deliberately a plain `INSERT` plus a caught `23505` unique-violation,
/// **not** `INSERT ... ON CONFLICT (dedup_key) DO NOTHING` — confirmed by a
/// real failing test, not assumed: Postgres's `ON CONFLICT` clause needs to
/// resolve whether a conflicting row exists via the table's `SELECT` row
/// security policy, and `notifications`'s `SELECT` policy is owner-scoped
/// (`user_id = app.current_user_id`). Every creation path here writes a
/// notification for *someone else* (an admin verifying a doctor, the system
/// crediting a doctor's wallet, the scheduler acting for no particular
/// session at all) — a case `ON CONFLICT` cannot resolve under this policy
/// regardless of which role/`TO` clause is used, since the row being
/// potentially-conflicted-with is never visible to the *inserting* session's
/// own identity. A plain `INSERT`'s uniqueness violation is caught at the
/// index level, independent of `SELECT` visibility, which is exactly the
/// semantics this needs.
///
/// Takes `impl Acquire<'e, Database = Postgres>` rather than a bare
/// `PgExecutor` and runs the insert inside `executor.begin()` — a second,
/// equally real bug a live test caught (not assumed): a plain `PgPool`
/// auto-commits each statement independently, so catching the 23505 there is
/// enough, but every caller here that's *already inside* a larger
/// transaction (the admin verify/reject handler, the consultation-completion
/// transaction) would otherwise poison that whole outer transaction the
/// moment the unique violation occurred — Postgres aborts the entire
/// transaction on any statement error, and catching the error in Rust does
/// not un-abort it; every later statement in that same transaction (the
/// audit-log write, the final commit) then fails with "current transaction
/// is aborted." `Acquire::begin()` on an existing `Transaction` creates a
/// real `SAVEPOINT`, so rolling back just this one failed insert leaves the
/// outer transaction perfectly usable — while `begin()` on a bare `PgPool`
/// (the scheduler's and the isolated-service-call tests' case) transparently
/// behaves as an ordinary top-level transaction instead.
#[allow(clippy::too_many_arguments)]
pub async fn insert_if_new<'e, A>(
    executor: A,
    id: Uuid,
    user_id: UserId,
    notification_type: &str,
    title: &str,
    body: &str,
    related_entity_type: Option<&str>,
    related_entity_id: Option<Uuid>,
    scheduled_for: Option<DateTime<Utc>>,
    dedup_key: &str,
    metadata: Option<serde_json::Value>,
) -> Result<bool, sqlx::Error>
where
    A: Acquire<'e, Database = Postgres> + Send,
{
    insert_if_new_with_ttl(
        executor,
        id,
        user_id,
        notification_type,
        title,
        body,
        related_entity_type,
        related_entity_id,
        scheduled_for,
        dedup_key,
        metadata,
        None,
    )
    .await
}

/// Same as [`insert_if_new`], but additionally sets `ttl_seconds` — for an
/// event whose usefulness genuinely expires (e.g. a Rejoin invitation for a
/// grace period that has already closed, or a Ready-to-join alert for a
/// consultation window that has already ended), so a delivery delayed past
/// that point is dropped by the provider/OS instead of misleadingly still
/// inviting the recipient to act. `None` (every existing caller via
/// [`insert_if_new`]) falls back to the transport's own default.
#[allow(clippy::too_many_arguments)]
pub async fn insert_if_new_with_ttl<'e, A>(
    executor: A,
    id: Uuid,
    user_id: UserId,
    notification_type: &str,
    title: &str,
    body: &str,
    related_entity_type: Option<&str>,
    related_entity_id: Option<Uuid>,
    scheduled_for: Option<DateTime<Utc>>,
    dedup_key: &str,
    metadata: Option<serde_json::Value>,
    ttl_seconds: Option<i32>,
) -> Result<bool, sqlx::Error>
where
    A: Acquire<'e, Database = Postgres> + Send,
{
    let mut savepoint = executor.begin().await?;
    let result = sqlx::query(
        "INSERT INTO notifications \
         (id, user_id, type, title, body, related_entity_type, related_entity_id, \
          scheduled_for, dedup_key, metadata, ttl_seconds) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)",
    )
    .bind(id)
    .bind(user_id)
    .bind(notification_type)
    .bind(title)
    .bind(body)
    .bind(related_entity_type)
    .bind(related_entity_id)
    .bind(scheduled_for)
    .bind(dedup_key)
    .bind(metadata)
    .bind(ttl_seconds)
    .execute(&mut *savepoint)
    .await;

    match result {
        Ok(_) => {
            savepoint.commit().await?;
            Ok(true)
        }
        Err(sqlx::Error::Database(e)) if e.code().as_deref() == Some("23505") => {
            savepoint.rollback().await?;
            Ok(false)
        }
        Err(e) => {
            let _ = savepoint.rollback().await;
            Err(e)
        }
    }
}

pub async fn list_for_user<'e, E: PgExecutor<'e>>(
    executor: E,
    user_id: UserId,
    limit: i64,
    offset: i64,
) -> Result<Vec<Notification>, sqlx::Error> {
    sqlx::query_as::<_, Notification>(
        "SELECT * FROM notifications WHERE user_id = $1 \
         ORDER BY created_at DESC,id DESC LIMIT $2 OFFSET $3",
    )
    .bind(user_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(executor)
    .await
}

pub async fn count_unread<'e, E: PgExecutor<'e>>(
    executor: E,
    user_id: UserId,
) -> Result<i64, sqlx::Error> {
    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM notifications WHERE user_id = $1 AND read_at IS NULL")
            .bind(user_id)
            .fetch_one(executor)
            .await?;
    Ok(count)
}

/// Reading an inbox message cancels a pending push, preserving the previous
/// behavior, but never replaces a real provider/terminal outcome with READ.
/// Claim fencing and read_at commit atomically under the owner's RLS policy.
pub async fn mark_read<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    user_id: UserId,
) -> Result<bool, sqlx::Error> {
    Ok(mark_owned_read(executor, user_id, Some(id)).await? > 0)
}

pub async fn mark_all_read<'e, E: PgExecutor<'e>>(
    executor: E,
    user_id: UserId,
) -> Result<u64, sqlx::Error> {
    mark_owned_read(executor, user_id, None).await
}

async fn mark_owned_read<'e, E: PgExecutor<'e>>(
    executor: E,
    user_id: UserId,
    id: Option<Uuid>,
) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query(
        "WITH targets AS MATERIALIZED ( \
           SELECT id,(delivery_status IN ('CREATED','DUE') OR delivery_status='FAILED' AND next_delivery_at IS NOT NULL) AS pending \
           FROM notifications WHERE user_id=$1 AND read_at IS NULL AND ($2::uuid IS NULL OR id=$2) FOR UPDATE \
         ) UPDATE notifications n SET read_at=clock_timestamp(), \
           delivery_status=CASE WHEN t.pending THEN 'SUPPRESSED' ELSE n.delivery_status END, \
           last_delivery_error=CASE WHEN t.pending THEN 'Suppressed: inbox notification already read' ELSE n.last_delivery_error END, \
           next_delivery_at=CASE WHEN t.pending THEN NULL ELSE n.next_delivery_at END, \
           delivery_claim_token=CASE WHEN t.pending THEN NULL ELSE n.delivery_claim_token END, \
           delivery_lease_until=CASE WHEN t.pending THEN NULL ELSE n.delivery_lease_until END \
         FROM targets t WHERE n.id=t.id",
    ).bind(user_id).bind(id).execute(executor).await?.rows_affected())
}

/// True only if a `CREATED`/`DUE` (i.e. not yet delivered/read) reminder
/// already exists for this exact dedup key — used by the scheduler to decide
/// whether it still needs to act, distinct from `insert_if_new`'s own
/// conflict check, for call sites that want to check-without-writing.
pub async fn exists_with_dedup_key<'e, E: PgExecutor<'e>>(
    executor: E,
    dedup_key: &str,
) -> Result<bool, sqlx::Error> {
    let (exists,): (bool,) =
        sqlx::query_as("SELECT EXISTS(SELECT 1 FROM notifications WHERE dedup_key = $1)")
            .bind(dedup_key)
            .fetch_one(executor)
            .await?;
    Ok(exists)
}

/// Deletes any still-pending (`CREATED`/`DUE` — i.e. never actually
/// delivered/read) T-15 reminder for a specific appointment, keyed by the
/// dedup-key prefix `consultation-reminder-15m:{appointment_id}:`. Called
/// when an appointment is cancelled so a reminder already queued for it does
/// not later surface in the doctor's in-app inbox for a consult that no
/// longer exists. A reminder that already reached `DELIVERED`/`READ` is left
/// alone — the doctor already saw it, and deleting history after the fact
/// would be a worse experience than leaving one stale item in the inbox.
pub async fn cancel_pending_reminders_for_appointment<'e, E: PgExecutor<'e>>(
    executor: E,
    appointment_id: Uuid,
) -> Result<u64, sqlx::Error> {
    let prefix = format!("consultation-reminder-15m:{appointment_id}:%");
    let result = sqlx::query(
        "DELETE FROM notifications \
         WHERE dedup_key LIKE $1 AND delivery_status IN ('CREATED', 'DUE')",
    )
    .bind(prefix)
    .execute(executor)
    .await?;
    Ok(result.rows_affected())
}

/// Called only after the campaign has atomically moved to CANCELLED. Preserve
/// attempted outcomes and fence their claims; keep existing removal semantics
/// for inbox entries which were never attempted. RLS is scoped to that campaign.
pub async fn cancel_campaign_deliveries(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<(), sqlx::Error> {
    // Preserve attempted rows (and any uncertain provider outcome), while
    // fencing in-flight/stale worker completions. Never erase send history.
    sqlx::query(
            "UPDATE notifications SET delivery_status='SUPPRESSED', \
                last_delivery_error='Suppressed: campaign cancelled',next_delivery_at=NULL, \
                delivery_claim_token=NULL,delivery_lease_until=NULL \
             WHERE related_entity_type='CAMPAIGN' AND related_entity_id=$1 \
               AND (delivery_status='DUE' OR delivery_status='FAILED' AND next_delivery_at IS NOT NULL)",
        ).bind(id).execute(&mut **tx).await?;
    sqlx::query(
        "DELETE FROM notifications WHERE related_entity_type='CAMPAIGN' AND related_entity_id=$1 \
             AND delivery_status='CREATED'",
    )
    .bind(id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Private worker data. Claim tokens are never part of the inbox DTO.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DueNotification {
    pub id: Uuid,
    pub user_id: UserId,
    #[sqlx(rename = "type")]
    pub notification_type: String,
    pub title: String,
    pub body: String,
    pub related_entity_type: Option<String>,
    pub related_entity_id: Option<Uuid>,
    pub dedup_key: String,
    pub delivery_attempts: i32,
    pub collapse_key: Option<String>,
    pub ttl_seconds: Option<i32>,
    pub frequency_claimed: bool,
    pub delivery_claim_token: Uuid,
    pub delivery_uncertain: bool,
}

/// A session advisory lock spans provider IO, NOT a database transaction.
/// Every DB operation uses this same system-role connection, so a worker does
/// not reserve one connection and then deadlock waiting for a second pool slot.
/// Cancellation/panic closes the connection rather than returning a locked
/// session to the pool. Normal release unlocks before recycling the connection.
pub struct DeliveryClaim {
    pub row: DueNotification,
    connection: DeliveryConnection,
}

struct DeliveryConnection(Option<sqlx::pool::PoolConnection<Postgres>>);

impl Drop for DeliveryConnection {
    fn drop(&mut self) {
        if let Some(connection) = self.0.as_mut() {
            connection.close_on_drop();
        }
    }
}

impl DeliveryClaim {
    pub fn connection(&mut self) -> &mut sqlx::PgConnection {
        self.connection
            .0
            .as_mut()
            .expect("live delivery connection")
    }

    pub async fn release(mut self) -> Result<(), sqlx::Error> {
        let id = self.row.id;
        let unlocked: bool =
            sqlx::query_scalar("SELECT pg_advisory_unlock(hashtextextended($1::text, 7314961))")
                .bind(id)
                .fetch_one(self.connection())
                .await?;
        if !unlocked {
            return Err(sqlx::Error::Protocol(
                "Notification delivery lock was lost".into(),
            ));
        }
        // Only an acknowledged unlock permits session reuse.
        drop(self.connection.0.take());
        Ok(())
    }
}

/// Claim just-in-time, never lease a sequential batch in advance. The
/// MATERIALIZED single-candidate CTE bounds session lock acquisition to one
/// row. A live worker retains exclusivity even if its durable lease expires.
/// FAILED + NULL retry time is terminal, including historical suppressed rows.
/// Expired DUE at the retry limit is reclaimed only to record terminal failure.
pub async fn claim_next_notification(
    pool: &sqlx::PgPool,
) -> Result<Option<DeliveryClaim>, sqlx::Error> {
    let connection = tokio::time::timeout(std::time::Duration::from_millis(250), pool.acquire())
        .await
        .map_err(|_| sqlx::Error::PoolTimedOut)??;
    let mut guard = DeliveryConnection(Some(connection));
    let token = Uuid::new_v4();
    let row = sqlx::query_as::<_, DueNotification>(
        "WITH due AS MATERIALIZED ( \
            SELECT id FROM notifications \
             WHERE (delivery_status='CREATED' AND delivery_attempts<5 \
                    OR delivery_status='FAILED' AND next_delivery_at IS NOT NULL AND delivery_attempts<5 \
                    OR delivery_status='DUE') \
               AND (scheduled_for IS NULL OR scheduled_for<=clock_timestamp()) \
               AND (CASE WHEN delivery_status='DUE' THEN COALESCE(delivery_lease_until,next_delivery_at) \
                         ELSE next_delivery_at END IS NULL \
                    OR CASE WHEN delivery_status='DUE' THEN COALESCE(delivery_lease_until,next_delivery_at) \
                            ELSE next_delivery_at END<=clock_timestamp()) \
             ORDER BY COALESCE(scheduled_for,created_at),created_at,id \
             FOR UPDATE SKIP LOCKED LIMIT 1 \
        ), owned AS MATERIALIZED ( \
            SELECT id FROM due WHERE pg_try_advisory_lock(hashtextextended(id::text,7314961)) \
        ) \
        UPDATE notifications n SET delivery_status='DUE', \
            delivery_attempts=n.delivery_attempts+1, delivery_claim_token=$1, \
            delivery_lease_until=clock_timestamp()+interval '2 minutes', next_delivery_at=NULL \
         FROM owned WHERE n.id=owned.id \
         RETURNING n.id,n.user_id,n.type,n.title,n.body,n.related_entity_type,n.related_entity_id, \
                   n.dedup_key,n.delivery_attempts,n.collapse_key,n.ttl_seconds,n.frequency_claimed, \
                   n.delivery_claim_token,n.delivery_uncertain",
    )
    .bind(token)
    .fetch_optional(&mut **guard.0.as_mut().expect("live delivery connection"))
    .await?;
    match row {
        Some(row) => Ok(Some(DeliveryClaim {
            row,
            connection: guard,
        })),
        None => {
            drop(guard.0.take());
            Ok(None)
        }
    }
}

/// An expired lease can be renewed only by the session still holding its
/// advisory lock; another connection cannot revive an abandoned claim.
pub async fn renew_delivery_claim<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    token: Uuid,
) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query(
        "UPDATE notifications SET delivery_lease_until=clock_timestamp()+interval '2 minutes' \
         WHERE id=$1 AND delivery_status='DUE' AND delivery_claim_token=$2 \
           AND (delivery_lease_until>clock_timestamp() OR EXISTS ( \
             SELECT 1 FROM pg_locks l WHERE l.locktype='advisory' AND l.pid=pg_backend_pid() AND l.granted \
               AND l.objsubid=1 \
               AND l.classid=((hashtextextended($1::text,7314961)>>32)&4294967295)::oid \
               AND l.objid=(hashtextextended($1::text,7314961)&4294967295)::oid))",
    )
    .bind(id)
    .bind(token)
    .execute(executor)
    .await?
    .rows_affected()
        == 1)
}

/// Durable uncertainty marker BEFORE contacting the provider. A crash, timeout,
/// or lost connection cannot be mistaken for a definitely unsent notification.
pub async fn begin_provider_send<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    token: Uuid,
) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query(
        "UPDATE notifications SET delivery_uncertain=TRUE WHERE id=$1 \
         AND delivery_status='DUE' AND delivery_claim_token=$2 AND delivery_lease_until>clock_timestamp()",
    ).bind(id).bind(token).execute(executor).await?.rows_affected()==1)
}

pub async fn record_definite_rejection<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    token: Uuid,
    previously_uncertain: bool,
) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query(
        "UPDATE notifications SET delivery_uncertain=$3 WHERE id=$1 \
         AND delivery_status='DUE' AND delivery_claim_token=$2 AND delivery_lease_until>clock_timestamp()",
    ).bind(id).bind(token).bind(previously_uncertain).execute(executor).await?.rows_affected()==1)
}

pub async fn mark_delivery_sent<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    token: Uuid,
    provider_message_id: Option<&str>,
) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query(
        "UPDATE notifications SET delivery_status='SENT_TO_PROVIDER', \
                delivered_at=clock_timestamp(),provider_message_id=$3, \
                next_delivery_at=NULL,last_delivery_error=NULL,delivery_claim_token=NULL, \
                delivery_lease_until=NULL,delivery_uncertain=FALSE \
         WHERE id=$1 AND delivery_status='DUE' AND delivery_claim_token=$2 AND delivery_lease_until>clock_timestamp()",
    ).bind(id).bind(token).bind(provider_message_id).execute(executor).await?.rows_affected()==1)
}

pub async fn mark_delivery_retry<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    token: Uuid,
    error: &str,
    next_delivery_at: DateTime<Utc>,
) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query(
        "UPDATE notifications SET delivery_status='FAILED',last_delivery_error=$3,next_delivery_at=$4, \
            delivery_claim_token=NULL,delivery_lease_until=NULL \
         WHERE id=$1 AND delivery_status='DUE' AND delivery_claim_token=$2 AND delivery_lease_until>clock_timestamp()",
    ).bind(id).bind(token).bind(error.chars().take(500).collect::<String>()).bind(next_delivery_at)
        .execute(executor).await?.rows_affected()==1)
}

pub async fn mark_delivery_permanent<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    token: Uuid,
    error: &str,
) -> Result<bool, sqlx::Error> {
    finish_without_send(executor, id, token, "FAILED", error).await
}

pub async fn mark_delivery_suppressed<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    token: Uuid,
    reason: &str,
) -> Result<bool, sqlx::Error> {
    finish_without_send(executor, id, token, "SUPPRESSED", reason).await
}

async fn finish_without_send<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    token: Uuid,
    status: &str,
    reason: &str,
) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query(
        "UPDATE notifications SET delivery_status=$3,last_delivery_error=$4,next_delivery_at=NULL, \
            delivery_claim_token=NULL,delivery_lease_until=NULL \
         WHERE id=$1 AND delivery_status='DUE' AND delivery_claim_token=$2 AND delivery_lease_until>clock_timestamp()",
    ).bind(id).bind(token).bind(status).bind(reason.chars().take(500).collect::<String>())
        .execute(executor).await?.rows_affected()==1)
}

/// Deferral before provider IO consumes no attempt. An interrupted in-flight
/// request retains its attempt and uncertainty, so deferral cannot reset retries.
pub async fn mark_delivery_deferred<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    token: Uuid,
    next_delivery_at: DateTime<Utc>,
    reason: &str,
    provider_started: bool,
) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query(
        "UPDATE notifications SET delivery_status='FAILED',last_delivery_error=$4,next_delivery_at=$3, \
            delivery_attempts=GREATEST(delivery_attempts-CASE WHEN $5 THEN 0 ELSE 1 END,0),delivery_claim_token=NULL,delivery_lease_until=NULL \
         WHERE id=$1 AND delivery_status='DUE' AND delivery_claim_token=$2 AND delivery_lease_until>clock_timestamp()",
    ).bind(id).bind(token).bind(next_delivery_at).bind(reason).bind(provider_started)
        .execute(executor).await?.rows_affected()==1)
}

#[derive(Debug, sqlx::FromRow)]
pub struct DeliveryQueueStats {
    pub pending: i64,
    pub processing: i64,
    pub retrying: i64,
    pub deferred: i64,
    pub suppressed: i64,
    pub sent_to_provider: i64,
    pub permanently_failed: i64,
    pub uncertain: i64,
    pub confirmed_delivered: i64,
    pub legacy_read: i64,
}

pub async fn delivery_queue_stats<'e, E: PgExecutor<'e>>(
    executor: E,
) -> Result<DeliveryQueueStats, sqlx::Error> {
    sqlx::query_as(
        "SELECT \
          COUNT(*) FILTER(WHERE delivery_status='CREATED') pending, \
          COUNT(*) FILTER(WHERE delivery_status='DUE') processing, \
          COUNT(*) FILTER(WHERE delivery_status='FAILED' AND next_delivery_at IS NOT NULL AND delivery_attempts<5 AND COALESCE(last_delivery_error,'') NOT LIKE 'Deferred:%') retrying, \
          COUNT(*) FILTER(WHERE delivery_status='FAILED' AND next_delivery_at IS NOT NULL AND delivery_attempts<5 AND last_delivery_error LIKE 'Deferred:%') deferred, \
          COUNT(*) FILTER(WHERE delivery_status='SUPPRESSED' OR delivery_status='FAILED' AND next_delivery_at IS NULL AND last_delivery_error LIKE 'Suppressed:%') suppressed, \
          COUNT(*) FILTER(WHERE delivery_status='SENT_TO_PROVIDER') sent_to_provider, \
          COUNT(*) FILTER(WHERE delivery_status='FAILED' AND (next_delivery_at IS NULL OR delivery_attempts>=5) AND COALESCE(last_delivery_error,'') NOT LIKE 'Suppressed:%') permanently_failed, \
          COUNT(*) FILTER(WHERE delivery_status='DELIVERED') confirmed_delivered, \
          COUNT(*) FILTER(WHERE delivery_status='READ') legacy_read, \
          COUNT(*) FILTER(WHERE delivery_uncertain) uncertain FROM notifications",
    ).fetch_one(executor).await
}

/// The gating facts for one notification type — `category` distinguishes
/// SECURITY/TRANSACTIONAL/CLINICAL (never subject to preference, quiet-hours
/// or frequency suppression — see `notification_scheduler.rs`'s delivery
/// gate) from OPERATIONAL/ENGAGEMENT (the only two categories this applies
/// to). Falls back to a conservative always-eligible default
/// (`category = "TRANSACTIONAL"`) if a type was somehow never seeded into
/// the registry, so a missing registry row can never silently suppress a
/// real notification.
#[derive(Debug, sqlx::FromRow)]
pub struct NotificationTypeGate {
    pub category: String,
    pub quiet_hours_allowed: bool,
    pub max_per_day: i32,
}

pub async fn notification_type_gate<'e, E: PgExecutor<'e>>(
    executor: E,
    notification_type: &str,
) -> Result<NotificationTypeGate, sqlx::Error> {
    let row: Option<NotificationTypeGate> = sqlx::query_as(
        "SELECT category, quiet_hours_allowed, max_per_day \
           FROM notification_type_registry WHERE notification_type = $1",
    )
    .bind(notification_type)
    .fetch_optional(executor)
    .await?;
    Ok(row.unwrap_or(NotificationTypeGate {
        category: "TRANSACTIONAL".to_string(),
        quiet_hours_allowed: false,
        max_per_day: 20,
    }))
}

/// A user's own delivery preferences, defaulted identically to a user with
/// no row at all (`notification_preferences` is only ever written once a
/// user actually opens notification settings) — see
/// `migrations/20261122000001_milestone3_localization_preferences`'s own
/// column defaults, which this mirrors exactly for the first five fields.
///
/// `greeting_notifications_enabled`/`religious_greetings_enabled`
/// (Milestone 4/5) are role-aware/strict-by-default and computed directly
/// in `user_notification_preferences`'s own query (via `COALESCE` against
/// the joined `users.role`) rather than here, since a plain Rust-level
/// `unwrap_or` has no access to the user's role.
#[derive(Debug, sqlx::FromRow)]
pub struct NotificationPreferences {
    pub operational_enabled: bool,
    pub engagement_enabled: bool,
    pub quiet_hours_enabled: bool,
    pub quiet_hours_start_local: chrono::NaiveTime,
    pub quiet_hours_end_local: chrono::NaiveTime,
    pub greeting_notifications_enabled: bool,
    pub religious_greetings_enabled: bool,
}

/// Role-aware default for a user with no `notification_preferences` row yet:
/// Doctors (and any non-Patient role) default to greetings ON, Patients
/// default OFF — "Patients should have greetings disabled by default" is a
/// stricter, separate axis from the general `engagement_enabled` default
/// (which stays on for everyone). Once a row exists, its own stored value is
/// used verbatim regardless of role. `religious_greetings_enabled` has no
/// role-aware branch at all — it is FALSE for every role until explicitly
/// opted into, since inferring religious preference from role (or anything
/// else) is exactly what this milestone's instructions forbid.
///
/// Reads `users.role`, so the caller's executor needs `SELECT` on that
/// column: `hellodoctor_api` already has it (own row only, via RLS) and
/// `hellodoctor_system_worker` has an unconditional full-row grant — the
/// two roles that ever call this.
pub async fn user_notification_preferences<'e, E: PgExecutor<'e>>(
    executor: E,
    user_id: UserId,
) -> Result<NotificationPreferences, sqlx::Error> {
    let row: Option<NotificationPreferences> = sqlx::query_as(
        "SELECT COALESCE(np.operational_enabled, TRUE) AS operational_enabled, \
                COALESCE(np.engagement_enabled, TRUE) AS engagement_enabled, \
                COALESCE(np.quiet_hours_enabled, FALSE) AS quiet_hours_enabled, \
                COALESCE(np.quiet_hours_start_local, TIME '22:00') AS quiet_hours_start_local, \
                COALESCE(np.quiet_hours_end_local, TIME '07:00') AS quiet_hours_end_local, \
                COALESCE(np.greeting_notifications_enabled, u.role <> 'PATIENT') AS greeting_notifications_enabled, \
                COALESCE(np.religious_greetings_enabled, FALSE) AS religious_greetings_enabled \
           FROM users u \
           LEFT JOIN notification_preferences np ON np.user_id = u.id \
          WHERE u.id = $1",
    )
    .bind(user_id)
    .fetch_optional(executor)
    .await?;
    Ok(row.unwrap_or(NotificationPreferences {
        operational_enabled: true,
        engagement_enabled: true,
        quiet_hours_enabled: false,
        quiet_hours_start_local: chrono::NaiveTime::from_hms_opt(22, 0, 0).unwrap(),
        quiet_hours_end_local: chrono::NaiveTime::from_hms_opt(7, 0, 0).unwrap(),
        // Only reachable if `user_id` resolves to no `users` row at all —
        // should never happen for a real authenticated caller, so this
        // strictest-possible fallback (greetings off) is purely defensive.
        greeting_notifications_enabled: false,
        religious_greetings_enabled: false,
    }))
}

/// Creates or replaces the caller's own preference row. Only ever called
/// from within a `begin_authed` transaction for the owning user — the
/// `notification_preferences_owner_insert`/`_owner_update` RLS policies
/// require `app.current_user_id` to already equal `user_id`, and `ON
/// CONFLICT`'s implicit `SELECT` needs that same session identity to see its
/// own existing row (the same RLS/`ON CONFLICT` interaction
/// `insert_if_new`'s own doc comment explains — safe here specifically
/// because this always writes the caller's *own* row, never someone else's).
#[allow(clippy::too_many_arguments)]
pub async fn upsert_preferences<'e, E: PgExecutor<'e>>(
    executor: E,
    user_id: UserId,
    operational_enabled: bool,
    engagement_enabled: bool,
    quiet_hours_enabled: bool,
    quiet_hours_start_local: chrono::NaiveTime,
    quiet_hours_end_local: chrono::NaiveTime,
    greeting_notifications_enabled: bool,
    religious_greetings_enabled: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO notification_preferences \
            (user_id, operational_enabled, engagement_enabled, quiet_hours_enabled, \
             quiet_hours_start_local, quiet_hours_end_local, \
             greeting_notifications_enabled, religious_greetings_enabled, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, clock_timestamp()) \
         ON CONFLICT (user_id) DO UPDATE SET \
            operational_enabled = EXCLUDED.operational_enabled, \
            engagement_enabled = EXCLUDED.engagement_enabled, \
            quiet_hours_enabled = EXCLUDED.quiet_hours_enabled, \
            quiet_hours_start_local = EXCLUDED.quiet_hours_start_local, \
            quiet_hours_end_local = EXCLUDED.quiet_hours_end_local, \
            greeting_notifications_enabled = EXCLUDED.greeting_notifications_enabled, \
            religious_greetings_enabled = EXCLUDED.religious_greetings_enabled, \
            updated_at = clock_timestamp()",
    )
    .bind(user_id)
    .bind(operational_enabled)
    .bind(engagement_enabled)
    .bind(quiet_hours_enabled)
    .bind(quiet_hours_start_local)
    .bind(quiet_hours_end_local)
    .bind(greeting_notifications_enabled)
    .bind(religious_greetings_enabled)
    .execute(executor)
    .await?;
    Ok(())
}

/// Atomically claims one daily send slot for `(user_id, notification_type,
/// local_date)` against `max_per_day`, returning `true` iff the claim
/// succeeded. Safe under any number of concurrent delivery-pass workers: the
/// `INSERT ... ON CONFLICT DO UPDATE ... WHERE ... RETURNING` is a single
/// statement, so Postgres's own row lock on the conflicting key serializes
/// concurrent claims against the same user+type+day — unlike a read-then-act
/// `SELECT COUNT(*)` check (this milestone's own Milestone 3 original
/// design), which two truly concurrent workers could both pass before
/// either commits. See `notifications.frequency_claimed` for how a single
/// notification row stays idempotent across delivery retries so a retried
/// send never claims a second slot for what is logically one notification.
pub async fn claim_frequency_slot<'e, E: PgExecutor<'e>>(
    executor: E,
    user_id: UserId,
    notification_type: &str,
    local_date: chrono::NaiveDate,
    max_per_day: i32,
) -> Result<bool, sqlx::Error> {
    let claimed: Option<(i32,)> = sqlx::query_as(
        "INSERT INTO notification_frequency_counters (user_id, notification_type, local_date, sent_count) \
         VALUES ($1, $2, $3, 1) \
         ON CONFLICT (user_id, notification_type, local_date) DO UPDATE SET \
            sent_count = notification_frequency_counters.sent_count + 1 \
         WHERE notification_frequency_counters.sent_count < $4 \
         RETURNING sent_count",
    )
    .bind(user_id)
    .bind(notification_type)
    .bind(local_date)
    .bind(max_per_day)
    .fetch_optional(executor)
    .await?;
    Ok(claimed.is_some())
}

/// Marks a notification row as having already resolved its frequency-cap
/// claim (whether that claim succeeded or the row was suppressed) — once
/// set, `run_push_delivery_pass` never re-evaluates the frequency gate for
/// this exact row again, so a transient-failure retry of an already-allowed
/// send can never claim a second daily slot for the same logical
/// notification.
pub async fn mark_frequency_claimed<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    token: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE notifications SET frequency_claimed=TRUE WHERE id=$1 AND delivery_claim_token=$2 AND delivery_status='DUE' AND delivery_lease_until>clock_timestamp()")
        .bind(id)
        .bind(token)
        .execute(executor)
        .await?;
    Ok(())
}

/// One row of the maintained Kenyan holiday calendar
/// (`migrations/20261124000001_milestone4_5_engagement_and_holidays`).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct KenyanHoliday {
    pub id: Uuid,
    pub holiday_key: String,
    pub is_religious: bool,
}

/// Holidays observed on this *exact* Nairobi-local date — strict equality,
/// never `<=`, so a scheduler that was down on the real day never "catches
/// up" and sends a holiday greeting late (this milestone's own "prevent
/// historical events from being sent late after downtime" requirement).
pub async fn holidays_observed_on<'e, E: PgExecutor<'e>>(
    executor: E,
    local_date: chrono::NaiveDate,
) -> Result<Vec<KenyanHoliday>, sqlx::Error> {
    sqlx::query_as::<_, KenyanHoliday>(
        "SELECT id, holiday_key, is_religious FROM kenyan_holidays WHERE observed_date = $1",
    )
    .bind(local_date)
    .fetch_all(executor)
    .await
}

/// Looks up whether a specific `kenyan_holidays` row (by id) is
/// religious-origin — used by the delivery gate to decide whether a claimed
/// `HOLIDAY_GREETING` row additionally needs `religious_greetings_enabled`.
/// `None` only if the holiday row was somehow deleted after the
/// notification referencing it was created; treated as non-religious (the
/// permissive case) since there is nothing left to gate against.
pub async fn holiday_is_religious<'e, E: PgExecutor<'e>>(
    executor: E,
    holiday_id: Uuid,
) -> Result<Option<bool>, sqlx::Error> {
    sqlx::query_scalar("SELECT is_religious FROM kenyan_holidays WHERE id = $1")
        .bind(holiday_id)
        .fetch_optional(executor)
        .await
}

pub mod device_tokens {
    use crate::domain::ids::UserId;
    use sqlx::{PgConnection, PgExecutor};
    use uuid::Uuid;

    pub async fn upsert(
        executor: &mut PgConnection,
        device_id: Uuid,
        user_id: UserId,
        platform: &str,
        push_token: &str,
    ) -> Result<(), sqlx::Error> {
        let updated = sqlx::query(
            "UPDATE notification_device_tokens SET push_token=$4, platform=$3,\
                    enabled=TRUE, last_seen_at=clock_timestamp(), updated_at=clock_timestamp() \
               WHERE device_id=$1 AND user_id=$2",
        )
        .bind(device_id)
        .bind(user_id)
        .bind(platform)
        .bind(push_token)
        .execute(&mut *executor)
        .await?;
        if updated.rows_affected() > 0 {
            return Ok(());
        }
        sqlx::query(
            "INSERT INTO notification_device_tokens \
             (device_id, user_id, platform, push_token, enabled, last_seen_at, updated_at) \
             VALUES ($1,$2,$3,$4,TRUE,now(),now()) \
             ON CONFLICT (user_id, push_token) DO UPDATE SET \
               device_id = EXCLUDED.device_id, enabled = TRUE,\
               platform = EXCLUDED.platform, last_seen_at = now(), updated_at = now()",
        )
        .bind(device_id)
        .bind(user_id)
        .bind(platform)
        .bind(push_token)
        .execute(&mut *executor)
        .await?;
        Ok(())
    }

    /// Disables every device registration for a user — called on logout, so a
    /// revoked session's device no longer receives push for this account.
    /// Never deletes the row (keeps the registration history for the
    /// eventual real-provider integration to re-enable on a fresh login).
    pub async fn disable_all_for_user<'e, E: PgExecutor<'e>>(
        executor: E,
        user_id: UserId,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE notification_device_tokens SET enabled = FALSE, updated_at = now() \
             WHERE user_id = $1",
        )
        .bind(user_id)
        .execute(executor)
        .await?;
        Ok(())
    }

    pub async fn disable_device<'e, E: PgExecutor<'e>>(
        executor: E,
        user_id: UserId,
        device_id: Option<Uuid>,
        push_token: Option<&str>,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE notification_device_tokens SET enabled=FALSE, updated_at=clock_timestamp() \
               WHERE user_id=$1 AND (($2::uuid IS NOT NULL AND device_id=$2)\
                                  OR ($3::text IS NOT NULL AND push_token=$3))",
        )
        .bind(user_id)
        .bind(device_id)
        .bind(push_token)
        .execute(executor)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn token_owned_by_other_user<'e, E: PgExecutor<'e>>(
        executor: E,
        push_token: &str,
        user_id: UserId,
    ) -> Result<bool, sqlx::Error> {
        let owner: Option<UserId> = sqlx::query_scalar(
            "SELECT user_id FROM notification_device_tokens WHERE push_token=$1 AND user_id<>$2 LIMIT 1",
        )
        .bind(push_token)
        .bind(user_id)
        .fetch_optional(executor)
        .await?;
        Ok(owner.is_some())
    }

    pub async fn find_enabled_for_user<'e, E: PgExecutor<'e>>(
        executor: E,
        user_id: UserId,
    ) -> Result<Vec<(Uuid, String)>, sqlx::Error> {
        sqlx::query_as(
            "SELECT device_id, push_token FROM notification_device_tokens \
             WHERE user_id = $1 AND enabled = TRUE",
        )
        .bind(user_id)
        .fetch_all(executor)
        .await
    }
}

pub async fn list_cursor<'e, E: PgExecutor<'e>>(
    executor: E,
    user_id: UserId,
    limit: i64,
    time: chrono::DateTime<chrono::Utc>,
    id: Uuid,
    before: bool,
) -> Result<Vec<Notification>, sqlx::Error> {
    let query = if before {
        "SELECT * FROM notifications WHERE user_id=$1 AND (created_at,id)<($3,$4) ORDER BY created_at DESC,id DESC LIMIT $2"
    } else {
        "SELECT * FROM notifications WHERE user_id=$1 AND (created_at,id)>($3,$4) ORDER BY created_at ASC,id ASC LIMIT $2"
    };
    sqlx::query_as::<_, Notification>(query)
        .bind(user_id)
        .bind(limit)
        .bind(time)
        .bind(id)
        .fetch_all(executor)
        .await
}

/// Navigation resolves only the authenticated recipient's own inbox row.
/// The typed DTO excludes claim/lease/provider internals from serialization.
pub async fn find_for_user<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    user_id: UserId,
) -> Result<Option<Notification>, sqlx::Error> {
    sqlx::query_as("SELECT * FROM notifications WHERE id=$1 AND user_id=$2")
        .bind(id)
        .bind(user_id)
        .fetch_optional(executor)
        .await
}
