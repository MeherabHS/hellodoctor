pub mod article_repo;
use crate::auth_context::AuthContext;
use crate::config::AppConfig;
use crate::error::AppError;
use sqlx::{PgPool, Postgres, Transaction};
use std::sync::Arc;

pub mod appointment_repo;
pub mod audit_repo;
pub mod chat_repo;
pub mod consultation_repo;
pub mod doctor_repo;
pub mod grievance_repo;
pub mod idempotency_repo;
pub mod legal_repo;
pub mod notification_repo;
pub mod otp_repo;
pub mod patient_auth_repo;
pub mod patient_repo;
pub mod payment_event_repo;
pub mod payment_methods_repo;
pub mod prescription_repo;
pub mod session_repo;
pub mod system_error_repo;
pub mod transaction_repo;
pub mod user_repo;
pub mod wallet_repo;

/// Which of the five least-privilege capability-role pools a transaction should
/// run against. Chosen by the service/handler code based on *what operation is
/// being performed*, never by the caller's JWT role — an ADMIN's request
/// does not get to pick the finance-worker connection just by asserting that
/// role; the service function for that specific lifecycle transition does.
/// See migrations/20261003000001_rls_hardening for the exact per-role grants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbRole {
    /// Ordinary reads and patient/doctor-authored content writes.
    Api,
    /// auth_sessions / mfa_credentials / OTP / login bootstrap, including the
    /// one patient self-registration insert at the end of first-time OTP verify.
    AuthWorker,
    /// Booking + payment-state transitions (appointments, slots, transactions).
    PaymentWorker,
    /// Background lifecycle jobs: slot/chat/consultation-session expiry.
    SystemWorker,
    /// Grievance adjudication, wallet settlement, disbursement batches.
    FinanceWorker,
}

#[derive(Clone)]
pub struct AppState {
    /// Ordinary request-serving pool, connected as `hellodoctor_api`.
    pub db: PgPool,
    pub auth_db: PgPool,
    pub payment_db: PgPool,
    pub system_db: PgPool,
    pub finance_db: PgPool,
    pub redis: deadpool_redis::Pool,
    pub config: Arc<AppConfig>,
    pub object_storage: Option<Arc<dyn object_store::ObjectStore>>,
    pub admission: Arc<crate::admission::Admission>,
    pub web_auth: Arc<crate::api::middleware::web_auth::WebAuthGate>,
    pub notification_transport:
        Arc<dyn crate::services::notification_transport::NotificationTransport>,
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        db: PgPool,
        auth_db: PgPool,
        payment_db: PgPool,
        system_db: PgPool,
        finance_db: PgPool,
        redis: deadpool_redis::Pool,
        config: Arc<AppConfig>,
    ) -> Self {
        // Local development does not install or start a storage service.
        // With no configured external store, document operations fail explicitly.
        let storage_configured = !config.storage_endpoint.is_empty()
            && !config.storage_access_key.is_empty()
            && !config.storage_secret_key.is_empty()
            && !config.storage_encryption_key_b64.is_empty();
        let object_storage: Option<Arc<dyn object_store::ObjectStore>> =
            storage_configured.then(|| {
                Arc::new(
                    object_store::aws::AmazonS3Builder::new()
                        .with_bucket_name(&config.storage_bucket)
                        .with_region("us-east-1")
                        .with_endpoint(&config.storage_endpoint)
                        .with_access_key_id(&config.storage_access_key)
                        .with_secret_access_key(&config.storage_secret_key)
                        .with_allow_http(config.environment == "development")
                        .with_client_options(
                            object_store::ClientOptions::new()
                                .with_allow_http(config.environment == "development")
                                .with_timeout(std::time::Duration::from_secs(6)),
                        )
                        .build()
                        .expect("Invalid private object-storage configuration"),
                ) as Arc<dyn object_store::ObjectStore>
            });
        // Five independent per-role pools (see `DbRole`) each hold
        // `db_pool_max_connections_per_role` connections, so the combined
        // real DB capacity behind admission is five times that value, not
        // one role's share of it. Passing just the per-role number here
        // under-admits concurrent traffic ~5x relative to what the pools
        // can actually serve.
        let admission = Arc::new(crate::admission::Admission::new(
            config.db_pool_max_connections_per_role.saturating_mul(5),
        ));
        let web_auth = Arc::new(crate::api::middleware::web_auth::WebAuthGate::new(
            !config.is_development_or_test(),
        ));
        let notification_transport: Arc<
            dyn crate::services::notification_transport::NotificationTransport,
        > = match crate::services::notification_transport::FcmTransport::from_config(&config) {
            Ok(Some(transport)) => Arc::new(transport),
            Ok(None) => Arc::new(crate::services::notification_transport::NoopTransport),
            Err(error) => panic!("FATAL: invalid FCM configuration: {error}"),
        };
        Self {
            db,
            auth_db,
            payment_db,
            system_db,
            finance_db,
            redis,
            config,
            object_storage,
            admission,
            web_auth,
            notification_transport,
        }
    }

    fn pool_for(&self, role: DbRole) -> &PgPool {
        match role {
            DbRole::Api => &self.db,
            DbRole::AuthWorker => &self.auth_db,
            DbRole::PaymentWorker => &self.payment_db,
            DbRole::SystemWorker => &self.system_db,
            DbRole::FinanceWorker => &self.finance_db,
        }
    }

    /// Begins a transaction on the given capability-role pool with the caller's
    /// identity bound to it via `set_config`, so every Row-Level Security policy
    /// evaluated inside this transaction sees `current_setting('app.current_user_id')`
    /// / `current_setting('app.current_role')`.
    ///
    /// `set_config(..., true)` is transaction-scoped (the `is_local` flag), not
    /// session-scoped, so it can never leak into a later request that reuses this
    /// same physical pooled connection — it is reset automatically on commit/rollback.
    /// This is why every protected handler must do its DB work inside this transaction
    /// rather than issuing ad-hoc queries against a pool directly.
    pub async fn begin_authed_as(
        &self,
        ctx: &AuthContext,
        role: DbRole,
    ) -> Result<Transaction<'_, Postgres>, AppError> {
        crate::api::middleware::request_context::bind_actor(
            ctx,
            self.config.consultation_trace_retention_days,
        );
        let started = std::time::Instant::now();
        let mut tx = tokio::time::timeout(
            std::time::Duration::from_millis(250),
            self.pool_for(role).begin(),
        )
        .await
        .map_err(|_| {
            metrics::counter!("db_acquire_deadline_total").increment(1);
            AppError::service_unavailable("Database operation is temporarily unavailable.".into())
        })??;
        metrics::histogram!("db_acquire_seconds").record(started.elapsed().as_secs_f64());
        sqlx::query("SELECT set_config('app.current_user_id', $1, true), set_config('app.current_role', $2, true)")
            .bind(ctx.user_id.to_string())
            .bind(ctx.role.as_str())
            .execute(&mut *tx)
            .await?;
        if matches!(role, DbRole::Api | DbRole::AuthWorker) {
            sqlx::query("SELECT set_config('app.current_patient_id',COALESCE((SELECT id::text FROM patient_profiles WHERE user_id=$1 AND $2='PATIENT'),''),true), set_config('app.current_doctor_id',COALESCE((SELECT id::text FROM doctor_profiles WHERE user_id=$1 AND $2='DOCTOR'),''),true), set_config('app.current_session_id',$3,true),set_config('app.current_request_id',$4,true)")
            .bind(ctx.user_id).bind(ctx.role.as_str())
            .bind(ctx.session_id.map(|id| id.to_string()).unwrap_or_default())
            .bind(ctx.request_id.as_deref().unwrap_or_default())
            .execute(&mut *tx).await?;
        } else {
            sqlx::query("SELECT set_config('app.current_patient_id','',true),set_config('app.current_doctor_id','',true),set_config('app.current_session_id',$1,true),set_config('app.current_request_id',$2,true)")
                .bind(ctx.session_id.map(|id| id.to_string()).unwrap_or_default()).bind(ctx.request_id.as_deref().unwrap_or_default()).execute(&mut *tx).await?;
        }
        sqlx::query("SELECT set_config('app.trace_retention_days',$1,true)")
            .bind(self.config.consultation_trace_retention_days.to_string())
            .execute(&mut *tx)
            .await?;
        Ok(tx)
    }

    /// Shorthand for `begin_authed_as(ctx, DbRole::Api)` — the common case for
    /// ordinary reads and patient/doctor-authored content writes.
    pub async fn begin_authed(
        &self,
        ctx: &AuthContext,
    ) -> Result<Transaction<'_, Postgres>, AppError> {
        self.begin_authed_as(ctx, DbRole::Api).await
    }
}
