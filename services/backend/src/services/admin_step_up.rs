//! Admin step-up re-authentication (product owner, 2026-11-21: "all
//! sensitive actions").
//!
//! Sensitive Admin actions (doctor verification, Peer Navigator decisions,
//! grievance decisions and refunds, data erasure, finance, publishing push
//! campaigns) require the Admin to have re-entered their password within the
//! last [`STEP_UP_TTL_SECS`] on this session. `POST /admin/step-up` checks the
//! password (5 wrong passwords per 15 minutes lock it, every wrong one is
//! audited) and marks the session; [`require`] answers 403 with code
//! `STEP_UP_REQUIRED` otherwise, which Admin Web turns into a password prompt
//! and a retry. Admin team invitations keep their per-request password.

use crate::abac::{self, Action};
use crate::auth_context::AuthContext;
use crate::domain::models::UserRole;
use crate::error::AppError;
use crate::repository::AppState;
use axum::extract::State;
use axum::Json;
use serde::Deserialize;
use std::time::Duration;

/// How long one password confirmation covers sensitive actions.
pub const STEP_UP_TTL_SECS: u64 = 2 * 60;
const REAUTH_MAX_FAILURES: u32 = 5;
const REAUTH_WINDOW_SECS: u64 = 900;

/// The mark is per login (token family), so it survives the silent access
/// token refresh, which replaces the session row but keeps its family.
async fn marker_key(state: &AppState, ctx: &AuthContext) -> Result<Option<String>, AppError> {
    let Some(session) = ctx.session_id else {
        return Ok(None);
    };
    let family: Option<uuid::Uuid> =
        sqlx::query_scalar("SELECT token_family_id FROM auth_sessions WHERE id = $1")
            .bind(session)
            .fetch_optional(&state.auth_db)
            .await?;
    Ok(family.map(|family| format!("admin-step-up:{}:{family}", ctx.user_id)))
}

/// Re-checks the signed-in Admin's password. After 5 wrong passwords in 15
/// minutes further checks are refused (correct ones never count); every wrong
/// password is audited (visible in the Activity Log). A wrong password
/// answers 403, not 401, so the session itself stays valid.
pub async fn confirm_password(
    state: &AppState,
    ctx: &AuthContext,
    password: &str,
    resource_type: &str,
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
                resource_type,
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

async fn redis_conn(state: &AppState) -> Result<deadpool_redis::Connection, AppError> {
    state
        .redis
        .get()
        .await
        .map_err(|e| AppError::service_unavailable(format!("Step-up store unavailable: {e}")))
}

#[derive(Deserialize)]
pub struct StepUpRequest {
    pub current_password: String,
}

impl std::fmt::Debug for StepUpRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StepUpRequest")
            .field("current_password", &"[redacted]")
            .finish()
    }
}

/// `POST /admin/step-up` — confirms the password and marks this session as
/// recently re-authenticated for [`STEP_UP_TTL_SECS`].
pub async fn step_up(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(body): Json<StepUpRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    abac::authorize(&ctx, Action::ViewAdminOverview)?;
    let key = marker_key(&state, &ctx)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Sign in again to continue.".into()))?;
    confirm_password(&state, &ctx, &body.current_password, "ADMIN_STEP_UP").await?;
    let mut conn = redis_conn(&state).await?;
    tokio::time::timeout(
        Duration::from_millis(500),
        deadpool_redis::redis::cmd("SET")
            .arg(&key)
            .arg(1)
            .arg("EX")
            .arg(STEP_UP_TTL_SECS)
            .query_async::<()>(&mut conn),
    )
    .await
    .map_err(|_| AppError::service_unavailable("Step-up store deadline exceeded.".into()))?
    .map_err(|e| AppError::service_unavailable(format!("Step-up store error: {e}")))?;
    crate::services::AuditService::log(
        &state,
        &ctx,
        crate::services::AuditLogEntry::new(
            Some(ctx.user_id),
            ctx.role,
            "ADMIN_STEP_UP",
            "ADMIN_STEP_UP",
            ctx.session_id,
            "SUCCESS",
        ),
    )
    .await?;
    Ok(Json(serde_json::json!({
        "success": true,
        "data": {"valid_for_seconds": STEP_UP_TTL_SECS}
    })))
}

/// Guard for sensitive Admin actions: passes only if this session re-entered
/// the Admin password within [`STEP_UP_TTL_SECS`]. Fails closed when the
/// store is unreachable.
pub async fn require(state: &AppState, ctx: &AuthContext) -> Result<(), AppError> {
    if ctx.role != UserRole::Admin {
        return Ok(()); // Role checks are the handler's own authorize call.
    }
    let Some(key) = marker_key(state, ctx).await? else {
        return Err(step_up_required());
    };
    let mut conn = redis_conn(state).await?;
    let present: bool = tokio::time::timeout(
        Duration::from_millis(250),
        deadpool_redis::redis::cmd("EXISTS")
            .arg(&key)
            .query_async(&mut conn),
    )
    .await
    .map_err(|_| AppError::service_unavailable("Step-up store deadline exceeded.".into()))?
    .map_err(|e| AppError::service_unavailable(format!("Step-up store error: {e}")))?;
    if present {
        Ok(())
    } else {
        Err(step_up_required())
    }
}

fn step_up_required() -> AppError {
    AppError::StepUpRequired("Re-enter your password to confirm this action.".into())
}
