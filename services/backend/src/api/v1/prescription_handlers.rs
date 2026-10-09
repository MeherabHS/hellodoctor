use crate::abac::{self, Action};
use crate::api::middleware::idempotency::Idempotency;
use crate::auth_context::AuthContext;
use crate::domain::ids::UserId;
use crate::domain::models::UserRole;
use crate::error::AppError;
use crate::rate_limit;
use crate::repository::{prescription_repo, AppState};
use crate::services::{document_storage_service::DocumentStorageService, PrescriptionService};
use axum::{
    extract::{Path, Query, State},
    response::{IntoResponse, Response},
    Json,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct PatientIntakeUploadRequest {
    pub files: Vec<FileData>,
}

#[derive(Debug, Deserialize)]
pub struct FileData {
    pub mime_type: String,
    pub base64_content: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DoctorRxPhotoUploadRequest {
    pub base64_photo: String,
    pub doctor_notes: Option<String>,
    pub mime_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CompleteNoRxRequest {
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: T,
}

fn decode_base64_payload(raw: &str) -> Result<Vec<u8>, AppError> {
    let stripped = raw.split(',').next_back().unwrap_or(raw);
    STANDARD
        .decode(stripped)
        .map_err(|_| AppError::UnprocessableEntity("File content is not valid base64.".into()))
}

pub async fn upload_patient_intake(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
    idempotency: Idempotency,
    Json(payload): Json<PatientIntakeUploadRequest>,
) -> Result<Response, AppError> {
    abac::authorize(&ctx, Action::UploadPatientIntake)?;
    rate_limit::check_upload_limit(&state.redis, &ctx.user_id.to_string()).await?;

    let patient_id = ctx.patient_profile_id(&state).await?;
    if let Some(response) = idempotency.replay(&state, &ctx, Some(id)).await? {
        return Ok(response);
    }
    let mut files = Vec::new();

    for f in payload.files {
        let bytes = decode_base64_payload(&f.base64_content)?;
        files.push((f.mime_type, bytes));
    }

    let uploaded =
        PrescriptionService::upload_patient_intake_documents(&state, &ctx, id, patient_id, files)
            .await?;

    Ok(idempotency.finish(
        id,
        (
            axum::http::StatusCode::CREATED,
            Json(ApiResponse {
                success: true,
                data: serde_json::json!({
                    "appointment_id": id,
                    "uploaded_count": uploaded.len(),
                    "documents": uploaded
                }),
            }),
        )
            .into_response(),
    ))
}

pub async fn upload_doctor_prescription(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(appointment_id): Path<Uuid>,
    idempotency: Idempotency,
    Json(payload): Json<DoctorRxPhotoUploadRequest>,
) -> Result<Response, AppError> {
    abac::authorize(&ctx, Action::UploadDoctorPrescription)?;
    rate_limit::check_upload_limit(&state.redis, &ctx.user_id.to_string()).await?;
    if payload
        .doctor_notes
        .as_ref()
        .is_some_and(|notes| notes.len() > 4000)
    {
        return Err(AppError::UnprocessableEntity(
            "Prescription notes must be at most 4000 characters.".into(),
        ));
    }
    let doctor_id = ctx.doctor_profile_id(&state).await?;
    if let Some(response) = idempotency
        .replay(&state, &ctx, Some(appointment_id))
        .await?
    {
        return Ok(response);
    }
    let bytes = decode_base64_payload(&payload.base64_photo)?;

    let rx = PrescriptionService::upload_doctor_prescription_photo(
        &state,
        &ctx,
        appointment_id,
        doctor_id,
        bytes,
        payload.mime_type,
        payload.doctor_notes,
    )
    .await?;

    Ok(idempotency.finish(
        appointment_id,
        (
            axum::http::StatusCode::CREATED,
            Json(ApiResponse {
                success: true,
                data: rx,
            }),
        )
            .into_response(),
    ))
}

pub async fn complete_no_rx(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(appointment_id): Path<Uuid>,
    Json(payload): Json<CompleteNoRxRequest>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::UploadDoctorPrescription)?;
    let reason = crate::domain::validation::validate_text_field(&payload.reason, "Reason", 1000)?;
    if reason.is_empty() {
        return Err(AppError::Validation(
            "A reason is required to complete without a prescription.".into(),
        ));
    }
    let doctor_id = ctx.doctor_profile_id(&state).await?;
    let rx = PrescriptionService::complete_without_prescription(
        &state,
        &ctx,
        appointment_id,
        doctor_id,
        reason,
    )
    .await?;

    Ok(Json(ApiResponse {
        success: true,
        data: rx,
    }))
}

/// `document_id` may refer to either a patient-uploaded intake document
/// (`prescription_intake_documents`) or a doctor-issued prescription image
/// (`prescriptions`) — both are checked, scoped by *both* `document_id` and
/// `appointment_id`. Real authorization happens twice, redundantly on
/// purpose: RLS on both tables already restricts visible rows to the owning
/// patient, the assigned doctor, or an admin role (a non-participant's query
/// returns no row at all, surfacing as 404 — the same "don't disclose
/// resource existence" property already established elsewhere in this
/// codebase), and the query is additionally scoped by `appointment_id` in
/// the handler itself as a second, independent check.
#[derive(Debug, Deserialize, Default)]
pub struct DocumentContextQuery {
    pub current_appointment_id: Option<Uuid>,
}

#[derive(Serialize, Deserialize)]
struct DocumentGrant {
    user_id: UserId,
    role: UserRole,
    current_appointment_id: Uuid,
    appointment_id: Uuid,
    document_id: Uuid,
}

#[derive(Deserialize)]
pub struct DownloadQuery {
    pub token: String,
}

#[derive(Deserialize)]
struct DocumentLocation {
    bucket: String,
    key: String,
    hash: Option<String>,
    mime: Option<String>,
    security_status: String,
}

async fn authorized_document(
    state: &AppState,
    ctx: &AuthContext,
    current_appointment_id: Uuid,
    appointment_id: Uuid,
    document_id: Uuid,
) -> Result<DocumentLocation, AppError> {
    if !matches!(
        ctx.role,
        UserRole::Doctor
            | UserRole::Patient
            | UserRole::ClinicalAdmin
            | UserRole::Compliance
            | UserRole::PlatformAdmin
    ) {
        return Err(AppError::Forbidden(
            "This role cannot access clinical documents.".into(),
        ));
    }
    let mut tx = state.begin_authed(ctx).await?;
    let found = if ctx.role == UserRole::Doctor {
        let location: Option<serde_json::Value> =
            sqlx::query_scalar("SELECT doctor_authorized_document($1,$2,$3,$4)")
                .bind(current_appointment_id)
                .bind(appointment_id)
                .bind(document_id)
                .bind(ctx.user_id)
                .fetch_one(&mut *tx)
                .await?;
        location
            .map(serde_json::from_value::<DocumentLocation>)
            .transpose()
            .map_err(|_| AppError::internal("Document metadata is invalid.".into()))?
    } else if let Some(doc) =
        prescription_repo::find_intake_document(&mut *tx, document_id, appointment_id).await?
    {
        Some(DocumentLocation {
            bucket: doc.object_bucket,
            key: doc.object_key,
            hash: doc.content_hash,
            mime: Some(doc.mime_type),
            security_status: doc.security_status,
        })
    } else {
        prescription_repo::find_prescription_by_id(&mut *tx, document_id, appointment_id)
            .await?
            .and_then(|rx| {
                Some(DocumentLocation {
                    bucket: rx.rx_image_object_bucket?,
                    key: rx.rx_image_object_key?,
                    hash: rx.integrity_verification_hash,
                    mime: rx.mime_type,
                    security_status: rx.security_status,
                })
            })
    };
    tx.commit().await?;
    let location = found
        .ok_or_else(|| AppError::NotFound("Document not found or authorization expired.".into()))?;
    if location.security_status != "CLEAN" {
        return Err(AppError::Conflict(
            "Document has not cleared security validation.".into(),
        ));
    }
    if location.bucket != state.config.storage_bucket || location.hash.is_none() {
        return Err(AppError::Conflict(
            "The document file is unavailable; it must be uploaded again.".into(),
        ));
    }
    Ok(location)
}

pub async fn get_document_download_url(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path((appointment_id, document_id)): Path<(Uuid, Uuid)>,
    Query(query): Query<DocumentContextQuery>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::AnyAuthenticated)?;
    let current_appointment_id = query.current_appointment_id.unwrap_or(appointment_id);
    authorized_document(
        &state,
        &ctx,
        current_appointment_id,
        appointment_id,
        document_id,
    )
    .await?;
    let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let grant = DocumentGrant {
        user_id: ctx.user_id,
        role: ctx.role,
        current_appointment_id,
        appointment_id,
        document_id,
    };
    let key = format!(
        "document-grant:{}",
        hex::encode(Sha256::digest(token.as_bytes()))
    );
    let mut conn =
        state.redis.get().await.map_err(|_| {
            AppError::internal("Document access is temporarily unavailable.".into())
        })?;
    let _: () = deadpool_redis::redis::cmd("SET")
        .arg(key)
        .arg(
            serde_json::to_string(&grant)
                .map_err(|_| AppError::internal("Document grant failed.".into()))?,
        )
        .arg("EX")
        .arg(300)
        .query_async(&mut conn)
        .await
        .map_err(|_| AppError::internal("Document access is temporarily unavailable.".into()))?;
    let pre_signed_url = format!("/api/v1/documents/download?token={token}");

    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "document_id": document_id,
            "download_url": pre_signed_url,
            "expires_in": 300
        }),
    }))
}

/// This public route is authenticated by a short-lived server-issued grant,
/// never by client-supplied ownership. RLS/time-window checks run again here.
pub async fn download_document(
    State(state): State<AppState>,
    Query(query): Query<DownloadQuery>,
) -> Result<impl IntoResponse, AppError> {
    if query.token.len() != 64 || !query.token.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(AppError::NotFound(
            "Document access expired or invalid.".into(),
        ));
    }
    let key = format!(
        "document-grant:{}",
        hex::encode(Sha256::digest(query.token.as_bytes()))
    );
    let mut conn = state
        .redis
        .get()
        .await
        .map_err(|_| AppError::internal("Document access unavailable.".into()))?;
    let raw: Option<String> = deadpool_redis::redis::cmd("GET")
        .arg(key)
        .query_async(&mut conn)
        .await
        .map_err(|_| AppError::internal("Document access unavailable.".into()))?;
    let grant: DocumentGrant = serde_json::from_str(
        &raw.ok_or_else(|| AppError::NotFound("Document access expired or invalid.".into()))?,
    )
    .map_err(|_| AppError::internal("Document grant is invalid.".into()))?;
    let ctx = AuthContext {
        user_id: grant.user_id,
        role: grant.role,
        permissions: vec![],
        session_id: None,
        request_id: None,
    };
    let location = authorized_document(
        &state,
        &ctx,
        grant.current_appointment_id,
        grant.appointment_id,
        grant.document_id,
    )
    .await?;
    let bytes =
        DocumentStorageService::get(&state, &location.key, location.hash.as_deref().unwrap())
            .await?;
    let mime = location
        .mime
        .unwrap_or_else(|| "application/octet-stream".into());
    Ok((
        [
            (axum::http::header::CONTENT_TYPE, mime),
            (
                axum::http::header::CACHE_CONTROL,
                "private, no-store".into(),
            ),
            (axum::http::header::X_CONTENT_TYPE_OPTIONS, "nosniff".into()),
            (axum::http::header::CONTENT_DISPOSITION, "attachment".into()),
            (
                axum::http::header::CONTENT_SECURITY_POLICY,
                "sandbox; default-src 'none'".into(),
            ),
        ],
        bytes,
    ))
}
