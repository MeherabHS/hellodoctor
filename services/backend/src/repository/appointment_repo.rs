use crate::domain::models::{Appointment, AppointmentClinicalIntake, AppointmentStatus};
use chrono::Utc;
use sqlx::PgExecutor;
use uuid::Uuid;

pub async fn insert<'e, E: PgExecutor<'e>>(executor: E, apt: &Appointment) -> Result<(), sqlx::Error> {
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

pub async fn find_by_id<'e, E: PgExecutor<'e>>(executor: E, id: Uuid) -> Result<Option<Appointment>, sqlx::Error> {
    sqlx::query_as::<_, Appointment>(
        "SELECT id, appointment_number, patient_id, doctor_id, slot_id, modality, status, consultation_fee, clinical_outcome, completed_at, created_at, updated_at \
         FROM appointments WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(executor)
    .await
}

pub async fn list_all<'e, E: PgExecutor<'e>>(executor: E) -> Result<Vec<Appointment>, sqlx::Error> {
    sqlx::query_as::<_, Appointment>(
        "SELECT id, appointment_number, patient_id, doctor_id, slot_id, modality, status, consultation_fee, clinical_outcome, completed_at, created_at, updated_at \
         FROM appointments ORDER BY created_at DESC",
    )
    .fetch_all(executor)
    .await
}

pub async fn set_status<'e, E: PgExecutor<'e>>(executor: E, id: Uuid, status: AppointmentStatus) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE appointments SET status = $2, updated_at = $3 WHERE id = $1")
        .bind(id)
        .bind(status)
        .bind(Utc::now())
        .execute(executor)
        .await?;
    Ok(())
}

pub async fn complete<'e, E: PgExecutor<'e>>(executor: E, id: Uuid, outcome: &str) -> Result<(), sqlx::Error> {
    let now = Utc::now();
    sqlx::query(
        "UPDATE appointments SET status = 'COMPLETED', clinical_outcome = $2, completed_at = $3, updated_at = $3 WHERE id = $1",
    )
    .bind(id)
    .bind(outcome)
    .bind(now)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn insert_clinical_intake<'e, E: PgExecutor<'e>>(executor: E, intake: &AppointmentClinicalIntake) -> Result<(), sqlx::Error> {
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
