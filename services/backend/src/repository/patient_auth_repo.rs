use crate::domain::ids::UserId;
use crate::domain::models::Gender;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct PatientRegistrationProfile {
    pub full_name: String,
    pub date_of_birth: NaiveDate,
    pub gender: Gender,
}
use sqlx::PgExecutor;

#[derive(sqlx::FromRow)]
pub struct PatientAuthChallenge {
    pub registration_profile: Option<serde_json::Value>,
    pub token: String,
    pub email: String,
    pub phone_number: Option<String>,
    pub password_hash: Option<String>,
    pub user_id: Option<UserId>,
    pub flow: String,
    pub code_hash: String,
    pub attempts: i16,
    pub expires_at: DateTime<Utc>,
}

#[allow(clippy::too_many_arguments)]
pub async fn insert<'e, E: PgExecutor<'e>>(
    executor: E,
    token: &str,
    email: &str,
    phone_number: Option<&str>,
    password_hash: Option<&str>,
    user_id: Option<UserId>,
    flow: &str,
    code_hash: &str,
    expires_at: DateTime<Utc>,
    profile: Option<&PatientRegistrationProfile>,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO patient_auth_challenges (token,email,phone_number,password_hash,user_id,flow,code_hash,expires_at,registration_profile) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)")
        .bind(token).bind(email).bind(phone_number).bind(password_hash).bind(user_id)
        .bind(flow).bind(code_hash).bind(expires_at).bind(profile.map(sqlx::types::Json)).execute(executor).await?;
    Ok(())
}

pub async fn find_for_update(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    token: &str,
) -> Result<Option<PatientAuthChallenge>, sqlx::Error> {
    sqlx::query_as("SELECT registration_profile,token,email,phone_number,password_hash,user_id,flow,code_hash,attempts,expires_at FROM patient_auth_challenges WHERE token=$1 FOR UPDATE")
        .bind(token).fetch_optional(&mut **tx).await
}

pub async fn increment_attempts(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    token: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE patient_auth_challenges SET attempts=attempts+1 WHERE token=$1")
        .bind(token)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub async fn delete<'e, E: PgExecutor<'e>>(executor: E, token: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM patient_auth_challenges WHERE token=$1")
        .bind(token)
        .execute(executor)
        .await?;
    Ok(())
}
