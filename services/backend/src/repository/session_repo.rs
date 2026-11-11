use crate::domain::ids::UserId;
use crate::domain::models::{AuthSession, PendingMfaChallenge};
use chrono::{DateTime, Utc};
use sqlx::PgExecutor;
use uuid::Uuid;

pub async fn insert_session<'e, E: PgExecutor<'e>>(
    executor: E,
    session: &AuthSession,
) -> Result<(), sqlx::Error> {
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

// Consumed identities must remain discoverable for reuse detection and logout.
pub async fn find_by_token_hash<'e, E: PgExecutor<'e>>(
    executor: E,
    token_hash: &str,
) -> Result<Option<AuthSession>, sqlx::Error> {
    sqlx::query_as::<_, AuthSession>(
        "SELECT id, user_id, token_family_id, refresh_token_hash, device_fingerprint, ip_hash, user_agent, created_at, last_used_at, rotated_at, revoked_at, expires_at \
         FROM auth_sessions WHERE refresh_token_hash = $1",
    )
    .bind(token_hash)
    .fetch_optional(executor)
    .await
}

/// Share this lock across refresh, ancestor reuse, and logout. A per-token row
/// lock cannot serialize revocation against a different generation's rotation.
pub async fn lock_family(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    family: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!("refresh-token-family:{family}"))
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub async fn revoke_family<'e, E: PgExecutor<'e>>(
    executor: E,
    token_family_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE auth_sessions SET revoked_at = $2 WHERE token_family_id = $1 AND revoked_at IS NULL")
        .bind(token_family_id)
        .bind(Utc::now())
        .execute(executor)
        .await?;
    Ok(())
}

/// Account-wide "sign out everywhere except here" — previously the one
/// missing primitive flagged during the Phase 0 email audit (only
/// `revoke_family`, scoped to a single login lineage, existed in live
/// code; a true "every session for this user" revoke only ever existed in
/// the offline `examples/reset_admin_password.rs` operator tool). Built now
/// for the change-password flow, where it is a real security requirement:
/// a password change must invalidate every OTHER active session, since
/// whoever had the old password may still hold a live token. Excludes
/// `keep_session_id` (the leaf `auth_sessions.id` making this very
/// request) rather than a `token_family_id`, because at most one leaf per
/// family is ever active (`rotated_at IS NULL`) — excluding the current
/// active leaf is equivalent to excluding its whole family, and this
/// avoids a second lookup to resolve that family id. Returns the number of
/// sessions revoked.
pub async fn revoke_all_other_sessions_for_user<'e, E: PgExecutor<'e>>(
    executor: E,
    user_id: UserId,
    keep_session_id: Uuid,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE auth_sessions SET revoked_at = $3 \
         WHERE user_id = $1 AND id <> $2 AND revoked_at IS NULL",
    )
    .bind(user_id)
    .bind(keep_session_id)
    .bind(Utc::now())
    .execute(executor)
    .await?;
    Ok(result.rows_affected())
}

/// Full account-wide "sign out everywhere" — unlike
/// `revoke_all_other_sessions_for_user`, there is no session to keep,
/// because the caller (password reset) is never an authenticated session to
/// begin with. Used exactly once, after a reset completes, so whoever held
/// the old password's live tokens is force-logged-out everywhere. Returns
/// the number of sessions revoked.
pub async fn revoke_all_sessions_for_user<'e, E: PgExecutor<'e>>(
    executor: E,
    user_id: UserId,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE auth_sessions SET revoked_at = $2 WHERE user_id = $1 AND revoked_at IS NULL",
    )
    .bind(user_id)
    .bind(Utc::now())
    .execute(executor)
    .await?;
    Ok(result.rows_affected())
}

pub async fn consume_refresh_token(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    session_id: Uuid,
    now: DateTime<Utc>,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("UPDATE auth_sessions SET rotated_at = $2, last_used_at = $2 WHERE id = $1 AND rotated_at IS NULL AND revoked_at IS NULL AND expires_at > $2")
        .bind(session_id)
        .bind(now)
        .execute(&mut **tx)
        .await?;
    Ok(result.rows_affected() == 1)
}

// ---- Pending (step-1-verified, awaiting-second-factor) MFA challenges ----

pub async fn insert_pending_challenge<'e, E: PgExecutor<'e>>(
    executor: E,
    token: &str,
    user_id: UserId,
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
        "SELECT token, user_id, code_hash, expires_at, created_at, attempt_count, max_attempts FROM pending_mfa_challenges WHERE token = $1",
    )
    .bind(token)
    .fetch_optional(executor)
    .await
}

/// `FOR UPDATE` row lock — must run inside an explicit transaction so the
/// lock is actually held across the subsequent check-then-increment, closing
/// the brute-force race a plain read-then-write would leave open (see
/// `AuthService::doctor_verify_otp`, the only caller).
pub async fn find_pending_challenge_for_update(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    token: &str,
) -> Result<Option<PendingMfaChallenge>, sqlx::Error> {
    sqlx::query_as::<_, PendingMfaChallenge>(
        "SELECT token, user_id, code_hash, expires_at, created_at, attempt_count, max_attempts \
         FROM pending_mfa_challenges WHERE token = $1 FOR UPDATE",
    )
    .bind(token)
    .fetch_optional(&mut **tx)
    .await
}

pub async fn increment_challenge_attempts(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    token: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE pending_mfa_challenges SET attempt_count = attempt_count + 1 WHERE token = $1",
    )
    .bind(token)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn delete_pending_challenge<'e, E: PgExecutor<'e>>(
    executor: E,
    token: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM pending_mfa_challenges WHERE token = $1")
        .bind(token)
        .execute(executor)
        .await?;
    Ok(())
}
