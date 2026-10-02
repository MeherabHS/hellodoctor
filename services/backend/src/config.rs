use std::env;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub port: u16,
    pub host: String,
    pub database_url: String,
    pub jwt_ed25519_private_key_b64: String,
    pub jwt_ed25519_public_key_b64: String,
    pub totp_encryption_key_b64: String,
    pub agora_app_id: String,
    pub agora_app_certificate: String,
    pub storage_bucket: String,
    pub environment: String,
    pub allowed_origins: Vec<String>,
}

/// Development-only fallback values. These NEVER apply outside `ENVIRONMENT=development`;
/// any other environment missing these variables fails fast at startup instead of booting insecurely.
mod dev_defaults {
    pub const JWT_PRIVATE_KEY_B64: &str = "MC4CAQAwBQYDK2VwBCIEIAT/WOtPd4fQqc9E/PQBXq2WXSXcAKdcGsvGqg7reaCS";
    pub const JWT_PUBLIC_KEY_B64: &str = "MCowBQYDK2VwAyEAKioTLvfEFI4gtalwnRy5/Y2u+Q4Wdz1sSYldnwk1Ubs=";
    pub const TOTP_ENC_KEY_B64: &str = "zjw5FesMAwveYp3JKXqYJhbZ4O1s2yHgTAp9xgvfqq0=";
    pub const AGORA_APP_ID: &str = "73ddf1abee12ac89822eecb0e7df9e28";
    pub const AGORA_APP_CERTIFICATE: &str = "12a7c64e686cb880ee5cb57ac9eed39d";
}

impl AppConfig {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();

        let environment = env::var("ENVIRONMENT").unwrap_or_else(|_| "development".into());
        let is_dev = environment == "development";

        let require_or_dev_default = |var: &str, dev_default: &str| -> String {
            match env::var(var) {
                Ok(v) if !v.is_empty() => v,
                _ if is_dev => {
                    tracing::warn!(
                        "{} not set — using an INSECURE development-only default. This will refuse to start in any non-development ENVIRONMENT.",
                        var
                    );
                    dev_default.to_string()
                }
                _ => panic!(
                    "FATAL: {} must be set when ENVIRONMENT != development. Refusing to start with an insecure default.",
                    var
                ),
            }
        };

        let database_url = env::var("DATABASE_URL")
            .unwrap_or_else(|_| panic!("FATAL: DATABASE_URL must be set (see .env.example)."));

        let allowed_origins = env::var("ALLOWED_ORIGINS")
            .unwrap_or_else(|_| "http://localhost:3000".into())
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        Self {
            port: env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8080),
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            database_url,
            jwt_ed25519_private_key_b64: require_or_dev_default(
                "JWT_ED25519_PRIVATE_KEY_B64",
                dev_defaults::JWT_PRIVATE_KEY_B64,
            ),
            jwt_ed25519_public_key_b64: require_or_dev_default(
                "JWT_ED25519_PUBLIC_KEY_B64",
                dev_defaults::JWT_PUBLIC_KEY_B64,
            ),
            totp_encryption_key_b64: require_or_dev_default(
                "TOTP_ENCRYPTION_KEY_B64",
                dev_defaults::TOTP_ENC_KEY_B64,
            ),
            agora_app_id: require_or_dev_default("AGORA_APP_ID", dev_defaults::AGORA_APP_ID),
            agora_app_certificate: require_or_dev_default(
                "AGORA_APP_CERTIFICATE",
                dev_defaults::AGORA_APP_CERTIFICATE,
            ),
            storage_bucket: env::var("STORAGE_BUCKET").unwrap_or_else(|_| "helodoc-medical-records".into()),
            environment,
            allowed_origins,
        }
    }
}
