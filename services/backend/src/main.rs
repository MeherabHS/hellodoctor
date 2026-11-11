pub mod abac;
pub mod admission;
pub mod api;
pub mod auth_context;
pub mod config;
pub mod crypto;
pub mod domain;
pub mod email_delivery;
pub mod error;
pub mod ip_jail;
pub mod migration_status;
pub mod rate_limit;
pub mod repository;
pub mod sanitize;
pub mod seed;
pub mod services;
pub mod system_error;
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
    assert!(
        api::middleware::web_auth::WebAuthGate::new(!config.is_development_or_test()).configured(),
        "WEB_AUTH_ASSERTION_SECRET must contain at least 32 non-whitespace characters for deployed Admin authentication"
    );
    tracing::info!("Initializing HelloDoctor Backend Service (v2.4.0)...");

    // Superuser connection, used only to run migrations (including creating the
    // least-privilege `hellodoctor_app` role and its RLS policies) at startup.
    let migrator_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&config.database_url)
        .await
        .expect("FATAL: failed to connect to Postgres. Is the database running and DATABASE_URL correct?");

    // Shared with `migration_status::check` (used by `GET /healthz`) so the
    // "what does this binary expect" answer only ever has one source of
    // truth — see that module's doc comment for the incident this closes.
    migration_status::MIGRATOR
        .run(&migrator_pool)
        .await
        .expect("FATAL: database migrations failed to apply");

    bootstrap_capability_role_logins(&migrator_pool, &config)
        .await
        .expect("FATAL: failed to bootstrap capability-role LOGIN/password");

    // Demo/seed accounts are opt-in only (never automatic), so a plain restart
    // behaves like a real environment instead of silently repopulating fixtures.
    if config.seed_dev_data {
        seed::seed_dev_data(&migrator_pool)
            .await
            .expect("failed to seed development data");
        tracing::info!("Development seed data inserted (SEED_DEV_DATA=true).");
    }
    migrator_pool.close().await;

    // Five live request-serving pools, one per least-privilege capability role
    // (see migrations/20261003000001_rls_hardening). Never a superuser.
    let max_conn = config.db_pool_max_connections_per_role;
    // Milestone 8 pool-capacity experiment: ONLY the API role's own pool may
    // differ from `max_conn`, and only when `DB_POOL_MAX_CONNECTIONS_API_ROLE`
    // is explicitly set (disposable benchmark stack only -- see this field's
    // doc comment in config.rs). Auth/Payment/System/Finance always use
    // `max_conn` unchanged, and `Admission::new`'s lane-sizing math (see
    // repository/mod.rs) still reads `max_conn` directly, never this
    // override, so admission concurrency ceilings cannot be silently widened
    // by this experiment.
    let max_conn_api = config
        .db_pool_max_connections_api_role_override
        .unwrap_or(max_conn);
    let (db, auth_db, payment_db, system_db, finance_db) = tokio::try_join!(
        connect_role(max_conn_api, config.api_role_dsn()),
        connect_role(max_conn, config.auth_worker_dsn()),
        connect_role(max_conn, config.payment_worker_dsn()),
        connect_role(max_conn, config.system_worker_dsn()),
        connect_role(max_conn, config.finance_worker_dsn()),
    )
    .expect("FATAL: failed to connect one of the five capability-role pools. Did bootstrap_capability_role_logins run?");

    // Centralized System Error Service writes through the system-worker pool,
    // matching the pre-existing `GRANT SELECT, INSERT ON system_error_logs TO
    // hellodoctor_system_worker` design intent rather than a new role.
    system_error::init(system_db.clone());

    let redis_cfg = deadpool_redis::Config::from_url(&config.redis_url);
    let redis = redis_cfg
        .create_pool(Some(deadpool_redis::Runtime::Tokio1))
        .expect("FATAL: failed to build Redis connection pool. Is REDIS_URL correct?");

    let state = AppState::new(
        db,
        auth_db,
        payment_db,
        system_db,
        finance_db,
        redis,
        Arc::new(config.clone()),
    );

    // Remediation E1: disaster-recovery erasure-reconciliation gate. A boot
    // with RECOVERY_MODE=true durably records that this incident is awaiting
    // reconciliation; a normal-mode boot refuses to serve ordinary traffic
    // while any recovery instance remains unresolved. See
    // `services::dr_reconciliation` module docs for the full design and its
    // disclosed limitations.
    if config.recovery_mode {
        services::dr_reconciliation::mark_recovery_boot(&state)
            .await
            .expect("FATAL: failed to record disaster-recovery reconciliation obligation");
    } else if let Err(reason) = services::dr_reconciliation::enforce_recovery_gate(&state).await {
        panic!("FATAL: refusing normal-mode startup: {reason}");
    }

    // Doctor Engagement extension's DB-backed due-notification worker (daily
    // consultation-count summary + T-15 reminders) — one background task,
    // same pattern as `system_error::init` above, no new broker/role.
    services::notification_scheduler::start(state.clone());

    let app = create_router(state);

    let addr: SocketAddr = format!("{}:{}", config.host, config.port).parse()?;
    tracing::info!("HelloDoctor Axum HTTP Server listening on http://{}", addr);

    let listener = TcpListener::bind(addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;

    Ok(())
}

async fn connect_role(max_connections: u32, dsn: String) -> Result<sqlx::PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(max_connections)
        .acquire_timeout(std::time::Duration::from_secs(5))
        .idle_timeout(std::time::Duration::from_secs(300))
        // Milestone 8 hold-duration investigation (2026-11-04): emits sqlx's
        // own built-in "acquired connection" tracing event on every
        // successful acquire, captured by `telemetry::SqlxAcquireMetricsLayer`
        // into `sqlx_pool_acquire_wait_seconds` -- additive-only, changes
        // nothing about pool behavior, timeouts, or connection handling.
        .acquire_time_level(log::LevelFilter::Info)
        .connect(&dsn)
        .await
}

/// Grants LOGIN and sets the password for each of the five capability roles
/// the migration creates as NOLOGIN. Runs on every startup via the superuser
/// connection so a password rotation (operator changes the env var and
/// restarts) takes effect without a separate manual step — idempotent, safe
/// to run repeatedly. Passwords never appear in committed SQL; they come
/// solely from the environment (see config.rs / .env.example).
///
/// Role names below are compile-time constants, never user input, so
/// interpolating them into DDL text carries no injection risk. Passwords are
/// operator-controlled secrets (generated with `openssl rand -hex 24`), and
/// are defensively validated as hex-only before interpolation — ALTER ROLE's
/// PASSWORD clause is a DDL grammar position that does not accept `$1` bind
/// parameters, so this is the only way to set it; the hex check is what keeps
/// it safe despite the string formatting.
async fn bootstrap_capability_role_logins(
    pool: &sqlx::PgPool,
    config: &AppConfig,
) -> Result<(), sqlx::Error> {
    let roles: [(&str, &str); 5] = [
        ("hellodoctor_api", &config.db_api_role_password),
        ("hellodoctor_auth_worker", &config.db_auth_worker_password),
        (
            "hellodoctor_payment_worker",
            &config.db_payment_worker_password,
        ),
        (
            "hellodoctor_system_worker",
            &config.db_system_worker_password,
        ),
        (
            "hellodoctor_finance_worker",
            &config.db_finance_worker_password,
        ),
    ];

    for (role, password) in roles {
        assert!(
            !password.is_empty() && password.chars().all(|c| c.is_ascii_hexdigit()),
            "FATAL: capability role password for {} must be a non-empty hex string (generate with `openssl rand -hex 24`). Refusing to interpolate an unsafe value into ALTER ROLE.",
            role
        );
        let stmt = format!("ALTER ROLE {} WITH LOGIN PASSWORD '{}'", role, password);
        sqlx::query(&stmt).execute(pool).await?;
    }

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
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
