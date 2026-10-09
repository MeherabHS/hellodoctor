use crate::auth_context::AuthContext;
use crate::domain::ids::UserId;
use crate::domain::models::*;
use crate::error::AppError;
use crate::repository::{chat_repo, AppState, DbRole};
use chrono::Utc;
use uuid::Uuid;

pub struct ChatService;

impl ChatService {
    /// Post message to 24-hour follow-up clinical chat
    pub async fn send_message(
        state: &AppState,
        ctx: &AuthContext,
        conversation_id: Uuid,
        sender_id: UserId,
        content: String,
    ) -> Result<ChatMessage, AppError> {
        if ctx.role == UserRole::Doctor {
            ctx.verified_doctor_profile_id(state).await?;
        }
        let mut tx = state.begin_authed(ctx).await?;

        let conv = chat_repo::find_conversation(&mut *tx, conversation_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Chat conversation not found.".into()))?;

        if conv.is_locked || Utc::now() > conv.expires_at {
            tx.commit().await?;
            if !conv.is_locked {
                // chat_conversations.is_locked is lifecycle state — the ordinary
                // API role has no UPDATE grant on it (see AUTH_SECURITY.md §12.3),
                // so the lock itself runs on a separate system-worker transaction.
                let mut lock_tx = state.begin_authed_as(ctx, DbRole::SystemWorker).await?;
                chat_repo::lock_conversation(&mut *lock_tx, conversation_id).await?;
                lock_tx.commit().await?;
            }
            return Err(AppError::Forbidden(
                "Clinical chat session has expired (24-hour window ended). Session is now locked."
                    .into(),
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

        chat_repo::insert_message(&mut *tx, &message).await?;
        tx.commit().await?;
        Ok(message)
    }

    /// Retrieve conversation history
    pub async fn get_messages(
        state: &AppState,
        ctx: &AuthContext,
        conversation_id: Uuid,
    ) -> Result<Vec<ChatMessage>, AppError> {
        if ctx.role == UserRole::Doctor {
            ctx.verified_doctor_profile_id(state).await?;
        }
        let mut tx = state.begin_authed(ctx).await?;

        chat_repo::find_conversation(&mut *tx, conversation_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Chat conversation not found.".into()))?;

        let messages = chat_repo::list_messages(&mut *tx, conversation_id).await?;
        tx.commit().await?;
        Ok(messages)
    }
}
