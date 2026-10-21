//! Cheap, shared auth target and delivery budgets. Origin admission runs first.
use crate::{crypto, error::AppError, rate_limit, repository::AppState};

/// Proactive target control closes the concurrent failure-jail race. A successful
/// password does not reset this budget. Hashing keeps email addresses out of keys.
pub async fn target_attempt(state: &AppState, realm: &str, account: &str) -> Result<(), AppError> {
    let target = crypto::hash_code(&account.trim().to_lowercase());
    rate_limit::check_and_increment(
        &state.redis,
        &format!("ratelimit:auth-target:{realm}:{target}"),
        5,
        900,
    )
    .await
}

/// Shared across Patient, Doctor and Admin, so changing surface cannot multiply
/// recipient cost. Reserve before challenge persistence and provider dispatch.
pub async fn otp_delivery(state: &AppState, recipient: &str) -> Result<(), AppError> {
    let target = crypto::hash_code(&recipient.trim().to_lowercase());
    rate_limit::check_otp_delivery_limit(&state.redis, &target).await
}

/// Never punish a caller for saturation/dependency failure as a bad credential.
pub fn credential_failure(error: &AppError) -> bool {
    matches!(error, AppError::Unauthorized(_))
}
