pub mod admin_article_repo;
pub mod article_quiz_repo;
pub mod article_repo;
use crate::auth_context::AuthContext;
use crate::config::{AppConfig, StorageProvider};
use crate::error::AppError;
use sqlx::{PgPool, Postgres, Transaction};
use std::sync::Arc;

pub mod appointment_repo;
pub mod audit_repo;
pub mod campaign_repo;
pub mod chat_repo;
pub mod consultation_repo;
pub mod doctor_repo;
pub mod email_outbox_repo;
pub mod grievance_repo;
pub mod idempotency_repo;
pub mod legal_repo;
pub mod notification_repo;
pub mod otp_repo;
pub mod password_reset_repo;
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
    /// auth_sessions / OTP / login bootstrap, including the one patient
    /// self-registration insert at the end of first-time OTP verify.
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
    /// Clinical/private storage handle. The selected container is fixed by
    /// server configuration and is never supplied by a client.
    pub object_storage: Option<Arc<dyn object_store::ObjectStore>>,
    /// Separate article-media handle. Azure deployments use a distinct
    /// private container so article media can never address clinical blobs.
    pub article_media_storage: Option<Arc<dyn object_store::ObjectStore>>,
    pub admission: Arc<crate::admission::Admission>,
    pub web_auth: Arc<crate::api::middleware::web_auth::WebAuthGate>,
    pub notification_transport:
        Arc<dyn crate::services::notification_transport::NotificationTransport>,
    pub email_transport: Arc<dyn crate::services::email_transport::EmailTransport>,
    /// Phase 2: shared presentation config for every transactional email
    /// template — constructed once from `AppConfig` so call sites never
    /// hand-roll a placeholder URL. `EmailAssets::none()`/`EmailLinks::none()`
    /// (no hero images, no CTA buttons) is what every environment gets
    /// until these env vars are deliberately set.
    pub email_assets: crate::services::email_templates::EmailAssets,
    pub email_links: crate::services::email_templates::EmailLinks,
}

impl AppState {
    fn build_storage(
        config: &AppConfig,
        container: &str,
    ) -> Option<Arc<dyn object_store::ObjectStore>> {
        let client_options = object_store::ClientOptions::new()
            .with_allow_http(config.is_development_or_test())
            .with_timeout(std::time::Duration::from_secs(6));
        match config.storage_provider {
            StorageProvider::Disabled => None,
            StorageProvider::LegacyS3 => Some(Arc::new(
                object_store::aws::AmazonS3Builder::new()
                    .with_bucket_name(container)
                    .with_region("us-east-1")
                    .with_endpoint(&config.storage_endpoint)
                    .with_access_key_id(&config.storage_access_key)
                    .with_secret_access_key(&config.storage_secret_key)
                    .with_allow_http(config.is_development_or_test())
                    .with_client_options(client_options)
                    .build()
                    .unwrap_or_else(|_| {
                        panic!("Invalid private legacy object-storage configuration")
                    }),
            ) as Arc<dyn object_store::ObjectStore>),
            StorageProvider::Azure => {
                let account = config
                    .azure_storage_account_name
                    .as_deref()
                    .expect("FATAL: Azure storage account configuration is missing");
                let mut builder = object_store::azure::MicrosoftAzureBuilder::new()
                    .with_account(account)
                    .with_container_name(container)
                    .with_allow_http(config.is_development_or_test())
                    .with_client_options(client_options);
                if let Some(endpoint) = config.azure_storage_endpoint.as_deref() {
                    builder = builder.with_endpoint(endpoint.to_owned());
                }
                match config
                    .azure_storage_auth_mode
                    .trim()
                    .to_ascii_lowercase()
                    .as_str()
                {
                    "managed_identity" => {
                        builder = builder.with_credential_type("managed_identity");
                        if let Some(client_id) = config.azure_storage_client_id.as_deref() {
                            builder = builder.with_client_id(client_id);
                        }
                    }
                    "account_key" => {
                        builder = builder.with_credential_type("access_key").with_access_key(
                            config
                                .azure_storage_account_key
                                .as_deref()
                                .expect("FATAL: Azure account key configuration is missing"),
                        );
                    }
                    "azurite" => {
                        builder = builder.with_use_emulator(true);
                        if let Some(account_key) = config.azure_storage_account_key.as_deref() {
                            builder = builder.with_access_key(account_key);
                        }
                    }
                    _ => panic!("FATAL: invalid Azure storage authentication mode"),
                }
                Some(Arc::new(
                    builder.build().unwrap_or_else(|_| {
                        panic!("Invalid private Azure Blob Storage configuration")
                    }),
                ) as Arc<dyn object_store::ObjectStore>)
            }
        }
    }

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
        let object_storage = Self::build_storage(config.as_ref(), &config.storage_bucket);
        let article_media_storage = match config.storage_provider {
            StorageProvider::Azure => Self::build_storage(
                config.as_ref(),
                config
                    .azure_storage_article_container
                    .as_deref()
                    .expect("FATAL: Azure article-media container configuration is missing"),
            ),
            StorageProvider::LegacyS3 => object_storage.clone(),
            StorageProvider::Disabled => None,
        };
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
        > = if config.recovery_mode {
            // A restored system must not send recovered outbox rows while an
            // operator is reconciling provider state. This is deliberately
            // enforced at construction time, rather than relying on an
            // operator to remember a second feature flag.
            tracing::warn!("FCM push transport: Noop (RECOVERY_MODE) — every notification row will be created but never pushed, by design, until recovery ends.");
            Arc::new(crate::services::notification_transport::NoopTransport)
        } else {
            match crate::services::notification_transport::FcmTransport::from_config(&config) {
                Ok(Some(transport)) => {
                    tracing::info!("FCM push transport: configured — notifications will be pushed to device tokens.");
                    Arc::new(transport)
                }
                Ok(None) => {
                    // Never logs the credential values themselves, only that
                    // they're absent — this silent fallback (an in-app
                    // notification row is still created; the push to the
                    // device is simply never attempted) was previously
                    // invisible in production logs, indistinguishable from a
                    // real delivery failure further down the pipeline.
                    tracing::warn!("FCM push transport: Noop — FCM_PROJECT_ID/FCM_CLIENT_EMAIL/FCM_PRIVATE_KEY_B64 are not all set, so no push notification will ever reach a device even though in-app notification rows are created normally.");
                    Arc::new(crate::services::notification_transport::NoopTransport)
                }
                Err(error) => panic!("FATAL: invalid FCM configuration: {error}"),
            }
        };
        let email_transport: Arc<dyn crate::services::email_transport::EmailTransport> =
            if config.recovery_mode {
                // Same safety posture as `notification_transport` above: a
                // restored system must not send email while an operator is
                // reconciling provider state. In practice this is moot for
                // ordinary login OTP email specifically, since RECOVERY_MODE
                // already disables ordinary authentication entirely
                // (see docs/RECOVERY_SECURITY_MODEL.md) — kept for
                // consistency and so any future, non-auth email trigger
                // inherits the same guarantee automatically.
                Arc::new(crate::services::email_transport::NoopEmailTransport)
            } else {
                match crate::services::email_transport::AcsEmailTransport::from_config(&config) {
                    Ok(Some(transport)) => Arc::new(transport),
                    Ok(None) => Arc::new(crate::services::email_transport::NoopEmailTransport),
                    Err(error) => panic!(
                        "FATAL: invalid Azure Communication Services Email configuration: {error}"
                    ),
                }
            };
        let email_assets = match config.email_asset_base_url.as_deref() {
            Some(base_url) => {
                crate::services::email_templates::EmailAssets::with_base_url(base_url)
            }
            None => crate::services::email_templates::EmailAssets::none(),
        };
        let email_links = crate::services::email_templates::EmailLinks::new(
            config.email_patient_app_url.as_deref(),
            config.email_doctor_portal_url.as_deref(),
        );
        Self {
            db,
            auth_db,
            payment_db,
            system_db,
            finance_db,
            redis,
            config,
            object_storage,
            article_media_storage,
            admission,
            web_auth,
            notification_transport,
            email_assets,
            email_links,
            email_transport,
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

#[cfg(test)]
mod storage_tests {
    use object_store::{path::Path, ObjectStore, PutMode, PutOptions};

    #[test]
    fn azure_builder_supports_explicit_account_key_and_managed_identity_modes() {
        let account_key = "Eby8vdM02xNOcqFlqUwJPLlmEtlCDXJ1OUzFT50uSRZ6IFsuFq2UVErCz4I6tq/K1SZFPTOtr/KBHBeksoGMGw==";
        object_store::azure::MicrosoftAzureBuilder::new()
            .with_account("hellodoctor")
            .with_container_name("clinical-private")
            .with_credential_type("access_key")
            .with_access_key(account_key)
            .build()
            .expect("account-key Azure builder should validate without network access");
        object_store::azure::MicrosoftAzureBuilder::new()
            .with_account("hellodoctor")
            .with_container_name("article-media")
            .with_credential_type("managed_identity")
            .build()
            .expect("managed-identity Azure builder should validate without network access");
    }

    #[tokio::test]
    async fn create_only_contract_rejects_overwrite() {
        let store = object_store::memory::InMemory::new();
        store
            .put_opts(
                &Path::from("clinical/test.bin"),
                vec![1_u8].into(),
                PutOptions {
                    mode: PutMode::Create,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let error = store
            .put_opts(
                &Path::from("clinical/test.bin"),
                vec![2_u8].into(),
                PutOptions {
                    mode: PutMode::Create,
                    ..Default::default()
                },
            )
            .await
            .expect_err("create-only writes must reject an existing object");
        assert!(matches!(error, object_store::Error::AlreadyExists { .. }));
    }
}
