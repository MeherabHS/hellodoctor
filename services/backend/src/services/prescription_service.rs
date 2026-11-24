use super::consultation_trace::{self, Event};
use crate::auth_context::AuthContext;
use crate::domain::ids::{DoctorProfileId, PatientProfileId};
use crate::domain::models::*;
use crate::error::AppError;
use crate::repository::{appointment_repo, prescription_repo, AppState};
use crate::services::document_storage_service::DocumentStorageService;
use crate::services::upload_security_service::UploadSecurityService;
use chrono::{DateTime, Duration, Utc};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Patient pre-consultation intake image cap, scoped to a single appointment
/// (consultation) — never lifetime/account-wide and never summed across
/// consultations, since the backing count query itself is per-appointment.
/// Each image independently fits `MAX_PRESCRIPTION_IMAGE_BYTES` (600 KB), so
/// this cap also bounds aggregate per-consultation storage at
/// 20 * 600 KB = 12 MB. See
/// docs/PRESCRIPTION_UPLOAD_40KB_20_IMAGE_VERIFICATION.md.
pub const PATIENT_MAX_IMAGES_PER_CONSULTATION: usize = 20;

fn patient_intake_status_allowed(status: AppointmentStatus) -> bool {
    matches!(
        status,
        AppointmentStatus::PendingPayment
            | AppointmentStatus::Confirmed
            | AppointmentStatus::Waiting
            | AppointmentStatus::InConsultation
    )
}

pub struct PrescriptionService;

/// Product owner, 2026-11-24: a doctor sends a prescription only for a
/// consultation that has started, or one completed in the last 24 hours
/// without a prescription decision. Never for a future, unpaid or cancelled
/// one, which dispatching would otherwise mark "completed with prescription"
/// before the patient was seen.
///
/// "Started" means: a video call someone has joined (IN_CONSULTATION, set by
/// the signed Agora presence callback), or a chat whose consultation window
/// has opened (chats never reach IN_CONSULTATION; they stay CONFIRMED until
/// completed). The window opens `window_before_mins` before the slot start,
/// the same moment the apps let doctor and patient in.
pub(crate) fn doctor_rx_allowed(
    status: &AppointmentStatus,
    modality: &Modality,
    slot_start: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
    window_before_mins: i64,
    now: DateTime<Utc>,
) -> bool {
    match status {
        AppointmentStatus::InConsultation => true,
        AppointmentStatus::Confirmed | AppointmentStatus::Waiting => {
            matches!(modality, Modality::Chat)
                && now >= slot_start - Duration::minutes(window_before_mins)
        }
        AppointmentStatus::Completed => {
            completed_at.is_some_and(|ended| now < ended + Duration::hours(24))
        }
        _ => false,
    }
}

/// The refusal a doctor sees when [`doctor_rx_allowed`] says no.
pub(crate) const RX_NOT_STARTED: &str =
    "A prescription can be sent once the consultation has started, or within 24 hours after it ends.";

impl PrescriptionService {
    /// Refuses a doctor prescription (page upload or completion with Rx) for a
    /// consultation that has not started or whose 24-hour window has closed.
    /// Called under the per-consultation advisory lock with a fresh read.
    pub(crate) async fn ensure_doctor_rx_allowed(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        state: &AppState,
        apt: &Appointment,
    ) -> Result<(), AppError> {
        let slot_start: DateTime<Utc> =
            sqlx::query_scalar("SELECT start_time FROM doctor_schedule_slots WHERE id=$1")
                .bind(apt.slot_id)
                .fetch_one(&mut **tx)
                .await?;
        if doctor_rx_allowed(
            &apt.status,
            &apt.modality,
            slot_start,
            apt.completed_at,
            state.config.agora_token_window_before_mins,
            Utc::now(),
        ) {
            Ok(())
        } else {
            Err(AppError::Conflict(RX_NOT_STARTED.into()))
        }
    }
}

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
        if files.len() > PATIENT_MAX_IMAGES_PER_CONSULTATION {
            return Err(AppError::UnprocessableEntity(format!(
                "Maximum {PATIENT_MAX_IMAGES_PER_CONSULTATION} prescription photos allowed per appointment."
            )));
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
            if !patient_intake_status_allowed(appointment.status) {
                lookup_tx.commit().await?;
                return Err(AppError::UnprocessableEntity(
                    "Select files for an active appointment.".into(),
                ));
            }
            let upload_allowed =
                appointment_repo::intake_upload_allowed(&mut *lookup_tx, appointment_id).await?;
            lookup_tx.commit().await?;
            if !upload_allowed {
                return Err(AppError::UnprocessableEntity(
                    "Select files for an active appointment.".into(),
                ));
            }
        }

        let mut sanitized_files = Vec::new();
        // Each file is compressed toward its OWN independent ceiling —
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

        // Serialize upload and expiry before the authoritative appointment read.
        appointment_repo::lock_for_document_upload(&mut *tx, appointment_id).await?;

        let appointment = appointment_repo::find_by_id(&mut *tx, appointment_id)
            .await?
            .filter(|appointment| appointment.patient_id == patient_id)
            .ok_or_else(|| AppError::NotFound("Appointment not found.".into()))?;
        if !patient_intake_status_allowed(appointment.status) {
            return Err(AppError::UnprocessableEntity(
                "Select files for an active appointment.".into(),
            ));
        }
        if !appointment_repo::intake_upload_allowed(&mut *tx, appointment_id).await? {
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
        let existing_count =
            prescription_repo::count_intake_documents(&mut *tx, appointment_id).await?;

        if existing_count as usize + sanitized_files.len() > PATIENT_MAX_IMAGES_PER_CONSULTATION {
            return Err(AppError::UnprocessableEntity(format!(
                "Maximum {PATIENT_MAX_IMAGES_PER_CONSULTATION} prescription photos allowed per appointment."
            )));
        }

        // A page the patient removed leaves a gap; new photos fill the lowest
        // free page numbers so the 1..=20 page check never rejects an upload
        // the count check above allowed.
        let free_pages = prescription_repo::free_intake_page_numbers(
            &mut *tx,
            appointment_id,
            sanitized_files.len() as i64,
        )
        .await?;
        if free_pages.len() != sanitized_files.len() {
            return Err(AppError::UnprocessableEntity(format!(
                "Maximum {PATIENT_MAX_IMAGES_PER_CONSULTATION} prescription photos allowed per appointment."
            )));
        }

        let mut uploaded: Vec<PrescriptionIntakeDocument> = Vec::new();
        for ((mime, sanitized), next_page) in sanitized_files.into_iter().zip(free_pages) {
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
            if !appointment_repo::intake_upload_allowed(&mut *tx, appointment_id).await? {
                DocumentStorageService::delete(state, &doc.object_key).await;
                for previous in &uploaded {
                    DocumentStorageService::delete(state, &previous.object_key).await;
                }
                return Err(AppError::UnprocessableEntity(
                    "Select files for an active appointment.".into(),
                ));
            }
            if let Err(error) = prescription_repo::insert_intake_document(&mut *tx, &doc).await {
                DocumentStorageService::delete(state, &doc.object_key).await;
                for previous in &uploaded {
                    DocumentStorageService::delete(state, &previous.object_key).await;
                }
                return Err(error.into());
            }
            if let Err(error) = consultation_trace::record(
                &mut tx,
                appointment_id,
                None,
                Event::accepted("ATTACHMENT_UPLOAD_COMPLETED", Some(("DOCUMENT", doc.id))),
            )
            .await
            {
                DocumentStorageService::delete(state, &doc.object_key).await;
                for previous in &uploaded {
                    DocumentStorageService::delete(state, &previous.object_key).await;
                }
                return Err(error);
            }
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

    /// The patient's own intake documents for one appointment, with whether
    /// they can still be removed (the same window in which they can upload).
    pub async fn list_patient_intake_documents(
        state: &AppState,
        ctx: &AuthContext,
        appointment_id: Uuid,
        patient_id: PatientProfileId,
    ) -> Result<(Vec<PrescriptionIntakeDocument>, bool), AppError> {
        let mut tx = state.begin_authed(ctx).await?;
        let appointment = appointment_repo::find_by_id(&mut *tx, appointment_id)
            .await?
            .filter(|appointment| appointment.patient_id == patient_id)
            .ok_or_else(|| AppError::NotFound("Appointment not found.".into()))?;
        let editable = patient_intake_status_allowed(appointment.status)
            && appointment_repo::intake_upload_allowed(&mut *tx, appointment_id).await?;
        let documents = prescription_repo::list_intake_documents(&mut *tx, appointment_id).await?;
        tx.commit().await?;
        Ok((documents, editable))
    }

    /// Patient removes one of their own intake photos (for example the wrong
    /// photo) so they can upload the right one. Allowed only in the upload
    /// window: once the consultation is completed, cancelled or expired the
    /// upload is part of the clinical record and stays. The row is deleted
    /// and the stored object is queued for durable deletion in the same
    /// transaction; a best-effort immediate delete follows the commit.
    pub async fn remove_patient_intake_document(
        state: &AppState,
        ctx: &AuthContext,
        appointment_id: Uuid,
        patient_id: PatientProfileId,
        document_id: Uuid,
    ) -> Result<(), AppError> {
        let mut tx = state.begin_authed(ctx).await?;
        // Same lock as upload and expiry, so a removal never interleaves with
        // a concurrent upload's page assignment or the hold-expiry snapshot.
        appointment_repo::lock_for_document_upload(&mut *tx, appointment_id).await?;
        let appointment = appointment_repo::find_by_id(&mut *tx, appointment_id)
            .await?
            .filter(|appointment| appointment.patient_id == patient_id)
            .ok_or_else(|| AppError::NotFound("Appointment not found.".into()))?;
        if !patient_intake_status_allowed(appointment.status)
            || !appointment_repo::intake_upload_allowed(&mut *tx, appointment_id).await?
        {
            return Err(AppError::Conflict(
                "Uploads can no longer be changed for this consultation.".into(),
            ));
        }
        let doc = prescription_repo::find_intake_document(&mut *tx, document_id, appointment_id)
            .await?
            .filter(|doc| doc.patient_id == patient_id)
            .ok_or_else(|| AppError::NotFound("Document not found.".into()))?;
        if !prescription_repo::remove_intake_document(&mut tx, &doc).await? {
            return Err(AppError::NotFound("Document not found.".into()));
        }
        consultation_trace::record(
            &mut tx,
            appointment_id,
            None,
            Event::accepted("ATTACHMENT_REMOVED", Some(("DOCUMENT", doc.id))),
        )
        .await?;
        tx.commit().await?;
        // The row is gone, so nobody can open the file any more; the queued
        // job guarantees the object itself is deleted even if this fails.
        DocumentStorageService::delete(state, &doc.object_key).await;
        Ok(())
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
        // Same fixed per-image ceiling as the patient side — not a
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
        // Dispatch seals the prescription (product-owner rule, 2026-11-13):
        // the client completes the consultation as COMPLETED_WITH_RX once
        // every page is through, and completion takes this same advisory
        // lock, so this post-lock re-read (not the pre-lock `apt` above) is
        // authoritative. A consultation completed WITH_RX accepts no further
        // pages; an exact retry of a stored page still replays above. A
        // consultation completed with another outcome (referred, escalated)
        // keeps the 24-hour late-Rx path.
        let locked = appointment_repo::find_by_id(&mut *tx, appointment_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Appointment not found.".into()))?;
        if locked.status == AppointmentStatus::Completed
            && locked.clinical_outcome.as_deref() == Some("COMPLETED_WITH_RX")
        {
            return Err(AppError::Conflict(
                "The prescription for this consultation has already been dispatched.".into(),
            ));
        }
        Self::ensure_doctor_rx_allowed(&mut tx, state, &locked).await?;
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
        if let Err(error) = consultation_trace::record(
            &mut tx,
            appointment_id,
            None,
            Event::accepted("RX_CREATED", Some(("PRESCRIPTION", rx_id))),
        )
        .await
        {
            DocumentStorageService::delete(state, key).await;
            return Err(error);
        }
        // Notify only once per appointment, the moment the first Rx page
        // becomes available — not on every subsequent page of the same
        // multi-page prescription (`count` is the number of pages that
        // existed *before* this insert).
        if count == 0 {
            if let Err(error) = crate::services::NotificationService::notify_prescription_available(
                &mut tx,
                appointment_id,
            )
            .await
            {
                DocumentStorageService::delete(state, key).await;
                return Err(error.into());
            }
        }
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

#[cfg(test)]
mod doctor_rx_rule_tests {
    use super::*;

    fn at(h: i64) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-11-24T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
            + Duration::hours(h)
    }

    #[test]
    fn only_started_or_recently_completed_consultations_take_a_prescription() {
        let now = at(0);
        let video = Modality::Video;
        let chat = Modality::Chat;
        // A video call someone has joined.
        assert!(doctor_rx_allowed(
            &AppointmentStatus::InConsultation,
            &video,
            at(-1),
            None,
            15,
            now
        ));
        // A video consultation nobody has joined: not yet.
        assert!(!doctor_rx_allowed(
            &AppointmentStatus::Confirmed,
            &video,
            at(-1),
            None,
            15,
            now
        ));
        assert!(!doctor_rx_allowed(
            &AppointmentStatus::Waiting,
            &video,
            at(-1),
            None,
            15,
            now
        ));
        // A chat whose window has opened (15 minutes before the start)...
        assert!(doctor_rx_allowed(
            &AppointmentStatus::Confirmed,
            &chat,
            now + Duration::minutes(10),
            None,
            15,
            now
        ));
        // ...but not a future chat.
        assert!(!doctor_rx_allowed(
            &AppointmentStatus::Confirmed,
            &chat,
            at(2),
            None,
            15,
            now
        ));
        // Completed: within 24 hours only.
        assert!(doctor_rx_allowed(
            &AppointmentStatus::Completed,
            &video,
            at(-5),
            Some(at(-4)),
            15,
            now
        ));
        assert!(!doctor_rx_allowed(
            &AppointmentStatus::Completed,
            &video,
            at(-30),
            Some(at(-25)),
            15,
            now
        ));
        assert!(!doctor_rx_allowed(
            &AppointmentStatus::Completed,
            &chat,
            at(-5),
            None,
            15,
            now
        ));
        // Never for unpaid, cancelled or expired consultations.
        for status in [
            AppointmentStatus::PendingPayment,
            AppointmentStatus::Cancelled,
            AppointmentStatus::Expired,
        ] {
            assert!(!doctor_rx_allowed(&status, &chat, at(-1), None, 15, now));
        }
    }
}
