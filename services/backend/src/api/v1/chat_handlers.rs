use crate::abac::{self, Action};
use crate::auth_context::AuthContext;
use crate::error::AppError;
use crate::rate_limit;
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
    ctx: AuthContext,
    Path(conversation_id): Path<Uuid>,
    Json(payload): Json<SendMessageRequest>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::AnyAuthenticated)?;
    rate_limit::check_chat_message_limit(&state.redis, &ctx.user_id.to_string()).await?;
    let content =
        crate::domain::validation::validate_text_field(&payload.content, "Message", 2000)?;
    if content.is_empty() {
        return Err(AppError::Validation("Message must not be empty.".into()));
    }
    // chat_messages.sender_id is FK'd to users(id) directly, not a profile id.
    let msg =
        ChatService::send_message(&state, &ctx, conversation_id, ctx.user_id, content).await?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(ApiResponse {
            success: true,
            data: msg,
        }),
    ))
}

pub async fn get_chat_messages(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(conversation_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::AnyAuthenticated)?;
    let msgs = ChatService::get_messages(&state, &ctx, conversation_id).await?;
    Ok(Json(ApiResponse {
        success: true,
        data: msgs,
    }))
}
