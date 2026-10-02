use crate::domain::models::{User, UserRole};
use chrono::Utc;
use sqlx::PgExecutor;
use uuid::Uuid;

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
    id: Uuid,
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

pub async fn insert_user<'e, E: PgExecutor<'e>>(executor: E, user: &User) -> Result<(), sqlx::Error> {
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
