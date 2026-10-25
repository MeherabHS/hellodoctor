//! The one, obvious seam where a real payment-provider API call belongs —
//! "the heart" `docs/SECURITY_AUDIT_RUN1_REMEDIATION.md`-adjacent work keeps
//! referring to. Before this module existed, a patient's or doctor's saved
//! payment method went straight from format validation to a database
//! INSERT with no step in between that so much as checked the platform
//! itself has a payment provider configured at all, let alone actually
//! asked that provider "is this a real, usable destination?" — confirmed
//! directly by reading `patient_handlers::save_payment_method` and
//! `wallet_handlers::bind_payout_destination` as they stood before this
//! file, not assumed.
//!
//! This does two things, and only two:
//! 1. Requires an active `platform_payment_methods` row for the right
//!    provider family to exist before any patient/doctor method can be
//!    saved at all — you cannot validate a real M-Pesa number without the
//!    platform's own M-Pesa merchant credentials configured first. This is
//!    enforced today, with real consequences (save fails otherwise), not a
//!    TODO.
//! 2. Calls `verify_with_provider`, which is the single, obvious seam a
//!    real driver plugs into later — today it performs no real network
//!    call (there is no real credential to call with yet) and returns a
//!    deliberately-named `AWAITING_PROVIDER_INTEGRATION` result, never
//!    `VERIFIED`, so nothing downstream can mistake a simulated pass for a
//!    real one.

use crate::domain::models::{Gateway, PayoutMethodType, PlatformPaymentMethod};
use crate::error::AppError;
use crate::repository::payment_methods_repo;
use sqlx::PgExecutor;

/// The maximum length `doctor_payout_destinations.verification_status` and
/// `patient_payment_methods`/this module's own status strings must respect
/// (`VARCHAR(30)` — the exact limit a prior version of this feature
/// overran and had to shorten to fit).
pub const VERIFICATION_STATUS_AWAITING_PROVIDER: &str = "AWAITING_PROVIDER_INTEGRATION";

fn provider_family_for_gateway(gateway: Gateway) -> Option<&'static str> {
    match gateway {
        Gateway::Mpesa => Some("MPESA"),
        Gateway::Card => Some("CARD"),
        Gateway::Bkash | Gateway::Nagad | Gateway::BankTransfer => None,
    }
}

fn provider_family_for_payout_method(method: PayoutMethodType) -> Option<&'static str> {
    match method {
        PayoutMethodType::Mpesa => Some("MPESA"),
        PayoutMethodType::BankCard => Some("CARD"),
        PayoutMethodType::Bkash | PayoutMethodType::BankTransfer => None,
    }
}

/// Finds the platform's own active, primary merchant configuration for the
/// given provider family (matched by `provider_code` prefix — e.g.
/// `MPESA_PAYBILL`/`MPESA_TILL` both match family `"MPESA"`). Returns
/// `None` when nothing is configured yet, which callers must treat as a
/// hard stop, not a warning.
async fn find_active_platform_method<'e, E: PgExecutor<'e>>(
    executor: E,
    family: &str,
) -> Result<Option<PlatformPaymentMethod>, AppError> {
    // `list_platform_payment_methods` is already ordered `is_primary DESC,
    // created_at DESC`, so the first match in that pre-sorted order is the
    // right one to take — not `.max_by_key(|m| m.is_primary)`, which (being a
    // boolean key) would instead return the *last* element among ties,
    // silently preferring a non-primary/oldest row whenever a family (e.g.
    // "MPESA" matching both MPESA_PAYBILL and MPESA_TILL) has more than one
    // configured method.
    let methods = payment_methods_repo::list_platform_payment_methods(executor).await?;
    Ok(methods
        .into_iter()
        .find(|m| m.provider_code.starts_with(family)))
}

pub struct VerificationOutcome {
    pub verification_status: &'static str,
    /// The platform merchant method this verification was performed
    /// against — recorded so a future real integration can see which
    /// credential set was used, even though no real call happened yet.
    pub platform_payment_method_id: uuid::Uuid,
}

/// The actual seam. Takes the already-validated, already-encrypted
/// identifier (never a raw secret) and the platform's resolved credential
/// row, and would call the real provider API here once one exists. Until
/// then: no network call, no claim of real verification, a single
/// `tracing::debug!` marking exactly where that call belongs.
fn verify_with_provider(
    platform_method: &PlatformPaymentMethod,
    _encrypted_identifier: &str,
) -> VerificationOutcome {
    tracing::debug!(
        provider_code = %platform_method.provider_code,
        platform_method_id = %platform_method.id,
        "payment method verification: no real provider driver wired up yet — \
         this is the exact call site a real integration replaces"
    );
    VerificationOutcome {
        verification_status: VERIFICATION_STATUS_AWAITING_PROVIDER,
        platform_payment_method_id: platform_method.id,
    }
}

/// Entry point for a patient saving a payment method. Returns
/// `AppError::Conflict` (not silently succeeding) when the platform has no
/// active merchant configuration for this gateway's provider family yet.
pub async fn verify_patient_payment_method<'e, E: PgExecutor<'e>>(
    executor: E,
    gateway: Gateway,
) -> Result<VerificationOutcome, AppError> {
    let family = provider_family_for_gateway(gateway).ok_or_else(|| {
        AppError::Validation(
            "This payment method is not available for the current Kenya deployment.".into(),
        )
    })?;
    let platform_method = find_active_platform_method(executor, family)
        .await?
        .ok_or_else(|| {
            AppError::Conflict(format!(
                "The platform has not configured a {family} payment provider yet — an administrator must add one under Payment Settings before this method can be saved."
            ))
        })?;
    Ok(verify_with_provider(&platform_method, ""))
}

/// Entry point for a doctor binding a payout destination. Same requirement,
/// same reasoning, the payout-side mirror of
/// `verify_patient_payment_method`.
pub async fn verify_doctor_payout_destination<'e, E: PgExecutor<'e>>(
    executor: E,
    method_type: PayoutMethodType,
) -> Result<VerificationOutcome, AppError> {
    let family = provider_family_for_payout_method(method_type).ok_or_else(|| {
        AppError::Validation(
            "This payout method is not available for the current Kenya deployment.".into(),
        )
    })?;
    let platform_method = find_active_platform_method(executor, family)
        .await?
        .ok_or_else(|| {
            AppError::Conflict(format!(
                "The platform has not configured a {family} payment provider yet — an administrator must add one under Payment Settings before a doctor can bind a payout destination."
            ))
        })?;
    Ok(verify_with_provider(&platform_method, ""))
}
