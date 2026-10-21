use crate::abac::{self, Action};
use crate::auth_context::AuthContext;
use crate::domain::models::*;
use crate::error::AppError;
use crate::rate_limit;
use crate::repository::{appointment_repo, consultation_repo, AppState};
use crate::services::GrievanceService;
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct SubmitGrievanceRequest {
    pub consultation_id: Uuid,
    pub target: GrievanceTarget,
    pub category: String,
    pub claim_summary: String,
}

#[derive(Debug, Deserialize)]
pub struct AdjudicateRequest {
    pub audit_notes: String,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: T,
}

pub async fn submit_grievance(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(payload): Json<SubmitGrievanceRequest>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::SubmitGrievance)?;
    rate_limit::check_grievance_limit(&state.redis, &ctx.user_id.to_string()).await?;

    let category =
        crate::domain::validation::validate_text_field(&payload.category, "Category", 150)?;
    let claim_summary = crate::domain::validation::validate_text_field(
        &payload.claim_summary,
        "Claim summary",
        1000,
    )?;
    if category.is_empty() || claim_summary.is_empty() {
        return Err(AppError::Validation(
            "Category and claim summary must not be empty.".into(),
        ));
    }

    // The consultation's own appointment already pins down which profile
    // (self or a guardianed dependent) this grievance is about — resolved
    // here rather than trusted from a client-supplied field, consistent
    // with `prescription_handlers::upload_patient_intake`. The DB trigger
    // `guard_grievance_subject` independently enforces this same
    // consistency at the database level regardless of what's passed here.
    let consultation_patient_id = {
        let mut lookup_tx = state.begin_authed(&ctx).await?;
        let consultation = consultation_repo::find_by_id(&mut *lookup_tx, payload.consultation_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Consultation session not found.".into()))?;
        let appointment =
            appointment_repo::find_by_id(&mut *lookup_tx, consultation.appointment_id)
                .await?
                .ok_or_else(|| AppError::NotFound("Appointment not found.".into()))?;
        lookup_tx.commit().await?;
        appointment.patient_id
    };
    let patient_id = ctx
        .acting_patient_profile_id(&state, Some(consultation_patient_id))
        .await?;
    let report = GrievanceService::submit_grievance(
        &state,
        &ctx,
        patient_id,
        payload.consultation_id,
        payload.target,
        category,
        claim_summary,
    )
    .await?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(ApiResponse {
            success: true,
            data: serde_json::json!({
                "grievance_id": report.id,
                "grievance_number": report.grievance_number,
                "status": report.status,
                "message": "Report submitted. Medical Administration is reviewing your claim."
            }),
        }),
    ))
}

pub async fn adjudicate_refund(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<AdjudicateRequest>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::AdjudicateGrievance)?;
    let audit_notes =
        crate::domain::validation::validate_text_field(&payload.audit_notes, "Audit notes", 1000)?;
    let report =
        GrievanceService::adjudicate_refund(&state, &ctx, id, ctx.user_id, audit_notes).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "grievance_id": report.id,
            "status": report.status,
            // Approval is not payment — must never claim money moved. Matches
            // the remedy text actually persisted by
            // GrievanceService::adjudicate_refund (see B-04 in
            // docs/audit/DOCTOR_SECURITY_BUG_AUDIT.md).
            "board_remedy": "Refund approved; provider execution pending. No money has been returned."
        }),
    }))
}

pub async fn adjudicate_warn(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<AdjudicateRequest>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::AdjudicateGrievance)?;
    let audit_notes =
        crate::domain::validation::validate_text_field(&payload.audit_notes, "Audit notes", 1000)?;
    let report =
        GrievanceService::adjudicate_warning(&state, &ctx, id, ctx.user_id, audit_notes).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "grievance_id": report.id,
            "status": report.status,
            "board_remedy": "Internal Platform Compliance Warning Logged in Doctor Dossier"
        }),
    }))
}

/// GET /api/v1/admin/finance/refund-requests
pub async fn list_refund_requests(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::InitiateDisbursement)?;
    let requests = GrievanceService::list_refund_requests(&state, &ctx).await?;
    Ok(Json(ApiResponse {
        success: true,
        data: requests,
    }))
}

#[derive(Debug, Deserialize)]
pub struct TransitionRefundRequest {
    pub status: String,
    pub provider_reference: Option<String>,
}

/// POST /api/v1/admin/finance/refund-requests/:id/transition
pub async fn transition_refund_request(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<TransitionRefundRequest>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::InitiateDisbursement)?;
    let provider_reference = payload
        .provider_reference
        .map(|r| crate::domain::validation::validate_text_field(&r, "Provider reference", 300))
        .transpose()?;
    let updated = GrievanceService::transition_refund_request(
        &state,
        &ctx,
        ctx.user_id,
        id,
        &payload.status,
        provider_reference,
    )
    .await?;
    Ok(Json(ApiResponse {
        success: true,
        data: updated,
    }))
}

pub async fn adjudicate_dismiss(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<AdjudicateRequest>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::AdjudicateGrievance)?;
    let audit_notes =
        crate::domain::validation::validate_text_field(&payload.audit_notes, "Audit notes", 1000)?;
    let report =
        GrievanceService::adjudicate_dismiss(&state, &ctx, id, ctx.user_id, audit_notes).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "grievance_id": report.id,
            "status": report.status,
            "board_remedy": "Claim Reviewed and Dismissed — No Compliance Action Warranted"
        }),
    }))
}
