use crate::domain::models::SystemErrorLog;
use chrono::{DateTime, Utc};
use sqlx::PgExecutor;
use uuid::Uuid;

/// Find an existing OPEN row with the same fingerprint reported within the
/// dedup window, so a recurring failure increments one counter instead of
/// flooding the admin list with one row per occurrence.
pub async fn find_open_by_fingerprint<'e, E: PgExecutor<'e>>(
    executor: E,
    fingerprint: &str,
    since: DateTime<Utc>,
) -> Result<Option<SystemErrorLog>, sqlx::Error> {
    sqlx::query_as::<_, SystemErrorLog>(
        "SELECT * FROM system_error_logs \
         WHERE fingerprint = $1 AND status = 'OPEN' AND last_seen_at > $2 \
         ORDER BY last_seen_at DESC LIMIT 1",
    )
    .bind(fingerprint)
    .bind(since)
    .fetch_optional(executor)
    .await
}

pub async fn bump_occurrence<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    occurred_at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE system_error_logs SET occurrence_count = occurrence_count + 1, last_seen_at = $2 \
         WHERE id = $1",
    )
    .bind(id)
    .bind(occurred_at)
    .execute(executor)
    .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn insert<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    trace_id: &str,
    subsystem: &str,
    component: &str,
    severity: &str,
    action_attempted: &str,
    error_message: &str,
    stack_trace: Option<&str>,
    impacted_user_id: Option<Uuid>,
    error_code: Option<&str>,
    fingerprint: &str,
    http_status: Option<i16>,
    endpoint: Option<&str>,
    client_context: Option<&serde_json::Value>,
    occurred_at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO system_error_logs \
         (id, trace_id, subsystem, component, severity, action_attempted, error_message, \
          stack_trace, impacted_user_id, error_code, fingerprint, http_status, endpoint, \
          occurrence_count, first_seen_at, last_seen_at, status, logged_at, client_context) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,1,$14,$14,'OPEN',$14,$15)",
    )
    .bind(id)
    .bind(trace_id)
    .bind(subsystem)
    .bind(component)
    .bind(severity)
    .bind(action_attempted)
    .bind(error_message)
    .bind(stack_trace)
    .bind(impacted_user_id)
    .bind(error_code)
    .bind(fingerprint)
    .bind(http_status)
    .bind(endpoint)
    .bind(occurred_at)
    .bind(client_context)
    .execute(executor)
    .await?;
    Ok(())
}

pub struct SystemErrorFilter {
    pub severity: Option<String>,
    pub status: Option<String>,
    pub subsystem: Option<String>,
}

pub async fn list<'e, E: PgExecutor<'e>>(
    executor: E,
    filter: &SystemErrorFilter,
    limit: i64,
    offset: i64,
) -> Result<Vec<SystemErrorLog>, sqlx::Error> {
    sqlx::query_as::<_, SystemErrorLog>(
        "SELECT * FROM system_error_logs \
         WHERE ($1::text IS NULL OR severity = $1) \
           AND ($2::text IS NULL OR status = $2) \
           AND ($3::text IS NULL OR subsystem = $3) \
         ORDER BY last_seen_at DESC LIMIT $4 OFFSET $5",
    )
    .bind(&filter.severity)
    .bind(&filter.status)
    .bind(&filter.subsystem)
    .bind(limit)
    .bind(offset)
    .fetch_all(executor)
    .await
}

pub async fn count<'e, E: PgExecutor<'e>>(
    executor: E,
    filter: &SystemErrorFilter,
) -> Result<i64, sqlx::Error> {
    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM system_error_logs \
         WHERE ($1::text IS NULL OR severity = $1) \
           AND ($2::text IS NULL OR status = $2) \
           AND ($3::text IS NULL OR subsystem = $3)",
    )
    .bind(&filter.severity)
    .bind(&filter.status)
    .bind(&filter.subsystem)
    .fetch_one(executor)
    .await?;
    Ok(count)
}

pub async fn find_by_id<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
) -> Result<Option<SystemErrorLog>, sqlx::Error> {
    sqlx::query_as::<_, SystemErrorLog>("SELECT * FROM system_error_logs WHERE id = $1")
        .bind(id)
        .fetch_optional(executor)
        .await
}

pub async fn acknowledge<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    admin_id: crate::domain::ids::UserId,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE system_error_logs SET status = 'ACKNOWLEDGED', acknowledged_by = $2, acknowledged_at = now() \
         WHERE id = $1 AND status = 'OPEN'",
    )
    .bind(id)
    .bind(admin_id)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn resolve<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    admin_id: crate::domain::ids::UserId,
    note: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE system_error_logs SET status = 'RESOLVED', resolved_by = $2, resolved_at = now(), resolution_note = $3 \
         WHERE id = $1",
    )
    .bind(id)
    .bind(admin_id)
    .bind(note)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn count_open_critical<'e, E: PgExecutor<'e>>(executor: E) -> Result<i64, sqlx::Error> {
    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM system_error_logs WHERE status = 'OPEN' AND severity = 'CRITICAL'",
    )
    .fetch_one(executor)
    .await?;
    Ok(count)
}

/// Best-effort "system error traceability" correlation for the admin
/// consultation-timeline endpoint — **not** a hard foreign-key link.
/// `system_error_logs` has no `consultation_session_id` column (adding one
/// would require threading session context through the generic
/// `AppError`-driven reporting pipeline in `error.rs`, which is out of scope
/// for this pass), so this surfaces errors that merely *overlap* a session's
/// time window and impacted one of its two participants. Documented to the
/// admin caller as a heuristic, not asserted as exact.
pub async fn find_related<'e, E: PgExecutor<'e>>(
    executor: E,
    user_ids: &[crate::domain::ids::UserId],
    since: DateTime<Utc>,
    until: DateTime<Utc>,
) -> Result<Vec<SystemErrorLog>, sqlx::Error> {
    if user_ids.is_empty() {
        return Ok(Vec::new());
    }
    sqlx::query_as::<_, SystemErrorLog>(
        "SELECT * FROM system_error_logs \
         WHERE impacted_user_id = ANY($1) AND logged_at BETWEEN $2 AND $3 \
         ORDER BY logged_at ASC",
    )
    .bind(user_ids)
    .bind(since)
    .bind(until)
    .fetch_all(executor)
    .await
}

pub async fn count_open<'e, E: PgExecutor<'e>>(executor: E) -> Result<i64, sqlx::Error> {
    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM system_error_logs WHERE status = 'OPEN'")
            .fetch_one(executor)
            .await?;
    Ok(count)
}
