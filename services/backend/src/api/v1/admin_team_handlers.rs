//! Admin Team: list Admins, invite by email, resend and revoke invitations,
//! plus the two public steps of accepting an invitation. See
//! `services::admin_invitation_service`.
use crate::abac::{self, Action};
use crate::auth_context::AuthContext;
use crate::error::AppError;
use crate::repository::AppState;
use crate::services::admin_invitation_service::{AdminInvitationService, CreatedInvitation};
use axum::{
    extract::{ConnectInfo, Path, State},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use uuid::Uuid;

#[derive(Debug, Serialize)]
struct ApiResponse<T> {
    success: bool,
    data: T,
}

/// Inviting or resending needs the Admin's current password (product owner,
/// 2026-11-14), so a hijacked session alone cannot add Admins.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InviteRequest {
    pub email: String,
    pub current_password: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfirmRequest {
    pub current_password: String,
}

impl std::fmt::Debug for InviteRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InviteRequest")
            .field("email", &"[redacted]")
            .field("current_password", &"[redacted]")
            .finish()
    }
}

impl std::fmt::Debug for ConfirmRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfirmRequest")
            .field("current_password", &"[redacted]")
            .finish()
    }
}

/// Re-checks the signed-in Admin's password. After 5 wrong passwords in 15
/// minutes further checks are refused (correct ones never count); every wrong password is audited (visible in the Activity Log).
/// A wrong password answers 403, not 401, so the session itself stays valid.
const REAUTH_MAX_FAILURES: u32 = 5;
const REAUTH_WINDOW_SECS: u64 = 900;

async fn confirm_password(
    state: &AppState,
    ctx: &AuthContext,
    password: &str,
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
                "ADMIN_INVITATION",
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InvitationTokenRequest {
    pub token: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptInvitationRequest {
    pub token: String,
    pub phone_number: String,
    pub password: String,
}

fn created(result: CreatedInvitation) -> serde_json::Value {
    serde_json::json!({
        "invitation": result.invitation,
        "email_sent": result.email_sent,
        // Development only (None elsewhere): lets a local tester open the link
        // when email delivery is not configured.
        "dev_invite_url": result.dev_invite_url,
    })
}

/// GET /api/v1/admin/team
pub async fn team(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ManageAdminTeam)?;
    Ok(Json(ApiResponse {
        success: true,
        data: AdminInvitationService::team(&state, &ctx).await?,
    }))
}

/// POST /api/v1/admin/team/invitations
pub async fn invite(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(payload): Json<InviteRequest>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ManageAdminTeam)?;
    confirm_password(&state, &ctx, &payload.current_password).await?;
    let result = AdminInvitationService::invite(&state, &ctx, &payload.email, None).await?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(ApiResponse {
            success: true,
            data: created(result),
        }),
    ))
}

/// POST /api/v1/admin/team/invitations/:id/resend — revokes the old link and sends a new one.
pub async fn resend(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<ConfirmRequest>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ManageAdminTeam)?;
    confirm_password(&state, &ctx, &payload.current_password).await?;
    let result = AdminInvitationService::resend(&state, &ctx, id).await?;
    Ok(Json(ApiResponse {
        success: true,
        data: created(result),
    }))
}

/// POST /api/v1/admin/team/invitations/:id/revoke
pub async fn revoke(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ManageAdminTeam)?;
    AdminInvitationService::revoke(&state, &ctx, id).await?;
    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({ "id": id, "status": "REVOKED" }),
    }))
}

/// POST /api/v1/auth/admin/invitations/lookup (public) — token in the body,
/// never in a URL the backend logs.
pub async fn lookup(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(payload): Json<InvitationTokenRequest>,
) -> Result<impl IntoResponse, AppError> {
    let data = AdminInvitationService::lookup(&state, &addr.ip().to_string(), payload.token.trim())
        .await?;
    Ok(Json(ApiResponse {
        success: true,
        data,
    }))
}

/// POST /api/v1/auth/admin/invitations/accept (public)
pub async fn accept(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(payload): Json<AcceptInvitationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let data = AdminInvitationService::accept(
        &state,
        &addr.ip().to_string(),
        payload.token.trim(),
        &payload.phone_number,
        &payload.password,
    )
    .await?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(ApiResponse {
            success: true,
            data,
        }),
    ))
}
