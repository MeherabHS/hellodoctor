use crate::domain::ids::{DoctorProfileId, UserId};
use crate::domain::models::{
    DoctorProfile, ProfessionalVerificationStatus, ScheduleSlot, SlotStatus,
};
use sqlx::PgExecutor;
use uuid::Uuid;

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct PublicDoctorProfile {
    pub id: DoctorProfileId,
    pub full_name: String,
    pub license_number: String,
    pub license_authority: String,
    pub license_country: String,
    pub primary_specialty: String,
    pub experience_years: i16,
    pub current_hospital: String,
    pub qualifications: Vec<String>,
    pub consultation_fee_video: bigdecimal::BigDecimal,
    pub consultation_fee_chat: bigdecimal::BigDecimal,
    pub is_on_duty: bool,
    pub is_verified: bool,
    pub professional_verification_status: ProfessionalVerificationStatus,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    /// Non-sensitive directory-visible signal that a profile picture exists
    /// (and when it last changed, for client-side cache-busting) — the raw
    /// storage key is never exposed here or anywhere else; the photo bytes
    /// are fetched through `GET /doctors/:id/photo`.
    pub profile_picture_size_bytes: Option<i32>,
    pub profile_picture_updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Private object-storage reference for a profile picture — the raw
/// `object_key`/`bucket`/`content_hash`/`mime_type` are internal only and
/// never serialized to a client; `size_bytes`/`updated_at` may be surfaced
/// as a "does a photo exist" / cache-busting signal. Deliberately a separate,
/// narrowly-queried struct rather than new fields on `DoctorProfile`/
/// `PatientProfile` themselves, which are constructed as Rust struct
/// literals in several unrelated places (seed.rs, auth_service.rs, tests,
/// ...) that a required new field would otherwise force changes to.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ProfilePictureMeta {
    pub object_key: Option<String>,
    pub bucket: Option<String>,
    pub content_hash: Option<String>,
    pub mime_type: Option<String>,
    pub size_bytes: Option<i32>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

const PROFILE_PICTURE_META_COLUMNS: &str = "profile_picture_object_key AS object_key, profile_picture_bucket AS bucket, profile_picture_content_hash AS content_hash, profile_picture_mime_type AS mime_type, profile_picture_size_bytes AS size_bytes, profile_picture_updated_at AS updated_at";

pub async fn find_profile_picture<'e, E: PgExecutor<'e>>(
    executor: E,
    id: DoctorProfileId,
) -> Result<Option<ProfilePictureMeta>, sqlx::Error> {
    sqlx::query_as::<_, ProfilePictureMeta>(&format!(
        "SELECT {PROFILE_PICTURE_META_COLUMNS} FROM doctor_profiles WHERE id=$1 AND deleted_at IS NULL"
    ))
    .bind(id)
    .fetch_optional(executor)
    .await
}

/// Overwrites (never `COALESCE`s) every photo column, so passing `None` for
/// all of them — the "remove photo" path — genuinely clears the row rather
/// than leaving the previous value in place. Returns the number of rows
/// updated (0 means no such doctor profile exists), matching this
/// repository's existing `rows_affected()` contract.
#[allow(clippy::too_many_arguments)]
pub async fn update_profile_picture<'e, E: PgExecutor<'e>>(
    executor: E,
    id: DoctorProfileId,
    object_key: Option<&str>,
    bucket: Option<&str>,
    content_hash: Option<&str>,
    mime_type: Option<&str>,
    size_bytes: Option<i32>,
    updated_at: chrono::DateTime<chrono::Utc>,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE doctor_profiles SET profile_picture_object_key=$2, profile_picture_bucket=$3, \
         profile_picture_content_hash=$4, profile_picture_mime_type=$5, \
         profile_picture_size_bytes=$6, profile_picture_updated_at=$7 \
         WHERE id=$1 AND deleted_at IS NULL",
    )
    .bind(id)
    .bind(object_key)
    .bind(bucket)
    .bind(content_hash)
    .bind(mime_type)
    .bind(size_bytes)
    .bind(updated_at)
    .execute(executor)
    .await?;
    Ok(result.rows_affected())
}

pub async fn find_public<'e, E: PgExecutor<'e>>(
    executor: E,
    id: DoctorProfileId,
) -> Result<Option<PublicDoctorProfile>, sqlx::Error> {
    sqlx::query_as("SELECT * FROM doctor_public_directory WHERE id=$1")
        .bind(id)
        .fetch_optional(executor)
        .await
}

pub async fn list_public<'e, E: PgExecutor<'e>>(
    executor: E,
) -> Result<Vec<PublicDoctorProfile>, sqlx::Error> {
    sqlx::query_as("SELECT * FROM doctor_public_directory ORDER BY full_name")
        .fetch_all(executor)
        .await
}

pub async fn list<'e, E: PgExecutor<'e>>(executor: E) -> Result<Vec<DoctorProfile>, sqlx::Error> {
    sqlx::query_as::<_, DoctorProfile>(
        "SELECT id, user_id, full_name, license_number, license_authority, license_country, primary_specialty, experience_years, current_hospital, qualifications, consultation_fee_video, consultation_fee_chat, residential_address, verified_phone, is_on_duty, is_verified, professional_verification_status, professional_verification_reviewed_at, professional_verification_reviewed_by, professional_verification_note, created_at, updated_at, deleted_at \
         FROM doctor_profiles WHERE deleted_at IS NULL ORDER BY full_name",
    )
    .fetch_all(executor)
    .await
}

pub async fn find_by_id<'e, E: PgExecutor<'e>>(
    executor: E,
    id: DoctorProfileId,
) -> Result<Option<DoctorProfile>, sqlx::Error> {
    sqlx::query_as::<_, DoctorProfile>(
        "SELECT id, user_id, full_name, license_number, license_authority, license_country, primary_specialty, experience_years, current_hospital, qualifications, consultation_fee_video, consultation_fee_chat, residential_address, verified_phone, is_on_duty, is_verified, professional_verification_status, professional_verification_reviewed_at, professional_verification_reviewed_by, professional_verification_note, created_at, updated_at, deleted_at \
         FROM doctor_profiles WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_optional(executor)
    .await
}

pub async fn find_by_user_id<'e, E: PgExecutor<'e>>(
    executor: E,
    user_id: UserId,
) -> Result<Option<DoctorProfile>, sqlx::Error> {
    sqlx::query_as::<_, DoctorProfile>(
        "SELECT id, user_id, full_name, license_number, license_authority, license_country, primary_specialty, experience_years, current_hospital, qualifications, consultation_fee_video, consultation_fee_chat, residential_address, verified_phone, is_on_duty, is_verified, professional_verification_status, professional_verification_reviewed_at, professional_verification_reviewed_by, professional_verification_note, created_at, updated_at, deleted_at \
         FROM doctor_profiles WHERE user_id = $1 AND deleted_at IS NULL",
    )
    .bind(user_id)
    .fetch_optional(executor)
    .await
}

pub async fn find_by_license<'e, E: PgExecutor<'e>>(
    executor: E,
    license_number: &str,
) -> Result<Option<DoctorProfile>, sqlx::Error> {
    sqlx::query_as::<_, DoctorProfile>(
        "SELECT id, user_id, full_name, license_number, license_authority, license_country, primary_specialty, experience_years, current_hospital, qualifications, consultation_fee_video, consultation_fee_chat, residential_address, verified_phone, is_on_duty, is_verified, professional_verification_status, professional_verification_reviewed_at, professional_verification_reviewed_by, professional_verification_note, created_at, updated_at, deleted_at \
         FROM doctor_profiles WHERE license_number = $1 AND deleted_at IS NULL",
    )
    .bind(license_number)
    .fetch_optional(executor)
    .await
}

pub async fn insert<'e, E: PgExecutor<'e>>(
    executor: E,
    doc: &DoctorProfile,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO doctor_profiles (id, user_id, full_name, license_number, license_authority, license_country, primary_specialty, experience_years, current_hospital, qualifications, consultation_fee_video, consultation_fee_chat, residential_address, verified_phone, is_on_duty, is_verified, professional_verification_status, professional_verification_reviewed_at, professional_verification_reviewed_by, professional_verification_note, created_at, updated_at, deleted_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,$23)",
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
    .bind(doc.is_professionally_verified())
    .bind(doc.professional_verification_status)
    .bind(doc.professional_verification_reviewed_at)
    .bind(doc.professional_verification_reviewed_by)
    .bind(&doc.professional_verification_note)
    .bind(doc.created_at)
    .bind(doc.updated_at)
    .bind(doc.deleted_at)
    .execute(executor)
    .await?;
    Ok(())
}

/// The database condition serializes competing reviews: exactly one request
/// consumes PENDING, and no repeated/opposite terminal review can mutate it.
pub async fn review_professional_registration<'e, E: PgExecutor<'e>>(
    executor: E,
    id: DoctorProfileId,
    status: ProfessionalVerificationStatus,
    reviewer: UserId,
    note: Option<&str>,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE doctor_profiles SET professional_verification_status=$2, professional_verification_reviewed_at=NOW(), professional_verification_reviewed_by=$3, professional_verification_note=$4, updated_at=NOW() WHERE id=$1 AND deleted_at IS NULL AND professional_verification_status='PENDING' AND $2 IN ('VERIFIED','REJECTED')",
    )
    .bind(id).bind(status).bind(reviewer).bind(note).execute(executor).await?;
    Ok(result.rows_affected() == 1)
}

/// h22 remediation: the only reachable VERIFIED->REVOKED transition. Reuses
/// reviewed_at/reviewed_by/note for the revocation actor/timestamp/reason,
/// mirroring review_professional_registration's shape. The database
/// condition serializes competing revocations the same way: exactly one
/// request consumes VERIFIED.
pub async fn revoke_professional_registration<'e, E: PgExecutor<'e>>(
    executor: E,
    id: DoctorProfileId,
    reviewer: UserId,
    reason: &str,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE doctor_profiles SET professional_verification_status='REVOKED', professional_verification_reviewed_at=NOW(), professional_verification_reviewed_by=$2, professional_verification_note=$3, updated_at=NOW() WHERE id=$1 AND deleted_at IS NULL AND professional_verification_status='VERIFIED'",
    )
    .bind(id).bind(reviewer).bind(reason).execute(executor).await?;
    Ok(result.rows_affected() == 1)
}

pub async fn list_pending_verifications<'e, E: PgExecutor<'e>>(
    executor: E,
) -> Result<Vec<DoctorProfile>, sqlx::Error> {
    sqlx::query_as::<_, DoctorProfile>(
        "SELECT * FROM doctor_profiles WHERE professional_verification_status='PENDING' AND deleted_at IS NULL ORDER BY created_at, id",
    ).fetch_all(executor).await
}

// ---- Schedule slots ----

pub async fn list_slots_for_doctor<'e, E: PgExecutor<'e>>(
    executor: E,
    doctor_id: DoctorProfileId,
) -> Result<Vec<ScheduleSlot>, sqlx::Error> {
    sqlx::query_as::<_, ScheduleSlot>(
        "SELECT id, doctor_id, start_time, end_time, status, created_at FROM doctor_schedule_slots WHERE doctor_id = $1 AND status = 'AVAILABLE' AND start_time > clock_timestamp() AND EXISTS (SELECT 1 FROM doctor_public_directory d WHERE d.id=$1 AND d.professional_verification_status='VERIFIED') ORDER BY start_time",
    )
    .bind(doctor_id)
    .fetch_all(executor)
    .await
}

pub async fn list_all_slots<'e, E: PgExecutor<'e>>(
    executor: E,
) -> Result<Vec<ScheduleSlot>, sqlx::Error> {
    sqlx::query_as::<_, ScheduleSlot>(
        "SELECT id, doctor_id, start_time, end_time, status, created_at FROM doctor_schedule_slots ORDER BY start_time",
    )
    .fetch_all(executor)
    .await
}

pub async fn find_slot<'e, E: PgExecutor<'e>>(
    executor: E,
    slot_id: Uuid,
) -> Result<Option<ScheduleSlot>, sqlx::Error> {
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
    doctor_id: DoctorProfileId,
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

    // Check the wall clock after acquiring the row lock: a transaction may have
    // waited across the start time, so its transaction-start NOW() is stale.
    let result = sqlx::query("UPDATE doctor_schedule_slots SET status = 'LOCKED_IN_PAYMENT' WHERE id = $1 AND start_time > clock_timestamp()")
        .bind(slot_id)
        .execute(&mut **tx)
        .await?;
    Ok(result.rows_affected() == 1)
}

/// Compare-and-set under PostgreSQL's row lock: races with patient booking
/// have exactly one winner. Explicit ownership complements the existing RLS.
pub async fn set_own_slot_blocked<'e, E: PgExecutor<'e>>(
    executor: E,
    slot_id: Uuid,
    doctor_id: DoctorProfileId,
    blocked: bool,
) -> Result<bool, sqlx::Error> {
    let (from, to) = if blocked {
        (SlotStatus::Available, SlotStatus::Blocked)
    } else {
        (SlotStatus::Blocked, SlotStatus::Available)
    };
    let result = sqlx::query(
        "UPDATE doctor_schedule_slots SET status=$3 WHERE id=$1 AND doctor_id=$2 AND status=$4",
    )
    .bind(slot_id)
    .bind(doctor_id)
    .bind(to)
    .bind(from)
    .execute(executor)
    .await?;
    Ok(result.rows_affected() == 1)
}

pub async fn set_slot_status<'e, E: PgExecutor<'e>>(
    executor: E,
    slot_id: Uuid,
    status: SlotStatus,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE doctor_schedule_slots SET status = $2 WHERE id = $1")
        .bind(slot_id)
        .bind(status)
        .execute(executor)
        .await?;
    Ok(())
}

pub async fn insert_slot<'e, E: PgExecutor<'e>>(
    executor: E,
    slot: &ScheduleSlot,
) -> Result<(), sqlx::Error> {
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
