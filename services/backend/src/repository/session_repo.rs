use crate::domain::models::{AuthSession, MfaCredential, MfaMethod, PendingMfaChallenge};
use chrono::{DateTime, Utc};
use sqlx::PgExecutor;
use uuid::Uuid;

pub async fn insert_session<'e, E: PgExecutor<'e>>(executor: E, session: &AuthSession) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO auth_sessions (id, user_id, token_family_id, refresh_token_hash, device_fingerprint, ip_hash, user_agent, created_at, last_used_at, rotated_at, revoked_at, expires_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
    )
    .bind(session.id)
    .bind(session.user_id)
    .bind(session.token_family_id)
    .bind(&session.refresh_token_hash)
    .bind(&session.device_fingerprint)
    .bind(&session.ip_hash)
    .bind(&session.user_agent)
    .bind(session.created_at)
    .bind(session.last_used_at)
    .bind(session.rotated_at)
    .bind(session.revoked_at)
    .bind(session.expires_at)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn find_active_by_token_hash<'e, E: PgExecutor<'e>>(
    executor: E,
    token_hash: &str,
) -> Result<Option<AuthSession>, sqlx::Error> {
    sqlx::query_as::<_, AuthSession>(
        "SELECT id, user_id, token_family_id, refresh_token_hash, device_fingerprint, ip_hash, user_agent, created_at, last_used_at, rotated_at, revoked_at, expires_at \
         FROM auth_sessions WHERE refresh_token_hash = $1 AND revoked_at IS NULL",
    )
    .bind(token_hash)
    .fetch_optional(executor)
    .await
}

pub async fn revoke_family<'e, E: PgExecutor<'e>>(executor: E, token_family_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE auth_sessions SET revoked_at = $2 WHERE token_family_id = $1 AND revoked_at IS NULL")
        .bind(token_family_id)
        .bind(Utc::now())
        .execute(executor)
        .await?;
    Ok(())
}

pub async fn rotate<'e, E: PgExecutor<'e>>(
    executor: E,
    session_id: Uuid,
    new_hash: &str,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE auth_sessions SET refresh_token_hash = $2, rotated_at = $3, last_used_at = $3 WHERE id = $1")
        .bind(session_id)
        .bind(new_hash)
        .bind(now)
        .execute(executor)
        .await?;
    Ok(())
}

pub async fn find_mfa_credential<'e, E: PgExecutor<'e>>(
    executor: E,
    user_id: Uuid,
    method: MfaMethod,
) -> Result<Option<MfaCredential>, sqlx::Error> {
    sqlx::query_as::<_, MfaCredential>(
        "SELECT id, user_id, method, totp_secret_encrypted, recovery_codes_hash, is_enabled, enabled_at, last_verified_at, revoked_at, created_at \
         FROM mfa_credentials WHERE user_id = $1 AND method = $2 AND revoked_at IS NULL",
    )
    .bind(user_id)
    .bind(method)
    .fetch_optional(executor)
    .await
}

pub async fn upsert_mfa_credential<'e, E: PgExecutor<'e>>(executor: E, cred: &MfaCredential) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO mfa_credentials (id, user_id, method, totp_secret_encrypted, recovery_codes_hash, is_enabled, enabled_at, last_verified_at, revoked_at, created_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) \
         ON CONFLICT (user_id, method) WHERE revoked_at IS NULL DO UPDATE SET \
         totp_secret_encrypted = EXCLUDED.totp_secret_encrypted, is_enabled = EXCLUDED.is_enabled, enabled_at = EXCLUDED.enabled_at",
    )
    .bind(cred.id)
    .bind(cred.user_id)
    .bind(cred.method)
    .bind(&cred.totp_secret_encrypted)
    .bind(&cred.recovery_codes_hash)
    .bind(cred.is_enabled)
    .bind(cred.enabled_at)
    .bind(cred.last_verified_at)
    .bind(cred.revoked_at)
    .bind(cred.created_at)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn touch_mfa_verified<'e, E: PgExecutor<'e>>(executor: E, id: Uuid, at: DateTime<Utc>) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE mfa_credentials SET last_verified_at = $2 WHERE id = $1")
        .bind(id)
        .bind(at)
        .execute(executor)
        .await?;
    Ok(())
}

// ---- Pending (step-1-verified, awaiting-second-factor) MFA challenges ----

pub async fn insert_pending_challenge<'e, E: PgExecutor<'e>>(
    executor: E,
    token: &str,
    user_id: Uuid,
    code_hash: &str,
    expires_at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO pending_mfa_challenges (token, user_id, code_hash, expires_at, created_at) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(token)
    .bind(user_id)
    .bind(code_hash)
    .bind(expires_at)
    .bind(Utc::now())
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn find_pending_challenge<'e, E: PgExecutor<'e>>(
    executor: E,
    token: &str,
) -> Result<Option<PendingMfaChallenge>, sqlx::Error> {
    sqlx::query_as::<_, PendingMfaChallenge>(
        "SELECT token, user_id, code_hash, expires_at, created_at FROM pending_mfa_challenges WHERE token = $1",
    )
    .bind(token)
    .fetch_optional(executor)
    .await
}

pub async fn delete_pending_challenge<'e, E: PgExecutor<'e>>(executor: E, token: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM pending_mfa_challenges WHERE token = $1")
        .bind(token)
        .execute(executor)
        .await?;
    Ok(())
}
