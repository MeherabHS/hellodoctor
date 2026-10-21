//! Patient-saved payment methods and the platform's own merchant/collection
//! configuration. Mirrors `wallet_repo`'s `doctor_payout_destinations`
//! pattern (multiple rows per owner, exactly one active primary enforced by
//! a partial unique index) rather than inventing a new shape — see
//! `migrations/20261108000001_patient_payment_methods.up.sql` and
//! `20261108000002_platform_payment_methods.up.sql` for the schema this
//! queries against.

use crate::domain::ids::PatientProfileId;
use crate::domain::ids::UserId;
use crate::domain::models::{Gateway, PatientPaymentMethod, PlatformPaymentMethod};
use sqlx::PgExecutor;
use uuid::Uuid;

pub async fn list_patient_payment_methods<'e, E: PgExecutor<'e>>(
    executor: E,
    patient_id: PatientProfileId,
) -> Result<Vec<PatientPaymentMethod>, sqlx::Error> {
    sqlx::query_as::<_, PatientPaymentMethod>(
        "SELECT id, patient_id, gateway, provider_code, provider_reference, masked_destination, binding_status, verification_status, platform_payment_method_id, is_primary, created_at, updated_at \
         FROM patient_payment_methods WHERE patient_id = $1 AND binding_status = 'ACTIVE' ORDER BY is_primary DESC, created_at DESC",
    )
    .bind(patient_id)
    .fetch_all(executor)
    .await
}

pub async fn get_primary_patient_payment_method<'e, E: PgExecutor<'e>>(
    executor: E,
    patient_id: PatientProfileId,
) -> Result<Option<PatientPaymentMethod>, sqlx::Error> {
    sqlx::query_as::<_, PatientPaymentMethod>(
        "SELECT id, patient_id, gateway, provider_code, provider_reference, masked_destination, binding_status, verification_status, platform_payment_method_id, is_primary, created_at, updated_at \
         FROM patient_payment_methods WHERE patient_id = $1 AND is_primary = TRUE AND binding_status = 'ACTIVE' LIMIT 1",
    )
    .bind(patient_id)
    .fetch_optional(executor)
    .await
}

pub async fn find_patient_payment_method<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
) -> Result<Option<PatientPaymentMethod>, sqlx::Error> {
    sqlx::query_as::<_, PatientPaymentMethod>(
        "SELECT id, patient_id, gateway, provider_code, provider_reference, masked_destination, binding_status, verification_status, platform_payment_method_id, is_primary, created_at, updated_at \
         FROM patient_payment_methods WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(executor)
    .await
}

/// Inserts a new saved payment method. If `make_primary` is true (always
/// true for a patient's first method — see the service layer — or whenever
/// the caller explicitly asks), demotes any existing primary first within
/// the same transaction so the partial unique index
/// (`uq_patient_primary_payment_method`) never sees two primaries at once.
#[allow(clippy::too_many_arguments)]
pub async fn insert_patient_payment_method(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    patient_id: PatientProfileId,
    gateway: Gateway,
    provider_code: &str,
    provider_reference: &str,
    masked_destination: &str,
    verification_status: &str,
    platform_payment_method_id: Uuid,
    make_primary: bool,
) -> Result<PatientPaymentMethod, sqlx::Error> {
    // Serializes concurrent saves for the same patient, matching the exact
    // locking discipline `create_dev_payout_destination` already
    // established for doctors — an unreferenced modifying CTE does not
    // guarantee an UPDATE runs before an INSERT checks a partial unique
    // index.
    sqlx::query("SELECT id FROM patient_profiles WHERE id=$1 FOR UPDATE")
        .bind(patient_id)
        .execute(&mut **tx)
        .await?;
    if make_primary {
        sqlx::query(
            "UPDATE patient_payment_methods SET is_primary=FALSE, updated_at=NOW() WHERE patient_id=$1",
        )
        .bind(patient_id)
        .execute(&mut **tx)
        .await?;
    }
    sqlx::query_as::<_, PatientPaymentMethod>(
        "INSERT INTO patient_payment_methods (patient_id, gateway, provider_code, provider_reference, masked_destination, binding_status, verification_status, platform_payment_method_id, is_primary) \
         VALUES ($1, $2, $3, $4, $5, 'ACTIVE', $6, $7, $8) \
         RETURNING id, patient_id, gateway, provider_code, provider_reference, masked_destination, binding_status, verification_status, platform_payment_method_id, is_primary, created_at, updated_at",
    )
    .bind(patient_id)
    .bind(gateway)
    .bind(provider_code)
    .bind(provider_reference)
    .bind(masked_destination)
    .bind(verification_status)
    .bind(platform_payment_method_id)
    .bind(make_primary)
    .fetch_one(&mut **tx)
    .await
}

pub async fn set_primary_patient_payment_method(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    patient_id: PatientProfileId,
    method_id: Uuid,
) -> Result<bool, sqlx::Error> {
    sqlx::query("SELECT id FROM patient_profiles WHERE id=$1 FOR UPDATE")
        .bind(patient_id)
        .execute(&mut **tx)
        .await?;
    sqlx::query(
        "UPDATE patient_payment_methods SET is_primary=FALSE, updated_at=NOW() WHERE patient_id=$1",
    )
    .bind(patient_id)
    .execute(&mut **tx)
    .await?;
    let result = sqlx::query(
        "UPDATE patient_payment_methods SET is_primary=TRUE, updated_at=NOW() \
         WHERE id=$1 AND patient_id=$2 AND binding_status='ACTIVE'",
    )
    .bind(method_id)
    .bind(patient_id)
    .execute(&mut **tx)
    .await?;
    Ok(result.rows_affected() == 1)
}

pub async fn deactivate_patient_payment_method<'e, E: PgExecutor<'e>>(
    executor: E,
    patient_id: PatientProfileId,
    method_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE patient_payment_methods SET binding_status='REMOVED', is_primary=FALSE, updated_at=NOW() \
         WHERE id=$1 AND patient_id=$2 AND binding_status='ACTIVE'",
    )
    .bind(method_id)
    .bind(patient_id)
    .execute(executor)
    .await?;
    Ok(result.rows_affected() == 1)
}

// ---------------------------------------------------------------------
// Platform (admin) merchant payment methods.
// ---------------------------------------------------------------------

pub async fn list_platform_payment_methods<'e, E: PgExecutor<'e>>(
    executor: E,
) -> Result<Vec<PlatformPaymentMethod>, sqlx::Error> {
    sqlx::query_as::<_, PlatformPaymentMethod>(
        "SELECT id, provider_code, label, currency, masked_display, credentials_encrypted, is_primary, is_active, created_by, created_at, updated_at \
         FROM platform_payment_methods WHERE is_active = TRUE ORDER BY is_primary DESC, created_at DESC",
    )
    .fetch_all(executor)
    .await
}

pub async fn find_platform_payment_method<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
) -> Result<Option<PlatformPaymentMethod>, sqlx::Error> {
    sqlx::query_as::<_, PlatformPaymentMethod>(
        "SELECT id, provider_code, label, currency, masked_display, credentials_encrypted, is_primary, is_active, created_by, created_at, updated_at \
         FROM platform_payment_methods WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(executor)
    .await
}

/// Inserts a new platform merchant method. "Primary" is scoped per
/// `provider_code` (see the migration's partial unique index) — adding a
/// second M-Pesa config as primary demotes the first M-Pesa config, but
/// never touches a Card config's own primary, since the two are not
/// interchangeable alternatives of each other.
pub async fn insert_platform_payment_method(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    provider_code: &str,
    label: &str,
    masked_display: &str,
    credentials_encrypted: &str,
    created_by: UserId,
    make_primary: bool,
) -> Result<PlatformPaymentMethod, sqlx::Error> {
    if make_primary {
        sqlx::query(
            "UPDATE platform_payment_methods SET is_primary=FALSE, updated_at=NOW() WHERE provider_code=$1",
        )
        .bind(provider_code)
        .execute(&mut **tx)
        .await?;
    }
    sqlx::query_as::<_, PlatformPaymentMethod>(
        "INSERT INTO platform_payment_methods (provider_code, label, masked_display, credentials_encrypted, created_by, is_primary, is_active) \
         VALUES ($1, $2, $3, $4, $5, $6, TRUE) \
         RETURNING id, provider_code, label, currency, masked_display, credentials_encrypted, is_primary, is_active, created_by, created_at, updated_at",
    )
    .bind(provider_code)
    .bind(label)
    .bind(masked_display)
    .bind(credentials_encrypted)
    .bind(created_by)
    .bind(make_primary)
    .fetch_one(&mut **tx)
    .await
}

pub async fn set_primary_platform_payment_method(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    id: Uuid,
    provider_code: &str,
) -> Result<bool, sqlx::Error> {
    sqlx::query(
        "UPDATE platform_payment_methods SET is_primary=FALSE, updated_at=NOW() WHERE provider_code=$1",
    )
    .bind(provider_code)
    .execute(&mut **tx)
    .await?;
    let result = sqlx::query(
        "UPDATE platform_payment_methods SET is_primary=TRUE, updated_at=NOW() WHERE id=$1 AND is_active=TRUE",
    )
    .bind(id)
    .execute(&mut **tx)
    .await?;
    Ok(result.rows_affected() == 1)
}

pub async fn deactivate_platform_payment_method<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE platform_payment_methods SET is_active=FALSE, is_primary=FALSE, updated_at=NOW() WHERE id=$1 AND is_active=TRUE",
    )
    .bind(id)
    .execute(executor)
    .await?;
    Ok(result.rows_affected() == 1)
}
