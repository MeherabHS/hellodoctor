use crate::domain::ids::{PatientProfileId, UserId};
use crate::domain::models::{Gender, PatientProfile};
use bigdecimal::BigDecimal;
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::PgExecutor;

const PATIENT_PROFILE_COLUMNS: &str = "id, user_id, full_name, date_of_birth, gender, blood_group, weight_kg, emergency_contact_phone, emergency_contact_relation, guardian_patient_id, relationship, created_at, updated_at";

/// See `doctor_repo::ProfilePictureMeta`'s doc comment — same rationale for
/// keeping this a separate, narrowly-queried struct rather than new fields
/// on `PatientProfile` itself.
pub type ProfilePictureMeta = crate::repository::doctor_repo::ProfilePictureMeta;

const PROFILE_PICTURE_META_COLUMNS: &str = "profile_picture_object_key AS object_key, profile_picture_bucket AS bucket, profile_picture_content_hash AS content_hash, profile_picture_mime_type AS mime_type, profile_picture_size_bytes AS size_bytes, profile_picture_updated_at AS updated_at";

pub async fn find_profile_picture<'e, E: PgExecutor<'e>>(
    executor: E,
    id: PatientProfileId,
) -> Result<Option<ProfilePictureMeta>, sqlx::Error> {
    sqlx::query_as::<_, ProfilePictureMeta>(&format!(
        "SELECT {PROFILE_PICTURE_META_COLUMNS} FROM patient_profiles WHERE id=$1"
    ))
    .bind(id)
    .fetch_optional(executor)
    .await
}

/// Overwrites (never `COALESCE`s) every photo column — see
/// `doctor_repo::update_profile_picture`'s identical rationale.
#[allow(clippy::too_many_arguments)]
pub async fn update_profile_picture<'e, E: PgExecutor<'e>>(
    executor: E,
    id: PatientProfileId,
    object_key: Option<&str>,
    bucket: Option<&str>,
    content_hash: Option<&str>,
    mime_type: Option<&str>,
    size_bytes: Option<i32>,
    updated_at: DateTime<Utc>,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE patient_profiles SET profile_picture_object_key=$2, profile_picture_bucket=$3, \
         profile_picture_content_hash=$4, profile_picture_mime_type=$5, \
         profile_picture_size_bytes=$6, profile_picture_updated_at=$7 \
         WHERE id=$1",
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

pub async fn list<'e, E: PgExecutor<'e>>(executor: E) -> Result<Vec<PatientProfile>, sqlx::Error> {
    sqlx::query_as::<_, PatientProfile>(&format!(
        "SELECT {PATIENT_PROFILE_COLUMNS} FROM patient_profiles ORDER BY created_at DESC"
    ))
    .fetch_all(executor)
    .await
}

pub async fn find_by_id<'e, E: PgExecutor<'e>>(
    executor: E,
    id: PatientProfileId,
) -> Result<Option<PatientProfile>, sqlx::Error> {
    sqlx::query_as::<_, PatientProfile>(&format!(
        "SELECT {PATIENT_PROFILE_COLUMNS} FROM patient_profiles WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(executor)
    .await
}

/// Scoped alternative to `list` — see appointment_repo::find_by_ids's doc
/// comment for why an unconditional whole-table fetch is the wrong default
/// for a handler that only needs a small, known set of patients.
pub async fn find_by_ids<'e, E: PgExecutor<'e>>(
    executor: E,
    ids: &[PatientProfileId],
) -> Result<Vec<PatientProfile>, sqlx::Error> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    sqlx::query_as::<_, PatientProfile>(&format!(
        "SELECT {PATIENT_PROFILE_COLUMNS} FROM patient_profiles WHERE id = ANY($1)"
    ))
    .bind(ids)
    .fetch_all(executor)
    .await
}

pub async fn find_by_user_id<'e, E: PgExecutor<'e>>(
    executor: E,
    user_id: UserId,
) -> Result<Option<PatientProfile>, sqlx::Error> {
    sqlx::query_as::<_, PatientProfile>(&format!(
        "SELECT {PATIENT_PROFILE_COLUMNS} FROM patient_profiles WHERE user_id = $1"
    ))
    .bind(user_id)
    .fetch_optional(executor)
    .await
}

/// Existence-only check — deliberately queries only `user_id` (present since
/// the very first migration, `20261001000001_initial_schema`), never the
/// full column set. `auth_service::verify_otp_and_login`'s existing-user
/// branch only ever needs a yes/no answer here (to decide whether to insert
/// the first-login placeholder row), so this stays correct regardless of
/// which later columns a future migration adds to `patient_profiles` —
/// unlike `find_by_user_id`, which intentionally reads the full current
/// shape and is right to break loudly if that shape and the running
/// binary's expectations ever disagree.
pub async fn exists_for_user_id<'e, E: PgExecutor<'e>>(
    executor: E,
    user_id: UserId,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM patient_profiles WHERE user_id = $1)")
        .bind(user_id)
        .fetch_one(executor)
        .await
}

/// A guardian's own self profile plus every dependent profile they guardian
/// — the "family of 10, one device" profile-switcher list
/// (`GET /patients/me/profiles`). `self_id` is the caller's own already-
/// resolved self-profile id; RLS independently re-enforces this same scope,
/// this query is just the efficient shape (one role, ordered self-first).
pub async fn list_for_guardian<'e, E: PgExecutor<'e>>(
    executor: E,
    self_id: PatientProfileId,
) -> Result<Vec<PatientProfile>, sqlx::Error> {
    sqlx::query_as::<_, PatientProfile>(&format!(
        "SELECT {PATIENT_PROFILE_COLUMNS} FROM patient_profiles \
         WHERE id = $1 OR guardian_patient_id = $1 \
         ORDER BY (guardian_patient_id IS NOT NULL), created_at"
    ))
    .bind(self_id)
    .fetch_all(executor)
    .await
}

pub async fn insert<'e, E: PgExecutor<'e>>(
    executor: E,
    p: &PatientProfile,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO patient_profiles (id, user_id, full_name, date_of_birth, gender, blood_group, weight_kg, emergency_contact_phone, emergency_contact_relation, guardian_patient_id, relationship, created_at, updated_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)",
    )
    .bind(p.id)
    .bind(p.user_id)
    .bind(&p.full_name)
    .bind(p.date_of_birth)
    .bind(p.gender)
    .bind(&p.blood_group)
    .bind(&p.weight_kg)
    .bind(&p.emergency_contact_phone)
    .bind(&p.emergency_contact_relation)
    .bind(p.guardian_patient_id)
    .bind(&p.relationship)
    .bind(p.created_at)
    .bind(p.updated_at)
    .execute(executor)
    .await?;
    Ok(())
}

/// Partial update — every field is `Option`; `None` leaves the existing
/// column value unchanged (`COALESCE`), matching
/// `doctor_handlers::update_own_profile`'s single-field precedent, extended
/// to this profile's full set of patient-editable columns. Returns the
/// number of rows actually updated (0 means no profile with this `id`
/// exists), the same "let the caller turn a non-1 count into 404" contract
/// `doctor_repo`'s own profile update follows.
#[allow(clippy::too_many_arguments)]
pub async fn update<'e, E: PgExecutor<'e>>(
    executor: E,
    id: PatientProfileId,
    full_name: Option<String>,
    date_of_birth: Option<NaiveDate>,
    gender: Option<Gender>,
    blood_group: Option<String>,
    weight_kg: Option<BigDecimal>,
    emergency_contact_phone: Option<String>,
    emergency_contact_relation: Option<String>,
    relationship: Option<String>,
    updated_at: DateTime<Utc>,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE patient_profiles SET \
         full_name = COALESCE($2, full_name), \
         date_of_birth = COALESCE($3, date_of_birth), \
         gender = COALESCE($4, gender), \
         blood_group = COALESCE($5, blood_group), \
         weight_kg = COALESCE($6, weight_kg), \
         emergency_contact_phone = COALESCE($7, emergency_contact_phone), \
         emergency_contact_relation = COALESCE($8, emergency_contact_relation), \
         relationship = COALESCE($9, relationship), \
         updated_at = $10 \
         WHERE id = $1",
    )
    .bind(id)
    .bind(full_name)
    .bind(date_of_birth)
    .bind(gender)
    .bind(blood_group)
    .bind(weight_kg)
    .bind(emergency_contact_phone)
    .bind(emergency_contact_relation)
    .bind(relationship)
    .bind(updated_at)
    .execute(executor)
    .await?;
    Ok(result.rows_affected())
}
