//! Offline, operator-only first-Admin bootstrap.
//!
//! This module holds the entire business logic for creating the very first
//! Admin account on a production database that has none. It is invoked
//! exclusively by the `admin_bootstrap` binary (`src/bin/admin_bootstrap.rs`)
//! — there is no HTTP route anywhere that reaches this function, and it must
//! stay that way. The binary itself is intentionally a thin shell around this
//! function: `rpassword`'s no-echo terminal prompt opens `/dev/tty` directly
//! and cannot be driven by piped stdin, so it cannot be exercised by an
//! automated test. Putting all real logic here (taking an already-obtained
//! password as a plain `&str`) is what makes it testable at all.
//!
//! Connects and writes exclusively through the `hellodoctor_auth_worker`
//! capability role (the same least-privilege role `doctor_register`/
//! `patient_register` already use for their own first-time account inserts)
//! — never a superuser connection. RLS stays fully in force: `users_auth_bootstrap`
//! (migrations/20261026000001_canonical_user_isolation) already grants this role
//! unconditional insert on `users`, and `GRANT INSERT ON audit_events TO
//! hellodoctor_auth_worker` (migrations/20261013000001_atomic_doctor_onboarding)
//! already covers the audit write below — no new grant or migration is needed.
use crate::crypto;
use crate::domain::ids::UserId;
use crate::domain::models::{User, UserRole};
use crate::domain::validation::{
    normalize_kenya_phone_number, validate_email, validate_password_strength,
};
use crate::error::AppError;
use crate::repository::user_repo;
use crate::services::{AuditLogEntry, AuditService};
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

/// A fixed, arbitrary 64-bit key for the cluster-wide advisory lock below.
/// Only ever used to serialize concurrent bootstrap attempts against each
/// other; it names no table and touches no row.
const BOOTSTRAP_LOCK_KEY: &str = "hellodoctor:admin_bootstrap";

#[derive(Debug, PartialEq, Eq)]
pub struct BootstrapOutcome {
    pub user_id: Uuid,
}

/// Creates the very first Admin account, or fails cleanly without writing
/// anything if one already exists (by role) or if the email/phone is already
/// in use by any account. Concurrent calls against the same database are
/// serialized by a Postgres advisory lock held for the duration of the
/// checking + inserting transaction, so at most one call across any number
/// of concurrent processes can ever succeed.
///
/// `password` must already have been obtained securely by the caller (e.g.
/// a no-echo terminal prompt with confirmation) — this function only
/// validates its strength and hashes it; it never reads from argv, env, or
/// any persisted location, and never logs or returns it.
pub async fn bootstrap_first_admin(
    pool: &PgPool,
    email: &str,
    phone_number: &str,
    password: &str,
) -> Result<BootstrapOutcome, AppError> {
    validate_email(email)?;
    let email = email.trim().to_lowercase();
    let phone_number = normalize_kenya_phone_number(phone_number)?;
    validate_password_strength(password)?;

    let mut tx = pool.begin().await?;

    // Cluster-wide mutex: a second concurrent call blocks here until the
    // first call's transaction commits or rolls back, then proceeds to see
    // whatever that first call left behind (including a fully-committed
    // Admin row it must then refuse to duplicate). Transaction-scoped, so a
    // crashed or disconnected caller releases it automatically — no cleanup
    // step exists or is needed.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(BOOTSTRAP_LOCK_KEY)
        .execute(&mut *tx)
        .await?;

    let admin_already_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM users WHERE role = 'ADMIN' AND deleted_at IS NULL)",
    )
    .fetch_one(&mut *tx)
    .await?;
    if admin_already_exists {
        return Err(AppError::Conflict(
            "An Admin account already exists. This tool only creates the very first Admin; \
             use the ordinary Admin Web session (or an existing Admin's own account \
             management) for anything beyond that."
                .into(),
        ));
    }

    let duplicate_contact: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM users WHERE email = $1 OR phone_number = $2)",
    )
    .bind(&email)
    .bind(&phone_number)
    .fetch_one(&mut *tx)
    .await?;
    if duplicate_contact {
        return Err(AppError::Conflict(
            "An account with this email or phone number already exists.".into(),
        ));
    }

    let user_id = UserId::new();
    let now = Utc::now();
    // Synchronous Argon2id hashing (same function the real login path
    // verifies against) — acceptable to block this one-shot CLI's single
    // task briefly; there is no concurrent request load to protect here.
    let password_hash = crypto::hash_password(password)?;
    let user = User {
        id: user_id,
        phone_number: phone_number.clone(),
        email: Some(email.clone()),
        password_hash: Some(password_hash),
        role: UserRole::Admin,
        preferred_language: "en".into(),
        is_active: true,
        is_verified: true,
        created_at: now,
        updated_at: now,
        deleted_at: None,
    };
    user_repo::insert_user(&mut *tx, &user).await?;

    // Deliberately `actor_id: None` — this action was never performed by an
    // authenticated session, least of all the account being created, so it
    // must never be recorded as if that account had already logged in and
    // acted on its own behalf. `actor_role: UserRole::Admin` here classifies
    // the *tier* of action taken, not an attributed actor; `reason` spells
    // out the real provenance for anyone reading the audit trail.
    AuditService::log_in_tx(
        &mut *tx,
        AuditLogEntry::new(
            None,
            UserRole::Admin,
            "ADMIN_BOOTSTRAP_INITIAL_ACCOUNT_CREATED",
            "USER",
            Some(user_id.into_inner()),
            "SUCCESS",
        )
        .with_reason(Some(
            "Created by an operator running the offline admin_bootstrap CLI directly against \
             the database (VM/container access required). Not performed by, or attributable \
             to, an authenticated session of the account created."
                .into(),
        )),
    )
    .await?;

    tx.commit().await?;
    Ok(BootstrapOutcome {
        user_id: user_id.into_inner(),
    })
}
