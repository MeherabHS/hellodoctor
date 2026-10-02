use crate::domain::models::*;
use crate::error::AppError;
use crate::repository::{chat_repo, AppState};
use chrono::Utc;
use uuid::Uuid;

pub struct ChatService;

impl ChatService {
    /// Post message to 24-hour follow-up clinical chat
    pub async fn send_message(state: &AppState, conversation_id: Uuid, sender_id: Uuid, content: String) -> Result<ChatMessage, AppError> {
        let conv = chat_repo::find_conversation(&state.db, conversation_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Chat conversation not found.".into()))?;

        if conv.is_locked || Utc::now() > conv.expires_at {
            if !conv.is_locked {
                chat_repo::lock_conversation(&state.db, conversation_id).await?;
            }
            return Err(AppError::Forbidden(
                "Clinical chat session has expired (24-hour window ended). Session is now locked.".into(),
            ));
        }

        let message = ChatMessage {
            id: Uuid::new_v4(),
            conversation_id,
            sender_id,
            content,
            attachment_bucket: None,
            attachment_object_key: None,
            attachment_mime: None,
            attachment_size: None,
            attachment_hash: None,
            is_read: false,
            sent_at: Utc::now(),
        };

        chat_repo::insert_message(&state.db, &message).await?;
        Ok(message)
    }

    /// Retrieve conversation history
    pub async fn get_messages(state: &AppState, conversation_id: Uuid) -> Result<Vec<ChatMessage>, AppError> {
        chat_repo::find_conversation(&state.db, conversation_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Chat conversation not found.".into()))?;

        chat_repo::list_messages(&state.db, conversation_id).await.map_err(AppError::from)
    }
}
