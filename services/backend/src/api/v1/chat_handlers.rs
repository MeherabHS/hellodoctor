use crate::error::AppError;
use crate::repository::AppState;
use crate::services::ChatService;
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct SendMessageRequest {
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: T,
}

pub async fn send_chat_message(
    State(state): State<AppState>,
    Path(conversation_id): Path<Uuid>,
    Json(payload): Json<SendMessageRequest>,
) -> Result<impl IntoResponse, AppError> {
    // chat_messages.sender_id is FK'd to users(id), not patient_profiles(id).
    let sender_id = Uuid::parse_str("b0000001-0000-0000-0000-000000000001").unwrap();
    let msg = ChatService::send_message(&state, conversation_id, sender_id, payload.content).await?;

    Ok((axum::http::StatusCode::CREATED, Json(ApiResponse { success: true, data: msg })))
}

pub async fn get_chat_messages(
    State(state): State<AppState>,
    Path(conversation_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let msgs = ChatService::get_messages(&state, conversation_id).await?;
    Ok(Json(ApiResponse { success: true, data: msgs }))
}
