use crate::domain::models::AuditEvent;
use sqlx::PgExecutor;

pub async fn insert<'e, E: PgExecutor<'e>>(executor: E, event: &AuditEvent) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO audit_events (id, actor_id, actor_role, action, resource_type, resource_id, session_id, request_id, ip_hash, result, reason, created_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)",
    )
    .bind(event.id)
    .bind(event.actor_id)
    .bind(event.actor_role)
    .bind(&event.action)
    .bind(&event.resource_type)
    .bind(event.resource_id)
    .bind(event.session_id)
    .bind(&event.request_id)
    .bind(&event.ip_hash)
    .bind(&event.result)
    .bind(&event.reason)
    .bind(event.created_at)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn list_recent<'e, E: PgExecutor<'e>>(executor: E, limit: i64) -> Result<Vec<AuditEvent>, sqlx::Error> {
    sqlx::query_as::<_, AuditEvent>(
        "SELECT id, actor_id, actor_role, action, resource_type, resource_id, session_id, request_id, ip_hash, result, reason, created_at \
         FROM audit_events ORDER BY created_at DESC LIMIT $1",
    )
    .bind(limit)
    .fetch_all(executor)
    .await
}

pub async fn count_unresolved<'e, E: PgExecutor<'e>>(executor: E) -> Result<i64, sqlx::Error> {
    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM audit_events WHERE result IN ('FAILURE', 'ERROR')",
    )
    .fetch_one(executor)
    .await?;
    Ok(count)
}
