use crate::domain::models::PatientProfile;
use sqlx::PgExecutor;
use uuid::Uuid;

pub async fn list<'e, E: PgExecutor<'e>>(executor: E) -> Result<Vec<PatientProfile>, sqlx::Error> {
    sqlx::query_as::<_, PatientProfile>(
        "SELECT id, user_id, full_name, date_of_birth, gender, blood_group, weight_kg, emergency_contact_phone, emergency_contact_relation, created_at, updated_at \
         FROM patient_profiles ORDER BY created_at DESC",
    )
    .fetch_all(executor)
    .await
}

pub async fn find_by_id<'e, E: PgExecutor<'e>>(executor: E, id: Uuid) -> Result<Option<PatientProfile>, sqlx::Error> {
    sqlx::query_as::<_, PatientProfile>(
        "SELECT id, user_id, full_name, date_of_birth, gender, blood_group, weight_kg, emergency_contact_phone, emergency_contact_relation, created_at, updated_at \
         FROM patient_profiles WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(executor)
    .await
}

pub async fn find_by_user_id<'e, E: PgExecutor<'e>>(executor: E, user_id: Uuid) -> Result<Option<PatientProfile>, sqlx::Error> {
    sqlx::query_as::<_, PatientProfile>(
        "SELECT id, user_id, full_name, date_of_birth, gender, blood_group, weight_kg, emergency_contact_phone, emergency_contact_relation, created_at, updated_at \
         FROM patient_profiles WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(executor)
    .await
}

pub async fn insert<'e, E: PgExecutor<'e>>(executor: E, p: &PatientProfile) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO patient_profiles (id, user_id, full_name, date_of_birth, gender, blood_group, weight_kg, emergency_contact_phone, emergency_contact_relation, created_at, updated_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)",
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
    .bind(p.created_at)
    .bind(p.updated_at)
    .execute(executor)
    .await?;
    Ok(())
}
