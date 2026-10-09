use crate::abac::{self, Action};
use crate::auth_context::AuthContext;
use crate::domain::models::*;
use crate::error::AppError;
use crate::repository::AppState;
use crate::services::consultation_service::NewTelemetryEvent;
use crate::services::ConsultationService;
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct TelemetrySubmissionRequest {
    pub call_duration_seconds: i32,
    pub connection_state: String,
    pub prescription_issued: bool,
}

#[derive(Debug, Deserialize)]
pub struct TelemetryEventInput {
    pub event_type: String,
    pub rtt_ms: Option<i32>,
    pub packet_loss_percent: Option<BigDecimal>,
    pub occurred_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct TelemetryEventBatchRequest {
    pub events: Vec<TelemetryEventInput>,
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
    ctx: AuthContext,
    Path(appointment_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::AnyAuthenticated)?;
    let resp = ConsultationService::mint_agora_token(
        &state,
        &ctx,
        appointment_id,
        &state.config.agora_app_id,
        &state.config.agora_app_certificate,
    )
    .await?;

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
    ctx: AuthContext,
    Path(appointment_id): Path<Uuid>,
    Json(payload): Json<TelemetrySubmissionRequest>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::AnyAuthenticated)?;
    let telem = ConsultationService::record_telemetry(
        &state,
        &ctx,
        appointment_id,
        payload.call_duration_seconds,
        payload.connection_state,
        payload.prescription_issued,
    )
    .await?;

    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "telemetry_id": telem.id,
            "recorded": true,
            "premature_end": telem.premature_end
        }),
    }))
}

/// POST /api/v1/consultations/:appointment_id/telemetry/events — detailed,
/// per-perspective call lifecycle events supplementing `submit_telemetry`'s
/// single aggregate row. See `ConsultationService::record_telemetry_events`
/// for the fail-open/bounded ingestion contract.
pub async fn submit_telemetry_events(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(appointment_id): Path<Uuid>,
    Json(payload): Json<TelemetryEventBatchRequest>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::AnyAuthenticated)?;
    let events = payload
        .events
        .into_iter()
        .map(|e| NewTelemetryEvent {
            event_type: e.event_type,
            rtt_ms: e.rtt_ms,
            packet_loss_percent: e.packet_loss_percent,
            occurred_at: e.occurred_at,
        })
        .collect();
    let accepted =
        ConsultationService::record_telemetry_events(&state, &ctx, appointment_id, events).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({ "accepted": accepted }),
    }))
}

pub async fn complete_consultation(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(appointment_id): Path<Uuid>,
    Json(payload): Json<CompleteConsultationRequest>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::CompleteConsultation)?;
    ConsultationService::complete_consultation(&state, &ctx, appointment_id, payload.outcome)
        .await?;

    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "appointment_id": appointment_id,
            "status": "COMPLETED",
            "message": "Consultation completion confirmed. Financial state is managed by the backend."
        }),
    }))
}
