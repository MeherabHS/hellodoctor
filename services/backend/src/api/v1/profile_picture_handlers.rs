//! Patient and Doctor profile picture upload/replace/remove/retrieve.
//!
//! Mirrors `prescription_handlers.rs`'s conventions throughout: base64-JSON
//! request bodies (not multipart), the same upload rate-limit + concurrency
//! guard pair, and the same private-document response headers for the
//! Patient (private) retrieval path. See
//! `services/profile_picture_service.rs` for the actual upload/storage logic
//! and its doc comments for why Patient and Doctor directory visibility are
//! deliberately handled differently.

use crate::abac::{self, Action};
use crate::auth_context::AuthContext;
use crate::domain::ids::{DoctorProfileId, PatientProfileId};
use crate::error::AppError;
use crate::rate_limit;
use crate::repository::AppState;
use crate::services::profile_picture_service::{ProfilePictureService, StoredProfilePicture};
use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: T,
}

#[derive(Debug, Deserialize)]
pub struct UploadPhotoRequest {
    pub mime_type: String,
    pub base64_content: String,
    /// Only meaningful for the Patient endpoints — uploads to the caller's
    /// own self profile when omitted, or an explicitly-guardianed dependent
    /// profile otherwise. Ignored by the Doctor endpoints (a doctor has no
    /// dependents; `ctx.doctor_profile_id` always resolves the caller's own
    /// single profile).
    #[serde(default)]
    pub profile_id: Option<PatientProfileId>,
}

#[derive(Debug, Deserialize, Default)]
pub struct PhotoProfileQuery {
    #[serde(default)]
    pub profile_id: Option<PatientProfileId>,
}

fn decode_base64_payload(raw: &str) -> Result<Vec<u8>, AppError> {
    let stripped = raw.split(',').next_back().unwrap_or(raw);
    STANDARD
        .decode(stripped)
        .map_err(|_| AppError::UnprocessableEntity("File content is not valid base64.".into()))
}

fn photo_response(photo: StoredProfilePicture, private: bool) -> impl IntoResponse {
    let cache_control = if private {
        "private, no-store"
    } else {
        // Directory-visible doctor avatars are meant to be fetched
        // repeatedly by many patients — unlike the Patient-private path,
        // caching is deliberate here, not an oversight.
        "public, max-age=3600"
    };
    (
        [
            (axum::http::header::CONTENT_TYPE, photo.mime_type),
            (axum::http::header::CACHE_CONTROL, cache_control.into()),
            (axum::http::header::X_CONTENT_TYPE_OPTIONS, "nosniff".into()),
            (
                axum::http::header::CONTENT_SECURITY_POLICY,
                "sandbox; default-src 'none'".into(),
            ),
        ],
        photo.bytes,
    )
}

// ---- Patient ----

pub async fn upload_patient_photo(
    State(state): State<AppState>,
    ctx: AuthContext,
    axum::Json(payload): axum::Json<UploadPhotoRequest>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::PatientManageProfile)?;
    rate_limit::check_upload_limit(&state.redis, &ctx.user_id.to_string()).await?;
    let _upload_concurrency_guard =
        rate_limit::acquire_upload_concurrency(&state.redis, &ctx.user_id.to_string(), 2).await?;
    let patient_id = ctx
        .acting_patient_profile_id(&state, payload.profile_id)
        .await?;
    let bytes = decode_base64_payload(&payload.base64_content)?;
    ProfilePictureService::upload_patient_photo(&state, &ctx, patient_id, payload.mime_type, bytes)
        .await?;
    Ok(axum::Json(ApiResponse {
        success: true,
        data: serde_json::json!({"updated": true}),
    }))
}

pub async fn delete_patient_photo(
    State(state): State<AppState>,
    ctx: AuthContext,
    Query(query): Query<PhotoProfileQuery>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::PatientManageProfile)?;
    let patient_id = ctx
        .acting_patient_profile_id(&state, query.profile_id)
        .await?;
    ProfilePictureService::remove_patient_photo(&state, &ctx, patient_id).await?;
    Ok(axum::Json(ApiResponse {
        success: true,
        data: serde_json::json!({"removed": true}),
    }))
}

pub async fn get_patient_photo(
    State(state): State<AppState>,
    ctx: AuthContext,
    Query(query): Query<PhotoProfileQuery>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::PatientRead)?;
    let patient_id = ctx
        .acting_patient_profile_id(&state, query.profile_id)
        .await?;
    let photo = ProfilePictureService::get_patient_photo(&state, &ctx, patient_id).await?;
    Ok(photo_response(photo, true))
}

// ---- Doctor (self) ----

pub async fn upload_doctor_photo(
    State(state): State<AppState>,
    ctx: AuthContext,
    axum::Json(payload): axum::Json<UploadPhotoRequest>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::DoctorManageProfile)?;
    rate_limit::check_upload_limit(&state.redis, &ctx.user_id.to_string()).await?;
    let _upload_concurrency_guard =
        rate_limit::acquire_upload_concurrency(&state.redis, &ctx.user_id.to_string(), 2).await?;
    let doctor_id = ctx.doctor_profile_id(&state).await?;
    let bytes = decode_base64_payload(&payload.base64_content)?;
    ProfilePictureService::upload_doctor_photo(&state, &ctx, doctor_id, payload.mime_type, bytes)
        .await?;
    Ok(axum::Json(ApiResponse {
        success: true,
        data: serde_json::json!({"updated": true}),
    }))
}

pub async fn delete_doctor_photo(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::DoctorManageProfile)?;
    let doctor_id = ctx.doctor_profile_id(&state).await?;
    ProfilePictureService::remove_doctor_photo(&state, &ctx, doctor_id).await?;
    Ok(axum::Json(ApiResponse {
        success: true,
        data: serde_json::json!({"removed": true}),
    }))
}

pub async fn get_own_doctor_photo(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::DoctorRead)?;
    let doctor_id = ctx.doctor_profile_id(&state).await?;
    let photo = ProfilePictureService::get_own_doctor_photo(&state, &ctx, doctor_id).await?;
    Ok(photo_response(photo, true))
}

// ---- Doctor (directory, any authenticated viewer) ----

pub async fn get_directory_doctor_photo(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(doctor_id): Path<DoctorProfileId>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::AnyAuthenticated)?;
    let photo = ProfilePictureService::get_directory_doctor_photo(&state, &ctx, doctor_id).await?;
    Ok(photo_response(photo, false))
}
