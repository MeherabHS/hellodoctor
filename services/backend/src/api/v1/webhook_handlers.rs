use crate::crypto;
use crate::domain::models::Gateway;
use crate::error::AppError;
use crate::repository::AppState;
use crate::services::AppointmentService;
use axum::{
    body::Bytes,
    extract::{Path, State},
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct PaymentWebhookPayload {
    pub payment_session_id: Uuid,
    pub transaction_reference: String,
    pub status: String, // "SUCCESS"
    /// Event emission time, as reported by the provider. Required so a
    /// captured-and-replayed request can be rejected once stale, even though
    /// its signature is still valid.
    pub event_timestamp: DateTime<Utc>,
    /// The provider's own identifier for this specific callback/event
    /// (distinct from `transaction_reference`, which identifies the
    /// underlying payment attempt). Deduplicated against the unique
    /// `(gateway, provider_event_id)` index on `payment_events` — a replayed
    /// event under the same id has zero additional effect, independent of
    /// (and in addition to) the transaction-status-based idempotency already
    /// in `process_payment_webhook`.
    pub provider_event_id: String,
}

fn parse_gateway(provider: &str) -> Result<Gateway, AppError> {
    match provider.to_ascii_lowercase().as_str() {
        "bkash" => Ok(Gateway::Bkash),
        "nagad" => Ok(Gateway::Nagad),
        "card" => Ok(Gateway::Card),
        "mpesa" => Ok(Gateway::Mpesa),
        other => Err(AppError::NotFound(format!(
            "Unknown payment provider '{other}'."
        ))),
    }
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: T,
}

const MAX_WEBHOOK_AGE_SECONDS: i64 = 300;

/// Verifies `X-Signature: hex(hmac_sha256(WEBHOOK_HMAC_SECRET, raw_body))`
/// against the *exact raw bytes* of the request — before any JSON parsing —
/// so a payload the provider never signed can never be accepted, and checks
/// the event's reported timestamp is recent. Both checks run before the body
/// is even deserialized into `PaymentWebhookPayload`.
pub async fn handle_payment_webhook(
    State(state): State<AppState>,
    Path(provider): Path<String>,
    headers: HeaderMap,
    raw_body: Bytes,
) -> Result<impl IntoResponse, AppError> {
    if state.config.environment != "development" {
        return Err(AppError::Conflict(
            "Real payment-provider verification is not configured. No payment was confirmed."
                .into(),
        ));
    }
    let signature = headers
        .get("X-Signature")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::Unauthorized("Missing X-Signature header.".into()))?;

    if !crypto::verify_webhook_signature(&raw_body, signature, &state.config.webhook_hmac_secret) {
        tracing::warn!(provider = %provider, "Payment webhook rejected: invalid HMAC signature");
        return Err(AppError::Unauthorized("Invalid webhook signature.".into()));
    }

    let payload: PaymentWebhookPayload = serde_json::from_slice(&raw_body)
        .map_err(|e| AppError::UnprocessableEntity(format!("Malformed webhook payload: {e}")))?;

    let age_seconds = (Utc::now() - payload.event_timestamp).num_seconds();
    if !(0..=MAX_WEBHOOK_AGE_SECONDS).contains(&age_seconds) {
        tracing::warn!(provider = %provider, age_seconds, "Payment webhook rejected: stale or future-dated event timestamp");
        return Err(AppError::Unauthorized(
            "Webhook event timestamp is outside the acceptable freshness window.".into(),
        ));
    }

    if payload.status.to_uppercase() != "SUCCESS" {
        return Err(AppError::Conflict(
            "Payment failed or cancelled at gateway.".into(),
        ));
    }

    let gateway = parse_gateway(&provider)?;

    let apt = AppointmentService::process_payment_webhook(
        &state,
        payload.payment_session_id,
        gateway,
        &payload.transaction_reference,
        &payload.provider_event_id,
    )
    .await?;

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
