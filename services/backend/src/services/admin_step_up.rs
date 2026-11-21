//! Password step-up for sensitive Admin actions.
//!
//! A stolen Admin session must not be enough to approve doctors, move or
//! refund money, erase accounts or reveal identities. Those handlers call
//! [`require`]; it answers `REAUTH_REQUIRED` (403) unless the Admin confirmed
//! their password in this sign-in session within the last
//! [`STEP_UP_TTL_SECS`]. Admin Web then asks for the password, calls
//! `POST /admin/reauth` ([`confirm_and_grant`]) and retries the action.
//!
//! The grant is keyed by the session family (`sid` in the access token), so
//! it survives access-token refresh but not sign-out, and never carries over
//! to another device. Redis outages fail closed.

use crate::auth_context::AuthContext;
use crate::error::AppError;
use crate::repository::AppState;
use std::time::Duration;

/// How long one password confirmation unlocks sensitive actions.
/// Product owner, 2026-11-21: two minutes, shown as a countdown in Admin Web.
pub const STEP_UP_TTL_SECS: u64 = 120;

/// After 5 wrong passwords in 15 minutes further checks are refused (correct
/// ones never count).
const REAUTH_MAX_FAILURES: u32 = 5;
const REAUTH_WINDOW_SECS: u64 = 900;
const REDIS_DEADLINE: Duration = Duration::from_millis(250);

/// One grant per sign-in (token family): the silent access-token refresh
/// replaces the session row but keeps its family, so a refresh does not
/// throw away a password the Admin just confirmed.
async fn grant_key(state: &AppState, session_id: uuid::Uuid) -> Result<String, AppError> {
    let family: Option<uuid::Uuid> =
        sqlx::query_scalar("SELECT token_family_id FROM auth_sessions WHERE id = $1")
            .bind(session_id)
            .fetch_optional(&state.auth_db)
            .await?;
    let family = family.ok_or(AppError::ReauthRequired)?;
    Ok(format!("admin-stepup:{family}"))
}

fn store_unavailable(detail: impl std::fmt::Display) -> AppError {
    AppError::rate_limit_service_unavailable(format!("Step-up store unavailable: {detail}"))
}

/// Re-checks the signed-in Admin's password. Every wrong password is audited
/// (visible in the Activity Log) under `audit_resource`. A wrong password
/// answers 403, not 401, so the session itself stays valid.
pub async fn confirm_password(
    state: &AppState,
    ctx: &AuthContext,
    password: &str,
    audit_resource: &'static str,
) -> Result<(), AppError> {
    if password.is_empty() || password.len() > 128 {
        return Err(AppError::Forbidden(
            "Enter your current password to continue.".into(),
        ));
    }
    let failures_key = format!("ratelimit:admin-reauth-failures:{}", ctx.user_id);
    crate::rate_limit::ensure_failures_below(&state.redis, &failures_key, REAUTH_MAX_FAILURES)
        .await?;
    let mut tx = state
        .begin_authed_as(ctx, crate::repository::DbRole::AuthWorker)
        .await?;
    let user = crate::repository::user_repo::find_by_id(&mut *tx, ctx.user_id)
        .await?
        .ok_or_else(|| AppError::internal("Authenticated user record is missing.".into()))?;
    tx.commit().await?;
    let ok = match user.password_hash {
        Some(hash) => crate::crypto::verify_password_async(password.to_string(), hash).await?,
        None => {
            crate::crypto::verify_password_dummy_async().await?;
            false
        }
    };
    if !ok {
        crate::rate_limit::record_failure(&state.redis, &failures_key, REAUTH_WINDOW_SECS).await?;
        crate::services::AuditService::log(
            state,
            ctx,
            crate::services::AuditLogEntry::new(
                Some(ctx.user_id),
                ctx.role,
                "ADMIN_REAUTH_FAILED",
                audit_resource,
                None,
                "DENIED",
            ),
        )
        .await?;
        return Err(AppError::Forbidden(
            "Your current password is incorrect.".into(),
        ));
    }
    Ok(())
}

/// `POST /admin/reauth`: confirms the password and unlocks sensitive actions
/// for this sign-in session for [`STEP_UP_TTL_SECS`].
pub async fn confirm_and_grant(
    state: &AppState,
    ctx: &AuthContext,
    password: &str,
) -> Result<(), AppError> {
    let session_id = ctx.session_id.ok_or(AppError::ReauthRequired)?;
    confirm_password(state, ctx, password, "ADMIN_STEP_UP").await?;
    let key = grant_key(state, session_id).await?;
    let write = async {
        let mut conn = state.redis.get().await.map_err(store_unavailable)?;
        let stored: Result<(), _> = deadpool_redis::redis::cmd("SET")
            .arg(&key)
            .arg(1)
            .arg("EX")
            .arg(STEP_UP_TTL_SECS)
            .query_async(&mut conn)
            .await;
        stored.map_err(store_unavailable)
    };
    tokio::time::timeout(REDIS_DEADLINE, write)
        .await
        .map_err(|_| store_unavailable("deadline exceeded"))??;
    crate::services::AuditService::log(
        state,
        ctx,
        crate::services::AuditLogEntry::new(
            Some(ctx.user_id),
            ctx.role,
            "ADMIN_REAUTH_CONFIRMED",
            "ADMIN_STEP_UP",
            None,
            "SUCCESS",
        ),
    )
    .await?;
    Ok(())
}

/// Call at the top of a sensitive Admin handler, after `abac::authorize`.
pub async fn require(state: &AppState, ctx: &AuthContext) -> Result<(), AppError> {
    if !state.config.admin_step_up_required {
        return Ok(());
    }
    let session_id = ctx.session_id.ok_or(AppError::ReauthRequired)?;
    let key = grant_key(state, session_id).await?;
    let read = async {
        let mut conn = state.redis.get().await.map_err(store_unavailable)?;
        let exists: Result<bool, _> = deadpool_redis::redis::cmd("EXISTS")
            .arg(&key)
            .query_async(&mut conn)
            .await;
        exists.map_err(store_unavailable)
    };
    let granted = tokio::time::timeout(REDIS_DEADLINE, read)
        .await
        .map_err(|_| store_unavailable("deadline exceeded"))??;
    if granted {
        Ok(())
    } else {
        Err(AppError::ReauthRequired)
    }
}
