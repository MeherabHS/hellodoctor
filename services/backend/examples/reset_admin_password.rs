//! Owner-only offline Admin password-recovery tool. Local development/test use only.
//!
//! Usage: cargo run --example reset_admin_password -- <admin-email>
//!
//! Resets the password_hash for exactly one existing ADMIN user (matched by
//! email), using the project's own Argon2id hashing (crypto::hash_password)
//! and strength policy (domain::validation::validate_password_strength), then
//! revokes that admin's existing sessions so old sessions can't keep working
//! under the old password. Refuses to run outside ENVIRONMENT=development|test,
//! refuses to touch a non-ADMIN account, and never prints the password or hash.
use hellodoctor_backend::{crypto, domain::validation::validate_password_strength};
use sqlx::postgres::PgPoolOptions;
use sqlx::Row;
use uuid::Uuid;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tokio::runtime::Runtime::new()?.block_on(run())
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let environment = std::env::var("ENVIRONMENT").unwrap_or_default();
    if environment != "development" && environment != "test" {
        return Err("Refusing to run: ENVIRONMENT must be 'development' or 'test'.".into());
    }

    let email = std::env::args()
        .nth(1)
        .ok_or("Usage: cargo run --example reset_admin_password -- <admin-email>")?;

    let database_url = std::env::var("DATABASE_URL")?;
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await?;

    let row = sqlx::query(
        "SELECT id, role::text AS role FROM users WHERE email = $1 AND deleted_at IS NULL",
    )
    .bind(&email)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| format!("No user found with email {email}"))?;

    let user_id: Uuid = row.try_get("id")?;
    let role: String = row.try_get("role")?;
    if role != "ADMIN" {
        return Err(format!("User {email} has role {role}, not ADMIN. Refusing to reset.").into());
    }

    let password = rpassword::prompt_password("New admin password: ")?;
    let confirm = rpassword::prompt_password("Confirm new admin password: ")?;
    if password != confirm {
        return Err("Passwords did not match.".into());
    }
    validate_password_strength(&password).map_err(|e| e.to_string())?;

    let hash = crypto::hash_password(&password)?;
    drop(password);
    drop(confirm);

    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE users SET password_hash = $1, updated_at = NOW() WHERE id = $2")
        .bind(&hash)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

    let revoked = sqlx::query(
        "UPDATE auth_sessions SET revoked_at = NOW() WHERE user_id = $1 AND revoked_at IS NULL",
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    println!(
        "Password reset for {email} (user_id={user_id}). Revoked {} existing session(s).",
        revoked.rows_affected()
    );
    Ok(())
}
