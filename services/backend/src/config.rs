use std::env;

#[derive(Clone, Debug)]
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
    /// Controlled admission cap on NEW user activation only (patient
    /// self-registration, admin-created doctor accounts) — existing users'
    /// sessions, bookings, and consultations are never affected once this is
    /// reached. An operational dial, not an architectural ceiling: raising it
    /// is a config change, not a redesign. See docs/CHECKPOINT.md for the
    /// evidence-gated process for actually raising it in a real deployment.
    pub max_active_users: u32,
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
    pub webhook_hmac_secret: String,
    pub storage_bucket: String,
    pub storage_endpoint: String,
    pub storage_access_key: String,
    pub storage_secret_key: String,
    pub storage_encryption_key_b64: String,
    pub public_api_base_url: String,
    pub environment: String,
    pub seed_dev_data: bool,
    pub dev_auth_otp_enabled: bool,
    pub dev_simulation_enabled: bool,
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
}

impl AppConfig {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();

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

        // No implicit local storage installation or development credentials.
        let storage_setting = |name: &str| -> String {
            if environment == "development" {
                env::var(name).unwrap_or_default()
            } else {
                require(name)
            }
        };

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
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            database_url: require("DATABASE_URL"),
            app_db_host: env::var("APP_DB_HOST").unwrap_or_else(|_| "localhost".into()),
            app_db_port: env::var("APP_DB_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(5432),
            app_db_name: env::var("APP_DB_NAME").unwrap_or_else(|_| "hellodoctor".into()),
            db_api_role_password: require("DB_API_ROLE_PASSWORD"),
            db_auth_worker_password: require("DB_AUTH_WORKER_PASSWORD"),
            db_payment_worker_password: require("DB_PAYMENT_WORKER_PASSWORD"),
            db_system_worker_password: require("DB_SYSTEM_WORKER_PASSWORD"),
            db_finance_worker_password: require("DB_FINANCE_WORKER_PASSWORD"),
            db_pool_max_connections_per_role: env::var("DB_POOL_MAX_CONNECTIONS_PER_ROLE")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(10),
            max_active_users: env::var("MAX_ACTIVE_USERS")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(1200),
            global_ip_rate_limit_per_min: env::var("GLOBAL_IP_RATE_LIMIT_PER_MIN")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(120),
            redis_url: require("REDIS_URL"),
            jwt_ed25519_private_key_b64: require("JWT_ED25519_PRIVATE_KEY_B64"),
            jwt_ed25519_public_key_b64: require("JWT_ED25519_PUBLIC_KEY_B64"),
            totp_encryption_key_b64: require("TOTP_ENCRYPTION_KEY_B64"),
            agora_app_id: require("AGORA_APP_ID"),
            agora_app_certificate: require("AGORA_APP_CERTIFICATE"),
            agora_token_window_before_mins: env::var("AGORA_TOKEN_WINDOW_BEFORE_MINS")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(15),
            agora_token_window_after_mins: env::var("AGORA_TOKEN_WINDOW_AFTER_MINS")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(60),
            webhook_hmac_secret: require("WEBHOOK_HMAC_SECRET"),
            storage_bucket: env::var("STORAGE_BUCKET")
                .unwrap_or_else(|_| "helodoc-medical-records".into()),
            storage_endpoint: storage_setting("STORAGE_ENDPOINT"),
            storage_access_key: storage_setting("STORAGE_ACCESS_KEY"),
            storage_secret_key: storage_setting("STORAGE_SECRET_KEY"),
            storage_encryption_key_b64: storage_setting("STORAGE_ENCRYPTION_KEY_B64"),
            public_api_base_url: env::var("PUBLIC_API_BASE_URL").unwrap_or_else(|_| {
                if environment == "development" {
                    "http://127.0.0.1:8080".into()
                } else {
                    require("PUBLIC_API_BASE_URL")
                }
            }),
            environment,
            seed_dev_data,
            dev_auth_otp_enabled: boolean("DEV_AUTH_OTP_ENABLED"),
            dev_simulation_enabled: boolean("DEV_SIMULATION_ENABLED"),
            trusted_proxy_ips,
            metrics_scrape_token: env::var("METRICS_SCRAPE_TOKEN").unwrap_or_default(),
            allowed_origins,
            doctor_daily_summary_time_local: env::var("DOCTOR_DAILY_SUMMARY_TIME_LOCAL")
                .unwrap_or_else(|_| "07:00".into()),
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

    pub fn simulations_allowed(&self) -> bool {
        self.dev_simulation_enabled
            && matches!(
                self.environment.as_str(),
                "development" | "test" | "staging"
            )
    }

    /// Called before any database/Redis connection; tests also exercise it directly.
    pub fn validate_public_exposure(&self) -> Result<(), &'static str> {
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
        if self.requires_https() {
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

#[cfg(test)]
mod startup_mode_tests {
    use super::validate_startup_mode;

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
}
