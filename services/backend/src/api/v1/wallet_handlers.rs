use crate::error::AppError;
use crate::repository::AppState;
use crate::services::DisbursementService;
use axum::{extract::State, response::IntoResponse, Json};
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: T,
}

pub async fn get_doctor_wallet(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    // Default to seeded doctor 1 for viewing
    let doc_id = Uuid::parse_str("da000001-0000-0000-0000-000000000001").unwrap();
    let summary = DisbursementService::get_doctor_wallet_summary(&state, doc_id).await?;

    Ok(Json(ApiResponse { success: true, data: summary }))
}
