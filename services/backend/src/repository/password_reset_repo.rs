//! Phase 2: Secure Email OTP Password Recovery — persistence for the
//! Patient/Doctor self-service reset flow. Mirrors `session_repo`'s
//! `pending_mfa_challenges` functions closely (row-locked read-modify-write
//! for brute-force-safe attempt counting), extended with a second,
//! post-verification secret (`authorization_hash`) so the OTP itself is
//! single-use and the final "set new password" step is a genuinely separate
//! request, not a resubmission of the code.
use crate::domain::ids::UserId;
use crate::domain::models::PasswordResetRequest;
use chrono::{DateTime, Utc};
use sqlx::PgExecutor;

pub async fn insert<'e, E: PgExecutor<'e>>(
    executor: E,
    token: &str,
    user_id: UserId,
    code_hash: &str,
    otp_expires_at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO password_reset_requests (token, user_id, code_hash, otp_expires_at, created_at) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(token)
    .bind(user_id)
    .bind(code_hash)
    .bind(otp_expires_at)
    .bind(Utc::now())
    .execute(executor)
    .await?;
    Ok(())
}

/// A fresh request supersedes any earlier one for the same account — at most
/// one live reset flow per user at a time, so `find_active_for_user_for_update`
/// never has to disambiguate between several candidate rows. This single
/// atomic statement (not a separate DELETE then INSERT) is what actually
/// enforces that invariant: two concurrent `request_reset` calls for the same
/// account now serialize on the `password_reset_requests_user_id_unique`
/// constraint (migrations/20261208000001) instead of each being able to
/// delete-then-insert their own independent row. Resets every column a fresh
/// request implies — including clearing any `authorization_hash` an earlier,
/// already-verified-but-not-yet-completed flow had set — since a brand new
/// code must never inherit a stale flow's verification state.
pub async fn upsert_for_user<'e, E: PgExecutor<'e>>(
    executor: E,
    token: &str,
    user_id: UserId,
    code_hash: &str,
    otp_expires_at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO password_reset_requests \
            (token, user_id, code_hash, otp_expires_at, created_at) \
         VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (user_id) DO UPDATE SET \
            token = EXCLUDED.token, \
            code_hash = EXCLUDED.code_hash, \
            attempt_count = DEFAULT, \
            max_attempts = DEFAULT, \
            otp_expires_at = EXCLUDED.otp_expires_at, \
            authorization_hash = NULL, \
            authorization_expires_at = NULL, \
            created_at = EXCLUDED.created_at",
    )
    .bind(token)
    .bind(user_id)
    .bind(code_hash)
    .bind(otp_expires_at)
    .bind(Utc::now())
    .execute(executor)
    .await?;
    Ok(())
}

/// Invalidates every outstanding recovery challenge for an account — used by
/// `complete_reset` (inside the same transaction as the password update and
/// session revocation) to clean up defensively, and available for any other
/// caller that needs to unconditionally clear reset state for a user. Not
/// used by `request_reset` anymore — see `upsert_for_user`.
pub async fn invalidate_all_for_user<'e, E: PgExecutor<'e>>(
    executor: E,
    user_id: UserId,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM password_reset_requests WHERE user_id = $1")
        .bind(user_id)
        .execute(executor)
        .await?;
    Ok(())
}

/// `FOR UPDATE` row lock — must run inside an explicit transaction, exactly
/// like `session_repo::find_pending_challenge_for_update`, so the
/// check-then-increment attempt count can't race under concurrent guesses.
pub async fn find_active_for_user_for_update(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: UserId,
) -> Result<Option<PasswordResetRequest>, sqlx::Error> {
    sqlx::query_as::<_, PasswordResetRequest>(
        "SELECT token, user_id, code_hash, attempt_count, max_attempts, otp_expires_at, \
         authorization_hash, authorization_expires_at, created_at \
         FROM password_reset_requests WHERE user_id = $1 \
         ORDER BY created_at DESC LIMIT 1 FOR UPDATE",
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await
}

/// Looked up by the hashed post-verification authorization secret, never by
/// user id — `complete_reset` has no other identifier (the request is
/// unauthenticated), and the hash is unique by construction (see the
/// partial unique index in migrations/20261207000001_password_reset).
pub async fn find_by_authorization_hash_for_update(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    authorization_hash: &str,
) -> Result<Option<PasswordResetRequest>, sqlx::Error> {
    sqlx::query_as::<_, PasswordResetRequest>(
        "SELECT token, user_id, code_hash, attempt_count, max_attempts, otp_expires_at, \
         authorization_hash, authorization_expires_at, created_at \
         FROM password_reset_requests WHERE authorization_hash = $1 FOR UPDATE",
    )
    .bind(authorization_hash)
    .fetch_optional(&mut **tx)
    .await
}

pub async fn increment_attempts(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    token: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE password_reset_requests SET attempt_count = attempt_count + 1 WHERE token = $1",
    )
    .bind(token)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Transitions REQUESTED -> OTP_VERIFIED: the OTP itself can never be reused
/// again after this (no code-comparison path reads `code_hash` once this has
/// been set), replaced by the fresh, single-use `authorization_hash`.
pub async fn mark_verified(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    token: &str,
    authorization_hash: &str,
    authorization_expires_at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE password_reset_requests SET authorization_hash = $2, authorization_expires_at = $3 \
         WHERE token = $1",
    )
    .bind(token)
    .bind(authorization_hash)
    .bind(authorization_expires_at)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn delete<'e, E: PgExecutor<'e>>(executor: E, token: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM password_reset_requests WHERE token = $1")
        .bind(token)
        .execute(executor)
        .await?;
    Ok(())
}
