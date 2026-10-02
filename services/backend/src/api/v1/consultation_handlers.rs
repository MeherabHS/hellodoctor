use crate::domain::models::*;
use crate::error::AppError;
use crate::repository::AppState;
use crate::services::ConsultationService;
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct TelemetrySubmissionRequest {
    pub call_duration_seconds: i32,
    pub connection_state: String,
    pub prescription_issued: bool,
}

#[derive(Debug, Deserialize)]
pub struct CompleteConsultationRequest {
    pub outcome: ClinicalOutcome,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: T,
}

pub async fn mint_agora_token(
    State(state): State<AppState>,
    Path(appointment_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    // For testing/mock caller, user_id defaults to patient or doctor
    let user_id = Uuid::parse_str("ba000001-0000-0000-0000-000000000001").unwrap();
    let resp = ConsultationService::mint_agora_token(
        &state,
        appointment_id,
        user_id,
        "a1b2c3d4e5f67890123456789abcdef0",
        "cert_secure_server_only_secret_9988",
    )?;

    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "channel_name": resp.channel_name,
            "token": resp.token,
            "uid": resp.uid,
            "app_id": resp.app_id,
            "expires_in": resp.expires_in
        }),
    }))
}

pub async fn submit_telemetry(
    State(state): State<AppState>,
    Path(appointment_id): Path<Uuid>,
    Json(payload): Json<TelemetrySubmissionRequest>,
) -> Result<impl IntoResponse, AppError> {
    let telem = ConsultationService::record_telemetry(
        &state,
        appointment_id,
        payload.call_duration_seconds,
        payload.connection_state,
        payload.prescription_issued,
    )?;

    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "telemetry_id": telem.id,
            "recorded": true,
            "premature_end": telem.premature_end
        }),
    }))
}

pub async fn complete_consultation(
    State(state): State<AppState>,
    Path(appointment_id): Path<Uuid>,
    Json(payload): Json<CompleteConsultationRequest>,
) -> Result<impl IntoResponse, AppError> {
    ConsultationService::complete_consultation(&state, appointment_id, payload.outcome)?;

    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "appointment_id": appointment_id,
            "status": "COMPLETED",
            "message": "Consultation concluded and payment settled to doctor wallet."
        }),
    }))
}
