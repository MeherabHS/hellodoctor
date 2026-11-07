//! Secret-free startup configuration validation.
//!
//! Usage on a VM (the file is read by dotenvy and values are never printed):
//! `HELLODOCTOR_ENV_FILE=/etc/hellodoctor/hellodoctor.env cargo run --example config_preflight --release`

use hellodoctor_backend::{api::middleware::web_auth::WebAuthGate, config::AppConfig};

fn main() {
    let config = AppConfig::from_env();
    if !WebAuthGate::new(!config.is_development_or_test()).configured() {
        panic!("FATAL: WEB_AUTH_ASSERTION_SECRET is required outside development/test.");
    }
    println!(
        "HelloDoctor configuration is valid for ENVIRONMENT={} (password recovery disabled={}).",
        config.environment, !config.password_reset_enabled
    );
}
