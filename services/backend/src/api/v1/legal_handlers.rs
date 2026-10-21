use crate::abac::{self, Action};
use crate::auth_context::AuthContext;
use crate::domain::ids::UserId;
use crate::error::AppError;
use crate::repository::{legal_repo, AppState};
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: T,
}

/// GET /api/v1/legal/documents — public. A Doctor reads these before they
/// have any session at all, so this runs pre-auth through `state.auth_db`,
/// the same pool `doctor_register` already queries directly for its
/// duplicate-license/phone/email checks. Backs the mobile Legal & Consent
/// screen: body text is fetched here, never duplicated as static strings in
/// the app, so there is exactly one place the shown text and its hash can
/// ever come from.
pub async fn list_current_documents(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let documents = legal_repo::list_current(&state.auth_db).await?;
    Ok(Json(ApiResponse {
        success: true,
        data: documents,
    }))
}

/// GET /api/v1/doctor/legal/acceptances — a Doctor's own acceptance history.
/// Gated the same shape as `get_own_profile`/`get_own_duty`: any
/// authenticated Doctor reading their own record, not a new action.
pub async fn get_own_acceptances(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::DoctorRead)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let acceptances = legal_repo::list_for_user(&mut *tx, ctx.user_id).await?;
    tx.commit().await?;
    Ok(Json(ApiResponse {
        success: true,
        data: acceptances,
    }))
}

/// GET /api/v1/patients/legal/acceptances — a Patient's own immutable
/// onboarding-consent history. The same ledger is shared with Doctor legal
/// acceptance evidence, but the Patient path uses PatientRead authorization.
pub async fn get_patient_acceptances(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::PatientRead)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let acceptances = legal_repo::list_for_user(&mut *tx, ctx.user_id).await?;
    tx.commit().await?;
    Ok(Json(ApiResponse {
        success: true,
        data: acceptances,
    }))
}

/// GET /api/v1/admin/legal/acceptances/:user_id — Compliance/ClinicalAdmin/
/// PlatformAdmin evidence view of a specific user's acceptance history.
pub async fn admin_list_acceptances(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(user_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ViewLegalAcceptances)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let acceptances = legal_repo::list_for_user(&mut *tx, UserId::from(user_id)).await?;
    tx.commit().await?;
    Ok(Json(ApiResponse {
        success: true,
        data: acceptances,
    }))
}
