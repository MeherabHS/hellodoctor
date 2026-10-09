use crate::domain::ids::PatientProfileId;
use crate::domain::models::{Prescription, PrescriptionIntakeDocument};
use sqlx::PgExecutor;
use uuid::Uuid;

pub async fn count_intake_documents<'e, E: PgExecutor<'e>>(
    executor: E,
    appointment_id: Uuid,
) -> Result<i64, sqlx::Error> {
    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM prescription_intake_documents WHERE appointment_id = $1",
    )
    .bind(appointment_id)
    .fetch_one(executor)
    .await?;
    Ok(count)
}

pub async fn insert_intake_document<'e, E: PgExecutor<'e>>(
    executor: E,
    doc: &PrescriptionIntakeDocument,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO prescription_intake_documents (id, appointment_id, patient_id, page_number, object_bucket, object_key, content_hash, file_size_bytes, mime_type, security_status, created_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)",
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
    .bind(&doc.security_status)
    .bind(doc.created_at)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn list_intake_documents_for_patient<'e, E: PgExecutor<'e>>(
    executor: E,
    patient_id: PatientProfileId,
) -> Result<Vec<PrescriptionIntakeDocument>, sqlx::Error> {
    sqlx::query_as::<_, PrescriptionIntakeDocument>(
        "SELECT id, appointment_id, patient_id, page_number, object_bucket, object_key, content_hash, file_size_bytes, mime_type, security_status, created_at \
         FROM prescription_intake_documents WHERE patient_id = $1 ORDER BY created_at DESC",
    )
    .bind(patient_id)
    .fetch_all(executor)
    .await
}

/// Scoped by **both** `id` and `appointment_id` — belt-and-suspenders beyond
/// RLS (which already restricts visible rows to the owning patient/assigned
/// doctor/admin roles): a caller who somehow guessed a real document id
/// still can't pull it under the wrong `appointment_id` path parameter.
pub async fn find_intake_document<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    appointment_id: Uuid,
) -> Result<Option<PrescriptionIntakeDocument>, sqlx::Error> {
    sqlx::query_as::<_, PrescriptionIntakeDocument>(
        "SELECT id, appointment_id, patient_id, page_number, object_bucket, object_key, content_hash, file_size_bytes, mime_type, security_status, created_at \
         FROM prescription_intake_documents WHERE id = $1 AND appointment_id = $2",
    )
    .bind(id)
    .bind(appointment_id)
    .fetch_optional(executor)
    .await
}

pub async fn insert_prescription<'e, E: PgExecutor<'e>>(
    executor: E,
    rx: &Prescription,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO prescriptions (id, appointment_id, doctor_id, patient_id, rx_number, integrity_verification_hash, rx_image_object_bucket, rx_image_object_key, doctor_notes, is_no_rx_required, no_rx_reason, security_status, created_at, page_number, mime_type) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)",
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
    .bind(&rx.security_status)
    .bind(rx.created_at)
    .bind(rx.page_number)
    .bind(&rx.mime_type)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn find_prescription_by_appointment<'e, E: PgExecutor<'e>>(
    executor: E,
    appointment_id: Uuid,
) -> Result<Option<Prescription>, sqlx::Error> {
    sqlx::query_as::<_, Prescription>(
        "SELECT id, appointment_id, doctor_id, patient_id, rx_number, integrity_verification_hash, rx_image_object_bucket, rx_image_object_key, doctor_notes, is_no_rx_required, no_rx_reason, security_status, created_at, page_number, mime_type \
         FROM prescriptions WHERE appointment_id = $1 ORDER BY page_number LIMIT 1",
    )
    .bind(appointment_id)
    .fetch_optional(executor)
    .await
}

/// Scoped by both `id` and `appointment_id` — see `find_intake_document`'s
/// doc comment for the rationale (RLS already restricts rows; this is an
/// explicit belt-and-suspenders check in the handler too).
pub async fn find_prescription_by_id<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    appointment_id: Uuid,
) -> Result<Option<Prescription>, sqlx::Error> {
    sqlx::query_as::<_, Prescription>(
        "SELECT id, appointment_id, doctor_id, patient_id, rx_number, integrity_verification_hash, rx_image_object_bucket, rx_image_object_key, doctor_notes, is_no_rx_required, no_rx_reason, security_status, created_at, page_number, mime_type \
         FROM prescriptions WHERE id = $1 AND appointment_id = $2",
    )
    .bind(id)
    .bind(appointment_id)
    .fetch_optional(executor)
    .await
}

pub async fn list_all_prescriptions<'e, E: PgExecutor<'e>>(
    executor: E,
) -> Result<Vec<Prescription>, sqlx::Error> {
    sqlx::query_as::<_, Prescription>(
        "SELECT id, appointment_id, doctor_id, patient_id, rx_number, integrity_verification_hash, rx_image_object_bucket, rx_image_object_key, doctor_notes, is_no_rx_required, no_rx_reason, security_status, created_at, page_number, mime_type \
         FROM prescriptions",
    )
    .fetch_all(executor)
    .await
}
