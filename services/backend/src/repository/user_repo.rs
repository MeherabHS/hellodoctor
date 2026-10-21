use crate::domain::ids::UserId;
use crate::domain::models::{User, UserRole};
use chrono::Utc;
use sqlx::PgExecutor;
use uuid::Uuid;

/// Data-rights erasure/anonymization of the login row: scrubs every
/// personally-identifying column and deactivates the account outright.
/// `hellodoctor_api` has no grant on `users` at all beyond a column-scoped
/// `SELECT` — only `hellodoctor_auth_worker` has `UPDATE`
/// (migration 20261003000001), so every caller must run this against
/// `DbRole::AuthWorker`, never the default `Api` pool.
pub async fn anonymize_and_deactivate<'e, E: PgExecutor<'e>>(
    executor: E,
    id: UserId,
    anonymized_phone: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE users SET phone_number=$2, email=NULL, password_hash=NULL, \
         is_active=FALSE, deleted_at=NOW(), updated_at=NOW() WHERE id=$1",
    )
    .bind(id)
    .bind(anonymized_phone)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn find_by_phone<'e, E: PgExecutor<'e>>(
    executor: E,
    phone_number: &str,
) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as::<_, User>(
        "SELECT id, phone_number, email, password_hash, role, preferred_language, is_active, is_verified, created_at, updated_at, deleted_at \
         FROM users WHERE phone_number = $1 AND deleted_at IS NULL",
    )
    .bind(phone_number)
    .fetch_optional(executor)
    .await
}

pub async fn find_by_id<'e, E: PgExecutor<'e>>(
    executor: E,
    id: UserId,
) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as::<_, User>(
        "SELECT id, phone_number, email, password_hash, role, preferred_language, is_active, is_verified, created_at, updated_at, deleted_at \
         FROM users WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_optional(executor)
    .await
}

pub async fn find_by_email<'e, E: PgExecutor<'e>>(
    executor: E,
    email: &str,
) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as::<_, User>(
        "SELECT id, phone_number, email, password_hash, role, preferred_language, is_active, is_verified, created_at, updated_at, deleted_at \
         FROM users WHERE email = $1 AND deleted_at IS NULL",
    )
    .bind(email)
    .fetch_optional(executor)
    .await
}

pub async fn create_patient_user<'e, E: PgExecutor<'e>>(
    executor: E,
    phone_number: &str,
) -> Result<User, sqlx::Error> {
    let now = Utc::now();
    sqlx::query_as::<_, User>(
        "INSERT INTO users (id, phone_number, email, password_hash, role, preferred_language, is_active, is_verified, created_at, updated_at, deleted_at) \
         VALUES ($1, $2, NULL, NULL, $3, 'en', TRUE, TRUE, $4, $4, NULL) \
         RETURNING id, phone_number, email, password_hash, role, preferred_language, is_active, is_verified, created_at, updated_at, deleted_at",
    )
    .bind(Uuid::new_v4())
    .bind(phone_number)
    .bind(UserRole::Patient)
    .bind(now)
    .fetch_one(executor)
    .await
}

pub async fn create_patient_user_with_credentials<'e, E: PgExecutor<'e>>(
    executor: E,
    email: &str,
    phone_number: &str,
    password_hash: &str,
) -> Result<User, sqlx::Error> {
    let now = Utc::now();
    sqlx::query_as::<_, User>(
        "INSERT INTO users (id, phone_number, email, password_hash, role, preferred_language, is_active, is_verified, created_at, updated_at, deleted_at) \
         VALUES ($1, $2, $3, $4, $5, 'en', TRUE, TRUE, $6, $6, NULL) \
         RETURNING id, phone_number, email, password_hash, role, preferred_language, is_active, is_verified, created_at, updated_at, deleted_at",
    )
    .bind(Uuid::new_v4())
    .bind(phone_number)
    .bind(email)
    .bind(password_hash)
    .bind(UserRole::Patient)
    .bind(now)
    .fetch_one(executor)
    .await
}

/// Total active, non-deleted user accounts — the metric `MAX_ACTIVE_USERS`
/// is checked against before admitting a brand-new registration (see
/// `auth_service::verify_otp_and_login`'s patient self-registration branch
/// and `admin_handlers::create_doctor`). Existing users/sessions are never
/// affected by this count — it only gates new activation.
pub async fn count_active<'e, E: PgExecutor<'e>>(executor: E) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT count(*) FROM users WHERE is_active = TRUE AND deleted_at IS NULL")
        .fetch_one(executor)
        .await
}

pub async fn insert_user<'e, E: PgExecutor<'e>>(
    executor: E,
    user: &User,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO users (id, phone_number, email, password_hash, role, preferred_language, is_active, is_verified, created_at, updated_at, deleted_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
    )
    .bind(user.id)
    .bind(&user.phone_number)
    .bind(&user.email)
    .bind(&user.password_hash)
    .bind(user.role)
    .bind(&user.preferred_language)
    .bind(user.is_active)
    .bind(user.is_verified)
    .bind(user.created_at)
    .bind(user.updated_at)
    .bind(user.deleted_at)
    .execute(executor)
    .await?;
    Ok(())
}
