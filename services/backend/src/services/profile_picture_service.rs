//! Patient and Doctor profile pictures.
//!
//! Deliberately mirrors `PrescriptionService`'s two-phase shape: sanitization
//! (CPU-bound, semaphore-throttled, via the shared `UploadSecurityService`)
//! runs with no database transaction open, so its worst-case latency can
//! never hold a connection from the shared Api-role pool
//! (h03-upload-tx-lock-held-across-semaphore-throttled-sanitize, the same
//! incident `PrescriptionService`'s own doc comments describe). The short
//! authoritative transaction only re-validates ownership, persists the
//! already-sanitized bytes, and commits.
//!
//! Unlike prescription documents (which only ever append new pages), a
//! profile picture is replaced in place: uploading a new one, or removing
//! the existing one, must clean up the OLD object — but only after the new
//! database state has successfully committed, never before (a failed
//! replace must leave the previous photo intact and servable).

use crate::auth_context::AuthContext;
use crate::domain::ids::{DoctorProfileId, PatientProfileId};
use crate::error::AppError;
use crate::repository::{doctor_repo, patient_repo, AppState};
use crate::services::document_storage_service::DocumentStorageService;
use crate::services::upload_security_service::UploadSecurityService;
use chrono::Utc;
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub struct ProfilePictureService;

fn validate_profile_photo_mime(mime_type: &str) -> Result<(), AppError> {
    if matches!(mime_type, "image/jpeg" | "image/png") {
        Ok(())
    } else {
        Err(AppError::UnprocessableEntity(
            "Profile pictures must be JPEG or PNG images.".into(),
        ))
    }
}

pub struct StoredProfilePicture {
    pub bytes: Vec<u8>,
    pub mime_type: String,
}

/// Same advisory-lock shape as `appointment_repo::lock_for_document_upload`
/// (an opaque per-resource mutex via `pg_advisory_xact_lock`, released
/// automatically at transaction end) — kept local rather than reused from
/// `appointment_repo` since a profile picture is not an appointment-scoped
/// resource; the two share the same lock keyspace, which is fine; a
/// collision between a random appointment id and a random profile id
/// derived into the same 64-bit key is astronomically unlikely, and a
/// spurious shared wait is harmless (it only ever serializes, never
/// corrupts).
pub(crate) async fn lock_profile_picture<'e, E: sqlx::PgExecutor<'e>>(
    executor: E,
    id: Uuid,
) -> Result<(), AppError> {
    let key = i64::from_be_bytes(id.as_bytes()[0..8].try_into().unwrap());
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(key)
        .execute(executor)
        .await?;
    Ok(())
}

impl ProfilePictureService {
    pub async fn upload_patient_photo(
        state: &AppState,
        ctx: &AuthContext,
        patient_id: PatientProfileId,
        mime_type: String,
        bytes: Vec<u8>,
    ) -> Result<(), AppError> {
        validate_profile_photo_mime(&mime_type)?;
        // PHASE A — no transaction held across this CPU-bound work.
        let sanitized = UploadSecurityService::sanitize_async(bytes, mime_type.clone()).await?;
        let hash = format!("{:x}", Sha256::digest(&sanitized));
        let object_key = format!("avatars/patient/{}/{}.bin", patient_id, Uuid::now_v7());

        // PHASE B — short authoritative transaction.
        let mut tx = state.begin_authed(ctx).await?;
        lock_profile_picture(&mut *tx, patient_id.into_inner()).await?;
        reject_erasure_photo_write(&mut tx, "PATIENT", patient_id.into_inner()).await?;
        let existing = patient_repo::find_profile_picture(&mut *tx, patient_id).await?;

        DocumentStorageService::put(state, &object_key, &sanitized).await?;
        let rows = match patient_repo::update_profile_picture(
            &mut *tx,
            patient_id,
            Some(&object_key),
            Some(&state.config.storage_bucket),
            Some(&hash),
            Some(&mime_type),
            Some(sanitized.len() as i32),
            Utc::now(),
        )
        .await
        {
            Ok(rows) => rows,
            Err(error) => {
                DocumentStorageService::delete(state, &object_key).await;
                return Err(error.into());
            }
        };
        if rows != 1 {
            DocumentStorageService::delete(state, &object_key).await;
            return Err(AppError::NotFound("Patient profile not found.".into()));
        }
        if let Err(error) = tx.commit().await {
            DocumentStorageService::delete(state, &object_key).await;
            return Err(error.into());
        }
        // Only after a successful commit does the new object key become the
        // row of record — only now is it safe to delete the previous one.
        if let Some(old_key) = existing.and_then(|m| m.object_key) {
            if old_key != object_key {
                DocumentStorageService::delete(state, &old_key).await;
            }
        }
        Ok(())
    }

    pub async fn remove_patient_photo(
        state: &AppState,
        ctx: &AuthContext,
        patient_id: PatientProfileId,
    ) -> Result<(), AppError> {
        let mut tx = state.begin_authed(ctx).await?;
        lock_profile_picture(&mut *tx, patient_id.into_inner()).await?;
        let existing = patient_repo::find_profile_picture(&mut *tx, patient_id).await?;
        let rows = patient_repo::update_profile_picture(
            &mut *tx,
            patient_id,
            None,
            None,
            None,
            None,
            None,
            Utc::now(),
        )
        .await?;
        if rows != 1 {
            return Err(AppError::NotFound("Patient profile not found.".into()));
        }
        tx.commit().await?;
        if let Some(old_key) = existing.and_then(|m| m.object_key) {
            DocumentStorageService::delete(state, &old_key).await;
        }
        Ok(())
    }

    pub async fn get_patient_photo(
        state: &AppState,
        ctx: &AuthContext,
        patient_id: PatientProfileId,
    ) -> Result<StoredProfilePicture, AppError> {
        let mut tx = state.begin_authed(ctx).await?;
        let meta = patient_repo::find_profile_picture(&mut *tx, patient_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Patient profile not found.".into()))?;
        tx.commit().await?;
        let (key, hash, mime) = match (meta.object_key, meta.content_hash, meta.mime_type) {
            (Some(k), Some(h), Some(m)) => (k, h, m),
            _ => return Err(AppError::NotFound("No profile picture is set.".into())),
        };
        let bytes = DocumentStorageService::get(state, &key, &hash).await?;
        Ok(StoredProfilePicture {
            bytes,
            mime_type: mime,
        })
    }

    pub async fn upload_doctor_photo(
        state: &AppState,
        ctx: &AuthContext,
        doctor_id: DoctorProfileId,
        mime_type: String,
        bytes: Vec<u8>,
    ) -> Result<(), AppError> {
        validate_profile_photo_mime(&mime_type)?;
        // PHASE A
        let sanitized = UploadSecurityService::sanitize_async(bytes, mime_type.clone()).await?;
        let hash = format!("{:x}", Sha256::digest(&sanitized));
        let object_key = format!("avatars/doctor/{}/{}.bin", doctor_id, Uuid::now_v7());

        // PHASE B
        let mut tx = state.begin_authed(ctx).await?;
        lock_profile_picture(&mut *tx, doctor_id.into_inner()).await?;
        reject_erasure_photo_write(&mut tx, "DOCTOR", doctor_id.into_inner()).await?;
        let existing = doctor_repo::find_profile_picture(&mut *tx, doctor_id).await?;

        DocumentStorageService::put(state, &object_key, &sanitized).await?;
        let rows = match doctor_repo::update_profile_picture(
            &mut *tx,
            doctor_id,
            Some(&object_key),
            Some(&state.config.storage_bucket),
            Some(&hash),
            Some(&mime_type),
            Some(sanitized.len() as i32),
            Utc::now(),
        )
        .await
        {
            Ok(rows) => rows,
            Err(error) => {
                DocumentStorageService::delete(state, &object_key).await;
                return Err(error.into());
            }
        };
        if rows != 1 {
            DocumentStorageService::delete(state, &object_key).await;
            return Err(AppError::NotFound("Doctor profile not found.".into()));
        }
        if let Err(error) = tx.commit().await {
            DocumentStorageService::delete(state, &object_key).await;
            return Err(error.into());
        }
        if let Some(old_key) = existing.and_then(|m| m.object_key) {
            if old_key != object_key {
                DocumentStorageService::delete(state, &old_key).await;
            }
        }
        Ok(())
    }

    pub async fn remove_doctor_photo(
        state: &AppState,
        ctx: &AuthContext,
        doctor_id: DoctorProfileId,
    ) -> Result<(), AppError> {
        let mut tx = state.begin_authed(ctx).await?;
        lock_profile_picture(&mut *tx, doctor_id.into_inner()).await?;
        let existing = doctor_repo::find_profile_picture(&mut *tx, doctor_id).await?;
        let rows = doctor_repo::update_profile_picture(
            &mut *tx,
            doctor_id,
            None,
            None,
            None,
            None,
            None,
            Utc::now(),
        )
        .await?;
        if rows != 1 {
            return Err(AppError::NotFound("Doctor profile not found.".into()));
        }
        tx.commit().await?;
        if let Some(old_key) = existing.and_then(|m| m.object_key) {
            DocumentStorageService::delete(state, &old_key).await;
        }
        Ok(())
    }

    /// Serves a doctor's own photo regardless of directory visibility (so a
    /// not-yet-verified doctor can still preview what they uploaded) — an
    /// ordinary RLS-scoped self-row read.
    pub async fn get_own_doctor_photo(
        state: &AppState,
        ctx: &AuthContext,
        doctor_id: DoctorProfileId,
    ) -> Result<StoredProfilePicture, AppError> {
        let mut tx = state.begin_authed(ctx).await?;
        let meta = doctor_repo::find_profile_picture(&mut *tx, doctor_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Doctor profile not found.".into()))?;
        tx.commit().await?;
        Self::photo_from_meta(state, meta.object_key, meta.content_hash, meta.mime_type).await
    }

    /// Serves ANY authenticated caller's view of a doctor's directory-visible
    /// photo — deliberately bypasses the normal self-row RLS scope via the
    /// `doctor_public_photo` SECURITY DEFINER function (see the migration's
    /// own doc comment for why this is safe: the function evaluates the
    /// exact same visibility condition `doctor_public_directory` already
    /// uses, it is not a blanket RLS bypass).
    pub async fn get_directory_doctor_photo(
        state: &AppState,
        ctx: &AuthContext,
        doctor_id: DoctorProfileId,
    ) -> Result<StoredProfilePicture, AppError> {
        #[derive(serde::Deserialize)]
        struct Row {
            bucket: Option<String>,
            key: Option<String>,
            hash: Option<String>,
            mime: Option<String>,
        }
        let mut tx = state.begin_authed(ctx).await?;
        let raw: Option<serde_json::Value> = sqlx::query_scalar("SELECT doctor_public_photo($1)")
            .bind(doctor_id)
            .fetch_one(&mut *tx)
            .await?;
        tx.commit().await?;
        let row: Row = raw
            .map(serde_json::from_value)
            .transpose()
            .map_err(|_| AppError::internal("Profile picture metadata is invalid.".into()))?
            .ok_or_else(|| AppError::NotFound("No profile picture is set.".into()))?;
        if row.bucket.as_deref() != Some(state.config.storage_bucket.as_str()) {
            return Err(AppError::Conflict(
                "The profile picture is unavailable; it must be uploaded again.".into(),
            ));
        }
        Self::photo_from_meta(state, row.key, row.hash, row.mime).await
    }

    async fn photo_from_meta(
        state: &AppState,
        object_key: Option<String>,
        content_hash: Option<String>,
        mime_type: Option<String>,
    ) -> Result<StoredProfilePicture, AppError> {
        let (key, hash, mime) = match (object_key, content_hash, mime_type) {
            (Some(k), Some(h), Some(m)) => (k, h, m),
            _ => return Err(AppError::NotFound("No profile picture is set.".into())),
        };
        let bytes = DocumentStorageService::get(state, &key, &hash).await?;
        Ok(StoredProfilePicture {
            bytes,
            mime_type: mime,
        })
    }
}

async fn reject_erasure_photo_write(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    kind: &str,
    id: uuid::Uuid,
) -> Result<(), AppError> {
    let frozen: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM account_erasure_jobs WHERE profile_kind=$1 AND profile_id=$2)",
    )
    .bind(kind)
    .bind(id)
    .fetch_one(&mut **tx)
    .await?;
    if frozen {
        return Err(AppError::Conflict(
            "Profile erasure has been requested.".into(),
        ));
    }
    Ok(())
}
