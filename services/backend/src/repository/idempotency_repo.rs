use crate::domain::models::IdempotencyRecord;
use chrono::{DateTime, Utc};
use sqlx::PgExecutor;
use uuid::Uuid;

pub async fn find<'e, E: PgExecutor<'e>>(
    executor: E,
    route: &str,
    key_hash: &str,
) -> Result<Option<IdempotencyRecord>, sqlx::Error> {
    sqlx::query_as::<_, IdempotencyRecord>(
        "SELECT id, key_hash, user_id, route, request_hash, response_status, response_body_ref, response_body, created_at, expires_at \
         FROM idempotency_records WHERE route = $1 AND key_hash = $2 AND expires_at > now()",
    )
    .bind(route)
    .bind(key_hash)
    .fetch_optional(executor)
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn insert<'e, E: PgExecutor<'e>>(
    executor: E,
    key_hash: &str,
    route: &str,
    request_hash: &str,
    response_status: i16,
    response_body: &str,
    expires_at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO idempotency_records (id, key_hash, user_id, route, request_hash, response_status, response_body_ref, response_body, created_at, expires_at) \
         VALUES ($1,$2,NULL,$3,$4,$5,NULL,$6,$7,$8) \
         ON CONFLICT (route, key_hash) DO NOTHING",
    )
    .bind(Uuid::new_v4())
    .bind(key_hash)
    .bind(route)
    .bind(request_hash)
    .bind(response_status)
    .bind(response_body)
    .bind(Utc::now())
    .bind(expires_at)
    .execute(executor)
    .await?;
    Ok(())
}
