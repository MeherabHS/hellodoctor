use crate::{
    auth_context::AuthContext,
    domain::{ids::UserId, models::*},
    error::AppError,
    repository::{appointment_repo, chat_repo, notification_repo, AppState},
    services::consultation_trace::{self, Event},
};
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use uuid::Uuid;
pub struct ChatService;
impl ChatService {
    pub async fn send_message(
        state: &AppState,
        ctx: &AuthContext,
        conversation_id: Uuid,
        sender_id: UserId,
        content: String,
        key: Option<&str>,
    ) -> Result<ChatMessage, AppError> {
        if !matches!(ctx.role, UserRole::Doctor | UserRole::Patient) {
            return Err(AppError::Forbidden(
                "Only consultation participants may send messages.".into(),
            ));
        }
        if ctx.role == UserRole::Doctor {
            ctx.verified_doctor_profile_id(state).await?;
        }
        let mut tx = state.begin_authed(ctx).await?;
        let conv = chat_repo::find_conversation(&mut *tx, conversation_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Chat conversation not found.".into()))?;
        crate::api::middleware::request_context::bind_consultation(
            conv.appointment_id,
            Some(conversation_id),
        );
        let apt = appointment_repo::find_by_id(&mut *tx, conv.appointment_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Appointment not found.".into()))?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
            .bind(format!("chat-send:{conversation_id}"))
            .execute(&mut *tx)
            .await?;
        let key_hash = key.map(|k| hex::encode(Sha256::digest(k.as_bytes())));
        if let Some(hash) = &key_hash {
            let existing:Option<Uuid>=sqlx::query_scalar("SELECT id FROM chat_messages WHERE conversation_id=$1 AND sender_id=$2 AND idempotency_key_hash=$3").bind(conversation_id).bind(sender_id).bind(hash).fetch_optional(&mut *tx).await?;
            if let Some(id) = existing {
                let message = chat_repo::find_message(&mut *tx, conversation_id, id)
                    .await?
                    .ok_or_else(|| AppError::internal("Message replay unavailable".into()))?;
                if message.content != content {
                    return Err(AppError::Conflict(
                        "This message key was used for different content.".into(),
                    ));
                }
                tx.commit().await?;
                return Ok(message);
            }
        }
        let active:bool=sqlx::query_scalar("SELECT NOT is_locked AND expires_at>clock_timestamp() FROM chat_conversations WHERE id=$1").bind(conversation_id).fetch_one(&mut *tx).await?;
        if !active
            || !matches!(
                apt.status,
                AppointmentStatus::Confirmed
                    | AppointmentStatus::Waiting
                    | AppointmentStatus::InConsultation
                    | AppointmentStatus::Completed
            )
        {
            tx.rollback().await?;
            consultation_trace::rejected(
                state,
                ctx,
                apt.id,
                Some(conversation_id),
                "CHAT_MESSAGE_REJECTED",
                "WINDOW_CLOSED",
            )
            .await?;
            return Err(AppError::Forbidden(
                "This consultation is read-only. The messaging window has ended.".into(),
            ));
        }
        let first:bool=sqlx::query_scalar("SELECT NOT EXISTS(SELECT 1 FROM chat_messages WHERE conversation_id=$1 AND sender_id=$2)").bind(conversation_id).bind(sender_id).fetch_one(&mut *tx).await?;
        let msg = ChatMessage {
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
        if let Err(error) = chat_repo::insert_message(&mut *tx, &msg).await {
            tx.rollback().await?;
            if error
                .as_database_error()
                .is_some_and(|e| e.code().as_deref() == Some("42501"))
            {
                consultation_trace::rejected(
                    state,
                    ctx,
                    apt.id,
                    Some(conversation_id),
                    "CHAT_MESSAGE_REJECTED",
                    "WINDOW_CLOSED",
                )
                .await?;
                return Err(AppError::Forbidden(
                    "This consultation is read-only.".into(),
                ));
            }
            return Err(error.into());
        }
        if let Some(hash) = key_hash {
            // Only this transaction may set the replay identity through a narrow DB function.
            sqlx::query("SELECT bind_chat_message_key($1,$2)")
                .bind(msg.id)
                .bind(hash)
                .execute(&mut *tx)
                .await?;
        }
        Self::record_committed_message(&mut tx, ctx, &conv, &msg, first).await?;
        tx.commit().await?;
        Ok(msg)
    }
    pub async fn record_committed_message(
        tx: &mut sqlx::PgConnection,
        ctx: &AuthContext,
        conv: &ChatConversation,
        msg: &ChatMessage,
        first: bool,
    ) -> Result<(), AppError> {
        consultation_trace::record(
            tx,
            conv.appointment_id,
            Some(conv.id),
            Event::accepted("CHAT_MESSAGE_SENT", Some(("MESSAGE", msg.id))),
        )
        .await?;
        if first {
            consultation_trace::record(
                tx,
                conv.appointment_id,
                Some(conv.id),
                Event::accepted("FIRST_CHAT_MESSAGE_SENT", Some(("MESSAGE", msg.id))),
            )
            .await?;
        }
        let recipient: UserId = sqlx::query_scalar("SELECT chat_counterparty_user($1)")
            .bind(conv.id)
            .fetch_one(&mut *tx)
            .await?;
        let notice = Uuid::new_v4();
        let body = if ctx.role == UserRole::Doctor {
            "You have a new message from your doctor."
        } else {
            "Your patient sent you a consultation message."
        };
        notification_repo::insert_if_new(
            &mut *tx,
            notice,
            recipient,
            "NEW_CHAT_MESSAGE",
            "New consultation message",
            body,
            Some("CHAT"),
            Some(conv.id),
            None,
            &format!("chat-message:{}:{recipient}", msg.id),
            Some(serde_json::json!({"message_id":msg.id,"appointment_id":conv.appointment_id})),
        )
        .await?;
        consultation_trace::record(
            tx,
            conv.appointment_id,
            Some(conv.id),
            Event::accepted(
                "NEW_CHAT_NOTIFICATION_CREATED",
                Some(("NOTIFICATION", notice)),
            ),
        )
        .await?;
        Ok(())
    }
    pub async fn get_messages(
        state: &AppState,
        ctx: &AuthContext,
        conversation_id: Uuid,
        before: Option<(DateTime<Utc>, Uuid)>,
        after: Option<(DateTime<Utc>, Uuid)>,
    ) -> Result<Vec<ChatMessage>, AppError> {
        if ctx.role == UserRole::Doctor {
            ctx.verified_doctor_profile_id(state).await?;
        }
        let mut tx = state.begin_authed(ctx).await?;
        let conv = chat_repo::find_conversation(&mut *tx, conversation_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Chat conversation not found.".into()))?;
        crate::api::middleware::request_context::bind_consultation(
            conv.appointment_id,
            Some(conversation_id),
        );
        let mut messages =
            chat_repo::list_messages(&mut *tx, conversation_id, before, after).await?;
        if after.is_none() {
            messages.reverse();
        }
        consultation_trace::admin_read(&mut tx, ctx, "ADMIN_CHAT_VIEWED", "CHAT", conv.id).await?;
        tx.commit().await?;
        Ok(messages)
    }
}
