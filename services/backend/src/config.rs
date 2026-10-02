use std::env;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub port: u16,
    pub host: String,
    pub database_url: Option<String>,
    pub jwt_secret: String,
    pub agora_app_id: String,
    pub agora_app_certificate: String,
    pub storage_bucket: String,
    pub environment: String,
}

impl AppConfig {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();

        Self {
            port: env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            database_url: env::var("DATABASE_URL").ok(),
            jwt_secret: env::var("JWT_SECRET")
                .unwrap_or_else(|_| "helodoc-ed25519-development-secret-key-32b!".into()),
            agora_app_id: env::var("AGORA_APP_ID")
                .unwrap_or_else(|_| "a1b2c3d4e5f67890123456789abcdef0".into()),
            agora_app_certificate: env::var("AGORA_APP_CERTIFICATE")
                .unwrap_or_else(|_| "cert_secure_server_only_secret_9988".into()),
            storage_bucket: env::var("STORAGE_BUCKET")
                .unwrap_or_else(|_| "helodoc-medical-records".into()),
            environment: env::var("APP_ENV").unwrap_or_else(|_| "development".into()),
        }
    }
}
