use crate::domain::models::{DoctorProfile, ScheduleSlot, SlotStatus};
use chrono::Utc;
use sqlx::PgExecutor;
use uuid::Uuid;

pub async fn list<'e, E: PgExecutor<'e>>(executor: E) -> Result<Vec<DoctorProfile>, sqlx::Error> {
    sqlx::query_as::<_, DoctorProfile>(
        "SELECT id, user_id, full_name, license_number, license_authority, license_country, primary_specialty, experience_years, current_hospital, qualifications, consultation_fee_video, consultation_fee_chat, residential_address, verified_phone, is_on_duty, is_verified, created_at, updated_at, deleted_at \
         FROM doctor_profiles WHERE deleted_at IS NULL ORDER BY full_name",
    )
    .fetch_all(executor)
    .await
}

pub async fn find_by_id<'e, E: PgExecutor<'e>>(executor: E, id: Uuid) -> Result<Option<DoctorProfile>, sqlx::Error> {
    sqlx::query_as::<_, DoctorProfile>(
        "SELECT id, user_id, full_name, license_number, license_authority, license_country, primary_specialty, experience_years, current_hospital, qualifications, consultation_fee_video, consultation_fee_chat, residential_address, verified_phone, is_on_duty, is_verified, created_at, updated_at, deleted_at \
         FROM doctor_profiles WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_optional(executor)
    .await
}

pub async fn find_by_license<'e, E: PgExecutor<'e>>(executor: E, license_number: &str) -> Result<Option<DoctorProfile>, sqlx::Error> {
    sqlx::query_as::<_, DoctorProfile>(
        "SELECT id, user_id, full_name, license_number, license_authority, license_country, primary_specialty, experience_years, current_hospital, qualifications, consultation_fee_video, consultation_fee_chat, residential_address, verified_phone, is_on_duty, is_verified, created_at, updated_at, deleted_at \
         FROM doctor_profiles WHERE license_number = $1 AND deleted_at IS NULL",
    )
    .bind(license_number)
    .fetch_optional(executor)
    .await
}

pub async fn insert<'e, E: PgExecutor<'e>>(executor: E, doc: &DoctorProfile) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO doctor_profiles (id, user_id, full_name, license_number, license_authority, license_country, primary_specialty, experience_years, current_hospital, qualifications, consultation_fee_video, consultation_fee_chat, residential_address, verified_phone, is_on_duty, is_verified, created_at, updated_at, deleted_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19)",
    )
    .bind(doc.id)
    .bind(doc.user_id)
    .bind(&doc.full_name)
    .bind(&doc.license_number)
    .bind(&doc.license_authority)
    .bind(&doc.license_country)
    .bind(&doc.primary_specialty)
    .bind(doc.experience_years)
    .bind(&doc.current_hospital)
    .bind(&doc.qualifications)
    .bind(&doc.consultation_fee_video)
    .bind(&doc.consultation_fee_chat)
    .bind(&doc.residential_address)
    .bind(&doc.verified_phone)
    .bind(doc.is_on_duty)
    .bind(doc.is_verified)
    .bind(doc.created_at)
    .bind(doc.updated_at)
    .bind(doc.deleted_at)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn set_verified<'e, E: PgExecutor<'e>>(executor: E, id: Uuid, is_verified: bool, is_on_duty: Option<bool>) -> Result<(), sqlx::Error> {
    if let Some(on_duty) = is_on_duty {
        sqlx::query("UPDATE doctor_profiles SET is_verified = $2, is_on_duty = $3, updated_at = $4 WHERE id = $1")
            .bind(id)
            .bind(is_verified)
            .bind(on_duty)
            .bind(Utc::now())
            .execute(executor)
            .await?;
    } else {
        sqlx::query("UPDATE doctor_profiles SET is_verified = $2, updated_at = $3 WHERE id = $1")
            .bind(id)
            .bind(is_verified)
            .bind(Utc::now())
            .execute(executor)
            .await?;
    }
    Ok(())
}

// ---- Schedule slots ----

pub async fn list_slots_for_doctor<'e, E: PgExecutor<'e>>(executor: E, doctor_id: Uuid) -> Result<Vec<ScheduleSlot>, sqlx::Error> {
    sqlx::query_as::<_, ScheduleSlot>(
        "SELECT id, doctor_id, start_time, end_time, status, created_at FROM doctor_schedule_slots WHERE doctor_id = $1 AND status = 'AVAILABLE' ORDER BY start_time",
    )
    .bind(doctor_id)
    .fetch_all(executor)
    .await
}

pub async fn list_all_slots<'e, E: PgExecutor<'e>>(executor: E) -> Result<Vec<ScheduleSlot>, sqlx::Error> {
    sqlx::query_as::<_, ScheduleSlot>(
        "SELECT id, doctor_id, start_time, end_time, status, created_at FROM doctor_schedule_slots ORDER BY start_time",
    )
    .fetch_all(executor)
    .await
}

pub async fn find_slot<'e, E: PgExecutor<'e>>(executor: E, slot_id: Uuid) -> Result<Option<ScheduleSlot>, sqlx::Error> {
    sqlx::query_as::<_, ScheduleSlot>(
        "SELECT id, doctor_id, start_time, end_time, status, created_at FROM doctor_schedule_slots WHERE id = $1",
    )
    .bind(slot_id)
    .fetch_optional(executor)
    .await
}

/// Locks the row (`FOR UPDATE`) and atomically transitions AVAILABLE -> LOCKED_IN_PAYMENT.
/// Must be called within a transaction. Returns the slot's doctor_id if the lock succeeded.
pub async fn try_lock_slot(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    slot_id: Uuid,
    doctor_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let row = sqlx::query_as::<_, ScheduleSlot>(
        "SELECT id, doctor_id, start_time, end_time, status, created_at FROM doctor_schedule_slots WHERE id = $1 FOR UPDATE",
    )
    .bind(slot_id)
    .fetch_optional(&mut **tx)
    .await?;

    let Some(slot) = row else { return Ok(false) };
    if slot.doctor_id != doctor_id || slot.status != SlotStatus::Available {
        return Ok(false);
    }

    sqlx::query("UPDATE doctor_schedule_slots SET status = 'LOCKED_IN_PAYMENT' WHERE id = $1")
        .bind(slot_id)
        .execute(&mut **tx)
        .await?;
    Ok(true)
}

pub async fn set_slot_status<'e, E: PgExecutor<'e>>(executor: E, slot_id: Uuid, status: SlotStatus) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE doctor_schedule_slots SET status = $2 WHERE id = $1")
        .bind(slot_id)
        .bind(status)
        .execute(executor)
        .await?;
    Ok(())
}

pub async fn insert_slot<'e, E: PgExecutor<'e>>(executor: E, slot: &ScheduleSlot) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO doctor_schedule_slots (id, doctor_id, start_time, end_time, status, created_at) VALUES ($1,$2,$3,$4,$5,$6)",
    )
    .bind(slot.id)
    .bind(slot.doctor_id)
    .bind(slot.start_time)
    .bind(slot.end_time)
    .bind(slot.status)
    .bind(slot.created_at)
    .execute(executor)
    .await?;
    Ok(())
}
