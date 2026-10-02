use crate::domain::models::PhoneOtp;
use chrono::{DateTime, Utc};
use sqlx::PgExecutor;

pub async fn upsert<'e, E: PgExecutor<'e>>(
    executor: E,
    phone_number: &str,
    code_hash: &str,
    expires_at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO phone_otps (phone_number, code_hash, attempts, expires_at, created_at) \
         VALUES ($1, $2, 0, $3, $4) \
         ON CONFLICT (phone_number) DO UPDATE SET code_hash = EXCLUDED.code_hash, attempts = 0, expires_at = EXCLUDED.expires_at, created_at = EXCLUDED.created_at",
    )
    .bind(phone_number)
    .bind(code_hash)
    .bind(expires_at)
    .bind(Utc::now())
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn find<'e, E: PgExecutor<'e>>(executor: E, phone_number: &str) -> Result<Option<PhoneOtp>, sqlx::Error> {
    sqlx::query_as::<_, PhoneOtp>(
        "SELECT phone_number, code_hash, attempts, expires_at, created_at FROM phone_otps WHERE phone_number = $1",
    )
    .bind(phone_number)
    .fetch_optional(executor)
    .await
}

pub async fn increment_attempts<'e, E: PgExecutor<'e>>(executor: E, phone_number: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE phone_otps SET attempts = attempts + 1 WHERE phone_number = $1")
        .bind(phone_number)
        .execute(executor)
        .await?;
    Ok(())
}

pub async fn delete<'e, E: PgExecutor<'e>>(executor: E, phone_number: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM phone_otps WHERE phone_number = $1")
        .bind(phone_number)
        .execute(executor)
        .await?;
    Ok(())
}
