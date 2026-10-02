pub mod api;
pub mod config;
pub mod crypto;
pub mod domain;
pub mod error;
pub mod repository;
pub mod seed;
pub mod services;
pub mod telemetry;

use api::create_router;
use config::AppConfig;
use repository::AppState;
use sqlx::postgres::PgPoolOptions;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    telemetry::init_tracing();

    let config = AppConfig::from_env();
    tracing::info!("Initializing HelloDoctor Backend Service (v2.4.0)...");

    let pool = PgPoolOptions::new()
        .max_connections(20)
        .connect(&config.database_url)
        .await
        .expect("FATAL: failed to connect to Postgres. Is the database running and DATABASE_URL correct?");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("FATAL: database migrations failed to apply");

    // Demo/seed accounts are opt-in only (never automatic), so a plain restart
    // behaves like a real environment instead of silently repopulating fixtures.
    if std::env::var("SEED_DEV_DATA").as_deref() == Ok("true") {
        seed::seed_dev_data(&pool).await.expect("failed to seed development data");
        tracing::info!("Development seed data inserted (SEED_DEV_DATA=true).");
    }

    let state = AppState::new(pool, Arc::new(config.clone()));
    let app = create_router(state);

    let addr: SocketAddr = format!("{}:{}", config.host, config.port).parse()?;
    tracing::info!("HelloDoctor Axum HTTP Server listening on http://{}", addr);

    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("Shutdown signal received, draining in-flight requests...");
}
