//! Doctor Web redesign (2026-11-14, product-owner approved): professional
//! verification resubmission. A PENDING or REJECTED Doctor uploads 1-5 supporting
//! documents (JPEG/PNG/PDF); a REJECTED registration returns to PENDING for a new
//! Admin review. Files are sanitised, encrypted and served only to their owner and Admins.
use crate::{
    abac::{self, Action},
    auth_context::AuthContext,
    domain::ids::DoctorProfileId,
    error::AppError,
    repository::AppState,
    services::{
        article_media_service,
        audit_service::{AuditLogEntry, AuditService},
        document_storage_service::DocumentStorageService,
        upload_security_service::UploadSecurityService,
    },
};
use axum::{
    extract::{Path, State},
    http::header,
    response::{IntoResponse, Response},
    Json,
};
use base64::Engine;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::Row;
use uuid::Uuid;

pub const MAX_DOCUMENTS: usize = 5;
pub const MAX_SUBMISSION_BODY: usize = 30 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentUpload {
    pub mime_type: String,
    pub base64_content: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmissionRequest {
    #[serde(default)]
    pub message: Option<String>,
    pub documents: Vec<DocumentUpload>,
}

/// Images are re-encoded to a readable JPEG (at most 500 KB, metadata stripped);
/// PDFs pass the shared structure validation unchanged.
async fn sanitize(doc: DocumentUpload) -> Result<(String, Vec<u8>), AppError> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(doc.base64_content.split(',').next_back().unwrap_or(""))
        .map_err(|_| AppError::Validation("Document content is not valid base64.".into()))?;
    match doc.mime_type.as_str() {
        "image/jpeg" | "image/png" => Ok((
            "image/jpeg".into(),
            article_media_service::sanitize_async(bytes, doc.mime_type).await?,
        )),
        "application/pdf" => Ok((
            "application/pdf".into(),
            UploadSecurityService::sanitize_async(bytes, doc.mime_type).await?,
        )),
        _ => Err(AppError::Validation(
            "Documents must be JPEG, PNG or PDF files.".into(),
        )),
    }
}

pub async fn submit(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(payload): Json<SubmissionRequest>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::DoctorManageProfile)?;
    let doctor_id = ctx.doctor_profile_id(&state).await?;
    if payload.documents.is_empty() || payload.documents.len() > MAX_DOCUMENTS {
        return Err(AppError::Validation(
            "Attach between 1 and 5 documents.".into(),
        ));
    }
    let message = match payload.message.as_deref().map(str::trim) {
        Some("") | None => None,
        Some(text) if text.chars().count() > 1000 || text.contains('\0') => {
            return Err(AppError::Validation(
                "The message must be at most 1000 characters.".into(),
            ))
        }
        Some(text) => Some(text.to_owned()),
    };
    crate::rate_limit::check_upload_limit(&state.redis, &ctx.user_id.to_string()).await?;
    let _guard =
        crate::rate_limit::acquire_upload_concurrency(&state.redis, &ctx.user_id.to_string(), 1)
            .await?;
    // Status preflight before any decoding or storage work.
    let status = current_status(&state, &ctx, doctor_id).await?;
    if !matches!(status.as_str(), "PENDING" | "REJECTED") {
        return Err(AppError::Conflict(
            "Documents can only be submitted while verification is pending or after it was not approved.".into(),
        ));
    }
    let mut prepared = Vec::with_capacity(payload.documents.len());
    for doc in payload.documents {
        prepared.push(sanitize(doc).await?);
    }
    let submission_id = Uuid::new_v4();
    let mut stored: Vec<(Uuid, String, String, String, i32)> = Vec::new();
    for (mime, bytes) in &prepared {
        let id = Uuid::new_v4();
        let key = format!("doctor-verification/{doctor_id}/{id}.bin");
        if let Err(error) = DocumentStorageService::put(&state, &key, bytes).await {
            for (_, key, ..) in &stored {
                DocumentStorageService::delete(&state, key).await;
            }
            return Err(error);
        }
        stored.push((
            id,
            key,
            mime.clone(),
            hex::encode(Sha256::digest(bytes)),
            bytes.len() as i32,
        ));
    }
    let result = record(&state, &ctx, doctor_id, submission_id, message, &stored).await;
    if result.is_err() {
        for (_, key, ..) in &stored {
            DocumentStorageService::delete(&state, key).await;
        }
    }
    result
}

async fn current_status(
    state: &AppState,
    ctx: &AuthContext,
    doctor_id: DoctorProfileId,
) -> Result<String, AppError> {
    let mut tx = state.begin_authed(ctx).await?;
    let status: String = sqlx::query_scalar("SELECT professional_verification_status::text FROM doctor_profiles WHERE id=$1 AND deleted_at IS NULL")
        .bind(doctor_id).fetch_optional(&mut *tx).await?
        .ok_or_else(|| AppError::NotFound("Doctor profile not found.".into()))?;
    tx.commit().await?;
    Ok(status)
}

async fn record(
    state: &AppState,
    ctx: &AuthContext,
    doctor_id: DoctorProfileId,
    submission_id: Uuid,
    message: Option<String>,
    stored: &[(Uuid, String, String, String, i32)],
) -> Result<axum::response::Response, AppError> {
    let mut tx = state.begin_authed(ctx).await?;
    let row = sqlx::query("SELECT professional_verification_status::text AS status, professional_verification_note AS note FROM doctor_profiles WHERE id=$1 AND deleted_at IS NULL FOR UPDATE")
        .bind(doctor_id).fetch_optional(&mut *tx).await?
        .ok_or_else(|| AppError::NotFound("Doctor profile not found.".into()))?;
    let status: String = row.get("status");
    if !matches!(status.as_str(), "PENDING" | "REJECTED") {
        return Err(AppError::Conflict(
            "Verification status changed. Refresh and try again.".into(),
        ));
    }
    let previous_note: Option<String> = row.get("note");
    sqlx::query("INSERT INTO doctor_verification_submissions(id,doctor_id,message,previous_status,previous_note) VALUES($1,$2,$3,$4::professional_verification_status_enum,$5)")
        .bind(submission_id).bind(doctor_id).bind(&message).bind(&status).bind(&previous_note).execute(&mut *tx).await?;
    for (id, key, mime, hash, size) in stored {
        sqlx::query("INSERT INTO doctor_verification_documents(id,submission_id,doctor_id,object_key,mime_type,content_hash,size_bytes) VALUES($1,$2,$3,$4,$5,$6,$7)")
            .bind(id).bind(submission_id).bind(doctor_id).bind(key).bind(mime).bind(hash).bind(size).execute(&mut *tx).await?;
    }
    if status == "REJECTED" {
        sqlx::query("UPDATE doctor_profiles SET professional_verification_status='PENDING', professional_verification_reviewed_at=NULL, professional_verification_reviewed_by=NULL, professional_verification_note=NULL, updated_at=NOW() WHERE id=$1 AND professional_verification_status='REJECTED'")
            .bind(doctor_id).execute(&mut *tx).await?;
    }
    AuditService::log_in_tx(
        &mut *tx,
        AuditLogEntry::new(Some(ctx.user_id), ctx.role, "DOCTOR_VERIFICATION_RESUBMIT", "DOCTOR_PROFILE", Some(doctor_id.into_inner()), "SUCCESS")
            .with_before(json!({"professional_verification_status": status}))
            .with_after(json!({"professional_verification_status": "PENDING", "documents": stored.len(), "submission_id": submission_id})),
    )
    .await?;
    tx.commit().await?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(json!({"success": true, "data": {"submission_id": submission_id, "professional_verification_status": "PENDING", "documents": stored.len()}})),
    )
        .into_response())
}

fn submissions_json(rows: Vec<sqlx::postgres::PgRow>) -> Vec<Value> {
    rows.into_iter().map(|r| json!({
        "id": r.get::<Uuid,_>("id"), "message": r.get::<Option<String>,_>("message"),
        "previous_status": r.get::<String,_>("previous_status"), "previous_note": r.get::<Option<String>,_>("previous_note"),
        "created_at": r.get::<chrono::DateTime<chrono::Utc>,_>("created_at"), "documents": r.get::<Value,_>("documents"),
    })).collect()
}
const SUBMISSIONS: &str = "SELECT s.id,s.message,s.previous_status::text AS previous_status,s.previous_note,s.created_at, \
    COALESCE((SELECT jsonb_agg(jsonb_build_object('id',d.id,'mime_type',d.mime_type,'size_bytes',d.size_bytes,'created_at',d.created_at) ORDER BY d.created_at,d.id) \
      FROM doctor_verification_documents d WHERE d.submission_id=s.id),'[]'::jsonb) AS documents \
    FROM doctor_verification_submissions s WHERE s.doctor_id=$1 ORDER BY s.created_at DESC LIMIT 20";

pub async fn own_submissions(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<Json<Value>, AppError> {
    abac::authorize(&ctx, Action::DoctorManageProfile)?;
    let doctor_id = ctx.doctor_profile_id(&state).await?;
    let mut tx = state.begin_authed(&ctx).await?;
    let rows = sqlx::query(SUBMISSIONS)
        .bind(doctor_id)
        .fetch_all(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(
        json!({"success": true, "data": submissions_json(rows)}),
    ))
}

pub async fn admin_submissions(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(doctor_id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    abac::authorize(&ctx, Action::ManageProfessionalVerification)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let rows = sqlx::query(SUBMISSIONS)
        .bind(doctor_id)
        .fetch_all(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(
        json!({"success": true, "data": submissions_json(rows)}),
    ))
}

pub async fn admin_document(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path((doctor_id, document_id)): Path<(Uuid, Uuid)>,
) -> Result<Response, AppError> {
    abac::authorize(&ctx, Action::ManageProfessionalVerification)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let row = sqlx::query("SELECT object_key,mime_type,content_hash FROM doctor_verification_documents WHERE id=$1 AND doctor_id=$2")
        .bind(document_id).bind(doctor_id).fetch_optional(&mut *tx).await?
        .ok_or_else(|| AppError::NotFound("Verification document not found.".into()))?;
    AuditService::log_in_tx(
        &mut *tx,
        AuditLogEntry::new(
            Some(ctx.user_id),
            ctx.role,
            "DOCTOR_VERIFICATION_DOCUMENT_VIEWED",
            "DOCTOR_PROFILE",
            Some(doctor_id),
            "SUCCESS",
        )
        .with_after(json!({"document_id": document_id})),
    )
    .await?;
    tx.commit().await?;
    let mime: String = row.get("mime_type");
    let bytes = DocumentStorageService::get(
        &state,
        &row.get::<String, _>("object_key"),
        &row.get::<String, _>("content_hash"),
    )
    .await?;
    Ok((
        [
            (header::CONTENT_TYPE, mime),
            (header::CACHE_CONTROL, "private, no-store".to_owned()),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_owned()),
            (
                header::CONTENT_SECURITY_POLICY,
                "sandbox; default-src 'none'".to_owned(),
            ),
        ],
        bytes,
    )
        .into_response())
}
