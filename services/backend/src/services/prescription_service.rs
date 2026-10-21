use super::consultation_trace::{self, Event};
use crate::auth_context::AuthContext;
use crate::domain::ids::{DoctorProfileId, PatientProfileId};
use crate::domain::models::*;
use crate::error::AppError;
use crate::repository::{appointment_repo, prescription_repo, AppState};
use crate::services::document_storage_service::DocumentStorageService;
use crate::services::upload_security_service::UploadSecurityService;
use chrono::Utc;
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Patient pre-consultation intake image cap, scoped to a single appointment
/// (consultation) — never lifetime/account-wide and never summed across
/// consultations, since the backing count query itself is per-appointment.
/// Each image independently fits `MAX_PRESCRIPTION_IMAGE_BYTES` (40 KB), so
/// this cap also bounds aggregate per-consultation storage at
/// 20 * 40 KB = 800 KB. See
/// docs/PRESCRIPTION_UPLOAD_40KB_20_IMAGE_VERIFICATION.md.
const PATIENT_MAX_IMAGES_PER_CONSULTATION: usize = 20;

pub struct PrescriptionService;

impl PrescriptionService {
    /// Patient multi-prescription upload intake (max 20 photos per appointment)
    ///
    /// Deliberately two phases, with NO database transaction held during the
    /// first: holding a transaction + advisory lock on the shared, small
    /// Api-role pool across up to 20 sequential, CPU-bound, adversarially-
    /// compressible image sanitizations previously let a single account
    /// plausibly exhaust that pool and deny DB access to all ordinary API
    /// traffic (h03-upload-tx-lock-held-across-semaphore-throttled-sanitize).
    /// Sanitization now runs with no transaction open at all; the
    /// authoritative transaction only re-validates and persists already-
    /// sanitized bytes, so its hold time is bounded by fast DB/storage work,
    /// not by worst-case compression latency.
    pub async fn upload_patient_intake_documents(
        state: &AppState,
        ctx: &AuthContext,
        appointment_id: Uuid,
        patient_id: PatientProfileId,
        files: Vec<(String, Vec<u8>)>, // (mime_type, file_bytes)
    ) -> Result<Vec<PrescriptionIntakeDocument>, AppError> {
        if files.is_empty() {
            return Err(AppError::UnprocessableEntity(
                "Select files for an active appointment.".into(),
            ));
        }

        // PHASE A — no transaction held across this block's CPU-bound work.
        // A short read-only check establishes ownership/state up front (so a
        // request for someone else's or a cancelled appointment fails fast,
        // before spending any sanitize-semaphore capacity on it); the
        // authoritative check happens again under lock in Phase B, since
        // state can change between the two.
        {
            let mut lookup_tx = state.begin_authed(ctx).await?;
            let appointment = appointment_repo::find_by_id(&mut *lookup_tx, appointment_id)
                .await?
                .filter(|appointment| appointment.patient_id == patient_id)
                .ok_or_else(|| AppError::NotFound("Appointment not found.".into()))?;
            lookup_tx.commit().await?;
            if appointment.status == AppointmentStatus::Cancelled {
                return Err(AppError::UnprocessableEntity(
                    "Select files for an active appointment.".into(),
                ));
            }
        }

        let mut sanitized_files = Vec::new();
        // Each file is compressed toward its OWN independent 40 KB ceiling —
        // never a shared budget divided across however many files are in
        // this request. See UploadSecurityService's own doc comments for the
        // full reasoning. No DB transaction is open during this loop.
        for (mime, bytes) in files {
            let sanitized = UploadSecurityService::sanitize_async(bytes, mime.clone()).await?;
            sanitized_files.push((mime, sanitized));
        }

        // PHASE B — short authoritative transaction: lock, re-validate
        // ownership/state/quota under the lock, persist already-sanitized
        // bytes, commit. Sanitization never happens inside this block.
        let mut tx = state.begin_authed(ctx).await?;

        let appointment = appointment_repo::find_by_id(&mut *tx, appointment_id)
            .await?
            .filter(|appointment| appointment.patient_id == patient_id)
            .ok_or_else(|| AppError::NotFound("Appointment not found.".into()))?;
        if appointment.status == AppointmentStatus::Cancelled {
            return Err(AppError::UnprocessableEntity(
                "Select files for an active appointment.".into(),
            ));
        }

        // Serializes concurrent upload calls for the SAME appointment on the
        // count-check-insert sequence below — without this, two concurrent
        // calls could each read a count under the cap and together exceed
        // it (a real race; see docs/CHECKPOINT.md §10). Taken AFTER
        // sanitization, not before, so the lock's hold time is bounded by DB
        // work only.
        appointment_repo::lock_for_document_upload(&mut *tx, appointment_id).await?;

        let existing_count =
            prescription_repo::count_intake_documents(&mut *tx, appointment_id).await?;

        if existing_count as usize + sanitized_files.len() > PATIENT_MAX_IMAGES_PER_CONSULTATION {
            return Err(AppError::UnprocessableEntity(format!(
                "Maximum {PATIENT_MAX_IMAGES_PER_CONSULTATION} prescription photos allowed per appointment."
            )));
        }

        let mut uploaded: Vec<PrescriptionIntakeDocument> = Vec::new();
        let mut next_page = existing_count as i16;
        for (mime, sanitized) in sanitized_files {
            next_page += 1;

            let hash = format!("{:x}", Sha256::digest(&sanitized));
            let object_key = format!(
                "intake/{}/{}_{}.bin",
                appointment_id,
                Uuid::now_v7(),
                next_page
            );

            let doc = PrescriptionIntakeDocument {
                id: Uuid::new_v4(),
                appointment_id,
                patient_id,
                page_number: next_page,
                object_bucket: state.config.storage_bucket.clone(),
                object_key,
                content_hash: Some(hash),
                file_size_bytes: sanitized.len() as i64,
                mime_type: mime,
                // Always "CLEAN" here: a file that failed validation above
                // already returned an error and never reaches this insert —
                // see UploadSecurityService's own doc comment for why the
                // column still exists (a future async re-scan path).
                security_status: "CLEAN".into(),
                created_at: Utc::now(),
            };

            if let Err(error) =
                DocumentStorageService::put(state, &doc.object_key, &sanitized).await
            {
                for previous in &uploaded {
                    DocumentStorageService::delete(state, &previous.object_key).await;
                }
                return Err(error);
            }
            if let Err(error) = prescription_repo::insert_intake_document(&mut *tx, &doc).await {
                DocumentStorageService::delete(state, &doc.object_key).await;
                for previous in &uploaded {
                    DocumentStorageService::delete(state, &previous.object_key).await;
                }
                return Err(error.into());
            }
            consultation_trace::record(
                &mut tx,
                appointment_id,
                None,
                Event::accepted("ATTACHMENT_UPLOAD_COMPLETED", Some(("DOCUMENT", doc.id))),
            )
            .await?;
            uploaded.push(doc);
        }

        if let Err(error) = tx.commit().await {
            for document in &uploaded {
                DocumentStorageService::delete(state, &document.object_key).await;
            }
            return Err(error.into());
        }
        Ok(uploaded)
    }

    /// Doctor handwritten prescription photo upload
    ///
    /// Same two-phase shape as `upload_patient_intake_documents` and for the
    /// same reason: sanitization (CPU-bound, semaphore-throttled) must not
    /// happen while a transaction/lock on the shared Api-role pool is open
    /// (h03-upload-tx-lock-held-across-semaphore-throttled-sanitize).
    pub async fn upload_doctor_prescription_photo(
        state: &AppState,
        ctx: &AuthContext,
        appointment_id: Uuid,
        doctor_id: DoctorProfileId,
        photo_bytes: Vec<u8>,
        mime_type: Option<String>,
        doctor_notes: Option<String>,
    ) -> Result<Prescription, AppError> {
        // PHASE A — no transaction held. Mime detection and sanitization are
        // pure CPU work with no DB dependency.
        let mime = mime_type.unwrap_or_else(|| {
            if photo_bytes.starts_with(b"%PDF") {
                "application/pdf".into()
            } else if photo_bytes.starts_with(&[0x89, 0x50, 0x4e, 0x47]) {
                "image/png".into()
            } else {
                "image/jpeg".into()
            }
        });
        // Same fixed 40 KB per-image ceiling as the patient side — not a
        // divided batch budget. The doctor's own 5-page-per-appointment cap
        // below is unrelated and unchanged.
        let sanitized = UploadSecurityService::sanitize_async(photo_bytes, mime.clone()).await?;
        let hash = format!("{:x}", Sha256::digest(&sanitized));

        // PHASE B — short authoritative transaction: ownership, lock,
        // consultation-window authorization, dedup/count checks, persist.
        let mut tx = state.begin_authed(ctx).await?;

        let apt = appointment_repo::find_by_id(&mut *tx, appointment_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Appointment not found.".into()))?;

        if apt.doctor_id != doctor_id {
            return Err(AppError::Forbidden(
                "Caller is not the assigned physician for this appointment.".into(),
            ));
        }
        appointment_repo::lock_for_document_upload(&mut *tx, appointment_id).await?;
        let allowed: bool = sqlx::query_scalar("SELECT doctor_history_context_allowed($1,$2)")
            .bind(appointment_id)
            .bind(ctx.user_id)
            .fetch_one(&mut *tx)
            .await?;
        if !allowed {
            return Err(AppError::Forbidden(
                "Prescription access requires an active consultation authorization window.".into(),
            ));
        }
        if let Some(existing) = sqlx::query_as::<_, Prescription>(
            "SELECT * FROM prescriptions WHERE appointment_id=$1 AND integrity_verification_hash=$2",
        ).bind(appointment_id).bind(&hash).fetch_optional(&mut *tx).await? {
            tx.commit().await?;
            return Ok(existing);
        }
        let (count, no_rx): (i64, bool) = sqlx::query_as("SELECT COUNT(*),COALESCE(BOOL_OR(is_no_rx_required),FALSE) FROM prescriptions WHERE appointment_id=$1")
            .bind(appointment_id).fetch_one(&mut *tx).await?;
        if no_rx {
            return Err(AppError::Conflict(
                "This consultation already has a no-prescription record.".into(),
            ));
        }
        if count >= 5 {
            return Err(AppError::UnprocessableEntity(
                "Maximum 5 prescription pages allowed per appointment.".into(),
            ));
        }
        let rx_id = Uuid::new_v4();
        let rx_number = format!(
            "RX-{}-{}",
            Utc::now().format("%Y%m%d"),
            rx_id.to_string()[0..4].to_uppercase()
        );
        let extension = match mime.as_str() {
            "application/pdf" => "pdf",
            "image/png" => "png",
            _ => "jpg",
        };
        let object_key = format!("rx/{}/{}.{}", appointment_id, Uuid::now_v7(), extension);

        let prescription = Prescription {
            id: rx_id,
            appointment_id,
            doctor_id,
            patient_id: apt.patient_id,
            rx_number,
            integrity_verification_hash: Some(hash),
            rx_image_object_bucket: Some(state.config.storage_bucket.clone()),
            rx_image_object_key: Some(object_key),
            doctor_notes,
            is_no_rx_required: false,
            no_rx_reason: None,
            security_status: "CLEAN".into(),
            created_at: Utc::now(),
            page_number: count as i16 + 1,
            mime_type: Some(mime),
        };
        let key = prescription.rx_image_object_key.as_deref().unwrap();
        DocumentStorageService::put(state, key, &sanitized).await?;
        if let Err(error) = prescription_repo::insert_prescription(&mut *tx, &prescription).await {
            DocumentStorageService::delete(state, key).await;
            return Err(error.into());
        }
        consultation_trace::record(
            &mut tx,
            appointment_id,
            None,
            Event::accepted("RX_CREATED", Some(("PRESCRIPTION", rx_id))),
        )
        .await?;
        if let Err(error) = tx.commit().await {
            DocumentStorageService::delete(state, key).await;
            return Err(error.into());
        }
        Ok(prescription)
    }

    /// Doctor explicitly signs that no prescription is required (follow-ups/lifestyle advice)
    pub async fn complete_without_prescription(
        state: &AppState,
        ctx: &AuthContext,
        appointment_id: Uuid,
        doctor_id: DoctorProfileId,
        reason: String,
    ) -> Result<Prescription, AppError> {
        let mut tx = state.begin_authed(ctx).await?;
        appointment_repo::lock_for_document_upload(&mut *tx, appointment_id).await?;

        let apt = appointment_repo::find_by_id(&mut *tx, appointment_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Appointment not found.".into()))?;

        if apt.doctor_id != doctor_id {
            return Err(AppError::Forbidden(
                "Caller is not the assigned physician for this appointment.".into(),
            ));
        }

        // Use the existing clinical-document authorization window. Outside it
        // RLS intentionally hides existing decisions; do not treat an invisible
        // prescription as absent and attempt a duplicate page insert.
        let context_allowed: bool =
            sqlx::query_scalar("SELECT doctor_history_context_allowed($1,$2)")
                .bind(appointment_id)
                .bind(ctx.user_id)
                .fetch_one(&mut *tx)
                .await?;
        if !context_allowed {
            return Err(AppError::Forbidden(
                "Clinical document access is not available in this consultation window.".into(),
            ));
        }

        if let Some(existing) =
            prescription_repo::find_prescription_by_appointment(&mut *tx, appointment_id).await?
        {
            if existing.is_no_rx_required
                && existing.no_rx_reason.as_deref() == Some(reason.as_str())
            {
                super::ConsultationService::complete_in_transaction(
                    &mut tx,
                    ctx,
                    appointment_id,
                    ClinicalOutcome::CompletedNoRx,
                )
                .await?;
                tx.commit().await?;
                return Ok(existing);
            }
            return Err(AppError::Conflict(
                "A prescription decision already exists for this consultation.".into(),
            ));
        }

        let rx_id = Uuid::new_v4();
        let rx_number = format!(
            "NO-RX-{}-{}",
            Utc::now().format("%Y%m%d"),
            rx_id.to_string()[0..4].to_uppercase()
        );

        let prescription = Prescription {
            id: rx_id,
            appointment_id,
            doctor_id,
            patient_id: apt.patient_id,
            rx_number,
            integrity_verification_hash: None,
            rx_image_object_bucket: None,
            rx_image_object_key: None,
            doctor_notes: None,
            is_no_rx_required: true,
            no_rx_reason: Some(reason),
            security_status: "CLEAN".into(),
            created_at: Utc::now(),
            page_number: 1,
            mime_type: None,
        };

        prescription_repo::insert_prescription(&mut *tx, &prescription).await?;
        super::ConsultationService::complete_in_transaction(
            &mut tx,
            ctx,
            appointment_id,
            ClinicalOutcome::CompletedNoRx,
        )
        .await?;
        tx.commit().await?;
        Ok(prescription)
    }
}
