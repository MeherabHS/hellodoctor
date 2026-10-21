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
    let mut savepoint = executor.begin().await?;
    let result = sqlx::query(
        "INSERT INTO notifications \
         (id, user_id, type, title, body, related_entity_type, related_entity_id, \
          scheduled_for, dedup_key, metadata) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
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

/// Scoped by `user_id` as well as `id` — RLS already enforces this at the
/// database layer (see the migration's `notifications_owner_mark_read`
/// policy), but binding it explicitly here too means a caller reading this
/// function can see the ownership boundary without having to go look at the
/// policy, and the query degenerates safely (0 rows affected) if the two
/// layers ever disagree rather than silently relying on just one of them.
pub async fn mark_read<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    user_id: UserId,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE notifications SET read_at = now(), delivery_status = 'READ' \
         WHERE id = $1 AND user_id = $2 AND read_at IS NULL",
    )
    .bind(id)
    .bind(user_id)
    .execute(executor)
    .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn mark_all_read<'e, E: PgExecutor<'e>>(
    executor: E,
    user_id: UserId,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE notifications SET read_at = now(), delivery_status = 'READ' \
         WHERE user_id = $1 AND read_at IS NULL",
    )
    .bind(user_id)
    .execute(executor)
    .await?;
    Ok(result.rows_affected())
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

pub mod device_tokens {
    use crate::domain::ids::UserId;
    use sqlx::PgExecutor;
    use uuid::Uuid;

    pub async fn upsert<'e, E: PgExecutor<'e>>(
        executor: E,
        device_id: Uuid,
        user_id: UserId,
        platform: &str,
        push_token: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO notification_device_tokens \
             (device_id, user_id, platform, push_token, enabled, last_seen_at, updated_at) \
             VALUES ($1,$2,$3,$4,TRUE,now(),now()) \
             ON CONFLICT (user_id, push_token) DO UPDATE SET \
               enabled = TRUE, platform = EXCLUDED.platform, last_seen_at = now(), updated_at = now()",
        )
        .bind(device_id)
        .bind(user_id)
        .bind(platform)
        .bind(push_token)
        .execute(executor)
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

    pub async fn find_enabled_for_user<'e, E: PgExecutor<'e>>(
        executor: E,
        user_id: UserId,
    ) -> Result<Vec<String>, sqlx::Error> {
        sqlx::query_scalar(
            "SELECT push_token FROM notification_device_tokens \
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
