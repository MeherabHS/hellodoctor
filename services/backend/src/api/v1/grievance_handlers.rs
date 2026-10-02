use crate::domain::models::*;
use crate::error::AppError;
use crate::repository::AppState;
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
    Json(payload): Json<SubmitGrievanceRequest>,
) -> Result<impl IntoResponse, AppError> {
    let patient_id = Uuid::parse_str("ba000001-0000-0000-0000-000000000001").unwrap();
    let report = GrievanceService::submit_grievance(
        &state,
        patient_id,
        payload.consultation_id,
        payload.target,
        payload.category,
        payload.claim_summary,
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
    Path(id): Path<Uuid>,
    Json(payload): Json<AdjudicateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let admin_id = Uuid::parse_str("a0000001-0000-0000-0000-000000000001").unwrap();
    let report = GrievanceService::adjudicate_refund(&state, id, admin_id, payload.audit_notes).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "grievance_id": report.id,
            "status": report.status,
            "board_remedy": "Payment Refund Disbursed to Patient MFS Account"
        }),
    }))
}

pub async fn adjudicate_warn(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<AdjudicateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let admin_id = Uuid::parse_str("a0000001-0000-0000-0000-000000000001").unwrap();
    let report = GrievanceService::adjudicate_warning(&state, id, admin_id, payload.audit_notes).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "grievance_id": report.id,
            "status": report.status,
            "board_remedy": "Internal Platform Compliance Warning Logged in Doctor Dossier"
        }),
    }))
}

pub async fn adjudicate_dismiss(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<AdjudicateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let admin_id = Uuid::parse_str("a0000001-0000-0000-0000-000000000001").unwrap();
    let report = GrievanceService::adjudicate_dismiss(&state, id, admin_id, payload.audit_notes).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "grievance_id": report.id,
            "status": report.status,
            "board_remedy": "Claim Reviewed and Dismissed — No Compliance Action Warranted"
        }),
    }))
}
