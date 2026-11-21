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

async fn confirm_password(
    state: &AppState,
    ctx: &AuthContext,
    password: &str,
) -> Result<(), AppError> {
    crate::services::admin_step_up::confirm_password(state, ctx, password, "ADMIN_INVITATION").await
}

/// `POST /admin/reauth` — confirms the Admin's password and unlocks the
/// sensitive Admin actions for this sign-in session for two minutes
/// (`services::admin_step_up`).
pub async fn reauth(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(payload): Json<ConfirmRequest>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ViewAdminOverview)?;
    crate::services::admin_step_up::confirm_and_grant(&state, &ctx, &payload.current_password)
        .await?;
    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "confirmed": true,
            "valid_for_seconds": crate::services::admin_step_up::STEP_UP_TTL_SECS,
        }),
    }))
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
    // Revoking needs a fresh password too (product owner, 2026-11-20).
    crate::services::admin_step_up::require(&state, &ctx).await?;
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
