use crate::domain::models::{Prescription, PrescriptionIntakeDocument};
use sqlx::PgExecutor;
use uuid::Uuid;

pub async fn count_intake_documents<'e, E: PgExecutor<'e>>(executor: E, appointment_id: Uuid) -> Result<i64, sqlx::Error> {
    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM prescription_intake_documents WHERE appointment_id = $1",
    )
    .bind(appointment_id)
    .fetch_one(executor)
    .await?;
    Ok(count)
}

pub async fn insert_intake_document<'e, E: PgExecutor<'e>>(executor: E, doc: &PrescriptionIntakeDocument) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO prescription_intake_documents (id, appointment_id, patient_id, page_number, object_bucket, object_key, content_hash, file_size_bytes, mime_type, created_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
    )
    .bind(doc.id)
    .bind(doc.appointment_id)
    .bind(doc.patient_id)
    .bind(doc.page_number)
    .bind(&doc.object_bucket)
    .bind(&doc.object_key)
    .bind(&doc.content_hash)
    .bind(doc.file_size_bytes)
    .bind(&doc.mime_type)
    .bind(doc.created_at)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn list_intake_documents_for_patient<'e, E: PgExecutor<'e>>(executor: E, patient_id: Uuid) -> Result<Vec<PrescriptionIntakeDocument>, sqlx::Error> {
    sqlx::query_as::<_, PrescriptionIntakeDocument>(
        "SELECT id, appointment_id, patient_id, page_number, object_bucket, object_key, content_hash, file_size_bytes, mime_type, created_at \
         FROM prescription_intake_documents WHERE patient_id = $1 ORDER BY created_at DESC",
    )
    .bind(patient_id)
    .fetch_all(executor)
    .await
}

pub async fn insert_prescription<'e, E: PgExecutor<'e>>(executor: E, rx: &Prescription) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO prescriptions (id, appointment_id, doctor_id, patient_id, rx_number, integrity_verification_hash, rx_image_object_bucket, rx_image_object_key, doctor_notes, is_no_rx_required, no_rx_reason, created_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)",
    )
    .bind(rx.id)
    .bind(rx.appointment_id)
    .bind(rx.doctor_id)
    .bind(rx.patient_id)
    .bind(&rx.rx_number)
    .bind(&rx.integrity_verification_hash)
    .bind(&rx.rx_image_object_bucket)
    .bind(&rx.rx_image_object_key)
    .bind(&rx.doctor_notes)
    .bind(rx.is_no_rx_required)
    .bind(&rx.no_rx_reason)
    .bind(rx.created_at)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn find_prescription_by_appointment<'e, E: PgExecutor<'e>>(executor: E, appointment_id: Uuid) -> Result<Option<Prescription>, sqlx::Error> {
    sqlx::query_as::<_, Prescription>(
        "SELECT id, appointment_id, doctor_id, patient_id, rx_number, integrity_verification_hash, rx_image_object_bucket, rx_image_object_key, doctor_notes, is_no_rx_required, no_rx_reason, created_at \
         FROM prescriptions WHERE appointment_id = $1",
    )
    .bind(appointment_id)
    .fetch_optional(executor)
    .await
}

pub async fn list_all_prescriptions<'e, E: PgExecutor<'e>>(executor: E) -> Result<Vec<Prescription>, sqlx::Error> {
    sqlx::query_as::<_, Prescription>(
        "SELECT id, appointment_id, doctor_id, patient_id, rx_number, integrity_verification_hash, rx_image_object_bucket, rx_image_object_key, doctor_notes, is_no_rx_required, no_rx_reason, created_at \
         FROM prescriptions",
    )
    .fetch_all(executor)
    .await
}
