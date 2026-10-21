use crate::abac::{self, Action};
use crate::auth_context::AuthContext;
use crate::error::AppError;
use crate::rate_limit;
use crate::repository::AppState;
use crate::services::ChatService;
use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
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
    headers: HeaderMap,
    Json(payload): Json<SendMessageRequest>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::AnyAuthenticated)?;
    if let Err(error) =
        rate_limit::check_chat_message_limit(&state.redis, &ctx.user_id.to_string()).await
    {
        let reason = if matches!(&error, AppError::RateLimitExceeded(_)) {
            "RATE_LIMITED"
        } else {
            "LIMITER_UNAVAILABLE"
        };
        crate::services::consultation_trace::reject_chat_if_visible(
            &state,
            &ctx,
            conversation_id,
            reason,
        )
        .await?;
        return Err(error);
    }
    let content =
        match crate::domain::validation::validate_text_field(&payload.content, "Message", 2000) {
            Ok(content) => content,
            Err(error) => {
                crate::services::consultation_trace::reject_chat_if_visible(
                    &state,
                    &ctx,
                    conversation_id,
                    "INVALID_MESSAGE",
                )
                .await?;
                return Err(error);
            }
        };
    if content.is_empty() {
        crate::services::consultation_trace::reject_chat_if_visible(
            &state,
            &ctx,
            conversation_id,
            "EMPTY_MESSAGE",
        )
        .await?;
        return Err(AppError::Validation("Message must not be empty.".into()));
    }
    // chat_messages.sender_id is FK'd to users(id) directly, not a profile id.
    let key = headers.get("Idempotency-Key").and_then(|v| v.to_str().ok());
    if key.is_some_and(|k| k.is_empty() || k.len() > 128) {
        crate::services::consultation_trace::reject_chat_if_visible(
            &state,
            &ctx,
            conversation_id,
            "INVALID_MESSAGE_KEY",
        )
        .await?;
        return Err(AppError::Validation("Invalid message request key.".into()));
    }
    let msg =
        ChatService::send_message(&state, &ctx, conversation_id, ctx.user_id, content, key).await?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(ApiResponse {
            success: true,
            data: super::chat_attachment_handlers::public_message(&msg),
        }),
    ))
}

#[derive(Default, Deserialize)]
pub struct MessageQuery {
    pub before_time: Option<chrono::DateTime<chrono::Utc>>,
    pub before_id: Option<Uuid>,
    pub after_time: Option<chrono::DateTime<chrono::Utc>>,
    pub after_id: Option<Uuid>,
}
pub async fn get_chat_messages(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(conversation_id): Path<Uuid>,
    Query(query): Query<MessageQuery>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::AnyAuthenticated)?;
    if query.before_time.is_some() != query.before_id.is_some()
        || query.after_time.is_some() != query.after_id.is_some()
        || (query.before_id.is_some() && query.after_id.is_some())
    {
        return Err(AppError::Validation("Invalid message cursor.".into()));
    }
    let msgs = ChatService::get_messages(
        &state,
        &ctx,
        conversation_id,
        query.before_time.zip(query.before_id),
        query.after_time.zip(query.after_id),
    )
    .await?;
    Ok(Json(ApiResponse {
        success: true,
        data: msgs
            .iter()
            .map(super::chat_attachment_handlers::public_message)
            .collect::<Vec<_>>(),
    }))
}
