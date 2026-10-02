use crate::error::AppError;
use crate::repository::AppState;
use crate::services::AppointmentService;
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct PaymentWebhookPayload {
    pub payment_session_id: Uuid,
    pub transaction_reference: String,
    pub status: String, // "SUCCESS"
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: T,
}

pub async fn handle_payment_webhook(
    State(state): State<AppState>,
    Path(provider): Path<String>,
    Json(payload): Json<PaymentWebhookPayload>,
) -> Result<impl IntoResponse, AppError> {
    if payload.status.to_uppercase() != "SUCCESS" {
        return Err(AppError::Conflict("Payment failed or cancelled at gateway.".into()));
    }

    let apt = AppointmentService::process_payment_webhook(
        &state,
        payload.payment_session_id,
        &payload.transaction_reference,
    )?;

    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "provider": provider,
            "appointment_id": apt.id,
            "status": apt.status,
            "message": "Payment confirmed and consultation slot locked."
        }),
    }))
}
