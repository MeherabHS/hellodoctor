use crate::domain::models::*;
use crate::error::AppError;
use crate::repository::AppState;
use chrono::Utc;
use uuid::Uuid;

pub struct ChatService;

impl ChatService {
    /// Post message to 24-hour follow-up clinical chat
    pub fn send_message(
        state: &AppState,
        conversation_id: Uuid,
        sender_id: Uuid,
        content: String,
    ) -> Result<ChatMessage, AppError> {
        let mut convs = state.chat_conversations.write();
        let conv = convs.get_mut(&conversation_id).ok_or_else(|| {
            AppError::NotFound("Chat conversation not found.".into())
        })?;

        // 24-Hour auto-locking check
        if conv.is_locked || Utc::now() > conv.expires_at {
            conv.is_locked = true;
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

        let mut messages_map = state.chat_messages.write();
        messages_map
            .entry(conversation_id)
            .or_default()
            .push(message.clone());

        Ok(message)
    }

    /// Retrieve conversation history
    pub fn get_messages(
        state: &AppState,
        conversation_id: Uuid,
    ) -> Result<Vec<ChatMessage>, AppError> {
        let convs = state.chat_conversations.read();
        let _ = convs.get(&conversation_id).ok_or_else(|| {
            AppError::NotFound("Chat conversation not found.".into())
        })?;

        let messages_map = state.chat_messages.read();
        let messages = messages_map.get(&conversation_id).cloned().unwrap_or_default();
        Ok(messages)
    }
}
