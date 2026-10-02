use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Validation failed: {0}")]
    Validation(String),

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Rate limit exceeded: {0}")]
    RateLimitExceeded(String),

    #[error("Payload too large: {0}")]
    PayloadTooLarge(String),

    #[error("Unprocessable entity: {0}")]
    UnprocessableEntity(String),

    #[error("Payment required: {0}")]
    PaymentRequired(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorDetail {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    pub issue: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Vec<ErrorDetail>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub error: ErrorBody,
    pub meta: ResponseMeta,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResponseMeta {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub request_id: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            AppError::Validation(msg) => (StatusCode::UNPROCESSABLE_ENTITY, "VALIDATION_FAILED", msg.clone()),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, "RESOURCE_NOT_FOUND", msg.clone()),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, "RESOURCE_CONFLICT", msg.clone()),
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, "UNAUTHORIZED", msg.clone()),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, "FORBIDDEN", msg.clone()),
            AppError::RateLimitExceeded(msg) => (StatusCode::TOO_MANY_REQUESTS, "RATE_LIMIT_EXCEEDED", msg.clone()),
            AppError::PayloadTooLarge(msg) => (StatusCode::PAYLOAD_TOO_LARGE, "PAYLOAD_TOO_LARGE", msg.clone()),
            AppError::UnprocessableEntity(msg) => (StatusCode::UNPROCESSABLE_ENTITY, "UNPROCESSABLE_ENTITY", msg.clone()),
            AppError::PaymentRequired(msg) => (StatusCode::PAYMENT_REQUIRED, "PAYMENT_REQUIRED", msg.clone()),
            AppError::Internal(msg) => {
                tracing::error!("Internal error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_SERVER_ERROR", "An unexpected error occurred.".into())
            }
        };

        let body = ErrorResponse {
            success: false,
            error: ErrorBody {
                code: code.to_string(),
                message,
                details: None,
            },
            meta: ResponseMeta {
                timestamp: chrono::Utc::now(),
                request_id: uuid::Uuid::new_v4().to_string(),
            },
        };

        (status, Json(body)).into_response()
    }
}
