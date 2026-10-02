use crate::config::AppConfig;
use sqlx::PgPool;
use std::sync::Arc;

pub mod appointment_repo;
pub mod audit_repo;
pub mod chat_repo;
pub mod consultation_repo;
pub mod doctor_repo;
pub mod grievance_repo;
pub mod idempotency_repo;
pub mod otp_repo;
pub mod patient_repo;
pub mod prescription_repo;
pub mod session_repo;
pub mod transaction_repo;
pub mod user_repo;
pub mod wallet_repo;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Arc<AppConfig>,
}

impl AppState {
    pub fn new(db: PgPool, config: Arc<AppConfig>) -> Self {
        Self { db, config }
    }
}
