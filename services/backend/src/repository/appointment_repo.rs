use crate::domain::models::{Appointment, AppointmentClinicalIntake, AppointmentStatus};
use chrono::Utc;
use sqlx::PgExecutor;
use uuid::Uuid;

pub async fn insert<'e, E: PgExecutor<'e>>(
    executor: E,
    apt: &Appointment,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO appointments (id, appointment_number, patient_id, doctor_id, slot_id, modality, status, consultation_fee, clinical_outcome, completed_at, created_at, updated_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)",
    )
    .bind(apt.id)
    .bind(&apt.appointment_number)
    .bind(apt.patient_id)
    .bind(apt.doctor_id)
    .bind(apt.slot_id)
    .bind(apt.modality)
    .bind(apt.status)
    .bind(&apt.consultation_fee)
    .bind(&apt.clinical_outcome)
    .bind(apt.completed_at)
    .bind(apt.created_at)
    .bind(apt.updated_at)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn find_by_id<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
) -> Result<Option<Appointment>, sqlx::Error> {
    sqlx::query_as::<_, Appointment>(
        "SELECT id, appointment_number, patient_id, doctor_id, slot_id, modality, status, consultation_fee, clinical_outcome, completed_at, created_at, updated_at \
         FROM appointments WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(executor)
    .await
}

/// Serializes concurrent callers on one appointment via a Postgres advisory
/// lock, scoped to the current transaction (auto-released at commit/
/// rollback) — the fix for a real race: two concurrent prescription-intake
/// uploads for the same appointment could each individually read a document
/// count under the 5-photo cap and both proceed to insert, together
/// exceeding it (see `PrescriptionService::upload_patient_intake_documents`,
/// the only caller).
///
/// Deliberately **not** `SELECT ... FOR UPDATE` on the `appointments` row:
/// that requires the `UPDATE` privilege in Postgres, which
/// `hellodoctor_api` intentionally does not have on this table (see
/// migrations/20261003000001_rls_hardening's own comment — appointment
/// lifecycle state is payment-worker-only). Confirmed by actually hitting
/// `permission denied for table appointments` when first tried, not
/// assumed. An advisory lock needs no table grant at all and is the
/// correct tool for "serialize on this logical key" regardless.
pub async fn lock_for_document_upload<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
) -> Result<(), sqlx::Error> {
    let key = i64::from_be_bytes(id.as_bytes()[0..8].try_into().unwrap());
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(key)
        .execute(executor)
        .await?;
    Ok(())
}

pub async fn list_all<'e, E: PgExecutor<'e>>(executor: E) -> Result<Vec<Appointment>, sqlx::Error> {
    sqlx::query_as::<_, Appointment>(
        "SELECT id, appointment_number, patient_id, doctor_id, slot_id, modality, status, consultation_fee, clinical_outcome, completed_at, created_at, updated_at \
         FROM appointments ORDER BY created_at DESC",
    )
    .fetch_all(executor)
    .await
}

/// Scoped alternative to `list_all` for handlers that only need a known,
/// typically-small set of appointments (e.g. the ones behind a handful of
/// *active* consultation sessions) — a whole-table fetch here scales with
/// total appointment history, not with the small amount of data actually
/// used, which is exactly backwards for an "active now" admin dashboard.
/// Confirmed as a real latency problem under a production-scale seeded
/// dataset (docs/CONSTRAINED_CAPACITY_TEST.md), not a hypothetical one.
pub async fn find_by_ids<'e, E: PgExecutor<'e>>(
    executor: E,
    ids: &[Uuid],
) -> Result<Vec<Appointment>, sqlx::Error> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    sqlx::query_as::<_, Appointment>(
        "SELECT id, appointment_number, patient_id, doctor_id, slot_id, modality, status, consultation_fee, clinical_outcome, completed_at, created_at, updated_at \
         FROM appointments WHERE id = ANY($1)",
    )
    .bind(ids)
    .fetch_all(executor)
    .await
}

pub async fn set_status<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    status: AppointmentStatus,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE appointments SET status = $2, updated_at = $3 WHERE id = $1")
        .bind(id)
        .bind(status)
        .bind(Utc::now())
        .execute(executor)
        .await?;
    Ok(())
}

pub async fn complete<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    outcome: &str,
) -> Result<(), sqlx::Error> {
    let now = Utc::now();
    sqlx::query(
        "UPDATE appointments SET status = 'COMPLETED', clinical_outcome = $2, completed_at = COALESCE(completed_at, $3), updated_at = $3 WHERE id = $1",
    )
    .bind(id)
    .bind(outcome)
    .bind(now)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn insert_clinical_intake<'e, E: PgExecutor<'e>>(
    executor: E,
    intake: &AppointmentClinicalIntake,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO appointment_clinical_intake (appointment_id, patient_id, chief_complaint, clinical_notes, created_at, updated_at) \
         VALUES ($1,$2,$3,$4,$5,$6)",
    )
    .bind(intake.appointment_id)
    .bind(intake.patient_id)
    .bind(&intake.chief_complaint)
    .bind(&intake.clinical_notes)
    .bind(intake.created_at)
    .bind(intake.updated_at)
    .execute(executor)
    .await?;
    Ok(())
}
