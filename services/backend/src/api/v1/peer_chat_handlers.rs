//! Peer Navigator chat, Phase 3a (migration `20261222000006_peer_chat`).
//! Same message rules as doctor chat — 2,000 characters, `Idempotency-Key`
//! duplicate protection, the chat rate limit, image attachments compressed
//! and stored encrypted — but the conversation never locks. Both the patient
//! and the peer use these endpoints; row-level security limits every query
//! to the conversation's two parties.

use crate::abac::{self, Action};
use crate::auth_context::AuthContext;
use crate::domain::ids::UserId;
use crate::domain::models::UserRole;
use crate::error::AppError;
use crate::repository::AppState;
use crate::services::document_storage_service::DocumentStorageService;
use crate::services::upload_security_service::UploadSecurityService;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;

fn ok<T: Serialize>(data: T) -> Json<serde_json::Value> {
    Json(json!({ "success": true, "data": data }))
}

fn require_party_role(ctx: &AuthContext) -> Result<(), AppError> {
    abac::authorize(ctx, Action::AnyAuthenticated)?;
    if !matches!(ctx.role, UserRole::Patient | UserRole::Peer) {
        return Err(AppError::Forbidden(
            "Peer support chat is for the patient and their peer navigator.".into(),
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct PeerChatSummary {
    pub id: Uuid,
    pub request_id: Uuid,
    /// Patient side: the peer's name. Peer side: "Amina O."
    pub counterpart_name: Option<String>,
    pub support_type: String,
    pub request_status: String,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub last_message_at: Option<DateTime<Utc>>,
    pub last_message: Option<String>,
    pub unread_count: i64,
    #[serde(skip)]
    pub call_ended: bool,
    #[sqlx(default)]
    pub call_closed: bool,
}

impl crate::repository::peer_repo::PeerCallWindow for PeerChatSummary {
    fn call_window(&self) -> (&str, DateTime<Utc>, bool) {
        (&self.request_status, self.end_time, self.call_ended)
    }
    fn set_call_closed(&mut self, closed: bool) {
        self.call_closed = closed;
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct PeerMessageRow {
    id: Uuid,
    conversation_id: Uuid,
    sender_id: UserId,
    content: String,
    attachment_object_key: Option<String>,
    attachment_mime: Option<String>,
    attachment_size: Option<i64>,
    attachment_hash: Option<String>,
    is_read: bool,
    sent_at: DateTime<Utc>,
}

const MESSAGE_COLUMNS: &str = "id, conversation_id, sender_id, content, attachment_object_key, \
    attachment_mime, attachment_size, attachment_hash, is_read, sent_at";

/// Same public shape as doctor chat messages, so the Flutter chat screens can
/// render both.
fn public_message(m: &PeerMessageRow) -> serde_json::Value {
    json!({
        "id": m.id, "conversation_id": m.conversation_id, "sender_id": m.sender_id,
        "content": m.content, "is_read": m.is_read, "sent_at": m.sent_at,
        "attachment_id": m.attachment_object_key.as_ref().map(|_| m.id),
        "attachment": m.attachment_object_key.as_ref().map(|_| json!({
            "id": m.id, "display_name": "Image", "mime_type": m.attachment_mime,
            "size_bytes": m.attachment_size
        }))
    })
}

/// `GET /peer-chats` — the caller's peer conversations, most recent first.
pub async fn list(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<impl IntoResponse, AppError> {
    require_party_role(&ctx)?;
    let counterpart = if ctx.role == UserRole::Peer {
        "r.patient_display_name"
    } else {
        "p.full_name"
    };
    let mut tx = state.begin_authed(&ctx).await?;
    let mut rows = sqlx::query_as::<_, PeerChatSummary>(&format!(
        "SELECT c.id, c.request_id, {counterpart} AS counterpart_name, r.support_type, \
                r.status AS request_status, s.start_time, s.end_time, c.last_message_at, \
                (SELECT CASE WHEN m.content <> '' THEN left(m.content, 120) ELSE '[Image]' END \
                   FROM peer_messages m WHERE m.conversation_id = c.id \
                   ORDER BY m.sent_at DESC, m.id DESC LIMIT 1) AS last_message, \
                (SELECT count(*) FROM peer_messages m WHERE m.conversation_id = c.id \
                   AND NOT m.is_read AND m.sender_id <> $1) AS unread_count, \
                EXISTS (SELECT 1 FROM peer_call_sessions pc WHERE pc.request_id = r.id \
                   AND pc.status = 'ENDED') AS call_ended \
         FROM peer_conversations c \
         JOIN peer_requests r ON r.id = c.request_id \
         JOIN peer_slots s ON s.id = r.slot_id \
         LEFT JOIN peer_profiles p ON p.id = r.peer_id \
         ORDER BY COALESCE(c.last_message_at, c.created_at) DESC LIMIT 200"
    ))
    .bind(ctx.user_id)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    crate::repository::peer_repo::mark_call_closed(
        &mut rows,
        state.config.agora_token_window_after_mins,
        chrono::Utc::now(),
    );
    Ok(ok(rows))
}

async fn find_conversation(
    tx: &mut sqlx::PgConnection,
    id: Uuid,
) -> Result<(UserId, UserId), AppError> {
    sqlx::query_as::<_, (UserId, UserId)>(
        "SELECT patient_user_id, peer_user_id FROM peer_conversations WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Chat not found.".into()))
}

#[derive(Default, Deserialize)]
pub struct MessagePage {
    pub before_time: Option<DateTime<Utc>>,
    pub before_id: Option<Uuid>,
}

/// `GET /peer-chats/:id/messages` — up to 50 messages, oldest first; page
/// back with `before_time` and `before_id` from the oldest one shown.
pub async fn messages(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
    Query(page): Query<MessagePage>,
) -> Result<impl IntoResponse, AppError> {
    require_party_role(&ctx)?;
    let mut tx = state.begin_authed(&ctx).await?;
    find_conversation(&mut tx, id).await?;
    let mut rows = sqlx::query_as::<_, PeerMessageRow>(&format!(
        "SELECT {MESSAGE_COLUMNS} FROM peer_messages WHERE conversation_id = $1 \
         AND ($2::timestamptz IS NULL OR (sent_at, id) < ($2, COALESCE($3, '00000000-0000-0000-0000-000000000000'::uuid))) \
         ORDER BY sent_at DESC, id DESC LIMIT 50"
    ))
    .bind(id)
    .bind(page.before_time)
    .bind(page.before_id)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    rows.reverse();
    Ok(ok(rows.iter().map(public_message).collect::<Vec<_>>()))
}

#[derive(Deserialize)]
pub struct SendPeerMessage {
    pub content: String,
}

fn message_key(headers: &HeaderMap) -> Result<Option<String>, AppError> {
    let key = headers.get("Idempotency-Key").and_then(|v| v.to_str().ok());
    if key.is_some_and(|k| k.is_empty() || k.len() > 128) {
        return Err(AppError::Validation("Invalid message request key.".into()));
    }
    Ok(key.map(|k| hex::encode(Sha256::digest(k.as_bytes()))))
}

/// Inserts a message (or replays the same keyed one), bumps the
/// conversation and notifies the other party without message content.
async fn store_message(
    state: &AppState,
    ctx: &AuthContext,
    conversation_id: Uuid,
    content: &str,
    attachment: Option<(&str, &str, i64, &str)>,
    key_hash: Option<&str>,
    message_id: Uuid,
) -> Result<PeerMessageRow, AppError> {
    let mut tx = state.begin_authed(ctx).await?;
    let (patient, peer) = find_conversation(&mut tx, conversation_id).await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("peer-chat-send:{conversation_id}"))
        .execute(&mut *tx)
        .await?;
    if let Some(hash) = key_hash {
        let existing = sqlx::query_as::<_, PeerMessageRow>(&format!(
            "SELECT {MESSAGE_COLUMNS} FROM peer_messages \
             WHERE conversation_id = $1 AND sender_id = $2 AND idempotency_key_hash = $3"
        ))
        .bind(conversation_id)
        .bind(ctx.user_id)
        .bind(hash)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(existing) = existing {
            if existing.content != content {
                return Err(AppError::Conflict(
                    "This message key was used for different content.".into(),
                ));
            }
            tx.commit().await?;
            return Ok(existing);
        }
    }
    let (key, mime, size, hash) = match attachment {
        Some((k, m, s, h)) => (Some(k), Some(m), Some(s), Some(h)),
        None => (None, None, None, None),
    };
    let row = sqlx::query_as::<_, PeerMessageRow>(&format!(
        "INSERT INTO peer_messages (id, conversation_id, sender_id, content, attachment_object_key, \
         attachment_mime, attachment_size, attachment_hash, idempotency_key_hash) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9) RETURNING {MESSAGE_COLUMNS}"
    ))
    .bind(message_id)
    .bind(conversation_id)
    .bind(ctx.user_id)
    .bind(content)
    .bind(key)
    .bind(mime)
    .bind(size)
    .bind(hash)
    .bind(key_hash)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query("UPDATE peer_conversations SET last_message_at = $2 WHERE id = $1")
        .bind(conversation_id)
        .bind(row.sent_at)
        .execute(&mut *tx)
        .await?;
    let recipient = if ctx.user_id == patient {
        peer
    } else {
        patient
    };
    let language = crate::services::NotificationService::user_language(&state.system_db, recipient)
        .await
        .unwrap_or_else(|_| "en".into());
    let sw = language == "sw";
    let (title, body) = crate::services::NotificationService::render(
        &mut *tx,
        "new_peer_chat_message",
        if sw { "sw" } else { "en" },
        &[],
        if sw { "Ujumbe mpya" } else { "New message" },
        if sw {
            "Una ujumbe mpya kwenye mazungumzo ya msaada rika."
        } else {
            "You have a new message in your peer support chat."
        },
    )
    .await?;
    crate::repository::notification_repo::insert_if_new(
        &mut *tx,
        Uuid::new_v4(),
        recipient,
        "NEW_CHAT_MESSAGE",
        &title,
        &body,
        Some("PEER_CHAT"),
        Some(conversation_id),
        None,
        &format!("peer-chat-message:{}", row.id),
        None,
    )
    .await?;
    tx.commit().await?;
    Ok(row)
}

/// `POST /peer-chats/:id/messages` — never locked; only the two parties.
pub async fn send(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(body): Json<SendPeerMessage>,
) -> Result<impl IntoResponse, AppError> {
    require_party_role(&ctx)?;
    crate::rate_limit::check_chat_message_limit(&state.redis, &ctx.user_id.to_string()).await?;
    let content = crate::domain::validation::validate_text_field(&body.content, "Message", 2000)?;
    if content.is_empty() {
        return Err(AppError::Validation("Message must not be empty.".into()));
    }
    let key = message_key(&headers)?;
    let row = store_message(
        &state,
        &ctx,
        id,
        &content,
        None,
        key.as_deref(),
        Uuid::new_v4(),
    )
    .await?;
    Ok((StatusCode::CREATED, ok(public_message(&row))))
}

#[derive(Deserialize)]
pub struct PeerImage {
    pub mime_type: String,
    pub base64_content: String,
}

/// `POST /peer-chats/:id/attachments` — one image, compressed and stored
/// encrypted exactly like doctor chat images.
pub async fn upload_image(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(body): Json<PeerImage>,
) -> Result<impl IntoResponse, AppError> {
    require_party_role(&ctx)?;
    // Photos only: no PDFs or other files between a patient and a peer.
    if !matches!(body.mime_type.as_str(), "image/jpeg" | "image/png") {
        return Err(AppError::UnprocessableEntity(
            "Select a JPEG or PNG photo.".into(),
        ));
    }
    if body.base64_content.len() > 14 * 1024 * 1024 {
        return Err(AppError::PayloadTooLarge(
            "The selected photo exceeds the upload limit.".into(),
        ));
    }
    crate::rate_limit::check_upload_limit(&state.redis, &ctx.user_id.to_string()).await?;
    {
        let mut tx = state.begin_authed(&ctx).await?;
        find_conversation(&mut tx, id).await?;
        tx.commit().await?;
    }
    let raw = STANDARD
        .decode(
            body.base64_content
                .split(',')
                .next_back()
                .unwrap_or(&body.base64_content),
        )
        .map_err(|_| AppError::UnprocessableEntity("Image content is not valid base64.".into()))?;
    let sanitized = UploadSecurityService::sanitize_async(raw, body.mime_type.clone()).await?;
    let hash = hex::encode(Sha256::digest(&sanitized));
    let message_id = Uuid::new_v4();
    let object_key = format!("peer-chat/{id}/{message_id}");
    DocumentStorageService::put(&state, &object_key, &sanitized).await?;
    let key = message_key(&headers)?;
    match store_message(
        &state,
        &ctx,
        id,
        "",
        Some((&object_key, &body.mime_type, sanitized.len() as i64, &hash)),
        key.as_deref(),
        message_id,
    )
    .await
    {
        Ok(row) => {
            if row.id != message_id {
                // A replayed key returned the earlier message; drop this copy.
                DocumentStorageService::delete(&state, &object_key).await;
            }
            Ok((StatusCode::CREATED, ok(public_message(&row))))
        }
        Err(error) => {
            DocumentStorageService::delete(&state, &object_key).await;
            Err(error)
        }
    }
}

/// `GET /peer-chats/:id/messages/:message_id/attachment` — the image bytes,
/// never cached.
pub async fn view_image(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path((id, message_id)): Path<(Uuid, Uuid)>,
) -> Result<Response, AppError> {
    require_party_role(&ctx)?;
    let mut tx = state.begin_authed(&ctx).await?;
    find_conversation(&mut tx, id).await?;
    let message = sqlx::query_as::<_, PeerMessageRow>(&format!(
        "SELECT {MESSAGE_COLUMNS} FROM peer_messages WHERE conversation_id = $1 AND id = $2"
    ))
    .bind(id)
    .bind(message_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Image not found.".into()))?;
    tx.commit().await?;
    let (Some(key), Some(hash)) = (message.attachment_object_key, message.attachment_hash) else {
        return Err(AppError::NotFound("Image not found.".into()));
    };
    let bytes = DocumentStorageService::get(&state, &key, &hash).await?;
    Ok((
        [
            ("Cache-Control", "no-store, private"),
            ("Pragma", "no-cache"),
            ("X-Content-Type-Options", "nosniff"),
        ],
        Json(json!({"success": true, "data": {
            "mime_type": message.attachment_mime, "base64_content": STANDARD.encode(bytes)
        }})),
    )
        .into_response())
}

/// `POST /peer-chats/:id/read` — marks the other party's messages read.
pub async fn mark_read(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    require_party_role(&ctx)?;
    let mut tx = state.begin_authed(&ctx).await?;
    find_conversation(&mut tx, id).await?;
    let updated = sqlx::query(
        "UPDATE peer_messages SET is_read = true \
         WHERE conversation_id = $1 AND sender_id <> $2 AND NOT is_read",
    )
    .bind(id)
    .bind(ctx.user_id)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    tx.commit().await?;
    Ok(ok(json!({ "marked_read": updated })))
}

/// `GET /peer-requests/:id/call` — whether the caller can join the support
/// call now (same join window as doctor video), and the room's state.
pub async fn call_state(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    require_party_role(&ctx)?;
    let call = crate::services::peer_call_service::PeerCallService::state(&state, &ctx, id).await?;
    Ok(ok(call))
}

/// `POST /peer-requests/:id/rtc-token` — an Agora token for the support
/// call (voice or video), only inside the join window.
pub async fn call_token(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    require_party_role(&ctx)?;
    let token =
        crate::services::peer_call_service::PeerCallService::mint_token(&state, &ctx, id).await?;
    // Same response shape as the doctor consultation token.
    Ok(ok(json!({
        "channel_name": token.channel_name,
        "token": token.token,
        "uid": token.uid,
        "app_id": token.app_id,
        "expires_in": token.expires_in
    })))
}
