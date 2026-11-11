use base64::{engine::general_purpose::STANDARD, Engine as _};
use std::env;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageProvider {
    Disabled,
    Azure,
    LegacyS3,
}

impl StorageProvider {
    fn parse(value: &str) -> Result<Self, &'static str> {
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "disabled" | "none" => Ok(Self::Disabled),
            "azure" => Ok(Self::Azure),
            "s3" | "legacy_s3" => Ok(Self::LegacyS3),
            _ => Err("STORAGE_PROVIDER must be disabled, azure, or legacy_s3."),
        }
    }
}

/// Formal production product decision (not a development bypass): whether
/// appointment booking requires a real payment transaction before a
/// consultation may proceed clinically. `Enabled` is the default and
/// preserves every existing payment/earnings/settlement requirement
/// unchanged, including this backend's current real-world behavior of
/// refusing to book at all outside `development` (no payment gateway is
/// actually integrated yet — see `AppointmentService::book_appointment`).
/// `Deferred` is an explicit operator opt-in that lets booking skip payment
/// entirely for a production launch, while every existing financial code
/// path (wallet, settlement, payouts, refunds) remains fully intact and
/// untouched for later re-enablement — see docs/PROGRESS.md for the
/// accompanying design note.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FinancialModuleMode {
    Deferred,
    Enabled,
}

impl FinancialModuleMode {
    fn parse(value: &str) -> Result<Self, &'static str> {
        match value.trim().to_ascii_lowercase().as_str() {
            "enabled" => Ok(Self::Enabled),
            "deferred" => Ok(Self::Deferred),
            _ => Err("FINANCIAL_MODULE_MODE must be enabled or deferred."),
        }
    }
}

#[derive(Clone)]
pub struct AppConfig {
    pub port: u16,
    pub host: String,
    pub database_url: String,
    /// Shared connection coordinates for the five least-privilege capability
    /// role pools (see migrations/20261003000001_rls_hardening). Each role
    /// connects to the same physical database with its own password — never
    /// a shared one — so a DSN is built per-role from these parts rather than
    /// hardcoding five near-duplicate URLs.
    pub app_db_host: String,
    pub app_db_port: u16,
    pub app_db_name: String,
    pub db_api_role_password: String,
    pub db_auth_worker_password: String,
    pub db_payment_worker_password: String,
    pub db_system_worker_password: String,
    pub db_finance_worker_password: String,
    pub db_pool_max_connections_per_role: u32,
    /// Milestone 8 pool-capacity experiment (2026-11-04): an isolated,
    /// narrowly-scoped override for ONLY the API-role pool's connection
    /// count, independent of `db_pool_max_connections_per_role`. Added after
    /// evidence showed the API role's pool (shared by Normal/Consultation/
    /// Expensive admission classes) was the dominant source of
    /// `dependency_pressure` rejections, to test whether a larger API pool
    /// reduces that specific failure mode -- exclusively on the disposable
    /// `hellodoctor-m8-final-local` benchmark stack. `None` (the default,
    /// unless `DB_POOL_MAX_CONNECTIONS_API_ROLE` is explicitly set) means the
    /// API role falls back to `db_pool_max_connections_per_role` exactly as
    /// before -- zero behavior change for every other deployment. Does NOT
    /// feed `Admission::new`'s lane-sizing math (still
    /// `db_pool_max_connections_per_role * 5`, deliberately unchanged, so
    /// this cannot silently widen admission concurrency ceilings), and does
    /// NOT affect the Auth/Payment/System/Finance role pools, which always
    /// use `db_pool_max_connections_per_role` regardless of this override.
    pub db_pool_max_connections_api_role_override: Option<u32>,
    /// h23 remediation: how long a `PendingPayment` appointment's
    /// `LOCKED_IN_PAYMENT` slot hold may sit unpaid before `book_appointment`
    /// opportunistically reclaims it for the next booker. No product-level
    /// checkout-abandonment SLA exists yet in the docs this repository ships
    /// with, so this default is a `PENDING_PRODUCT_DECISION` placeholder,
    /// not a specified requirement — override via env for real deployments.
    pub payment_reservation_window_secs: i64,
    /// Controlled admission cap on NEW user activation only (patient
    /// self-registration, admin-created doctor accounts) — existing users'
    /// sessions, bookings, and consultations are never affected once this is
    /// reached. An operational dial, not an architectural ceiling: raising it
    /// is a config change, not a redesign. See docs/CHECKPOINT.md for the
    /// evidence-gated process for actually raising it in a real deployment.
    pub max_active_users: u32,
    pub consultation_trace_retention_days: u32,
    /// See `rate_limit::check_global_ip_limit`'s doc comment — default 120
    /// matches docs/AUTH_SECURITY.md §9 and should stay that way in any real
    /// deployment; only load-testing tooling has a reason to raise it.
    pub global_ip_rate_limit_per_min: u32,
    pub redis_url: String,
    pub jwt_ed25519_private_key_b64: String,
    pub jwt_ed25519_public_key_b64: String,
    pub totp_encryption_key_b64: String,
    pub agora_app_id: String,
    pub agora_app_certificate: String,
    /// How long before a slot's `start_time` an RTC token may first be
    /// minted, and how long after `end_time` it may still be minted (a
    /// grace period for a consultation genuinely running long) — see
    /// `ConsultationService::mint_agora_token`. Configurable per
    /// docs/AUTH_SECURITY.md §10.2 rather than a hardcoded window, since the
    /// right values are a product decision, not an architectural constant.
    pub agora_token_window_before_mins: i64,
    pub agora_token_window_after_mins: i64,
    /// Emergency backstop for a `CONFIRMED` live-consultation reservation
    /// (`consultation_live_reservations`, M8-G-30) — not a maximum
    /// consultation duration, and expiry must never be treated as proof
    /// Agora actually disconnected. It exists only to recover a reservation
    /// stuck by a permanently lost Left webhook, refreshed on every further
    /// accepted presence event for that user so a normally-reconnecting call
    /// never approaches it. `PENDING_PRODUCT_DECISION` default per
    /// docs/MILESTONE_8_SINGLE_LIVE_CONSULTATION_DESIGN.md §7 — approved at
    /// 6 hours (21600s) for this implementation.
    pub consultation_live_reservation_max_secs: i64,
    pub webhook_hmac_secret: String,
    /// Dedicated HMAC key for provider-confirmed Agora presence callbacks.
    /// Empty in development means callback ingestion fails closed until the
    /// provider integration is actually configured.
    pub agora_webhook_hmac_secret: String,
    pub storage_bucket: String,
    pub storage_endpoint: String,
    pub storage_access_key: String,
    pub storage_secret_key: String,
    pub storage_encryption_key_b64: String,
    pub storage_provider: StorageProvider,
    pub azure_storage_account_name: Option<String>,
    pub azure_storage_auth_mode: String,
    pub azure_storage_clinical_container: Option<String>,
    pub azure_storage_article_container: Option<String>,
    pub azure_storage_account_key: Option<String>,
    pub azure_storage_client_id: Option<String>,
    pub azure_storage_endpoint: Option<String>,
    pub azure_storage_use_emulator: bool,
    /// Remediation E3: explicit, production-specific override required
    /// before `AZURE_STORAGE_AUTH_MODE=account_key` is accepted outside
    /// development/test. Defaults to `false` (rejected) so a deployed
    /// environment falls back to managed identity unless an operator has
    /// deliberately opted into the account-key exception — see
    /// docs/AZURE_BLOB_SETUP.md "Account-key mode" for the operational
    /// risk this acknowledges. This flag only widens the single
    /// account-key gate below; it has no effect on any other Azure
    /// validation (container names, encryption key, endpoint, emulator
    /// restrictions all still apply) and grants no Azure RBAC permissions
    /// of its own.
    pub azure_storage_account_key_risk_acknowledged: bool,
    pub public_api_base_url: String,
    /// See `FinancialModuleMode`. Defaults to `Enabled` (today's exact
    /// behavior) unless `FINANCIAL_MODULE_MODE=deferred` is explicitly set —
    /// never silently defaulted to deferred, since that would silently stop
    /// collecting payment.
    pub financial_module_mode: FinancialModuleMode,
    pub environment: String,
    pub seed_dev_data: bool,
    pub dev_auth_otp_enabled: bool,
    pub dev_simulation_enabled: bool,
    /// Password recovery is deliberately disabled until a complete,
    /// rate-limited, email-backed reset flow is implemented. Keeping the
    /// switch in the canonical server config makes an accidental enablement
    /// fail before any listener or database pool is opened.
    pub password_reset_enabled: bool,
    /// Recovery mode starts the application in a read-mostly, outbound-safe
    /// posture while an operator validates a restored environment. It is
    /// intentionally a single fail-closed switch; normal mode requires a
    /// restart with RECOVERY_MODE=false after the recovery checklist passes.
    pub recovery_mode: bool,
    /// SHA-256 digest of a separately delivered, short-lived recovery-operator
    /// token. The token itself is never stored in configuration or logs.
    pub recovery_operator_token_sha256: Option<String>,
    /// Remediation E1: identifies the specific disaster-recovery incident
    /// this boot belongs to. Required whenever RECOVERY_MODE=true so the
    /// reconciliation gate (see `services::dr_reconciliation`) can bind its
    /// evidence to exactly one recovery instance and point, never an
    /// ambient/global flag.
    pub recovery_instance_id: Option<String>,
    /// Filesystem path to the externally-custodied, HMAC-chained erasure
    /// journal (Remediation E1). MUST be outside any Postgres backup domain
    /// in production — see `services::dr_erasure_journal` module docs for
    /// why a local path alone is not a production guarantee.
    pub external_erasure_journal_path: Option<String>,
    pub external_erasure_journal_hmac_key_b64: Option<String>,
    pub external_erasure_journal_anchor_key_b64: Option<String>,
    pub external_erasure_journal_key_version: String,
    pub trusted_proxy_ips: Vec<std::net::IpAddr>,
    pub metrics_scrape_token: String,
    pub allowed_origins: Vec<String>,
    /// Kenya-local (`Africa/Nairobi`) time of day, `HH:MM`, at which each
    /// Doctor's `DAILY_CONSULTATION_SUMMARY` notification is generated.
    /// Configuration-backed per the Doctor Engagement extension's own
    /// requirement ("do not hardcode timing throughout business logic") —
    /// changing the delivery time is an env var, not a code change. See
    /// `notification_scheduler.rs`.
    pub doctor_daily_summary_time_local: String,
    /// Kenya-local time of day, `HH:MM`, at which the weekly good-morning
    /// engagement greeting (`DOCTOR_GREETING`/`PATIENT_GREETING`) and the
    /// weekly `HEALTH_EDUCATION` nudge are generated (Milestones 4/5).
    /// Mirrors `doctor_daily_summary_time_local`'s own config-not-code
    /// rationale.
    pub engagement_greeting_time_local: String,
    pub azure_email_endpoint: Option<String>,
    pub azure_email_access_key: Option<String>,
    pub azure_email_sender: Option<String>,
    /// `access_key` (the only implemented mode, HMAC-SHA256-signed requests)
    /// or `managed_identity` (recognized but rejected at startup — see
    /// `validate_azure_email_configuration`'s doc comment for why). Defaults
    /// to `access_key` so existing deployments need no new configuration.
    pub azure_email_auth_mode: String,
    /// FCM HTTP v1 service-account material. These values are server-only and
    /// are intentionally optional in development/test, where NoopTransport
    /// remains the safe default.
    pub fcm_project_id: Option<String>,
    pub fcm_client_email: Option<String>,
    pub fcm_private_key_b64: Option<String>,
    /// Phase 2 transactional email presentation config — all optional,
    /// `None` (no hero images, no CTA buttons) is the safe default until an
    /// operator deliberately configures real, approved values. See
    /// `services::email_templates::{EmailAssets, EmailLinks}`.
    pub email_asset_base_url: Option<String>,
    pub email_patient_app_url: Option<String>,
    pub email_doctor_portal_url: Option<String>,
}

/// Remediation E3: hand-written in place of `#[derive(Debug)]` so credential
/// and key-material fields can never reach a log line, panic message, or
/// test-failure assertion through ordinary `{:?}` formatting. Each field is
/// listed explicitly and defaults closed: a field added to `AppConfig` later
/// is simply absent from this output until someone deliberately adds it
/// below (and decides whether it needs `secret`/`secret_opt`), rather than
/// `derive(Debug)`'s default of printing a new field's raw value. See
/// `debug_redaction_tests` for the regression test against synthetic secrets.
impl std::fmt::Debug for AppConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fn secret(value: &str) -> &'static str {
            if value.is_empty() {
                "<unset>"
            } else {
                "<redacted>"
            }
        }
        fn secret_opt(value: &Option<String>) -> &'static str {
            match value {
                Some(v) if !v.is_empty() => "<redacted>",
                _ => "<unset>",
            }
        }
        f.debug_struct("AppConfig")
            .field("port", &self.port)
            .field("host", &self.host)
            .field("database_url", &secret(&self.database_url))
            .field("app_db_host", &self.app_db_host)
            .field("app_db_port", &self.app_db_port)
            .field("app_db_name", &self.app_db_name)
            .field("db_api_role_password", &secret(&self.db_api_role_password))
            .field(
                "db_auth_worker_password",
                &secret(&self.db_auth_worker_password),
            )
            .field(
                "db_payment_worker_password",
                &secret(&self.db_payment_worker_password),
            )
            .field(
                "db_system_worker_password",
                &secret(&self.db_system_worker_password),
            )
            .field(
                "db_finance_worker_password",
                &secret(&self.db_finance_worker_password),
            )
            .field(
                "db_pool_max_connections_per_role",
                &self.db_pool_max_connections_per_role,
            )
            .field(
                "db_pool_max_connections_api_role_override",
                &self.db_pool_max_connections_api_role_override,
            )
            .field(
                "payment_reservation_window_secs",
                &self.payment_reservation_window_secs,
            )
            .field("max_active_users", &self.max_active_users)
            .field(
                "consultation_trace_retention_days",
                &self.consultation_trace_retention_days,
            )
            .field(
                "global_ip_rate_limit_per_min",
                &self.global_ip_rate_limit_per_min,
            )
            .field("redis_url", &secret(&self.redis_url))
            .field(
                "jwt_ed25519_private_key_b64",
                &secret(&self.jwt_ed25519_private_key_b64),
            )
            .field(
                "jwt_ed25519_public_key_b64",
                &self.jwt_ed25519_public_key_b64,
            )
            .field(
                "totp_encryption_key_b64",
                &secret(&self.totp_encryption_key_b64),
            )
            .field("agora_app_id", &self.agora_app_id)
            .field(
                "agora_app_certificate",
                &secret(&self.agora_app_certificate),
            )
            .field(
                "agora_token_window_before_mins",
                &self.agora_token_window_before_mins,
            )
            .field(
                "agora_token_window_after_mins",
                &self.agora_token_window_after_mins,
            )
            .field(
                "consultation_live_reservation_max_secs",
                &self.consultation_live_reservation_max_secs,
            )
            .field("webhook_hmac_secret", &secret(&self.webhook_hmac_secret))
            .field(
                "agora_webhook_hmac_secret",
                &secret(&self.agora_webhook_hmac_secret),
            )
            .field("storage_bucket", &self.storage_bucket)
            .field("storage_endpoint", &self.storage_endpoint)
            .field("storage_access_key", &secret(&self.storage_access_key))
            .field("storage_secret_key", &secret(&self.storage_secret_key))
            .field(
                "storage_encryption_key_b64",
                &secret(&self.storage_encryption_key_b64),
            )
            .field("storage_provider", &self.storage_provider)
            .field(
                "azure_storage_account_name",
                &self.azure_storage_account_name,
            )
            .field("azure_storage_auth_mode", &self.azure_storage_auth_mode)
            .field(
                "azure_storage_clinical_container",
                &self.azure_storage_clinical_container,
            )
            .field(
                "azure_storage_article_container",
                &self.azure_storage_article_container,
            )
            .field(
                "azure_storage_account_key",
                &secret_opt(&self.azure_storage_account_key),
            )
            .field("azure_storage_client_id", &self.azure_storage_client_id)
            .field("azure_storage_endpoint", &self.azure_storage_endpoint)
            .field(
                "azure_storage_use_emulator",
                &self.azure_storage_use_emulator,
            )
            .field(
                "azure_storage_account_key_risk_acknowledged",
                &self.azure_storage_account_key_risk_acknowledged,
            )
            .field("public_api_base_url", &self.public_api_base_url)
            .field("financial_module_mode", &self.financial_module_mode)
            .field("environment", &self.environment)
            .field("seed_dev_data", &self.seed_dev_data)
            .field("dev_auth_otp_enabled", &self.dev_auth_otp_enabled)
            .field("dev_simulation_enabled", &self.dev_simulation_enabled)
            .field("password_reset_enabled", &self.password_reset_enabled)
            .field("recovery_mode", &self.recovery_mode)
            .field(
                "recovery_operator_token_sha256",
                &self.recovery_operator_token_sha256,
            )
            .field("recovery_instance_id", &self.recovery_instance_id)
            .field(
                "external_erasure_journal_path",
                &self.external_erasure_journal_path,
            )
            .field(
                "external_erasure_journal_hmac_key_b64",
                &secret_opt(&self.external_erasure_journal_hmac_key_b64),
            )
            .field(
                "external_erasure_journal_anchor_key_b64",
                &secret_opt(&self.external_erasure_journal_anchor_key_b64),
            )
            .field(
                "external_erasure_journal_key_version",
                &self.external_erasure_journal_key_version,
            )
            .field("trusted_proxy_ips", &self.trusted_proxy_ips)
            .field("metrics_scrape_token", &secret(&self.metrics_scrape_token))
            .field("allowed_origins", &self.allowed_origins)
            .field(
                "doctor_daily_summary_time_local",
                &self.doctor_daily_summary_time_local,
            )
            .field(
                "engagement_greeting_time_local",
                &self.engagement_greeting_time_local,
            )
            .field("azure_email_endpoint", &self.azure_email_endpoint)
            .field(
                "azure_email_access_key",
                &secret_opt(&self.azure_email_access_key),
            )
            .field("azure_email_sender", &self.azure_email_sender)
            .field("azure_email_auth_mode", &self.azure_email_auth_mode)
            .field("fcm_project_id", &self.fcm_project_id)
            .field("fcm_client_email", &self.fcm_client_email)
            .field(
                "fcm_private_key_b64",
                &secret_opt(&self.fcm_private_key_b64),
            )
            .field("email_asset_base_url", &self.email_asset_base_url)
            .field("email_patient_app_url", &self.email_patient_app_url)
            .field("email_doctor_portal_url", &self.email_doctor_portal_url)
            .finish()
    }
}

impl AppConfig {
    pub fn from_env() -> Self {
        if let Ok(path) = env::var("HELLODOCTOR_ENV_FILE") {
            dotenvy::from_path(&path).unwrap_or_else(|error| {
                panic!("FATAL: HELLODOCTOR_ENV_FILE could not be loaded: {error}")
            });
        } else {
            dotenvy::dotenv().ok();
        }

        // Validate runtime mode before reading credentials or opening any pool.
        let configured_environment = env::var("ENVIRONMENT").ok();
        let configured_seed = env::var("SEED_DEV_DATA").ok();
        let (environment, seed_dev_data) = validate_startup_mode(
            configured_environment.as_deref(),
            configured_seed.as_deref(),
        )
        .unwrap_or_else(|error| panic!("FATAL: {error}"));

        let require = |var: &str| -> String {
            match env::var(var) {
                Ok(v) if !v.is_empty() => v,
                _ => panic!(
                    "FATAL: {} must be set (see .env.example for how to generate it). Refusing to start with no secret.",
                    var
                ),
            }
        };

        let storage_provider = StorageProvider::parse(
            &env::var("STORAGE_PROVIDER").unwrap_or_else(|_| "disabled".into()),
        )
        .unwrap_or_else(|error| panic!("FATAL: {error}"));

        // Defaults to "enabled" (today's existing behavior, unchanged) —
        // only an explicit FINANCIAL_MODULE_MODE=deferred unlocks the new
        // payment-optional booking path. An invalid value fails closed at
        // startup, same as STORAGE_PROVIDER above, never silently falling
        // back to either mode.
        let financial_module_mode = FinancialModuleMode::parse(
            &env::var("FINANCIAL_MODULE_MODE").unwrap_or_else(|_| "enabled".into()),
        )
        .unwrap_or_else(|error| panic!("FATAL: {error}"));

        // No implicit local storage installation or development credentials.
        let storage_setting = |name: &str| -> String {
            if environment == "development"
                || !matches!(storage_provider, StorageProvider::LegacyS3)
            {
                env::var(name).unwrap_or_default()
            } else {
                require(name)
            }
        };
        let legacy_storage_bucket =
            env::var("STORAGE_BUCKET").unwrap_or_else(|_| "helodoc-medical-records".into());

        let allowed_origins: Vec<String> = env::var("ALLOWED_ORIGINS")
            .unwrap_or_else(|_| "http://localhost:3000".into())
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let boolean = |name: &str| match env::var(name).as_deref().unwrap_or("false") {
            "true" => true,
            "false" => false,
            _ => panic!("FATAL: {name} must be true or false."),
        };
        let trusted_proxy_ips = env::var("TRUSTED_PROXY_IPS").unwrap_or_default()
            .split(',').map(str::trim).filter(|ip| !ip.is_empty())
            .map(|ip| ip.parse().expect("FATAL: TRUSTED_PROXY_IPS must contain exact IP addresses, not networks or hostnames."))
            .collect();
        let config = Self {
            port: env::var("PORT")
                .map(|p| {
                    p.parse()
                        .unwrap_or_else(|_| panic!("FATAL: PORT must be a valid unsigned integer."))
                })
                .unwrap_or(8080),
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            database_url: require("DATABASE_URL"),
            app_db_host: env::var("APP_DB_HOST").unwrap_or_else(|_| "localhost".into()),
            app_db_port: env::var("APP_DB_PORT")
                .map(|p| {
                    p.parse().unwrap_or_else(|_| {
                        panic!("FATAL: APP_DB_PORT must be a valid unsigned integer.")
                    })
                })
                .unwrap_or(5432),
            app_db_name: env::var("APP_DB_NAME").unwrap_or_else(|_| "hellodoctor".into()),
            db_api_role_password: require("DB_API_ROLE_PASSWORD"),
            db_auth_worker_password: require("DB_AUTH_WORKER_PASSWORD"),
            db_payment_worker_password: require("DB_PAYMENT_WORKER_PASSWORD"),
            db_system_worker_password: require("DB_SYSTEM_WORKER_PASSWORD"),
            db_finance_worker_password: require("DB_FINANCE_WORKER_PASSWORD"),
            db_pool_max_connections_per_role: env::var("DB_POOL_MAX_CONNECTIONS_PER_ROLE")
                .map(|p| {
                    p.parse().unwrap_or_else(|_| {
                        panic!(
                            "FATAL: DB_POOL_MAX_CONNECTIONS_PER_ROLE must be a valid unsigned integer."
                        )
                    })
                })
                .unwrap_or(10),
            db_pool_max_connections_api_role_override: env::var(
                "DB_POOL_MAX_CONNECTIONS_API_ROLE",
            )
            .ok()
            .map(|p| {
                p.parse().unwrap_or_else(|_| {
                    panic!(
                        "FATAL: DB_POOL_MAX_CONNECTIONS_API_ROLE must be a valid unsigned integer."
                    )
                })
            }),
            payment_reservation_window_secs: env::var("PAYMENT_RESERVATION_WINDOW_SECS")
                .map(|p| {
                    p.parse().unwrap_or_else(|_| {
                        panic!(
                            "FATAL: PAYMENT_RESERVATION_WINDOW_SECS must be a valid integer."
                        )
                    })
                })
                .unwrap_or(600),
            consultation_trace_retention_days: env::var("CONSULTATION_TRACE_RETENTION_DAYS")
                .map(|v| {
                    v.parse::<u32>()
                        .expect("Trace retention must be 3 to 5 days")
                })
                .unwrap_or(5),
            max_active_users: env::var("MAX_ACTIVE_USERS")
                .map(|p| {
                    p.parse().unwrap_or_else(|_| {
                        panic!("FATAL: MAX_ACTIVE_USERS must be a valid unsigned integer.")
                    })
                })
                .unwrap_or(1200),
            global_ip_rate_limit_per_min: env::var("GLOBAL_IP_RATE_LIMIT_PER_MIN")
                .map(|p| {
                    p.parse().unwrap_or_else(|_| {
                        panic!(
                            "FATAL: GLOBAL_IP_RATE_LIMIT_PER_MIN must be a valid unsigned integer."
                        )
                    })
                })
                .unwrap_or(120),
            redis_url: require("REDIS_URL"),
            jwt_ed25519_private_key_b64: require("JWT_ED25519_PRIVATE_KEY_B64"),
            jwt_ed25519_public_key_b64: require("JWT_ED25519_PUBLIC_KEY_B64"),
            totp_encryption_key_b64: require("TOTP_ENCRYPTION_KEY_B64"),
            agora_app_id: require("AGORA_APP_ID"),
            agora_app_certificate: require("AGORA_APP_CERTIFICATE"),
            agora_token_window_before_mins: env::var("AGORA_TOKEN_WINDOW_BEFORE_MINS")
                .map(|p| {
                    p.parse().unwrap_or_else(|_| {
                        panic!("FATAL: AGORA_TOKEN_WINDOW_BEFORE_MINS must be a valid integer.")
                    })
                })
                .unwrap_or(15),
            agora_token_window_after_mins: env::var("AGORA_TOKEN_WINDOW_AFTER_MINS")
                .map(|p| {
                    p.parse().unwrap_or_else(|_| {
                        panic!("FATAL: AGORA_TOKEN_WINDOW_AFTER_MINS must be a valid integer.")
                    })
                })
                .unwrap_or(60),
            consultation_live_reservation_max_secs: env::var(
                "CONSULTATION_LIVE_RESERVATION_MAX_SECS",
            )
            .map(|p| {
                p.parse().unwrap_or_else(|_| {
                    panic!(
                        "FATAL: CONSULTATION_LIVE_RESERVATION_MAX_SECS must be a valid integer."
                    )
                })
            })
            .unwrap_or(21_600),
            webhook_hmac_secret: require("WEBHOOK_HMAC_SECRET"),
            agora_webhook_hmac_secret: env::var("AGORA_WEBHOOK_HMAC_SECRET").unwrap_or_default(),
            storage_bucket: if storage_provider == StorageProvider::Azure {
                env::var("AZURE_STORAGE_CLINICAL_CONTAINER")
                    .unwrap_or_else(|_| legacy_storage_bucket.clone())
            } else {
                legacy_storage_bucket
            },
            storage_endpoint: storage_setting("STORAGE_ENDPOINT"),
            storage_access_key: storage_setting("STORAGE_ACCESS_KEY"),
            storage_secret_key: storage_setting("STORAGE_SECRET_KEY"),
            storage_encryption_key_b64: env::var("STORAGE_ENCRYPTION_KEY_B64").unwrap_or_default(),
            storage_provider,
            azure_storage_account_name: env::var("AZURE_STORAGE_ACCOUNT_NAME")
                .ok()
                .filter(|v| !v.trim().is_empty()),
            azure_storage_auth_mode: env::var("AZURE_STORAGE_AUTH_MODE")
                .unwrap_or_else(|_| "managed_identity".into()),
            azure_storage_clinical_container: env::var("AZURE_STORAGE_CLINICAL_CONTAINER")
                .ok()
                .filter(|v| !v.trim().is_empty()),
            azure_storage_article_container: env::var("AZURE_STORAGE_ARTICLE_CONTAINER")
                .ok()
                .filter(|v| !v.trim().is_empty()),
            azure_storage_account_key: env::var("AZURE_STORAGE_ACCOUNT_KEY")
                .ok()
                .filter(|v| !v.trim().is_empty()),
            azure_storage_client_id: env::var("AZURE_STORAGE_CLIENT_ID")
                .ok()
                .filter(|v| !v.trim().is_empty()),
            azure_storage_endpoint: env::var("AZURE_STORAGE_ENDPOINT")
                .ok()
                .filter(|v| !v.trim().is_empty()),
            azure_storage_use_emulator: boolean("AZURE_STORAGE_USE_EMULATOR"),
            azure_storage_account_key_risk_acknowledged: boolean(
                "AZURE_STORAGE_ACCOUNT_KEY_RISK_ACKNOWLEDGED",
            ),
            public_api_base_url: env::var("PUBLIC_API_BASE_URL").unwrap_or_else(|_| {
                if environment == "development" {
                    "http://127.0.0.1:8080".into()
                } else {
                    require("PUBLIC_API_BASE_URL")
                }
            }),
            financial_module_mode,
            environment,
            seed_dev_data,
            dev_auth_otp_enabled: boolean("DEV_AUTH_OTP_ENABLED"),
            dev_simulation_enabled: boolean("DEV_SIMULATION_ENABLED"),
            password_reset_enabled: boolean("PASSWORD_RESET_ENABLED"),
            recovery_mode: boolean("RECOVERY_MODE"),
            recovery_operator_token_sha256: env::var("RECOVERY_OPERATOR_TOKEN_SHA256")
                .ok()
                .filter(|v| !v.trim().is_empty()),
            recovery_instance_id: env::var("RECOVERY_INSTANCE_ID")
                .ok()
                .filter(|v| !v.trim().is_empty()),
            external_erasure_journal_path: env::var("EXTERNAL_ERASURE_JOURNAL_PATH")
                .ok()
                .filter(|v| !v.trim().is_empty()),
            external_erasure_journal_hmac_key_b64: env::var("EXTERNAL_ERASURE_JOURNAL_HMAC_KEY_B64")
                .ok()
                .filter(|v| !v.trim().is_empty()),
            external_erasure_journal_anchor_key_b64: env::var(
                "EXTERNAL_ERASURE_JOURNAL_ANCHOR_KEY_B64",
            )
            .ok()
            .filter(|v| !v.trim().is_empty()),
            external_erasure_journal_key_version: env::var("EXTERNAL_ERASURE_JOURNAL_KEY_VERSION")
                .ok()
                .filter(|v| !v.trim().is_empty())
                .unwrap_or_else(|| "v1".into()),
            trusted_proxy_ips,
            metrics_scrape_token: env::var("METRICS_SCRAPE_TOKEN").unwrap_or_default(),
            allowed_origins,
            doctor_daily_summary_time_local: env::var("DOCTOR_DAILY_SUMMARY_TIME_LOCAL")
                .unwrap_or_else(|_| "07:00".into()),
            engagement_greeting_time_local: env::var("ENGAGEMENT_GREETING_TIME_LOCAL")
                .unwrap_or_else(|_| "08:00".into()),
            azure_email_endpoint: env::var("AZURE_EMAIL_ENDPOINT")
                .ok()
                .filter(|v| !v.trim().is_empty()),
            azure_email_access_key: env::var("AZURE_EMAIL_ACCESS_KEY")
                .ok()
                .filter(|v| !v.trim().is_empty()),
            azure_email_sender: env::var("AZURE_EMAIL_SENDER")
                .ok()
                .filter(|v| !v.trim().is_empty()),
            azure_email_auth_mode: env::var("AZURE_EMAIL_AUTH_MODE")
                .unwrap_or_else(|_| "access_key".into()),
            fcm_project_id: env::var("FCM_PROJECT_ID")
                .ok()
                .filter(|v| !v.trim().is_empty()),
            fcm_client_email: env::var("FCM_CLIENT_EMAIL")
                .ok()
                .filter(|v| !v.trim().is_empty()),
            fcm_private_key_b64: env::var("FCM_PRIVATE_KEY_B64")
                .ok()
                .filter(|v| !v.trim().is_empty()),
            email_asset_base_url: env::var("EMAIL_ASSET_BASE_URL")
                .ok()
                .filter(|v| !v.trim().is_empty()),
            email_patient_app_url: env::var("EMAIL_PATIENT_APP_URL")
                .ok()
                .filter(|v| !v.trim().is_empty()),
            email_doctor_portal_url: env::var("EMAIL_DOCTOR_PORTAL_URL")
                .ok()
                .filter(|v| !v.trim().is_empty()),
        };
        config
            .validate_public_exposure()
            .unwrap_or_else(|error| panic!("FATAL: {error}"));
        config
    }

    pub fn is_development_or_test(&self) -> bool {
        matches!(self.environment.as_str(), "development" | "test")
    }

    pub fn requires_https(&self) -> bool {
        matches!(self.environment.as_str(), "staging" | "production")
    }

    pub fn financial_module_deferred(&self) -> bool {
        matches!(self.financial_module_mode, FinancialModuleMode::Deferred)
    }

    /// Remediation E1: decoded erasure-journal secrets, when configured.
    /// `validate_public_exposure` already guarantees both keys are present,
    /// independent, and at least 32 bytes whenever this deployment requires
    /// HTTPS or is in RECOVERY_MODE; this helper never re-derives a missing
    /// key, matching the fail-closed contract of `dr_erasure_journal`.
    pub fn erasure_journal_hmac_key(&self) -> Option<Vec<u8>> {
        self.external_erasure_journal_hmac_key_b64
            .as_deref()
            .and_then(|v| STANDARD.decode(v).ok())
    }

    pub fn erasure_journal_anchor_key(&self) -> Option<Vec<u8>> {
        self.external_erasure_journal_anchor_key_b64
            .as_deref()
            .and_then(|v| STANDARD.decode(v).ok())
    }

    pub fn simulations_allowed(&self) -> bool {
        self.dev_simulation_enabled
            && matches!(
                self.environment.as_str(),
                "development" | "test" | "staging"
            )
    }

    /// Called before any database/Redis connection; tests also exercise it directly.
    pub fn validate_public_exposure(&self) -> Result<(), &'static str> {
        validate_storage_configuration(self)?;
        if self.requires_https() && (self.port == 0 || self.app_db_port == 0) {
            return Err("PORT and APP_DB_PORT must be between 1 and 65535");
        }
        if self.db_pool_max_connections_per_role == 0 {
            return Err("DB_POOL_MAX_CONNECTIONS_PER_ROLE must be greater than zero");
        }
        if self.db_pool_max_connections_api_role_override == Some(0) {
            return Err("DB_POOL_MAX_CONNECTIONS_API_ROLE must be greater than zero when set");
        }
        if !(self.redis_url.starts_with("redis://") || self.redis_url.starts_with("rediss://")) {
            return Err("REDIS_URL must use redis:// or rediss://");
        }
        let valid_http_url = |value: &str| {
            value.parse::<axum::http::Uri>().is_ok_and(|url| {
                matches!(url.scheme_str(), Some("http" | "https"))
                    && url.authority().is_some_and(|authority| {
                        !authority.host().is_empty() && !authority.as_str().contains('@')
                    })
                    && url.query().is_none()
                    && !value.contains('#')
            })
        };
        if !valid_http_url(&self.public_api_base_url)
            || self.allowed_origins.is_empty()
            || !self
                .allowed_origins
                .iter()
                .all(|origin| valid_http_url(origin))
        {
            return Err("PUBLIC_API_BASE_URL and ALLOWED_ORIGINS must be absolute HTTP(S) URLs");
        }
        for (name, password) in [
            ("DB_API_ROLE_PASSWORD", &self.db_api_role_password),
            ("DB_AUTH_WORKER_PASSWORD", &self.db_auth_worker_password),
            (
                "DB_PAYMENT_WORKER_PASSWORD",
                &self.db_payment_worker_password,
            ),
            ("DB_SYSTEM_WORKER_PASSWORD", &self.db_system_worker_password),
            (
                "DB_FINANCE_WORKER_PASSWORD",
                &self.db_finance_worker_password,
            ),
        ] {
            if password.is_empty() || !password.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(match name {
                    "DB_API_ROLE_PASSWORD" => "DB_API_ROLE_PASSWORD must be a non-empty hex string",
                    "DB_AUTH_WORKER_PASSWORD" => {
                        "DB_AUTH_WORKER_PASSWORD must be a non-empty hex string"
                    }
                    "DB_PAYMENT_WORKER_PASSWORD" => {
                        "DB_PAYMENT_WORKER_PASSWORD must be a non-empty hex string"
                    }
                    "DB_SYSTEM_WORKER_PASSWORD" => {
                        "DB_SYSTEM_WORKER_PASSWORD must be a non-empty hex string"
                    }
                    _ => "DB_FINANCE_WORKER_PASSWORD must be a non-empty hex string",
                });
            }
        }
        if !(3..=5).contains(&self.consultation_trace_retention_days) {
            return Err("CONSULTATION_TRACE_RETENTION_DAYS must be between 3 and 5");
        }
        validate_startup_mode(
            Some(&self.environment),
            Some(if self.seed_dev_data { "true" } else { "false" }),
        )?;
        if self.dev_auth_otp_enabled && !self.is_development_or_test() {
            return Err("DEV_AUTH_OTP_ENABLED is forbidden outside development/test.");
        }
        if self.dev_simulation_enabled && self.environment == "production" {
            return Err("DEV_SIMULATION_ENABLED is forbidden in production.");
        }
        if self.recovery_mode
            && !valid_recovery_operator_digest(self.recovery_operator_token_sha256.as_deref())
        {
            return Err(
                "RECOVERY_OPERATOR_TOKEN_SHA256 must be a 64-character hex digest when RECOVERY_MODE=true.",
            );
        }
        if self.recovery_mode && self.recovery_instance_id.is_none() {
            return Err(
                "RECOVERY_INSTANCE_ID is required when RECOVERY_MODE=true, so the Remediation E1 \
                 reconciliation gate can bind its evidence to exactly one recovery incident.",
            );
        }
        validate_erasure_journal_configuration(self)?;
        validate_azure_email_configuration(self)?;
        if self.password_reset_enabled {
            let email_configured = self.azure_email_endpoint.is_some()
                && self.azure_email_access_key.is_some()
                && self.azure_email_sender.is_some();
            validate_password_reset_policy(
                true,
                email_configured,
                self.requires_https(),
                self.environment == "development",
            )?;
        }
        if self.requires_https() {
            let fcm_configured = self.fcm_project_id.is_some()
                && self.fcm_client_email.is_some()
                && self.fcm_private_key_b64.is_some();
            let fcm_any_configured = self.fcm_project_id.is_some()
                || self.fcm_client_email.is_some()
                || self.fcm_private_key_b64.is_some();
            if fcm_any_configured && !fcm_configured {
                return Err("FCM_PROJECT_ID, FCM_CLIENT_EMAIL and FCM_PRIVATE_KEY_B64 must be configured together.");
            }
            if !fcm_configured {
                return Err(
                    "FCM HTTP v1 push delivery must be configured outside development/test.",
                );
            }
            if self.trusted_proxy_ips.is_empty()
                || self
                    .trusted_proxy_ips
                    .iter()
                    .any(|ip| ip.is_unspecified() || ip.is_multicast())
            {
                return Err("EXTERNAL_TLS_TERMINATION_REQUIRED: configure exact TRUSTED_PROXY_IPS and restrict direct backend access.");
            }
            let https_url = |value: &str| {
                value.parse::<axum::http::Uri>().is_ok_and(|url| {
                    url.scheme_str() == Some("https")
                        && url.authority().is_some_and(|authority| {
                            !authority.host().is_empty() && !authority.as_str().contains('@')
                        })
                        && url.query().is_none()
                        && !value.contains('#')
                })
            };
            if !https_url(&self.public_api_base_url)
                || self.allowed_origins.is_empty()
                || !self.allowed_origins.iter().all(|origin| https_url(origin))
            {
                return Err("Deployed PUBLIC_API_BASE_URL and ALLOWED_ORIGINS must use HTTPS.");
            }
            if self.metrics_scrape_token.len() < 32
                || !self.metrics_scrape_token.is_ascii()
                || self
                    .metrics_scrape_token
                    .bytes()
                    .any(|b| b.is_ascii_whitespace())
            {
                return Err("METRICS_SCRAPE_TOKEN must be an explicit credential of at least 32 non-whitespace ASCII characters.");
            }
        }
        Ok(())
    }

    /// Builds the connection string for one of the five capability-role pools.
    /// Role passwords are generated with `openssl rand -hex 24` (hex-only), so
    /// they never need percent-encoding when embedded directly in the DSN.
    fn role_dsn(&self, role: &str, password: &str) -> String {
        format!(
            "postgres://{}:{}@{}:{}/{}",
            role, password, self.app_db_host, self.app_db_port, self.app_db_name
        )
    }

    pub fn api_role_dsn(&self) -> String {
        self.role_dsn("hellodoctor_api", &self.db_api_role_password)
    }
    pub fn auth_worker_dsn(&self) -> String {
        self.role_dsn("hellodoctor_auth_worker", &self.db_auth_worker_password)
    }
    pub fn payment_worker_dsn(&self) -> String {
        self.role_dsn(
            "hellodoctor_payment_worker",
            &self.db_payment_worker_password,
        )
    }
    pub fn system_worker_dsn(&self) -> String {
        self.role_dsn("hellodoctor_system_worker", &self.db_system_worker_password)
    }
    pub fn finance_worker_dsn(&self) -> String {
        self.role_dsn(
            "hellodoctor_finance_worker",
            &self.db_finance_worker_password,
        )
    }
}

/// Remediation E1: the externally-custodied erasure journal must be
/// configured whenever account erasure can durably happen (any staging or
/// production deployment, and any boot already in RECOVERY_MODE, since
/// reconciliation needs it to read what the journal proves). Development
/// and test may omit it; the outbox-sync pass then logs and retries rather
/// than silently pretending success (see `services::dr_reconciliation`).
fn validate_erasure_journal_configuration(config: &AppConfig) -> Result<(), &'static str> {
    let must_configure = config.requires_https() || config.recovery_mode;
    if !must_configure {
        return Ok(());
    }
    let path = config
        .external_erasure_journal_path
        .as_deref()
        .ok_or("EXTERNAL_ERASURE_JOURNAL_PATH is required outside development/test, and whenever RECOVERY_MODE=true.")?;
    if path.trim().is_empty() {
        return Err("EXTERNAL_ERASURE_JOURNAL_PATH must not be blank.");
    }
    let hmac_key = config
        .external_erasure_journal_hmac_key_b64
        .as_deref()
        .ok_or("EXTERNAL_ERASURE_JOURNAL_HMAC_KEY_B64 is required outside development/test, and whenever RECOVERY_MODE=true.")?;
    let anchor_key = config
        .external_erasure_journal_anchor_key_b64
        .as_deref()
        .ok_or("EXTERNAL_ERASURE_JOURNAL_ANCHOR_KEY_B64 is required outside development/test, and whenever RECOVERY_MODE=true.")?;
    if hmac_key == anchor_key {
        return Err(
            "EXTERNAL_ERASURE_JOURNAL_HMAC_KEY_B64 and _ANCHOR_KEY_B64 must be independently \
             custodied, distinct values, so a single leaked key cannot forge both the journal \
             and its checkpoint.",
        );
    }
    for (name, value) in [
        ("EXTERNAL_ERASURE_JOURNAL_HMAC_KEY_B64", hmac_key),
        ("EXTERNAL_ERASURE_JOURNAL_ANCHOR_KEY_B64", anchor_key),
    ] {
        let decoded = STANDARD.decode(value).map_err(|_| match name {
            "EXTERNAL_ERASURE_JOURNAL_HMAC_KEY_B64" => {
                "EXTERNAL_ERASURE_JOURNAL_HMAC_KEY_B64 must be valid base64."
            }
            _ => "EXTERNAL_ERASURE_JOURNAL_ANCHOR_KEY_B64 must be valid base64.",
        })?;
        if decoded.len() < 32 {
            return Err(match name {
                "EXTERNAL_ERASURE_JOURNAL_HMAC_KEY_B64" => {
                    "EXTERNAL_ERASURE_JOURNAL_HMAC_KEY_B64 must decode to at least 32 bytes."
                }
                _ => "EXTERNAL_ERASURE_JOURNAL_ANCHOR_KEY_B64 must decode to at least 32 bytes.",
            });
        }
    }
    Ok(())
}

fn valid_recovery_operator_digest(value: Option<&str>) -> bool {
    value.is_some_and(|value| {
        value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}

fn validate_startup_mode(
    environment: Option<&str>,
    seed: Option<&str>,
) -> Result<(String, bool), &'static str> {
    let environment = environment.map(str::trim).ok_or(
        "ENVIRONMENT must be explicitly set to development, test, staging, or production.",
    )?;
    if !matches!(
        environment,
        "development" | "test" | "staging" | "production"
    ) {
        return Err(
            "ENVIRONMENT must be explicitly set to development, test, staging, or production.",
        );
    }
    let seed = match seed.map(str::trim).unwrap_or("false") {
        "true" => true,
        "false" => false,
        _ => return Err("SEED_DEV_DATA must be true or false."),
    };
    if seed && !matches!(environment, "development" | "test") {
        return Err("SEED_DEV_DATA=true is forbidden outside development/test.");
    }
    Ok((environment.to_owned(), seed))
}

/// Azure Communication Services Email is the only email provider this
/// codebase integrates with (no SMTP fallback). `access_key` (HMAC-SHA256
/// signed requests, see `services::email_transport::AcsEmailTransport`) is
/// the only implemented auth mode.
///
/// `managed_identity` is recognized here — not silently ignored or treated
/// as a typo — so a misconfigured deployment fails at startup with an exact
/// explanation rather than silently falling back to `access_key` or an
/// unauthenticated transport. It is deliberately **rejected outright**: the
/// exact Microsoft Entra ID / managed-identity REST contract for the ACS
/// Email data-plane `/emails:send` endpoint (the required OAuth scope, the
/// RBAC role, and whether a bare `Authorization: Bearer` header is accepted
/// on this specific endpoint by a hand-rolled REST client rather than an
/// official SDK's credential pipeline) could not be verified against
/// official Microsoft documentation during this phase. Microsoft Entra ID
/// authentication is confirmed supported for the Email service as a
/// category, but every worked example found uses an SDK's
/// `DefaultAzureCredential`, never a documented raw REST contract.
/// Implementing a guessed wire format for an authentication path would be
/// worse than not having it: a subtly wrong signing/header scheme fails
/// exactly like a correct one hitting a transient network error, which this
/// codebase's own retry classification would then misclassify as
/// `Retryable` and retry forever instead of surfacing a clear
/// misconfiguration. This function keeps that decision enforced in code,
/// not just in a comment.
fn validate_azure_email_configuration(config: &AppConfig) -> Result<(), &'static str> {
    let auth_mode = config.azure_email_auth_mode.trim().to_ascii_lowercase();
    match auth_mode.as_str() {
        "access_key" => {}
        "managed_identity" => {
            return Err(
                "AZURE_EMAIL_AUTH_MODE=managed_identity is not yet implemented: the Microsoft \
                 Entra ID / managed-identity REST auth contract for the Azure Communication \
                 Services Email data-plane endpoint could not be verified against official \
                 documentation. Use access_key.",
            );
        }
        _ => {
            return Err("AZURE_EMAIL_AUTH_MODE must be access_key or managed_identity.");
        }
    }
    let any_configured = config.azure_email_endpoint.is_some()
        || config.azure_email_access_key.is_some()
        || config.azure_email_sender.is_some();
    let fully_configured = config.azure_email_endpoint.is_some()
        && config.azure_email_access_key.is_some()
        && config.azure_email_sender.is_some();
    if any_configured && !fully_configured {
        return Err(
            "AZURE_EMAIL_ENDPOINT, AZURE_EMAIL_ACCESS_KEY and AZURE_EMAIL_SENDER must be configured together.",
        );
    }
    if config.requires_https() && !fully_configured {
        return Err(
            "Azure Communication Services Email must be configured outside development/test \
             (Patient, Doctor and Admin login all depend on it to complete MFA).",
        );
    }
    Ok(())
}

/// Phase 2: Secure Email OTP Password Recovery is now implemented
/// end-to-end (see `services::password_reset_service`) and covered by the
/// security/regression test suite required before this guard could be
/// relaxed at all — see `tests/cases/password_reset.rs`. It remains
/// fail-closed in every environment that `requires_https()` (staging,
/// production): this codebase has never completed a real Azure
/// Communication Services delivery acceptance test (see
/// `EMAIL_OUTBOX_RECOVERY_POLICY.md` and the Phase 0 closure notes), and
/// enabling self-service account recovery in production ahead of that
/// verification — on an unexercised, novel attack surface — is a
/// deliberately separate decision this change does not make unilaterally.
/// Development and test may enable it once email delivery is otherwise
/// configured (or bypassed via `DEV_AUTH_OTP_ENABLED`, same as every other
/// OTP flow).
fn validate_password_reset_policy(
    enabled: bool,
    email_configured: bool,
    requires_https: bool,
    is_development: bool,
) -> Result<(), &'static str> {
    if !enabled {
        return Ok(());
    }
    if requires_https {
        return Err(
            "PASSWORD_RESET_ENABLED cannot be enabled in staging/production until a real Azure \
             Communication Services delivery acceptance test has been completed and separately approved.",
        );
    }
    // Mirrors the exact condition `auth_service`'s own login-OTP paths use
    // (`environment != "development"`): only `development` may rely on the
    // `DEV_AUTH_OTP_ENABLED` bypass instead of real email configuration —
    // `test` must still configure (even if fake/no-op) email delivery,
    // since the integration suite exercises the real send path.
    if !is_development && !email_configured {
        return Err(
            "PASSWORD_RESET_ENABLED requires AZURE_EMAIL_ENDPOINT, AZURE_EMAIL_ACCESS_KEY and AZURE_EMAIL_SENDER \
             outside development.",
        );
    }
    Ok(())
}

fn valid_storage_name(value: &str, max_len: usize) -> bool {
    (3..=max_len).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && !value.starts_with('-')
        && !value.ends_with('-')
        && !value.contains("--")
}

fn validate_storage_configuration(config: &AppConfig) -> Result<(), &'static str> {
    match config.storage_provider {
        StorageProvider::Disabled => {
            if config.requires_https() {
                return Err("STORAGE_PROVIDER must be azure outside development/test.");
            }
        }
        StorageProvider::LegacyS3 => {
            if config.requires_https() {
                return Err("STORAGE_PROVIDER must be azure outside development/test.");
            }
            if config.storage_endpoint.is_empty()
                || config.storage_access_key.is_empty()
                || config.storage_secret_key.is_empty()
                || STANDARD
                    .decode(&config.storage_encryption_key_b64)
                    .map_or(true, |key| key.len() != 32)
            {
                return Err(
                    "Legacy S3 storage requires valid endpoint/credentials and a 32-byte STORAGE_ENCRYPTION_KEY_B64.",
                );
            }
        }
        StorageProvider::Azure => {
            let account = config
                .azure_storage_account_name
                .as_deref()
                .ok_or("AZURE_STORAGE_ACCOUNT_NAME is required when STORAGE_PROVIDER=azure.")?;
            let clinical = config.azure_storage_clinical_container.as_deref().ok_or(
                "AZURE_STORAGE_CLINICAL_CONTAINER is required when STORAGE_PROVIDER=azure.",
            )?;
            let article = config.azure_storage_article_container.as_deref().ok_or(
                "AZURE_STORAGE_ARTICLE_CONTAINER is required when STORAGE_PROVIDER=azure.",
            )?;
            if !valid_storage_name(account, 24) {
                return Err("AZURE_STORAGE_ACCOUNT_NAME must be 3-24 lowercase letters or digits.");
            }
            if !valid_storage_name(clinical, 63) || !valid_storage_name(article, 63) {
                return Err("Azure container names must be 3-63 lowercase letters, digits or single hyphens.");
            }
            if clinical == article {
                return Err("Azure clinical and article containers must be distinct.");
            }
            if config.storage_encryption_key_b64.is_empty()
                || STANDARD
                    .decode(&config.storage_encryption_key_b64)
                    .map_or(true, |key| key.len() != 32)
            {
                return Err(
                    "STORAGE_ENCRYPTION_KEY_B64 must decode to 32 bytes when Azure storage is enabled.",
                );
            }
            let auth_mode = config.azure_storage_auth_mode.trim().to_ascii_lowercase();
            match auth_mode.as_str() {
                "managed_identity" => {
                    if config.azure_storage_account_key.is_some() {
                        return Err(
                            "AZURE_STORAGE_ACCOUNT_KEY must be empty in managed_identity mode.",
                        );
                    }
                }
                "account_key" => {
                    if config.azure_storage_account_key.is_none() {
                        return Err("AZURE_STORAGE_ACCOUNT_KEY is required in account_key mode.");
                    }
                    if config.azure_storage_client_id.is_some() {
                        return Err("AZURE_STORAGE_CLIENT_ID must be empty in account_key mode.");
                    }
                    if config.requires_https()
                        && !config.azure_storage_account_key_risk_acknowledged
                    {
                        return Err(
                            "AZURE_STORAGE_AUTH_MODE=account_key is rejected in staging/production \
                             unless AZURE_STORAGE_ACCOUNT_KEY_RISK_ACKNOWLEDGED=true is explicitly \
                             set; use managed_identity instead (see docs/AZURE_BLOB_SETUP.md).",
                        );
                    }
                }
                "azurite" => {
                    if !config.azure_storage_use_emulator {
                        return Err("azurite auth mode requires AZURE_STORAGE_USE_EMULATOR=true.");
                    }
                }
                _ => {
                    return Err(
                        "AZURE_STORAGE_AUTH_MODE must be managed_identity, account_key or azurite.",
                    )
                }
            }
            if config.azure_storage_use_emulator {
                if !config.is_development_or_test() {
                    return Err(
                        "AZURE_STORAGE_USE_EMULATOR is forbidden outside development/test.",
                    );
                }
                if auth_mode != "azurite" {
                    return Err(
                        "AZURE_STORAGE_USE_EMULATOR requires AZURE_STORAGE_AUTH_MODE=azurite.",
                    );
                }
            } else if auth_mode == "azurite" {
                return Err(
                    "AZURE_STORAGE_AUTH_MODE=azurite requires AZURE_STORAGE_USE_EMULATOR=true.",
                );
            }
            if let Some(endpoint) = config.azure_storage_endpoint.as_deref() {
                let valid = endpoint.parse::<axum::http::Uri>().is_ok_and(|url| {
                    matches!(url.scheme_str(), Some("http" | "https"))
                        && url.authority().is_some_and(|authority| {
                            !authority.host().is_empty() && !authority.as_str().contains('@')
                        })
                        && url.path() == "/"
                        && url.query().is_none()
                        && !endpoint.contains('#')
                });
                if !valid {
                    return Err("AZURE_STORAGE_ENDPOINT must be an absolute HTTP(S) origin.");
                }
                if !config.is_development_or_test() && !endpoint.starts_with("https://") {
                    return Err("AZURE_STORAGE_ENDPOINT must use HTTPS outside development/test.");
                }
                if config.azure_storage_use_emulator
                    && !endpoint.starts_with("http://127.0.0.1")
                    && !endpoint.starts_with("http://localhost")
                {
                    return Err("Azurite endpoint must be loopback-only.");
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod startup_mode_tests {
    use super::{
        valid_recovery_operator_digest, valid_storage_name, validate_password_reset_policy,
        validate_startup_mode, FinancialModuleMode, StorageProvider,
    };

    #[test]
    fn missing_or_unrecognized_environment_is_rejected() {
        for environment in [None, Some(""), Some(" "), Some("prod"), Some("Development")] {
            assert!(validate_startup_mode(environment, None).is_err());
        }
    }

    #[test]
    fn explicit_modes_are_accepted_without_development_seed() {
        for environment in ["development", "test", "staging", "production"] {
            assert_eq!(
                validate_startup_mode(Some(environment), None).unwrap(),
                (environment.into(), false)
            );
            assert_eq!(
                validate_startup_mode(Some(environment), Some("false")).unwrap(),
                (environment.into(), false)
            );
        }
    }

    #[test]
    fn development_seed_cannot_escape_development_or_test() {
        for environment in ["staging", "production"] {
            assert!(validate_startup_mode(Some(environment), Some("true")).is_err());
        }
        for environment in ["development", "test"] {
            assert_eq!(
                validate_startup_mode(Some(environment), Some("true")).unwrap(),
                (environment.into(), true)
            );
        }
        assert!(validate_startup_mode(Some("development"), Some("yes")).is_err());
    }

    #[test]
    fn password_reset_is_disabled_by_default() {
        assert!(validate_password_reset_policy(false, false, false, false).is_ok());
        assert!(validate_password_reset_policy(false, true, false, false).is_ok());
    }

    #[test]
    fn password_reset_always_fails_closed_in_staging_or_production() {
        // Even with email fully configured, production/staging must never
        // accept PASSWORD_RESET_ENABLED=true — Phase 2's implementation has
        // no completed real Azure Communication Services delivery
        // acceptance test, so enabling it there remains a deliberately
        // separate, unapproved decision.
        assert!(validate_password_reset_policy(true, true, true, false)
            .expect_err("production/staging must stay disabled regardless of email config")
            .contains("staging/production"));
    }

    #[test]
    fn password_reset_in_test_environment_requires_real_email_configuration() {
        assert!(validate_password_reset_policy(true, false, false, false)
            .expect_err("missing email configuration must fail outside development")
            .contains("AZURE_EMAIL"));
        assert!(validate_password_reset_policy(true, true, false, false).is_ok());
    }

    #[test]
    fn password_reset_in_development_may_rely_on_the_otp_bypass_instead_of_email_config() {
        assert!(validate_password_reset_policy(true, false, false, true).is_ok());
        assert!(validate_password_reset_policy(true, true, false, true).is_ok());
    }

    #[test]
    fn storage_provider_selection_is_explicit() {
        assert_eq!(
            StorageProvider::parse("disabled"),
            Ok(StorageProvider::Disabled)
        );
        assert_eq!(StorageProvider::parse("azure"), Ok(StorageProvider::Azure));
        assert_eq!(
            StorageProvider::parse("legacy_s3"),
            Ok(StorageProvider::LegacyS3)
        );
        assert!(StorageProvider::parse("minio").is_err());
    }

    #[test]
    fn financial_module_mode_selection_is_explicit() {
        assert_eq!(
            FinancialModuleMode::parse("enabled"),
            Ok(FinancialModuleMode::Enabled)
        );
        assert_eq!(
            FinancialModuleMode::parse("deferred"),
            Ok(FinancialModuleMode::Deferred)
        );
        assert_eq!(
            FinancialModuleMode::parse("ENABLED"),
            Ok(FinancialModuleMode::Enabled)
        );
        assert!(FinancialModuleMode::parse("off").is_err());
        assert!(FinancialModuleMode::parse("").is_err());
    }

    #[test]
    fn azure_names_follow_storage_service_constraints() {
        assert!(valid_storage_name("hellodoctor", 24));
        assert!(valid_storage_name("article-media", 63));
        assert!(!valid_storage_name("HelloDoctor", 24));
        assert!(!valid_storage_name("article--media", 63));
        assert!(!valid_storage_name("-clinical", 63));
    }

    #[test]
    fn recovery_operator_digest_is_required_and_secret_free() {
        assert!(!valid_recovery_operator_digest(None));
        assert!(!valid_recovery_operator_digest(Some("operator-token")));
        assert!(valid_recovery_operator_digest(Some(&"a".repeat(64))));
    }
}

/// Remediation E3 regression coverage: Azure account-key gating and the
/// hand-written `Debug` redaction. `synthetic_config` fills in every field
/// with a recognizable, obviously-synthetic value (`SYNTH_*` for secrets) so
/// a test can assert those exact strings never reach `{:?}` output without
/// touching real credentials.
#[cfg(test)]
pub(crate) mod remediation_e3_tests {
    use super::{
        validate_azure_email_configuration, validate_storage_configuration, AppConfig,
        FinancialModuleMode, StorageProvider, STANDARD,
    };
    use base64::Engine as _;

    /// `pub(crate)` so other modules' tests (e.g.
    /// `services::email_transport::tests`) can build a valid `AppConfig`
    /// without duplicating this large fixture — see
    /// `email_transport::tests::configuring_azure_email_yields_the_real_transport_ready_to_send_templates`.
    pub(crate) fn synthetic_config() -> AppConfig {
        let encryption_key = STANDARD.encode([0x11u8; 32]);
        AppConfig {
            port: 8080,
            host: "0.0.0.0".into(),
            database_url: "postgres://user:SYNTH_DB_URL_SECRET_abc123@localhost/db".into(),
            app_db_host: "localhost".into(),
            app_db_port: 5432,
            app_db_name: "hellodoctor".into(),
            db_api_role_password: "SYNTH_API_PW_111111".into(),
            db_auth_worker_password: "SYNTH_AUTH_PW_222222".into(),
            db_payment_worker_password: "SYNTH_PAYMENT_PW_333333".into(),
            db_system_worker_password: "SYNTH_SYSTEM_PW_444444".into(),
            db_finance_worker_password: "SYNTH_FINANCE_PW_555555".into(),
            db_pool_max_connections_per_role: 10,
            db_pool_max_connections_api_role_override: None,
            payment_reservation_window_secs: 600,
            max_active_users: 1200,
            consultation_trace_retention_days: 5,
            global_ip_rate_limit_per_min: 120,
            redis_url: "redis://:SYNTH_REDIS_PW_666666@localhost:6379".into(),
            jwt_ed25519_private_key_b64: "SYNTH_JWT_PRIVATE_777777".into(),
            jwt_ed25519_public_key_b64: "PUBLIC_KEY_VISIBLE_888888".into(),
            totp_encryption_key_b64: "SYNTH_TOTP_KEY_999999".into(),
            agora_app_id: "agora-app-id-visible".into(),
            agora_app_certificate: "SYNTH_AGORA_CERT_AAAAAA".into(),
            agora_token_window_before_mins: 15,
            agora_token_window_after_mins: 60,
            consultation_live_reservation_max_secs: 21_600,
            webhook_hmac_secret: "SYNTH_WEBHOOK_HMAC_BBBBBB".into(),
            agora_webhook_hmac_secret: "SYNTH_AGORA_WEBHOOK_HMAC_CCCCCC".into(),
            storage_bucket: "helodoc-medical-records".into(),
            storage_endpoint: "https://example-endpoint.invalid".into(),
            storage_access_key: "SYNTH_STORAGE_ACCESS_KEY_DDDDDD".into(),
            storage_secret_key: "SYNTH_STORAGE_SECRET_KEY_EEEEEE".into(),
            storage_encryption_key_b64: encryption_key,
            storage_provider: StorageProvider::Azure,
            azure_storage_account_name: Some("hellodoctor".into()),
            azure_storage_auth_mode: "account_key".into(),
            azure_storage_clinical_container: Some("clinical-private".into()),
            azure_storage_article_container: Some("article-media".into()),
            azure_storage_account_key: Some("SYNTH_AZURE_ACCOUNT_KEY_GGGGGG".into()),
            azure_storage_client_id: None,
            azure_storage_endpoint: Some("https://hellodoctor.blob.core.windows.net".into()),
            azure_storage_use_emulator: false,
            azure_storage_account_key_risk_acknowledged: true,
            public_api_base_url: "https://api.example.invalid".into(),
            financial_module_mode: FinancialModuleMode::Enabled,
            environment: "production".into(),
            seed_dev_data: false,
            dev_auth_otp_enabled: false,
            dev_simulation_enabled: false,
            password_reset_enabled: false,
            recovery_mode: false,
            recovery_operator_token_sha256: None,
            recovery_instance_id: None,
            external_erasure_journal_path: Some("/var/lib/hellodoctor/erasure.journal".into()),
            external_erasure_journal_hmac_key_b64: Some("SYNTH_ERASURE_HMAC_KEY_HHHHHH".into()),
            external_erasure_journal_anchor_key_b64: Some("SYNTH_ERASURE_ANCHOR_KEY_IIIIII".into()),
            external_erasure_journal_key_version: "v1".into(),
            trusted_proxy_ips: vec!["10.0.0.1".parse().unwrap()],
            metrics_scrape_token: "SYNTH_METRICS_TOKEN_JJJJJJ".into(),
            allowed_origins: vec!["https://app.example.invalid".into()],
            doctor_daily_summary_time_local: "07:00".into(),
            engagement_greeting_time_local: "08:00".into(),
            azure_email_endpoint: Some("https://example.communication.azure.com".into()),
            azure_email_access_key: Some("SYNTH_AZURE_EMAIL_KEY_KKKKKK".into()),
            azure_email_sender: Some("no-reply@example.invalid".into()),
            azure_email_auth_mode: "access_key".into(),
            fcm_project_id: Some("example-project".into()),
            fcm_client_email: Some("fcm@example.iam.gserviceaccount.com".into()),
            fcm_private_key_b64: Some("SYNTH_FCM_PRIVATE_KEY_LLLLLL".into()),
            email_asset_base_url: None,
            email_patient_app_url: Some("https://app.hellodoctor.asia".into()),
            email_doctor_portal_url: Some("https://doctor.hellodoctor.asia".into()),
        }
    }

    #[test]
    fn debug_format_redacts_all_known_secret_fields() {
        let cfg = synthetic_config();
        let formatted = format!("{cfg:?}");
        let secrets = [
            "SYNTH_DB_URL_SECRET_abc123",
            "SYNTH_API_PW_111111",
            "SYNTH_AUTH_PW_222222",
            "SYNTH_PAYMENT_PW_333333",
            "SYNTH_SYSTEM_PW_444444",
            "SYNTH_FINANCE_PW_555555",
            "SYNTH_REDIS_PW_666666",
            "SYNTH_JWT_PRIVATE_777777",
            "SYNTH_TOTP_KEY_999999",
            "SYNTH_AGORA_CERT_AAAAAA",
            "SYNTH_WEBHOOK_HMAC_BBBBBB",
            "SYNTH_AGORA_WEBHOOK_HMAC_CCCCCC",
            "SYNTH_STORAGE_ACCESS_KEY_DDDDDD",
            "SYNTH_STORAGE_SECRET_KEY_EEEEEE",
            "SYNTH_AZURE_ACCOUNT_KEY_GGGGGG",
            "SYNTH_ERASURE_HMAC_KEY_HHHHHH",
            "SYNTH_ERASURE_ANCHOR_KEY_IIIIII",
            "SYNTH_METRICS_TOKEN_JJJJJJ",
            "SYNTH_AZURE_EMAIL_KEY_KKKKKK",
            "SYNTH_FCM_PRIVATE_KEY_LLLLLL",
        ];
        for secret in secrets {
            assert!(
                !formatted.contains(secret),
                "Debug output leaked secret marker: {secret}"
            );
        }
        // The storage encryption key is derived (base64 of 32 zero-ish
        // bytes) rather than a literal marker string, but must still be
        // redacted rather than printed in full.
        assert!(!formatted.contains(&cfg.storage_encryption_key_b64));
        // Non-secret diagnostic fields remain visible for operability.
        assert!(formatted.contains("PUBLIC_KEY_VISIBLE_888888"));
        assert!(formatted.contains("agora-app-id-visible"));
        assert!(formatted.contains("production"));
        assert!(formatted.contains("<redacted>"));
    }

    #[test]
    fn account_key_mode_is_rejected_in_production_without_acknowledgment() {
        let cfg = AppConfig {
            environment: "production".into(),
            azure_storage_account_key_risk_acknowledged: false,
            ..synthetic_config()
        };
        let error = validate_storage_configuration(&cfg)
            .expect_err("unacknowledged account_key must be rejected in production");
        assert!(error.contains("ACCOUNT_KEY_RISK_ACKNOWLEDGED"));
    }

    #[test]
    fn account_key_mode_is_rejected_in_staging_without_acknowledgment() {
        let cfg = AppConfig {
            environment: "staging".into(),
            azure_storage_account_key_risk_acknowledged: false,
            ..synthetic_config()
        };
        assert!(validate_storage_configuration(&cfg).is_err());
    }

    #[test]
    fn account_key_mode_is_allowed_in_production_with_explicit_acknowledgment() {
        let cfg = AppConfig {
            environment: "production".into(),
            azure_storage_account_key_risk_acknowledged: true,
            ..synthetic_config()
        };
        assert!(validate_storage_configuration(&cfg).is_ok());
    }

    #[test]
    fn account_key_mode_in_development_is_unaffected_by_the_acknowledgment_flag() {
        let cfg = AppConfig {
            environment: "development".into(),
            azure_storage_account_key_risk_acknowledged: false,
            ..synthetic_config()
        };
        assert!(validate_storage_configuration(&cfg).is_ok());
    }

    #[test]
    fn managed_identity_mode_is_never_gated_by_the_acknowledgment_flag() {
        let cfg = AppConfig {
            environment: "production".into(),
            azure_storage_auth_mode: "managed_identity".into(),
            azure_storage_account_key: None,
            azure_storage_account_key_risk_acknowledged: false,
            ..synthetic_config()
        };
        assert!(validate_storage_configuration(&cfg).is_ok());
    }

    #[test]
    fn acknowledgment_flag_does_not_bypass_unrelated_azure_checks() {
        // Even with the flag set, an invalid encryption key must still fail.
        let cfg = AppConfig {
            environment: "production".into(),
            azure_storage_account_key_risk_acknowledged: true,
            storage_encryption_key_b64: "not-valid-base64!!".into(),
            ..synthetic_config()
        };
        assert!(validate_storage_configuration(&cfg).is_err());

        // Nor does it allow the clinical and article containers to collide.
        let cfg = AppConfig {
            environment: "production".into(),
            azure_storage_account_key_risk_acknowledged: true,
            azure_storage_article_container: Some("clinical-private".into()),
            ..synthetic_config()
        };
        assert!(validate_storage_configuration(&cfg).is_err());
    }

    /// AfyaMkononi/HelloDoctor.Asia production Azure Blob audit: confirms the
    /// exact production account/container/auth shape (managed identity, a
    /// `clinical-files`-style clinical container, a distinct article-media
    /// container) satisfies `validate_storage_configuration` with no code
    /// changes - the container name itself is operator-supplied configuration
    /// (`AZURE_STORAGE_CLINICAL_CONTAINER`), never hardcoded.
    #[test]
    fn production_clinical_container_with_managed_identity_is_valid() {
        let cfg = AppConfig {
            environment: "production".into(),
            azure_storage_account_name: Some("stafyamkononiprod01".into()),
            azure_storage_auth_mode: "managed_identity".into(),
            azure_storage_clinical_container: Some("clinical-files".into()),
            azure_storage_article_container: Some("article-media".into()),
            azure_storage_account_key: None,
            azure_storage_account_key_risk_acknowledged: false,
            azure_storage_endpoint: Some(
                "https://stafyamkononiprod01.blob.core.windows.net".into(),
            ),
            ..synthetic_config()
        };
        assert!(validate_storage_configuration(&cfg).is_ok());

        // The same account name is rejected if it ever exceeds the Storage
        // service's 3-24 character limit or contains anything but lowercase
        // letters/digits - "stafyamkononiprod01" (19 chars) is comfortably
        // within that limit today, but this guards against a future rename
        // drifting out of range unnoticed.
        let too_long = AppConfig {
            azure_storage_account_name: Some("a".repeat(25)),
            ..cfg
        };
        assert!(validate_storage_configuration(&too_long).is_err());
    }

    /// Both production containers now exist in Azure
    /// (clinical-files + article-media on stafyamkononiprod01); this closes
    /// the one coverage gap `production_clinical_container_with_managed_identity_is_valid`
    /// didn't exercise: a missing article container must still fail closed at
    /// startup even when every other production value (account, clinical
    /// container, managed identity) is otherwise correct - it is not enough
    /// for the clinical container alone to validate.
    #[test]
    fn missing_article_container_fails_startup_validation() {
        let cfg = AppConfig {
            environment: "production".into(),
            azure_storage_account_name: Some("stafyamkononiprod01".into()),
            azure_storage_auth_mode: "managed_identity".into(),
            azure_storage_clinical_container: Some("clinical-files".into()),
            azure_storage_article_container: None,
            azure_storage_account_key: None,
            azure_storage_account_key_risk_acknowledged: false,
            ..synthetic_config()
        };
        assert_eq!(
            validate_storage_configuration(&cfg),
            Err("AZURE_STORAGE_ARTICLE_CONTAINER is required when STORAGE_PROVIDER=azure.")
        );
    }

    /// `azure_names_follow_storage_service_constraints` already unit-tests
    /// `valid_storage_name` directly; this test instead proves an invalid
    /// name is caught through the actual `validate_storage_configuration`
    /// path a misconfigured production deployment would hit, using the
    /// `clinical-files` container name with one character capitalized as the
    /// realistic typo this guards against.
    #[test]
    fn invalid_container_name_is_rejected_through_full_validation() {
        let cfg = AppConfig {
            environment: "production".into(),
            azure_storage_account_name: Some("stafyamkononiprod01".into()),
            azure_storage_auth_mode: "managed_identity".into(),
            azure_storage_clinical_container: Some("Clinical-Files".into()),
            azure_storage_article_container: Some("article-media".into()),
            azure_storage_account_key: None,
            azure_storage_account_key_risk_acknowledged: false,
            ..synthetic_config()
        };
        assert_eq!(
            validate_storage_configuration(&cfg),
            Err("Azure container names must be 3-63 lowercase letters, digits or single hyphens.")
        );
    }

    #[test]
    fn managed_identity_auth_mode_is_rejected_as_unimplemented() {
        let cfg = AppConfig {
            azure_email_auth_mode: "managed_identity".into(),
            ..synthetic_config()
        };
        let error = validate_azure_email_configuration(&cfg)
            .expect_err("managed_identity must be rejected until its REST contract is verified");
        assert!(error.contains("not yet implemented"));
    }

    #[test]
    fn unrecognized_auth_mode_is_rejected() {
        let cfg = AppConfig {
            azure_email_auth_mode: "sas_token".into(),
            ..synthetic_config()
        };
        assert_eq!(
            validate_azure_email_configuration(&cfg),
            Err("AZURE_EMAIL_AUTH_MODE must be access_key or managed_identity.")
        );
    }

    #[test]
    fn partial_azure_email_configuration_is_rejected_in_every_environment() {
        for environment in ["development", "test", "staging", "production"] {
            let cfg = AppConfig {
                environment: environment.into(),
                azure_email_access_key: None,
                ..synthetic_config()
            };
            assert_eq!(
                validate_azure_email_configuration(&cfg),
                Err(
                    "AZURE_EMAIL_ENDPOINT, AZURE_EMAIL_ACCESS_KEY and AZURE_EMAIL_SENDER must be configured together."
                ),
                "environment={environment}"
            );
        }
    }

    #[test]
    fn azure_email_is_required_outside_development_and_test() {
        for environment in ["staging", "production"] {
            let cfg = AppConfig {
                environment: environment.into(),
                azure_email_endpoint: None,
                azure_email_access_key: None,
                azure_email_sender: None,
                ..synthetic_config()
            };
            let error = validate_azure_email_configuration(&cfg)
                .expect_err("email must be required outside development/test");
            assert!(
                error.contains("must be configured"),
                "environment={environment}"
            );
        }
        for environment in ["development", "test"] {
            let cfg = AppConfig {
                environment: environment.into(),
                azure_email_endpoint: None,
                azure_email_access_key: None,
                azure_email_sender: None,
                ..synthetic_config()
            };
            assert!(
                validate_azure_email_configuration(&cfg).is_ok(),
                "environment={environment}"
            );
        }
    }

    #[test]
    fn fully_configured_access_key_mode_is_valid_in_every_environment() {
        for environment in ["development", "test", "staging", "production"] {
            let cfg = AppConfig {
                environment: environment.into(),
                ..synthetic_config()
            };
            assert!(
                validate_azure_email_configuration(&cfg).is_ok(),
                "environment={environment}"
            );
        }
    }
}
