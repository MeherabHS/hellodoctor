//! Clinical images use the existing chat attachment columns, private encrypted
//! DocumentStorageService and image security pipeline. No public object URLs.
use crate::services::consultation_trace::{self, Event};
use crate::{
    abac::{self, Action},
    auth_context::AuthContext,
    domain::models::*,
    error::AppError,
    rate_limit,
    repository::{appointment_repo, chat_repo, AppState},
    services::{
        document_storage_service::DocumentStorageService,
        upload_security_service::UploadSecurityService,
    },
};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::Utc;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClinicalImageRequest {
    pub mime_type: String,
    pub base64_content: String,
}

pub fn public_message(m: &ChatMessage) -> serde_json::Value {
    serde_json::json!({"id":m.id,"conversation_id":m.conversation_id,"sender_id":m.sender_id,
        "content":m.content,"is_read":m.is_read,"sent_at":m.sent_at,
        "attachment_id":m.attachment_object_key.as_ref().map(|_|m.id),
        "attachment":m.attachment_object_key.as_ref().map(|_|serde_json::json!({
            "id":m.id,"display_name":"Clinical image","mime_type":m.attachment_mime,"size_bytes":m.attachment_size}))})
}

async fn check_doctor_window(
    state: &AppState,
    ctx: &AuthContext,
    appointment: Uuid,
) -> Result<(), AppError> {
    if ctx.role == UserRole::Doctor {
        ctx.verified_doctor_profile_id(state).await?;
        let mut tx = state.begin_authed(ctx).await?;
        let allowed: bool = sqlx::query_scalar("SELECT doctor_own_encounter_allowed($1,$2)")
            .bind(appointment)
            .bind(ctx.user_id)
            .fetch_one(&mut *tx)
            .await?;
        tx.commit().await?;
        if !allowed {
            return Err(AppError::Forbidden(
                "Clinical documents are unavailable outside the authorized consultation window."
                    .into(),
            ));
        }
    }
    Ok(())
}

pub async fn upload(
    state: State<AppState>,
    ctx: AuthContext,
    path: Path<Uuid>,
    headers: HeaderMap,
    body: Json<ClinicalImageRequest>,
) -> Result<Response, AppError> {
    abac::authorize(&ctx, Action::UploadDoctorPrescription)?;
    ctx.verified_doctor_profile_id(&state).await?;
    let mut tx = state.begin_authed(&ctx).await?;
    let conversation = chat_repo::find_conversation(&mut *tx, path.0)
        .await?
        .ok_or_else(|| AppError::NotFound("Conversation not found.".into()))?;
    consultation_trace::record(
        &mut tx,
        conversation.appointment_id,
        Some(conversation.id),
        Event::accepted("ATTACHMENT_UPLOAD_STARTED", Some(("CHAT", conversation.id))),
    )
    .await?;
    tx.commit().await?;
    let result = upload_authorized(state.clone(), ctx.clone(), path, headers, body).await;
    if let Err(error) = &result {
        let reason = match error {
            AppError::Forbidden(_) => "WINDOW_CLOSED_OR_DENIED",
            AppError::PayloadTooLarge(_) => "UPLOAD_TOO_LARGE",
            AppError::UnprocessableEntity(_) | AppError::Validation(_) => "UPLOAD_INVALID",
            AppError::Conflict(_) => "UPLOAD_CONFLICT",
            _ => "UPLOAD_UNCONFIRMED",
        };
        consultation_trace::rejected(
            &state,
            &ctx,
            conversation.appointment_id,
            Some(conversation.id),
            "ATTACHMENT_UPLOAD_REJECTED",
            reason,
        )
        .await?;
    }
    result
}

async fn upload_authorized(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(conversation_id): Path<Uuid>,
    headers: HeaderMap,
    Json(body): Json<ClinicalImageRequest>,
) -> Result<Response, AppError> {
    abac::authorize(&ctx, Action::UploadDoctorPrescription)?;
    ctx.verified_doctor_profile_id(&state).await?;
    rate_limit::check_upload_limit(&state.redis, &ctx.user_id.to_string()).await?;
    let mut tx = state.begin_authed(&ctx).await?;
    let conv = chat_repo::find_conversation(&mut *tx, conversation_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Conversation not found.".into()))?;
    let appointment = appointment_repo::find_by_id(&mut *tx, conv.appointment_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Appointment not found.".into()))?;
    let authorization = if appointment.modality == Modality::Chat {
        "SELECT doctor_own_encounter_allowed($1,$2)"
    } else {
        "SELECT doctor_history_context_allowed($1,$2)"
    };
    let allowed: bool = sqlx::query_scalar(authorization)
        .bind(conv.appointment_id)
        .bind(ctx.user_id)
        .fetch_one(&mut *tx)
        .await?;
    if !allowed {
        return Err(AppError::Forbidden(
            "Clinical documents are unavailable outside the authorized consultation window.".into(),
        ));
    }
    if !matches!(
        appointment.status,
        AppointmentStatus::Confirmed
            | AppointmentStatus::Waiting
            | AppointmentStatus::InConsultation
            | AppointmentStatus::Completed
    ) || conv.is_locked
        || conv.expires_at <= Utc::now()
    {
        return Err(AppError::Forbidden(
            "This consultation is read-only.".into(),
        ));
    }
    consultation_trace::record(
        &mut tx,
        appointment.id,
        Some(conversation_id),
        Event::accepted(
            "ATTACHMENT_UPLOAD_AUTHORIZED",
            Some(("CHAT", conversation_id)),
        ),
    )
    .await?;
    tx.commit().await?;
    if !matches!(body.mime_type.as_str(), "image/jpeg" | "image/png") {
        return Err(AppError::UnprocessableEntity(
            "Select a JPEG or PNG clinical image.".into(),
        ));
    }
    if body.base64_content.len() > 14 * 1024 * 1024 {
        return Err(AppError::PayloadTooLarge(
            "The selected image exceeds the upload limit.".into(),
        ));
    }
    let raw = STANDARD
        .decode(&body.base64_content)
        .map_err(|_| AppError::UnprocessableEntity("Invalid image content.".into()))?;
    let mime = body.mime_type;
    let sanitized = UploadSecurityService::sanitize_async(raw, mime).await?;
    let hash = hex::encode(Sha256::digest(&sanitized));
    let mime = if sanitized.starts_with(&[0x89, 0x50, 0x4e, 0x47]) {
        "image/png"
    } else {
        "image/jpeg"
    };
    let key = headers
        .get("Idempotency-Key")
        .and_then(|v| v.to_str().ok())
        .filter(|v| !v.is_empty() && v.len() <= 128)
        .ok_or_else(|| {
            AppError::Validation("Idempotency-Key is required for attachment upload.".into())
        })?;
    let digest = Sha256::digest(format!(
        "clinical-image:{}:{conversation_id}:{key}",
        ctx.user_id
    ));
    let message_id = Uuid::from_slice(&digest[..16])
        .map_err(|_| AppError::internal("Attachment identity failed.".into()))?;
    let lock = i64::from_be_bytes(
        digest[..8]
            .try_into()
            .map_err(|_| AppError::internal("Attachment lock failed.".into()))?,
    );
    let mut tx = state.begin_authed(&ctx).await?;
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(lock)
        .execute(&mut *tx)
        .await?;
    let still_active: bool = sqlx::query_scalar("SELECT NOT is_locked AND expires_at > clock_timestamp() FROM chat_conversations WHERE id=$1")
        .bind(conversation_id).fetch_one(&mut *tx).await?;
    if !still_active {
        return Err(AppError::Forbidden(
            "This consultation is read-only.".into(),
        ));
    }
    // Text and attachment sends share one sender/conversation replay identity.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("chat-send:{conversation_id}"))
        .execute(&mut *tx)
        .await?;
    let replay_hash = hex::encode(Sha256::digest(key.as_bytes()));
    let logical:Option<Uuid>=sqlx::query_scalar("SELECT id FROM chat_messages WHERE conversation_id=$1 AND sender_id=$2 AND idempotency_key_hash=$3")
        .bind(conversation_id).bind(ctx.user_id).bind(&replay_hash).fetch_optional(&mut *tx).await?;
    if logical.is_some_and(|id| id != message_id) {
        return Err(AppError::Conflict(
            "This message key was used for different content.".into(),
        ));
    }
    if let Some(existing) = chat_repo::find_message(&mut *tx, conversation_id, message_id).await? {
        if existing.attachment_hash.as_deref() != Some(&hash) {
            return Err(AppError::Conflict(
                "This upload key was already used for another image.".into(),
            ));
        }
        tx.commit().await?;
        return Ok((
            StatusCode::CREATED,
            Json(serde_json::json!({"success":true,"data":public_message(&existing)})),
        )
            .into_response());
    }
    let object_key = format!("chat/{conversation_id}/{message_id}");
    let message = ChatMessage {
        id: message_id,
        conversation_id,
        sender_id: ctx.user_id,
        content: String::new(),
        attachment_bucket: Some(state.config.storage_bucket.clone()),
        attachment_object_key: Some(object_key.clone()),
        attachment_mime: Some(mime.into()),
        attachment_size: Some(sanitized.len() as i64),
        attachment_hash: Some(hash),
        is_read: false,
        sent_at: Utc::now(),
    };
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("chat-send:{conversation_id}"))
        .execute(&mut *tx)
        .await?;
    let first: bool = sqlx::query_scalar(
        "SELECT NOT EXISTS(SELECT 1 FROM chat_messages WHERE conversation_id=$1 AND sender_id=$2)",
    )
    .bind(conversation_id)
    .bind(ctx.user_id)
    .fetch_one(&mut *tx)
    .await?;
    DocumentStorageService::put(&state, &object_key, &sanitized).await?;
    if let Err(e) = chat_repo::insert_message(&mut *tx, &message).await {
        DocumentStorageService::delete(&state, &object_key).await;
        if e.as_database_error()
            .is_some_and(|error| error.code().as_deref() == Some("42501"))
        {
            return Err(AppError::Forbidden(
                "This consultation is read-only.".into(),
            ));
        }
        return Err(e.into());
    }
    if let Err(error) = sqlx::query("SELECT bind_chat_message_key($1,$2)")
        .bind(message_id)
        .bind(replay_hash)
        .execute(&mut *tx)
        .await
    {
        DocumentStorageService::delete(&state, &object_key).await;
        return Err(error.into());
    }
    if let Err(error) = consultation_trace::record(
        &mut tx,
        appointment.id,
        Some(conversation_id),
        Event::accepted(
            "ATTACHMENT_UPLOAD_COMPLETED",
            Some(("DOCUMENT", message_id)),
        ),
    )
    .await
    {
        DocumentStorageService::delete(&state, &object_key).await;
        return Err(error);
    }
    if let Err(error) = crate::services::ChatService::record_committed_message(
        &mut tx, &ctx, &conv, &message, first,
    )
    .await
    {
        DocumentStorageService::delete(&state, &object_key).await;
        return Err(error);
    }
    if let Err(e) = tx.commit().await {
        // A transport error can arrive after COMMIT succeeded. Deleting here
        // could destroy an authoritative attachment. Preserve private ciphertext
        // for idempotent retry / existing storage reconciliation instead.
        tracing::warn!(
            "Chat attachment commit unconfirmed; private storage reconciliation may be needed"
        );
        return Err(e.into());
    }
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({"success":true,"data":public_message(&message)})),
    )
        .into_response())
}

/// Request-bound authorization on every view, with no-store headers and no
/// reusable public URL, storage key or clinical bytes in a generic disk cache.
pub async fn view(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path((conversation_id, message_id)): Path<(Uuid, Uuid)>,
) -> Result<Response, AppError> {
    abac::authorize(&ctx, Action::AnyAuthenticated)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let conv = chat_repo::find_conversation(&mut *tx, conversation_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Conversation not found.".into()))?;
    let message = chat_repo::find_message(&mut *tx, conversation_id, message_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Attachment not found.".into()))?;
    tx.commit().await?;
    check_doctor_window(&state, &ctx, conv.appointment_id).await?;
    let key = message
        .attachment_object_key
        .ok_or_else(|| AppError::NotFound("Attachment not found.".into()))?;
    let hash = message
        .attachment_hash
        .ok_or_else(|| AppError::NotFound("Attachment not found.".into()))?;
    let mut trace_tx = state.begin_authed(&ctx).await?;
    consultation_trace::record(
        &mut trace_tx,
        conv.appointment_id,
        Some(conversation_id),
        Event::accepted("ATTACHMENT_VIEW_AUTHORIZED", Some(("DOCUMENT", message_id))),
    )
    .await?;
    trace_tx.commit().await?;
    let bytes = DocumentStorageService::get(&state, &key, &hash).await?;
    let mut trace_tx = state.begin_authed(&ctx).await?;
    consultation_trace::record(
        &mut trace_tx,
        conv.appointment_id,
        Some(conversation_id),
        Event::accepted("ATTACHMENT_VIEWED", Some(("DOCUMENT", message_id))),
    )
    .await?;
    consultation_trace::admin_read(
        &mut trace_tx,
        &ctx,
        "ADMIN_ATTACHMENT_VIEWED",
        "DOCUMENT",
        message_id,
    )
    .await?;
    trace_tx.commit().await?;
    Ok(([("Cache-Control","no-store, private"),("Pragma","no-cache"),("X-Content-Type-Options","nosniff")],
        Json(serde_json::json!({"success":true,"data":{"mime_type":message.attachment_mime,"base64_content":STANDARD.encode(bytes)}}))).into_response())
}
