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

/// The lowest `count` page numbers (1..=20) not used by this appointment's
/// intake documents. A removed page leaves a gap; the next upload fills it,
/// so the page check (1..=20) and the 20-photo cap stay consistent.
pub async fn free_intake_page_numbers<'e, E: PgExecutor<'e>>(
    executor: E,
    appointment_id: Uuid,
    count: i64,
) -> Result<Vec<i16>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT p::smallint FROM generate_series(1, 20) p \
         WHERE NOT EXISTS (SELECT 1 FROM prescription_intake_documents d \
                           WHERE d.appointment_id = $1 AND d.page_number = p) \
         ORDER BY p LIMIT $2",
    )
    .bind(appointment_id)
    .bind(count)
    .fetch_all(executor)
    .await
}

/// This appointment's intake documents, page order. Metadata only.
pub async fn list_intake_documents<'e, E: PgExecutor<'e>>(
    executor: E,
    appointment_id: Uuid,
) -> Result<Vec<PrescriptionIntakeDocument>, sqlx::Error> {
    sqlx::query_as::<_, PrescriptionIntakeDocument>(
        "SELECT id, appointment_id, patient_id, page_number, object_bucket, object_key, content_hash, file_size_bytes, mime_type, security_status, created_at \
         FROM prescription_intake_documents WHERE appointment_id = $1 ORDER BY page_number",
    )
    .bind(appointment_id)
    .fetch_all(executor)
    .await
}

/// Removes one intake row and, in the same transaction, queues its stored
/// object for durable deletion. Returns false when no row was removed.
pub async fn remove_intake_document(
    tx: &mut sqlx::PgConnection,
    doc: &PrescriptionIntakeDocument,
) -> Result<bool, sqlx::Error> {
    let removed = sqlx::query(
        "DELETE FROM prescription_intake_documents WHERE id = $1 AND appointment_id = $2",
    )
    .bind(doc.id)
    .bind(doc.appointment_id)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if removed != 1 {
        return Ok(false);
    }
    sqlx::query(
        "INSERT INTO intake_document_removal_jobs (document_id, appointment_id, object_bucket, object_key) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(doc.id)
    .bind(doc.appointment_id)
    .bind(&doc.object_bucket)
    .bind(&doc.object_key)
    .execute(&mut *tx)
    .await?;
    Ok(true)
}

#[derive(Debug, sqlx::FromRow)]
pub struct IntakeRemovalJob {
    pub document_id: Uuid,
    pub appointment_id: Uuid,
    pub object_key: String,
    pub attempts: i32,
}

pub const MAX_INTAKE_REMOVAL_ATTEMPTS: i32 = 20;

/// Claims due removal jobs with a short lease (same shape as the expiry
/// cleanup queue): a crashed worker's RUNNING row becomes due again.
pub async fn claim_intake_removal_jobs<'e, E: PgExecutor<'e>>(
    executor: E,
    limit: i64,
) -> Result<Vec<IntakeRemovalJob>, sqlx::Error> {
    sqlx::query_as(
        "WITH candidates AS ( \
             SELECT document_id FROM intake_document_removal_jobs \
             WHERE (state='PENDING' AND next_attempt_at <= clock_timestamp()) \
                OR (state='RUNNING' AND lease_until <= clock_timestamp()) \
             ORDER BY next_attempt_at, document_id \
             LIMIT $1 FOR UPDATE SKIP LOCKED \
         ) \
         UPDATE intake_document_removal_jobs j \
            SET state='RUNNING', attempts=j.attempts+1, \
                lease_until=clock_timestamp()+interval '2 minutes', last_error=NULL \
           FROM candidates c WHERE j.document_id=c.document_id \
         RETURNING j.document_id, j.appointment_id, j.object_key, j.attempts",
    )
    .bind(limit)
    .fetch_all(executor)
    .await
}

pub async fn finish_intake_removal_job<'e, E: PgExecutor<'e>>(
    executor: E,
    document_id: Uuid,
    error: Option<&str>,
) -> Result<bool, sqlx::Error> {
    match error {
        None => {
            sqlx::query(
                "UPDATE intake_document_removal_jobs \
                    SET state='COMPLETED', completed_at=clock_timestamp(), lease_until=NULL \
                  WHERE document_id=$1 AND state='RUNNING'",
            )
            .bind(document_id)
            .execute(executor)
            .await?;
            Ok(false)
        }
        Some(error) => {
            let exhausted: Option<bool> = sqlx::query_scalar(
                "UPDATE intake_document_removal_jobs \
                    SET state=CASE WHEN attempts >= $3 THEN 'FAILED' ELSE 'PENDING' END, \
                        next_attempt_at=clock_timestamp()+ \
                            make_interval(secs => LEAST(300, GREATEST(5, attempts * 5))), \
                        lease_until=NULL, last_error=$2 \
                  WHERE document_id=$1 AND state='RUNNING' \
                RETURNING state='FAILED'",
            )
            .bind(document_id)
            .bind(error.chars().take(500).collect::<String>())
            .bind(MAX_INTAKE_REMOVAL_ATTEMPTS)
            .fetch_optional(executor)
            .await?;
            Ok(exhausted.unwrap_or(false))
        }
    }
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
