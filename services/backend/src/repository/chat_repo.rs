use crate::domain::models::{ChatConversation, ChatMessage};
use sqlx::PgExecutor;
use uuid::Uuid;

pub async fn insert_conversation<'e, E: PgExecutor<'e>>(executor: E, conv: &ChatConversation) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO chat_conversations (id, appointment_id, patient_id, doctor_id, expires_at, is_locked, created_at) VALUES ($1,$2,$3,$4,$5,$6,$7)",
    )
    .bind(conv.id)
    .bind(conv.appointment_id)
    .bind(conv.patient_id)
    .bind(conv.doctor_id)
    .bind(conv.expires_at)
    .bind(conv.is_locked)
    .bind(conv.created_at)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn find_conversation<'e, E: PgExecutor<'e>>(executor: E, id: Uuid) -> Result<Option<ChatConversation>, sqlx::Error> {
    sqlx::query_as::<_, ChatConversation>(
        "SELECT id, appointment_id, patient_id, doctor_id, expires_at, is_locked, created_at FROM chat_conversations WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(executor)
    .await
}

pub async fn lock_conversation<'e, E: PgExecutor<'e>>(executor: E, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE chat_conversations SET is_locked = TRUE WHERE id = $1")
        .bind(id)
        .execute(executor)
        .await?;
    Ok(())
}

pub async fn insert_message<'e, E: PgExecutor<'e>>(executor: E, msg: &ChatMessage) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO chat_messages (id, conversation_id, sender_id, content, attachment_bucket, attachment_object_key, attachment_mime, attachment_size, attachment_hash, is_read, sent_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)",
    )
    .bind(msg.id)
    .bind(msg.conversation_id)
    .bind(msg.sender_id)
    .bind(&msg.content)
    .bind(&msg.attachment_bucket)
    .bind(&msg.attachment_object_key)
    .bind(&msg.attachment_mime)
    .bind(msg.attachment_size)
    .bind(&msg.attachment_hash)
    .bind(msg.is_read)
    .bind(msg.sent_at)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn list_messages<'e, E: PgExecutor<'e>>(executor: E, conversation_id: Uuid) -> Result<Vec<ChatMessage>, sqlx::Error> {
    sqlx::query_as::<_, ChatMessage>(
        "SELECT id, conversation_id, sender_id, content, attachment_bucket, attachment_object_key, attachment_mime, attachment_size, attachment_hash, is_read, sent_at \
         FROM chat_messages WHERE conversation_id = $1 ORDER BY sent_at",
    )
    .bind(conversation_id)
    .fetch_all(executor)
    .await
}
