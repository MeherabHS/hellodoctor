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

/// What the prescription rule needs to know about one consultation. Loaded
/// by [`PrescriptionService::load_rx_facts`] (upload and completion) and by
/// the doctor queue's own SQL ([`rx_facts_columns`]), so both read the same
/// facts.
#[derive(Debug, Clone)]
pub(crate) struct RxFacts {
    pub status: AppointmentStatus,
    pub modality: Modality,
    pub slot_start: DateTime<Utc>,
    /// When the consultation ended: the session's end, else the completion
    /// time; the same instant the doctor's 24-hour clinical-access window
    /// (`doctor_history_context_allowed`) counts from.
    pub ended_at: Option<DateTime<Utc>>,
    pub clinical_outcome: Option<String>,
    /// Both the doctor's and the patient's app reported joining the video
    /// call (`DOCTOR_JOINED` / `PATIENT_JOINED` telemetry; the reporter's side
    /// is set by the server from the caller's identity, so a doctor cannot
    /// report the patient's join).
    pub both_joined: bool,
    /// Both the doctor and the patient have sent at least one chat message.
    pub both_messaged: bool,
}

/// The prescription rule's answer (product owner, 2026-11-24).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RxDecision {
    /// A prescription can be sent now; `closes_at` is the deadline for a
    /// completed consultation.
    Allowed { closes_at: Option<DateTime<Utc>> },
    /// The consultation has not started yet.
    NotStarted,
    /// Completed more than 24 hours ago.
    WindowClosed,
    /// A prescription decision is already recorded.
    Decided,
    /// Unpaid, cancelled, expired or otherwise not a consultation.
    Unavailable,
}

impl RxDecision {
    pub fn allowed(&self) -> bool {
        matches!(self, RxDecision::Allowed { .. })
    }
    /// Stable code for clients.
    pub fn code(&self) -> &'static str {
        match self {
            RxDecision::Allowed { .. } => "OK",
            RxDecision::NotStarted => "NOT_STARTED",
            RxDecision::WindowClosed => "WINDOW_CLOSED",
            RxDecision::Decided => "DECIDED",
            RxDecision::Unavailable => "UNAVAILABLE",
        }
    }
    pub fn message(&self) -> &'static str {
        match self {
            RxDecision::Allowed { .. } => "",
            RxDecision::NotStarted => RX_NOT_STARTED,
            RxDecision::WindowClosed => {
                "The 24-hour prescription window for this consultation has closed."
            }
            RxDecision::Decided => {
                "The prescription decision for this consultation is already recorded and cannot be changed."
            }
            RxDecision::Unavailable => "This consultation cannot take a prescription.",
        }
    }
    pub fn to_json(self) -> serde_json::Value {
        serde_json::json!({
            "allowed": self.allowed(),
            "reason": self.code(),
            "message": self.message(),
            "closes_at": match self { RxDecision::Allowed { closes_at } => closes_at, _ => None },
        })
    }
}

/// How long after the end a completed consultation can still take a
/// prescription.
pub(crate) const RX_WINDOW_HOURS: i64 = 24;

/// Product owner, 2026-11-24: a doctor sends a prescription only for a
/// consultation that has started, or one completed in the last 24 hours
/// without a prescription decision. Never before the patient is seen; once
/// decided, never again. Server time (Africa/Nairobi deployment) decides.
///
/// "Started" means:
/// * video: someone joined and the signed Agora presence callback moved it to
///   IN_CONSULTATION, or both apps reported joining the call (covers a lost
///   provider callback);
/// * chat: its window has opened (`window_before_mins` before the slot) and
///   both the doctor and the patient have sent a message, so a patient who
///   never turned up cannot be prescribed for.
pub(crate) fn doctor_rx_decision(
    f: &RxFacts,
    window_before_mins: i64,
    now: DateTime<Utc>,
) -> RxDecision {
    match f.status {
        AppointmentStatus::Completed => {
            if matches!(
                f.clinical_outcome.as_deref(),
                Some("COMPLETED_WITH_RX") | Some("COMPLETED_NO_RX")
            ) {
                return RxDecision::Decided;
            }
            match f.ended_at {
                Some(ended) if now < ended + Duration::hours(RX_WINDOW_HOURS) => {
                    RxDecision::Allowed {
                        closes_at: Some(ended + Duration::hours(RX_WINDOW_HOURS)),
                    }
                }
                _ => RxDecision::WindowClosed,
            }
        }
        AppointmentStatus::InConsultation => RxDecision::Allowed { closes_at: None },
        AppointmentStatus::Confirmed | AppointmentStatus::Waiting => {
            let started = match f.modality {
                Modality::Video => f.both_joined,
                Modality::Chat => {
                    now >= f.slot_start - Duration::minutes(window_before_mins) && f.both_messaged
                }
            };
            if started {
                RxDecision::Allowed { closes_at: None }
            } else {
                RxDecision::NotStarted
            }
        }
        _ => RxDecision::Unavailable,
    }
}

/// The doctor-side refusal for a consultation that has not started.
pub(crate) const RX_NOT_STARTED: &str = "A prescription can be sent once the consultation has started: after you and the patient have both joined the call, or both sent a message in the chat.";

/// SQL for the two "started" facts of the appointment whose id is the SQL
/// expression `appointment` and whose doctor's user id is `doctor_user`.
/// Shared by [`PrescriptionService::load_rx_facts`] and the doctor queue so
/// both evaluate the same thing.
pub(crate) fn rx_facts_columns(appointment: &str, doctor_user: &str) -> String {
    format!(
        "(EXISTS(SELECT 1 FROM consultation_sessions rs JOIN consultation_telemetry_events re ON re.session_id=rs.id \
           WHERE rs.appointment_id={appointment} AND re.event_type='DOCTOR_JOINED' AND re.actor_perspective='DOCTOR') \
          AND EXISTS(SELECT 1 FROM consultation_sessions rs JOIN consultation_telemetry_events re ON re.session_id=rs.id \
           WHERE rs.appointment_id={appointment} AND re.event_type='PATIENT_JOINED' AND re.actor_perspective='PATIENT')) AS rx_both_joined, \
         (EXISTS(SELECT 1 FROM chat_conversations rc JOIN chat_messages rm ON rm.conversation_id=rc.id \
           WHERE rc.appointment_id={appointment} AND rm.sender_id={doctor_user}) \
          AND EXISTS(SELECT 1 FROM chat_conversations rc JOIN chat_messages rm ON rm.conversation_id=rc.id \
           WHERE rc.appointment_id={appointment} AND rm.sender_id<>{doctor_user})) AS rx_both_messaged"
    )
}

/// The rule's answer for one doctor-queue row (columns from
/// `doctor_repo::list_appointments_as_of`), as the `rx` object the apps
/// display: they show what the server decides instead of re-deriving it.
pub(crate) fn rx_json_for_queue_row(
    row: &sqlx::postgres::PgRow,
    window_before_mins: i64,
    now: DateTime<Utc>,
) -> serde_json::Value {
    use sqlx::Row;
    let parse = |column: &str| serde_json::Value::String(row.get::<String, _>(column));
    let (Ok(status), Ok(modality)) = (
        serde_json::from_value::<AppointmentStatus>(parse("status")),
        serde_json::from_value::<Modality>(parse("modality")),
    ) else {
        return RxDecision::Unavailable.to_json();
    };
    let ended_at = row
        .get::<Option<DateTime<Utc>>, _>("session_ended_at")
        .or(row.get::<Option<DateTime<Utc>>, _>("completed_at"));
    let facts = RxFacts {
        status,
        modality,
        slot_start: row.get("start_time"),
        ended_at,
        clinical_outcome: row.get("clinical_outcome"),
        both_joined: row.get("rx_both_joined"),
        both_messaged: row.get("rx_both_messaged"),
    };
    doctor_rx_decision(&facts, window_before_mins, now).to_json()
}

impl PrescriptionService {
    /// The facts [`doctor_rx_decision`] needs, read in the caller's own
    /// (RLS-scoped) transaction.
    pub(crate) async fn load_rx_facts(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        apt: &Appointment,
    ) -> Result<RxFacts, AppError> {
        let sql = format!(
            "SELECT s.start_time, COALESCE(cs.ended_at, a.completed_at) AS ended_at, {} \
               FROM appointments a JOIN doctor_schedule_slots s ON s.id=a.slot_id \
               JOIN doctor_profiles d ON d.id=a.doctor_id \
               LEFT JOIN consultation_sessions cs ON cs.appointment_id=a.id \
              WHERE a.id=$1",
            rx_facts_columns("a.id", "d.user_id")
        );
        let (slot_start, ended_at, both_joined, both_messaged): (
            DateTime<Utc>,
            Option<DateTime<Utc>>,
            bool,
            bool,
        ) = sqlx::query_as(&sql)
            .bind(apt.id)
            .fetch_one(&mut **tx)
            .await?;
        Ok(RxFacts {
            status: apt.status,
            modality: apt.modality,
            slot_start,
            ended_at,
            clinical_outcome: apt.clinical_outcome.clone(),
            both_joined,
            both_messaged,
        })
    }

    /// Refuses a doctor prescription (page upload or completion with Rx)
    /// unless [`doctor_rx_decision`] allows it. Called under the
    /// per-consultation advisory lock with a fresh read.
    pub(crate) async fn ensure_doctor_rx_allowed(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        state: &AppState,
        apt: &Appointment,
    ) -> Result<(), AppError> {
        let facts = Self::load_rx_facts(tx, apt).await?;
        let decision = doctor_rx_decision(
            &facts,
            state.config.agora_token_window_before_mins,
            Utc::now(),
        );
        if decision.allowed() {
            Ok(())
        } else {
            Err(AppError::Conflict(decision.message().into()))
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

/// A dispatched prescription is final (product owner, 2026-11-13); a doctor
/// corrects it with a written note the patient sees (2026-11-24). Notes are
/// append-only, at most this many per consultation, and only within this
/// many days of the consultation's end.
pub(crate) const RX_CORRECTIONS_MAX: i64 = 5;
pub(crate) const RX_CORRECTION_DAYS: i64 = 7;

impl PrescriptionService {
    /// Adds a correction note to a dispatched prescription and notifies the
    /// patient, in one transaction.
    pub async fn add_prescription_correction(
        state: &AppState,
        ctx: &AuthContext,
        appointment_id: Uuid,
        doctor_id: DoctorProfileId,
        note: String,
    ) -> Result<serde_json::Value, AppError> {
        let note = note.trim().to_string();
        if !(3..=1000).contains(&note.chars().count()) {
            return Err(AppError::UnprocessableEntity(
                "A correction note must be 3 to 1000 characters.".into(),
            ));
        }
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
        if apt.status != AppointmentStatus::Completed
            || apt.clinical_outcome.as_deref() != Some("COMPLETED_WITH_RX")
        {
            return Err(AppError::Conflict(
                "A correction note can be added only after the prescription has been sent.".into(),
            ));
        }
        let ended = Self::load_rx_facts(&mut tx, &apt)
            .await?
            .ended_at
            .unwrap_or_else(Utc::now);
        if Utc::now() >= ended + Duration::days(RX_CORRECTION_DAYS) {
            return Err(AppError::Conflict(format!(
                "Correction notes can be added for {RX_CORRECTION_DAYS} days after the consultation."
            )));
        }
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM prescription_corrections WHERE appointment_id=$1",
        )
        .bind(appointment_id)
        .fetch_one(&mut *tx)
        .await?;
        if count >= RX_CORRECTIONS_MAX {
            return Err(AppError::Conflict(format!(
                "At most {RX_CORRECTIONS_MAX} correction notes per prescription."
            )));
        }
        let (id, created_at): (Uuid, DateTime<Utc>) = sqlx::query_as(
            "INSERT INTO prescription_corrections(appointment_id, doctor_id, note) VALUES ($1,$2,$3) RETURNING id, created_at",
        )
        .bind(appointment_id)
        .bind(doctor_id)
        .bind(&note)
        .fetch_one(&mut *tx)
        .await?;
        crate::services::NotificationService::notify_prescription_corrected(
            &mut tx,
            appointment_id,
            id,
        )
        .await?;
        tx.commit().await?;
        Ok(serde_json::json!({"id": id, "note": note, "created_at": created_at}))
    }

    /// The correction notes on one consultation's prescription, oldest
    /// first. Row-level security limits them to its doctor and patient.
    pub async fn list_prescription_corrections(
        state: &AppState,
        ctx: &AuthContext,
        appointment_id: Uuid,
    ) -> Result<Vec<serde_json::Value>, AppError> {
        let mut tx = state.begin_authed(ctx).await?;
        // Someone who cannot see the consultation gets 404, like every other
        // consultation route (found by the pentest: an empty 200 before).
        appointment_repo::find_by_id(&mut *tx, appointment_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Appointment not found.".into()))?;
        let rows: Vec<(Uuid, String, DateTime<Utc>)> = sqlx::query_as(
            "SELECT id, note, created_at FROM prescription_corrections WHERE appointment_id=$1 ORDER BY created_at, id",
        )
        .bind(appointment_id)
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(rows
            .into_iter()
            .map(|(id, note, created_at)| serde_json::json!({"id": id, "note": note, "created_at": created_at}))
            .collect())
    }
}

#[cfg(test)]
mod doctor_rx_rule_tests {
    use super::*;

    fn at(h: f64) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-11-24T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
            + Duration::minutes((h * 60.0) as i64)
    }

    fn facts(status: AppointmentStatus, modality: Modality) -> RxFacts {
        RxFacts {
            status,
            modality,
            slot_start: at(-1.0),
            ended_at: None,
            clinical_outcome: None,
            both_joined: false,
            both_messaged: false,
        }
    }

    #[test]
    fn video_starts_when_someone_joined() {
        let now = at(0.0);
        let d = |f: &RxFacts| doctor_rx_decision(f, 15, now);
        assert!(d(&facts(AppointmentStatus::InConsultation, Modality::Video)).allowed());
        assert_eq!(
            d(&facts(AppointmentStatus::Confirmed, Modality::Video)),
            RxDecision::NotStarted
        );
        assert_eq!(
            d(&facts(AppointmentStatus::Waiting, Modality::Video)),
            RxDecision::NotStarted
        );
        // The provider callback was lost, but both apps reported joining.
        let joined = RxFacts {
            both_joined: true,
            ..facts(AppointmentStatus::Confirmed, Modality::Video)
        };
        assert!(d(&joined).allowed());
    }

    #[test]
    fn chat_starts_when_both_have_written_inside_the_window() {
        let now = at(0.0);
        let d = |f: &RxFacts| doctor_rx_decision(f, 15, now);
        let open = facts(AppointmentStatus::Confirmed, Modality::Chat);
        // Window open, but the patient never wrote: a no-show.
        assert_eq!(d(&open), RxDecision::NotStarted);
        assert!(d(&RxFacts {
            both_messaged: true,
            ..open.clone()
        })
        .allowed());
        // A chat booked for tomorrow does not count before its window.
        let tomorrow = RxFacts {
            slot_start: at(24.0),
            both_messaged: true,
            ..open.clone()
        };
        assert_eq!(d(&tomorrow), RxDecision::NotStarted);
        // 10 minutes before the slot is inside the 15-minute window.
        let soon = RxFacts {
            slot_start: at(10.0 / 60.0),
            both_messaged: true,
            ..open
        };
        assert!(d(&soon).allowed());
    }

    #[test]
    fn completed_has_24_hours_and_one_decision() {
        let now = at(0.0);
        let d = |f: &RxFacts| doctor_rx_decision(f, 15, now);
        let done = |ended: f64, outcome: Option<&str>| RxFacts {
            ended_at: Some(at(ended)),
            clinical_outcome: outcome.map(str::to_string),
            ..facts(AppointmentStatus::Completed, Modality::Video)
        };
        assert_eq!(
            d(&done(-2.0, None)),
            RxDecision::Allowed {
                closes_at: Some(at(22.0))
            }
        );
        assert_eq!(d(&done(-25.0, None)), RxDecision::WindowClosed);
        assert_eq!(
            d(&done(-1.0, Some("COMPLETED_WITH_RX"))),
            RxDecision::Decided
        );
        assert_eq!(d(&done(-1.0, Some("COMPLETED_NO_RX"))), RxDecision::Decided);
        assert_eq!(
            d(&facts(AppointmentStatus::Completed, Modality::Chat)),
            RxDecision::WindowClosed
        );
    }

    #[test]
    fn never_for_unpaid_cancelled_or_expired() {
        for status in [
            AppointmentStatus::PendingPayment,
            AppointmentStatus::Cancelled,
            AppointmentStatus::Expired,
        ] {
            let f = RxFacts {
                both_messaged: true,
                both_joined: true,
                ..facts(status, Modality::Chat)
            };
            assert_eq!(doctor_rx_decision(&f, 15, at(0.0)), RxDecision::Unavailable);
        }
    }
}
