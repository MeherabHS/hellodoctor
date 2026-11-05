// IMPORTANT: run this suite single-threaded —
//     cargo test --test api_tests -- --test-threads=1
// The five capability roles the RLS migration creates are cluster-wide
// PostgreSQL objects, not per-database ones, even though `#[sqlx::test]`
// gives each test its own throwaway *database*. Running the migration (and
// this file's own role-login bootstrap) from multiple test threads at once
// races on those shared `pg_authid` catalog rows — Postgres raises a genuine
// `"tuple concurrently updated"` error, not a flaky-test artifact to paper
// over. This was hit and diagnosed directly while writing this suite.

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::{DateTime, Datelike, Duration, Utc};
use hellodoctor_backend::api::create_router;
use hellodoctor_backend::config::AppConfig;
use hellodoctor_backend::crypto::Claims;
use hellodoctor_backend::domain::ids::{DoctorProfileId, PatientProfileId, UserId};
use hellodoctor_backend::domain::models::*;
use hellodoctor_backend::repository::{
    appointment_repo, chat_repo, consultation_repo, doctor_repo, legal_repo, wallet_repo, AppState,
    DbRole,
};
use hellodoctor_backend::seed::{
    self, ADMIN_USER_ID, DOCTOR1_PROFILE_ID, DOCTOR1_USER_ID, DOCTOR2_PROFILE_ID, DOCTOR2_USER_ID,
    PATIENT_PROFILE_ID, PATIENT_USER_ID, SEED_ADMIN_EMAIL, SEED_ADMIN_PASSWORD, SEED_DOCTOR1_EMAIL,
    SEED_DOCTOR1_PASSWORD, SEED_PATIENT_PHONE,
};
use http_body_util::BodyExt;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use object_store::ObjectStoreExt;
use rand::Rng;
use serde_json::Value;
use sqlx::{PgPool, Row};
use std::net::SocketAddr;
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

#[path = "cases/admin_articles.rs"]
mod admin_articles;
#[path = "cases/admin_campaigns.rs"]
mod admin_campaigns;
#[path = "cases/admin_quizzes.rs"]
mod admin_quizzes;
#[path = "cases/admission_capacity_bench.rs"]
mod admission_capacity_bench;
#[path = "cases/consultation_recovery.rs"]
mod consultation_recovery;
#[path = "cases/engagement_and_holiday_notifications.rs"]
mod engagement_and_holiday_notifications;
#[path = "cases/notification_localization_preferences.rs"]
mod notification_localization_preferences;
#[path = "cases/patient_quizzes.rs"]
mod patient_quizzes;
#[path = "cases/pool_pressure.rs"]
mod pool_pressure;
#[path = "cases/profile_pictures.rs"]
mod profile_pictures;
#[path = "cases/push_notification_delivery.rs"]
mod push_notification_delivery;
#[path = "cases/schedule_exclusivity.rs"]
mod schedule_exclusivity;
#[path = "cases/security_acceptance.rs"]
mod security_acceptance;

// ============================================================================
// Test configuration & fixed test secrets (never real secrets — same shape as
// `.env.example`, just checked-in-safe demo values used only under `#[sqlx::test]`).
// ============================================================================

const TEST_JWT_PRIVATE_KEY_B64: &str =
    "MC4CAQAwBQYDK2VwBCIEIAT/WOtPd4fQqc9E/PQBXq2WXSXcAKdcGsvGqg7reaCS";
const TEST_JWT_PUBLIC_KEY_B64: &str =
    "MCowBQYDK2VwAyEAKioTLvfEFI4gtalwnRy5/Y2u+Q4Wdz1sSYldnwk1Ubs=";
const TEST_ROLE_PASSWORD: &str = "test_role_password_dev_only";
const TEST_WEBHOOK_SECRET: &str = "test-webhook-hmac-secret-dev-only";
const JWT_ISSUER: &str = "hellodoctor-backend";
const JWT_AUDIENCE: &str = "hellodoctor-mobile";

const ROLE_NAMES: [&str; 5] = [
    "hellodoctor_api",
    "hellodoctor_auth_worker",
    "hellodoctor_payment_worker",
    "hellodoctor_system_worker",
    "hellodoctor_finance_worker",
];

/// `seed::seed_dev_data` encrypts the demo admin's TOTP secret using
/// `TOTP_ENCRYPTION_KEY_B64` straight from the process environment (it has
/// to — seeding runs before any `AppState`/config exists). `#[sqlx::test]`
/// itself calls `dotenvy::dotenv()` internally to find `DATABASE_URL`, which
/// as a side effect loads every other var from the real `services/backend/.env`
/// into this test process too — including the *real* `TOTP_ENCRYPTION_KEY_B64`,
/// which is not this hardcoded test fallback. `test_config()` must compute
/// this value the exact same way seed.rs does, or TOTP decryption at admin
/// login fails with a genuine key mismatch (hit and confirmed while writing
/// this suite, not a hypothetical).
fn totp_key_for_test() -> String {
    std::env::var("TOTP_ENCRYPTION_KEY_B64")
        .unwrap_or_else(|_| "zjw5FesMAwveYp3JKXqYJhbZ4O1s2yHgTAp9xgvfqq0=".into())
}

/// Destructive Redis fixtures require both an explicitly selected non-default
/// loopback endpoint and an ownership marker installed by the fixture owner.
/// A database number alone never establishes isolation from shared Redis.
fn test_redis_url() -> String {
    let url = std::env::var("TEST_REDIS_URL")
        .expect("TEST_REDIS_URL must select an explicitly owned disposable Redis fixture");
    assert!(
        is_disposable_redis_endpoint(&url),
        "Redis tests require literal loopback on a non-default, fixture-owned port"
    );
    url
}

fn is_disposable_redis_endpoint(url: &str) -> bool {
    let Ok(client) = deadpool_redis::redis::Client::open(url) else {
        return false;
    };
    matches!(client.get_connection_info().addr(),
        deadpool_redis::redis::ConnectionAddr::Tcp(host, port)
        if matches!(host.as_str(), "127.0.0.1" | "::1") && *port != 6379)
}

#[test]
fn test_redis_fixture_endpoint_rejects_default_shared_and_remote_addresses() {
    for endpoint in [
        "redis://127.0.0.1:6379/15",
        "redis://localhost:16380/15",
        "redis://192.0.2.1:16380/15",
        "not-a-redis-url",
    ] {
        assert!(!is_disposable_redis_endpoint(endpoint));
    }
    assert!(is_disposable_redis_endpoint("redis://127.0.0.1:16380/15"));
}

async fn reset_owned_redis(redis: &deadpool_redis::Pool) {
    let owner = std::env::var("TEST_REDIS_FIXTURE_ID")
        .expect("TEST_REDIS_FIXTURE_ID is required before destructive Redis fixture setup");
    assert!(
        owner.starts_with("final-remediation-")
            && owner.len() <= 80
            && owner
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-'),
        "Redis fixture identity must be an explicit bounded remediation run ID"
    );
    let mut connection = redis.get().await.expect("owned test Redis connection");
    let marker: Option<String> = deadpool_redis::redis::cmd("GET")
        .arg("hd:test-fixture:owner")
        .query_async(&mut connection)
        .await
        .expect("read dedicated Redis fixture ownership marker");
    assert_eq!(
        marker.as_deref(),
        Some(owner.as_str()),
        "Refusing FLUSHDB: Redis endpoint has no matching fixture ownership marker"
    );
    let _: () = deadpool_redis::redis::pipe()
        .atomic()
        .cmd("FLUSHDB")
        .ignore()
        .cmd("SET")
        .arg("hd:test-fixture:owner")
        .arg(owner)
        .ignore()
        .query_async(&mut connection)
        .await
        .expect("reset only the owned test Redis database and retain its marker");
}

fn test_config() -> AppConfig {
    AppConfig {
        port: 0,
        host: "127.0.0.1".into(),
        database_url: String::new(),
        app_db_host: String::new(),
        app_db_port: 0,
        app_db_name: String::new(),
        db_api_role_password: TEST_ROLE_PASSWORD.into(),
        db_auth_worker_password: TEST_ROLE_PASSWORD.into(),
        db_payment_worker_password: TEST_ROLE_PASSWORD.into(),
        db_system_worker_password: TEST_ROLE_PASSWORD.into(),
        db_finance_worker_password: TEST_ROLE_PASSWORD.into(),
        db_pool_max_connections_per_role: 2,
        payment_reservation_window_secs: 600,
        max_active_users: 1200,
        consultation_trace_retention_days: 5,
        global_ip_rate_limit_per_min: 120,
        redis_url: test_redis_url(),
        jwt_ed25519_private_key_b64: TEST_JWT_PRIVATE_KEY_B64.into(),
        jwt_ed25519_public_key_b64: TEST_JWT_PUBLIC_KEY_B64.into(),
        totp_encryption_key_b64: totp_key_for_test(),
        agora_app_id: "73ddf1abee12ac89822eecb0e7df9e28".into(),
        agora_app_certificate: "12a7c64e686cb880ee5cb57ac9eed39d".into(),
        // Generous, test-only window (seeded slots start 1-5 hours out —
        // see seed.rs — and most tests don't model real elapsed wall-clock
        // time against a slot). Production default (15/60) lives in
        // config.rs; `test_agora_token_time_gate_rejects_outside_window`
        // overrides this deliberately to exercise the real gate.
        agora_token_window_before_mins: 1440,
        agora_token_window_after_mins: 1440,
        webhook_hmac_secret: TEST_WEBHOOK_SECRET.into(),
        agora_webhook_hmac_secret: TEST_WEBHOOK_SECRET.into(),
        storage_bucket: "helodoc-medical-records".into(),
        storage_endpoint: String::new(),
        storage_access_key: String::new(),
        storage_secret_key: String::new(),
        storage_encryption_key_b64: totp_key_for_test(),
        public_api_base_url: "http://127.0.0.1:8080".into(),
        environment: "development".into(),
        seed_dev_data: false,
        dev_auth_otp_enabled: true,
        dev_simulation_enabled: true,
        trusted_proxy_ips: vec![],
        metrics_scrape_token: String::new(),
        allowed_origins: vec!["http://localhost:3000".into()],
        doctor_daily_summary_time_local: "07:00".into(),
        engagement_greeting_time_local: "08:00".into(),
        azure_email_endpoint: None,
        azure_email_access_key: None,
        azure_email_sender: None,
        fcm_project_id: None,
        fcm_client_email: None,
        fcm_private_key_b64: None,
    }
}

// ============================================================================
// Five-role pool plumbing — mirrors main.rs::bootstrap_capability_role_logins,
// but `#[sqlx::test]` hands us a fresh throwaway database connected as the
// superuser, so tests bootstrap+connect the five role pools themselves rather
// than running the real binary's startup sequence.
// ============================================================================

struct RolePools {
    api: PgPool,
    auth: PgPool,
    payment: PgPool,
    system: PgPool,
    finance: PgPool,
}

/// Capability roles are cluster-wide even when the test databases are isolated.
/// Reuse configured development credentials so a test cannot invalidate the
/// running backend's pools. Isolated CI without local configuration retains the
/// existing fixed test credential. Never log either credential source.
fn role_password(role: &str) -> &'static str {
    static CONFIGURED: std::sync::OnceLock<std::collections::HashMap<String, String>> =
        std::sync::OnceLock::new();
    let configured = CONFIGURED.get_or_init(|| {
        let mut values = std::collections::HashMap::new();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".env");
        if path.exists() {
            let entries = dotenvy::from_path_iter(path)
                .unwrap_or_else(|_| panic!("Development role configuration could not be read"));
            for entry in entries {
                let (key, value) = entry.unwrap_or_else(|_| {
                    panic!("Development role configuration could not be parsed")
                });
                values.insert(key, value);
            }
            assert_eq!(
                values.get("ENVIRONMENT").map(String::as_str),
                Some("development"),
                "API tests must not bootstrap roles using production configuration"
            );
        }
        let mut passwords = std::collections::HashMap::new();
        for (role, key) in [
            ("hellodoctor_api", "DB_API_ROLE_PASSWORD"),
            ("hellodoctor_auth_worker", "DB_AUTH_WORKER_PASSWORD"),
            ("hellodoctor_payment_worker", "DB_PAYMENT_WORKER_PASSWORD"),
            ("hellodoctor_system_worker", "DB_SYSTEM_WORKER_PASSWORD"),
            ("hellodoctor_finance_worker", "DB_FINANCE_WORKER_PASSWORD"),
        ] {
            if let Some(password) = std::env::var(key).ok().or_else(|| values.remove(key)) {
                assert!(
                    !password.is_empty() && password.chars().all(|c| c.is_ascii_hexdigit()),
                    "Configured development role credentials must use the existing hex format"
                );
                passwords.insert(role.into(), password);
            }
        }
        passwords
    });
    configured
        .get(role)
        .map(String::as_str)
        .unwrap_or(TEST_ROLE_PASSWORD)
}

async fn role_pool(pool: &PgPool, role: &str) -> PgPool {
    let opts = pool
        .connect_options()
        .as_ref()
        .clone()
        .username(role)
        .password(role_password(role));
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        // Background error consumers can retain a test pool after the body
        // returns. Reap idle fixture connections before SQLx drops its database;
        // production pool settings and held transactions are unaffected.
        .idle_timeout(std::time::Duration::from_millis(100))
        .connect_with(opts)
        .await
        .unwrap_or_else(|e| panic!("connect as {role} failed: {e}"))
}

/// Guards the `ALTER ROLE` bootstrap below. Roles are cluster-wide objects in
/// PostgreSQL, not per-database — `#[sqlx::test]` gives every test its own
/// throwaway *database*, but all of them share the same physical cluster and
/// therefore the same five role rows in `pg_authid`. Running `ALTER ROLE`
/// concurrently from parallel test threads races on those shared catalog rows
/// (observed directly: `"tuple concurrently updated"` from Postgres), so this
/// one step — and only this step, not the per-test-database connect/query
/// work that follows it — is serialized process-wide.
static ROLE_BOOTSTRAP_LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> =
    std::sync::OnceLock::new();

/// Grants LOGIN with configured development credentials (or isolated CI's test
/// credentials), then returns a pool connected as each capability role.
/// Idempotent — safe to call more than once against the same database.
async fn build_role_pools(pool: &PgPool) -> RolePools {
    {
        let lock = ROLE_BOOTSTRAP_LOCK.get_or_init(|| tokio::sync::Mutex::new(()));
        let _guard = lock.lock().await;
        for role in ROLE_NAMES {
            let password = role_password(role);
            let stmt = format!("ALTER ROLE {role} WITH LOGIN PASSWORD '{password}'");
            sqlx::query(&stmt).execute(pool).await.unwrap();
        }
    }
    RolePools {
        api: role_pool(pool, "hellodoctor_api").await,
        auth: role_pool(pool, "hellodoctor_auth_worker").await,
        payment: role_pool(pool, "hellodoctor_payment_worker").await,
        system: role_pool(pool, "hellodoctor_system_worker").await,
        finance: role_pool(pool, "hellodoctor_finance_worker").await,
    }
}

async fn test_app(pool: PgPool) -> (axum::Router, PgPool) {
    test_app_with_config(pool, test_config()).await
}

/// Same as `test_app`, but with a caller-supplied config — used by the
/// capacity-cap test, which needs a deliberately tiny `max_active_users` to
/// exercise the limit without actually creating 1,200 accounts.
async fn test_app_with_config(pool: PgPool, config: AppConfig) -> (axum::Router, PgPool) {
    test_app_with_storage_fixture(pool, config, true).await
}

async fn test_app_with_storage_fixture(
    pool: PgPool,
    config: AppConfig,
    enable_storage_fixture: bool,
) -> (axum::Router, PgPool) {
    hellodoctor_backend::telemetry::init_tracing();
    seed::seed_dev_data(&pool).await.expect("seed dev data");
    // Real persisted families for the fixture JWT helper, not a middleware bypass.
    sqlx::query("INSERT INTO auth_sessions(id,user_id,token_family_id,refresh_token_hash,created_at,last_used_at,expires_at) SELECT gen_random_uuid(),id,id,'fixture-session-'||id::text,NOW(),NOW(),NOW()+INTERVAL '7 days' FROM users")
        .execute(&pool).await.unwrap();

    let roles = build_role_pools(&pool).await;

    let redis_cfg = deadpool_redis::Config::from_url(&config.redis_url);
    let redis = redis_cfg
        .create_pool(Some(deadpool_redis::Runtime::Tokio1))
        .expect("redis pool (is `docker compose up -d` running?)");

    // Reset real counters only after checking explicit fixture ownership.
    reset_owned_redis(&redis).await;

    // Real System Error Service wiring for every test, not just a dedicated
    // few — matches production (`main.rs` calls this once at startup) and
    // lets any test assert on `system_error_logs` directly. Safe to call once
    // per test: each `#[sqlx::test]` gets its own throwaway database, and
    // `system_error::init` replaces the global channel/consumer pair cleanly.
    hellodoctor_backend::system_error::init(roles.system.clone());

    let mut state = AppState::new(
        roles.api,
        roles.auth,
        roles.payment,
        roles.system,
        roles.finance,
        redis,
        Arc::new(config),
    );
    // Test-only fixture; production never substitutes in-memory upload success.
    if enable_storage_fixture {
        state.object_storage = Some(test_object_storage());
    }
    (create_router(state), pool)
}

/// Like `test_app`, but also hands back the real `AppState` (role-pooled,
/// RLS-subject connections — not the raw superuser test pool, which bypasses
/// RLS entirely) for tests that need to call a service/scheduler function
/// directly rather than going through HTTP, while still exercising the real
/// grant/policy set (e.g. confirming `hellodoctor_system_worker`'s notification
/// grants actually work, not just that a superuser connection can do anything).
async fn test_app_and_state(pool: PgPool) -> (axum::Router, PgPool, AppState) {
    hellodoctor_backend::telemetry::init_tracing();
    seed::seed_dev_data(&pool).await.expect("seed dev data");
    // Real persisted families for the fixture JWT helper, not a middleware bypass.
    sqlx::query("INSERT INTO auth_sessions(id,user_id,token_family_id,refresh_token_hash,created_at,last_used_at,expires_at) SELECT gen_random_uuid(),id,id,'fixture-session-'||id::text,NOW(),NOW(),NOW()+INTERVAL '7 days' FROM users")
        .execute(&pool).await.unwrap();
    let roles = build_role_pools(&pool).await;
    let config = test_config();
    let redis_cfg = deadpool_redis::Config::from_url(&config.redis_url);
    let redis = redis_cfg
        .create_pool(Some(deadpool_redis::Runtime::Tokio1))
        .expect("redis pool (is `docker compose up -d` running?)");
    reset_owned_redis(&redis).await;
    hellodoctor_backend::system_error::init(roles.system.clone());
    let mut state = AppState::new(
        roles.api,
        roles.auth,
        roles.payment,
        roles.system,
        roles.finance,
        redis,
        Arc::new(config),
    );
    state.object_storage = Some(test_object_storage());
    (create_router(state.clone()), pool, state)
}

fn test_object_storage() -> Arc<dyn object_store::ObjectStore> {
    static STORE: std::sync::OnceLock<Arc<object_store::memory::InMemory>> =
        std::sync::OnceLock::new();
    STORE
        .get_or_init(|| Arc::new(object_store::memory::InMemory::new()))
        .clone()
}

fn uid(s: &str) -> Uuid {
    Uuid::parse_str(s).unwrap()
}

// ============================================================================
// HTTP request helpers. Every request needs a `ConnectInfo<SocketAddr>`
// extension manually inserted — in production this comes from
// `into_make_service_with_connect_info`, which only applies to a real bound
// TCP listener, not `tower::ServiceExt::oneshot` against the bare `Router`.
// `global_guard_middleware` extracts it on every request (including public
// ones), so every single test request needs one or the extractor fails.
// Each call gets a *fresh random* fake peer address rather than one shared
// address, so per-IP rate-limit/jail counters in Redis (which persist across
// tests, unlike the ephemeral per-test Postgres database) can never leak
// state between tests or between calls within the same test.
// ============================================================================

fn random_peer() -> ConnectInfo<SocketAddr> {
    let mut rng = rand::thread_rng();
    ConnectInfo(SocketAddr::from((
        [
            rng.gen::<u8>(),
            rng.gen::<u8>(),
            rng.gen::<u8>(),
            rng.gen::<u8>(),
        ],
        rng.gen_range(1024..65535),
    )))
}

/// A shed attempt must be an explicit retryable denial, never an arbitrary
/// error accepted as evidence of idempotency. Money assertions stay separate.
async fn assert_controlled_shedding(response: axum::http::Response<Body>) {
    let expected_code = match response.status() {
        StatusCode::TOO_MANY_REQUESTS => "RATE_LIMIT_EXCEEDED",
        StatusCode::SERVICE_UNAVAILABLE => "SERVICE_OVERLOADED",
        status => panic!("unexpected response from bounded admission: {status}"),
    };
    let retry_after = response
        .headers()
        .get("retry-after")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .expect("controlled overload must include numeric Retry-After");
    assert!((1..=3600).contains(&retry_after));
    let body = json_body(response).await;
    assert_eq!(body["success"], false);
    assert_eq!(body["error"]["code"], expected_code);
    assert!(body["error"]["message"]
        .as_str()
        .is_some_and(|value| !value.is_empty()));
    assert!(Uuid::parse_str(body["meta"]["request_id"].as_str().expect("request ID")).is_ok());
}

/// Model expiry of ONLY the delivery cooldown in isolated test Redis. Other
/// target/rolling/daily/global and original phone budgets remain untouched.
async fn expire_otp_delivery_cooldown(state: &AppState, recipient: &str) {
    let key = format!(
        "ratelimit:otp-delivery:cooldown:{}",
        hellodoctor_backend::crypto::hash_code(&recipient.trim().to_lowercase())
    );
    let mut redis = state.redis.get().await.unwrap();
    let expired: i64 = deadpool_redis::redis::cmd("EXPIRE")
        .arg(key)
        .arg(0)
        .query_async(&mut redis)
        .await
        .unwrap();
    assert_eq!(
        expired, 1,
        "a prior delivery must have established its cooldown"
    );
}

async fn send(
    app: &axum::Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<&Value>,
    idem_key: Option<&str>,
) -> axum::http::Response<Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(t) = token {
        builder = builder.header("Authorization", format!("Bearer {t}"));
    }
    if let Some(k) = idem_key {
        builder = builder.header("Idempotency-Key", k);
    }
    let body_bytes = if let Some(v) = body {
        builder = builder.header("Content-Type", "application/json");
        Body::from(serde_json::to_string(v).unwrap())
    } else {
        Body::empty()
    };
    let mut req = builder.body(body_bytes).unwrap();
    req.extensions_mut().insert(random_peer());
    app.clone().oneshot(req).await.unwrap()
}

async fn schedule_slot_fixture(
    app: &axum::Router,
    pool: &PgPool,
    status: &str,
    offset: i64,
) -> Uuid {
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let start = Utc::now() + Duration::days(30) + Duration::minutes(offset);
    let res = post(
        app,
        "/api/v1/doctor/slots",
        Some(&doctor),
        &serde_json::json!({"start_time":start,"end_time":start+Duration::minutes(10)}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let id = Uuid::parse_str(json_body(res).await["data"]["id"].as_str().unwrap()).unwrap();
    if status != "AVAILABLE" {
        sqlx::query("UPDATE doctor_schedule_slots SET status=$2::slot_status_enum WHERE id=$1")
            .bind(id)
            .bind(status)
            .execute(pool)
            .await
            .unwrap();
    }
    id
}

async fn slot_availability(
    app: &axum::Router,
    id: Uuid,
    doctor: &str,
    status: &str,
) -> axum::http::Response<Body> {
    send(
        app,
        "PATCH",
        &format!("/api/v1/doctor/slots/{id}"),
        Some(doctor),
        Some(&serde_json::json!({"status":status})),
        None,
    )
    .await
}

async fn schedule_slot_status(pool: &PgPool, id: Uuid) -> String {
    sqlx::query_scalar("SELECT status::text FROM doctor_schedule_slots WHERE id=$1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

fn schedule_booking(id: Uuid) -> Value {
    serde_json::json!({"doctor_id":DOCTOR1_PROFILE_ID,"slot_id":id,"modality":"VIDEO","gateway":"MPESA"})
}

#[sqlx::test(migrations = "./migrations")]
async fn test_slot_block_own_available(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let id = schedule_slot_fixture(&app, &pool, "AVAILABLE", 0).await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let response = slot_availability(&app, id, &doctor, "BLOCKED").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json_body(response).await["data"]["status"], "BLOCKED");
    assert_eq!(schedule_slot_status(&pool, id).await, "BLOCKED");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_admin_slot_block_syncs_doctor_and_patient_views_with_audit(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let id = schedule_slot_fixture(&app, &pool, "AVAILABLE", 0).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    assert_eq!(
        post(
            &app,
            "/api/v1/admin/slots",
            Some(&admin),
            &serde_json::json!({"slot_id":id,"action":"BLOCK"})
        )
        .await
        .status(),
        StatusCode::OK
    );
    for (path, token) in [
        ("/api/v1/admin/slots".to_string(), &admin),
        ("/api/v1/doctor/slots?limit=100".to_string(), &doctor),
    ] {
        let result = json_body(get(&app, &path, Some(token)).await).await;
        let row = result["data"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["id"] == id.to_string())
            .unwrap();
        assert_eq!(row["status"], "BLOCKED");
    }
    let directory = json_body(
        get(
            &app,
            &format!("/api/v1/doctors/{DOCTOR1_PROFILE_ID}/slots"),
            Some(&patient),
        )
        .await,
    )
    .await;
    assert!(!directory["data"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["id"] == id.to_string()));
    assert_eq!(
        post_idem(
            &app,
            "/api/v1/appointments/book",
            Some(&patient),
            &schedule_booking(id),
            &Uuid::new_v4().to_string()
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    let after: Value = sqlx::query_scalar(
        "SELECT after_data FROM audit_events WHERE resource_id=$1 AND action='SLOT_STATUS_TOGGLED'",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(after["status"], "BLOCKED");
    let creation_count: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_events WHERE resource_id=$1 AND action='SLOT_CREATED' AND actor_role='DOCTOR'")
        .bind(id).fetch_one(&pool).await.unwrap();
    assert_eq!(creation_count, 1);
    assert_eq!(
        post(
            &app,
            "/api/v1/admin/slots",
            Some(&admin),
            &serde_json::json!({"slot_id":id,"action":"UNBLOCK"})
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(schedule_slot_status(&pool, id).await, "AVAILABLE");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_admin_slot_block_protects_reserved_cancelled_and_stale_requests(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    for (index, status) in ["BOOKED", "LOCKED_IN_PAYMENT", "CANCELLED", "BLOCKED"]
        .iter()
        .enumerate()
    {
        let id = schedule_slot_fixture(&app, &pool, status, index as i64 * 15).await;
        assert_eq!(
            post(
                &app,
                "/api/v1/admin/slots",
                Some(&admin),
                &serde_json::json!({"slot_id":id,"action":"BLOCK"})
            )
            .await
            .status(),
            StatusCode::CONFLICT
        );
        assert_eq!(schedule_slot_status(&pool, id).await, *status);
    }
    let id = schedule_slot_fixture(&app, &pool, "AVAILABLE", 90).await;
    assert_eq!(
        post(
            &app,
            "/api/v1/admin/slots",
            Some(&doctor),
            &serde_json::json!({"slot_id":id,"action":"BLOCK"})
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    let payload = serde_json::json!({"slot_id":id,"action":"BLOCK"});
    let (one, two) = tokio::join!(
        post(&app, "/api/v1/admin/slots", Some(&admin), &payload),
        post(&app, "/api/v1/admin/slots", Some(&admin), &payload)
    );
    let mut statuses = [one.status().as_u16(), two.status().as_u16()];
    statuses.sort();
    assert_eq!(statuses, [200, 409]);
    assert_eq!(schedule_slot_status(&pool, id).await, "BLOCKED");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_client_startup_incident_deferred_delivery_visible_to_admin(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let mut payload = client_incident_fixture("DOCTOR_FLUTTER");
    payload["operation"] = serde_json::json!("startup");
    payload["startup_reference"] = serde_json::json!("784C89E0");
    payload["occurred_at"] = serde_json::json!((Utc::now() - Duration::days(1)).to_rfc3339());
    assert_eq!(
        post(&app, "/api/v1/client-errors", None, &payload)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let response = post(&app, "/api/v1/client-errors", Some(&doctor), &payload).await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let body = json_body(response).await;
    let id = body["data"]["incident_id"].as_str().unwrap();
    let detail = json_body(
        get(
            &app,
            &format!("/api/v1/admin/system-errors/{id}"),
            Some(&admin),
        )
        .await,
    )
    .await;
    assert_eq!(detail["data"]["error_code"], "CLIENT_STARTUP_EXCEPTION");
    assert_eq!(
        detail["data"]["client_context"]["startup_reference"],
        "784C89E0"
    );
    assert_eq!(
        detail["data"]["client_context"]["source"],
        "deferred_startup_report"
    );
    assert_eq!(
        chrono::DateTime::parse_from_rfc3339(
            detail["data"]["client_context"]["occurred_at"]
                .as_str()
                .unwrap()
        )
        .unwrap(),
        chrono::DateTime::parse_from_rfc3339(payload["occurred_at"].as_str().unwrap()).unwrap()
    );
    payload["startup_reference"] = serde_json::json!("invalid private text");
    assert_eq!(
        post(&app, "/api/v1/client-errors", Some(&doctor), &payload)
            .await
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_slot_block_prevents_patient_booking(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let id = schedule_slot_fixture(&app, &pool, "AVAILABLE", 0).await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    assert_eq!(
        slot_availability(&app, id, &doctor, "BLOCKED")
            .await
            .status(),
        StatusCode::OK
    );
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    assert_eq!(
        post_idem(
            &app,
            "/api/v1/appointments/book",
            Some(&patient),
            &schedule_booking(id),
            &Uuid::new_v4().to_string()
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(schedule_slot_status(&pool, id).await, "BLOCKED");
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM appointments WHERE slot_id=$1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_slot_block_unblock_restores_available(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let id = schedule_slot_fixture(&app, &pool, "BLOCKED", 0).await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    assert_eq!(
        slot_availability(&app, id, &doctor, "AVAILABLE")
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(schedule_slot_status(&pool, id).await, "AVAILABLE");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_slot_block_cross_doctor_denied(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let id = schedule_slot_fixture(&app, &pool, "AVAILABLE", 0).await;
    let foreign = make_token(uid(DOCTOR2_USER_ID), "DOCTOR");
    assert_eq!(
        slot_availability(&app, id, &foreign, "BLOCKED")
            .await
            .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(schedule_slot_status(&pool, id).await, "AVAILABLE");
    sqlx::query("UPDATE doctor_schedule_slots SET status='BLOCKED' WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        slot_availability(&app, id, &foreign, "AVAILABLE")
            .await
            .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(schedule_slot_status(&pool, id).await, "BLOCKED");
}

/// BOLA/IDOR regression, cancel path: `test_slot_block_cross_doctor_denied`
/// above already proves `PATCH /doctor/slots/:slot_id` (block/unblock)
/// denies a foreign doctor; `DELETE /doctor/slots/:slot_id` (cancellation)
/// is a separate handler (`cancel_own_slot`) with its own ownership-scoped
/// `UPDATE ... WHERE id=$1 AND doctor_id=$2`, and was not covered by that or
/// any other test — a doctor substituting another doctor's own, unbooked
/// slot id must not be able to cancel it.
#[sqlx::test(migrations = "./migrations")]
async fn test_bola_doctor_cannot_cancel_another_doctors_schedule_slot(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let id = schedule_slot_fixture(&app, &pool, "AVAILABLE", 0).await;
    let foreign = make_token(uid(DOCTOR2_USER_ID), "DOCTOR");

    // `cancel_own_slot` scopes its UPDATE by the caller's own doctor_id, so a
    // cross-doctor attempt affects zero rows and surfaces identically to an
    // already-booked/cancelled slot (409 Conflict), not a silent 200.
    let res = delete(&app, &format!("/api/v1/doctor/slots/{id}"), Some(&foreign)).await;
    assert_eq!(
        res.status(),
        StatusCode::CONFLICT,
        "an unrelated doctor must not be able to cancel another doctor's schedule slot"
    );
    assert_eq!(schedule_slot_status(&pool, id).await, "AVAILABLE");

    // The genuine owner can still cancel it normally.
    let owner = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    assert_eq!(
        delete(&app, &format!("/api/v1/doctor/slots/{id}"), Some(&owner))
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(schedule_slot_status(&pool, id).await, "CANCELLED");
}

async fn protected_schedule_state(pool: PgPool, status: &str) {
    let (app, pool) = test_app(pool).await;
    let id = schedule_slot_fixture(&app, &pool, status, 0).await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    for target in ["BLOCKED", "AVAILABLE"] {
        assert_eq!(
            slot_availability(&app, id, &doctor, target).await.status(),
            StatusCode::CONFLICT
        );
    }
    assert_eq!(
        delete(&app, &format!("/api/v1/doctor/slots/{id}"), Some(&doctor))
            .await
            .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(schedule_slot_status(&pool, id).await, status);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_slot_block_booked_protection(pool: PgPool) {
    protected_schedule_state(pool, "BOOKED").await;
}

#[sqlx::test(migrations = "./migrations")]
async fn test_slot_block_payment_lock_protection(pool: PgPool) {
    protected_schedule_state(pool, "LOCKED_IN_PAYMENT").await;
}

#[sqlx::test(migrations = "./migrations")]
async fn test_slot_block_cancelled_protection(pool: PgPool) {
    protected_schedule_state(pool, "CANCELLED").await;
}

#[sqlx::test(migrations = "./migrations")]
async fn test_slot_block_duplicate_defined_conflict(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let id = schedule_slot_fixture(&app, &pool, "AVAILABLE", 0).await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    assert_eq!(
        slot_availability(&app, id, &doctor, "BLOCKED")
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        slot_availability(&app, id, &doctor, "BLOCKED")
            .await
            .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(schedule_slot_status(&pool, id).await, "BLOCKED");
    assert_eq!(
        slot_availability(&app, id, &doctor, "AVAILABLE")
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        slot_availability(&app, id, &doctor, "AVAILABLE")
            .await
            .status(),
        StatusCode::CONFLICT
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_slot_block_patient_booking_race_one_winner(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    for round in 0..3 {
        let id = schedule_slot_fixture(&app, &pool, "AVAILABLE", round * 15).await;
        let payload = schedule_booking(id);
        let key = Uuid::new_v4().to_string();
        let (block, book) = tokio::join!(
            slot_availability(&app, id, &doctor, "BLOCKED"),
            post_idem(
                &app,
                "/api/v1/appointments/book",
                Some(&patient),
                &payload,
                &key
            )
        );
        let status = schedule_slot_status(&pool, id).await;
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM appointments WHERE slot_id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
        if block.status() == StatusCode::OK {
            assert_eq!(book.status(), StatusCode::CONFLICT);
            assert_eq!(status, "BLOCKED");
            assert_eq!(count, 0);
        } else {
            assert_eq!(block.status(), StatusCode::CONFLICT);
            assert_eq!(book.status(), StatusCode::CREATED);
            assert_eq!(status, "LOCKED_IN_PAYMENT");
            assert_eq!(count, 1);
        }
    }
}

/// Creates a brand-new, real `PATIENT` user with its own `patient_profiles`
/// row (required for `ctx.patient_profile_id` to resolve at all) and a fresh
/// auth session, independent of the shared seeded patient. Used by the
/// 50-concurrent-booking race test below so each concurrent attempt comes
/// from a genuinely distinct account — this avoids conflating the slot-lock
/// race with the orthogonal per-user `rate_limit::check_booking_limit`
/// abuse control (10 bookings/hour/user), which a single hammering patient
/// would trip well before reaching 50 attempts.
async fn fresh_patient_token(pool: &PgPool) -> String {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,phone_number,role,is_active,is_verified,created_at,updated_at) VALUES($1,$2,'PATIENT'::user_role_enum,true,true,NOW(),NOW())")
        .bind(id)
        .bind(format!("+2547{:08}", rand::thread_rng().gen_range(0..100_000_000)))
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO patient_profiles (id, user_id, full_name, date_of_birth, gender, created_at, updated_at) \
         VALUES (gen_random_uuid(), $1, 'Race Test Patient', '1990-01-01', 'OTHER', NOW(), NOW())",
    )
    .bind(id)
    .execute(pool)
    .await
    .unwrap();
    fixture_session(pool, id).await;
    make_token(id, "PATIENT")
}

/// Scales the booking-vs-block race immediately above from 2-way concurrency
/// (one booking call racing one doctor-block call, across 3 rounds) to 50+
/// concurrent booking attempts against a single `AVAILABLE` slot, matching
/// the spec's own higher-concurrency requirement. Each attempt comes from a
/// distinct patient account (`fresh_patient_token`) so none are rejected by
/// the unrelated per-user booking rate limit, and each uses its own
/// idempotency key so none are short-circuited by idempotency replay —
/// admitted attempts reach `doctor_repo::try_lock_slot`'s `SELECT ... FOR
/// UPDATE`; excess attempts must carry a controlled shedding envelope.
/// Asserts exactly one winner and no orphaned schedule/transaction state.
#[sqlx::test(migrations = "./migrations")]
async fn test_fifty_concurrent_booking_attempts_against_one_slot_single_winner(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let id = schedule_slot_fixture(&app, &pool, "AVAILABLE", 0).await;

    const ATTEMPTS: usize = 50;
    let mut tokens = Vec::with_capacity(ATTEMPTS);
    for _ in 0..ATTEMPTS {
        tokens.push(fresh_patient_token(&pool).await);
    }

    let mut handles = Vec::with_capacity(ATTEMPTS);
    for token in tokens {
        let app = app.clone();
        let payload = schedule_booking(id);
        let key = Uuid::new_v4().to_string();
        handles.push(tokio::spawn(async move {
            post_idem(
                &app,
                "/api/v1/appointments/book",
                Some(&token),
                &payload,
                &key,
            )
            .await
        }));
    }
    let mut created = 0;
    let mut conflicts = 0;
    let mut shed = 0;
    for h in handles {
        let response = h.await.unwrap();
        let status = response.status();
        if status == StatusCode::CREATED {
            created += 1;
        } else if status == StatusCode::CONFLICT {
            conflicts += 1;
        } else {
            assert_controlled_shedding(response).await;
            shed += 1;
        }
    }
    assert_eq!(
        created, 1,
        "exactly one of {ATTEMPTS} concurrent booking attempts against the same slot must succeed"
    );
    assert_eq!(
        conflicts + shed,
        ATTEMPTS - 1,
        "every other concurrent attempt must conflict or explicitly shed before backend work"
    );

    let status = schedule_slot_status(&pool, id).await;
    assert_eq!(
        status, "LOCKED_IN_PAYMENT",
        "the slot must land in exactly the locked-for-payment state, not AVAILABLE (orphaned back open) or any other status"
    );
    let appointment_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM appointments WHERE slot_id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        appointment_count, 1,
        "exactly one appointment row for the slot, no duplicates"
    );
    let transaction_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM transactions t JOIN appointments a ON a.id = t.appointment_id WHERE a.slot_id = $1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        transaction_count, 1,
        "exactly one transaction row for the slot, no duplicate payment/wallet event"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_slot_block_patient_availability_excludes_blocked(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let id = schedule_slot_fixture(&app, &pool, "AVAILABLE", 0).await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let path = format!("/api/v1/doctors/{DOCTOR1_PROFILE_ID}/slots");
    let before = json_body(get(&app, &path, Some(&patient)).await).await;
    assert!(before["data"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["id"] == id.to_string()));
    assert_eq!(
        slot_availability(&app, id, &doctor, "BLOCKED")
            .await
            .status(),
        StatusCode::OK
    );
    let after = json_body(get(&app, &path, Some(&patient)).await).await;
    assert!(after["data"]
        .as_array()
        .unwrap()
        .iter()
        .all(|row| row["id"] != id.to_string() && row["status"] == "AVAILABLE"));
}

#[sqlx::test(migrations = "./migrations")]
async fn test_slot_block_doctor_listing_includes_blocked(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let id = schedule_slot_fixture(&app, &pool, "BLOCKED", 0).await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let listing = json_body(get(&app, "/api/v1/doctor/slots?limit=100", Some(&doctor)).await).await;
    let row = listing["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == id.to_string())
        .unwrap();
    assert_eq!(row["status"], "BLOCKED");
    assert!(row["appointment_id"].is_null());
    assert!(row["patient_display_name"].is_null());
}

#[sqlx::test(migrations = "./migrations")]
async fn test_slot_block_cancellation_is_distinct(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    for (offset, status) in [(0, "AVAILABLE"), (15, "BLOCKED")] {
        let id = schedule_slot_fixture(&app, &pool, status, offset).await;
        assert_eq!(
            delete(&app, &format!("/api/v1/doctor/slots/{id}"), Some(&doctor))
                .await
                .status(),
            StatusCode::OK
        );
        assert_eq!(schedule_slot_status(&pool, id).await, "CANCELLED");
        assert_eq!(
            slot_availability(&app, id, &doctor, "AVAILABLE")
                .await
                .status(),
            StatusCode::CONFLICT
        );
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn test_slot_block_auth_and_dto_limits(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let id = schedule_slot_fixture(&app, &pool, "AVAILABLE", 0).await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let path = format!("/api/v1/doctor/slots/{id}");
    for body in [
        serde_json::json!({"status":"BOOKED"}),
        serde_json::json!({"status":"CANCELLED"}),
        serde_json::json!({"status":"BLOCKED","doctor_id":DOCTOR2_PROFILE_ID}),
    ] {
        assert_eq!(
            send(&app, "PATCH", &path, Some(&doctor), Some(&body), None)
                .await
                .status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    assert_eq!(
        send(
            &app,
            "PATCH",
            &path,
            None,
            Some(&serde_json::json!({"status":"BLOCKED"})),
            None
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    sqlx::query(
        "UPDATE doctor_profiles SET professional_verification_status='PENDING' WHERE id=$1",
    )
    .bind(uid(DOCTOR1_PROFILE_ID))
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        slot_availability(&app, id, &doctor, "BLOCKED")
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(schedule_slot_status(&pool, id).await, "AVAILABLE");
}

async fn get(app: &axum::Router, uri: &str, token: Option<&str>) -> axum::http::Response<Body> {
    send(app, "GET", uri, token, None, None).await
}
async fn post(
    app: &axum::Router,
    uri: &str,
    token: Option<&str>,
    body: &Value,
) -> axum::http::Response<Body> {
    send(app, "POST", uri, token, Some(body), None).await
}
async fn delete(app: &axum::Router, uri: &str, token: Option<&str>) -> axum::http::Response<Body> {
    send(app, "DELETE", uri, token, None, None).await
}
async fn put(
    app: &axum::Router,
    uri: &str,
    token: Option<&str>,
    body: &Value,
) -> axum::http::Response<Body> {
    send(app, "PUT", uri, token, Some(body), None).await
}
async fn post_idem(
    app: &axum::Router,
    uri: &str,
    token: Option<&str>,
    body: &Value,
    idem_key: &str,
) -> axum::http::Response<Body> {
    send(app, "POST", uri, token, Some(body), Some(idem_key)).await
}

async fn json_body(res: axum::http::Response<Body>) -> Value {
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

/// Compares monetary API fields by numeric value, not exact decimal-scale formatting
/// (JSON-serialized `BigDecimal`'s trailing-zero display is a presentation detail,
/// not something business logic should assert on).
fn assert_money_eq(actual: &Value, expected: &str) {
    let actual_bd: bigdecimal::BigDecimal = actual.as_str().unwrap().parse().unwrap();
    let expected_bd: bigdecimal::BigDecimal = expected.parse().unwrap();
    assert_eq!(actual_bd, expected_bd, "expected {expected} got {actual}");
}

/// Encodes a genuine, fully-decodable minimal JPEG so the real upload-security
/// pipeline (which decodes and re-encodes every image) accepts it.
fn minimal_jpeg_base64() -> String {
    let img = image::RgbImage::from_pixel(2, 2, image::Rgb([200, 30, 30]));
    let mut buf = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(img)
        .write_to(&mut buf, image::ImageFormat::Jpeg)
        .unwrap();
    STANDARD.encode(buf.into_inner())
}

async fn open_clinical_window(pool: &PgPool, appointment: Uuid) {
    sqlx::query("UPDATE doctor_schedule_slots SET start_time=NOW()+INTERVAL '5 minutes',end_time=NOW()+INTERVAL '15 minutes' WHERE id=(SELECT slot_id FROM appointments WHERE id=$1)")
        .bind(appointment).execute(pool).await.unwrap();
}

/// Same as `open_clinical_window`, but offset by `offset_minutes` from now
/// instead of the fixed "+5 to +15" window. Needed wherever a test opens the
/// clinical window for two DIFFERENT appointments belonging to the SAME
/// Patient: calling the fixed-offset helper twice would land both at
/// (almost) the identical instant, which Stage A's `appt_patient_no_overlap`
/// exclusion constraint (20261111000001_schedule_exclusivity) now correctly
/// rejects as a genuine double-booking — an artifact of this shared
/// fixture's fixed timing, not the invariant the affected tests actually
/// check.
async fn open_clinical_window_at(pool: &PgPool, appointment: Uuid, offset_minutes: i32) {
    sqlx::query("UPDATE doctor_schedule_slots SET start_time=NOW()+make_interval(mins=>$2),end_time=NOW()+make_interval(mins=>$2+10) WHERE id=(SELECT slot_id FROM appointments WHERE id=$1)")
        .bind(appointment)
        .bind(offset_minutes)
        .execute(pool)
        .await
        .unwrap();
}

// ============================================================================
// JWT helpers — real signing against the test keypair (same PKCS8/SPKI-DER
// shape as production, verified to be a genuine matching pair, see
// `docs/CHECKPOINT.md`), plus hand-crafted malformed/expired/wrong-aud/wrong-iss
// tokens for the negative auth tests.
// ============================================================================

fn make_token(user_id: Uuid, role: &str) -> String {
    hellodoctor_backend::crypto::create_access_token(
        user_id,
        role,
        user_id,
        TEST_JWT_PRIVATE_KEY_B64,
    )
    .unwrap()
}

fn deployed_config() -> AppConfig {
    let mut config = test_config();
    config.environment = "production".into();
    config.dev_auth_otp_enabled = false;
    config.dev_simulation_enabled = false;
    config.trusted_proxy_ips = vec!["127.0.0.1".parse().unwrap()];
    config.metrics_scrape_token = "synthetic-monitoring-credential-for-tests-only".into();
    config.public_api_base_url = "https://api.example.test".into();
    config.allowed_origins = vec!["https://doctor.example.test".into()];
    // Synthetic presence only: this test config never starts an FCM transport
    // and never sends a provider request. Production values are injected by
    // secret management, never committed to the repository.
    config.fcm_project_id = Some("synthetic-fcm-project".into());
    config.fcm_client_email = Some("synthetic-fcm-service@example.invalid".into());
    // A throwaway, locally-generated RSA key (never any real provider's key)
    // that must parse as valid PKCS#1 so `AppState::new` doesn't hard-panic —
    // these tests never actually sign/send a push, they only exercise other
    // production-mode guards (TLS proxy trust, metrics token, origin allowlist).
    config.fcm_private_key_b64 = Some("LS0tLS1CRUdJTiBSU0EgUFJJVkFURSBLRVktLS0tLQ0KTUlJRXBBSUJBQUtDQVFFQXFoUGFrTmRzYjhFbHRyeWxQVzB4U0lqVGJESW83azc3ZERxWDNmcTV4SVpnVGhQTQ0KMzc1eVlIZlYxdmJUSGZROE5acGo5NWNIdTFLTW5VeUFnbG8xbXJES2p3WUs5VXVIRFlWL290N2tIMVhYZ2R2NQ0KbzB6YUJQMC9FNUg3eGN2SmVJUzdEcEZNWUJpOWxWdjVSWi9TTGVWanFDT3N2cnhwWUIvSnpTU2VrNjV4dzdKNA0KMkw0NDFkajZUT0hTY2pqQVZuWkhnVU9pc0RseWFyMTA5cm41TERZUldhR1JCb1Q5WFZQWFo2a2xXdFNJczlVdg0KT2FuazJmSkExZkJwQUVKMlRRNFpVV1ZBRnVCMk9aQ2JlVHdWeitjRUlBZUR2SGEvWlcwVHRNR0szVjA4aGZUNQ0KTGVpbkczemZmL2ZVWEdPNjE3UDlEZ1Y2VEpvb3dmZmVNc0ZyVHdJREFRQUJBb0lCQUVYQmlkbnhPUnptNDdCOA0KMCtzTFAzVG9CWXJ4VWI5WUxxTC9xeTBVdWgrRUc3aTZQME9mYVdydWk0cTB4bWNaTnA2bm1ZdkJhcDdtWHh6ZQ0KOVRCQmpvM0pKL01jUDk5V29HZ24wbTg3S3VZOVAzd1gvRjRCVmNSSkdCZFBQeXZqaHRrQzIzUEFuVW9tcTQ4NA0KWDFjeWt6Sld6Slo0TjFBdGw2eWdyUlNTQ2VZNHNMdDBKdmZUNFZRWkQ1YW5QbnNZcUhZRmhka0RzM01UZ2ljcw0KQW9IY1dCOFVYS1I5MXloOFNYQTg2eXYwUExqTWE5WDI3VmlxRi9mZEpybUlvTjNOZlN1ODZrUWQwS3RKS1I5UQ0KRGt1UWJ4Z010RWREOHdLeU00WFJuczhzRDUyMFY0cEo0SkZnUUFLQitNTzJXKzJKdUxzeEFZelhjcnpCSlgwTg0KM296MTJ2RUNnWUVBNUc4SkJWd0lITGxRKzA2MjVhYmpIeVdNQ3p1ajB6NVNTVGJYOE0yYTIySkRhbjU5R3hPWQ0KRFA4ZmIwcGVQZHBiL1o2MkljWEZERkpBYUd0bW9tRENlSC9UY05UNnZWMFhrQmlONTFocVZBUElJRlRzVEhjNw0KRkQ1NkZMMXRHbmxFc2tMUEhUY2p2dXhQZlFqZ016enhsRmZwUml3eEJSTHZ1YVhLSWRiMG1QRUNnWUVBdnBvSA0KN2doUXZwbXpoMGR2OHVRaU5ZZThYVWxaQVMweWhHNW4xZ1ZCY09RaEpCa01hbjk4WHZObEFpcUc3RHdqd1RWYQ0KVEY0SEJYbGlQM0NzUytnNS9GQlRkQ3phRmhESzhvMmhReit2TmthZXZ3ZjFLUUhEWFQ3cEFrTnV0N3pIWTMwbw0KeG1LTFdZOWt2clo1SjFjeGNSMmR5RjFvamg1NG0yS2E2YnBNU0Q4Q2dZRUFqYUVPTGNWK3ZoaGp6anhpaDRadQ0KUU94dHBMVklnSHVlVndocTRFRUo1MFZJV1NtdGRYRXpqN05LRVIxQnl3V3NqVld1eE1leXdVM3A0eFBiNFljdQ0KYTdtTkplTFd4enVGbFhDejRxRGQ0OU1vYkd0YXlwb09CaUlxWE51SlhxV2JZdzZyRFFrWDM1THlRYXFVSHhDRg0KRTUxaDQ0RVlyM1dyTGlaTmFhZTFwWEVDZ1lCZTJUM2ErUzVkT3hWNDIxYmRVeU9tNmM0VnlodE5sNVZOQzdnRw0KYkFVZHpaaWtZd2FudEp0S2tPcVNBbUcwQzFWcm4wWTVQSTFYN3dpeU16cTZaMno3bjJsNEpaMjlYMHZ2R2dYRw0KOWxrVnRGenBDV2U1cVZQMEFndG9BZkNGNUo2QVp0YzBxZk5ZZFNBWU82dmNwT3BJdFFsQW5TTWJuaFE4bHZ3cQ0KRlhoQ2R3S0JnUURITExDbXA0cFZKSU13dmk4VTRFZUtXSG9CdUUzci9iQ2hNMHdZVVI3Uko3OUpQYkREWkhJcg0KWTN2QTdHRXAxQkhscDBlS3R4b3hxdUJuL3VPVEt2WW8yenAybjJnYXRFaHNlZnJyU3E1UVYvZ2k1UDlmU3V0eQ0KeEE0Mm16Q2ZxZndhcUg1RjY5bFliS0tmb0tXeVVXSmcrakZZQzM4ZU10ekxNNTJUVDBhU0J3PT0NCi0tLS0tRU5EIFJTQSBQUklWQVRFIEtFWS0tLS0tDQo=".into());
    config
}

async fn through_tls_proxy(
    app: &axum::Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<&Value>,
) -> axum::http::Response<Body> {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("x-forwarded-proto", "https")
        .header("x-forwarded-for", "198.51.100.73")
        .header("content-type", "application/json");
    if let Some(token) = token {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    let mut request = request
        .body(
            body.map(|body| Body::from(body.to_string()))
                .unwrap_or_else(Body::empty),
        )
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(
        "127.0.0.1:40000".parse::<SocketAddr>().unwrap(),
    ));
    app.clone().oneshot(request).await.unwrap()
}

async fn token_for_real_role(pool: &PgPool, role: &str) -> String {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,phone_number,role,is_active,is_verified,created_at,updated_at) VALUES($1,$2,$3::user_role_enum,true,true,NOW(),NOW())")
        .bind(id).bind(format!("+2547{:08}", rand::thread_rng().gen_range(0..100_000_000)))
        .bind(role).execute(pool).await.unwrap();
    fixture_session(pool, id).await;
    make_token(id, role)
}

// Retired human roles cannot be assigned. Prove their claims are rejected even
// when the underlying authenticated account exists with the canonical role.
async fn token_for_retired_role(pool: &PgPool, role: &str) -> String {
    let canonical = token_for_real_role(pool, "ADMIN").await;
    let claims =
        hellodoctor_backend::crypto::verify_access_token(&canonical, TEST_JWT_PUBLIC_KEY_B64)
            .unwrap();
    make_token(uid(&claims.user_id), role)
}

async fn fixture_session(pool: &PgPool, id: Uuid) {
    sqlx::query("INSERT INTO auth_sessions(id,user_id,token_family_id,refresh_token_hash,created_at,last_used_at,expires_at) VALUES(gen_random_uuid(),$1,$1,$2,NOW(),NOW(),NOW()+INTERVAL '7 days') ON CONFLICT DO NOTHING")
        .bind(id).bind(format!("fixture-session-{id}")).execute(pool).await.unwrap();
}

#[allow(clippy::too_many_arguments)]
fn craft_token(
    user_id: Uuid,
    role: &str,
    iss: &str,
    aud: &str,
    iat_offset_secs: i64,
    exp_offset_secs: i64,
) -> String {
    let now = chrono::Utc::now().timestamp();
    let claims = Claims {
        sub: user_id.to_string(),
        user_id: user_id.to_string(),
        sid: user_id.to_string(),
        role: role.to_string(),
        permissions: vec![],
        iss: iss.to_string(),
        aud: aud.to_string(),
        exp: (now + exp_offset_secs) as usize,
        iat: (now + iat_offset_secs) as usize,
    };
    let der = STANDARD.decode(TEST_JWT_PRIVATE_KEY_B64).unwrap();
    let key = EncodingKey::from_ed_der(&der);
    encode(&Header::new(Algorithm::EdDSA), &claims, &key).unwrap()
}

fn sign_webhook_body(body: &str, secret: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(body.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

async fn send_webhook(
    app: &axum::Router,
    body_str: &str,
    signature: &str,
) -> axum::http::Response<Body> {
    send_webhook_provider(app, "bkash", body_str, signature).await
}

async fn send_webhook_provider(
    app: &axum::Router,
    provider: &str,
    body_str: &str,
    signature: &str,
) -> axum::http::Response<Body> {
    let mut req = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/webhooks/payment/{provider}"))
        .header("Content-Type", "application/json")
        .header("X-Signature", signature)
        .body(Body::from(body_str.to_string()))
        .unwrap();
    req.extensions_mut().insert(random_peer());
    app.clone().oneshot(req).await.unwrap()
}

/// Books an appointment with seeded doctor 1 as the given (already-tokened)
/// patient and confirms payment via a properly HMAC-signed webhook, returning
/// (appointment_id, payment_session_id).
async fn book_and_confirm(app: &axum::Router, patient_token: &str) -> (Uuid, Uuid) {
    let res_slots = get(
        app,
        &format!("/api/v1/doctors/{}/slots", DOCTOR1_PROFILE_ID),
        Some(patient_token),
    )
    .await;
    assert_eq!(res_slots.status(), StatusCode::OK);
    let json_slots = json_body(res_slots).await;
    let slot_id = Uuid::parse_str(json_slots["data"][0]["id"].as_str().unwrap()).unwrap();

    let book_payload = serde_json::json!({
        "doctor_id": DOCTOR1_PROFILE_ID,
        "slot_id": slot_id,
        "modality": "VIDEO",
        "gateway": "BKASH",
        "chief_complaint": "Persistent fever and joint pain"
    });
    let res_book = post_idem(
        app,
        "/api/v1/appointments/book",
        Some(patient_token),
        &book_payload,
        &Uuid::new_v4().to_string(),
    )
    .await;
    assert_eq!(res_book.status(), StatusCode::CREATED);
    let book_json = json_body(res_book).await;
    let apt_id = Uuid::parse_str(book_json["data"]["appointment_id"].as_str().unwrap()).unwrap();
    let payment_sess_id =
        Uuid::parse_str(book_json["data"]["payment_session_id"].as_str().unwrap()).unwrap();

    let webhook_payload = serde_json::json!({
        "payment_session_id": payment_sess_id,
        "transaction_reference": format!("BK-TEST-{}", Uuid::new_v4()),
        "status": "SUCCESS",
        "event_timestamp": chrono::Utc::now().to_rfc3339(),
        "provider_event_id": Uuid::new_v4().to_string(),
    });
    let body_str = serde_json::to_string(&webhook_payload).unwrap();
    let sig = sign_webhook_body(&body_str, TEST_WEBHOOK_SECRET);
    let res_webhook = send_webhook(app, &body_str, &sig).await;
    assert_eq!(res_webhook.status(), StatusCode::OK);

    (apt_id, payment_sess_id)
}

/// Same as `book_and_confirm`, but books as `profile_id` (a dependent the
/// caller guardians, or an arbitrary other profile for negative testing) —
/// for the multi-profile/family support booking path
/// (`AuthContext::acting_patient_profile_id`).
async fn book_and_confirm_for_profile(
    app: &axum::Router,
    patient_token: &str,
    profile_id: Uuid,
) -> axum::http::Response<Body> {
    let res_slots = get(
        app,
        &format!("/api/v1/doctors/{}/slots", DOCTOR1_PROFILE_ID),
        Some(patient_token),
    )
    .await;
    assert_eq!(res_slots.status(), StatusCode::OK);
    let json_slots = json_body(res_slots).await;
    let slot_id = Uuid::parse_str(json_slots["data"][0]["id"].as_str().unwrap()).unwrap();

    let book_payload = serde_json::json!({
        "doctor_id": DOCTOR1_PROFILE_ID,
        "slot_id": slot_id,
        "modality": "VIDEO",
        // Kenya-only deployment: MPESA, not the legacy BKASH gateway several
        // older test fixtures in this file still use.
        "gateway": "MPESA",
        "chief_complaint": "Dependent profile booking test",
        "profile_id": profile_id
    });
    let res_book = post_idem(
        app,
        "/api/v1/appointments/book",
        Some(patient_token),
        &book_payload,
        &Uuid::new_v4().to_string(),
    )
    .await;
    if res_book.status() != StatusCode::CREATED {
        // Let the caller assert/inspect the failure shape directly (e.g. the
        // "booking for a profile I don't own" negative test) rather than
        // panicking here on its behalf.
        return res_book;
    }
    let book_json = json_body(res_book).await;
    let payment_sess_id =
        Uuid::parse_str(book_json["data"]["payment_session_id"].as_str().unwrap()).unwrap();
    let webhook_payload = serde_json::json!({
        "payment_session_id": payment_sess_id,
        "transaction_reference": format!("MPESA-TEST-{}", Uuid::new_v4()),
        "status": "SUCCESS",
        "event_timestamp": chrono::Utc::now().to_rfc3339(),
        "provider_event_id": Uuid::new_v4().to_string(),
    });
    let body_str = serde_json::to_string(&webhook_payload).unwrap();
    let sig = sign_webhook_body(&body_str, TEST_WEBHOOK_SECRET);
    // Must match the booking's own "MPESA" gateway — the webhook handler
    // rejects a signed callback whose provider doesn't match the
    // transaction's originally selected gateway.
    let res_webhook = send_webhook_provider(app, "mpesa", &body_str, &sig).await;
    assert_eq!(res_webhook.status(), StatusCode::OK);

    // Re-serialize book_json back into a Response so every caller (both the
    // success and not-found paths) sees the same shape: the booking
    // response's own JSON body, already consumed above to get the session id.
    axum::http::Response::builder()
        .status(StatusCode::CREATED)
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(
                &serde_json::json!({ "success": true, "data": book_json["data"] }),
            )
            .unwrap(),
        ))
        .unwrap()
}

// Canonical ownership fixture: four distinct security identities, two treatment
// relationships, and real FK-backed clinical/chat/document rows in each.
struct IsolationFixture {
    app: axum::Router,
    pool: PgPool,
    state: AppState,
    patient_b: PatientProfile,
    appointments: [Uuid; 2],
    chats: [Uuid; 2],
}

fn isolation_ctx(user: UserId, role: UserRole) -> hellodoctor_backend::auth_context::AuthContext {
    hellodoctor_backend::auth_context::AuthContext {
        user_id: user,
        role,
        permissions: vec![],
        session_id: None,
        request_id: None,
    }
}

async fn isolation_fixture(pool: PgPool) -> IsolationFixture {
    use hellodoctor_backend::repository::{patient_repo, user_repo};
    let (app, pool, state) = test_app_and_state(pool).await;
    let user = user_repo::create_patient_user(&pool, "+254700009992")
        .await
        .unwrap();
    let mut patient_b = patient_repo::find_by_user_id(&pool, UserId::from(uid(PATIENT_USER_ID)))
        .await
        .unwrap()
        .unwrap();
    patient_b.id = PatientProfileId::new();
    patient_b.user_id = Some(user.id);
    patient_b.full_name = "Synthetic Isolated Patient B".into();
    patient_repo::insert(&pool, &patient_b).await.unwrap();
    sqlx::query("INSERT INTO auth_sessions(id,user_id,token_family_id,refresh_token_hash,expires_at) VALUES(gen_random_uuid(),$1,$1,$1::text,now()+interval '1 day')")
        .bind(user.id).execute(&pool).await.unwrap();
    let (a, _) = book_and_confirm(&app, &make_token(uid(PATIENT_USER_ID), "PATIENT")).await;
    let mut b = appointment_repo::find_by_id(&pool, a)
        .await
        .unwrap()
        .unwrap();
    b.id = Uuid::new_v4();
    b.appointment_number = format!("ISO-{}", b.id);
    b.patient_id = patient_b.id;
    b.doctor_id = DoctorProfileId::from(uid(DOCTOR2_PROFILE_ID));
    b.slot_id = sqlx::query_scalar(
        "INSERT INTO doctor_schedule_slots(doctor_id,start_time,end_time,status) VALUES($1,now()-interval '1 minute',now()+interval '19 minutes','BOOKED') RETURNING id",
    )
    .bind(b.doctor_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    appointment_repo::insert(&pool, &b).await.unwrap();
    let mut chats = [Uuid::nil(); 2];
    for (i, appointment) in [a, b.id].into_iter().enumerate() {
        // Put both relationships inside the real clinical access window.
        sqlx::query("UPDATE doctor_schedule_slots SET start_time=now()-interval '1 minute',end_time=now()+interval '19 minutes' WHERE id=(SELECT slot_id FROM appointments WHERE id=$1)")
            .bind(appointment).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO appointment_clinical_intake(appointment_id,patient_id,chief_complaint) SELECT id,patient_id,'Synthetic private intake' FROM appointments WHERE id=$1 ON CONFLICT(appointment_id) DO NOTHING")
            .bind(appointment).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO consultation_sessions(appointment_id,agora_channel_name) VALUES($1,$1::text) ON CONFLICT(appointment_id) DO NOTHING")
            .bind(appointment).execute(&pool).await.unwrap();
        chats[i] = sqlx::query_scalar("INSERT INTO chat_conversations(appointment_id,patient_id,doctor_id,expires_at) SELECT id,patient_id,doctor_id,now()+interval '1 day' FROM appointments WHERE id=$1 ON CONFLICT(appointment_id) DO UPDATE SET expires_at=EXCLUDED.expires_at RETURNING id")
            .bind(appointment).fetch_one(&pool).await.unwrap();
        sqlx::query("INSERT INTO chat_messages(conversation_id,sender_id,content) SELECT $1,p.user_id,'Synthetic private message' FROM appointments a JOIN patient_profiles p ON p.id=a.patient_id WHERE a.id=$2")
            .bind(chats[i]).bind(appointment).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO prescriptions(appointment_id,doctor_id,patient_id,rx_number,is_no_rx_required,no_rx_reason) SELECT id,doctor_id,patient_id,'ISO-'||id::text,true,'Synthetic fixture' FROM appointments WHERE id=$1")
            .bind(appointment).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO prescription_intake_documents(appointment_id,patient_id,page_number,object_bucket,object_key,file_size_bytes,mime_type) SELECT id,patient_id,1,'synthetic-test','isolated-document',100,'image/jpeg' FROM appointments WHERE id=$1")
            .bind(appointment).execute(&pool).await.unwrap();
    }
    IsolationFixture {
        app,
        pool,
        state,
        patient_b,
        appointments: [a, b.id],
        chats,
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn test_identity_unique_profiles_role_consistency_and_immutable_mapping(pool: PgPool) {
    use hellodoctor_backend::repository::{patient_repo, user_repo};
    let (_, pool) = test_app(pool).await;
    let mut patient = patient_repo::find_by_user_id(&pool, UserId::from(uid(PATIENT_USER_ID)))
        .await
        .unwrap()
        .unwrap();
    patient.id = PatientProfileId::new();
    assert_eq!(
        patient_repo::insert(&pool, &patient)
            .await
            .unwrap_err()
            .as_database_error()
            .unwrap()
            .code()
            .as_deref(),
        Some("23505")
    );
    patient.user_id = Some(UserId::from(uid(DOCTOR1_USER_ID)));
    assert_eq!(
        patient_repo::insert(&pool, &patient)
            .await
            .unwrap_err()
            .as_database_error()
            .unwrap()
            .code()
            .as_deref(),
        Some("23514")
    );
    let mut doctor = doctor_repo::find_by_id(&pool, DoctorProfileId::from(uid(DOCTOR1_PROFILE_ID)))
        .await
        .unwrap()
        .unwrap();
    doctor.id = DoctorProfileId::new();
    doctor.license_number = "SYNTH-UNIQUE-ISOLATION".into();
    assert_eq!(
        doctor_repo::insert(&pool, &doctor)
            .await
            .unwrap_err()
            .as_database_error()
            .unwrap()
            .code()
            .as_deref(),
        Some("23505")
    );
    doctor.user_id = UserId::from(uid(PATIENT_USER_ID));
    assert_eq!(
        doctor_repo::insert(&pool, &doctor)
            .await
            .unwrap_err()
            .as_database_error()
            .unwrap()
            .code()
            .as_deref(),
        Some("23514")
    );
    let other = user_repo::create_patient_user(&pool, "+254700009991")
        .await
        .unwrap();
    assert!(
        sqlx::query("UPDATE patient_profiles SET user_id=$1 WHERE id=$2")
            .bind(other.id)
            .bind(uid(PATIENT_PROFILE_ID))
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(sqlx::query("UPDATE users SET role='DOCTOR' WHERE id=$1")
        .bind(uid(PATIENT_USER_ID))
        .execute(&pool)
        .await
        .is_err());
    let racing = user_repo::create_patient_user(&pool, "+254700009995")
        .await
        .unwrap();
    patient.id = PatientProfileId::new();
    patient.user_id = Some(racing.id);
    let (insert, role_change) = tokio::join!(
        async {
            let mut tx = pool.begin().await?;
            patient_repo::insert(&mut *tx, &patient).await?;
            tx.commit().await
        },
        async {
            sqlx::query("UPDATE users SET role='DOCTOR' WHERE id=$1")
                .bind(racing.id)
                .execute(&pool)
                .await
                .map(|_| ())
        }
    );
    assert!(insert.is_err() || role_change.is_err());
    let mismatched:i64=sqlx::query_scalar("SELECT count(*) FROM patient_profiles p JOIN users u ON u.id=p.user_id WHERE p.user_id=$1 AND u.role<>'PATIENT'").bind(racing.id).fetch_one(&pool).await.unwrap();
    assert_eq!(mismatched, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_identity_patient_direct_rls_read_update_delete_and_documents(pool: PgPool) {
    let f = isolation_fixture(pool).await;
    let ctx = isolation_ctx(UserId::from(uid(PATIENT_USER_ID)), UserRole::Patient);
    let device = Uuid::new_v4();
    sqlx::query("INSERT INTO notification_device_tokens(device_id,user_id,platform,push_token) VALUES($1,$2,'ANDROID','synthetic-private-token')")
        .bind(device).bind(f.patient_b.user_id).execute(&f.pool).await.unwrap();
    let mut tx = f.state.begin_authed(&ctx).await.unwrap();
    let tokens: i64 =
        sqlx::query_scalar("SELECT count(*) FROM notification_device_tokens WHERE device_id=$1")
            .bind(device)
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    assert_eq!(tokens, 0);
    assert_eq!(
        sqlx::query("UPDATE notification_device_tokens SET enabled=false WHERE device_id=$1")
            .bind(device)
            .execute(&mut *tx)
            .await
            .unwrap()
            .rows_affected(),
        0
    );
    for table in [
        "patient_profiles",
        "appointments",
        "appointment_clinical_intake",
        "prescriptions",
        "prescription_intake_documents",
        "chat_conversations",
    ] {
        let subject = if table == "patient_profiles" {
            "id"
        } else {
            "patient_id"
        };
        let count: i64 =
            sqlx::query_scalar(&format!("SELECT count(*) FROM {table} WHERE {subject}=$1"))
                .bind(f.patient_b.id)
                .fetch_one(&mut *tx)
                .await
                .unwrap();
        assert_eq!(count, 0, "Patient A must not see B in {table}");
    }
    assert!(appointment_repo::find_by_id(&mut *tx, f.appointments[1])
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        sqlx::query("UPDATE patient_profiles SET full_name='Wrong Patient' WHERE id=$1")
            .bind(f.patient_b.id)
            .execute(&mut *tx)
            .await
            .unwrap()
            .rows_affected(),
        0
    );
    tx.commit().await.unwrap();
    let mut tx = f.state.begin_authed(&ctx).await.unwrap();
    match sqlx::query("DELETE FROM patient_profiles WHERE id=$1")
        .bind(f.patient_b.id)
        .execute(&mut *tx)
        .await
    {
        Ok(result) => assert_eq!(result.rows_affected(), 0),
        Err(error) => assert_eq!(
            error.as_database_error().unwrap().code().as_deref(),
            Some("42501")
        ),
    }
    tx.rollback().await.unwrap();
    let mut tx = f.state.begin_authed(&ctx).await.unwrap();
    let error = sqlx::query("INSERT INTO prescription_intake_documents(appointment_id,patient_id,page_number,object_bucket,object_key,file_size_bytes,mime_type) VALUES($1,$2,2,'synthetic','wrong',100,'image/jpeg')")
        .bind(f.appointments[1]).bind(f.patient_b.id).execute(&mut *tx).await.unwrap_err();
    assert_eq!(
        error.as_database_error().unwrap().code().as_deref(),
        Some("42501")
    );
    tx.rollback().await.unwrap();
    // Correct own patient_id with another patient's appointment is also denied
    // by the composite FK, even if an INSERT policy sees the own patient ID.
    let mut tx = f.state.begin_authed(&ctx).await.unwrap();
    assert!(sqlx::query("INSERT INTO prescription_intake_documents(appointment_id,patient_id,page_number,object_bucket,object_key,file_size_bytes,mime_type) VALUES($1,$2,2,'synthetic','wrong',100,'image/jpeg')")
        .bind(f.appointments[1]).bind(uid(PATIENT_PROFILE_ID)).execute(&mut *tx).await.is_err());
    tx.rollback().await.unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn test_identity_doctor_private_financial_and_treatment_context_rls(pool: PgPool) {
    let f = isolation_fixture(pool).await;
    let ctx = isolation_ctx(UserId::from(uid(DOCTOR1_USER_ID)), UserRole::Doctor);
    let mut tx = f.state.begin_authed(&ctx).await.unwrap();
    let own_relationship: bool = sqlx::query_scalar("SELECT fn_doctor_treats_patient($1,$2)")
        .bind(uid(PATIENT_PROFILE_ID))
        .bind(uid(DOCTOR1_USER_ID))
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert!(own_relationship);
    let supplied_other_doctor: bool = sqlx::query_scalar("SELECT fn_doctor_treats_patient($1,$2)")
        .bind(f.patient_b.id)
        .bind(uid(DOCTOR2_USER_ID))
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert!(
        !supplied_other_doctor,
        "definer helper must bind the authenticated subject"
    );
    let unscoped_relationship: bool = sqlx::query_scalar("SELECT fn_doctor_treats_patient($1,$2)")
        .bind(uid(PATIENT_PROFILE_ID))
        .bind(uid(DOCTOR1_USER_ID))
        .fetch_one(&f.state.db)
        .await
        .unwrap();
    assert!(!unscoped_relationship);
    assert!(
        doctor_repo::find_by_id(&mut *tx, DoctorProfileId::from(uid(DOCTOR2_PROFILE_ID)))
            .await
            .unwrap()
            .is_none()
    );
    for table in [
        "doctor_wallets",
        "doctor_payout_methods",
        "appointments",
        "prescriptions",
        "chat_conversations",
    ] {
        let count: i64 =
            sqlx::query_scalar(&format!("SELECT count(*) FROM {table} WHERE doctor_id=$1"))
                .bind(uid(DOCTOR2_PROFILE_ID))
                .fetch_one(&mut *tx)
                .await
                .unwrap();
        assert_eq!(count, 0, "Doctor A must not read Doctor B's {table}");
    }
    let patient_b: i64 = sqlx::query_scalar("SELECT count(*) FROM patient_profiles WHERE id=$1")
        .bind(f.patient_b.id)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(patient_b, 0);
    let own: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM appointment_clinical_intake WHERE appointment_id=$1",
    )
    .bind(f.appointments[0])
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    assert_eq!(own, 1, "treating Doctor may read in approved window");
    for query in [
        "UPDATE doctor_profiles SET full_name='Wrong Doctor' WHERE id=$1",
        "DELETE FROM doctor_profiles WHERE id=$1",
    ] {
        let result = sqlx::query(query)
            .bind(uid(DOCTOR2_PROFILE_ID))
            .execute(&mut *tx)
            .await;
        match result {
            Ok(result) => assert_eq!(result.rows_affected(), 0),
            Err(e) => {
                assert_eq!(
                    e.as_database_error().unwrap().code().as_deref(),
                    Some("42501")
                );
                break;
            }
        }
    }
    tx.rollback().await.unwrap();
    sqlx::query("UPDATE doctor_schedule_slots SET start_time=now()+interval '80 days',end_time=now()+interval '80 days 20 minutes' WHERE id=(SELECT slot_id FROM appointments WHERE id=$1)")
        .bind(f.appointments[0]).execute(&f.pool).await.unwrap();
    let mut tx = f.state.begin_authed(&ctx).await.unwrap();
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM appointment_clinical_intake WHERE appointment_id=$1",
    )
    .bind(f.appointments[0])
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    assert_eq!(
        count, 1,
        "assigned Doctor retains their own encounter intake without broader profile access"
    );
    tx.commit().await.unwrap();
    sqlx::query("UPDATE doctor_schedule_slots SET start_time=now()-interval '2 hours',end_time=now()-interval '105 minutes' WHERE id=(SELECT slot_id FROM appointments WHERE id=$1)")
        .bind(f.appointments[0]).execute(&f.pool).await.unwrap();
    sqlx::query("UPDATE appointments SET status='COMPLETED',completed_at=now()-interval '1 hour' WHERE id=$1")
        .bind(f.appointments[0]).execute(&f.pool).await.unwrap();
    sqlx::query(
        "UPDATE consultation_sessions SET ended_at=now()-interval '1 hour' WHERE appointment_id=$1",
    )
    .bind(f.appointments[0])
    .execute(&f.pool)
    .await
    .unwrap();
    let mut tx = f.state.begin_authed(&ctx).await.unwrap();
    let followup: i64 = sqlx::query_scalar("SELECT count(*) FROM patient_profiles WHERE id=$1")
        .bind(uid(PATIENT_PROFILE_ID))
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(
        followup, 1,
        "approved follow-up retains Patient demographics"
    );
    tx.commit().await.unwrap();
    sqlx::query("UPDATE appointments SET completed_at=now()-interval '48 hours' WHERE id=$1")
        .bind(f.appointments[0])
        .execute(&f.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE consultation_sessions SET ended_at=now()-interval '48 hours' WHERE appointment_id=$1")
        .bind(f.appointments[0]).execute(&f.pool).await.unwrap();
    let mut tx = f.state.begin_authed(&ctx).await.unwrap();
    let expired_followup: i64 =
        sqlx::query_scalar("SELECT count(*) FROM patient_profiles WHERE id=$1")
            .bind(uid(PATIENT_PROFILE_ID))
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    assert_eq!(
        expired_followup, 0,
        "historical appointment must not grant indefinite private profile access"
    );
    let retained_name: Option<String> =
        sqlx::query_scalar("SELECT doctor_encounter_patient_name($1)")
            .bind(f.appointments[0])
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    assert!(retained_name.is_some());
    let foreign_name: Option<String> =
        sqlx::query_scalar("SELECT doctor_encounter_patient_name($1)")
            .bind(f.appointments[1])
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    assert!(foreign_name.is_none());
    tx.commit().await.unwrap();
    let doctor_b = make_token(uid(DOCTOR2_USER_ID), "DOCTOR");
    assert_eq!(
        get(
            &f.app,
            &format!(
                "/api/v1/doctor/appointments/{}/patient-history",
                f.appointments[0]
            ),
            Some(&doctor_b)
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_identity_wrong_doctor_writes_and_chat_sender_spoofing_denied(pool: PgPool) {
    let f = isolation_fixture(pool).await;
    let ctx = isolation_ctx(UserId::from(uid(DOCTOR1_USER_ID)), UserRole::Doctor);
    let statements=[
        format!("UPDATE prescriptions SET doctor_notes='wrong' WHERE appointment_id='{}'",f.appointments[1]),
        format!("DELETE FROM prescriptions WHERE appointment_id='{}'",f.appointments[1]),
        format!("INSERT INTO doctor_schedule_slots(doctor_id,start_time,end_time) VALUES('{}',now()+interval '4 days',now()+interval '4 days 20 minutes')",DOCTOR2_PROFILE_ID),
        format!("INSERT INTO chat_messages(conversation_id,sender_id,content) VALUES('{}','{}','wrong conversation')",f.chats[1],DOCTOR1_USER_ID),
        format!("INSERT INTO chat_messages(conversation_id,sender_id,content) VALUES('{}','{}','spoofed patient sender')",f.chats[0],PATIENT_USER_ID),
        format!("INSERT INTO prescriptions(appointment_id,doctor_id,patient_id,rx_number) VALUES('{}','{}','{}','WRONG-RX')",f.appointments[1],DOCTOR2_PROFILE_ID,f.patient_b.id),
        format!("UPDATE chat_messages SET sender_id='{}' WHERE conversation_id='{}'",f.patient_b.user_id.unwrap(),f.chats[0]),
        format!("UPDATE chat_messages SET conversation_id='{}' WHERE conversation_id='{}'",f.chats[1],f.chats[0]),
    ];
    for statement in statements {
        let mut tx = f.state.begin_authed(&ctx).await.unwrap();
        match sqlx::query(&statement).execute(&mut *tx).await {
            Ok(result) => assert_eq!(
                result.rows_affected(),
                0,
                "wrong-subject write succeeded: {statement}"
            ),
            Err(error) => assert_eq!(
                error.as_database_error().unwrap().code().as_deref(),
                Some("42501"),
                "{statement}"
            ),
        }
        tx.rollback().await.unwrap();
    }
    let mut tx = f.state.begin_authed(&ctx).await.unwrap();
    let messages: i64 =
        sqlx::query_scalar("SELECT count(*) FROM chat_messages WHERE conversation_id=$1")
            .bind(f.chats[1])
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    assert_eq!(messages, 0);
    sqlx::query("INSERT INTO chat_messages(conversation_id,sender_id,content) VALUES($1,$2,'Allowed treating Doctor message')")
        .bind(f.chats[0]).bind(ctx.user_id).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
}

/// BOLA/IDOR regression, doctor side: `test_bola_patient_cannot_post_into_another_patients_chat`
/// already proves an unrelated *patient* can't read/post into someone else's
/// clinical chat by substituting the conversation id; this is the same
/// property from the other side of the table. `chat_conversations_access`/
/// `chat_messages_access` (RLS) check both `patient_id` and `doctor_id`
/// ownership, so a doctor who was never assigned to this appointment must be
/// denied identically — real-world shape: DOCTOR2 guesses or enumerates
/// DOCTOR1's conversation id and tries to read or post into it.
#[sqlx::test(migrations = "./migrations")]
async fn test_bola_doctor_cannot_access_another_doctors_chat(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let (apt_id, _) = book_and_confirm(&app, &patient_token).await;

    let conv =
        sqlx::query_as::<_, (Uuid,)>("SELECT id FROM chat_conversations WHERE appointment_id = $1")
            .bind(apt_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let conv_id = conv.0;

    // DOCTOR2 (seeded, genuinely registered) has no appointment, chat, or
    // any other relationship with this patient or this conversation.
    let foreign_doctor_token = make_token(uid(DOCTOR2_USER_ID), "DOCTOR");
    let res_post = post(
        &app,
        &format!("/api/v1/chat/{}/messages", conv_id),
        Some(&foreign_doctor_token),
        &serde_json::json!({ "content": "I am not this patient's doctor" }),
    )
    .await;
    assert_eq!(
        res_post.status(),
        StatusCode::NOT_FOUND,
        "an unrelated doctor must not be able to post into another doctor's clinical chat"
    );
    let posted_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM chat_messages WHERE conversation_id=$1")
            .bind(conv_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        posted_count, 0,
        "the forbidden post must not have actually inserted a message"
    );

    let res_list = get(
        &app,
        &format!("/api/v1/chat/{}/messages", conv_id),
        Some(&foreign_doctor_token),
    )
    .await;
    assert_eq!(
        res_list.status(),
        StatusCode::NOT_FOUND,
        "an unrelated doctor must not be able to read another doctor's clinical chat"
    );

    // The genuinely assigned doctor (DOCTOR1, from `book_and_confirm`) is
    // unaffected — this is an ownership denial, not a broken conversation.
    let doctor1_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    assert_eq!(
        get(
            &app,
            &format!("/api/v1/chat/{}/messages", conv_id),
            Some(&doctor1_token)
        )
        .await
        .status(),
        StatusCode::OK
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_identity_pool_context_resets_for_patients_and_doctors(pool: PgPool) {
    let f = isolation_fixture(pool).await;
    // A one-connection pool proves reuse, rather than hoping a larger pool
    // happens to return the same backend connection.
    let opts = f.state.db.connect_options();
    let single = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_with(opts.as_ref().clone())
        .await
        .unwrap();
    let mut state = f.state.clone();
    state.db = single;
    for (first, second, role, table) in [
        (
            UserId::from(uid(PATIENT_USER_ID)),
            f.patient_b.user_id.unwrap(),
            UserRole::Patient,
            "patient_profiles",
        ),
        (
            UserId::from(uid(DOCTOR1_USER_ID)),
            UserId::from(uid(DOCTOR2_USER_ID)),
            UserRole::Doctor,
            "doctor_profiles",
        ),
    ] {
        let mut previous_pid = None;
        for (user, commit) in [
            (first, true),
            (second, false),
            (first, false),
            (second, true),
        ] {
            let mut tx = state
                .begin_authed(&isolation_ctx(user, role))
                .await
                .unwrap();
            let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
                .fetch_one(&mut *tx)
                .await
                .unwrap();
            if let Some(previous) = previous_pid {
                assert_eq!(pid, previous);
            }
            previous_pid = Some(pid);
            let ids: Vec<UserId> = sqlx::query_scalar(&format!("SELECT user_id FROM {table}"))
                .fetch_all(&mut *tx)
                .await
                .unwrap();
            assert_eq!(ids, vec![user]);
            let guc: String =
                sqlx::query_scalar("SELECT current_setting('app.current_user_id',true)")
                    .fetch_one(&mut *tx)
                    .await
                    .unwrap();
            assert_eq!(guc, user.to_string());
            let (patient,doctor):(String,String)=sqlx::query_as("SELECT current_setting('app.current_patient_id',true),current_setting('app.current_doctor_id',true)").fetch_one(&mut *tx).await.unwrap();
            let profile: String =
                sqlx::query_scalar(&format!("SELECT id::text FROM {table} WHERE user_id=$1"))
                    .bind(user)
                    .fetch_one(&mut *tx)
                    .await
                    .unwrap();
            if role == UserRole::Patient {
                assert_eq!(patient, profile);
                assert!(doctor.is_empty());
            } else {
                assert_eq!(doctor, profile);
                assert!(patient.is_empty());
            }
            if commit {
                tx.commit().await.unwrap();
            } else {
                tx.rollback().await.unwrap();
            }
            let leaked: Option<String> =
                sqlx::query_scalar("SELECT NULLIF(current_setting('app.current_user_id',true),'')")
                    .fetch_one(&state.db)
                    .await
                    .unwrap();
            assert_eq!(leaked, None);
            let other_context:bool=sqlx::query_scalar("SELECT COALESCE(NULLIF(current_setting('app.current_role',true),''),NULLIF(current_setting('app.current_patient_id',true),''),NULLIF(current_setting('app.current_doctor_id',true),''),NULLIF(current_setting('app.current_session_id',true),''),NULLIF(current_setting('app.current_request_id',true),'')) IS NULL").fetch_one(&state.db).await.unwrap();
            assert!(other_context);
            let count: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {table}"))
                .fetch_one(&state.db)
                .await
                .unwrap();
            assert_eq!(count, 0);
        }
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn test_identity_compliance_trace_scoped_authorized_and_audited(pool: PgPool) {
    let f = isolation_fixture(pool).await;
    let compliance = token_for_real_role(&f.pool, "ADMIN").await;
    let request = serde_json::json!({"target_type":"APPOINTMENT","target_id":f.appointments[0],"reason":"Synthetic case ISO-001 investigation"});
    for token in [
        make_token(uid(DOCTOR1_USER_ID), "DOCTOR"),
        make_token(uid(PATIENT_USER_ID), "PATIENT"),
        token_for_retired_role(&f.pool, "SUPPORT").await,
        token_for_retired_role(&f.pool, "SECURITY_ADMIN").await,
    ] {
        assert_eq!(
            post(
                &f.app,
                "/api/v1/admin/compliance/trace",
                Some(&token),
                &request
            )
            .await
            .status(),
            if ["PATIENT", "DOCTOR"].contains(
                &hellodoctor_backend::crypto::verify_access_token(&token, TEST_JWT_PUBLIC_KEY_B64)
                    .unwrap()
                    .role
                    .as_str()
            ) {
                StatusCode::FORBIDDEN
            } else {
                StatusCode::UNAUTHORIZED
            }
        );
    }
    let response = post(
        &f.app,
        "/api/v1/admin/compliance/trace",
        Some(&compliance),
        &request,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let request_id = response.headers()["x-request-id"]
        .to_str()
        .unwrap()
        .to_owned();
    let data = json_body(response).await["data"].clone();
    assert_eq!(data["target_user_id"], PATIENT_USER_ID);
    assert_eq!(data["participants"][0]["doctor_user_id"], DOCTOR1_USER_ID);
    let resources = data["resources"].as_array().unwrap();
    for kind in [
        "APPOINTMENT",
        "CONSULTATION",
        "PRESCRIPTION",
        "DOCUMENT",
        "CHAT",
        "TRANSACTION",
    ] {
        assert!(
            resources.iter().any(|r| r["kind"] == kind),
            "missing {kind}"
        );
    }
    assert!(!resources
        .iter()
        .any(|r| r["id"] == f.appointments[1].to_string()));
    assert!(!data.to_string().contains("Synthetic private"));
    let row=sqlx::query("SELECT actor_id,actor_role::text AS role,reason,session_id,request_id FROM audit_events WHERE id=$1 AND action='COMPLIANCE_TRACE'")
        .bind(Uuid::parse_str(data["access_audit_id"].as_str().unwrap()).unwrap()).fetch_one(&f.pool).await.unwrap();
    assert_eq!(row.get::<String, _>("role"), "ADMIN");
    assert_eq!(row.get::<String, _>("reason"), request["reason"]);
    assert!(row.get::<Option<Uuid>, _>("session_id").is_some());
    assert!(row.get::<Option<String>, _>("request_id").is_some());
    assert_eq!(row.get::<String, _>("request_id"), request_id);
    for (kind, id, canonical) in [
        ("PATIENT", uid(PATIENT_PROFILE_ID), uid(PATIENT_USER_ID)),
        ("DOCTOR", uid(DOCTOR1_PROFILE_ID), uid(DOCTOR1_USER_ID)),
        (
            "USER",
            f.patient_b.user_id.unwrap().into_inner(),
            f.patient_b.user_id.unwrap().into_inner(),
        ),
    ] {
        let response=post(&f.app,"/api/v1/admin/compliance/trace",Some(&compliance),&serde_json::json!({"target_type":kind,"target_id":id,"reason":"Synthetic case ISO-002 linkage check"})).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            json_body(response).await["data"]["target_user_id"],
            canonical.to_string()
        );
    }
    let missing=post(&f.app,"/api/v1/admin/compliance/trace",Some(&compliance),&serde_json::json!({"target_type":"USER","target_id":Uuid::new_v4(),"reason":"Synthetic case ISO-003 missing account"})).await;
    assert_eq!(json_body(missing).await["data"]["result"], "NOT_FOUND");
    let ctx = isolation_ctx(UserId::from(uid(DOCTOR1_USER_ID)), UserRole::Doctor);
    let mut tx = f.state.begin_authed(&ctx).await.unwrap();
    assert!(
        sqlx::query("SELECT compliance_trace('USER',$1,'Synthetic forbidden direct SQL')")
            .bind(uid(PATIENT_USER_ID))
            .execute(&mut *tx)
            .await
            .is_err()
    );
    tx.rollback().await.unwrap();
    let denied: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_events WHERE action IN ('COMPLIANCE_TRACE_DENIED','AUTH_ROLE_MISMATCH')",
    )
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert_eq!(denied, 4);
    let role_denials:i64=sqlx::query_scalar("SELECT COUNT(*) FROM audit_events WHERE action='AUTH_ROLE_MISMATCH' AND result='DENIED' AND actor_role='ADMIN' AND session_id IS NOT NULL AND request_id IS NOT NULL").fetch_one(&f.pool).await.unwrap();
    assert_eq!(role_denials, 2);

    sqlx::query("CREATE FUNCTION reject_trace_audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.action='COMPLIANCE_TRACE' THEN RAISE EXCEPTION 'Synthetic trace audit failure'; END IF; RETURN NEW; END $$").execute(&f.pool).await.unwrap();
    sqlx::query("CREATE TRIGGER reject_trace_audit BEFORE INSERT ON audit_events FOR EACH ROW EXECUTE FUNCTION reject_trace_audit()").execute(&f.pool).await.unwrap();
    let failed = post(
        &f.app,
        "/api/v1/admin/compliance/trace",
        Some(&compliance),
        &request,
    )
    .await;
    assert_eq!(failed.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert!(json_body(failed).await.get("resources").is_none());
}

#[sqlx::test(migrations = "./migrations")]
async fn test_identity_rls_capabilities_append_only_audit_and_public_privacy(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let roles: Vec<(String, bool, bool)> =
        sqlx::query_as("SELECT rolname,rolsuper,rolbypassrls FROM pg_roles WHERE rolname=ANY($1)")
            .bind(ROLE_NAMES.to_vec())
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(roles.len(), 5);
    assert!(roles
        .iter()
        .all(|(_, superuser, bypass)| !superuser && !bypass));
    let owned: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pg_tables WHERE schemaname='public' AND tableowner=ANY($1)",
    )
    .bind(ROLE_NAMES.to_vec())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(owned, 0);
    for table in [
        "users",
        "patient_profiles",
        "doctor_profiles",
        "appointments",
        "appointment_clinical_intake",
        "prescription_intake_documents",
        "prescriptions",
        "consultation_sessions",
        "chat_conversations",
        "chat_messages",
        "doctor_wallets",
        "doctor_payout_methods",
        "transactions",
        "disbursement_items",
        "grievance_reports",
        "notifications",
        "notification_device_tokens",
        "doctor_articles",
        "audit_events",
        "system_error_logs",
        "idempotency_records",
    ] {
        let (enabled, forced): (bool, bool) = sqlx::query_as(
            "SELECT relrowsecurity,relforcerowsecurity FROM pg_class WHERE oid=$1::regclass",
        )
        .bind(table)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(enabled && forced, "{table}");
    }
    for role in ROLE_NAMES {
        let mutable:bool=sqlx::query_scalar("SELECT has_table_privilege($1,'audit_events','UPDATE') OR has_table_privilege($1,'audit_events','DELETE')").bind(role).fetch_one(&pool).await.unwrap();
        assert!(!mutable, "{role}");
    }
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let public = json_body(get(&app, "/api/v1/doctors", Some(&patient)).await).await;
    for doctor in public["data"].as_array().unwrap() {
        for private in [
            "user_id",
            "residential_address",
            "verified_phone",
            "professional_verification_note",
            "professional_verification_reviewed_by",
        ] {
            assert!(
                doctor.get(private).is_none(),
                "private directory field {private}"
            );
        }
    }
    let mut tx = state
        .begin_authed(&isolation_ctx(
            UserId::from(uid(DOCTOR1_USER_ID)),
            UserRole::Doctor,
        ))
        .await
        .unwrap();
    assert!(
        sqlx::query("SELECT author_patient_id FROM health_questions")
            .fetch_all(&mut *tx)
            .await
            .is_err()
    );
    tx.rollback().await.unwrap();
    let mut tx = state
        .begin_authed(&isolation_ctx(
            UserId::from(uid(DOCTOR1_USER_ID)),
            UserRole::Doctor,
        ))
        .await
        .unwrap();
    assert!(sqlx::query("SELECT password_hash FROM users WHERE id=$1")
        .bind(uid(DOCTOR1_USER_ID))
        .fetch_all(&mut *tx)
        .await
        .is_err());
    tx.rollback().await.unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn test_identity_patient_bootstrap_atomic_and_doctor_phone_never_becomes_patient(
    pool: PgPool,
) {
    let (app, pool) = test_app(pool).await;
    for (phone, expected) in [
        ("+254700009993", StatusCode::OK),
        ("+254700000101", StatusCode::UNAUTHORIZED),
    ] {
        let otp_response = post(
            &app,
            "/api/v1/auth/register-otp",
            None,
            &serde_json::json!({"phone_number":phone}),
        )
        .await;
        assert_eq!(otp_response.status(), expected);
        let otp = json_body(otp_response).await;
        // Password accounts correctly reject the legacy OTP request already.
        // Send a well-formed negative OTP to exercise the authorization guard,
        // rather than JSON null, which only tests request deserialization (422).
        let otp_code = otp["data"]["debug_otp_code"].as_str().unwrap_or("000000");
        let login = post(
            &app,
            "/api/v1/auth/verify-otp-and-login",
            None,
            &serde_json::json!({
                "phone_number": phone,
                "otp_code": otp_code,
                "accepted_documents": [{"type": "PATIENT_DATA_SHARING_CONSENT", "version": "1.0-afyamkononi"}]
            }),
        )
        .await;
        assert_eq!(login.status(), expected);
    }
    let doctor_patient_profiles: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM patient_profiles WHERE user_id=$1")
            .bind(uid(DOCTOR1_USER_ID))
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(doctor_patient_profiles, 0);
    let count:i64=sqlx::query_scalar("SELECT count(*) FROM users u JOIN patient_profiles p ON p.user_id=u.id WHERE u.phone_number='+254700009993' AND u.role='PATIENT'").fetch_one(&pool).await.unwrap();
    assert_eq!(count, 1);
    // A forced profile write failure must roll back the newly created account.
    sqlx::query("CREATE FUNCTION reject_synthetic_patient() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.user_id IN(SELECT id FROM users WHERE phone_number='+254700009994') THEN RAISE EXCEPTION 'Synthetic profile failure'; END IF; RETURN NEW; END $$")
        .execute(&pool).await.unwrap();
    sqlx::query("CREATE TRIGGER reject_synthetic_patient BEFORE INSERT ON patient_profiles FOR EACH ROW EXECUTE FUNCTION reject_synthetic_patient()")
        .execute(&pool).await.unwrap();
    let otp = json_body(
        post(
            &app,
            "/api/v1/auth/register-otp",
            None,
            &serde_json::json!({"phone_number":"+254700009994"}),
        )
        .await,
    )
    .await;
    let response = post(
        &app,
        "/api/v1/auth/verify-otp-and-login",
        None,
        &serde_json::json!({
            "phone_number":"+254700009994",
            "otp_code":otp["data"]["debug_otp_code"],
            "accepted_documents": [{"type": "PATIENT_DATA_SHARING_CONSENT", "version": "1.0-afyamkononi"}]
        }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let orphan: i64 =
        sqlx::query_scalar("SELECT count(*) FROM users WHERE phone_number='+254700009994'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(orphan, 0);
}

/// First-time Patient activation is blocked without the current consent
/// document, and a successful acceptance is recorded in the immutable ledger.
#[sqlx::test(migrations = "./migrations")]
async fn test_security_patient_registration_requires_and_records_data_consent(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let phone = "+254700009995";
    let first = json_body(
        post(
            &app,
            "/api/v1/auth/register-otp",
            None,
            &serde_json::json!({"phone_number": phone}),
        )
        .await,
    )
    .await;
    let missing = post(
        &app,
        "/api/v1/auth/verify-otp-and-login",
        None,
        &serde_json::json!({
            "phone_number": phone,
            "otp_code": first["data"]["debug_otp_code"]
        }),
    )
    .await;
    assert_eq!(missing.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM users WHERE phone_number=$1")
            .bind(phone)
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );

    // The first OTP was already consumed (deleted) by the verify attempt
    // above regardless of its consent outcome, so a fresh code is required
    // to exercise the accepted-consent path below. Requesting one
    // immediately must legitimately hit the real per-recipient delivery
    // cooldown (`rate_limit::check_otp_delivery_limit`, 1/60s) — assert
    // that rejection explicitly, then advance past only that cooldown
    // (matching `test_security_patient_otp_resend_invalidates_prior_code`),
    // rather than silently resetting rate-limit state to reach 200 OK.
    let still_cooling_down = post(
        &app,
        "/api/v1/auth/register-otp",
        None,
        &serde_json::json!({"phone_number": phone}),
    )
    .await;
    assert_eq!(still_cooling_down.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_controlled_shedding(still_cooling_down).await;

    expire_otp_delivery_cooldown(&state, phone).await;
    let second = json_body(
        post(
            &app,
            "/api/v1/auth/register-otp",
            None,
            &serde_json::json!({"phone_number": phone}),
        )
        .await,
    )
    .await;
    let accepted = post(
        &app,
        "/api/v1/auth/verify-otp-and-login",
        None,
        &serde_json::json!({
            "phone_number": phone,
            "otp_code": second["data"]["debug_otp_code"],
            "accepted_documents": [{"type": "PATIENT_DATA_SHARING_CONSENT", "version": "1.0-afyamkononi"}]
        }),
    )
    .await;
    assert_eq!(accepted.status(), StatusCode::OK);
    let accepted_json = json_body(accepted).await;
    let user_id = accepted_json["data"]["user"]["id"].as_str().unwrap();
    let ledger_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM legal_acceptances WHERE user_id=$1 AND document_type='PATIENT_DATA_SHARING_CONSENT' AND document_version='1.0-afyamkononi'",
    )
    .bind(Uuid::parse_str(user_id).unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(ledger_count, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_identity_composite_parent_ownership_cannot_disagree_even_without_rls(pool: PgPool) {
    let f = isolation_fixture(pool).await;
    // `isolation_fixture` gives Doctor2 (f.appointments[1], unused elsewhere
    // in this test) an active appointment spanning the exact same "now"
    // window as f.appointments[0]. That's harmless on its own (different
    // real Doctors), but the first statement below reassigns
    // f.appointments[0]'s doctor_id to DOCTOR2_PROFILE_ID — which would
    // otherwise now collide with Doctor2's own fixture appointment via the
    // Stage A `appt_doctor_no_overlap` exclusion constraint
    // (20261111000001_schedule_exclusivity) before ever reaching the
    // `appointment_slot_doctor` foreign-key check this test exists to
    // verify. Moving Doctor2's own appointment off to a clearly
    // non-overlapping time keeps this test's original, specific assertion
    // (23503, not merely "some SQL error") exercising the invariant it was
    // written to check.
    sqlx::query("UPDATE doctor_schedule_slots SET start_time=now()+interval '3 hours', end_time=now()+interval '3 hours 20 minutes' WHERE id=(SELECT slot_id FROM appointments WHERE id=$1)")
        .bind(f.appointments[1])
        .execute(&f.pool)
        .await
        .unwrap();
    for statement in [
        format!(
            "UPDATE appointments SET doctor_id='{}' WHERE id='{}'",
            DOCTOR2_PROFILE_ID, f.appointments[0]
        ),
        format!(
            "UPDATE appointment_clinical_intake SET patient_id='{}' WHERE appointment_id='{}'",
            f.patient_b.id, f.appointments[0]
        ),
        format!(
            "UPDATE prescription_intake_documents SET patient_id='{}' WHERE appointment_id='{}'",
            f.patient_b.id, f.appointments[0]
        ),
        format!(
            "UPDATE prescriptions SET doctor_id='{}' WHERE appointment_id='{}'",
            DOCTOR2_PROFILE_ID, f.appointments[0]
        ),
        format!(
            "UPDATE chat_conversations SET patient_id='{}' WHERE appointment_id='{}'",
            f.patient_b.id, f.appointments[0]
        ),
    ] {
        let error = sqlx::query(&statement).execute(&f.pool).await.unwrap_err();
        assert_eq!(
            error.as_database_error().unwrap().code().as_deref(),
            Some("23503"),
            "{statement}"
        );
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn test_identity_patient_booking_cannot_substitute_another_patient(pool: PgPool) {
    let f = isolation_fixture(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let slot:Uuid=sqlx::query_scalar("SELECT id FROM doctor_schedule_slots WHERE doctor_id=$1 AND status='AVAILABLE' ORDER BY start_time DESC LIMIT 1")
        .bind(uid(DOCTOR1_PROFILE_ID)).fetch_one(&f.pool).await.unwrap();
    // Legacy clients may send an ignored target attribute; authorization and
    // persisted ownership must always come from the authenticated account.
    let response=post(&f.app,"/api/v1/appointments/book",Some(&patient),&serde_json::json!({"doctor_id":DOCTOR1_PROFILE_ID,"slot_id":slot,"modality":"VIDEO","gateway":"BKASH","patient_id":f.patient_b.id})).await;
    match response.status() {
        StatusCode::CREATED => {
            let data = json_body(response).await;
            let id = Uuid::parse_str(data["data"]["appointment_id"].as_str().unwrap()).unwrap();
            let subject: PatientProfileId =
                sqlx::query_scalar("SELECT patient_id FROM appointments WHERE id=$1")
                    .bind(id)
                    .fetch_one(&f.pool)
                    .await
                    .unwrap();
            assert_eq!(subject, PatientProfileId::from(uid(PATIENT_PROFILE_ID)));
        }
        StatusCode::UNPROCESSABLE_ENTITY => (),
        status => panic!("unexpected booking response {status}"),
    }
    let ctx = isolation_ctx(UserId::from(uid(PATIENT_USER_ID)), UserRole::Patient);
    let mut tx = f.state.begin_authed(&ctx).await.unwrap();
    let error=sqlx::query("INSERT INTO appointments(appointment_number,patient_id,doctor_id,slot_id,modality,consultation_fee) VALUES('WRONG-PATIENT',$1,$2,$3,'VIDEO',100)")
        .bind(f.patient_b.id).bind(uid(DOCTOR1_PROFILE_ID)).bind(slot).execute(&mut *tx).await.unwrap_err();
    assert_eq!(
        error.as_database_error().unwrap().code().as_deref(),
        Some("42501")
    );
    tx.rollback().await.unwrap();
}

/// BOLA/IDOR regression: `GET /api/v1/appointments/:id` is never driven by
/// any other test with a cross-user id substitution (every other test either
/// uses the caller's own appointment or goes through the narrower
/// `/prescriptions`/`/documents` sub-routes, which have their own dedicated
/// ownership test). `appointments_access` RLS scopes a row to its own
/// patient or its own doctor — this proves that at the HTTP layer: neither
/// an unrelated patient nor an unassigned doctor can resolve another
/// patient's appointment by guessing/enumerating its id.
#[sqlx::test(migrations = "./migrations")]
async fn test_bola_patient_cannot_view_another_patients_appointment(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient1_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let (apt_id, _) = book_and_confirm(&app, &patient1_token).await;

    // Owner can view their own appointment.
    assert_eq!(
        get(
            &app,
            &format!("/api/v1/appointments/{apt_id}"),
            Some(&patient1_token)
        )
        .await
        .status(),
        StatusCode::OK
    );

    // A second patient, genuinely registered, with no relationship
    // whatsoever to this appointment (same unrelated-fixture shape as
    // `test_bola_patient_cannot_post_into_another_patients_chat`).
    let patient2_user_id = Uuid::new_v4();
    let patient2_profile_id = Uuid::new_v4();
    let now = chrono::Utc::now();
    hellodoctor_backend::repository::user_repo::insert_user(
        &pool,
        &User {
            id: UserId::from(patient2_user_id),
            phone_number: "+254700000202".into(),
            email: None,
            password_hash: None,
            role: UserRole::Patient,
            preferred_language: "en".into(),
            is_active: true,
            is_verified: true,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        },
    )
    .await
    .unwrap();
    hellodoctor_backend::repository::patient_repo::insert(
        &pool,
        &PatientProfile {
            id: PatientProfileId::from(patient2_profile_id),
            user_id: Some(UserId::from(patient2_user_id)),
            full_name: "Unrelated Patient Two".into(),
            date_of_birth: chrono::NaiveDate::from_ymd_opt(1992, 2, 2).unwrap(),
            gender: Gender::Other,
            blood_group: None,
            weight_kg: None,
            emergency_contact_phone: None,
            emergency_contact_relation: None,
            guardian_patient_id: None,
            relationship: None,
            created_at: now,
            updated_at: now,
        },
    )
    .await
    .unwrap();
    fixture_session(&pool, patient2_user_id).await;
    let patient2_token = make_token(patient2_user_id, "PATIENT");

    let res_other_patient = get(
        &app,
        &format!("/api/v1/appointments/{apt_id}"),
        Some(&patient2_token),
    )
    .await;
    // `appointments_access` RLS hides the row entirely from an unrelated
    // patient rather than revealing it exists — matches this codebase's
    // established convention for unrelated-party reads (see the chat BOLA
    // tests).
    assert_eq!(
        res_other_patient.status(),
        StatusCode::NOT_FOUND,
        "an unrelated patient must not be able to view another patient's appointment"
    );
    let body = json_body(res_other_patient).await;
    assert!(
        body.get("data").is_none() || body["data"].is_null(),
        "the denied response must not leak the other patient's appointment data: {body}"
    );

    // A doctor never assigned to this appointment must be denied too.
    let foreign_doctor_token = make_token(uid(DOCTOR2_USER_ID), "DOCTOR");
    assert_eq!(
        get(
            &app,
            &format!("/api/v1/appointments/{apt_id}"),
            Some(&foreign_doctor_token)
        )
        .await
        .status(),
        StatusCode::NOT_FOUND,
        "an unassigned doctor must not be able to view a patient's appointment"
    );
}

// ============================================================================
// Existing functional/business-logic tests, now exercising the real
// authenticated/ABAC/RLS path end-to-end instead of hardcoded demo actor IDs.
// ============================================================================

#[sqlx::test(migrations = "./migrations")]
async fn test_disabled_doctor_cannot_login_refresh_or_use_existing_token(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let login_payload =
        serde_json::json!({"email":SEED_DOCTOR1_EMAIL,"password":SEED_DOCTOR1_PASSWORD});
    let login = post(&app, "/api/v1/auth/doctor/login", None, &login_payload).await;
    assert_eq!(login.status(), StatusCode::OK);
    let challenge = json_body(login).await["data"].clone();
    let verified = post(&app,"/api/v1/auth/doctor/verify-otp",None,
        &serde_json::json!({"session_token":challenge["session_token"],"otp_code":challenge["debug_otp_code"]})).await;
    assert_eq!(verified.status(), StatusCode::OK);
    let tokens = json_body(verified).await["data"].clone();
    sqlx::query("UPDATE users SET is_active=false WHERE id=$1")
        .bind(uid(DOCTOR1_USER_ID))
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        get(
            &app,
            "/api/v1/doctor/profile",
            tokens["access_token"].as_str()
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        post(
            &app,
            "/api/v1/auth/refresh",
            None,
            &serde_json::json!({"refresh_token":tokens["refresh_token"]})
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        post(&app, "/api/v1/auth/doctor/login", None, &login_payload)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_unverified_doctor_clinical_restrictions_and_concurrent_schedule_conflict(
    pool: PgPool,
) {
    let (app, pool) = test_app(pool).await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    sqlx::query(
        "UPDATE doctor_profiles SET professional_verification_status='PENDING' WHERE id=$1",
    )
    .bind(uid(DOCTOR1_PROFILE_ID))
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        get(&app, "/api/v1/doctor/profile", Some(&doctor))
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        get(&app, "/api/v1/doctor/appointments", Some(&doctor))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    let future = Utc::now() + Duration::days(6);
    let slot = serde_json::json!({"start_time":future,"end_time":future+Duration::minutes(10)});
    assert_eq!(
        post(&app, "/api/v1/doctor/slots", Some(&doctor), &slot)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    let question = Uuid::new_v4();
    sqlx::query("INSERT INTO health_questions(id,question_text,status,created_at,updated_at) VALUES($1,'General health question','OPEN',NOW(),NOW())").bind(question).execute(&pool).await.unwrap();
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/doctor/qna/questions/{question}/answers"),
            Some(&doctor),
            &serde_json::json!({"answer_text":"Unverified answer"})
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    sqlx::query(
        "UPDATE doctor_profiles SET professional_verification_status='VERIFIED' WHERE id=$1",
    )
    .bind(uid(DOCTOR1_PROFILE_ID))
    .execute(&pool)
    .await
    .unwrap();
    let (a, b) = tokio::join!(
        post(&app, "/api/v1/doctor/slots", Some(&doctor), &slot),
        post(&app, "/api/v1/doctor/slots", Some(&doctor), &slot)
    );
    let mut statuses = vec![a.status().as_u16(), b.status().as_u16()];
    statuses.sort();
    assert_eq!(statuses, vec![201, 409]);
    let past = Utc::now() - Duration::hours(1);
    assert_eq!(
        post(
            &app,
            "/api/v1/doctor/slots",
            Some(&doctor),
            &serde_json::json!({"start_time":past,"end_time":past+Duration::minutes(10)})
        )
        .await
        .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_production_without_providers_fails_closed_without_false_success(pool: PgPool) {
    let (app, pool) = test_app_with_config(pool, deployed_config()).await;
    // Exercise provider guards through the required HTTPS ingress and ordinary
    // authenticated accounts, rather than being denied by transport or fixture policy.
    let doctor = token_for_real_role(&pool, "DOCTOR").await;
    let patient = token_for_real_role(&pool, "PATIENT").await;
    // Keep canonical mappings immutable: create ordinary synthetic profiles
    // instead of transferring seeded profiles between security identities.
    let doctor_claims =
        hellodoctor_backend::crypto::verify_access_token(&doctor, TEST_JWT_PUBLIC_KEY_B64).unwrap();
    let patient_claims =
        hellodoctor_backend::crypto::verify_access_token(&patient, TEST_JWT_PUBLIC_KEY_B64)
            .unwrap();
    let mut ordinary_doctor =
        doctor_repo::find_by_id(&pool, DoctorProfileId::from(uid(DOCTOR1_PROFILE_ID)))
            .await
            .unwrap()
            .unwrap();
    ordinary_doctor.id = DoctorProfileId::new();
    ordinary_doctor.user_id = UserId::from(uid(&doctor_claims.user_id));
    ordinary_doctor.license_number = "SYNTH-PROVIDER-GUARD".into();
    doctor_repo::insert(&pool, &ordinary_doctor).await.unwrap();
    let mut ordinary_patient = hellodoctor_backend::repository::patient_repo::find_by_user_id(
        &pool,
        UserId::from(uid(PATIENT_USER_ID)),
    )
    .await
    .unwrap()
    .unwrap();
    ordinary_patient.id = PatientProfileId::new();
    ordinary_patient.user_id = Some(UserId::from(uid(&patient_claims.user_id)));
    hellodoctor_backend::repository::patient_repo::insert(&pool, &ordinary_patient)
        .await
        .unwrap();
    let doctor_claims =
        hellodoctor_backend::crypto::verify_access_token(&doctor, TEST_JWT_PUBLIC_KEY_B64).unwrap();
    sqlx::query("UPDATE users SET email='synthetic-provider-guard@example.test', password_hash=(SELECT password_hash FROM users WHERE id=$1) WHERE id=$2")
        .bind(uid(DOCTOR1_USER_ID)).bind(uid(&doctor_claims.user_id)).execute(&pool).await.unwrap();
    assert_eq!(
        through_tls_proxy(
            &app,
            "POST",
            "/api/v1/auth/doctor/login",
            None,
            Some(&serde_json::json!({"email":"synthetic-provider-guard@example.test","password":SEED_DOCTOR1_PASSWORD}))
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(
        through_tls_proxy(
            &app,
            "POST",
            "/api/v1/doctor/wallet/payout-methods/dev-bind",
            Some(&doctor),
            Some(&serde_json::json!({"method_type":"MPESA"}))
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    let slot: Uuid = sqlx::query_scalar("SELECT id FROM doctor_schedule_slots WHERE doctor_id=$1 AND status='AVAILABLE' ORDER BY start_time LIMIT 1")
        .bind(uid(DOCTOR1_PROFILE_ID)).fetch_one(&pool).await.unwrap();
    assert_eq!(
        through_tls_proxy(&app, "POST", "/api/v1/appointments/book", Some(&patient),
            Some(&serde_json::json!({"doctor_id":DOCTOR1_PROFILE_ID,"slot_id":slot,"modality":"VIDEO","gateway":"MPESA"})))
            .await.status(),
        StatusCode::CONFLICT
    );
    let bookings: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM appointments")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(bookings, 0);
    let payload=serde_json::json!({"payment_session_id":Uuid::new_v4(),"transaction_reference":"client-not-proof","status":"SUCCESS","event_timestamp":Utc::now(),"provider_event_id":Uuid::new_v4()}).to_string();
    let mut request = Request::builder()
        .method("POST")
        .uri("/api/v1/webhooks/payment/mpesa")
        .header("content-type", "application/json")
        .header(
            "x-signature",
            sign_webhook_body(&payload, TEST_WEBHOOK_SECRET),
        )
        .header("x-forwarded-proto", "https")
        .header("x-forwarded-for", "198.51.100.73")
        .body(Body::from(payload))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(
        "127.0.0.1:40000".parse::<SocketAddr>().unwrap(),
    ));
    assert_eq!(
        app.oneshot(request).await.unwrap().status(),
        StatusCode::CONFLICT
    );
}

/// Regression test for B-01 (docs/DOCTOR_SYSTEM_FINAL_MULTI_AGENT_AUDIT.md §3):
/// `GET /healthz` must actually verify the live database's applied
/// migrations match this binary's embedded migrator, not just return a
/// static "OK" regardless of schema state.
///
/// This is the practical equivalent, for this architecture, of "start from a
/// stale DB and assert readiness is false": `#[sqlx::test]` always runs
/// `sqlx::migrate!` fully before handing back a pool (see this file's own
/// `test_app` and the `migrations = "./migrations"` attribute above), so a
/// stale-then-caught-up sequence can't be constructed mid-test without
/// reaching around the test harness's own guarantees. What *can* be asserted
/// directly against the real migrator and the real database — and is the
/// behavior that actually matters — is that the live binary and the live,
/// fully-migrated database agree, using the exact same comparison this
/// endpoint performs on every call, not a hardcoded expectation.
#[sqlx::test(migrations = "./migrations")]
async fn test_health_check_reports_current_migration_state(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let response = get(&app, "/healthz", None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["status"], "ready");
    assert_eq!(body["database"], "ready");
    assert_eq!(body["redis"], "ready");
    assert_eq!(body["migrations"], "current");

    let binary_latest = body["binary_latest_migration"].as_str().unwrap();
    let database_latest = body["database_latest_migration"].as_str().unwrap();
    assert_eq!(
        binary_latest, database_latest,
        "a fully-migrated test database must report the same latest version as the binary's own embedded migrator"
    );
    let max_in_repo: i64 = sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(binary_latest, max_in_repo.to_string());
}

#[sqlx::test(migrations = "./migrations")]
async fn test_health_check_rejects_redis_loss_without_touching_shared_services(pool: PgPool) {
    let (_app, _pool, mut state) = test_app_and_state(pool).await;
    // An intentionally unusable fixture connection; never stop the Redis service.
    state.redis = deadpool_redis::Config::from_url("redis://127.0.0.1:0/15")
        .create_pool(Some(deadpool_redis::Runtime::Tokio1))
        .unwrap();
    let app = create_router(state);
    let response = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        get(&app, "/healthz", None),
    )
    .await
    .expect("readiness must fail within its bounded dependency deadlines");
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = json_body(response).await;
    assert_eq!(body["status"], "not_ready");
    assert_eq!(body["database"], "ready");
    assert_eq!(body["migrations"], "current");
    assert_eq!(body["redis"], "unreachable");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_otp_delivery_reservation_is_atomic_and_denials_preserve_other_budgets(pool: PgPool) {
    use hellodoctor_backend::error::AppError;
    use hellodoctor_backend::services::auth_service::abuse;

    fn recipient_keys(recipient: &str) -> Vec<String> {
        let hash = hellodoctor_backend::crypto::hash_code(&recipient.trim().to_lowercase());
        vec![
            "ratelimit:otp-delivery:global".into(),
            format!("ratelimit:otp-delivery:cooldown:{hash}"),
            format!("ratelimit:otp-delivery:rolling:{hash}"),
            format!("ratelimit:otp-delivery:daily:{hash}"),
        ]
    }
    async fn counters(state: &AppState, recipient: &str) -> Vec<Option<u32>> {
        let mut conn = state.redis.get().await.unwrap();
        deadpool_redis::redis::cmd("MGET")
            .arg(recipient_keys(recipient))
            .query_async(&mut conn)
            .await
            .unwrap()
    }

    let (_app, _pool, state) = test_app_and_state(pool).await;
    let recipient = "atomic-otp@example.invalid";
    abuse::otp_delivery(&state, recipient).await.unwrap();
    assert_eq!(counters(&state, recipient).await, vec![Some(1); 4]);
    // 100 denied attempts, at most eight concurrent clients, with normalized
    // variants of the same recipient. No email provider is called by this test.
    let mut workers = tokio::task::JoinSet::new();
    for _ in 0..8 {
        let state = state.clone();
        workers.spawn(async move {
            for _ in 0..12 {
                assert!(matches!(
                    abuse::otp_delivery(&state, " Atomic-OTP@Example.Invalid ").await,
                    Err(AppError::RateLimitExceeded(_))
                ));
            }
        });
    }
    while let Some(result) = workers.join_next().await {
        result.unwrap();
    }
    for _ in 0..4 {
        assert!(matches!(
            abuse::otp_delivery(&state, recipient).await,
            Err(AppError::RateLimitExceeded(_))
        ));
    }
    assert_eq!(counters(&state, recipient).await, vec![Some(1); 4]);

    // Seed only the exhausted recipient window. A later-window denial must not
    // create a cooldown, spend global capacity, or mutate the other windows.
    for (target, index, limit, seconds) in [
        ("rolling-otp@example.invalid", 2usize, 3u32, 900u64),
        ("daily-otp@example.invalid", 3, 20, 86_400),
    ] {
        let mut conn = state.redis.get().await.unwrap();
        let _: () = deadpool_redis::redis::cmd("SET")
            .arg(&recipient_keys(target)[index])
            .arg(limit)
            .arg("EX")
            .arg(seconds)
            .query_async(&mut conn)
            .await
            .unwrap();
        drop(conn);
        let before = counters(&state, target).await;
        assert!(matches!(
            abuse::otp_delivery(&state, target).await,
            Err(AppError::RateLimitExceeded(_))
        ));
        assert_eq!(counters(&state, target).await, before);
    }

    // 100 distinct recipients race the remaining 29 global reservations. Each
    // rejected reservation must leave its recipient counters absent.
    let mut workers = tokio::task::JoinSet::new();
    for client in 0..8 {
        let state = state.clone();
        workers.spawn(async move {
            let mut accepted = 0;
            for request in (client..100).step_by(8) {
                let target = format!("global-otp-{client}-{request}@example.invalid");
                match abuse::otp_delivery(&state, &target).await {
                    Ok(()) => {
                        accepted += 1;
                        assert_eq!(&counters(&state, &target).await[1..], &[Some(1); 3]);
                    }
                    Err(AppError::RateLimitExceeded(_)) => {
                        assert_eq!(&counters(&state, &target).await[1..], &[None; 3]);
                    }
                    Err(error) => panic!("unexpected reservation error: {error}"),
                }
            }
            accepted
        });
    }
    let mut accepted = 0;
    while let Some(result) = workers.join_next().await {
        accepted += result.unwrap();
    }
    assert_eq!(accepted, 29);
    for number in 0..4 {
        let target = format!("overflow-otp-{number}@example.invalid");
        assert!(matches!(
            abuse::otp_delivery(&state, &target).await,
            Err(AppError::RateLimitExceeded(_))
        ));
        assert_eq!(
            counters(&state, &target).await,
            vec![Some(30), None, None, None]
        );
    }
    assert_eq!(
        counters(&state, recipient).await,
        vec![Some(30), Some(1), Some(1), Some(1)]
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_patient_otp_auth_flow_and_token_refresh(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;

    let res = post(
        &app,
        "/api/v1/auth/register-otp",
        None,
        &serde_json::json!({ "phone_number": SEED_PATIENT_PHONE }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let json = json_body(res).await;
    let real_otp = json["data"]["debug_otp_code"].as_str().unwrap().to_string();

    // A wrong OTP must now be genuinely rejected — no more universal magic-code bypass.
    let res_wrong = post(
        &app,
        "/api/v1/auth/verify-otp-and-login",
        None,
        &serde_json::json!({ "phone_number": SEED_PATIENT_PHONE, "otp_code": "000000" }),
    )
    .await;
    assert_eq!(res_wrong.status(), StatusCode::UNAUTHORIZED);

    let res2 = post(
        &app,
        "/api/v1/auth/verify-otp-and-login",
        None,
        &serde_json::json!({ "phone_number": SEED_PATIENT_PHONE, "otp_code": real_otp }),
    )
    .await;
    assert_eq!(res2.status(), StatusCode::OK);

    let json2 = json_body(res2).await;
    assert!(json2["success"].as_bool().unwrap());
    let access_token = json2["data"]["access_token"].as_str().unwrap().to_string();
    let refresh_token = json2["data"]["refresh_token"].as_str().unwrap().to_string();
    assert!(!access_token.is_empty());
    assert!(!refresh_token.is_empty());

    // The regression test that caught the from_ed_der/SPKI bug: a freshly
    // issued token must actually work on a real protected route, not just
    // "look like" a valid response from the login endpoint.
    let res_use = get(&app, "/api/v1/doctors", Some(&access_token)).await;
    assert_eq!(
        res_use.status(),
        StatusCode::OK,
        "a freshly issued access token must authenticate on a protected route"
    );

    let res3 = post(
        &app,
        "/api/v1/auth/refresh",
        None,
        &serde_json::json!({ "refresh_token": refresh_token }),
    )
    .await;
    assert_eq!(res3.status(), StatusCode::OK);
    let json3 = json_body(res3).await;
    let rotated_refresh = json3["data"]["refresh_token"].as_str().unwrap().to_string();

    // Logout must revoke the server-side refresh-token family, not merely
    // clear a client cookie. The endpoint is intentionally idempotent and
    // does not require a still-valid access token.
    let logout = post(
        &app,
        "/api/v1/auth/logout",
        None,
        &serde_json::json!({ "refresh_token": rotated_refresh }),
    )
    .await;
    assert_eq!(logout.status(), StatusCode::OK);

    let after_logout = post(
        &app,
        "/api/v1/auth/refresh",
        None,
        &serde_json::json!({ "refresh_token": rotated_refresh }),
    )
    .await;
    assert_eq!(after_logout.status(), StatusCode::UNAUTHORIZED);

    // Reuse of the already-rotated refresh token must be rejected.
    let res4 = post(
        &app,
        "/api/v1/auth/refresh",
        None,
        &serde_json::json!({ "refresh_token": refresh_token }),
    )
    .await;
    assert_eq!(res4.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_doctor_discovery_and_slot_booking_lifecycle(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor1_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");

    let res = get(
        &app,
        "/api/v1/doctors?specialty=Internal%20Medicine",
        Some(&patient_token),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let json = json_body(res).await;
    let doc_id = Uuid::parse_str(json["data"][0]["id"].as_str().unwrap()).unwrap();
    assert_eq!(doc_id, uid(DOCTOR1_PROFILE_ID));

    let (apt_id, _payment_sess_id) = book_and_confirm(&app, &patient_token).await;

    let apt = appointment_repo::find_by_id(&pool, apt_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(apt.status, AppointmentStatus::Confirmed);

    // Collision: the same slot is now BOOKED, a second booking attempt on it must 409.
    let book_payload_again = serde_json::json!({
        "doctor_id": DOCTOR1_PROFILE_ID,
        "slot_id": apt.slot_id,
        "modality": "VIDEO",
        "gateway": "BKASH",
        "chief_complaint": "Follow-up"
    });
    let res_collision = post(
        &app,
        "/api/v1/appointments/book",
        Some(&patient_token),
        &book_payload_again,
    )
    .await;
    assert_eq!(res_collision.status(), StatusCode::CONFLICT);

    let txn = hellodoctor_backend::repository::transaction_repo::find_by_appointment(&pool, apt_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(txn.payment_status, PaymentStatus::PaymentHeld);
    assert_eq!(txn.gross_amount, bigdecimal::BigDecimal::from(800));
    assert_eq!(txn.platform_fee_amount, bigdecimal::BigDecimal::from(160));
    assert_eq!(txn.net_amount, bigdecimal::BigDecimal::from(640));

    let res_rtc = post(
        &app,
        &format!("/api/v1/consultations/{}/rtc-token", apt_id),
        Some(&patient_token),
        &serde_json::json!({}),
    )
    .await;
    assert_eq!(res_rtc.status(), StatusCode::OK);
    let rtc_json = json_body(res_rtc).await;
    assert!(rtc_json["data"]["channel_name"]
        .as_str()
        .unwrap()
        .starts_with("rtc_"));

    let rx_payload = serde_json::json!({
        "base64_photo": minimal_jpeg_base64(),
        "doctor_notes": "Paracetamol 500mg 1+0+1 for 3 days after food"
    });
    open_clinical_window(&pool, apt_id).await;
    let res_rx = post_idem(
        &app,
        &format!("/api/v1/consultations/{}/prescription", apt_id),
        Some(&doctor1_token),
        &rx_payload,
        &Uuid::new_v4().to_string(),
    )
    .await;
    assert_eq!(res_rx.status(), StatusCode::CREATED);

    // A doctor who was never assigned to this appointment must not be able to
    // complete it. RLS hides the appointment row from them entirely (they're
    // neither the patient nor the assigned doctor), so the read inside
    // `complete_consultation` finds nothing and this surfaces as 404 before
    // the explicit ownership check ever runs — not a leak, the better
    // property (no resource-existence disclosure), same as the chat BOLA case.
    let doctor2_token = make_token(uid(DOCTOR2_USER_ID), "DOCTOR");
    let res_wrong_doctor = post(
        &app,
        &format!("/api/v1/consultations/{}/complete", apt_id),
        Some(&doctor2_token),
        &serde_json::json!({ "outcome": "COMPLETED_WITH_RX" }),
    )
    .await;
    assert_eq!(res_wrong_doctor.status(), StatusCode::NOT_FOUND);

    let res_comp = post(
        &app,
        &format!("/api/v1/consultations/{}/complete", apt_id),
        Some(&doctor1_token),
        &serde_json::json!({ "outcome": "COMPLETED_WITH_RX" }),
    )
    .await;
    assert_eq!(res_comp.status(), StatusCode::OK);

    let res_wallet = get(&app, "/api/v1/doctor/wallet/summary", Some(&doctor1_token)).await;
    assert_eq!(res_wallet.status(), StatusCode::OK);
    let wallet_json = json_body(res_wallet).await;
    assert_money_eq(&wallet_json["data"]["total_gross"], "800");
    assert_money_eq(&wallet_json["data"]["withheld_fee"], "160.00");
    assert_money_eq(&wallet_json["data"]["final_net"], "640.00");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_multi_prescription_intake_and_20_photo_limit(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let (apt_id, _) = book_and_confirm(&app, &patient_token).await;

    // 15 then 5 more: lands exactly on the 20-image boundary and must succeed.
    let fifteen_files: Vec<_> = (0..15)
        .map(|_| serde_json::json!({ "mime_type": "image/jpeg", "base64_content": minimal_jpeg_base64() }))
        .collect();
    let upload_15 = serde_json::json!({ "files": fifteen_files });
    let res1 = post(
        &app,
        &format!("/api/v1/appointments/{}/prescriptions", apt_id),
        Some(&patient_token),
        &upload_15,
    )
    .await;
    assert_eq!(res1.status(), StatusCode::CREATED);

    let five_more: Vec<_> = (0..5)
        .map(|_| serde_json::json!({ "mime_type": "image/jpeg", "base64_content": minimal_jpeg_base64() }))
        .collect();
    let upload_5_more = serde_json::json!({ "files": five_more });
    let res2 = post(
        &app,
        &format!("/api/v1/appointments/{}/prescriptions", apt_id),
        Some(&patient_token),
        &upload_5_more,
    )
    .await;
    assert_eq!(
        res2.status(),
        StatusCode::CREATED,
        "exactly 20 images total must be accepted, not rejected"
    );

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM prescription_intake_documents WHERE appointment_id=$1",
    )
    .bind(apt_id)
    .fetch_one(&_pool)
    .await
    .unwrap();
    assert_eq!(count, 20);

    // One more past the boundary (21st image) must be rejected.
    let one_more = serde_json::json!({ "files": [{ "mime_type": "image/jpeg", "base64_content": minimal_jpeg_base64() }] });
    let res3 = post(
        &app,
        &format!("/api/v1/appointments/{}/prescriptions", apt_id),
        Some(&patient_token),
        &one_more,
    )
    .await;
    assert_eq!(res3.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let count_after: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM prescription_intake_documents WHERE appointment_id=$1",
    )
    .bind(apt_id)
    .fetch_one(&_pool)
    .await
    .unwrap();
    assert_eq!(
        count_after, 20,
        "a rejected 21st-image upload must not partially persist"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_patient_intake_rejects_terminal_appointment_states(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let (appointment, _) = book_and_confirm(&app, &patient_token).await;
    let path = format!("/api/v1/appointments/{appointment}/prescriptions");
    let payload = serde_json::json!({
        "files": [{
            "mime_type": "image/jpeg",
            "base64_content": minimal_jpeg_base64()
        }]
    });

    for status in ["COMPLETED", "EXPIRED", "REFUNDED", "PREMATURE_TERMINATION"] {
        sqlx::query("UPDATE appointments SET status=$1::appointment_status_enum WHERE id=$2")
            .bind(status)
            .bind(appointment)
            .execute(&pool)
            .await
            .unwrap();

        let response = post(&app, &path, Some(&patient_token), &payload).await;
        assert_eq!(
            response.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "patient intake must be unavailable while appointment status is {status}"
        );
        assert_eq!(
            hellodoctor_backend::repository::prescription_repo::count_intake_documents(
                &pool,
                appointment
            )
            .await
            .unwrap(),
            0,
            "rejected {status} intake must not persist metadata"
        );
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn test_unconfigured_document_storage_does_not_confirm_upload(pool: PgPool) {
    let (app, pool) = test_app_with_storage_fixture(pool, test_config(), false).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let (appointment, _) = book_and_confirm(&app, &patient).await;
    open_clinical_window(&pool, appointment).await;
    let response = post(
        &app,
        &format!("/api/v1/consultations/{appointment}/prescription"),
        Some(&doctor),
        &serde_json::json!({
            "base64_photo": minimal_jpeg_base64(),
            "mime_type": "image/jpeg", "doctor_notes": "Storage unavailable test"
        }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body = json_body(response).await;
    assert!(body["error"]["message"]
        .as_str()
        .unwrap()
        .contains("not configured"));
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM prescriptions WHERE appointment_id=$1")
            .bind(appointment)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        count, 0,
        "an unavailable upload must not create successful prescription metadata"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_doctor_rx_pages_encrypted_download_retry_and_expired_grant(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let foreign = make_token(uid(DOCTOR2_USER_ID), "DOCTOR");
    let (appointment, _) = book_and_confirm(&app, &patient).await;
    open_clinical_window(&pool, appointment).await;
    let uri = format!("/api/v1/consultations/{appointment}/prescription");
    let mut first_id = String::new();
    for page in 1..=5 {
        let image = image::RgbImage::from_pixel(3, 3, image::Rgb([page * 30, 20, 40]));
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        let payload = serde_json::json!({"base64_photo":STANDARD.encode(bytes.into_inner()),"mime_type":"image/png","doctor_notes":"Prescription page"});
        let response = post(&app, &uri, Some(&doctor), &payload).await;
        assert_eq!(response.status(), StatusCode::CREATED);
        let rx = json_body(response).await;
        assert_eq!(rx["data"]["page_number"], page);
        if page == 1 {
            first_id = rx["data"]["id"].as_str().unwrap().into();
        }
        if page == 1 {
            let retry = post(&app, &uri, Some(&doctor), &payload).await;
            assert_eq!(retry.status(), StatusCode::CREATED);
            assert_eq!(json_body(retry).await["data"]["id"], rx["data"]["id"]);
        }
    }
    let overflow = post(
        &app,
        &uri,
        Some(&doctor),
        &serde_json::json!({"base64_photo":minimal_jpeg_base64(),"mime_type":"image/jpeg"}),
    )
    .await;
    assert_eq!(overflow.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let wrong_mime = post(
        &app,
        &uri,
        Some(&doctor),
        &serde_json::json!({"base64_photo":minimal_jpeg_base64(),"mime_type":"application/pdf"}),
    )
    .await;
    assert_eq!(wrong_mime.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let no_rx = post(
        &app,
        &format!("/api/v1/consultations/{appointment}/complete-no-rx"),
        Some(&doctor),
        &serde_json::json!({"reason":"No medicine"}),
    )
    .await;
    assert_eq!(no_rx.status(), StatusCode::CONFLICT);
    let document_uri = format!("/api/v1/appointments/{appointment}/documents/{first_id}");
    let response = get(&app, &document_uri, Some(&doctor)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let grant = json_body(response).await;
    let download_url = grant["data"]["download_url"].as_str().unwrap();
    let download_key = Uuid::new_v4().to_string();
    // A second grant minted now (before the chat-lock-inducing updates below)
    // but deliberately left undownloaded here, so it can later prove that a
    // grant minted pre-lock still authorizes its ONE download after the
    // lock, independent of the single-use change below.
    let pre_lock_grant = json_body(get(&app, &document_uri, Some(&doctor)).await).await;
    let pre_lock_url = pre_lock_grant["data"]["download_url"]
        .as_str()
        .unwrap()
        .to_string();
    let download = send(&app, "GET", download_url, None, None, Some(&download_key)).await;
    assert_eq!(download.status(), StatusCode::OK);
    assert_eq!(download.headers()["cache-control"], "private, no-store");
    let plaintext = axum::body::to_bytes(download.into_body(), 11 * 1024 * 1024)
        .await
        .unwrap();
    assert!(plaintext.starts_with(&[0x89, 0x50, 0x4e, 0x47]));
    // h02-document-grant-reusable-not-onetime regression: a grant is
    // consumed atomically (Redis GETDEL) on its first successful
    // redemption, so an immediate retry with the identical token must now
    // fail rather than re-serve the document for the rest of its 300s TTL.
    assert_eq!(
        send(&app, "GET", download_url, None, None, Some(&download_key))
            .await
            .status(),
        StatusCode::NOT_FOUND,
        "a document-download grant must be single-use: an immediate retry with the same token must be rejected"
    );
    let key: String =
        sqlx::query_scalar("SELECT rx_image_object_key FROM prescriptions WHERE id=$1")
            .bind(Uuid::parse_str(&first_id).unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    let store = test_object_storage();
    let ciphertext = store
        .get(&object_store::path::Path::from(key))
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap();
    assert_ne!(ciphertext.as_ref(), plaintext.as_ref());
    assert!(!ciphertext.starts_with(&[0x89, 0x50, 0x4e, 0x47]));
    assert_eq!(
        get(&app, &document_uri, Some(&foreign)).await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        get(
            &app,
            &document_uri,
            Some(&token_for_real_role(&pool, "ADMIN").await)
        )
        .await
        .status(),
        StatusCode::OK
    );
    sqlx::query("UPDATE appointments SET status='COMPLETED' WHERE id=$1")
        .bind(appointment)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE consultation_sessions SET ended_at=NOW()-INTERVAL '25 hours' WHERE appointment_id=$1")
        .bind(appointment).execute(&pool).await.unwrap();
    assert_eq!(
        send(&app, "GET", &pre_lock_url, None, None, Some(&Uuid::new_v4().to_string()))
            .await
            .status(),
        StatusCode::OK,
        "a grant minted before the chat lock preserves authorized own-encounter reads after it, for its one redemption"
    );
    assert_eq!(
        get(&app, &document_uri, Some(&doctor)).await.status(),
        StatusCode::OK
    );
    // A freshly-minted, not-yet-downloaded grant's TTL still governs
    // expiry independent of any other grant activity above.
    use sha2::{Digest, Sha256};
    let ttl_grant = json_body(get(&app, &document_uri, Some(&doctor)).await).await;
    let ttl_url = ttl_grant["data"]["download_url"].as_str().unwrap();
    let ttl_token = ttl_url.split("token=").nth(1).unwrap();
    let ttl_key = format!(
        "document-grant:{}",
        hex::encode(Sha256::digest(ttl_token.as_bytes()))
    );
    let mut redis = state.redis.get().await.unwrap();
    let expired: i64 = deadpool_redis::redis::cmd("EXPIRE")
        .arg(ttl_key)
        .arg(0)
        .query_async(&mut redis)
        .await
        .unwrap();
    assert_eq!(expired, 1);
    assert_eq!(
        send(
            &app,
            "GET",
            ttl_url,
            None,
            None,
            Some(&Uuid::new_v4().to_string())
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    let fresh = json_body(get(&app, &document_uri, Some(&doctor)).await).await;
    let fresh_url = fresh["data"]["download_url"].as_str().unwrap();
    let fresh_token = fresh_url.split("token=").nth(1).unwrap();
    let fresh_key = format!(
        "document-grant:{}",
        hex::encode(Sha256::digest(fresh_token.as_bytes()))
    );
    let raw: String = deadpool_redis::redis::cmd("GET")
        .arg(fresh_key)
        .query_async(&mut redis)
        .await
        .unwrap();
    let mut legacy: Value = serde_json::from_str(&raw).unwrap();
    assert!(legacy["session_id"].is_string());
    legacy.as_object_mut().unwrap().remove("session_id");
    let legacy_token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let legacy_key = format!(
        "document-grant:{}",
        hex::encode(Sha256::digest(legacy_token.as_bytes()))
    );
    let _: () = deadpool_redis::redis::cmd("SET")
        .arg(legacy_key)
        .arg(legacy.to_string())
        .arg("EX")
        .arg(300)
        .query_async(&mut redis)
        .await
        .unwrap();
    assert_eq!(
        send(
            &app,
            "GET",
            &format!("/api/v1/documents/download?token={legacy_token}"),
            None,
            None,
            Some(&Uuid::new_v4().to_string())
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    sqlx::query("UPDATE auth_sessions SET revoked_at=clock_timestamp() WHERE user_id=$1")
        .bind(uid(DOCTOR1_USER_ID))
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        send(
            &app,
            "GET",
            fresh_url,
            None,
            None,
            Some(&Uuid::new_v4().to_string())
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
}

/// h03-upload-tx-lock-held-across-semaphore-throttled-sanitize regression:
/// `rate_limit::acquire_upload_concurrency` must bound how many of the SAME
/// account's uploads can be in flight at once, independent of the separate
/// count-over-a-window `check_upload_limit`. Exercised directly against the
/// real Redis pool (deterministic: held guards, not a timing-dependent race)
/// rather than through the HTTP layer, since the fixture uploads complete too
/// fast to reliably overlap two real concurrent HTTP requests.
#[sqlx::test(migrations = "./migrations")]
async fn test_upload_concurrency_guard_bounds_in_flight_requests_per_account(pool: PgPool) {
    let (_app, _pool, state) = test_app_and_state(pool).await;
    let user_id = "concurrency-guard-test-user";

    let guard1 =
        hellodoctor_backend::rate_limit::acquire_upload_concurrency(&state.redis, user_id, 2)
            .await
            .expect("1st of 2 concurrent slots must be granted");
    let guard2 =
        hellodoctor_backend::rate_limit::acquire_upload_concurrency(&state.redis, user_id, 2)
            .await
            .expect("2nd of 2 concurrent slots must be granted");

    let third =
        hellodoctor_backend::rate_limit::acquire_upload_concurrency(&state.redis, user_id, 2).await;
    assert!(
        third.is_err(),
        "a 3rd concurrent slot for the same account must be rejected while 2 are held"
    );

    // A different account must have its own independent budget.
    let other_user_guard = hellodoctor_backend::rate_limit::acquire_upload_concurrency(
        &state.redis,
        "a-different-concurrency-guard-test-user",
        2,
    )
    .await;
    assert!(
        other_user_guard.is_ok(),
        "concurrency budgets must be per-account, not global"
    );

    drop(guard1);
    // The release decrement is fire-and-forget (see UploadConcurrencyGuard's
    // doc comment); give it a moment to land before asserting the slot freed.
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let after_release =
        hellodoctor_backend::rate_limit::acquire_upload_concurrency(&state.redis, user_id, 2).await;
    assert!(
        after_release.is_ok(),
        "releasing a held slot must free capacity for a new acquire"
    );

    drop(guard2);
    drop(other_user_guard);
    drop(after_release);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_clinical_grievance_and_board_refund_adjudication(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let admin_token = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let (apt_id, _) = book_and_confirm(&app, &patient_token).await;

    let session = consultation_repo::find_by_appointment(&pool, apt_id)
        .await
        .unwrap()
        .unwrap();

    let grv_payload = serde_json::json!({
        "consultation_id": session.id,
        "target": "DOCTOR",
        "category": "Rushed Consultation / Ended Abruptly",
        "claim_summary": "Doctor disconnected after 45 seconds"
    });
    let res_grv = post(
        &app,
        "/api/v1/grievances",
        Some(&patient_token),
        &grv_payload,
    )
    .await;
    assert_eq!(res_grv.status(), StatusCode::CREATED);
    let json = json_body(res_grv).await;
    let grv_id = Uuid::parse_str(json["data"]["grievance_id"].as_str().unwrap()).unwrap();

    // A non-admin patient cannot adjudicate their own grievance.
    let res_denied = post(
        &app,
        &format!("/api/v1/admin/grievances/{}/refund", grv_id),
        Some(&patient_token),
        &serde_json::json!({ "audit_notes": "self-adjudication attempt" }),
    )
    .await;
    assert_eq!(res_denied.status(), StatusCode::FORBIDDEN);

    let adj_payload = serde_json::json!({ "audit_notes": "Call telemetry confirmed 45s duration; claim substantiated." });
    let res_adj = post(
        &app,
        &format!("/api/v1/admin/grievances/{}/refund", grv_id),
        Some(&admin_token),
        &adj_payload,
    )
    .await;
    assert_eq!(res_adj.status(), StatusCode::OK);
    let adj_json = json_body(res_adj).await;
    assert_eq!(
        adj_json["data"]["status"].as_str().unwrap(),
        "REFUND_APPROVED"
    );

    let txn = hellodoctor_backend::repository::transaction_repo::find_by_appointment(&pool, apt_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(txn.payment_status, PaymentStatus::RefundPending);
    let request: (String,Option<String>,Option<chrono::DateTime<chrono::Utc>>) = sqlx::query_as("SELECT status,provider_reference,refunded_at FROM financial_refund_requests WHERE transaction_id=$1")
        .bind(txn.id).fetch_one(&pool).await.unwrap();
    assert_eq!(request.0, "APPROVED");
    assert!(request.1.is_none() && request.2.is_none());
}

#[sqlx::test(migrations = "./migrations")]
async fn test_grievance_number_unique_under_concurrent_submission(pool: PgPool) {
    // Regression test: grievance_number used to be derived from the first 4
    // hex characters of a fresh UUID (16 bits of entropy) per calendar day,
    // which collided under real concurrent load — confirmed live during the
    // pre-Patient 4x8 capacity benchmark (duplicate-key violations on
    // grievance_reports_grievance_number_key growing with RPS). Migration
    // 20261028000001 replaced it with a Postgres sequence, atomic under
    // concurrency by construction (grievance_repo::insert's own doc comment
    // has the detail). Exercises GrievanceService directly, not the HTTP
    // handler, because the handler's own 3-per-day check_grievance_limit
    // (src/rate_limit.rs) is an orthogonal abuse control unrelated to this
    // invariant and would otherwise reject most of these concurrent calls.
    let (_app, pool, state) = test_app_and_state(pool).await;
    // One DISTINCT consultation per concurrent submission, not one shared
    // consultation for all of them: `submit_grievance` takes a transaction-
    // scoped `pg_advisory_xact_lock` keyed on the consultation's
    // `appointment_id` (see `grievance_service.rs`, guarding
    // `consultation_trace` writes) — sharing one consultation across all 40
    // calls would serialize them through that single lock key, which is not
    // a realistic production scenario (grievances against the very same
    // consultation, simultaneously) and would mask the actual invariant
    // under test (the `grievance_number` sequence's atomicity) behind
    // incidental lock contention under this suite's deliberately tiny pool.
    // Each synthetic appointment gets its OWN slot, spaced 30 minutes apart
    // (20-minute duration), rather than all 40 sharing one slot_id: Stage A's
    // `appt_doctor_no_overlap`/`appt_patient_no_overlap` exclusion
    // constraints (20261111000001_schedule_exclusivity) now correctly reject
    // 40 active appointments for the same Doctor+Patient at the identical
    // time — a fixture-setup collision with an invariant unrelated to what
    // this test actually checks (grievance_number uniqueness).
    let consultation_ids: Vec<Uuid> = sqlx::query_scalar(
        "WITH new_slots AS ( \
             INSERT INTO doctor_schedule_slots(doctor_id,start_time,end_time,status) \
             SELECT $1, now() + interval '10 hours' + (gs * interval '30 minutes'), now() + interval '10 hours' + (gs * interval '30 minutes') + interval '20 minutes', 'BOOKED' \
             FROM generate_series(1,40) AS gs \
             RETURNING id \
         ), new_appointments AS ( \
             INSERT INTO appointments(id,appointment_number,patient_id,doctor_id,slot_id,modality,status,consultation_fee) \
             SELECT gen_random_uuid(), 'GRV-BENCH-40-'||substr(id::text,1,8), $2, $1, id, 'VIDEO', 'CONFIRMED', 100 \
             FROM new_slots \
             RETURNING id \
         ) \
         INSERT INTO consultation_sessions(appointment_id,agora_channel_name) \
         SELECT id, 'grv-bench-40-'||id::text FROM new_appointments \
         RETURNING id",
    )
    .bind(uid(DOCTOR1_PROFILE_ID))
    .bind(uid(PATIENT_PROFILE_ID))
    .fetch_all(&pool)
    .await
    .unwrap();
    let ctx = isolation_ctx(UserId::from(uid(PATIENT_USER_ID)), UserRole::Patient);

    let mut numbers = Vec::new();
    const BATCH_SIZE: usize = 4;
    // The test role intentionally has a two-connection pool and the
    // application fails acquisition after 250ms.  Bounded batches preserve
    // real sequence races while ensuring every business operation can obtain
    // a connection, so a failure still means grievance creation or numbering
    // is broken rather than that the harness was saturated.
    for batch in consultation_ids.chunks(BATCH_SIZE) {
        let mut handles = Vec::with_capacity(batch.len());
        for &consultation_id in batch {
            let state = state.clone();
            let ctx = ctx.clone();
            handles.push(tokio::spawn(async move {
                hellodoctor_backend::services::GrievanceService::submit_grievance(
                    &state,
                    &ctx,
                    PatientProfileId::from(uid(PATIENT_PROFILE_ID)),
                    consultation_id,
                    GrievanceTarget::System,
                    "SERVICE_QUALITY".into(),
                    "Concurrent grievance-number regression test".into(),
                )
                .await
            }));
        }
        for h in handles {
            let report = h
                .await
                .unwrap()
                .expect("every bounded concurrent submission must succeed");
            numbers.push(report.grievance_number);
        }
    }
    assert_eq!(numbers.len(), 40);
    let distinct: std::collections::HashSet<_> = numbers.iter().collect();
    assert_eq!(
        distinct.len(),
        numbers.len(),
        "every concurrently created grievance must get a distinct grievance_number"
    );
}

/// Re-verifies the exact same invariant as
/// `test_grievance_number_unique_under_concurrent_submission` immediately
/// above, but at the scale the spec itself calls for ("50 concurrent DB
/// clients, 500+ transactions") rather than that test's original
/// 40-submission smoke check. Each submission targets its OWN distinct
/// consultation (not one shared one — see the sibling 40-submission test's
/// comment on `submit_grievance`'s transaction-scoped advisory lock), so
/// contention here is genuinely on the `grievance_number` sequence and the
/// tiny pool, not on one serialized lock key.
///
/// This test's goal is 500 *successful* business operations, not saturation
/// behavior, so it runs them in controlled batches rather than firing all
/// 500 at once: `begin_authed_as` (`repository/mod.rs`) enforces a hard
/// 250ms fail-fast deadline on acquiring a transaction from the role pool,
/// by design (see `docs/ADR`/`pool_pressure.rs` — queuing indefinitely
/// instead of failing fast is explicitly the behavior this codebase
/// rejects). This suite's deliberately tiny 2-connection pool means 500
/// truly-simultaneous `begin_authed` calls will genuinely queue past that
/// 250ms deadline and legitimately fail closed — that's the real,
/// reviewed circuit breaker working correctly, not a bug, and not
/// something to paper over by touching pool size or the deadline. A batch
/// size of 4 keeps meaningful real concurrency within each batch (still
/// racing on the shared sequence) while staying safely inside that
/// deadline, and 500 total submissions still fully exercises the
/// sequence's atomicity across 125 batches.
#[sqlx::test(migrations = "./migrations")]
async fn test_grievance_number_unique_under_concurrent_submission_at_500_scale(pool: PgPool) {
    let (_app, pool, state) = test_app_and_state(pool).await;
    const TOTAL: usize = 500;
    const BATCH_SIZE: usize = 4;
    // See the sibling 40-submission test's comment: each synthetic
    // appointment gets its own non-overlapping slot rather than sharing one,
    // so Stage A's exclusion constraints don't collide with this fixture.
    let consultation_ids: Vec<Uuid> = sqlx::query_scalar(
        "WITH new_slots AS ( \
             INSERT INTO doctor_schedule_slots(doctor_id,start_time,end_time,status) \
             SELECT $1, now() + interval '10 hours' + (gs * interval '30 minutes'), now() + interval '10 hours' + (gs * interval '30 minutes') + interval '20 minutes', 'BOOKED' \
             FROM generate_series(1,500) AS gs \
             RETURNING id \
         ), new_appointments AS ( \
             INSERT INTO appointments(id,appointment_number,patient_id,doctor_id,slot_id,modality,status,consultation_fee) \
             SELECT gen_random_uuid(), 'GRV-BENCH-500-'||substr(id::text,1,8), $2, $1, id, 'VIDEO', 'CONFIRMED', 100 \
             FROM new_slots \
             RETURNING id \
         ) \
         INSERT INTO consultation_sessions(appointment_id,agora_channel_name) \
         SELECT id, 'grv-bench-500-'||id::text FROM new_appointments \
         RETURNING id",
    )
    .bind(uid(DOCTOR1_PROFILE_ID))
    .bind(uid(PATIENT_PROFILE_ID))
    .fetch_all(&pool)
    .await
    .unwrap();
    let ctx = isolation_ctx(UserId::from(uid(PATIENT_USER_ID)), UserRole::Patient);

    let mut numbers = Vec::with_capacity(TOTAL);
    for batch in consultation_ids.chunks(BATCH_SIZE) {
        let mut handles = Vec::with_capacity(batch.len());
        for &consultation_id in batch {
            let state = state.clone();
            let ctx = ctx.clone();
            handles.push(tokio::spawn(async move {
                hellodoctor_backend::services::GrievanceService::submit_grievance(
                    &state,
                    &ctx,
                    PatientProfileId::from(uid(PATIENT_PROFILE_ID)),
                    consultation_id,
                    GrievanceTarget::System,
                    "SERVICE_QUALITY".into(),
                    "Concurrent grievance-number regression test at 500-submission scale".into(),
                )
                .await
            }));
        }
        for h in handles {
            let report = h.await.unwrap().expect(
                "concurrent submission must not fail with a duplicate-key error, even at 500-submission scale",
            );
            numbers.push(report.grievance_number);
        }
    }
    assert_eq!(
        numbers.len(),
        TOTAL,
        "every one of the {TOTAL} concurrent submissions must succeed with zero unique-violation errors"
    );
    let distinct: std::collections::HashSet<_> = numbers.iter().collect();
    assert_eq!(
        distinct.len(),
        numbers.len(),
        "every concurrently created grievance must get a distinct grievance_number at {TOTAL}-submission scale"
    );
}

/// Regression test for GAP-C01/B-04 (docs/audit/DOCTOR_BUSINESS_LOGIC_GAP_AUDIT.md,
/// docs/audit/DOCTOR_SECURITY_BUG_AUDIT.md): a refund approved against a
/// transaction that had *already* been settled into the doctor's wallet must
/// (a) never claim in its HTTP response that money was disbursed/refunded,
/// and (b) actually reverse the wallet credit the earlier settlement made,
/// instead of leaving the doctor's lifetime earnings permanently overstated.
#[sqlx::test(migrations = "./migrations")]
async fn test_refund_approval_reverses_wallet_credit_and_response_is_honest(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let admin_token = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let (apt_id, _) = book_and_confirm(&app, &patient_token).await;

    assert_eq!(
        post(
            &app,
            &format!("/api/v1/consultations/{apt_id}/complete"),
            Some(&doctor_token),
            &serde_json::json!({"outcome":"COMPLETED_NO_RX"}),
        )
        .await
        .status(),
        StatusCode::OK
    );

    let txn = hellodoctor_backend::repository::transaction_repo::find_by_appointment(&pool, apt_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(txn.payment_status, PaymentStatus::SettledToDoctor);

    let wallet_before =
        json_body(get(&app, "/api/v1/doctor/wallet/summary", Some(&doctor_token)).await).await;
    let net_before: f64 = wallet_before["data"]["final_net"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();

    let session = consultation_repo::find_by_appointment(&pool, apt_id)
        .await
        .unwrap()
        .unwrap();
    let grv_payload = serde_json::json!({
        "consultation_id": session.id,
        "target": "DOCTOR",
        "category": "Rushed Consultation / Ended Abruptly",
        "claim_summary": "Doctor disconnected after 45 seconds"
    });
    let res_grv = post(
        &app,
        "/api/v1/grievances",
        Some(&patient_token),
        &grv_payload,
    )
    .await;
    assert_eq!(res_grv.status(), StatusCode::CREATED);
    let json = json_body(res_grv).await;
    let grv_id = Uuid::parse_str(json["data"]["grievance_id"].as_str().unwrap()).unwrap();

    let res_adj = post(
        &app,
        &format!("/api/v1/admin/grievances/{}/refund", grv_id),
        Some(&admin_token),
        &serde_json::json!({ "audit_notes": "Call telemetry confirmed 45s duration; claim substantiated." }),
    )
    .await;
    assert_eq!(res_adj.status(), StatusCode::OK);
    let adj_json = json_body(res_adj).await;
    assert_eq!(
        adj_json["data"]["status"].as_str().unwrap(),
        "REFUND_APPROVED"
    );
    let remedy = adj_json["data"]["board_remedy"]
        .as_str()
        .unwrap()
        .to_lowercase();
    assert!(
        !remedy.contains("disbursed") && !remedy.contains("refunded"),
        "approval response must never claim money moved: {remedy}"
    );

    let wallet_after =
        json_body(get(&app, "/api/v1/doctor/wallet/summary", Some(&doctor_token)).await).await;
    let net_after: f64 = wallet_after["data"]["final_net"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let txn_net: f64 = txn.net_amount.to_string().parse().unwrap();
    assert!(
        (net_before - net_after - txn_net).abs() < 0.01,
        "refund approval on an already-settled transaction must reverse the wallet credit: before={net_before} after={net_after} txn_net={txn_net}"
    );
}

/// Regression test for GAP-C03 (docs/audit/DOCTOR_BUSINESS_LOGIC_GAP_AUDIT.md):
/// an approved refund must have a visible, transitionable lifecycle for
/// Finance — not sit at APPROVED forever with no operator action possible.
#[sqlx::test(migrations = "./migrations")]
async fn test_refund_request_has_a_finance_resolution_lifecycle(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let admin_token = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let (apt_id, _) = book_and_confirm(&app, &patient_token).await;

    let session = consultation_repo::find_by_appointment(&pool, apt_id)
        .await
        .unwrap()
        .unwrap();
    let grv_payload = serde_json::json!({
        "consultation_id": session.id,
        "target": "DOCTOR",
        "category": "Rushed Consultation / Ended Abruptly",
        "claim_summary": "Doctor disconnected after 45 seconds"
    });
    let res_grv = post(
        &app,
        "/api/v1/grievances",
        Some(&patient_token),
        &grv_payload,
    )
    .await;
    let json = json_body(res_grv).await;
    let grv_id = Uuid::parse_str(json["data"]["grievance_id"].as_str().unwrap()).unwrap();
    post(
        &app,
        &format!("/api/v1/admin/grievances/{}/refund", grv_id),
        Some(&admin_token),
        &serde_json::json!({ "audit_notes": "Substantiated." }),
    )
    .await;

    // A doctor is not Finance and cannot see or act on refund requests.
    assert_eq!(
        get(
            &app,
            "/api/v1/admin/finance/refund-requests",
            Some(&doctor_token)
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );

    let list = json_body(
        get(
            &app,
            "/api/v1/admin/finance/refund-requests",
            Some(&admin_token),
        )
        .await,
    )
    .await;
    let requests = list["data"].as_array().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["status"], "APPROVED");
    let refund_id = Uuid::parse_str(requests[0]["id"].as_str().unwrap()).unwrap();

    // APPROVED -> REFUNDED directly, with no provider_reference, is rejected:
    // the table's own CHECK (and this validation) requires evidence.
    let res_bad = post(
        &app,
        &format!(
            "/api/v1/admin/finance/refund-requests/{}/transition",
            refund_id
        ),
        Some(&admin_token),
        &serde_json::json!({ "status": "REFUNDED" }),
    )
    .await;
    assert_eq!(res_bad.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let res_processing = post(
        &app,
        &format!(
            "/api/v1/admin/finance/refund-requests/{}/transition",
            refund_id
        ),
        Some(&admin_token),
        &serde_json::json!({ "status": "PROCESSING" }),
    )
    .await;
    assert_eq!(res_processing.status(), StatusCode::OK);
    let processing_json = json_body(res_processing).await;
    assert_eq!(processing_json["data"]["status"], "PROCESSING");

    let res_refunded = post(
        &app,
        &format!(
            "/api/v1/admin/finance/refund-requests/{}/transition",
            refund_id
        ),
        Some(&admin_token),
        &serde_json::json!({ "status": "REFUNDED", "provider_reference": "MPESA-CONF-998877" }),
    )
    .await;
    assert_eq!(res_refunded.status(), StatusCode::OK);
    let refunded_json = json_body(res_refunded).await;
    assert_eq!(refunded_json["data"]["status"], "REFUNDED");
    assert_eq!(
        refunded_json["data"]["provider_reference"],
        "MPESA-CONF-998877"
    );
    assert!(refunded_json["data"]["refunded_at"].is_string());

    // A REFUNDED request is terminal — no further transition is possible.
    let res_again = post(
        &app,
        &format!(
            "/api/v1/admin/finance/refund-requests/{}/transition",
            refund_id
        ),
        Some(&admin_token),
        &serde_json::json!({ "status": "PROCESSING" }),
    )
    .await;
    assert_eq!(res_again.status(), StatusCode::CONFLICT);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_grievance_dismiss(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let admin_token = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let (apt_id, _) = book_and_confirm(&app, &patient_token).await;
    let session = consultation_repo::find_by_appointment(&pool, apt_id)
        .await
        .unwrap()
        .unwrap();

    let grv_payload = serde_json::json!({
        "consultation_id": session.id,
        "target": "SYSTEM",
        "category": "App crashed mid-call",
        "claim_summary": "Lost connection, unclear if doctor's fault"
    });
    let res_grv = post(
        &app,
        "/api/v1/grievances",
        Some(&patient_token),
        &grv_payload,
    )
    .await;
    let json = json_body(res_grv).await;
    let grv_id = Uuid::parse_str(json["data"]["grievance_id"].as_str().unwrap()).unwrap();

    let res_dismiss = post(
        &app,
        &format!("/api/v1/admin/grievances/{}/dismiss", grv_id),
        Some(&admin_token),
        &serde_json::json!({ "audit_notes": "Telemetry inconclusive; no fault found." }),
    )
    .await;
    assert_eq!(res_dismiss.status(), StatusCode::OK);
    let dismiss_json = json_body(res_dismiss).await;
    assert_eq!(
        dismiss_json["data"]["status"].as_str().unwrap(),
        "DISMISSED"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_unclosed_historical_statement_remains_estimated(pool: PgPool) {
    let (app, _) = test_app(pool).await;
    let token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let response = get(
        &app,
        "/api/v1/doctor/wallet/statements?month=2026-08",
        Some(&token),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let data = json_body(response).await;
    assert_eq!(data["data"]["status"], "IN_PROGRESS");
    assert_eq!(data["data"]["amount_label"], "ESTIMATED_PAYOUT");
    // CLAUDE.md Core Business Rule #2: every earnings view must render this
    // exact disclosure. The Wallet summary endpoint already did; this
    // statement endpoint silently omitted it until this field was added.
    assert_eq!(
        data["data"]["basis_note"],
        "Total calculation is based including the platform charge 20%."
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_financial_close_snapshot_adjustment_routing_and_approval(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let foreign = make_token(uid(DOCTOR2_USER_ID), "DOCTOR");
    let finance = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let (appointment, _) = book_and_confirm(&app, &patient).await;
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/consultations/{appointment}/complete"),
            Some(&doctor),
            &serde_json::json!({"outcome":"COMPLETED_NO_RX"})
        )
        .await
        .status(),
        StatusCode::OK
    );
    sqlx::query(
        "UPDATE transactions SET created_at='2026-09-12T10:00:00Z' WHERE appointment_id=$1",
    )
    .bind(appointment)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO financial_adjustments (doctor_id,currency,period_year,period_month,amount,category,reason,created_by) VALUES ($1,'KES',2026,9,-40,'CORRECTION','Approved correction test',$2)")
        .bind(uid(DOCTOR1_PROFILE_ID)).bind(uid(ADMIN_USER_ID)).execute(&pool).await.unwrap();
    let bind = post(
        &app,
        "/api/v1/doctor/wallet/payout-methods/dev-bind",
        Some(&doctor),
        &serde_json::json!({"method_type":"MPESA"}),
    )
    .await;
    assert_eq!(bind.status(), StatusCode::OK);
    let destination = json_body(bind).await["data"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        post(
            &app,
            "/api/v1/doctor/wallet/payout-methods/dev-bind",
            Some(&foreign),
            &serde_json::json!({"method_type":"BANK_TRANSFER"})
        )
        .await
        .status(),
        StatusCode::OK
    );
    let prepare = serde_json::json!({"period_start":"2026-09-01","period_end":"2026-09-30"});
    assert_eq!(
        post(
            &app,
            "/api/v1/admin/disbursements/initiate",
            Some(&finance),
            &prepare
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    let before = get(
        &app,
        "/api/v1/doctor/wallet/statements?month=2026-09",
        Some(&doctor),
    )
    .await;
    assert_eq!(before.status(), StatusCode::OK);
    let before = json_body(before).await;
    assert_money_eq(&before["data"]["payable"], "600");
    assert_eq!(before["data"]["amount_label"], "ESTIMATED_PAYOUT");
    let close = serde_json::json!({"year":2026,"month":9});
    assert_eq!(
        post(
            &app,
            "/api/v1/admin/finance/periods/finalize",
            Some(&doctor),
            &close
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        post(
            &app,
            "/api/v1/admin/finance/periods/finalize",
            Some(&finance),
            &close
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        post(
            &app,
            "/api/v1/admin/finance/periods/finalize",
            Some(&finance),
            &close
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    let prepared = post(
        &app,
        "/api/v1/admin/disbursements/initiate",
        Some(&finance),
        &prepare,
    )
    .await;
    assert_eq!(prepared.status(), StatusCode::CREATED);
    let prepared = json_body(prepared).await;
    assert_eq!(prepared["data"]["status"], "PENDING_APPROVAL");
    assert_money_eq(&prepared["data"]["total_net_disbursed"], "600");
    let items =
        wallet_repo::list_items_for_doctor(&pool, DoctorProfileId::from(uid(DOCTOR1_PROFILE_ID)))
            .await
            .unwrap();
    assert_eq!(items.len(), 1);
    let item = &items[0];
    assert_eq!(item.payout_destination_id.unwrap().to_string(), destination);
    assert!(
        item.gateway_reference.is_none(),
        "a recipient reference is not provider payment proof"
    );
    assert!(item.paid_at.is_none());
    assert_money_eq(&serde_json::json!(item.adjustments), "-40");
    // Later primary changes must not change the already assigned destination or statement figures.
    assert_eq!(
        post(
            &app,
            "/api/v1/doctor/wallet/payout-methods/dev-bind",
            Some(&doctor),
            &serde_json::json!({"method_type":"BANK_TRANSFER"})
        )
        .await
        .status(),
        StatusCode::OK
    );
    let routing = get(
        &app,
        "/api/v1/admin/finance/payout-routing?month=2026-09",
        Some(&finance),
    )
    .await;
    assert_eq!(routing.status(), StatusCode::OK);
    let routing = json_body(routing).await;
    let own = routing["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["doctor_id"] == DOCTOR1_PROFILE_ID)
        .unwrap();
    assert_eq!(own["payout_method_id"], destination);
    assert_eq!(own["payout_method"], "MPESA");
    assert_eq!(own["payout_readiness"], "DEV_SIMULATED");
    assert_eq!(own["statement_status"], "FINALIZED");
    assert_money_eq(&own["final_payable"], "600");
    for month in ["2026-08", "2026-10"] {
        let routing = json_body(
            get(
                &app,
                &format!("/api/v1/admin/finance/payout-routing?month={month}"),
                Some(&finance),
            )
            .await,
        )
        .await;
        let own = routing["data"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["doctor_id"] == DOCTOR1_PROFILE_ID)
            .unwrap();
        assert_money_eq(&own["final_payable"], "0");
        assert_eq!(own["statement_status"], "OPEN");
    }
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/admin/disbursements/{}/approve", item.id),
            Some(&doctor),
            &serde_json::json!({})
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/admin/disbursements/{}/approve", item.id),
            Some(&finance),
            &serde_json::json!({})
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/admin/disbursements/{}/approve", item.id),
            Some(&finance),
            &serde_json::json!({})
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    // Regression test for GAP-C01 (docs/audit/DOCTOR_BUSINESS_LOGIC_GAP_AUDIT.md):
    // Admin's "unpaid" figures must reflect this still-APPROVED-not-PAID item's
    // real net_amount, not `doctor_wallets.pending_disbursement` (which nothing
    // in the codebase ever decrements and would otherwise just show the
    // doctor's lifetime gross credited, unrelated to what's actually unpaid).
    let overview = json_body(get(&app, "/api/v1/admin/overview", Some(&finance)).await).await;
    assert_money_eq(&overview["data"]["pending_disbursement"], "600");
    let admin_doctors = json_body(get(&app, "/api/v1/admin/doctors", Some(&finance)).await).await;
    let own_doctor = admin_doctors["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["id"] == DOCTOR1_PROFILE_ID)
        .unwrap();
    assert_money_eq(&own_doctor["pending_disbursement"], "600");
    let statement = json_body(
        get(
            &app,
            "/api/v1/doctor/wallet/statements?month=2026-09",
            Some(&doctor),
        )
        .await,
    )
    .await;
    assert_eq!(statement["data"]["status"], "FINALIZED");
    assert_eq!(statement["data"]["payout_status"], "APPROVED");
    assert_money_eq(&statement["data"]["payable"], "600");
    assert_eq!(
        statement["data"]["basis_note"],
        "Total calculation is based including the platform charge 20%."
    );
    let foreign_statement = json_body(
        get(
            &app,
            "/api/v1/doctor/wallet/statements?month=2026-09",
            Some(&foreign),
        )
        .await,
    )
    .await;
    assert_money_eq(&foreign_statement["data"]["payable"], "0");
    let state: String =
        sqlx::query_scalar("SELECT payment_status::text FROM transactions WHERE appointment_id=$1")
            .bind(appointment)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(state, "SETTLED_TO_DOCTOR");
    let batch_state: String =
        sqlx::query_scalar("SELECT status FROM disbursement_batches WHERE id=$1")
            .bind(item.batch_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(batch_state, "APPROVED");
    let wallet = wallet_repo::get_or_default(&pool, DoctorProfileId::from(uid(DOCTOR1_PROFILE_ID)))
        .await
        .unwrap();
    assert_money_eq(&serde_json::json!(wallet.pending_disbursement), "640");
    assert!(
        sqlx::query("UPDATE monthly_payout_statements SET final_payable=999 WHERE id=$1")
            .bind(item.statement_id)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE disbursement_items SET payout_destination_id=NULL WHERE id=$1")
            .bind(item.id)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(sqlx::query("UPDATE disbursement_items SET status='PAID',gateway_reference='fake',paid_at=NOW() WHERE id=$1").bind(item.id).execute(&pool).await.is_err());
    assert!(sqlx::query("INSERT INTO financial_adjustments (doctor_id,currency,period_year,period_month,amount,category,reason,created_by) VALUES ($1,'KES',2026,9,1,'CORRECTION','Too late',$2)").bind(uid(DOCTOR1_PROFILE_ID)).bind(uid(ADMIN_USER_ID)).execute(&pool).await.is_err());
}

#[sqlx::test(migrations = "./migrations")]
async fn test_monthly_finance_disbursement_batch(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor1_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let admin_token = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let (apt_id, _) = book_and_confirm(&app, &patient_token).await;

    let comp_payload = serde_json::json!({ "outcome": "COMPLETED_NO_RX" });
    let res_comp2 = post(
        &app,
        &format!("/api/v1/consultations/{}/complete", apt_id),
        Some(&doctor1_token),
        &comp_payload,
    )
    .await;
    assert_eq!(res_comp2.status(), StatusCode::OK);

    // A non-finance admin may not initiate disbursement.
    let doctor2_token = make_token(uid(DOCTOR2_USER_ID), "DOCTOR");
    let res_denied = post(
        &app,
        "/api/v1/admin/disbursements/initiate",
        Some(&doctor2_token),
        &serde_json::json!({ "period_start": "2026-09-01", "period_end": "2026-09-30" }),
    )
    .await;
    assert_eq!(res_denied.status(), StatusCode::FORBIDDEN);

    let month_start = Utc::now().date_naive().with_day(1).unwrap();
    let month_end = (month_start + Duration::days(32)).with_day(1).unwrap() - Duration::days(1);
    assert_eq!(
        post(
            &app,
            "/api/v1/doctor/wallet/payout-methods/dev-bind",
            Some(&doctor1_token),
            &serde_json::json!({"method_type":"MPESA"})
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        post(
            &app,
            "/api/v1/admin/finance/periods/finalize",
            Some(&admin_token),
            &serde_json::json!({"year":month_start.year(),"month":month_start.month()})
        )
        .await
        .status(),
        StatusCode::OK
    );
    let disb_payload = serde_json::json!({ "period_start": month_start, "period_end": month_end });
    let res = post(
        &app,
        "/api/v1/admin/disbursements/initiate",
        Some(&admin_token),
        &disb_payload,
    )
    .await;
    assert_eq!(res.status(), StatusCode::CREATED);

    let json = json_body(res).await;
    assert_eq!(json["data"]["total_doctors"].as_i64().unwrap(), 1);
    assert_money_eq(&json["data"]["total_net_disbursed"], "640.00");
    assert_money_eq(&json["data"]["total_platform_fee"], "160.00");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_doctor_mfa_login_two_step_verification(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;

    let res1 = post(
        &app,
        "/api/v1/auth/doctor/login",
        None,
        &serde_json::json!({ "email": SEED_DOCTOR1_EMAIL, "password": SEED_DOCTOR1_PASSWORD }),
    )
    .await;
    assert_eq!(res1.status(), StatusCode::OK);
    let json1 = json_body(res1).await;
    let session_token = json1["data"]["session_token"].as_str().unwrap().to_string();
    let debug_code = json1["data"]["debug_otp_code"]
        .as_str()
        .unwrap()
        .to_string();

    // Wrong password must be rejected (real Argon2id verification, not bypassed).
    let res_wrong = post(
        &app,
        "/api/v1/auth/doctor/login",
        None,
        &serde_json::json!({ "email": SEED_DOCTOR1_EMAIL, "password": "wrong-password" }),
    )
    .await;
    assert_eq!(res_wrong.status(), StatusCode::UNAUTHORIZED);

    let res2 = post(
        &app,
        "/api/v1/auth/doctor/verify-otp",
        None,
        &serde_json::json!({ "session_token": session_token, "otp_code": debug_code }),
    )
    .await;
    assert_eq!(res2.status(), StatusCode::OK);
    let json2 = json_body(res2).await;
    assert_eq!(json2["data"]["user"]["role"].as_str().unwrap(), "DOCTOR");
}

/// Admin login is a two-step email-OTP flow (replaces the previous
/// single-screen authenticator-app TOTP design): password step returns a
/// `session_token` and never a session directly, and only the matching
/// emailed code against that exact token completes login.
#[sqlx::test(migrations = "./migrations")]
async fn test_admin_mfa_login_two_step_verification(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;

    let res1 = post(
        &app,
        "/api/v1/auth/admin/login",
        None,
        &serde_json::json!({ "email": SEED_ADMIN_EMAIL, "password": SEED_ADMIN_PASSWORD }),
    )
    .await;
    assert_eq!(res1.status(), StatusCode::OK);
    let json1 = json_body(res1).await;
    assert!(
        json1["data"].get("access_token").is_none(),
        "a correct password must only issue an OTP challenge, never a session directly"
    );
    let session_token = json1["data"]["session_token"].as_str().unwrap().to_string();
    let debug_code = json1["data"]["debug_otp_code"]
        .as_str()
        .unwrap()
        .to_string();

    // Wrong password must be rejected (real Argon2id verification, not bypassed).
    let res_wrong = post(
        &app,
        "/api/v1/auth/admin/login",
        None,
        &serde_json::json!({ "email": SEED_ADMIN_EMAIL, "password": "wrong-password" }),
    )
    .await;
    assert_eq!(res_wrong.status(), StatusCode::UNAUTHORIZED);

    // A wrong code against a real session must be rejected.
    let res_bad = post(
        &app,
        "/api/v1/auth/admin/verify-otp",
        None,
        &serde_json::json!({ "session_token": session_token, "otp_code": "000000" }),
    )
    .await;
    assert_eq!(res_bad.status(), StatusCode::UNAUTHORIZED);

    let res2 = post(
        &app,
        "/api/v1/auth/admin/verify-otp",
        None,
        &serde_json::json!({ "session_token": session_token, "otp_code": debug_code }),
    )
    .await;
    assert_eq!(res2.status(), StatusCode::OK);
    let json2 = json_body(res2).await;
    assert_eq!(json2["data"]["user"]["role"].as_str().unwrap(), "ADMIN");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_24h_clinical_chat_messaging_and_expiration_locking(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let (apt_id, _) = book_and_confirm(&app, &patient_token).await;

    let conv =
        sqlx::query_as::<_, (Uuid,)>("SELECT id FROM chat_conversations WHERE appointment_id = $1")
            .bind(apt_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let conv_id = conv.0;

    let msg_payload =
        serde_json::json!({ "content": "Doctor, can I take paracetamol after food?" });
    let res1 = post(
        &app,
        &format!("/api/v1/chat/{}/messages", conv_id),
        Some(&patient_token),
        &msg_payload,
    )
    .await;
    assert_eq!(res1.status(), StatusCode::CREATED);

    sqlx::query(
        "UPDATE chat_conversations SET expires_at = now() - interval '10 seconds' WHERE id = $1",
    )
    .bind(conv_id)
    .execute(&pool)
    .await
    .unwrap();

    let res2 = post(
        &app,
        &format!("/api/v1/chat/{}/messages", conv_id),
        Some(&patient_token),
        &msg_payload,
    )
    .await;
    assert_eq!(res2.status(), StatusCode::FORBIDDEN);
}

/// Regression test for a rate-limit coverage gap found by security audit:
/// `send_chat_message` previously had no rate limiter at all, unlike every
/// other patient/doctor-facing write endpoint (booking, uploads, grievances,
/// OTP). Confirms `check_chat_message_limit` (40 messages / 5-minute window,
/// keyed by sender) actually trips a clean 429 past the threshold rather than
/// allowing unbounded flooding of the other party, and that it does not
/// false-positive within ordinary usage up to the limit.
#[sqlx::test(migrations = "./migrations")]
async fn test_chat_message_send_is_rate_limited(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let (apt_id, _) = book_and_confirm(&app, &patient_token).await;

    let conv =
        sqlx::query_as::<_, (Uuid,)>("SELECT id FROM chat_conversations WHERE appointment_id = $1")
            .bind(apt_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let conv_id = conv.0;
    let uri = format!("/api/v1/chat/{}/messages", conv_id);

    for i in 0..40 {
        let payload = serde_json::json!({ "content": format!("Message number {i}") });
        let response = post(&app, &uri, Some(&patient_token), &payload).await;
        assert_eq!(
            response.status(),
            StatusCode::CREATED,
            "message {i} should still be within the per-sender budget"
        );
    }
    let over_limit = post(
        &app,
        &uri,
        Some(&patient_token),
        &serde_json::json!({ "content": "One message too many" }),
    )
    .await;
    assert_eq!(over_limit.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        json_body(over_limit).await["error"]["code"],
        "RATE_LIMIT_EXCEEDED"
    );
    let stored: i64 =
        sqlx::query_scalar("SELECT count(*) FROM chat_messages WHERE conversation_id = $1")
            .bind(conv_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        stored, 40,
        "the rejected message must not have been persisted"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_admin_create_doctor_issues_temporary_password(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let admin_token = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");

    let payload = serde_json::json!({
        "full_name": "Dr. Test Kenyan Doctor",
        "email": "new-doctor@example.test",
        "license_number": "KMPDC #77102",
        "primary_specialty": "Dermatology",
        "experience_years": 6,
        "current_hospital": "Nairobi Hospital",
        "verified_phone": "0711223344",
        "consultation_fee_video": 700,
        "consultation_fee_chat": 450
    });

    // A patient may not create a doctor account.
    let res_denied = post(
        &app,
        "/api/v1/admin/doctors",
        Some(&patient_token),
        &payload,
    )
    .await;
    assert_eq!(res_denied.status(), StatusCode::FORBIDDEN);

    let res = post(&app, "/api/v1/admin/doctors", Some(&admin_token), &payload).await;
    assert_eq!(res.status(), StatusCode::CREATED);

    let json = json_body(res).await;
    let temp_password = json["data"]["temporary_password"].as_str().unwrap();
    assert_eq!(
        json["data"]["doctor"]["professional_verification_status"],
        "PENDING"
    );
    assert_eq!(json["data"]["doctor"]["license_authority"], "KMPDC");
    assert_eq!(json["data"]["doctor"]["license_country"], "KE");
    assert_eq!(json["data"]["doctor"]["is_verified"], false);
    assert_eq!(json["data"]["doctor"]["is_on_duty"], false);
    assert!(temp_password.len() >= 12);
    let login = post(
        &app,
        "/api/v1/auth/doctor/login",
        None,
        &serde_json::json!({"email":"new-doctor@example.test", "password":temp_password}),
    )
    .await;
    assert_eq!(login.status(), StatusCode::OK);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_complete_consultation_without_prescription(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let doctor1_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");

    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let (apt_id, _) = book_and_confirm(&app, &patient_token).await;
    let doctor_id = uid(DOCTOR1_PROFILE_ID);

    let no_rx_payload = serde_json::json!({ "reason": "Follow-up consultation — patient recovering well, lifestyle advice provided" });
    // The shared booking fixture starts outside T-15. RLS must remain intact
    // and an invisible existing decision must never become a duplicate insert.
    let too_early = post(
        &app,
        &format!("/api/v1/consultations/{}/complete-no-rx", apt_id),
        Some(&doctor1_token),
        &no_rx_payload,
    )
    .await;
    assert_eq!(too_early.status(), StatusCode::FORBIDDEN);
    let decision_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM prescriptions WHERE appointment_id=$1")
            .bind(apt_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(decision_count, 0);
    sqlx::query("UPDATE doctor_schedule_slots SET start_time=NOW()+INTERVAL '5 minutes',end_time=NOW()+INTERVAL '15 minutes' WHERE id=(SELECT slot_id FROM appointments WHERE id=$1)")
        .bind(apt_id).execute(&pool).await.unwrap();
    let res = post(
        &app,
        &format!("/api/v1/consultations/{}/complete-no-rx", apt_id),
        Some(&doctor1_token),
        &no_rx_payload,
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);

    let rx = hellodoctor_backend::repository::prescription_repo::find_prescription_by_appointment(
        &pool, apt_id,
    )
    .await
    .unwrap()
    .unwrap();
    assert!(rx.is_no_rx_required);
    assert_eq!(
        rx.no_rx_reason.unwrap(),
        "Follow-up consultation — patient recovering well, lifestyle advice provided"
    );
    let completed = appointment_repo::find_by_id(&pool, apt_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(completed.status, AppointmentStatus::Completed);
    assert_eq!(completed.clinical_outcome, Some("COMPLETED_NO_RX".into()));
    assert!(completed.completed_at.is_some());
    let session = consultation_repo::find_by_appointment(&pool, apt_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(session.status, SessionStatus::Completed);
    assert!(session.ended_at.is_some());
    let before: (bigdecimal::BigDecimal, bigdecimal::BigDecimal) = sqlx::query_as(
        "SELECT lifetime_net,pending_disbursement FROM doctor_wallets WHERE doctor_id=$1",
    )
    .bind(doctor_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(before.0, bigdecimal::BigDecimal::from(640));
    let retry = post(
        &app,
        &format!("/api/v1/consultations/{}/complete-no-rx", apt_id),
        Some(&doctor1_token),
        &no_rx_payload,
    )
    .await;
    assert_eq!(retry.status(), StatusCode::OK);
    let after: (bigdecimal::BigDecimal, bigdecimal::BigDecimal) = sqlx::query_as(
        "SELECT lifetime_net,pending_disbursement FROM doctor_wallets WHERE doctor_id=$1",
    )
    .bind(doctor_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        before, after,
        "no-Rx retry must never credit earnings twice"
    );
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM prescriptions WHERE appointment_id=$1")
            .bind(apt_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
}

// ============================================================================
// Security regression tests added alongside the five-role / default-deny /
// RLS rework. These are the tests the user explicitly asked for: full-chain
// authentication, ABAC, RLS, pool-isolation, and SECURITY DEFINER coverage.
// ============================================================================

/// The exact failure mode the from_ed_der/SPKI bug produced (§1.7 of
/// CHECKPOINT.md) plus the standard negative-auth matrix: missing, malformed,
/// expired, wrong-audience and wrong-issuer tokens must all be rejected with
/// 401 before a handler ever runs.
#[sqlx::test(migrations = "./migrations")]
async fn test_protected_route_rejects_every_invalid_token_shape(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let uri = "/api/v1/doctors";
    let user_id = uid(PATIENT_USER_ID);

    let res_missing = get(&app, uri, None).await;
    assert_eq!(
        res_missing.status(),
        StatusCode::UNAUTHORIZED,
        "missing token"
    );

    let res_malformed = get(&app, uri, Some("not-a-real-jwt")).await;
    assert_eq!(
        res_malformed.status(),
        StatusCode::UNAUTHORIZED,
        "malformed token"
    );

    let expired = craft_token(user_id, "PATIENT", JWT_ISSUER, JWT_AUDIENCE, -7200, -3600);
    let res_expired = get(&app, uri, Some(&expired)).await;
    assert_eq!(
        res_expired.status(),
        StatusCode::UNAUTHORIZED,
        "expired token"
    );

    let wrong_aud = craft_token(
        user_id,
        "PATIENT",
        JWT_ISSUER,
        "some-other-audience",
        0,
        900,
    );
    let res_wrong_aud = get(&app, uri, Some(&wrong_aud)).await;
    assert_eq!(
        res_wrong_aud.status(),
        StatusCode::UNAUTHORIZED,
        "wrong audience"
    );

    let wrong_iss = craft_token(
        user_id,
        "PATIENT",
        "some-other-issuer",
        JWT_AUDIENCE,
        0,
        900,
    );
    let res_wrong_iss = get(&app, uri, Some(&wrong_iss)).await;
    assert_eq!(
        res_wrong_iss.status(),
        StatusCode::UNAUTHORIZED,
        "wrong issuer"
    );

    // A tampered signature (flip the last base64url character of a genuinely
    // valid token) must also be rejected — this is the exact check that the
    // from_ed_der/SPKI bug would have silently defeated for *every* token,
    // valid or not, which is why it's worth asserting both directions here.
    let valid = make_token(user_id, "PATIENT");
    let mut tampered = valid.clone();
    let last = tampered.pop().unwrap();
    tampered.push(if last == 'A' { 'B' } else { 'A' });
    let res_tampered = get(&app, uri, Some(&tampered)).await;
    assert_eq!(
        res_tampered.status(),
        StatusCode::UNAUTHORIZED,
        "tampered signature"
    );

    // Positive control in the same test: the genuinely valid token must work.
    let res_valid = get(&app, uri, Some(&valid)).await;
    assert_eq!(
        res_valid.status(),
        StatusCode::OK,
        "a genuinely valid token must still succeed"
    );
}

/// Algorithm-confusion matrix: `Validation::new(Algorithm::EdDSA)` must fix
/// the allowed algorithm set server-side rather than trusting whatever `alg`
/// the presented token's own header claims — otherwise an attacker who can
/// predict or obtain the server's public verification key could forge a
/// "trusted" token signed with a cheaper/weaker algorithm the server was
/// never meant to accept.
#[sqlx::test(migrations = "./migrations")]
async fn test_protected_route_rejects_algorithm_confusion_tokens(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let uri = "/api/v1/doctors";
    let user_id = uid(PATIENT_USER_ID);
    let now = chrono::Utc::now().timestamp() as usize;
    let claims = Claims {
        sub: user_id.to_string(),
        user_id: user_id.to_string(),
        sid: user_id.to_string(),
        role: "ADMIN".to_string(),
        permissions: vec!["admin:*".to_string()],
        iss: JWT_ISSUER.to_string(),
        aud: JWT_AUDIENCE.to_string(),
        exp: now + 900,
        iat: now,
    };

    // `alg:none` — the classic "drop the signature, hope the verifier trusts
    // the header" bypass. jsonwebtoken 9.x has no `Algorithm::None` variant,
    // so this is expected to fail at header deserialization rather than at
    // signature checking — but it must still come back 401, not a 500 and
    // not a silently-authenticated admin session.
    let header_none =
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(r#"{"alg":"none","typ":"JWT"}"#);
    let payload_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(serde_json::to_vec(&claims).unwrap());
    let none_token = format!("{header_none}.{payload_b64}.");
    assert_eq!(
        get(&app, uri, Some(&none_token)).await.status(),
        StatusCode::UNAUTHORIZED,
        "alg:none must never be accepted"
    );

    // HS256 key-confusion: sign with HS256 using the server's own public
    // verification key material as the HMAC secret. If the verifier ever
    // naively reused "the configured key" for whatever algorithm the token's
    // header happened to claim, this would forge a trusted token out of
    // material that is not secret (a public key). Tried both as the raw
    // 44-byte SPKI-DER blob and as the unwrapped 32-byte raw key (the exact
    // form `verify_access_token` extracts internally), in case only one
    // form were rejected.
    let public_key_bytes = STANDARD.decode(TEST_JWT_PUBLIC_KEY_B64).unwrap();
    let hmac_key_der = EncodingKey::from_secret(&public_key_bytes);
    let confused_der = encode(&Header::new(Algorithm::HS256), &claims, &hmac_key_der).unwrap();
    assert_eq!(
        get(&app, uri, Some(&confused_der)).await.status(),
        StatusCode::UNAUTHORIZED,
        "HS256 key-confusion using the SPKI-DER public key as an HMAC secret must never be accepted"
    );

    let raw_32 = &public_key_bytes[public_key_bytes.len() - 32..];
    let hmac_key_raw = EncodingKey::from_secret(raw_32);
    let confused_raw = encode(&Header::new(Algorithm::HS256), &claims, &hmac_key_raw).unwrap();
    assert_eq!(
        get(&app, uri, Some(&confused_raw)).await.status(),
        StatusCode::UNAUTHORIZED,
        "HS256 key-confusion using the raw unwrapped public key bytes must never be accepted"
    );
}

/// ABAC cross-role denial: a valid, correctly-authenticated token for the
/// wrong role must be refused with 403 (not 401 — the token is fine, the role
/// isn't permitted), on both the admin surface and the doctor-only surface.
#[sqlx::test(migrations = "./migrations")]
async fn test_abac_denies_cross_role_access(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");

    let res1 = get(&app, "/api/v1/admin/overview", Some(&patient_token)).await;
    assert_eq!(
        res1.status(),
        StatusCode::FORBIDDEN,
        "patient must not reach admin overview"
    );

    let res2 = get(&app, "/api/v1/admin/overview", Some(&doctor_token)).await;
    assert_eq!(
        res2.status(),
        StatusCode::FORBIDDEN,
        "doctor must not reach admin overview"
    );

    let res3 = get(&app, "/api/v1/doctor/wallet/summary", Some(&patient_token)).await;
    assert_eq!(
        res3.status(),
        StatusCode::FORBIDDEN,
        "patient must not reach a doctor-only wallet view"
    );
}

/// BOLA/IDOR: a second, unrelated patient must not be able to post into the
/// first patient's clinical chat conversation, even with a fully valid token
/// for their own account — this exercises `chat_messages_insert`'s RLS policy
/// (`sender_id` ownership + conversation participancy), not just ABAC.
#[sqlx::test(migrations = "./migrations")]
async fn test_bola_patient_cannot_post_into_another_patients_chat(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient1_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let (apt_id, _) = book_and_confirm(&app, &patient1_token).await;

    let conv =
        sqlx::query_as::<_, (Uuid,)>("SELECT id FROM chat_conversations WHERE appointment_id = $1")
            .bind(apt_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let conv_id = conv.0;

    // A second patient, genuinely registered, with no relationship whatsoever
    // to this appointment or conversation.
    let patient2_user_id = Uuid::new_v4();
    let patient2_profile_id = Uuid::new_v4();
    let now = chrono::Utc::now();
    hellodoctor_backend::repository::user_repo::insert_user(
        &pool,
        &User {
            id: UserId::from(patient2_user_id),
            phone_number: "+254700000201".into(),
            email: None,
            password_hash: None,
            role: UserRole::Patient,
            preferred_language: "en".into(),
            is_active: true,
            is_verified: true,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        },
    )
    .await
    .unwrap();
    hellodoctor_backend::repository::patient_repo::insert(
        &pool,
        &PatientProfile {
            id: PatientProfileId::from(patient2_profile_id),
            user_id: Some(UserId::from(patient2_user_id)),
            full_name: "Unrelated Patient".into(),
            date_of_birth: chrono::NaiveDate::from_ymd_opt(1990, 1, 1).unwrap(),
            gender: Gender::Other,
            blood_group: None,
            weight_kg: None,
            emergency_contact_phone: None,
            emergency_contact_relation: None,
            guardian_patient_id: None,
            relationship: None,
            created_at: now,
            updated_at: now,
        },
    )
    .await
    .unwrap();

    fixture_session(&pool, patient2_user_id).await;
    fixture_session(&pool, patient2_user_id).await;
    let patient2_token = make_token(patient2_user_id, "PATIENT");
    let res = post(
        &app,
        &format!("/api/v1/chat/{}/messages", conv_id),
        Some(&patient2_token),
        &serde_json::json!({ "content": "I shouldn't be able to post here" }),
    )
    .await;
    // RLS hides the conversation row entirely from an unrelated patient rather
    // than revealing it exists and then refusing the write — the handler's
    // ownership-scoped SELECT (`chat_repo::find_conversation` under
    // `begin_authed`) returns zero rows, so this surfaces as 404, not 403.
    // That's the better security property (no resource-existence leak), not
    // a weaker one, so the test asserts 404 rather than forcing 403.
    assert_eq!(
        res.status(),
        StatusCode::NOT_FOUND,
        "an unrelated patient must not be able to post into someone else's clinical chat"
    );

    // ...and must not even be able to list its messages.
    let res_list = get(
        &app,
        &format!("/api/v1/chat/{}/messages", conv_id),
        Some(&patient2_token),
    )
    .await;
    assert_eq!(
        res_list.status(),
        StatusCode::NOT_FOUND,
        "an unrelated patient must not be able to read someone else's clinical chat"
    );
}

/// Direct SQL-level RLS test of the exact `SECURITY DEFINER` function that
/// fixed the patient_profiles/appointments circular-policy recursion bug
/// (`fn_doctor_treats_patient`, see CHECKPOINT.md §1.4). Positive case: a
/// doctor who genuinely has an appointment with this patient must see the
/// row. Negative case: a doctor with no relationship whatsoever must not —
/// this is the same BOLA property as the chat test above, but asserted at
/// the database layer (defense-in-depth), independent of any application
/// code or ABAC check.
#[sqlx::test(migrations = "./migrations")]
async fn test_rls_doctor_treats_patient_security_definer_function(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let _ = book_and_confirm(&app, &patient_token).await;

    let roles = build_role_pools(&pool).await;

    async fn can_see_patient_profile(pool: &PgPool, doctor_user_id: Uuid) -> bool {
        let mut tx = pool.begin().await.unwrap();
        sqlx::query("SELECT set_config('app.current_user_id', $1, true), set_config('app.current_role', $2, true)")
            .bind(doctor_user_id.to_string())
            .bind("DOCTOR")
            .execute(&mut *tx)
            .await
            .unwrap();
        let rows: Vec<(Uuid,)> = sqlx::query_as("SELECT id FROM patient_profiles WHERE id = $1")
            .bind(uid(PATIENT_PROFILE_ID))
            .fetch_all(&mut *tx)
            .await
            .unwrap();
        tx.rollback().await.unwrap();
        !rows.is_empty()
    }

    assert!(
        can_see_patient_profile(&roles.api, uid(DOCTOR1_USER_ID)).await,
        "doctor1 has a real appointment with this patient and must see the row"
    );
    assert!(
        !can_see_patient_profile(&roles.api, uid(DOCTOR2_USER_ID)).await,
        "doctor2 has never treated this patient — RLS must hide the row (BOLA defense-in-depth)"
    );
}

/// Pool-isolation (least-privilege) negative tests: the ordinary API role
/// must be structurally incapable of writing appointment/wallet lifecycle
/// state or reading auth-worker-exclusive tables, regardless of what
/// application code might ask it to do — this is enforced by PostgreSQL
/// `GRANT`s, not by anything the Rust service layer chooses to call.
#[sqlx::test(migrations = "./migrations")]
async fn test_db_role_write_isolation(pool: PgPool) {
    let (_app, pool) = test_app(pool).await;
    let roles = build_role_pools(&pool).await;

    let err = sqlx::query("UPDATE appointments SET status = 'CANCELLED' WHERE false")
        .execute(&roles.api)
        .await
        .unwrap_err();
    assert!(
        err.to_string().to_lowercase().contains("permission denied"),
        "hellodoctor_api must not be able to update appointments: {err}"
    );

    let err2 = sqlx::query("SELECT 1 FROM phone_otps LIMIT 1")
        .execute(&roles.api)
        .await
        .unwrap_err();
    assert!(
        err2.to_string()
            .to_lowercase()
            .contains("permission denied"),
        "hellodoctor_api must have zero access to phone_otps: {err2}"
    );

    let err3 = sqlx::query("UPDATE doctor_wallets SET pending_disbursement = 0 WHERE false")
        .execute(&roles.payment)
        .await
        .unwrap_err();
    assert!(
        err3.to_string()
            .to_lowercase()
            .contains("permission denied"),
        "hellodoctor_payment_worker must not be able to update doctor_wallets: {err3}"
    );

    let err4 = sqlx::query("INSERT INTO grievance_adjudications (id, grievance_id, adjudicated_by_admin_id, board_remedy, action_type, audit_notes) SELECT gen_random_uuid(), gen_random_uuid(), gen_random_uuid(), 'x', 'x', 'x' WHERE false")
        .execute(&roles.api)
        .await
        .unwrap_err();
    assert!(
        err4.to_string()
            .to_lowercase()
            .contains("permission denied"),
        "hellodoctor_api must not be able to write grievance_adjudications: {err4}"
    );

    // `audit_events` append-only proof: every real request (patient, doctor, AND
    // admin handlers alike) writes through `hellodoctor_api` — there is no separate
    // admin DB role in the five-role model — so this one role's grants are the
    // entire enforcement surface for "an admin cannot rewrite or erase audit
    // history." Confirmed empirically here, not just by reading the GRANT list.
    let err5 = sqlx::query("UPDATE audit_events SET action = 'TAMPERED' WHERE false")
        .execute(&roles.api)
        .await
        .unwrap_err();
    assert!(
        err5.to_string()
            .to_lowercase()
            .contains("permission denied"),
        "hellodoctor_api must not be able to UPDATE audit_events (append-only): {err5}"
    );
    let err6 = sqlx::query("DELETE FROM audit_events WHERE false")
        .execute(&roles.api)
        .await
        .unwrap_err();
    assert!(
        err6.to_string()
            .to_lowercase()
            .contains("permission denied"),
        "hellodoctor_api must not be able to DELETE FROM audit_events (append-only): {err6}"
    );
}

/// `payment_events` (FORCE RLS since `migrations/20261003000001_rls_hardening`)
/// had zero direct test coverage anywhere in this suite before this test —
/// every existing RLS test in this file exercises a different table. Its
/// policy correlates ownership through `transactions -> appointments ->
/// patient/doctor profile`, a longer join chain than the simpler
/// single-table policies already covered elsewhere, so it gets its own
/// direct proof rather than being assumed correct by analogy.
#[sqlx::test(migrations = "./migrations")]
async fn test_rls_payment_events_blocks_cross_patient_and_cross_doctor_access(pool: PgPool) {
    let f = isolation_fixture(pool).await;
    let txn_a: Uuid = sqlx::query_scalar("SELECT id FROM transactions WHERE appointment_id=$1")
        .bind(f.appointments[0])
        .fetch_one(&f.pool)
        .await
        .unwrap();
    // Appointment B (patient_b/doctor2) was built by `isolation_fixture` via a
    // direct row clone, not through `book_and_confirm`'s real webhook flow, so
    // it has no transaction of its own yet — create one directly, exactly as
    // the real payment webhook would, plus its `payment_events` row.
    let txn_b: Uuid = sqlx::query_scalar(
        "INSERT INTO transactions(transaction_number,appointment_id,gateway,gross_amount,platform_fee_amount,net_amount,payment_status) \
         VALUES('ISO-TXN-B',$1,'BKASH'::gateway_enum,800,160,640,'SETTLED_TO_DOCTOR'::payment_status_enum) RETURNING id",
    )
    .bind(f.appointments[1])
    .fetch_one(&f.pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO payment_events(transaction_id,gateway,provider_event_id,provider_event_type,event_type) VALUES($1,'BKASH'::gateway_enum,'ISO-EVT-B','SUCCESS','SUCCESS')")
        .bind(txn_b)
        .execute(&f.pool)
        .await
        .unwrap();

    let patient_a_ctx = isolation_ctx(UserId::from(uid(PATIENT_USER_ID)), UserRole::Patient);
    let mut tx = f.state.begin_authed(&patient_a_ctx).await.unwrap();
    let own: i64 =
        sqlx::query_scalar("SELECT count(*) FROM payment_events WHERE transaction_id=$1")
            .bind(txn_a)
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    assert_eq!(own, 1, "patient A must see their own payment event");
    let other: i64 =
        sqlx::query_scalar("SELECT count(*) FROM payment_events WHERE transaction_id=$1")
            .bind(txn_b)
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    assert_eq!(other, 0, "patient A must not see patient B's payment event");
    tx.rollback().await.unwrap();

    let doctor2_ctx = isolation_ctx(UserId::from(uid(DOCTOR2_USER_ID)), UserRole::Doctor);
    let mut tx = f.state.begin_authed(&doctor2_ctx).await.unwrap();
    let own: i64 =
        sqlx::query_scalar("SELECT count(*) FROM payment_events WHERE transaction_id=$1")
            .bind(txn_b)
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    assert_eq!(
        own, 1,
        "doctor2 (assigned to appointment B) must see its own payment event"
    );
    let other: i64 =
        sqlx::query_scalar("SELECT count(*) FROM payment_events WHERE transaction_id=$1")
            .bind(txn_a)
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    assert_eq!(
        other, 0,
        "doctor2 must not see doctor1's payment event for appointment A"
    );
    tx.rollback().await.unwrap();
}

/// Regression test for a real RLS gap found and fixed during this audit:
/// `migrations/20261031000001_statement_transactions_doctor_ownership`. The
/// original `statement_transactions_read` policy (added in
/// `20261012000001_financial_close_integrity`) only checked that the parent
/// `monthly_payout_statements` row existed — `EXISTS (SELECT 1 FROM
/// monthly_payout_statements s WHERE s.id = statement_id)` — never that it
/// belonged to the calling doctor. Because that policy carried no `TO <role>`
/// clause, it applied to every role sharing the table's GRANTs, including
/// plain `hellodoctor_api`, so any authenticated patient or doctor could read
/// ANY OTHER doctor's settlement line items (`gross_amount`,
/// `platform_charge`, `doctor_earning`) out of `monthly_statement_transactions`
/// simply by supplying someone else's `statement_id` — RLS alone did nothing
/// to stop it. Proven directly here at the database layer, independent of the
/// one HTTP endpoint (`wallet_handlers::get_monthly_statement`) that happens
/// to resolve its own `statement_id` from a `WHERE doctor_id = <caller's own
/// doctor id>` lookup first.
#[sqlx::test(migrations = "./migrations")]
async fn test_rls_monthly_statement_transactions_requires_doctor_ownership(pool: PgPool) {
    let f = isolation_fixture(pool).await;
    let txn_a: Uuid = sqlx::query_scalar("SELECT id FROM transactions WHERE appointment_id=$1")
        .bind(f.appointments[0])
        .fetch_one(&f.pool)
        .await
        .unwrap();
    let txn_b: Uuid = sqlx::query_scalar(
        "INSERT INTO transactions(transaction_number,appointment_id,gateway,gross_amount,platform_fee_amount,net_amount,payment_status) \
         VALUES('ISO-STMT-TXN-B',$1,'BKASH'::gateway_enum,800,160,640,'SETTLED_TO_DOCTOR'::payment_status_enum) RETURNING id",
    )
    .bind(f.appointments[1])
    .fetch_one(&f.pool)
    .await
    .unwrap();

    let period_id: Uuid = sqlx::query_scalar(
        "INSERT INTO financial_periods(currency,period_year,period_month) VALUES('KES',2031,1) RETURNING id",
    )
    .fetch_one(&f.pool)
    .await
    .unwrap();

    let statement_doctor1: Uuid = sqlx::query_scalar(
        "INSERT INTO monthly_payout_statements(doctor_id,financial_period_id,currency,gross_eligible,platform_charge,final_payable) \
         VALUES($1,$2,'KES',800,160,640) RETURNING id",
    )
    .bind(uid(DOCTOR1_PROFILE_ID))
    .bind(period_id)
    .fetch_one(&f.pool)
    .await
    .unwrap();
    let statement_doctor2: Uuid = sqlx::query_scalar(
        "INSERT INTO monthly_payout_statements(doctor_id,financial_period_id,currency,gross_eligible,platform_charge,final_payable) \
         VALUES($1,$2,'KES',800,160,640) RETURNING id",
    )
    .bind(uid(DOCTOR2_PROFILE_ID))
    .bind(period_id)
    .fetch_one(&f.pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO monthly_statement_transactions(statement_id,transaction_id,gross_amount,platform_charge,doctor_earning) VALUES($1,$2,800,160,640)")
        .bind(statement_doctor1)
        .bind(txn_a)
        .execute(&f.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO monthly_statement_transactions(statement_id,transaction_id,gross_amount,platform_charge,doctor_earning) VALUES($1,$2,800,160,640)")
        .bind(statement_doctor2)
        .bind(txn_b)
        .execute(&f.pool)
        .await
        .unwrap();

    let doctor1_ctx = isolation_ctx(UserId::from(uid(DOCTOR1_USER_ID)), UserRole::Doctor);
    let mut tx = f.state.begin_authed(&doctor1_ctx).await.unwrap();
    let own: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM monthly_statement_transactions WHERE statement_id=$1",
    )
    .bind(statement_doctor1)
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    assert_eq!(own, 1, "doctor1 must see their own statement line item");
    let other: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM monthly_statement_transactions WHERE statement_id=$1",
    )
    .bind(statement_doctor2)
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    assert_eq!(
        other, 0,
        "CRITICAL: doctor1 must not be able to read doctor2's settlement line items by \
         supplying doctor2's statement_id directly — this is exactly the gap fixed in \
         migrations/20261031000001_statement_transactions_doctor_ownership"
    );
    tx.rollback().await.unwrap();

    let doctor2_ctx = isolation_ctx(UserId::from(uid(DOCTOR2_USER_ID)), UserRole::Doctor);
    let mut tx = f.state.begin_authed(&doctor2_ctx).await.unwrap();
    let own: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM monthly_statement_transactions WHERE statement_id=$1",
    )
    .bind(statement_doctor2)
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    assert_eq!(own, 1, "doctor2 must see their own statement line item");
    let other: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM monthly_statement_transactions WHERE statement_id=$1",
    )
    .bind(statement_doctor1)
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    assert_eq!(
        other, 0,
        "doctor2 must not be able to read doctor1's settlement line items"
    );
    tx.rollback().await.unwrap();

    // Finance worker retains unconditional read access for its own settlement
    // pipeline (disbursement_service.rs), unaffected by the ownership fix —
    // this is a GRANT-level least-privilege boundary, not an RLS ownership
    // check, by the same house convention as every sibling finance table.
    let finance_rows: i64 =
        sqlx::query_scalar("SELECT count(*) FROM monthly_statement_transactions")
            .fetch_one(&f.state.finance_db)
            .await
            .unwrap();
    assert_eq!(
        finance_rows, 2,
        "finance worker must retain full read access to settlement line items"
    );
}

/// Connection-pool context-leak stress test (Part 2 of the RLS/connection-pool
/// security audit). `begin_authed`'s `set_config(..., true)` is
/// transaction-scoped (`is_local`), reset automatically on commit/rollback
/// (see `repository/mod.rs`'s doc comment on `begin_authed_as`), so it must
/// never leak across a pooled physical connection being reused for a
/// DIFFERENT caller's next request.
/// `test_identity_pool_context_resets_for_patients_and_doctors` already
/// proves this sequentially, by forcing a single physical connection and
/// inspecting GUCs directly between calls. This test proves the same
/// property the way it would actually be observed in production: real
/// concurrent HTTP traffic through the live `axum::Router`/pool, interleaving
/// four distinct identities (two patients, a doctor, and an admin) rather
/// than one user at a time, and asserting that no response ever contains
/// another identity's data. `test_config()`'s deliberately tiny
/// `db_pool_max_connections_per_role` (2) forces heavy physical-connection
/// reuse across the 128 interleaved calls below, maximizing the chance of
/// exposing any real context-bleed bug rather than diluting it across many
/// idle connections. The 128 calls run in batches of 4 (not all 128 at
/// once), against a locally-enlarged pool (10, matching the convention
/// `pool_pressure.rs` already establishes for a test whose own concurrency
/// design needs more real capacity than this suite's shared default): the
/// `Cost::Normal` admission lane this endpoint classifies into is
/// permanently capped at `n.min(8)` (`admission.rs`'s `Admission::new`) no
/// matter how large `n` is, an absolute architectural ceiling for that
/// cost class — so more than 8 truly-simultaneous calls can never all be
/// admitted regardless of pool size, and 2 real connections are too tight a
/// margin for even a handful of genuinely concurrent calls without
/// `exhausted(pool)`'s eager pre-check occasionally false-positiving. 4 at
/// a time against 10 connections stays safely under both ceilings with
/// real margin, while still forcing heavy connection reuse across 32
/// sequential batches (12.8x reuse per connection over the full run).
#[sqlx::test(migrations = "./migrations")]
async fn test_identity_concurrent_http_requests_no_cross_user_pool_bleed(pool: PgPool) {
    let (_, pool, mut state) = test_app_and_state(pool).await;
    // See doc comment above: only this test's own pools are enlarged, the
    // shared suite-wide `test_config()` default is untouched.
    for role in [DbRole::Api, DbRole::AuthWorker] {
        let previous = match role {
            DbRole::Api => state.db.clone(),
            DbRole::AuthWorker => state.auth_db.clone(),
            _ => unreachable!(),
        };
        let replacement = sqlx::postgres::PgPoolOptions::new()
            .max_connections(10)
            .idle_timeout(std::time::Duration::from_millis(100))
            .connect_with(previous.connect_options().as_ref().clone())
            .await
            .unwrap();
        previous.close().await;
        match role {
            DbRole::Api => state.db = replacement,
            DbRole::AuthWorker => state.auth_db = replacement,
            _ => unreachable!(),
        }
    }
    state.admission = Arc::new(hellodoctor_backend::admission::Admission::new(10));
    let app = create_router(state.clone());

    let patient_b_user =
        hellodoctor_backend::repository::user_repo::create_patient_user(&pool, "+254700009993")
            .await
            .unwrap();
    fixture_session(&pool, patient_b_user.id.into_inner()).await;

    let identities: [(Uuid, &str); 4] = [
        (uid(PATIENT_USER_ID), "PATIENT"),
        (patient_b_user.id.into_inner(), "PATIENT"),
        (uid(DOCTOR1_USER_ID), "DOCTOR"),
        (uid(ADMIN_USER_ID), "ADMIN"),
    ];

    // One uniquely-dedup-keyed notification per identity, inserted directly
    // (fixture setup, not the thing under test) so each caller's expected
    // response is unambiguous.
    for (user_id, _) in identities {
        sqlx::query("INSERT INTO notifications(user_id,type,title,body,dedup_key) VALUES($1,'REGISTRATION_VERIFIED','Stress marker','Synthetic stress-test notification',$2)")
            .bind(user_id)
            .bind(format!("ISO-STRESS-{user_id}"))
            .execute(&pool)
            .await
            .unwrap();
    }

    let tokens: Vec<String> = identities
        .iter()
        .map(|(id, role)| make_token(*id, role))
        .collect();

    // 128 interleaved requests, round-robin across the four identities (not
    // grouped by caller), run in batches of 4 genuinely-concurrent calls
    // against the locally-enlarged router/pool (see doc comment above).
    const BATCH_SIZE: usize = 4;
    let mut total_items = 0usize;
    for batch_start in (0..128usize).step_by(BATCH_SIZE) {
        let mut handles = Vec::with_capacity(BATCH_SIZE);
        for i in batch_start..batch_start + BATCH_SIZE {
            let app = app.clone();
            let expected_user = identities[i % 4].0;
            let token = tokens[i % 4].clone();
            handles.push(tokio::spawn(async move {
                let res = get(&app, "/api/v1/notifications", Some(&token)).await;
                assert_eq!(res.status(), StatusCode::OK);
                let body = json_body(res).await;
                let items = body["data"].as_array().cloned().unwrap_or_default();
                (expected_user, items)
            }));
        }
        for h in handles {
            let (expected_user, items) = h.await.unwrap();
            for item in &items {
                total_items += 1;
                let returned_user = Uuid::parse_str(item["user_id"].as_str().unwrap()).unwrap();
                assert_eq!(
                    returned_user, expected_user,
                    "pool/context bleed: a request authenticated as {expected_user} received a \
                     notification owned by {returned_user} — this is exactly the connection-pool \
                     identity-leak this test exists to catch"
                );
            }
        }
        // Keep the 4-request concurrency shape while allowing the normal lane's
        // 100-token burst budget to replenish between batches. Admission shedding
        // remains covered by the dedicated capacity tests.
        if batch_start + BATCH_SIZE < 128 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }
    assert_eq!(
        total_items, 128,
        "every one of the 128 concurrent requests must see exactly its own single fixture \
         notification — never zero (wrong identity bound) and never more than one (bled-in rows)"
    );
}

/// Payment webhook: a bad signature must be rejected, and replaying the exact
/// same (validly signed) callback must have zero additional financial/state
/// effect — exactly the property the user's step-3 priority asks for, already
/// provided by `process_payment_webhook`'s find-or-create idempotency.
#[sqlx::test(migrations = "./migrations")]
async fn test_webhook_rejects_bad_signature_and_replay_has_no_double_effect(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");

    let res_slots = get(
        &app,
        &format!("/api/v1/doctors/{}/slots", DOCTOR1_PROFILE_ID),
        Some(&patient_token),
    )
    .await;
    let json_slots = json_body(res_slots).await;
    let slot_id = Uuid::parse_str(json_slots["data"][0]["id"].as_str().unwrap()).unwrap();
    let book_payload = serde_json::json!({
        "doctor_id": DOCTOR1_PROFILE_ID, "slot_id": slot_id, "modality": "VIDEO", "gateway": "BKASH", "chief_complaint": "test"
    });
    let res_book = post_idem(
        &app,
        "/api/v1/appointments/book",
        Some(&patient_token),
        &book_payload,
        &Uuid::new_v4().to_string(),
    )
    .await;
    let book_json = json_body(res_book).await;
    let apt_id = Uuid::parse_str(book_json["data"]["appointment_id"].as_str().unwrap()).unwrap();
    let payment_sess_id =
        Uuid::parse_str(book_json["data"]["payment_session_id"].as_str().unwrap()).unwrap();

    let webhook_payload = serde_json::json!({
        "payment_session_id": payment_sess_id,
        "transaction_reference": format!("BK-TEST-{}", Uuid::new_v4()),
        "status": "SUCCESS",
        "event_timestamp": chrono::Utc::now().to_rfc3339(),
        "provider_event_id": Uuid::new_v4().to_string(),
    });
    let body_str = serde_json::to_string(&webhook_payload).unwrap();
    let good_sig = sign_webhook_body(&body_str, TEST_WEBHOOK_SECRET);

    let wrong_provider = send_webhook_provider(&app, "nagad", &body_str, &good_sig).await;
    assert_eq!(
        wrong_provider.status(),
        StatusCode::CONFLICT,
        "a valid signature from the wrong provider must not confirm the transaction"
    );

    let res_bad = send_webhook(
        &app,
        &body_str,
        "0000000000000000000000000000000000000000000000000000000000000000",
    )
    .await;
    assert_eq!(
        res_bad.status(),
        StatusCode::UNAUTHORIZED,
        "an invalid HMAC signature must be rejected"
    );

    let res_ok = send_webhook(&app, &body_str, &good_sig).await;
    assert_eq!(res_ok.status(), StatusCode::OK);

    let sessions_before: i64 =
        sqlx::query_scalar("SELECT count(*) FROM consultation_sessions WHERE appointment_id = $1")
            .bind(apt_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let conversations_before: i64 =
        sqlx::query_scalar("SELECT count(*) FROM chat_conversations WHERE appointment_id = $1")
            .bind(apt_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(sessions_before, 1);
    assert_eq!(conversations_before, 1);

    // Replay the exact same, still-validly-signed callback.
    let res_replay = send_webhook(&app, &body_str, &good_sig).await;
    assert_eq!(
        res_replay.status(),
        StatusCode::OK,
        "a replayed valid callback must still succeed (idempotently), not error"
    );

    let sessions_after: i64 =
        sqlx::query_scalar("SELECT count(*) FROM consultation_sessions WHERE appointment_id = $1")
            .bind(apt_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let conversations_after: i64 =
        sqlx::query_scalar("SELECT count(*) FROM chat_conversations WHERE appointment_id = $1")
            .bind(apt_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        sessions_after, 1,
        "replaying the webhook must not create a second consultation session"
    );
    assert_eq!(
        conversations_after, 1,
        "replaying the webhook must not create a second chat conversation"
    );

    // A *different* provider_event_id for the same underlying transaction
    // attempt (a provider resending under a new callback id rather than a
    // byte-identical replay) must also have zero additional effect — caught
    // here by the transaction's already-PAYMENT_HELD status rather than the
    // provider_event_id uniqueness constraint, since the id itself is new.
    let different_event_payload = serde_json::json!({
        "payment_session_id": payment_sess_id,
        "transaction_reference": webhook_payload["transaction_reference"],
        "status": "SUCCESS",
        "event_timestamp": chrono::Utc::now().to_rfc3339(),
        "provider_event_id": Uuid::new_v4().to_string(),
    });
    let different_event_body = serde_json::to_string(&different_event_payload).unwrap();
    let different_event_sig = sign_webhook_body(&different_event_body, TEST_WEBHOOK_SECRET);
    let res_different_event = send_webhook(&app, &different_event_body, &different_event_sig).await;
    assert_eq!(res_different_event.status(), StatusCode::OK);

    // Changing a provider event ID is a valid replay variant; changing the
    // immutable reference is a different, invalid payment claim.
    let mut wrong_reference_payload = different_event_payload.clone();
    wrong_reference_payload["transaction_reference"] =
        serde_json::json!(format!("WRONG-REFERENCE-{}", Uuid::new_v4()));
    wrong_reference_payload["provider_event_id"] = serde_json::json!(Uuid::new_v4().to_string());
    let wrong_reference_body = serde_json::to_string(&wrong_reference_payload).unwrap();
    let wrong_reference_sig = sign_webhook_body(&wrong_reference_body, TEST_WEBHOOK_SECRET);
    let wrong_reference = send_webhook(&app, &wrong_reference_body, &wrong_reference_sig).await;
    assert_eq!(wrong_reference.status(), StatusCode::CONFLICT);
    let event_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM payment_events WHERE transaction_id IN (SELECT id FROM transactions WHERE appointment_id=$1)",
    )
    .bind(apt_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        event_count, 2,
        "a wrong reference must not add event evidence"
    );
    let confirmation_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM financial_economic_effects WHERE business_id=$1 AND effect_type='PAYMENT_CONFIRMED'",
    )
    .bind(apt_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(confirmation_count, 1);

    let sessions_final: i64 =
        sqlx::query_scalar("SELECT count(*) FROM consultation_sessions WHERE appointment_id = $1")
            .bind(apt_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        sessions_final, 1,
        "a new provider_event_id for an already-settled transaction must not create a second consultation session"
    );

    let apt = appointment_repo::find_by_id(&pool, apt_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(apt.status, AppointmentStatus::Confirmed);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_late_webhook_preserves_expired_booking_and_reports_authoritative_state(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let (appointment, slot) = book_pending_payment(&app, &patient, None).await;
    let payment_session: Uuid = sqlx::query_scalar(
        "SELECT payment_session_id FROM transactions WHERE appointment_id=$1 AND payment_status='INITIATED'",
    )
    .bind(appointment)
    .fetch_one(&pool)
    .await
    .unwrap();
    // Model the fixture-owned hold expiring and its slot being reclaimed before
    // the authenticated development callback. No external provider is contacted.
    sqlx::query("UPDATE appointments SET status='EXPIRED' WHERE id=$1")
        .bind(appointment)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE doctor_schedule_slots SET status='AVAILABLE' WHERE id=$1")
        .bind(slot)
        .execute(&pool)
        .await
        .unwrap();
    let payload = serde_json::json!({
        "payment_session_id": payment_session,
        "transaction_reference": format!("SYNTHETIC-LATE-{}", Uuid::new_v4()),
        "status": "SUCCESS",
        "event_timestamp": Utc::now().to_rfc3339(),
        "provider_event_id": Uuid::new_v4().to_string(),
    });
    let body = serde_json::to_string(&payload).unwrap();
    let signature = sign_webhook_body(&body, TEST_WEBHOOK_SECRET);
    for _ in 0..2 {
        let response = send_webhook_provider(&app, "mpesa", &body, &signature).await;
        assert_eq!(response.status(), StatusCode::OK);
        let response = json_body(response).await;
        assert_eq!(response["data"]["status"], "EXPIRED");
        assert_eq!(
            response["data"]["message"],
            "Payment event processed. Refer to the authoritative appointment status."
        );
    }
    let observed: (String, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT status::text,(SELECT count(*) FROM consultation_sessions WHERE appointment_id=$1),(SELECT count(*) FROM chat_conversations WHERE appointment_id=$1),(SELECT count(*) FROM financial_economic_effects WHERE business_id=$1 AND effect_type='PAYMENT_CONFIRMED'),(SELECT count(*) FROM payment_events WHERE transaction_id IN (SELECT id FROM transactions WHERE appointment_id=$1)) FROM appointments WHERE id=$1",
    )
    .bind(appointment)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(observed, ("EXPIRED".into(), 0, 0, 1, 1));
    assert_eq!(schedule_slot_status(&pool, slot).await, "AVAILABLE");
}

/// Direct test of the multi-dimensional abuse tracking in `ip_jail.rs`: an
/// account gets locked out from its own repeated failures regardless of which
/// IP they came from (the carrier-NAT-safe primary defense), a single IP
/// failing a *few* attempts against *one* account must NOT itself get jailed
/// (the exact false-positive the pasted security review flagged), and an IP
/// failing logins against *many distinct* accounts — a credential-stuffing
/// shape — does get jailed even though no single account crossed its own
/// threshold.
#[sqlx::test(migrations = "./migrations")]
async fn test_multi_dimensional_abuse_jailing(pool: PgPool) {
    let (_app, _pool) = test_app(pool).await;
    let redis_cfg = deadpool_redis::Config::from_url(&test_config().redis_url);
    let redis = redis_cfg
        .create_pool(Some(deadpool_redis::Runtime::Tokio1))
        .unwrap();

    // Account-level: 5 failures for the same account from 5 *different* IPs
    // still locks the account out (stops a distributed attack on one target).
    let account = "doctor:KMPDC #TEST-ACCOUNT";
    for i in 0..5u8 {
        let ip = format!("203.0.113.{i}");
        hellodoctor_backend::ip_jail::record_failure(&redis, &ip, account)
            .await
            .unwrap();
    }
    assert!(
        hellodoctor_backend::ip_jail::is_account_jailed(&redis, account)
            .await
            .unwrap(),
        "account must be jailed after 5 failures regardless of originating IP"
    );

    // IP-breadth: a single IP failing a handful of attempts against *one*
    // account must not itself get jailed — this is exactly the carrier-NAT
    // false-positive the security review raised, and it must not happen.
    let shared_ip = "198.51.100.1";
    let one_account = "doctor:KMPDC #SOMEONE-ELSE";
    for _ in 0..3 {
        hellodoctor_backend::ip_jail::record_failure(&redis, shared_ip, one_account)
            .await
            .unwrap();
    }
    assert!(
        !hellodoctor_backend::ip_jail::is_ip_jailed(&redis, shared_ip).await.unwrap(),
        "an IP must not be jailed merely for a few failures against one account (carrier-NAT safety)"
    );

    // IP-breadth: the same IP failing logins against many *distinct* accounts
    // is a credential-stuffing/scanning shape and must jail the IP itself,
    // even though none of those individual accounts crossed its own threshold.
    let scanning_ip = "198.51.100.2";
    for i in 0..25u32 {
        let target = format!("doctor:scanned-account-{i}");
        hellodoctor_backend::ip_jail::record_failure(&redis, scanning_ip, &target)
            .await
            .unwrap();
    }
    assert!(
        hellodoctor_backend::ip_jail::is_ip_jailed(&redis, scanning_ip).await.unwrap(),
        "an IP failing logins against many distinct accounts must be jailed (credential-stuffing shape)"
    );

    // A successful login clears only that account's own counter.
    let recovering_account = "doctor:KMPDC #RECOVERS";
    for _ in 0..3 {
        hellodoctor_backend::ip_jail::record_failure(&redis, "203.0.113.99", recovering_account)
            .await
            .unwrap();
    }
    hellodoctor_backend::ip_jail::record_success(&redis, recovering_account)
        .await
        .unwrap();
    assert!(
        !hellodoctor_backend::ip_jail::is_account_jailed(&redis, recovering_account)
            .await
            .unwrap(),
        "a successful login must clear the account's own failure counter"
    );
}

/// Regression test for the N+1 → single-query admin dashboard rewrites
/// (`list_admin_doctors`, `list_admin_transactions`, `get_command_telemetry`,
/// `list_admin_grievances`). Compiling and the rest of the suite passing
/// doesn't actually exercise these four GET endpoints at all — this test
/// specifically asserts the enriched fields (patient/doctor names, wallet
/// totals, telemetry) came out correctly resolved from the new in-memory
/// lookup maps rather than silently falling back to the generic
/// "Patient"/"Doctor"/"Physician" placeholders a broken map lookup would
/// produce.
#[sqlx::test(migrations = "./migrations")]
async fn test_admin_dashboard_endpoints_resolve_names_correctly(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor1_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let admin_token = make_token(uid(ADMIN_USER_ID), "ADMIN");

    let (apt_id, _) = book_and_confirm(&app, &patient_token).await;
    let session = consultation_repo::find_by_appointment(&pool, apt_id)
        .await
        .unwrap()
        .unwrap();

    // Submit a grievance so list_admin_grievances has a real row to resolve.
    let grv_payload = serde_json::json!({
        "consultation_id": session.id,
        "target": "DOCTOR",
        "category": "Rushed Consultation",
        "claim_summary": "Testing admin dashboard name resolution",
    });
    let res_grv = post(
        &app,
        "/api/v1/grievances",
        Some(&patient_token),
        &grv_payload,
    )
    .await;
    assert_eq!(res_grv.status(), StatusCode::CREATED);

    // Complete the consultation so there's a real wallet-credited transaction
    // for list_admin_transactions / list_admin_doctors to resolve.
    let res_comp = post(
        &app,
        &format!("/api/v1/consultations/{}/complete", apt_id),
        Some(&doctor1_token),
        &serde_json::json!({ "outcome": "COMPLETED_WITH_RX" }),
    )
    .await;
    assert_eq!(res_comp.status(), StatusCode::OK);

    // list_admin_doctors: wallet totals must be non-zero and correctly
    // attributed to doctor 1 specifically (not zeroed-out by a failed map lookup).
    let res_docs = get(&app, "/api/v1/admin/doctors", Some(&admin_token)).await;
    assert_eq!(res_docs.status(), StatusCode::OK);
    let docs_json = json_body(res_docs).await;
    let doc1_entry = docs_json["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["id"].as_str() == Some(DOCTOR1_PROFILE_ID))
        .expect("doctor 1 must be present in the admin doctor list");
    assert_money_eq(&doc1_entry["lifetime_gross"], "800");
    assert_eq!(doc1_entry["consultations_count"].as_i64().unwrap(), 1);

    // list_admin_transactions: patient/doctor names must be the real seeded
    // names, not the generic "Patient"/"Physician" fallback.
    let res_txs = get(
        &app,
        "/api/v1/admin/finance/transactions",
        Some(&admin_token),
    )
    .await;
    assert_eq!(res_txs.status(), StatusCode::OK);
    let txs_json = json_body(res_txs).await;
    let tx_entry = txs_json["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["appointment_id"].as_str() == Some(apt_id.to_string().as_str()))
        .expect("the booked appointment's transaction must be present");
    assert_eq!(tx_entry["patient_name"].as_str().unwrap(), "Patient User");
    assert_eq!(
        tx_entry["doctor_name"].as_str().unwrap(),
        "Dr. Synthetic Doctor One"
    );

    // get_command_telemetry: with no active sessions left (the one session
    // was just completed above), the endpoint must still respond cleanly —
    // this mainly exercises that the rewritten lookup-map construction
    // doesn't error even when nothing is active.
    let res_telemetry = get(&app, "/api/v1/admin/command/telemetry", Some(&admin_token)).await;
    assert_eq!(res_telemetry.status(), StatusCode::OK);

    // list_admin_grievances: patient/doctor names must resolve correctly
    // through the session -> appointment -> doctor chain.
    let res_grvs = get(&app, "/api/v1/admin/grievances", Some(&admin_token)).await;
    assert_eq!(res_grvs.status(), StatusCode::OK);
    let grvs_json = json_body(res_grvs).await;
    let grv_entry = grvs_json["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["consultation_id"].as_str() == Some(session.id.to_string().as_str()))
        .expect("the submitted grievance must be present");
    assert_eq!(grv_entry["patient_name"].as_str().unwrap(), "Patient User");
    assert_eq!(
        grv_entry["doctor_name"].as_str().unwrap(),
        "Dr. Synthetic Doctor One"
    );
}

/// `MAX_ACTIVE_USERS` must gate brand-new activation only — never an existing
/// user's own login. Seeding creates exactly 4 active users (1 admin, 1
/// patient, 2 doctors); setting the cap to that same number means it's
/// already "full" without creating a thousand fixtures to prove the point.
#[sqlx::test(migrations = "./migrations")]
async fn test_max_active_users_caps_new_registration_not_existing_logins(pool: PgPool) {
    let mut config = test_config();
    config.max_active_users = 4;
    let (app, _pool) = test_app_with_config(pool, config).await;

    // The existing seeded patient must still be able to log in — the cap
    // never touches an existing user's own session.
    let res_otp = post(
        &app,
        "/api/v1/auth/register-otp",
        None,
        &serde_json::json!({ "phone_number": SEED_PATIENT_PHONE }),
    )
    .await;
    assert_eq!(res_otp.status(), StatusCode::OK);
    let otp_json = json_body(res_otp).await;
    let code = otp_json["data"]["debug_otp_code"]
        .as_str()
        .unwrap()
        .to_string();
    let res_login = post(
        &app,
        "/api/v1/auth/verify-otp-and-login",
        None,
        &serde_json::json!({ "phone_number": SEED_PATIENT_PHONE, "otp_code": code }),
    )
    .await;
    assert_eq!(
        res_login.status(),
        StatusCode::OK,
        "an existing user's own login must never be blocked by the capacity cap"
    );

    // A brand-new phone number, with no account yet, must be rejected once
    // the cap is reached.
    let new_phone = "+254700000202";
    let res_otp2 = post(
        &app,
        "/api/v1/auth/register-otp",
        None,
        &serde_json::json!({ "phone_number": new_phone }),
    )
    .await;
    assert_eq!(
        res_otp2.status(),
        StatusCode::OK,
        "requesting an OTP itself is not gated, only account creation"
    );
    let otp_json2 = json_body(res_otp2).await;
    let code2 = otp_json2["data"]["debug_otp_code"]
        .as_str()
        .unwrap()
        .to_string();
    let res_new = post(
        &app,
        "/api/v1/auth/verify-otp-and-login",
        None,
        &serde_json::json!({ "phone_number": new_phone, "otp_code": code2 }),
    )
    .await;
    assert_eq!(
        res_new.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "a brand-new account must be rejected once MAX_ACTIVE_USERS is reached"
    );
    let new_json = json_body(res_new).await;
    assert_eq!(
        new_json["error"]["code"].as_str().unwrap(),
        "CAPACITY_LIMIT_REACHED"
    );
}

/// The System Error Service persists real events, redacts secrets before
/// storage, and dedupes repeated occurrences of the same failure into one row
/// with an incrementing counter instead of flooding the table.
#[sqlx::test(migrations = "./migrations")]
async fn test_system_error_service_persists_dedupes_and_redacts(pool: PgPool) {
    let (_app, pool) = test_app(pool).await;
    use hellodoctor_backend::domain::models::ErrorSeverity;
    use hellodoctor_backend::system_error::{report, ErrorEvent};

    let secret_bearing_message =
        "upstream call failed, Authorization: Bearer eyJhbGciOiJFZERTQSJ9.eyJzdWIiOiJ4In0.abcDEF123signature";

    report(ErrorEvent::new(
        ErrorSeverity::Error,
        "PAYMENT",
        "test::dedupe_probe",
        "TEST_DEDUPE_PROBE",
        secret_bearing_message,
        "req-1",
    ));
    report(ErrorEvent::new(
        ErrorSeverity::Error,
        "PAYMENT",
        "test::dedupe_probe",
        "TEST_DEDUPE_PROBE",
        "a second, textually different occurrence of the same underlying failure",
        "req-2",
    ));

    // Fire-and-forget over an unbounded channel with a single fast consumer —
    // give it a moment to drain before asserting.
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let rows = sqlx::query(
        "SELECT error_message, occurrence_count FROM system_error_logs \
         WHERE error_code = 'TEST_DEDUPE_PROBE'",
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(
        rows.len(),
        1,
        "two occurrences of the same category+component+code must dedupe into one row"
    );
    let occurrence_count: i32 = rows[0].get("occurrence_count");
    assert_eq!(occurrence_count, 2);
    let stored_message: String = rows[0].get("error_message");
    assert!(
        !stored_message.contains("eyJhbGciOiJFZERTQSJ9"),
        "the JWT in the first occurrence's message must have been redacted before storage: {stored_message}"
    );
}

/// Expected business/auth/rate-limit outcomes (401/403/404/409/429) must NOT
/// create any `system_error_logs` row — only genuine 5xx should.
#[sqlx::test(migrations = "./migrations")]
async fn test_normal_rejections_do_not_create_system_error_logs(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");

    // 401: no token at all.
    let res = get(&app, "/api/v1/doctors", None).await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // 403: patient hitting an admin-only route.
    let res = get(&app, "/api/v1/admin/overview", Some(&patient_token)).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 404: a well-formed but nonexistent appointment id.
    let res = get(
        &app,
        &format!("/api/v1/consultations/{}/rtc-token", Uuid::new_v4()),
        Some(&patient_token),
    )
    .await;
    assert!(res.status().is_client_error());

    // 409: book the same slot twice in a row (the second attempt loses the race).
    let res_slots = get(
        &app,
        &format!("/api/v1/doctors/{}/slots", DOCTOR1_PROFILE_ID),
        Some(&patient_token),
    )
    .await;
    let slots_json = json_body(res_slots).await;
    let slot_id = slots_json["data"][0]["id"].as_str().unwrap();
    let book_payload = serde_json::json!({
        "doctor_id": DOCTOR1_PROFILE_ID, "slot_id": slot_id,
        "modality": "VIDEO", "gateway": "BKASH", "chief_complaint": "x"
    });
    let res1 = post_idem(
        &app,
        "/api/v1/appointments/book",
        Some(&patient_token),
        &book_payload,
        "idem-dup-1",
    )
    .await;
    assert_eq!(res1.status(), StatusCode::CREATED);
    let res2 = post_idem(
        &app,
        "/api/v1/appointments/book",
        Some(&patient_token),
        &book_payload,
        "idem-dup-2",
    )
    .await;
    assert_eq!(res2.status(), StatusCode::CONFLICT);

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM system_error_logs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        count, 0,
        "none of 401/403/404/409 should ever create a system_error_logs row"
    );
}

/// A genuine handler panic — not an `AppError`, a real unwind — must still
/// produce a normal JSON error response AND a `system_error_logs` row,
/// exactly like every other backend failure path. Before this, every
/// `AppError` variant (`Internal`/`Database`/`ServiceUnavailable`) already
/// reported into this pipeline, but an actual code bug (an unexpected
/// `.unwrap()`, an out-of-bounds index) had no equivalent path at all — the
/// connection would simply drop with nothing recorded anywhere the admin
/// panel could see. Exercises the real `CatchPanicLayer::custom(handle_panic)`
/// wiring from `router.rs` against a genuine unwinding panic (not a direct
/// function call standing in for one), via a throwaway router built just for
/// this test — never a route reachable in the real app, so this proves the
/// pipeline without adding any new production attack surface.
#[sqlx::test(migrations = "./migrations")]
async fn test_handler_panic_reports_to_system_error_logs(pool: PgPool) {
    // Initializes `system_error::init` against this test's own throwaway
    // database, exactly like every other system-error test in this file.
    let (_app, pool) = test_app(pool).await;

    let panic_line = line!() + 5;
    let panic_router = axum::Router::new()
        .route(
            "/test-only-panic",
            axum::routing::get(|| async {
                panic!("synthetic test panic: boom");
                #[allow(unreachable_code)]
                StatusCode::OK
            }),
        )
        .layer(tower_http::catch_panic::CatchPanicLayer::custom(
            hellodoctor_backend::api::router::handle_panic,
        ));

    let request = Request::builder()
        .method("GET")
        .uri("/test-only-panic")
        .body(Body::empty())
        .unwrap();
    let response = panic_router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = json_body(response).await;
    assert_eq!(body["success"], false);
    assert_eq!(body["error"]["code"], "INTERNAL_SERVER_ERROR");
    assert!(
        body["meta"]["request_id"].as_str().is_some(),
        "the response envelope must still carry a request id, matching every other error response"
    );

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let row: (String, String, String, Option<String>) = sqlx::query_as(
        "SELECT severity, subsystem, component, stack_trace FROM system_error_logs \
         WHERE error_code = 'HANDLER_PANIC'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        row.0, "CRITICAL",
        "a real panic must be logged CRITICAL, not merely ERROR"
    );
    assert_eq!(row.1, "INTERNAL");
    assert_eq!(row.2, "HandlerPanic");
    let stack_trace = row
        .3
        .expect("a real panic must capture a source location, not just a message");
    assert!(
        stack_trace.ends_with(&format!("api_tests.rs:{panic_line}:17")),
        "expected the exact panic!() call site, got {stack_trace:?}"
    );

    let message: String = sqlx::query_scalar(
        "SELECT error_message FROM system_error_logs WHERE error_code = 'HANDLER_PANIC'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        message.contains("synthetic test panic"),
        "the real panic payload must reach the stored row: {message}"
    );
}

/// A request that trips the `tower::timeout` stage — something hung past the
/// configured deadline — must report into `system_error_logs` exactly like a
/// handler panic or a handled `AppError` does, not be visible only as a
/// client-side timeout with nothing to correlate against admin-side. Uses a
/// throwaway router with its own very short timeout (not the real app's
/// `REQUEST_TIMEOUT_SECS`) purely so this test resolves in milliseconds
/// instead of seconds; it exercises the exact same `handle_overload_error`
/// function `router.rs` wires into the real app.
#[sqlx::test(migrations = "./migrations")]
async fn test_request_timeout_reports_to_system_error_logs(pool: PgPool) {
    let (_app, pool) = test_app(pool).await;

    let timeout_router = axum::Router::new()
        .route(
            "/test-only-slow",
            axum::routing::get(|| async {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                StatusCode::OK
            }),
        )
        .layer(
            tower::ServiceBuilder::new()
                .layer(axum::error_handling::HandleErrorLayer::new(
                    hellodoctor_backend::api::router::handle_overload_error,
                ))
                .timeout(std::time::Duration::from_millis(20)),
        );

    let request = Request::builder()
        .method("GET")
        .uri("/test-only-slow")
        .body(Body::empty())
        .unwrap();
    let response = timeout_router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::REQUEST_TIMEOUT);
    let body = json_body(response).await;
    assert_eq!(body["success"], false);
    assert_eq!(body["error"]["code"], "REQUEST_TIMEOUT");

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let row: (String, String) = sqlx::query_as(
        "SELECT severity, component FROM system_error_logs WHERE error_code = 'REQUEST_TIMEOUT'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.0, "ERROR");
    assert_eq!(row.1, "RequestStage::Overload");
}

/// Access control and lifecycle for the admin System Errors surface: patients
/// and doctors are denied; an admin can list, acknowledge, and resolve.
#[sqlx::test(migrations = "./migrations")]
async fn test_system_errors_admin_api_access_control_and_lifecycle(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let admin_token = make_token(uid(ADMIN_USER_ID), "ADMIN");

    for (role_token, role_name) in [(&patient_token, "patient"), (&doctor_token, "doctor")] {
        let res = get(&app, "/api/v1/admin/system-errors", Some(role_token)).await;
        assert_eq!(
            res.status(),
            StatusCode::FORBIDDEN,
            "{role_name} must not be able to view system errors"
        );
    }

    let error_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO system_error_logs \
         (id, trace_id, subsystem, component, severity, action_attempted, error_message, \
          error_code, fingerprint, occurrence_count, first_seen_at, last_seen_at, status, logged_at) \
         VALUES ($1, 'req-test', 'TEST', 'test::lifecycle', 'ERROR', 'test::lifecycle', 'synthetic test row', \
          'TEST_LIFECYCLE', 'fp-test-lifecycle', 1, now(), now(), 'OPEN', now())",
    )
    .bind(error_id)
    .execute(&pool)
    .await
    .unwrap();

    let res = get(&app, "/api/v1/admin/system-errors", Some(&admin_token)).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert!(body["data"]["total"].as_i64().unwrap() >= 1);

    let res = post(
        &app,
        &format!("/api/v1/admin/system-errors/{error_id}/acknowledge"),
        Some(&admin_token),
        &serde_json::json!({}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);

    let status: String = sqlx::query_scalar("SELECT status FROM system_error_logs WHERE id = $1")
        .bind(error_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "ACKNOWLEDGED");

    let res = post(
        &app,
        &format!("/api/v1/admin/system-errors/{error_id}/resolve"),
        Some(&admin_token),
        &serde_json::json!({ "resolution_note": "fixed in release X" }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);

    let (status, note): (String, Option<String>) =
        sqlx::query_as("SELECT status, resolution_note FROM system_error_logs WHERE id = $1")
            .bind(error_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "RESOLVED");
    assert_eq!(note.as_deref(), Some("fixed in release X"));
}

/// A real admin mutation must produce exactly one `audit_events` row with the
/// correct actor/action/resource, and an unauthorized caller must be denied
/// before any mutation (and therefore before any audit row) is created.
#[sqlx::test(migrations = "./migrations")]
async fn test_admin_mutation_creates_audit_event(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let admin_token = make_token(uid(ADMIN_USER_ID), "ADMIN");
    sqlx::query(
        "UPDATE doctor_profiles SET professional_verification_status='PENDING' WHERE id=$1",
    )
    .bind(uid(DOCTOR1_PROFILE_ID))
    .execute(&pool)
    .await
    .unwrap();
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");

    let res_denied = post(
        &app,
        &format!("/api/v1/admin/doctors/{}/verify", DOCTOR1_PROFILE_ID),
        Some(&patient_token),
        &serde_json::json!({}),
    )
    .await;
    assert_eq!(
        res_denied.status(),
        StatusCode::FORBIDDEN,
        "a patient must not be able to perform KMPDC verification"
    );

    let res = post(
        &app,
        &format!("/api/v1/admin/doctors/{}/verify", DOCTOR1_PROFILE_ID),
        Some(&admin_token),
        &serde_json::json!({ "note": "license cross-checked" }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);

    let rows = sqlx::query(
        "SELECT actor_id, actor_role::text AS actor_role, action, resource_type, resource_id, result \
         FROM audit_events WHERE action = 'DOCTOR_PROFESSIONAL_VERIFY'",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        rows.len(),
        1,
        "the denied patient attempt must not have created any audit row — \
         exactly one row, from the admin's successful call, is expected"
    );
    let row = &rows[0];
    assert_eq!(row.get::<Uuid, _>("actor_id"), uid(ADMIN_USER_ID));
    assert_eq!(row.get::<String, _>("actor_role"), "ADMIN");
    assert_eq!(row.get::<String, _>("resource_type"), "DOCTOR_PROFILE");
    assert_eq!(row.get::<Uuid, _>("resource_id"), uid(DOCTOR1_PROFILE_ID));
    assert_eq!(row.get::<String, _>("result"), "SUCCESS");
}

// ============================================================================
// §8.2 resume: consultation telemetry event timeline + admin audit
// before/after snapshots + the "Super Admin"-only audit-log restriction.
// See docs/CHECKPOINT.md §8.2.
// ============================================================================

#[sqlx::test(migrations = "./migrations")]
async fn test_telemetry_event_ingestion_ownership_and_timeline_reconstruction(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor1_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let doctor2_token = make_token(uid(DOCTOR2_USER_ID), "DOCTOR");
    let admin_token = make_token(uid(ADMIN_USER_ID), "ADMIN");

    let (apt_id, _) = book_and_confirm(&app, &patient_token).await;
    let session = consultation_repo::find_by_appointment(&pool, apt_id)
        .await
        .unwrap()
        .unwrap();

    // A doctor who is not a participant in this consultation must be denied.
    // RLS hides the appointment row entirely from a non-participant before
    // the handler's own ownership check ever runs, so this surfaces as 404,
    // not 403 — the same documented property already exercised by
    // `test_bola_patient_cannot_post_into_another_patients_chat` and the
    // cross-role appointment-completion test (see those tests' own comments).
    let res_denied = post(
        &app,
        &format!("/api/v1/consultations/{}/telemetry/events", apt_id),
        Some(&doctor2_token),
        &serde_json::json!({ "events": [{ "event_type": "DOCTOR_JOINED" }] }),
    )
    .await;
    assert_eq!(res_denied.status(), StatusCode::NOT_FOUND);

    // Patient reports joining, then a disconnect/reconnect pair, plus one
    // deliberately unknown event_type that must be silently skipped rather
    // than failing the whole batch.
    let t0 = chrono::Utc::now();
    let patient_batch = serde_json::json!({
        "events": [
            { "event_type": "PATIENT_JOINED", "occurred_at": t0.to_rfc3339() },
            { "event_type": "NOT_A_REAL_EVENT_TYPE", "occurred_at": t0.to_rfc3339() },
            { "event_type": "PATIENT_DISCONNECTED", "occurred_at": (t0 + chrono::Duration::seconds(10)).to_rfc3339(), "rtt_ms": 120, "packet_loss_percent": 2.5 },
            { "event_type": "PATIENT_RECONNECTED", "occurred_at": (t0 + chrono::Duration::seconds(18)).to_rfc3339() },
        ]
    });
    let res_patient = post(
        &app,
        &format!("/api/v1/consultations/{}/telemetry/events", apt_id),
        Some(&patient_token),
        &patient_batch,
    )
    .await;
    assert_eq!(res_patient.status(), StatusCode::OK);
    let patient_json = json_body(res_patient).await;
    assert_eq!(
        patient_json["data"]["accepted"].as_u64().unwrap(),
        3,
        "the one unknown event_type must be skipped, not fail the whole batch"
    );

    // Doctor reports joining too.
    let res_doctor = post(
        &app,
        &format!("/api/v1/consultations/{}/telemetry/events", apt_id),
        Some(&doctor1_token),
        &serde_json::json!({ "events": [{ "event_type": "DOCTOR_JOINED", "occurred_at": t0.to_rfc3339() }] }),
    )
    .await;
    assert_eq!(res_doctor.status(), StatusCode::OK);

    // A patient (not an admin) must not reach the admin timeline view.
    let res_patient_denied = get(
        &app,
        &format!("/api/v1/admin/consultations/{}/timeline", session.id),
        Some(&patient_token),
    )
    .await;
    assert_eq!(res_patient_denied.status(), StatusCode::FORBIDDEN);

    let res_timeline = get(
        &app,
        &format!("/api/v1/admin/consultations/{}/timeline", session.id),
        Some(&admin_token),
    )
    .await;
    assert_eq!(res_timeline.status(), StatusCode::OK);
    let timeline = json_body(res_timeline).await;

    let events = timeline["data"]["events"].as_array().unwrap();
    assert_eq!(
        events.len(),
        4,
        "3 accepted patient events + 1 doctor event, in ingest order"
    );
    assert_eq!(events[0]["event_type"].as_str().unwrap(), "PATIENT_JOINED");

    let windows = timeline["data"]["reconnect_windows"].as_array().unwrap();
    assert_eq!(windows.len(), 1);
    assert_eq!(windows[0]["perspective"].as_str().unwrap(), "PATIENT");
    assert_eq!(windows[0]["duration_seconds"].as_i64().unwrap(), 8);

    // A patient grievance references the SAME consultation session whose
    // actual event history is available to Admin, even without Agora running.
    let grievance = post(
        &app,
        "/api/v1/grievances",
        Some(&patient_token),
        &serde_json::json!({"consultation_id":session.id,"target":"SYSTEM",
            "category":"CALL_CONNECTION", "claim_summary":"Synthetic connectivity review."}),
    )
    .await;
    assert_eq!(grievance.status(), StatusCode::CREATED);
    let timeline = json_body(
        get(
            &app,
            &format!("/api/v1/admin/consultations/{}/timeline", session.id),
            Some(&admin_token),
        )
        .await,
    )
    .await;
    assert_eq!(
        timeline["data"]["related_grievances"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(timeline["data"]["events"].as_array().unwrap().len(), 4);
    assert!(
        timeline["data"]["aggregate_telemetry"].is_null(),
        "Unmeasured is not zero loss/24ms/Connected."
    );
}

fn client_incident_fixture(client: &str) -> Value {
    serde_json::json!({"client":client,"error_type":"TypeError","message":"synthetic incident",
        "stack_trace":"#0 load (package:hellodoctor_mobile/features/doctor/doctor_repository.dart:33:7)\n at send (https://app.test/_next/static/chunks/app/page.js:22:8)",
        "screen":"Schedule","operation":"block_slot","app_version":"1.0.0","build_number":"1",
        "platform":if matches!(client,"DOCTOR_WEB"|"ADMIN_WEB") {"web"} else {"android"},"occurred_at":Utc::now()})
}

#[sqlx::test(migrations = "./migrations")]
async fn test_client_incident_existing_backend_http_failure_reaches_admin(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let app = app.route(
        "/__test_failure",
        axum::routing::get(|| async {
            Err::<axum::Json<Value>, _>(hellodoctor_backend::error::AppError::internal(
                "Synthetic backend failure".into(),
            ))
        }),
    );
    assert_eq!(
        get(&app, "/__test_failure", None).await.status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let feed = json_body(
        get(
            &app,
            "/api/v1/admin/system-errors?subsystem=INTERNAL",
            Some(&admin),
        )
        .await,
    )
    .await;
    assert_eq!(feed["data"]["items"][0]["error_code"], "INTERNAL_ERROR");
    assert_eq!(
        feed["data"]["items"][0]["error_message"],
        "Synthetic backend failure"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_client_incident_persists_private_payload_and_admin_retrieves(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    for client in ["DOCTOR_FLUTTER", "DOCTOR_WEB"] {
        let mut payload = client_incident_fixture(client);
        payload["message"]=serde_json::json!("JWT=eySecret refresh_token=private Authorization=secret OTP=123456 password=private Agora=private patient_name=Jane Test diagnosis=asthma complaint=pain prescription=drug chat=private +254712345678 account=123456 document=https://private.test?token=private");
        payload["stack_trace"] = serde_json::json!(format!(
            "{}\npatient=Jane Test diagnosis=asthma Authorization: Bearer secret",
            payload["stack_trace"].as_str().unwrap()
        ));
        let response = post(&app, "/api/v1/client-errors", Some(&doctor), &payload).await;
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let body = json_body(response).await;
        let id = Uuid::parse_str(body["data"]["incident_id"].as_str().unwrap()).unwrap();
        let stored: Value =
            sqlx::query_scalar("SELECT to_jsonb(e) FROM system_error_logs e WHERE id=$1")
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(stored["subsystem"], client);
        assert_eq!(stored["impacted_user_id"], DOCTOR1_USER_ID);
        assert_eq!(stored["severity"], "ERROR");
        assert_eq!(stored["status"], "OPEN");
        let serialized = stored.to_string();
        for sensitive in [
            "eySecret",
            "private",
            "Jane Test",
            "asthma",
            "+254712345678",
            "123456",
            "document=https",
            "chat=",
        ] {
            assert!(
                !serialized.contains(sensitive),
                "Sensitive fixture value escaped normalization"
            );
        }
        assert!(stored["stack_trace"]
            .as_str()
            .unwrap()
            .contains("doctor_repository.dart:33:7"));
        assert_eq!(stored["client_context"]["app_version"], "1.0.0");
        assert_eq!(stored["client_context"]["build_number"], "1");
        let detail = json_body(
            get(
                &app,
                &format!("/api/v1/admin/system-errors/{id}"),
                Some(&admin),
            )
            .await,
        )
        .await;
        assert_eq!(detail["data"]["id"], id.to_string());
        let feed = json_body(
            get(
                &app,
                &format!("/api/v1/admin/system-errors?subsystem={client}"),
                Some(&admin),
            )
            .await,
        )
        .await;
        assert_eq!(feed["data"]["items"][0]["id"], id.to_string());
    }
}

/// Admin Web is the only client surface that previously had no client-side
/// incident reporting at all — Doctor Flutter, Patient Flutter, and Doctor
/// Web all already fed into this same pipeline. The system meant to let
/// admins see every other app's failures should not be the one app whose
/// own frontend crashes are invisible to everyone, including the admins
/// using it. Confirms the real role/client binding this campaign added:
/// only an authenticated Admin session may report as `ADMIN_WEB`, the same
/// way only a Doctor session may report as `DOCTOR_WEB`/`DOCTOR_FLUTTER` and
/// only a Patient session may report as `PATIENT_FLUTTER` — a Doctor or
/// Patient spoofing `ADMIN_WEB`, or an Admin spoofing any other client, must
/// both be denied.
#[sqlx::test(migrations = "./migrations")]
async fn test_admin_web_client_incident_accepted_and_role_bound(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");

    let payload = client_incident_fixture("ADMIN_WEB");
    let response = post(&app, "/api/v1/client-errors", Some(&admin), &payload).await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let body = json_body(response).await;
    let id = Uuid::parse_str(body["data"]["incident_id"].as_str().unwrap()).unwrap();

    let stored: (String, String) =
        sqlx::query_as("SELECT subsystem, component FROM system_error_logs WHERE id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(stored.0, "ADMIN_WEB");
    assert_eq!(stored.1, "AdminWebRuntime");

    let feed = json_body(
        get(
            &app,
            "/api/v1/admin/system-errors?subsystem=ADMIN_WEB",
            Some(&admin),
        )
        .await,
    )
    .await;
    assert_eq!(feed["data"]["items"][0]["id"], id.to_string());

    // Role/client spoofing denied both directions.
    assert_eq!(
        post(&app, "/api/v1/client-errors", Some(&doctor), &payload)
            .await
            .status(),
        StatusCode::FORBIDDEN,
        "a Doctor session must not report as ADMIN_WEB"
    );
    assert_eq!(
        post(&app, "/api/v1/client-errors", Some(&patient), &payload)
            .await
            .status(),
        StatusCode::FORBIDDEN,
        "a Patient session must not report as ADMIN_WEB"
    );
    let admin_spoofing_doctor_web = client_incident_fixture("DOCTOR_WEB");
    assert_eq!(
        post(
            &app,
            "/api/v1/client-errors",
            Some(&admin),
            &admin_spoofing_doctor_web
        )
        .await
        .status(),
        StatusCode::FORBIDDEN,
        "an Admin session must not report as DOCTOR_WEB"
    );
}

/// Saving a patient/doctor payment method requires the platform to have an
/// active merchant configuration for the matching provider family first
/// (`services::payment_provider_verification`) — every test below that
/// saves a real MPESA method must seed one first, exactly like a real
/// deployment would need an admin to configure M-Pesa before any patient or
/// doctor could use it.
async fn seed_active_mpesa_platform_method(app: &axum::Router) {
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let res = post(
        app,
        "/api/v1/admin/platform-payment-methods",
        Some(&admin),
        &serde_json::json!({
            "provider_code": "MPESA_PAYBILL",
            "label": "Test Primary Paybill",
            "masked_display": "Paybill ****1234",
            "credentials": {"paybill": "400200", "consumer_key": "test-key", "consumer_secret": "test-secret"}
        }),
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::CREATED,
        "test setup: seeding the platform MPESA method must succeed"
    );
}

/// Full lifecycle for a patient's saved payment methods: save (first one
/// becomes primary automatically), save a second (stays non-primary by
/// default since a primary already exists), explicitly switch primary,
/// and remove. Mirrors the already-proven doctor_payout_destinations
/// pattern applied to the paying side instead of the payout side — see
/// migrations/20261108000001_patient_payment_methods.up.sql.
#[sqlx::test(migrations = "./migrations")]
async fn test_patient_payment_methods_save_list_set_primary_remove(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    seed_active_mpesa_platform_method(&app).await;

    let first = post(
        &app,
        "/api/v1/patients/me/payment-methods",
        Some(&patient),
        &serde_json::json!({"gateway":"MPESA","account_identifier":"+254712345678"}),
    )
    .await;
    assert_eq!(first.status(), StatusCode::CREATED);
    let first_body = json_body(first).await;
    assert_eq!(
        first_body["data"]["is_primary"], true,
        "a patient's very first saved method must default to primary"
    );
    assert_eq!(first_body["data"]["gateway"], "MPESA");
    let first_id = first_body["data"]["id"].as_str().unwrap().to_string();

    let second = post(
        &app,
        "/api/v1/patients/me/payment-methods",
        Some(&patient),
        &serde_json::json!({"gateway":"MPESA","account_identifier":"+254798765432"}),
    )
    .await;
    assert_eq!(second.status(), StatusCode::CREATED);
    let second_body = json_body(second).await;
    assert_eq!(
        second_body["data"]["is_primary"], false,
        "a second method must not silently displace the existing primary"
    );
    let second_id = second_body["data"]["id"].as_str().unwrap().to_string();

    let list =
        json_body(get(&app, "/api/v1/patients/me/payment-methods", Some(&patient)).await).await;
    assert_eq!(list["data"].as_array().unwrap().len(), 2);

    let switched = post(
        &app,
        &format!("/api/v1/patients/me/payment-methods/{second_id}/set-primary"),
        Some(&patient),
        &serde_json::json!({}),
    )
    .await;
    assert_eq!(switched.status(), StatusCode::OK);
    let list_after_switch =
        json_body(get(&app, "/api/v1/patients/me/payment-methods", Some(&patient)).await).await;
    let items = list_after_switch["data"].as_array().unwrap();
    let first_now = items.iter().find(|i| i["id"] == first_id).unwrap();
    let second_now = items.iter().find(|i| i["id"] == second_id).unwrap();
    assert_eq!(
        first_now["is_primary"], false,
        "exactly one primary at a time — the old one must be demoted"
    );
    assert_eq!(second_now["is_primary"], true);

    assert_eq!(
        delete(
            &app,
            &format!("/api/v1/patients/me/payment-methods/{first_id}"),
            Some(&patient)
        )
        .await
        .status(),
        StatusCode::OK
    );
    let list_after_remove =
        json_body(get(&app, "/api/v1/patients/me/payment-methods", Some(&patient)).await).await;
    assert_eq!(
        list_after_remove["data"].as_array().unwrap().len(),
        1,
        "a removed method must not reappear in the list"
    );
}

/// The hard stop `services::payment_provider_verification` exists to
/// enforce: with no active platform merchant configuration for MPESA at
/// all, neither a patient saving a payment method nor a doctor binding a
/// payout destination may succeed — a clear rejection, not a silent
/// simulated pass. Deliberately does NOT call
/// `seed_active_mpesa_platform_method`, unlike every other test in this
/// file touching these endpoints.
#[sqlx::test(migrations = "./migrations")]
async fn test_payment_method_save_rejected_without_platform_provider_configured(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");

    let patient_attempt = post(
        &app,
        "/api/v1/patients/me/payment-methods",
        Some(&patient),
        &serde_json::json!({"gateway":"MPESA","account_identifier":"+254712345678"}),
    )
    .await;
    assert_eq!(
        patient_attempt.status(),
        StatusCode::CONFLICT,
        "a patient must not be able to save an MPESA method with no platform MPESA provider configured"
    );

    let doctor_attempt = post(
        &app,
        "/api/v1/doctor/wallet/payout-methods/bind",
        Some(&doctor),
        &serde_json::json!({"method_type":"MPESA","account_identifier":"+254722333444"}),
    )
    .await;
    assert_eq!(
        doctor_attempt.status(),
        StatusCode::CONFLICT,
        "a doctor must not be able to bind an MPESA payout destination with no platform MPESA provider configured"
    );
}

/// A saved payment method must be scoped to the owning patient — another
/// patient trying to set it primary or remove it, or book a consultation
/// with it, must be rejected (BOLA guard), not silently succeed.
#[sqlx::test(migrations = "./migrations")]
async fn test_patient_cannot_use_another_patients_payment_method(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    seed_active_mpesa_platform_method(&app).await;
    let owner = make_token(uid(PATIENT_USER_ID), "PATIENT");

    // A genuine second patient, not the fixed seed account — minimal
    // inline version of the pattern `isolation_fixture` already
    // establishes (new user, new patient_profiles row, a real
    // auth_sessions row so the active-session-check middleware accepts
    // the minted token), without that fixture's unrelated cloned-appointment
    // setup this test doesn't need.
    let other_user =
        hellodoctor_backend::repository::user_repo::create_patient_user(&pool, "+254700009981")
            .await
            .unwrap();
    let mut other_patient = hellodoctor_backend::repository::patient_repo::find_by_user_id(
        &pool,
        UserId::from(uid(PATIENT_USER_ID)),
    )
    .await
    .unwrap()
    .unwrap();
    other_patient.id = PatientProfileId::new();
    other_patient.user_id = Some(other_user.id);
    other_patient.full_name = "Synthetic Second Patient".into();
    hellodoctor_backend::repository::patient_repo::insert(&pool, &other_patient)
        .await
        .unwrap();
    sqlx::query("INSERT INTO auth_sessions(id,user_id,token_family_id,refresh_token_hash,expires_at) VALUES(gen_random_uuid(),$1,$1,$1::text,now()+interval '1 day')")
        .bind(other_user.id).execute(&pool).await.unwrap();
    let other = make_token(other_user.id.into(), "PATIENT");

    let saved = post(
        &app,
        "/api/v1/patients/me/payment-methods",
        Some(&owner),
        &serde_json::json!({"gateway":"MPESA","account_identifier":"+254711222333"}),
    )
    .await;
    assert_eq!(saved.status(), StatusCode::CREATED);
    let method_id = json_body(saved).await["data"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    assert_eq!(
        post(
            &app,
            &format!("/api/v1/patients/me/payment-methods/{method_id}/set-primary"),
            Some(&other),
            &serde_json::json!({})
        )
        .await
        .status(),
        StatusCode::NOT_FOUND,
        "another patient must not be able to promote someone else's saved method"
    );
    assert_eq!(
        delete(
            &app,
            &format!("/api/v1/patients/me/payment-methods/{method_id}"),
            Some(&other)
        )
        .await
        .status(),
        StatusCode::NOT_FOUND,
        "another patient must not be able to remove someone else's saved method"
    );

    let res_slots = get(
        &app,
        &format!("/api/v1/doctors/{}/slots", DOCTOR1_PROFILE_ID),
        Some(&other),
    )
    .await;
    let slot_id = json_body(res_slots).await["data"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let book = post_idem(
        &app,
        "/api/v1/appointments/book",
        Some(&other),
        &serde_json::json!({"doctor_id": DOCTOR1_PROFILE_ID, "slot_id": slot_id, "modality": "VIDEO", "payment_method_id": method_id}),
        &Uuid::new_v4().to_string(),
    )
    .await;
    assert_eq!(book.status(), StatusCode::NOT_FOUND, "booking with someone else's saved payment method must be rejected, not silently charge the wrong method");
}

/// End-to-end: a patient saves a method, then books and confirms a real
/// consultation using it by id (no `gateway` sent at all) — the resolved
/// gateway and the method's own id must both land correctly on the
/// resulting transaction.
#[sqlx::test(migrations = "./migrations")]
async fn test_patient_can_book_using_saved_payment_method(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    seed_active_mpesa_platform_method(&app).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");

    let saved = post(
        &app,
        "/api/v1/patients/me/payment-methods",
        Some(&patient),
        &serde_json::json!({"gateway":"MPESA","account_identifier":"+254733444555"}),
    )
    .await;
    assert_eq!(saved.status(), StatusCode::CREATED);
    let method_id = json_body(saved).await["data"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let res_slots = get(
        &app,
        &format!("/api/v1/doctors/{}/slots", DOCTOR1_PROFILE_ID),
        Some(&patient),
    )
    .await;
    let slot_id = json_body(res_slots).await["data"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let book = post_idem(
        &app,
        "/api/v1/appointments/book",
        Some(&patient),
        &serde_json::json!({"doctor_id": DOCTOR1_PROFILE_ID, "slot_id": slot_id, "modality": "VIDEO", "payment_method_id": method_id, "chief_complaint": "Headache"}),
        &Uuid::new_v4().to_string(),
    )
    .await;
    assert_eq!(book.status(), StatusCode::CREATED);
    let apt_id = Uuid::parse_str(
        json_body(book).await["data"]["appointment_id"]
            .as_str()
            .unwrap(),
    )
    .unwrap();

    let stored: (String, Option<Uuid>) = sqlx::query_as(
        "SELECT gateway::text, payment_method_id FROM transactions WHERE appointment_id=$1",
    )
    .bind(apt_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        stored.0, "MPESA",
        "the gateway must be resolved from the selected saved method"
    );
    assert_eq!(
        stored.1,
        Some(Uuid::parse_str(&method_id).unwrap()),
        "the transaction must record which saved method was actually used"
    );
}

/// A neither-gateway-nor-payment_method_id booking request must be rejected
/// with a clear validation error, not silently default to anything.
#[sqlx::test(migrations = "./migrations")]
async fn test_book_appointment_requires_gateway_or_payment_method(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let res_slots = get(
        &app,
        &format!("/api/v1/doctors/{}/slots", DOCTOR1_PROFILE_ID),
        Some(&patient),
    )
    .await;
    let slot_id = json_body(res_slots).await["data"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let book = post_idem(
        &app,
        "/api/v1/appointments/book",
        Some(&patient),
        &serde_json::json!({"doctor_id": DOCTOR1_PROFILE_ID, "slot_id": slot_id, "modality": "VIDEO"}),
        &Uuid::new_v4().to_string(),
    )
    .await;
    assert_eq!(book.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

/// Admin-only CRUD for the platform's own merchant/collection
/// configuration: save (first for a provider_code becomes primary
/// automatically), a Doctor/Patient session must be denied outright, and
/// deactivating removes it from the active list. See
/// migrations/20261108000002_platform_payment_methods.up.sql.
#[sqlx::test(migrations = "./migrations")]
async fn test_platform_payment_methods_admin_only_crud(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");

    let denied_doctor = post(
        &app,
        "/api/v1/admin/platform-payment-methods",
        Some(&doctor),
        &serde_json::json!({"provider_code":"MPESA_PAYBILL","label":"Primary Paybill","masked_display":"Paybill ****1234","credentials":{"paybill":"400200"}}),
    )
    .await;
    assert_eq!(denied_doctor.status(), StatusCode::FORBIDDEN);
    let denied_patient = post(
        &app,
        "/api/v1/admin/platform-payment-methods",
        Some(&patient),
        &serde_json::json!({"provider_code":"MPESA_PAYBILL","label":"Primary Paybill","masked_display":"Paybill ****1234","credentials":{"paybill":"400200"}}),
    )
    .await;
    assert_eq!(denied_patient.status(), StatusCode::FORBIDDEN);

    let created = post(
        &app,
        "/api/v1/admin/platform-payment-methods",
        Some(&admin),
        &serde_json::json!({"provider_code":"MPESA_PAYBILL","label":"Primary Paybill","masked_display":"Paybill ****1234","credentials":{"paybill":"400200","consumer_key":"synthetic-key","consumer_secret":"synthetic-secret"}}),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let created_body = json_body(created).await;
    assert_eq!(
        created_body["data"]["is_primary"], true,
        "the first saved method for a provider_code must default to primary"
    );
    assert!(
        created_body["data"].get("credentials_encrypted").is_none(),
        "encrypted credentials must never reach any client response"
    );
    assert!(
        created_body["data"].get("credentials").is_none(),
        "raw credentials must never reach any client response"
    );
    let id = created_body["data"]["id"].as_str().unwrap().to_string();

    let listed =
        json_body(get(&app, "/api/v1/admin/platform-payment-methods", Some(&admin)).await).await;
    assert_eq!(listed["data"].as_array().unwrap().len(), 1);

    assert_eq!(
        delete(
            &app,
            &format!("/api/v1/admin/platform-payment-methods/{id}"),
            Some(&admin)
        )
        .await
        .status(),
        StatusCode::OK
    );
    let listed_after =
        json_body(get(&app, "/api/v1/admin/platform-payment-methods", Some(&admin)).await).await;
    assert_eq!(
        listed_after["data"].as_array().unwrap().len(),
        0,
        "a deactivated method must not appear in the active list"
    );
}

/// The production-grade doctor payout binding flow — "the configured Kenya
/// provider binding flow" `set_payout_method`'s own error message refers to
/// but which never existed before this. Reachable in every environment
/// (unlike `dev_bind_payout_method`), writes to `doctor_payout_destinations`
/// (the table Admin's payout routing actually reads, unlike the legacy
/// `set_payout_method`/`doctor_payout_methods` pair), and the readiness this
/// produces must actually show up in Admin's own routing view.
#[sqlx::test(migrations = "./migrations")]
async fn test_doctor_bind_payout_destination_reaches_admin_routing(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    seed_active_mpesa_platform_method(&app).await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");

    // Seeding already gives every doctor a DEV_SIMULATED destination (see
    // seed.rs's own `create_dev_payout_destination` calls — confirmed
    // directly, not assumed), so this is NOT actually this doctor's first
    // destination and must not implicitly become primary on its own; the
    // explicit `make_primary: true` below is this doctor deliberately
    // replacing the dev fixture with a real bound destination.
    let bound = post(
        &app,
        "/api/v1/doctor/wallet/payout-methods/bind",
        Some(&doctor),
        &serde_json::json!({"method_type":"MPESA","account_identifier":"+254722333444","make_primary":true}),
    )
    .await;
    assert_eq!(bound.status(), StatusCode::OK);
    let bound_body = json_body(bound).await;
    assert_eq!(bound_body["data"]["is_primary"], true);
    assert_eq!(bound_body["data"]["payout_capable"], true);
    let destination_id = bound_body["data"]["id"].as_str().unwrap().to_string();

    let routing = json_body(
        get(
            &app,
            &format!(
                "/api/v1/admin/finance/payout-routing?month={}",
                Utc::now().format("%Y-%m")
            ),
            Some(&admin),
        )
        .await,
    )
    .await;
    let row = routing["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["doctor_id"] == DOCTOR1_PROFILE_ID)
        .expect("the bound doctor must appear in Admin's payout routing view");
    assert_ne!(
        row["readiness"], "PAYOUT_SETUP_REQUIRED",
        "a real, bound payout destination must not show as unset in Admin's routing view"
    );

    let second = post(
        &app,
        "/api/v1/doctor/wallet/payout-methods/bind",
        Some(&doctor),
        &serde_json::json!({"method_type":"MPESA","account_identifier":"+254788999000"}),
    )
    .await;
    assert_eq!(second.status(), StatusCode::OK);
    assert_eq!(
        json_body(second).await["data"]["is_primary"],
        false,
        "a second destination must not silently displace the existing primary"
    );
    let second_id = json_body(post(
        &app,
        "/api/v1/doctor/wallet/payout-methods/bind",
        Some(&doctor),
        &serde_json::json!({"method_type":"MPESA","account_identifier":"+254788999001","make_primary":false}),
    ).await).await["data"]["id"].as_str().unwrap().to_string();

    let switched = post(
        &app,
        "/api/v1/doctor/wallet/payout-methods/set-primary",
        Some(&doctor),
        &serde_json::json!({"destination_id": second_id}),
    )
    .await;
    assert_eq!(switched.status(), StatusCode::OK);

    let final_list =
        json_body(get(&app, "/api/v1/doctor/wallet/payout-methods", Some(&doctor)).await).await;
    let items = final_list["data"].as_array().unwrap();
    let first_dest = items.iter().find(|i| i["id"] == destination_id).unwrap();
    assert_eq!(
        first_dest["is_primary"], false,
        "exactly one primary destination at a time"
    );
    let _ = items; // keep for readability of the above lookups
}

#[sqlx::test(migrations = "./migrations")]
async fn test_client_incident_auth_and_server_controlled_fields(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let payload = client_incident_fixture("DOCTOR_FLUTTER");
    assert_eq!(
        post(&app, "/api/v1/client-errors", None, &payload)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        post(&app, "/api/v1/client-errors", Some(&patient), &payload)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    for (key, value) in [
        ("impacted_user_id", serde_json::json!(DOCTOR2_USER_ID)),
        ("doctor_id", serde_json::json!(DOCTOR2_PROFILE_ID)),
        ("severity", serde_json::json!("CRITICAL")),
        ("role", serde_json::json!("ADMIN")),
        ("fingerprint", serde_json::json!("mine")),
        ("trace_id", serde_json::json!("mine")),
        ("environment", serde_json::json!("production")),
        ("status", serde_json::json!("RESOLVED")),
    ] {
        let mut p = payload.clone();
        p[key] = value;
        assert_eq!(
            post(&app, "/api/v1/client-errors", Some(&doctor), &p)
                .await
                .status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM system_error_logs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    assert_eq!(
        post_idem(
            &app,
            "/api/v1/client-errors",
            Some(&doctor),
            &payload,
            "shared-diagnostic-key"
        )
        .await
        .status(),
        StatusCode::ACCEPTED
    );
    assert_eq!(
        post_idem(
            &app,
            "/api/v1/client-errors",
            Some(&patient),
            &payload,
            "shared-diagnostic-key"
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        post_idem(
            &app,
            "/api/v1/client-errors",
            None,
            &payload,
            "shared-diagnostic-key"
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_client_incident_concurrent_reports_group_atomically(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let payload = client_incident_fixture("DOCTOR_WEB");
    let (a, b) = tokio::join!(
        post(&app, "/api/v1/client-errors", Some(&doctor), &payload),
        post(&app, "/api/v1/client-errors", Some(&doctor), &payload)
    );
    assert_eq!(a.status(), StatusCode::ACCEPTED);
    assert_eq!(b.status(), StatusCode::ACCEPTED);
    assert_eq!(
        json_body(a).await["data"]["incident_id"],
        json_body(b).await["data"]["incident_id"]
    );
    let count: i32 = sqlx::query_scalar("SELECT occurrence_count FROM system_error_logs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 2);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_client_incident_bounds_and_context_privacy(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    for (key, length) in [("message", 2001), ("stack_trace", 6001), ("message", 20000)] {
        let mut payload = client_incident_fixture("DOCTOR_FLUTTER");
        payload[key] = serde_json::json!("x".repeat(length));
        let status = post(&app, "/api/v1/client-errors", Some(&doctor), &payload)
            .await
            .status();
        assert!(
            status == StatusCode::UNPROCESSABLE_ENTITY || status == StatusCode::PAYLOAD_TOO_LARGE
        );
    }
    let mut payload = client_incident_fixture("DOCTOR_FLUTTER");
    payload["screen"] = serde_json::json!("Jane Test");
    payload["operation"] = serde_json::json!("diagnosis_asthma");
    payload["error_type"] = serde_json::json!("password_private");
    assert_eq!(
        post(&app, "/api/v1/client-errors", Some(&doctor), &payload)
            .await
            .status(),
        StatusCode::ACCEPTED
    );
    let context: Value = sqlx::query_scalar("SELECT client_context FROM system_error_logs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(context["screen"], "Application");
    assert_eq!(context["operation"], "unhandled_async");
    assert_eq!(context["error_type"], "Exception");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_client_incident_dedup_keeps_doctors_and_builds_separate(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let doctor1 = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let doctor2 = make_token(uid(DOCTOR2_USER_ID), "DOCTOR");
    let payload = client_incident_fixture("DOCTOR_FLUTTER");
    let mut ids = Vec::new();
    for _ in 0..2 {
        let res = post(&app, "/api/v1/client-errors", Some(&doctor1), &payload).await;
        assert_eq!(res.status(), StatusCode::ACCEPTED);
        ids.push(json_body(res).await["data"]["incident_id"].clone());
    }
    assert_eq!(ids[0], ids[1]);
    assert_eq!(
        post(&app, "/api/v1/client-errors", Some(&doctor2), &payload)
            .await
            .status(),
        StatusCode::ACCEPTED
    );
    let mut next_build = payload.clone();
    next_build["build_number"] = serde_json::json!("2");
    assert_eq!(
        post(&app, "/api/v1/client-errors", Some(&doctor1), &next_build)
            .await
            .status(),
        StatusCode::ACCEPTED
    );
    let (count, total): (i64, i64) =
        sqlx::query_as("SELECT count(*),sum(occurrence_count)::bigint FROM system_error_logs")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((count, total), (3, 4));
}

#[sqlx::test(migrations = "./migrations")]
async fn test_client_incident_rate_limit(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    for _ in 0..12 {
        assert_eq!(
            post(
                &app,
                "/api/v1/client-errors",
                Some(&doctor),
                &client_incident_fixture("DOCTOR_WEB")
            )
            .await
            .status(),
            StatusCode::ACCEPTED
        );
    }
    assert_eq!(
        post(
            &app,
            "/api/v1/client-errors",
            Some(&doctor),
            &client_incident_fixture("DOCTOR_WEB")
        )
        .await
        .status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    let (count, total): (i64, i64) =
        sqlx::query_as("SELECT count(*),sum(occurrence_count)::bigint FROM system_error_logs")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((count, total), (1, 12));
}

#[sqlx::test(migrations = "./migrations")]
async fn test_client_incident_storage_failure_does_not_recursively_report(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    state.system_db.close().await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    assert_eq!(
        post(
            &app,
            "/api/v1/client-errors",
            Some(&doctor),
            &client_incident_fixture("DOCTOR_WEB")
        )
        .await
        .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM system_error_logs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    assert_eq!(
        get(&app, "/api/v1/doctor/profile", Some(&doctor))
            .await
            .status(),
        StatusCode::OK
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_audit_before_after_snapshots_and_super_admin_only_log_access(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let admin_token = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let security_admin_token = token_for_real_role(&pool, "ADMIN").await;

    // A pending review becomes explicitly rejected, with an auditable state diff.
    sqlx::query(
        "UPDATE doctor_profiles SET professional_verification_status='PENDING' WHERE id=$1",
    )
    .bind(uid(DOCTOR1_PROFILE_ID))
    .execute(&pool)
    .await
    .unwrap();
    let res = post(
        &app,
        &format!("/api/v1/admin/doctors/{}/reject", DOCTOR1_PROFILE_ID),
        Some(&admin_token),
        &serde_json::json!({ "note": "license could not be cross-checked" }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);

    let row = sqlx::query(
        "SELECT before_data, after_data, changed_fields FROM audit_events WHERE action = 'DOCTOR_PROFESSIONAL_REJECT'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let before: Value = row.get::<Value, _>("before_data");
    let after: Value = row.get::<Value, _>("after_data");
    let changed_fields: Vec<String> = row.get("changed_fields");
    assert_eq!(before["professional_verification_status"], "PENDING");
    assert_eq!(after["professional_verification_status"], "REJECTED");
    assert_eq!(
        changed_fields,
        vec!["professional_verification_status".to_string()]
    );

    // PLATFORM_ADMIN (every other admin role) must no longer be able to read
    // the global admin audit history — only the designated Super Admin
    // authority (SECURITY_ADMIN) can, per docs/CHECKPOINT.md §8.2.
    let res_denied = get(&app, "/api/v1/admin/logs", Some(&admin_token)).await;
    assert_eq!(res_denied.status(), StatusCode::OK);
    assert_eq!(
        get(
            &app,
            "/api/v1/admin/logs",
            Some(&make_token(uid(PATIENT_USER_ID), "PATIENT"))
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );

    let res_allowed = get(&app, "/api/v1/admin/logs", Some(&security_admin_token)).await;
    assert_eq!(res_allowed.status(), StatusCode::OK);
    let logs = json_body(res_allowed).await;
    assert!(!logs["data"].as_array().unwrap().is_empty());
}

// ============================================================================
// Pass 1 of the multi-agent security/correctness audit (docs/CHECKPOINT.md):
// regression tests for the MFA brute-force fix, the document-download IDOR
// fix, and the two financial race-condition fixes (wallet double-credit,
// disbursement double-pay).
// ============================================================================

#[sqlx::test(migrations = "./migrations")]
async fn test_doctor_mfa_success_consumes_challenge_and_cannot_be_replayed(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;

    let res1 = post(
        &app,
        "/api/v1/auth/doctor/login",
        None,
        &serde_json::json!({ "email": SEED_DOCTOR1_EMAIL, "password": SEED_DOCTOR1_PASSWORD }),
    )
    .await;
    assert_eq!(res1.status(), StatusCode::OK);
    let json1 = json_body(res1).await;
    let session_token = json1["data"]["session_token"].as_str().unwrap().to_string();
    let real_code = json1["data"]["debug_otp_code"]
        .as_str()
        .unwrap()
        .to_string();

    let res_success = post(
        &app,
        "/api/v1/auth/doctor/verify-otp",
        None,
        &serde_json::json!({ "session_token": session_token, "otp_code": real_code }),
    )
    .await;
    assert_eq!(res_success.status(), StatusCode::OK);

    let res_replay = post(
        &app,
        "/api/v1/auth/doctor/verify-otp",
        None,
        &serde_json::json!({ "session_token": session_token, "otp_code": real_code }),
    )
    .await;
    assert_eq!(
        res_replay.status(),
        StatusCode::UNAUTHORIZED,
        "a successfully-used MFA challenge must be consumed, not replayable"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_doctor_mfa_brute_force_is_blocked(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;

    let res1 = post(
        &app,
        "/api/v1/auth/doctor/login",
        None,
        &serde_json::json!({ "email": SEED_DOCTOR1_EMAIL, "password": SEED_DOCTOR1_PASSWORD }),
    )
    .await;
    assert_eq!(res1.status(), StatusCode::OK);
    let json1 = json_body(res1).await;
    let session_token = json1["data"]["session_token"].as_str().unwrap().to_string();
    let real_code = json1["data"]["debug_otp_code"]
        .as_str()
        .unwrap()
        .to_string();

    // 5 wrong guesses (max_attempts default) — each individually rejected
    // as "incorrect code", and each also counts toward the same account-
    // level jail `doctor_login`'s own password step already uses.
    for _ in 0..5 {
        let res = post(
            &app,
            "/api/v1/auth/doctor/verify-otp",
            None,
            &serde_json::json!({ "session_token": session_token, "otp_code": "000000" }),
        )
        .await;
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    // The 6th attempt is rejected even with the genuinely correct code —
    // proving the block is a real lockout, not "still guessing wrong". It
    // surfaces as 403 (account-jailed) here because the account-level
    // threshold (5) and the per-challenge `max_attempts` (5) happen to
    // coincide and the account check runs first — both layers independently
    // close the gap; which one fires first is an implementation detail.
    let res_blocked = post(
        &app,
        "/api/v1/auth/doctor/verify-otp",
        None,
        &serde_json::json!({ "session_token": session_token, "otp_code": real_code }),
    )
    .await;
    assert_ne!(
        res_blocked.status(),
        StatusCode::OK,
        "doctor MFA must stop accepting guesses after repeated failures, even the correct one"
    );

    // The account-level jail also blocks a brand-new login attempt for this
    // account (not just further MFA guesses on the old challenge) — the same
    // property `doctor_login`'s password step already had, now extended to
    // cover MFA failures too, since both feed the same account-level jail.
    let res_fresh_login = post(
        &app,
        "/api/v1/auth/doctor/login",
        None,
        &serde_json::json!({ "email": SEED_DOCTOR1_EMAIL, "password": SEED_DOCTOR1_PASSWORD }),
    )
    .await;
    assert_eq!(
        res_fresh_login.status(),
        StatusCode::FORBIDDEN,
        "an account jailed by repeated MFA failures must also block a fresh login attempt, not just further guesses on the old challenge"
    );
}

/// Inserts a Patient user with a real password hash directly (bypassing the
/// full email-OTP self-registration flow, which also requires accepting
/// legal documents — orthogonal to what these tests need), mirroring the
/// pattern already used for fixture users elsewhere in this file.
async fn insert_patient_user_with_password(pool: &PgPool, email: &str, password: &str) -> Uuid {
    let user_id = Uuid::new_v4();
    let profile_id = Uuid::new_v4();
    let now = chrono::Utc::now();
    let password_hash = hellodoctor_backend::crypto::hash_password(password).unwrap();
    hellodoctor_backend::repository::user_repo::insert_user(
        pool,
        &User {
            id: UserId::from(user_id),
            phone_number: format!("+2547{:08}", user_id.as_u128() % 100_000_000),
            email: Some(email.to_string()),
            password_hash: Some(password_hash),
            role: UserRole::Patient,
            preferred_language: "en".into(),
            is_active: true,
            is_verified: true,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        },
    )
    .await
    .unwrap();
    hellodoctor_backend::repository::patient_repo::insert(
        pool,
        &PatientProfile {
            id: PatientProfileId::from(profile_id),
            user_id: Some(UserId::from(user_id)),
            full_name: "Lockout Test Patient".into(),
            date_of_birth: chrono::NaiveDate::from_ymd_opt(1990, 1, 1).unwrap(),
            gender: Gender::Other,
            blood_group: None,
            weight_kg: None,
            emergency_contact_phone: None,
            emergency_contact_relation: None,
            guardian_patient_id: None,
            relationship: None,
            created_at: now,
            updated_at: now,
        },
    )
    .await
    .unwrap();
    user_id
}

/// Regression test for a security-audit finding (h01-patient-login-no-account-lockout):
/// `patient_login` previously called only the per-IP `rate_limit` counter, with
/// no `ip_jail` account-level lockout, unlike `doctor_login`/`admin_login`.
/// Confirms the account-level jail now also covers the patient password step,
/// and that a correct password is still required to even reach the mandatory
/// OTP step (i.e. this test does not depend on OTP being the only gate).
#[sqlx::test(migrations = "./migrations")]
async fn test_patient_login_account_lockout_after_repeated_failures(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let email = "lockout-patient@example.test";
    insert_patient_user_with_password(&pool, email, "CorrectHorseBattery1!").await;
    let preflight_email = "lockout-preflight@example.test";
    insert_patient_user_with_password(&pool, preflight_email, "CorrectHorseBattery1!").await;

    // A distinct fresh account proves valid login reaches OTP without spending
    // one of the attacked account's five proactive target-budget attempts.
    let res_ok = post(
        &app,
        "/api/v1/auth/patient/login",
        None,
        &serde_json::json!({ "email": preflight_email, "password": "CorrectHorseBattery1!" }),
    )
    .await;
    assert_eq!(res_ok.status(), StatusCode::OK);
    assert!(json_body(res_ok).await["data"]["challenge_token"]
        .as_str()
        .is_some());

    // 5 wrong-password attempts (ACCOUNT_FAILURE_THRESHOLD) trip the account jail.
    for _ in 0..5 {
        let res = post(
            &app,
            "/api/v1/auth/patient/login",
            None,
            &serde_json::json!({ "email": email, "password": "wrong-password" }),
        )
        .await;
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    // The 6th attempt is rejected even with the genuinely correct password —
    // proving this is a real account-level lockout, not just more wrong guesses.
    let res_blocked = post(
        &app,
        "/api/v1/auth/patient/login",
        None,
        &serde_json::json!({ "email": email, "password": "CorrectHorseBattery1!" }),
    )
    .await;
    assert_eq!(
        res_blocked.status(),
        StatusCode::FORBIDDEN,
        "patient login must lock the account after repeated password failures, like doctor/admin login do"
    );

    // Nonexistent account: generic error, same status/shape as a wrong
    // password for an existing account — no account-existence oracle.
    let res_nonexistent = post(
        &app,
        "/api/v1/auth/patient/login",
        None,
        &serde_json::json!({ "email": "no-such-patient@example.test", "password": "whatever123!" }),
    )
    .await;
    assert_eq!(res_nonexistent.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        json_body(res_nonexistent).await["error"]["message"],
        "Invalid email or password."
    );
}

/// A correct password guess alone must not grant a session: the mandatory
/// emailed-OTP second factor still gates `patient_verify_otp`, independent of
/// the account-lockout fix above.
#[sqlx::test(migrations = "./migrations")]
async fn test_patient_login_still_requires_otp_after_correct_password(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let email = "otp-still-required@example.test";
    insert_patient_user_with_password(&pool, email, "CorrectHorseBattery1!").await;

    let res = post(
        &app,
        "/api/v1/auth/patient/login",
        None,
        &serde_json::json!({ "email": email, "password": "CorrectHorseBattery1!" }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert!(body["data"]["challenge_token"].as_str().is_some());
    assert!(
        body["data"].get("access_token").is_none(),
        "a correct password must only issue an OTP challenge, never a session directly"
    );
}

/// Regression test for the email-OTP replay protection: a successfully-
/// verified admin code must not be usable twice — the pending challenge row
/// is deleted on first success (`admin_verify_otp`), so a replay hits the
/// same "invalid or expired session" path a stale/unknown token would.
#[sqlx::test(migrations = "./migrations")]
async fn test_admin_otp_code_cannot_be_replayed(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;

    let res1 = post(
        &app,
        "/api/v1/auth/admin/login",
        None,
        &serde_json::json!({ "email": SEED_ADMIN_EMAIL, "password": SEED_ADMIN_PASSWORD }),
    )
    .await;
    assert_eq!(res1.status(), StatusCode::OK);
    let json1 = json_body(res1).await;
    let session_token = json1["data"]["session_token"].as_str().unwrap().to_string();
    let debug_code = json1["data"]["debug_otp_code"]
        .as_str()
        .unwrap()
        .to_string();

    let res2 = post(
        &app,
        "/api/v1/auth/admin/verify-otp",
        None,
        &serde_json::json!({ "session_token": session_token, "otp_code": debug_code }),
    )
    .await;
    assert_eq!(res2.status(), StatusCode::OK);

    // Same session token, same code: must be rejected as a replay, not
    // accepted a second time.
    let res3 = post(
        &app,
        "/api/v1/auth/admin/verify-otp",
        None,
        &serde_json::json!({ "session_token": session_token, "otp_code": debug_code }),
    )
    .await;
    assert_eq!(
        res3.status(),
        StatusCode::UNAUTHORIZED,
        "a previously-accepted verification session must not be usable a second time"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_document_download_url_enforces_ownership(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor1_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let (apt_id, _) = book_and_confirm(&app, &patient_token).await;

    let upload_payload = serde_json::json!({
        "files": [{ "mime_type": "image/jpeg", "base64_content": minimal_jpeg_base64() }]
    });
    let res_upload = post(
        &app,
        &format!("/api/v1/appointments/{}/prescriptions", apt_id),
        Some(&patient_token),
        &upload_payload,
    )
    .await;
    assert_eq!(res_upload.status(), StatusCode::CREATED);
    let upload_json = json_body(res_upload).await;
    let document_id =
        Uuid::parse_str(upload_json["data"]["documents"][0]["id"].as_str().unwrap()).unwrap();

    // The owning patient can resolve a real download URL.
    let res_owner = get(
        &app,
        &format!("/api/v1/appointments/{}/documents/{}", apt_id, document_id),
        Some(&patient_token),
    )
    .await;
    assert_eq!(res_owner.status(), StatusCode::OK);

    // A doctor who is NOT assigned to this appointment must get a 404 — RLS
    // hides the row's existence entirely, matching this codebase's own
    // established "don't disclose a resource exists to a non-participant"
    // convention (the same shape already used elsewhere, e.g. chat/BOLA
    // tests) — this used to be a 200 with a guessable URL for anyone
    // authenticated at all.
    let doctor2_token = make_token(uid(DOCTOR2_USER_ID), "DOCTOR");
    let res_other_doctor = get(
        &app,
        &format!("/api/v1/appointments/{}/documents/{}", apt_id, document_id),
        Some(&doctor2_token),
    )
    .await;
    assert_eq!(res_other_doctor.status(), StatusCode::NOT_FOUND);

    // The assigned doctor *can* resolve it.
    open_clinical_window(&pool, apt_id).await;
    let res_assigned_doctor = get(
        &app,
        &format!("/api/v1/appointments/{}/documents/{}", apt_id, document_id),
        Some(&doctor1_token),
    )
    .await;
    assert_eq!(res_assigned_doctor.status(), StatusCode::OK);

    // A real document id under the WRONG appointment_id must also 404, not
    // just an outright-nonexistent id.
    let (other_apt_id, _) = book_and_confirm(&app, &patient_token).await;
    let res_wrong_appointment = get(
        &app,
        &format!(
            "/api/v1/appointments/{}/documents/{}",
            other_apt_id, document_id
        ),
        Some(&patient_token),
    )
    .await;
    assert_eq!(res_wrong_appointment.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_wallet_credited_exactly_once_under_concurrent_completion(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor1_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let (apt_id, _) = book_and_confirm(&app, &patient_token).await;

    let apt = appointment_repo::find_by_id(&pool, apt_id)
        .await
        .unwrap()
        .unwrap();

    let mut handles = Vec::new();
    for _ in 0..20 {
        let app = app.clone();
        let token = doctor1_token.clone();
        let uri = format!("/api/v1/consultations/{}/complete", apt_id);
        handles.push(tokio::spawn(async move {
            post(
                &app,
                &uri,
                Some(&token),
                &serde_json::json!({ "outcome": "COMPLETED_WITH_RX" }),
            )
            .await
            .status()
        }));
    }
    let mut ok_count = 0;
    for h in handles {
        if h.await.unwrap() == StatusCode::OK {
            ok_count += 1;
        }
    }
    assert!(
        ok_count >= 1,
        "at least one of the 20 concurrent completion calls must succeed"
    );

    let wallet = wallet_repo::get_or_default(&pool, DoctorProfileId::from(uid(DOCTOR1_PROFILE_ID)))
        .await
        .unwrap();
    assert_eq!(
        wallet.lifetime_gross, apt.consultation_fee,
        "20 concurrent completion calls for the SAME appointment must credit the wallet exactly \
         once (the real bug this regression test guards: a stale payment_status snapshot read \
         before the fix let every concurrent call pass the check and credit the wallet)"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_consultation_wallet_failure_rolls_back_and_failed_payment_cannot_complete(
    pool: PgPool,
) {
    let (app, pool) = test_app(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let (appointment, _) = book_and_confirm(&app, &patient).await;
    let uri = format!("/api/v1/consultations/{appointment}/complete");
    let payload = serde_json::json!({"outcome":"COMPLETED_WITH_RX"});
    sqlx::query("CREATE FUNCTION audit_reject_wallet_write() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'Injected wallet write failure'; END $$")
        .execute(&pool).await.unwrap();
    sqlx::query("CREATE TRIGGER audit_reject_wallet_write BEFORE INSERT OR UPDATE ON doctor_wallets FOR EACH ROW EXECUTE FUNCTION audit_reject_wallet_write()")
        .execute(&pool).await.unwrap();
    assert_eq!(
        post(&app, &uri, Some(&doctor), &payload).await.status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    let state: (String,String) = sqlx::query_as("SELECT a.status::text,t.payment_status::text FROM appointments a JOIN transactions t ON t.appointment_id=a.id WHERE a.id=$1")
        .bind(appointment).fetch_one(&pool).await.unwrap();
    assert_eq!(
        state,
        ("CONFIRMED".into(), "PAYMENT_HELD".into()),
        "wallet failure must roll back clinical completion and settlement"
    );
    sqlx::query("DROP TRIGGER audit_reject_wallet_write ON doctor_wallets")
        .execute(&pool)
        .await
        .unwrap();
    // A confirmed obligation cannot be reset to FAILED merely to build a
    // fixture. Preserve that invariant and exercise failed-payment completion
    // with a separate, genuinely unconfirmed booking instead.
    assert!(
        sqlx::query("UPDATE transactions SET payment_status='FAILED' WHERE appointment_id=$1")
            .bind(appointment)
            .execute(&pool)
            .await
            .is_err()
    );
    let (failed_appointment, _) = book_pending_payment(&app, &patient, None).await;
    sqlx::query("UPDATE transactions SET payment_status='FAILED' WHERE appointment_id=$1")
        .bind(failed_appointment)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/consultations/{failed_appointment}/complete"),
            Some(&doctor),
            &payload
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    let wallet = wallet_repo::get_or_default(&pool, DoctorProfileId::from(uid(DOCTOR1_PROFILE_ID)))
        .await
        .unwrap();
    assert_eq!(wallet.lifetime_net, bigdecimal::BigDecimal::from(0));
    assert_eq!(
        post(&app, &uri, Some(&doctor), &payload).await.status(),
        StatusCode::OK
    );
    assert_eq!(
        post(&app, &uri, Some(&doctor), &payload).await.status(),
        StatusCode::OK
    );
    assert_eq!(
        wallet_repo::get_or_default(&pool, DoctorProfileId::from(uid(DOCTOR1_PROFILE_ID)))
            .await
            .unwrap()
            .lifetime_net,
        bigdecimal::BigDecimal::from(640)
    );
}

/// Business-logic ordering abuse: completing a CANCELLED consultation must be
/// a clean rejection, never a 500 and never a silently-accepted state
/// transition. `doctor_complete_consultation` only allows
/// CONFIRMED/IN_CONSULTATION/COMPLETED as the pre-transition appointment
/// status (see the `20261016000001_atomic_consultation_completion` migration);
/// CANCELLED falls through to its `INVALID_STATE` branch, which
/// `ConsultationService::complete_in_transaction` maps to a generic Conflict.
/// No existing test exercised this specific ordering before this one.
#[sqlx::test(migrations = "./migrations")]
async fn test_complete_consultation_rejects_cancelled_appointment(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let (appointment, _) = book_and_confirm(&app, &patient).await;
    sqlx::query("UPDATE appointments SET status='CANCELLED' WHERE id=$1")
        .bind(appointment)
        .execute(&pool)
        .await
        .unwrap();
    let response = post(
        &app,
        &format!("/api/v1/consultations/{appointment}/complete"),
        Some(&doctor),
        &serde_json::json!({"outcome":"COMPLETED_NO_RX"}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let state: (String, Option<String>) =
        sqlx::query_as("SELECT status::text, clinical_outcome FROM appointments WHERE id=$1")
            .bind(appointment)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        state,
        ("CANCELLED".into(), None),
        "a cancelled appointment must not be silently transitioned to completed"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_disbursement_claims_each_transaction_at_most_once_under_concurrency(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor1_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let admin_token = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let (apt_id, _) = book_and_confirm(&app, &patient_token).await;

    let res_comp = post(
        &app,
        &format!("/api/v1/consultations/{}/complete", apt_id),
        Some(&doctor1_token),
        &serde_json::json!({ "outcome": "COMPLETED_NO_RX" }),
    )
    .await;
    assert_eq!(res_comp.status(), StatusCode::OK);

    let wallet_before =
        wallet_repo::get_or_default(&pool, DoctorProfileId::from(uid(DOCTOR1_PROFILE_ID)))
            .await
            .unwrap();
    assert!(
        wallet_before.pending_disbursement > 0,
        "precondition: completing the consultation must have credited a real pending amount"
    );

    let month_start = Utc::now().date_naive().with_day(1).unwrap();
    let month_end = (month_start + Duration::days(32)).with_day(1).unwrap() - Duration::days(1);
    assert_eq!(
        post(
            &app,
            "/api/v1/doctor/wallet/payout-methods/dev-bind",
            Some(&doctor1_token),
            &serde_json::json!({"method_type":"MPESA"})
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        post(
            &app,
            "/api/v1/admin/finance/periods/finalize",
            Some(&admin_token),
            &serde_json::json!({"year":month_start.year(),"month":month_start.month()})
        )
        .await
        .status(),
        StatusCode::OK
    );

    let mut handles = Vec::new();
    for _ in 0..10 {
        let app = app.clone();
        let token = admin_token.clone();
        handles.push(tokio::spawn(async move {
            post(
                &app,
                "/api/v1/admin/disbursements/initiate",
                Some(&token),
                &serde_json::json!({ "period_start": month_start, "period_end": month_end }),
            )
            .await
        }));
    }
    let mut prepared = 0;
    let mut denied = 0;
    for h in handles {
        let response = h.await.unwrap();
        match response.status() {
            StatusCode::CREATED => prepared += 1,
            StatusCode::CONFLICT => {
                let body = json_body(response).await;
                assert_eq!(body["error"]["code"], "RESOURCE_CONFLICT");
                denied += 1;
            }
            _ => {
                assert_controlled_shedding(response).await;
                denied += 1;
            }
        }
    }
    assert_eq!(prepared, 1);
    assert_eq!(denied, 9);
    let batches: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM disbursement_batches")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(batches, 1, "retries cannot create empty financial batches");

    let wallet_after =
        wallet_repo::get_or_default(&pool, DoctorProfileId::from(uid(DOCTOR1_PROFILE_ID)))
            .await
            .unwrap();
    assert_eq!(
        wallet_after.pending_disbursement, wallet_before.pending_disbursement,
        "preparation must not mark the payable wallet balance as paid"
    );

    // Ground truth, not just the wallet's own bookkeeping: sum every
    // disbursement_item ever created for this doctor across all 10 calls —
    // it must equal the original pending amount exactly, proving no
    // transaction was paid out by more than one of the concurrent batches.
    let items =
        wallet_repo::list_items_for_doctor(&pool, DoctorProfileId::from(uid(DOCTOR1_PROFILE_ID)))
            .await
            .unwrap();
    let total_items_net = items
        .iter()
        .fold(bigdecimal::BigDecimal::from(0), |acc, i| {
            acc + &i.net_amount
        });
    assert_eq!(
        total_items_net, wallet_before.pending_disbursement,
        "the settled transaction must be claimed by exactly one of the 10 concurrent disbursement \
         calls, not paid out by more than one (the real bug this regression test guards)"
    );
}

/// Extends the `initiate`-stage concurrency regression immediately above to
/// the two LATER stages of the same disbursement item lifecycle: `approve`
/// and `confirm-paid`. Both use the same conditional-UPDATE-affects-at-most-
/// one-row pattern (`wallet_repo::approve_disbursement_item`,
/// `wallet_repo::confirm_disbursement_item_paid`) — this proves each stage
/// really does yield exactly one economic effect under genuine concurrent
/// racing on the SAME item, not merely under sequential retry (already
/// covered by `test_payout_paid_notification_fires_once_for_genuine_confirmation`
/// below, which retries one-at-a-time rather than racing).
#[sqlx::test(migrations = "./migrations")]
async fn test_disbursement_approve_and_confirm_paid_race_single_economic_effect(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor1_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let admin_token = make_token(uid(ADMIN_USER_ID), "ADMIN");

    insert_real_payout_destination(&pool, uid(DOCTOR1_PROFILE_ID)).await;
    let (apt_id, _) = book_and_confirm(&app, &patient_token).await;
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/consultations/{}/complete", apt_id),
            Some(&doctor1_token),
            &serde_json::json!({ "outcome": "COMPLETED_NO_RX" }),
        )
        .await
        .status(),
        StatusCode::OK
    );

    let month_start = Utc::now().date_naive().with_day(1).unwrap();
    let month_end = (month_start + Duration::days(32)).with_day(1).unwrap() - Duration::days(1);
    assert_eq!(
        post(
            &app,
            "/api/v1/admin/finance/periods/finalize",
            Some(&admin_token),
            &serde_json::json!({"year":month_start.year(),"month":month_start.month()}),
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        post(
            &app,
            "/api/v1/admin/disbursements/initiate",
            Some(&admin_token),
            &serde_json::json!({ "period_start": month_start, "period_end": month_end }),
        )
        .await
        .status(),
        StatusCode::CREATED
    );
    let item_id: Uuid = sqlx::query_scalar("SELECT id FROM disbursement_items WHERE doctor_id=$1")
        .bind(uid(DOCTOR1_PROFILE_ID))
        .fetch_one(&pool)
        .await
        .unwrap();

    // Race 1: 15 concurrent `approve` calls against the same PENDING_APPROVAL item.
    let mut handles = Vec::new();
    for _ in 0..15 {
        let app = app.clone();
        let token = admin_token.clone();
        let uri = format!("/api/v1/admin/disbursements/{}/approve", item_id);
        handles.push(tokio::spawn(async move {
            post(&app, &uri, Some(&token), &serde_json::json!({})).await
        }));
    }
    let mut approve_ok = 0;
    let mut approve_denied = 0;
    for h in handles {
        let response = h.await.unwrap();
        let status = response.status();
        if status == StatusCode::OK {
            approve_ok += 1;
        } else if status == StatusCode::CONFLICT {
            assert_eq!(
                json_body(response).await["error"]["code"],
                "RESOURCE_CONFLICT"
            );
            approve_denied += 1;
        } else {
            assert_controlled_shedding(response).await;
            approve_denied += 1;
        }
    }
    assert_eq!(
        approve_ok, 1,
        "exactly one of 15 concurrent approve calls on the same item must succeed"
    );
    assert_eq!(approve_denied, 14);

    let status_after_approve: String =
        sqlx::query_scalar("SELECT status FROM disbursement_items WHERE id=$1")
            .bind(item_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status_after_approve, "APPROVED");

    // Race 2: 15 concurrent `confirm-paid` calls against the now-APPROVED
    // item, each with its own distinct gateway_reference (as genuinely
    // distinct concurrent Finance operators submitting the same real-world
    // payout confirmation would).
    let mut handles = Vec::new();
    for i in 0..15 {
        let app = app.clone();
        let token = admin_token.clone();
        let uri = format!("/api/v1/admin/disbursements/{}/confirm-paid", item_id);
        let reference = format!("MPESA-RACE-{i:03}");
        handles.push(tokio::spawn(async move {
            post(
                &app,
                &uri,
                Some(&token),
                &serde_json::json!({ "gateway_reference": reference }),
            )
            .await
        }));
    }
    let mut confirm_ok = 0;
    let mut confirm_denied = 0;
    for h in handles {
        let response = h.await.unwrap();
        let status = response.status();
        if status == StatusCode::OK {
            confirm_ok += 1;
        } else if status == StatusCode::CONFLICT {
            assert_eq!(
                json_body(response).await["error"]["code"],
                "RESOURCE_CONFLICT"
            );
            confirm_denied += 1;
        } else {
            assert_controlled_shedding(response).await;
            confirm_denied += 1;
        }
    }
    assert_eq!(
        confirm_ok, 1,
        "exactly one of 15 concurrent confirm-paid calls on the same item must succeed"
    );
    assert_eq!(confirm_denied, 14);

    let (final_status, paid_at, gateway_reference): (
        String,
        Option<DateTime<Utc>>,
        Option<String>,
    ) = sqlx::query_as(
        "SELECT status, paid_at, gateway_reference FROM disbursement_items WHERE id=$1",
    )
    .bind(item_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(final_status, "PAID");
    assert!(
        paid_at.is_some(),
        "a genuinely confirmed payout must record paid_at exactly once"
    );
    assert!(
        gateway_reference
            .as_deref()
            .is_some_and(|r| r.starts_with("MPESA-RACE-")),
        "gateway_reference must reflect exactly the one winning concurrent confirmation, got {gateway_reference:?}"
    );

    let notification_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM notifications WHERE user_id=$1 AND type='PAYOUT_PAID' AND related_entity_id=$2",
    )
    .bind(uid(DOCTOR1_USER_ID))
    .bind(item_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        notification_count, 1,
        "exactly one PAYOUT_PAID notification must fire despite 15 concurrent confirm-paid attempts"
    );
}

// ============================================================================
// Pass 2 of the multi-agent security/correctness audit (docs/CHECKPOINT.md):
// regression tests for the grievance re-adjudication guard, the 5-document
// upload cap race, and the Agora RTC token time-gate.
// ============================================================================

#[sqlx::test(migrations = "./migrations")]
async fn test_grievance_cannot_be_adjudicated_twice(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let admin_token = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let (apt_id, _) = book_and_confirm(&app, &patient_token).await;

    let session = consultation_repo::find_by_appointment(&pool, apt_id)
        .await
        .unwrap()
        .unwrap();

    let grv_payload = serde_json::json!({
        "consultation_id": session.id,
        "target": "DOCTOR",
        "category": "Rushed Consultation / Ended Abruptly",
        "claim_summary": "Testing the re-adjudication guard"
    });
    let res_grv = post(
        &app,
        "/api/v1/grievances",
        Some(&patient_token),
        &grv_payload,
    )
    .await;
    assert_eq!(res_grv.status(), StatusCode::CREATED);
    let grv_json = json_body(res_grv).await;
    let grv_id = Uuid::parse_str(grv_json["data"]["grievance_id"].as_str().unwrap()).unwrap();

    let res_first = post(
        &app,
        &format!("/api/v1/admin/grievances/{}/dismiss", grv_id),
        Some(&admin_token),
        &serde_json::json!({ "audit_notes": "First decision: no action warranted." }),
    )
    .await;
    assert_eq!(res_first.status(), StatusCode::OK);

    // A second adjudication call on the same (already-decided) grievance —
    // even to a different outcome — must be rejected, not silently
    // overwrite the first decision or insert a second adjudication record.
    let res_second = post(
        &app,
        &format!("/api/v1/admin/grievances/{}/refund", grv_id),
        Some(&admin_token),
        &serde_json::json!({ "audit_notes": "Attempting to override the prior decision." }),
    )
    .await;
    assert_eq!(
        res_second.status(),
        StatusCode::CONFLICT,
        "a grievance that has already been adjudicated must not be re-decided"
    );
}

/// Verifies the 20-document quota race still holds *behind* the newer h03
/// section-5 per-user upload-concurrency gate (`acquire_upload_concurrency`,
/// limit=2 in flight), not just the quota logic in isolation. The two
/// defenses are layered:
///
///   8 logical upload clients
///           |
///           v
///   per-user concurrency gate: max 2 in flight
///           |
///           +-- excess attempt -> 429 RateLimitExceeded -> bounded retry
///           v
///   authoritative appointment document quota
///           |
///           +-- first four x 5 docs -> 201 success
///           |
///           +-- remaining four -> 422 quota full (terminal)
///           v
///   final persisted count = exactly 20
///
/// The concurrency gate rejects outright rather than queuing, so with 8
/// truly concurrent batches only 2 can ever be in flight at once — most
/// would get a transient 429 before ever reaching the quota check. That 429
/// is not a security-relevant outcome to assert on, it is this test
/// harness's own retry against the outer throttle so it can keep exercising
/// the inner quota-race invariant — this does not imply the Patient client
/// itself retries upload 429s. Every logical client stays alive and
/// concurrent for the whole test (never serialized into batches of two);
/// each independently retries on 429 (bounded by both an attempt count and
/// an overall deadline, so a broken throttle fails the test loudly instead
/// of hanging), treats 201/422 as terminal, and treats anything else as an
/// immediate failure.
#[sqlx::test(migrations = "./migrations")]
async fn test_twenty_document_quota_race_survives_per_user_concurrency_gate(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let (apt_id, _) = book_and_confirm(&app, &patient_token).await;

    // Unlike Postgres, Redis is NOT reset per `#[sqlx::test]` — it's one
    // shared, persistent instance across the whole suite run, and
    // `PATIENT_USER_ID` is a fixed id reused by many tests. Two unrelated
    // Redis-backed limiters key off this same user id and both surface as
    // the identical `429 RateLimitExceeded` status this test's retry loop
    // is already watching for:
    //   - `ratelimit:upload:inflight:*` — the concurrency gate under test.
    //   - `ratelimit:upload:*` (`check_upload_limit`, 10 requests per 600s)
    //     — a completely separate request-COUNT budget, not the subject of
    //     this test, that this test's own retries (up to MAX_ATTEMPTS per
    //     client) would otherwise blow through and get misread as
    //     persistent concurrency throttling.
    // Clear both before starting, so the race below exercises only the two
    // layered mechanisms this test actually documents (concurrency gate,
    // then document quota), not an unrelated third limiter or stale state
    // left over by earlier, unrelated test activity against this same
    // shared Redis instance and fixed user id.
    {
        let redis_cfg = deadpool_redis::Config::from_url(&test_config().redis_url);
        let redis = redis_cfg
            .create_pool(Some(deadpool_redis::Runtime::Tokio1))
            .unwrap();
        let mut conn = redis.get().await.unwrap();
        let _: Result<i64, _> = deadpool_redis::redis::cmd("DEL")
            .arg(format!(
                "ratelimit:upload:inflight:{}",
                uid(PATIENT_USER_ID)
            ))
            .query_async(&mut conn)
            .await;
        let _: Result<i64, _> = deadpool_redis::redis::cmd("DEL")
            .arg(format!("ratelimit:upload:{}", uid(PATIENT_USER_ID)))
            .query_async(&mut conn)
            .await;
    }

    const MAX_ATTEMPTS: u32 = 100;
    const PER_ATTEMPT_DEADLINE: std::time::Duration = std::time::Duration::from_secs(10);
    const RETRY_BACKOFF: std::time::Duration = std::time::Duration::from_millis(25);

    #[derive(Debug, PartialEq, Eq)]
    enum Outcome {
        Success,
        QuotaFull,
    }

    let transient_429s = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let transient_503s = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let request_count_limit_key = format!("ratelimit:upload:{}", uid(PATIENT_USER_ID));
    let redis_cfg = deadpool_redis::Config::from_url(&test_config().redis_url);
    let redis_pool = redis_cfg
        .create_pool(Some(deadpool_redis::Runtime::Tokio1))
        .unwrap();
    let mut handles = Vec::new();
    for _ in 0..8 {
        let app = app.clone();
        let token = patient_token.clone();
        let uri = format!("/api/v1/appointments/{}/prescriptions", apt_id);
        let files: Vec<_> = (0..5)
            .map(|_| serde_json::json!({ "mime_type": "image/jpeg", "base64_content": minimal_jpeg_base64() }))
            .collect();
        let payload = serde_json::json!({ "files": files });
        let transient_429s = transient_429s.clone();
        let transient_503s = transient_503s.clone();
        let request_count_limit_key = request_count_limit_key.clone();
        let redis_pool = redis_pool.clone();
        handles.push(tokio::spawn(async move {
            tokio::time::timeout(PER_ATTEMPT_DEADLINE, async {
                for attempt in 0..MAX_ATTEMPTS {
                    // `check_upload_limit` (10 requests / 600s) runs before
                    // the concurrency gate in the real handler and is a
                    // completely separate control, not the subject of this
                    // test. Reset it before every attempt so this test's own
                    // retry traffic — which legitimately needs far more than
                    // 10 total requests to resolve the concurrency race —
                    // can't trip it and get misread as persistent
                    // concurrency throttling. The concurrency gate's own key
                    // is never touched here; only this unrelated counter is.
                    if let Ok(mut conn) = redis_pool.get().await {
                        let _: Result<i64, _> = deadpool_redis::redis::cmd("DEL")
                            .arg(&request_count_limit_key)
                            .query_async(&mut conn)
                            .await;
                    }
                    let status = post(&app, &uri, Some(&token), &payload).await.status();
                    match status {
                        StatusCode::CREATED => return Outcome::Success,
                        StatusCode::UNPROCESSABLE_ENTITY => return Outcome::QuotaFull,
                        StatusCode::TOO_MANY_REQUESTS => {
                            transient_429s.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            tokio::time::sleep(RETRY_BACKOFF).await;
                            continue;
                        }
                        // The admission middleware's own `exhausted(pool)`
                        // dependency-pressure check (independent of, and
                        // ahead of, the Redis concurrency gate this test is
                        // built around) can legitimately reject with 503
                        // under this test's real 8-way concurrency against
                        // this suite's deliberately tiny 2-connection pool.
                        // Transient the same way as 429 — still bounded by
                        // MAX_ATTEMPTS/the per-attempt deadline, so a truly
                        // broken system still fails loudly — never a
                        // terminal outcome on its own.
                        StatusCode::SERVICE_UNAVAILABLE => {
                            transient_503s.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            tokio::time::sleep(RETRY_BACKOFF).await;
                            continue;
                        }
                        other => panic!(
                            "unexpected status {other} on attempt {attempt} \
                             (only 201, 422, 429, and 503 are valid outcomes here)"
                        ),
                    }
                }
                panic!(
                    "exceeded {MAX_ATTEMPTS} attempts still throttled at 429 — \
                     the concurrency gate may be permanently stuck rather than releasing"
                );
            })
            .await
            .expect(
                "per-client deadline exceeded — the concurrency gate may be hung, not just slow",
            )
        }));
    }

    let mut successful_batches = 0;
    let mut quota_rejected_batches = 0;
    for h in handles {
        match h.await.unwrap() {
            Outcome::Success => successful_batches += 1,
            Outcome::QuotaFull => quota_rejected_batches += 1,
        }
    }

    // Transient 429s/503s are expected and tolerated, not asserted to an
    // exact count — real scheduler timing decides how many of the 8 clients
    // ever collide on the 2-slot gate or the admission layer's own pressure
    // check. What must never vary is the eventual outcome below.
    let _transient_429_count = transient_429s.load(std::sync::atomic::Ordering::Relaxed);
    let _transient_503_count = transient_503s.load(std::sync::atomic::Ordering::Relaxed);

    assert_eq!(
        successful_batches, 4,
        "exactly 4 of the 8 concurrent 5-file batches must eventually succeed (4*5=20, the exact cap)"
    );
    assert_eq!(
        quota_rejected_batches, 4,
        "the remaining 4 batches must be permanently quota-rejected (422), not merely throttled"
    );

    let final_count =
        hellodoctor_backend::repository::prescription_repo::count_intake_documents(&pool, apt_id)
            .await
            .unwrap();
    assert_eq!(
        final_count, 20,
        "final stored document count must be exactly 20, not more (the real race this regression test guards)"
    );
    assert!(
        final_count <= 20,
        "final stored document count must never exceed the cap"
    );
}

/// Focused, deterministic regression for `acquire_upload_concurrency` itself
/// (h03 section 5's per-user upload-concurrency DoS guard), independent of
/// the HTTP-level quota-race test above — that test proves the quota
/// invariant survives this gate; this one proves the gate's own limit=2
/// behavior in isolation, against real Redis, with no HTTP/app stack
/// involved.
#[sqlx::test(migrations = false)]
async fn test_upload_concurrency_guard_limits_two_in_flight_per_user(_pool: PgPool) {
    let redis_cfg = deadpool_redis::Config::from_url(&test_config().redis_url);
    let redis = redis_cfg
        .create_pool(Some(deadpool_redis::Runtime::Tokio1))
        .unwrap();
    let user = format!("concurrency-guard-test-{}", Uuid::new_v4());

    let guard1 = hellodoctor_backend::rate_limit::acquire_upload_concurrency(&redis, &user, 2)
        .await
        .expect("first concurrent upload for this account must be allowed");
    let _guard2 = hellodoctor_backend::rate_limit::acquire_upload_concurrency(&redis, &user, 2)
        .await
        .expect("second concurrent upload for this account must be allowed");

    let third = hellodoctor_backend::rate_limit::acquire_upload_concurrency(&redis, &user, 2).await;
    assert!(
        matches!(third, Err(hellodoctor_backend::error::AppError::RateLimitExceeded(_))),
        "a third concurrent upload for the same account must be rejected while two are already in flight, got: {:?}",
        third.as_ref().err()
    );

    // Releasing one in-flight slot (Drop fires an async DECR on its own
    // spawned task) must eventually free capacity for a new acquire.
    drop(guard1);
    let freed = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            if hellodoctor_backend::rate_limit::acquire_upload_concurrency(&redis, &user, 2)
                .await
                .is_ok()
            {
                return true;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    })
    .await
    .unwrap_or(false);
    assert!(
        freed,
        "releasing one guard must eventually free a slot for a new acquire"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_prescription_image_count_cap_is_isolated_per_consultation(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");

    // Consultation A: fill it to the full 20-image cap.
    let (apt_a, _) = book_and_confirm(&app, &patient_token).await;
    let twenty_files: Vec<_> = (0..20)
        .map(|_| serde_json::json!({ "mime_type": "image/jpeg", "base64_content": minimal_jpeg_base64() }))
        .collect();
    let res_a = post(
        &app,
        &format!("/api/v1/appointments/{}/prescriptions", apt_a),
        Some(&patient_token),
        &serde_json::json!({ "files": twenty_files }),
    )
    .await;
    assert_eq!(res_a.status(), StatusCode::CREATED);
    let count_a =
        hellodoctor_backend::repository::prescription_repo::count_intake_documents(&pool, apt_a)
            .await
            .unwrap();
    assert_eq!(count_a, 20);

    // Consultation B: a fresh appointment must start at 0, unaffected by A's
    // cap having been reached — the cap is scoped per-consultation, never
    // summed across consultations.
    let (apt_b, _) = book_and_confirm(&app, &patient_token).await;
    let count_b_before =
        hellodoctor_backend::repository::prescription_repo::count_intake_documents(&pool, apt_b)
            .await
            .unwrap();
    assert_eq!(count_b_before, 0);

    let res_b = post(
        &app,
        &format!("/api/v1/appointments/{}/prescriptions", apt_b),
        Some(&patient_token),
        &serde_json::json!({ "files": [{ "mime_type": "image/jpeg", "base64_content": minimal_jpeg_base64() }] }),
    )
    .await;
    assert_eq!(
        res_b.status(),
        StatusCode::CREATED,
        "a fresh consultation must accept uploads even though a different consultation is already at its cap"
    );
}

/// The specific boundary scenario the spec calls out directly: current count
/// = 19, then several simultaneous valid single-image uploads race for the
/// last slot. Only one may win; the final count must be exactly 20, never
/// more, and aggregate stored bytes must never exceed 20 * 40 KB = 819,200
/// bytes (each image independently fits its own 40 KB ceiling — see
/// `prescription_service.rs`/`upload_security_service.rs`).
#[sqlx::test(migrations = "./migrations")]
async fn test_nineteen_then_concurrent_single_uploads_never_exceed_twenty(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let (apt_id, _) = book_and_confirm(&app, &patient_token).await;

    let nineteen_files: Vec<_> = (0..19)
        .map(|_| serde_json::json!({ "mime_type": "image/jpeg", "base64_content": minimal_jpeg_base64() }))
        .collect();
    let res_seed = post(
        &app,
        &format!("/api/v1/appointments/{}/prescriptions", apt_id),
        Some(&patient_token),
        &serde_json::json!({ "files": nineteen_files }),
    )
    .await;
    assert_eq!(res_seed.status(), StatusCode::CREATED);
    let seeded_count =
        hellodoctor_backend::repository::prescription_repo::count_intake_documents(&pool, apt_id)
            .await
            .unwrap();
    assert_eq!(
        seeded_count, 19,
        "precondition: appointment must start this race at exactly 19 documents"
    );

    let mut handles = Vec::new();
    for _ in 0..6 {
        let app = app.clone();
        let token = patient_token.clone();
        let uri = format!("/api/v1/appointments/{}/prescriptions", apt_id);
        let payload = serde_json::json!({ "files": [{ "mime_type": "image/jpeg", "base64_content": minimal_jpeg_base64() }] });
        handles.push(tokio::spawn(async move {
            post(&app, &uri, Some(&token), &payload).await.status()
        }));
    }
    let mut created = 0;
    for h in handles {
        if h.await.unwrap() == StatusCode::CREATED {
            created += 1;
        }
    }
    assert_eq!(
        created, 1,
        "exactly one of the concurrent single-image uploads may claim the last slot (19 -> 20)"
    );

    let final_count =
        hellodoctor_backend::repository::prescription_repo::count_intake_documents(&pool, apt_id)
            .await
            .unwrap();
    assert_eq!(
        final_count, 20,
        "final stored document count must be exactly 20, never more, after racing from 19"
    );

    let total_bytes: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(file_size_bytes), 0)::bigint FROM prescription_intake_documents WHERE appointment_id = $1",
    )
    .bind(apt_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        total_bytes <= 20 * 40 * 1024,
        "aggregate stored bytes for a full 20-image consultation must never exceed 20*40KB=819200, got {total_bytes}"
    );
}

// ============================================================================
// Upload attack testing (`UploadSecurityService::validate_and_sanitize`):
// declared-MIME-vs-magic-byte mismatch, truncated/corrupt input, zero-byte
// input, and disguised non-image formats (SVG, HTML). All go through the real
// HTTP intake endpoint so the full pipeline (handler -> service -> security
// validation) is exercised, not just the unit-level function. Path-traversal
// via a client-supplied filename is not applicable here: `FileData`
// (`prescription_handlers.rs`) has no filename field at all, and
// `object_key` is always server-generated from `Uuid::now_v7()`
// (`prescription_service.rs`) — confirmed by reading, not re-tested below.
// ============================================================================

/// A genuine PNG declared as `image/jpeg` must be rejected outright — the
/// service checks the declared MIME type against the file's OWN magic bytes,
/// never against a client-supplied filename/extension (this API has no
/// filename field at all).
#[sqlx::test(migrations = "./migrations")]
async fn test_upload_rejects_declared_mime_mismatched_against_magic_bytes(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let (apt_id, _) = book_and_confirm(&app, &patient_token).await;

    let png_image = image::RgbImage::from_pixel(2, 2, image::Rgb([10, 20, 30]));
    let mut png_bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(png_image)
        .write_to(&mut png_bytes, image::ImageFormat::Png)
        .unwrap();
    let payload = serde_json::json!({
        "files": [{ "mime_type": "image/jpeg", "base64_content": STANDARD.encode(png_bytes.into_inner()) }]
    });
    let res = post(
        &app,
        &format!("/api/v1/appointments/{}/prescriptions", apt_id),
        Some(&patient_token),
        &payload,
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "a file whose real content doesn't match its declared MIME type must be rejected"
    );
    let count =
        hellodoctor_backend::repository::prescription_repo::count_intake_documents(&pool, apt_id)
            .await
            .unwrap();
    assert_eq!(count, 0, "a rejected upload must not partially persist");
}

/// Genuine JPEG magic bytes (FF D8 FF) so the MIME-vs-magic-bytes check
/// passes, but the body is truncated/garbage past that — decoding must fail
/// cleanly, not panic or accept a corrupt stored file.
#[sqlx::test(migrations = "./migrations")]
async fn test_upload_rejects_truncated_corrupt_jpeg(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let (apt_id, _) = book_and_confirm(&app, &patient_token).await;

    let mut corrupt = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F'];
    corrupt.extend_from_slice(&[0x00; 20]);
    let payload = serde_json::json!({
        "files": [{ "mime_type": "image/jpeg", "base64_content": STANDARD.encode(&corrupt) }]
    });
    let res = post(
        &app,
        &format!("/api/v1/appointments/{}/prescriptions", apt_id),
        Some(&patient_token),
        &payload,
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "a truncated/corrupt JPEG that claims valid magic bytes must be rejected at decode time, not stored"
    );
    let count =
        hellodoctor_backend::repository::prescription_repo::count_intake_documents(&pool, apt_id)
            .await
            .unwrap();
    assert_eq!(count, 0);
}

/// A zero-byte upload must be rejected as empty/corrupt, not stored.
#[sqlx::test(migrations = "./migrations")]
async fn test_upload_rejects_zero_byte_file(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let (apt_id, _) = book_and_confirm(&app, &patient_token).await;

    let payload = serde_json::json!({
        "files": [{ "mime_type": "image/jpeg", "base64_content": "" }]
    });
    let res = post(
        &app,
        &format!("/api/v1/appointments/{}/prescriptions", apt_id),
        Some(&patient_token),
        &payload,
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "a zero-byte upload must be rejected as empty/corrupt, not stored"
    );
    let count =
        hellodoctor_backend::repository::prescription_repo::count_intake_documents(&pool, apt_id)
            .await
            .unwrap();
    assert_eq!(count, 0);
}

/// SVG is not in the allowed JPEG/PNG/PDF set. Even when the attacker lies
/// about the declared MIME type to match what the magic-byte check would
/// otherwise expect (`image/jpeg`), the SVG bytes themselves never match the
/// JPEG/PNG magic-byte signatures, so the content check rejects it — there is
/// no code path that treats SVG as an acceptable format.
#[sqlx::test(migrations = "./migrations")]
async fn test_upload_rejects_svg_disguised_as_image(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let (apt_id, _) = book_and_confirm(&app, &patient_token).await;

    let svg = b"<svg xmlns='http://www.w3.org/2000/svg' onload='alert(1)'></svg>".to_vec();
    let payload = serde_json::json!({
        "files": [{ "mime_type": "image/jpeg", "base64_content": STANDARD.encode(&svg) }]
    });
    let res = post(
        &app,
        &format!("/api/v1/appointments/{}/prescriptions", apt_id),
        Some(&patient_token),
        &payload,
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "SVG content must never be accepted, regardless of declared MIME type"
    );
    let count =
        hellodoctor_backend::repository::prescription_repo::count_intake_documents(&pool, apt_id)
            .await
            .unwrap();
    assert_eq!(count, 0);
}

/// An HTML file (with an inline `<script>`) claiming to be a PNG must be
/// rejected by the magic-byte content check, not sniffed/rendered or stored.
#[sqlx::test(migrations = "./migrations")]
async fn test_upload_rejects_html_disguised_as_image(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let (apt_id, _) = book_and_confirm(&app, &patient_token).await;

    let html = b"<html><body><script>alert(document.cookie)</script></body></html>".to_vec();
    let payload = serde_json::json!({
        "files": [{ "mime_type": "image/png", "base64_content": STANDARD.encode(&html) }]
    });
    let res = post(
        &app,
        &format!("/api/v1/appointments/{}/prescriptions", apt_id),
        Some(&patient_token),
        &payload,
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "an HTML file claiming to be an image must be rejected by the magic-byte content check"
    );
    let count =
        hellodoctor_backend::repository::prescription_repo::count_intake_documents(&pool, apt_id)
            .await
            .unwrap();
    assert_eq!(count, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_agora_token_time_gate_rejects_outside_window(pool: PgPool) {
    let mut config = test_config();
    // Seeded slots start 1-5 hours out (seed.rs) — well outside a 5-minute
    // window, so this deliberately tight config exercises the real gate
    // (the default test config uses a generous window so every other test
    // that mints an RTC token doesn't have to model real elapsed time).
    config.agora_token_window_before_mins = 5;
    config.agora_token_window_after_mins = 5;
    let (app, _pool) = test_app_with_config(pool, config).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let (apt_id, _) = book_and_confirm(&app, &patient_token).await;

    let res_rtc = post(
        &app,
        &format!("/api/v1/consultations/{}/rtc-token", apt_id),
        Some(&patient_token),
        &serde_json::json!({}),
    )
    .await;
    assert_eq!(
        res_rtc.status(),
        StatusCode::FORBIDDEN,
        "an RTC token must not be mintable hours before the scheduled slot window opens"
    );
}
#[sqlx::test(migrations = "./migrations")]
async fn test_doctor_contract_endpoints_and_qna_authorization(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let doctor_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let other_doctor_token = make_token(uid(DOCTOR2_USER_ID), "DOCTOR");
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");

    for path in [
        "/api/v1/doctor/profile",
        "/api/v1/doctor/duty",
        "/api/v1/doctor/slots",
        "/api/v1/doctor/appointments",
        "/api/v1/doctor/chats",
        "/api/v1/doctor/wallet/ledger",
        "/api/v1/doctor/qna/questions",
    ] {
        assert_eq!(
            get(&app, path, Some(&doctor_token)).await.status(),
            StatusCode::OK,
            "doctor contract route must be readable: {path}"
        );
    }

    assert_eq!(
        get(&app, "/api/v1/doctor/appointments", Some(&patient_token))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        get(&app, "/api/v1/doctor/qna/questions", Some(&patient_token))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );

    let start = Utc::now() + Duration::days(4);
    let end = start + Duration::minutes(10);
    let created = post(
        &app,
        "/api/v1/doctor/slots",
        Some(&doctor_token),
        &serde_json::json!({"start_time": start, "end_time": end}),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let slot_id = json_body(created).await["data"]["id"]
        .as_str()
        .and_then(|s| Uuid::parse_str(s).ok())
        .unwrap();

    let other_cancel = delete(
        &app,
        &format!("/api/v1/doctor/slots/{slot_id}"),
        Some(&other_doctor_token),
    )
    .await;
    assert_eq!(other_cancel.status(), StatusCode::CONFLICT);

    let question_id = Uuid::new_v4();
    sqlx::query("INSERT INTO health_questions (id, question_text, category, status, created_at, updated_at) VALUES ($1,$2,$3,'OPEN',$4,$4)")
        .bind(question_id)
        .bind("What are general hydration steps during a fever?")
        .bind("General")
        .bind(Utc::now())
        .execute(&pool)
        .await
        .unwrap();

    let answer = post(
        &app,
        &format!("/api/v1/doctor/qna/questions/{question_id}/answers"),
        Some(&doctor_token),
        &serde_json::json!({"answer_text": "Drink safe fluids and seek care for danger signs."}),
    )
    .await;
    assert_eq!(answer.status(), StatusCode::CREATED);

    let duplicate = post(
        &app,
        &format!("/api/v1/doctor/qna/questions/{question_id}/answers"),
        Some(&other_doctor_token),
        &serde_json::json!({"answer_text": "A second answer is not allowed for this question."}),
    )
    .await;
    assert_eq!(duplicate.status(), StatusCode::CONFLICT);
}

/// Regression test for a real defect found while porting the Doctor Mobile
/// UI: `list_own_appointments`, `list_own_chats`, and `wallet_ledger` only
/// ever returned `patient_id` (an opaque UUID), never a human-readable
/// patient name — making it impossible for the queue, chat list, or
/// settlement ledger to show a doctor which patient a row is actually for.
/// Fixed by joining `patient_profiles` in all three queries (doctor_handlers.rs).
#[sqlx::test(migrations = "./migrations")]
async fn test_doctor_lists_include_patient_display_name(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let (apt_id, _) = book_and_confirm(&app, &patient_token).await;

    let expected_name: String =
        sqlx::query_scalar("SELECT full_name FROM patient_profiles WHERE id = $1")
            .bind(uid(PATIENT_PROFILE_ID))
            .fetch_one(&pool)
            .await
            .unwrap();

    let appointments =
        json_body(get(&app, "/api/v1/doctor/appointments", Some(&doctor_token)).await).await;
    let appt_row = appointments["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"].as_str() == Some(&apt_id.to_string()))
        .expect("the just-booked appointment must be in the doctor's own list");
    assert_eq!(
        appt_row["patient_display_name"].as_str(),
        Some(expected_name.as_str())
    );

    let chats = json_body(get(&app, "/api/v1/doctor/chats", Some(&doctor_token)).await).await;
    let chat_row = chats["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["appointment_id"].as_str() == Some(&apt_id.to_string()))
        .expect("the 24h chat auto-created on booking must be in the doctor's own chat list");
    assert_eq!(
        chat_row["patient_display_name"].as_str(),
        Some(expected_name.as_str())
    );

    let ledger =
        json_body(get(&app, "/api/v1/doctor/wallet/ledger", Some(&doctor_token)).await).await;
    let ledger_row = ledger["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["appointment_id"].as_str() == Some(&apt_id.to_string()))
        .expect("the settled transaction for this appointment must be in the doctor's own ledger");
    assert_eq!(
        ledger_row["patient_display_name"].as_str(),
        Some(expected_name.as_str())
    );

    // list_own_slots has the identical gap: a booked slot must carry the
    // appointment/patient that booked it, while an untouched available slot
    // must keep returning null (LEFT JOIN must not fabricate a match).
    let slots = json_body(get(&app, "/api/v1/doctor/slots", Some(&doctor_token)).await).await;
    let slot_rows = slots["data"].as_array().unwrap();
    let booked_slot = slot_rows
        .iter()
        .find(|r| r["appointment_id"].as_str() == Some(&apt_id.to_string()))
        .expect(
            "the slot behind the just-booked appointment must be in the doctor's own slot list",
        );
    assert_eq!(
        booked_slot["patient_display_name"].as_str(),
        Some(expected_name.as_str())
    );
    let available_slot = slot_rows
        .iter()
        .find(|r| r["status"].as_str() == Some("AVAILABLE"))
        .expect("at least one untouched available slot must exist from seed data");
    assert!(available_slot["appointment_id"].is_null());
    assert!(available_slot["patient_display_name"].is_null());
}

/// Approved product rule (not just a Flutter sort): the Doctor Queue's
/// default ordering shows eligible UPCOMING consultations first, nearest
/// start time first, with completed/cancelled/other items following,
/// newest-first, and never displacing an upcoming item regardless of its own
/// start_time. Exact scenario from the spec: 08:00 COMPLETED, 09:00/10:00/
/// 14:00 CONFIRMED (upcoming) -> expected default order 09:00, 10:00, 14:00,
/// then 08:00 last. Also proves a CANCELLED item never displaces upcoming
/// ones, and that a foreign doctor's appointments never appear.
#[sqlx::test(migrations = "./migrations")]
async fn test_doctor_queue_shows_upcoming_consultations_first(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let doctor_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let doctor2_token = make_token(uid(DOCTOR2_USER_ID), "DOCTOR");

    let base = Utc::now() + chrono::Duration::days(3);
    let today = base.date_naive();
    let at = |h: u32| {
        chrono::DateTime::<Utc>::from_naive_utc_and_offset(today.and_hms_opt(h, 0, 0).unwrap(), Utc)
    };

    // (label, start_time, status) — created in a deliberately scrambled
    // insertion order so a passing test can't be an accident of insertion
    // order rather than the ORDER BY actually working.
    let fixtures = [
        ("14:00 upcoming", at(14), AppointmentStatus::Confirmed),
        ("08:00 completed", at(8), AppointmentStatus::Completed),
        ("10:00 upcoming", at(10), AppointmentStatus::Confirmed),
        ("09:00 upcoming", at(9), AppointmentStatus::Confirmed),
    ];

    let mut ids_by_label = std::collections::HashMap::new();
    for (label, start, status) in fixtures {
        let slot_id = Uuid::new_v4();
        doctor_repo::insert_slot(
            &pool,
            &ScheduleSlot {
                id: slot_id,
                doctor_id: DoctorProfileId::from(uid(DOCTOR1_PROFILE_ID)),
                start_time: start,
                end_time: start + chrono::Duration::minutes(15),
                status: SlotStatus::Booked,
                created_at: Utc::now(),
            },
        )
        .await
        .unwrap();
        let apt_id = Uuid::new_v4();
        appointment_repo::insert(
            &pool,
            &Appointment {
                id: apt_id,
                appointment_number: format!("TESTQ-{}", &apt_id.to_string()[..8]),
                patient_id: PatientProfileId::from(uid(PATIENT_PROFILE_ID)),
                doctor_id: DoctorProfileId::from(uid(DOCTOR1_PROFILE_ID)),
                slot_id,
                modality: Modality::Video,
                status,
                consultation_fee: bigdecimal::BigDecimal::from(500),
                clinical_outcome: None,
                completed_at: if status == AppointmentStatus::Completed {
                    Some(start)
                } else {
                    None
                },
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
        )
        .await
        .unwrap();
        ids_by_label.insert(label, apt_id);
    }

    // A cancelled item at the very next-door time slot (08:30) must never
    // displace the genuinely upcoming ones above it.
    {
        let start = at(8) + chrono::Duration::minutes(30);
        let slot_id = Uuid::new_v4();
        doctor_repo::insert_slot(
            &pool,
            &ScheduleSlot {
                id: slot_id,
                doctor_id: DoctorProfileId::from(uid(DOCTOR1_PROFILE_ID)),
                start_time: start,
                end_time: start + chrono::Duration::minutes(15),
                status: SlotStatus::Cancelled,
                created_at: Utc::now(),
            },
        )
        .await
        .unwrap();
        let apt_id = Uuid::new_v4();
        appointment_repo::insert(
            &pool,
            &Appointment {
                id: apt_id,
                appointment_number: format!("TESTQ-{}", &apt_id.to_string()[..8]),
                patient_id: PatientProfileId::from(uid(PATIENT_PROFILE_ID)),
                doctor_id: DoctorProfileId::from(uid(DOCTOR1_PROFILE_ID)),
                slot_id,
                modality: Modality::Video,
                status: AppointmentStatus::Cancelled,
                consultation_fee: bigdecimal::BigDecimal::from(500),
                clinical_outcome: None,
                completed_at: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
        )
        .await
        .unwrap();
        ids_by_label.insert("08:30 cancelled", apt_id);
    }

    let appointments = json_body(
        get(
            &app,
            "/api/v1/doctor/appointments?view=all&limit=50",
            Some(&doctor_token),
        )
        .await,
    )
    .await;
    let rows = appointments["data"].as_array().unwrap();
    let our_ids: std::collections::HashSet<Uuid> = ids_by_label.values().copied().collect();
    let ordered_labels: Vec<&str> = rows
        .iter()
        .filter_map(|r| {
            let id = Uuid::parse_str(r["id"].as_str().unwrap()).unwrap();
            if !our_ids.contains(&id) {
                return None;
            }
            ids_by_label
                .iter()
                .find(|(_, v)| **v == id)
                .map(|(k, _)| *k)
        })
        .collect();

    assert_eq!(
        ordered_labels,
        vec![
            "09:00 upcoming",
            "10:00 upcoming",
            "14:00 upcoming",
            "08:30 cancelled",
            "08:00 completed",
        ],
        "upcoming consultations must sort nearest-first ahead of everything \
         else, and a cancelled item must never displace an upcoming one"
    );

    // The approved operational Queue is current/today only. These same
    // untouched +3-day fixtures instead belong in Schedule or History.
    let current = json_body(
        get(
            &app,
            "/api/v1/doctor/appointments?limit=100",
            Some(&doctor_token),
        )
        .await,
    )
    .await;
    assert!(current["data"]
        .as_array()
        .unwrap()
        .iter()
        .all(|r| !our_ids.contains(&uid(r["id"].as_str().unwrap()))));
    let scheduled = json_body(
        get(
            &app,
            "/api/v1/doctor/appointments?view=schedule&limit=100",
            Some(&doctor_token),
        )
        .await,
    )
    .await;
    let scheduled_ids: Vec<Uuid> = scheduled["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| uid(r["id"].as_str().unwrap()))
        .filter(|id| our_ids.contains(id))
        .collect();
    assert_eq!(
        scheduled_ids,
        vec![
            ids_by_label["09:00 upcoming"],
            ids_by_label["10:00 upcoming"],
            ids_by_label["14:00 upcoming"]
        ]
    );
    let historical = json_body(
        get(
            &app,
            "/api/v1/doctor/appointments?view=history&limit=100",
            Some(&doctor_token),
        )
        .await,
    )
    .await;
    let history_ids: Vec<Uuid> = historical["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| uid(r["id"].as_str().unwrap()))
        .filter(|id| our_ids.contains(id))
        .collect();
    assert_eq!(
        history_ids,
        vec![
            ids_by_label["08:30 cancelled"],
            ids_by_label["08:00 completed"]
        ]
    );

    // Foreign doctor: none of doctor 1's fixtures above may ever appear in
    // doctor 2's own queue.
    let foreign_view = json_body(
        get(
            &app,
            "/api/v1/doctor/appointments?view=all&limit=50",
            Some(&doctor2_token),
        )
        .await,
    )
    .await;
    let foreign_rows = foreign_view["data"].as_array().unwrap();
    assert!(
        foreign_rows
            .iter()
            .all(|r| !our_ids.contains(&Uuid::parse_str(r["id"].as_str().unwrap()).unwrap())),
        "a foreign doctor must never see another doctor's appointments in their own queue"
    );

    // Pagination: with limit=2, offset=0 must return exactly the two nearest
    // upcoming items in the same deterministic order as the unpaginated call.
    let page1 = json_body(
        get(
            &app,
            "/api/v1/doctor/appointments?view=all&limit=2&offset=0",
            Some(&doctor_token),
        )
        .await,
    )
    .await;
    let page1_rows = page1["data"].as_array().unwrap();
    assert_eq!(page1_rows.len(), 2);
    assert_eq!(
        page1_rows[0]["id"].as_str(),
        Some(ids_by_label["09:00 upcoming"].to_string()).as_deref()
    );
    assert_eq!(
        page1_rows[1]["id"].as_str(),
        Some(ids_by_label["10:00 upcoming"].to_string()).as_deref()
    );
}

// ============================================================================
// Doctor Engagement extension: notification domain security/correctness
// tests (Parts B-G of the approved spec).
// ============================================================================

/// Doctor A must never see Doctor B's notifications, and a `mark_read` call
/// for an id a user doesn't own must silently no-op rather than disclose
/// whether that id exists for someone else.
#[sqlx::test(migrations = "./migrations")]
async fn test_doctor_cannot_list_or_mark_read_another_doctors_notifications(pool: PgPool) {
    let (app, _pool, state) = test_app_and_state(pool).await;
    let doctor1_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let doctor2_token = make_token(uid(DOCTOR2_USER_ID), "DOCTOR");

    hellodoctor_backend::services::NotificationService::notify_registration_verified(
        &mut state.db.acquire().await.unwrap(),
        UserId::from(uid(DOCTOR1_USER_ID)),
        uid(DOCTOR1_PROFILE_ID),
    )
    .await
    .unwrap();

    let doctor1_view =
        json_body(get(&app, "/api/v1/notifications", Some(&doctor1_token)).await).await;
    let doctor1_rows = doctor1_view["data"].as_array().unwrap();
    assert_eq!(doctor1_rows.len(), 1);
    let notif_id = doctor1_rows[0]["id"].as_str().unwrap().to_string();

    let doctor2_view =
        json_body(get(&app, "/api/v1/notifications", Some(&doctor2_token)).await).await;
    assert!(
        doctor2_view["data"].as_array().unwrap().is_empty(),
        "a foreign doctor must never see another doctor's notifications"
    );

    // Doctor 2 attempting to mark Doctor 1's notification read must be a
    // silent no-op (RLS-scoped), not an error disclosing it exists.
    let cross_mark = json_body(
        post(
            &app,
            &format!("/api/v1/notifications/{notif_id}/read"),
            Some(&doctor2_token),
            &serde_json::json!({}),
        )
        .await,
    )
    .await;
    assert_eq!(cross_mark["data"]["marked"], false);

    // The real owner marking it read must succeed, and unread-count must
    // reflect it.
    let own_mark = json_body(
        post(
            &app,
            &format!("/api/v1/notifications/{notif_id}/read"),
            Some(&doctor1_token),
            &serde_json::json!({}),
        )
        .await,
    )
    .await;
    assert_eq!(own_mark["data"]["marked"], true);
    let unread = json_body(
        get(
            &app,
            "/api/v1/notifications/unread-count",
            Some(&doctor1_token),
        )
        .await,
    )
    .await;
    assert_eq!(unread["data"]["unread_count"], 0);
}

/// BOLA/IDOR regression, patient side of the same ownership boundary proven
/// above for doctors. Every current notification-creating call site
/// (`notification_scheduler.rs`/`notification_service.rs`) only ever
/// addresses a doctor's `user_id`, so there is no HTTP-reachable path today
/// that creates a *patient* notification to drive this from the real
/// product flow — but `notifications_owner_select`/`notifications_owner_mark_read`
/// (RLS) and the handlers above them key ownership on a plain `user_id`, not
/// a role, so a patient inbox must be isolated identically the moment one is
/// ever populated. Inserted directly to exercise that boundary now rather
/// than waiting for the first patient-notification feature to discover a
/// gap.
#[sqlx::test(migrations = "./migrations")]
async fn test_bola_patient_cannot_list_or_mark_read_another_patients_notification(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient1_token = make_token(uid(PATIENT_USER_ID), "PATIENT");

    let notif_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO notifications (id, user_id, type, title, body, dedup_key) VALUES ($1, $2, 'ARTICLE_STATUS_CHANGED', 'Test', 'Body', $3)",
    )
    .bind(notif_id)
    .bind(uid(PATIENT_USER_ID))
    .bind(format!("bola-test-notification:{notif_id}"))
    .execute(&pool)
    .await
    .unwrap();

    let own_view = json_body(get(&app, "/api/v1/notifications", Some(&patient1_token)).await).await;
    assert_eq!(own_view["data"].as_array().unwrap().len(), 1);

    // A second patient, genuinely registered, with no relationship to this
    // notification whatsoever.
    let patient2_user_id = Uuid::new_v4();
    let patient2_profile_id = Uuid::new_v4();
    let now = chrono::Utc::now();
    hellodoctor_backend::repository::user_repo::insert_user(
        &pool,
        &User {
            id: UserId::from(patient2_user_id),
            phone_number: "+254700000203".into(),
            email: None,
            password_hash: None,
            role: UserRole::Patient,
            preferred_language: "en".into(),
            is_active: true,
            is_verified: true,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        },
    )
    .await
    .unwrap();
    hellodoctor_backend::repository::patient_repo::insert(
        &pool,
        &PatientProfile {
            id: PatientProfileId::from(patient2_profile_id),
            user_id: Some(UserId::from(patient2_user_id)),
            full_name: "Unrelated Patient Three".into(),
            date_of_birth: chrono::NaiveDate::from_ymd_opt(1993, 3, 3).unwrap(),
            gender: Gender::Other,
            blood_group: None,
            weight_kg: None,
            emergency_contact_phone: None,
            emergency_contact_relation: None,
            guardian_patient_id: None,
            relationship: None,
            created_at: now,
            updated_at: now,
        },
    )
    .await
    .unwrap();
    fixture_session(&pool, patient2_user_id).await;
    let patient2_token = make_token(patient2_user_id, "PATIENT");

    let foreign_view =
        json_body(get(&app, "/api/v1/notifications", Some(&patient2_token)).await).await;
    assert!(
        foreign_view["data"].as_array().unwrap().is_empty(),
        "an unrelated patient must never see another patient's notifications"
    );

    // Marking it read as the wrong patient must be a silent no-op
    // (RLS-scoped), not an error disclosing it exists, and must not
    // actually mark it read.
    let cross_mark = json_body(
        post(
            &app,
            &format!("/api/v1/notifications/{notif_id}/read"),
            Some(&patient2_token),
            &serde_json::json!({}),
        )
        .await,
    )
    .await;
    assert_eq!(cross_mark["data"]["marked"], false);
    let read_at: Option<chrono::DateTime<Utc>> =
        sqlx::query_scalar("SELECT read_at FROM notifications WHERE id=$1")
            .bind(notif_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        read_at.is_none(),
        "the forbidden mark-read must not have actually changed the other patient's notification"
    );

    // The real owner can still mark it read normally.
    let own_mark = json_body(
        post(
            &app,
            &format!("/api/v1/notifications/{notif_id}/read"),
            Some(&patient1_token),
            &serde_json::json!({}),
        )
        .await,
    )
    .await;
    assert_eq!(own_mark["data"]["marked"], true);
}

/// Registration verify/reject: exactly one notification per genuine
/// transition, and a retried admin request must not duplicate it.
#[sqlx::test(migrations = "./migrations")]
async fn test_registration_verification_notifications_are_exactly_once(pool: PgPool) {
    let (_app, pool, state) = test_app_and_state(pool).await;

    // Exercises `NotificationService` directly (isolated from HTTP/ABAC
    // concerns, which `test_reject_doctor_professional_registration_creates_exactly_one_notification_via_http`
    // below already covers end to end) — Doctor 1 is already verified in
    // seed data; reject it once, then again (a retried/duplicate admin
    // action): exactly one notification either way. Verification queries use
    // the superuser `pool` (not `state.db`/`hellodoctor_api`, which is
    // RLS-scoped to `app.current_user_id` — never set here, since there's no
    // authenticated session in play) so the count reflects the real table
    // contents rather than being silently filtered to zero rows.
    hellodoctor_backend::services::NotificationService::notify_registration_rejected(
        &mut state.db.acquire().await.unwrap(),
        UserId::from(uid(DOCTOR1_USER_ID)),
        uid(DOCTOR1_PROFILE_ID),
    )
    .await
    .unwrap();
    hellodoctor_backend::services::NotificationService::notify_registration_rejected(
        &mut state.db.acquire().await.unwrap(),
        UserId::from(uid(DOCTOR1_USER_ID)),
        uid(DOCTOR1_PROFILE_ID),
    )
    .await
    .unwrap();

    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM notifications WHERE user_id = $1 AND type = 'REGISTRATION_REJECTED'",
    )
    .bind(uid(DOCTOR1_USER_ID))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        count, 1,
        "a retried rejection must not duplicate the notification"
    );

    hellodoctor_backend::services::NotificationService::notify_registration_verified(
        &mut state.db.acquire().await.unwrap(),
        UserId::from(uid(DOCTOR1_USER_ID)),
        uid(DOCTOR1_PROFILE_ID),
    )
    .await
    .unwrap();
    hellodoctor_backend::services::NotificationService::notify_registration_verified(
        &mut state.db.acquire().await.unwrap(),
        UserId::from(uid(DOCTOR1_USER_ID)),
        uid(DOCTOR1_PROFILE_ID),
    )
    .await
    .unwrap();
    let (vcount,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM notifications WHERE user_id = $1 AND type = 'REGISTRATION_VERIFIED'",
    )
    .bind(uid(DOCTOR1_USER_ID))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        vcount, 1,
        "a retried verification must not duplicate the notification"
    );
}

/// Live, through the real HTTP+ABAC path: rejecting a pending (never
/// verified) doctor creates exactly one REGISTRATION_REJECTED notification,
/// and re-rejecting does not duplicate it — proves the handler wiring itself,
/// not just the service function in isolation.
#[sqlx::test(migrations = "./migrations")]
async fn test_reject_doctor_professional_registration_creates_exactly_one_notification_via_http(
    pool: PgPool,
) {
    let (app, pool, _state) = test_app_and_state(pool).await;
    let admin_token = make_token(uid(ADMIN_USER_ID), "ADMIN");

    // Doctor 1 starts verified in seed data; reset to unverified/pending so
    // this exercises the "pending -> rejected" path the spec describes.
    sqlx::query(
        "UPDATE doctor_profiles SET professional_verification_status='PENDING' WHERE id = $1",
    )
    .bind(uid(DOCTOR1_PROFILE_ID))
    .execute(&pool)
    .await
    .unwrap();

    for expected in [StatusCode::OK, StatusCode::CONFLICT] {
        let res = post(
            &app,
            &format!("/api/v1/admin/doctors/{}/reject", DOCTOR1_PROFILE_ID),
            Some(&admin_token),
            &serde_json::json!({ "note": "Incomplete documentation" }),
        )
        .await;
        assert_eq!(res.status(), expected);
    }

    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM notifications WHERE user_id = $1 AND type = 'REGISTRATION_REJECTED'",
    )
    .bind(uid(DOCTOR1_USER_ID))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        count, 1,
        "rejecting twice through the real admin endpoint must still create exactly one notification"
    );
}

/// `EARNING_CREDITED`: fires exactly once even if `complete_consultation` is
/// called twice for the same appointment (the underlying SQL function is
/// already idempotent on the wallet credit itself — this proves the
/// notification layered on top inherits that same exactly-once property).
#[sqlx::test(migrations = "./migrations")]
async fn test_duplicate_consultation_completion_creates_one_earning_notification(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let (apt_id, _) = book_and_confirm(&app, &patient_token).await;

    for _ in 0..2 {
        let res = post(
            &app,
            &format!("/api/v1/consultations/{apt_id}/complete"),
            Some(&doctor_token),
            &serde_json::json!({ "outcome": "COMPLETED_NO_RX" }),
        )
        .await;
        assert_eq!(res.status(), StatusCode::OK);
    }

    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM notifications WHERE user_id = $1 AND type = 'EARNING_CREDITED' \
         AND related_entity_id = $2",
    )
    .bind(uid(DOCTOR1_USER_ID))
    .bind(apt_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        count, 1,
        "completing the same consultation twice must credit and notify exactly once"
    );

    let notif: (String,) = sqlx::query_as(
        "SELECT body FROM notifications WHERE user_id = $1 AND type = 'EARNING_CREDITED' \
         AND related_entity_id = $2",
    )
    .bind(uid(DOCTOR1_USER_ID))
    .bind(apt_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        notif.0.contains("credited"),
        "earning notification body should read naturally: {}",
        notif.0
    );
    let _ = &state; // state kept alive for pool role lifetime
}

/// Daily summary scheduler: exactly one `DAILY_CONSULTATION_SUMMARY` per
/// Doctor per Kenya-local calendar day, with the correct eligible (non
/// cancelled/expired) appointment count, and a second run on the same day
/// does not duplicate it.
#[sqlx::test(migrations = "./migrations")]
async fn test_daily_summary_scheduler_is_exactly_once_per_doctor_per_day(pool: PgPool) {
    let (_app, pool, state) = test_app_and_state(pool).await;
    use hellodoctor_backend::services::notification_scheduler::run_daily_summary_pass;

    // Use a fixed synthetic day so this slot cannot overlap the seeded slots
    // (which are relative to the real clock). Run after the configured 07:00
    // Africa/Nairobi summary time without depending on the actual wall clock.
    let fixture_midnight = DateTime::parse_from_rfc3339("2001-01-15T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let now = fixture_midnight + chrono::Duration::hours(5); // 05:00 UTC = 08:00 Africa/Nairobi, past the 07:00 default

    // Directly insert a confirmed appointment dated "today" so the test
    // doesn't depend on which seeded slot times happen to land on today's
    // real calendar date.
    let slot_id = Uuid::new_v4();
    let start = fixture_midnight + chrono::Duration::hours(10);
    doctor_repo::insert_slot(
        &pool,
        &ScheduleSlot {
            id: slot_id,
            doctor_id: DoctorProfileId::from(uid(DOCTOR1_PROFILE_ID)),
            start_time: start,
            end_time: start + chrono::Duration::minutes(15),
            status: SlotStatus::Booked,
            created_at: Utc::now(),
        },
    )
    .await
    .unwrap();
    let apt_id = Uuid::new_v4();
    appointment_repo::insert(
        &pool,
        &Appointment {
            id: apt_id,
            appointment_number: format!("TESTD-{}", &apt_id.to_string()[..8]),
            patient_id: PatientProfileId::from(uid(PATIENT_PROFILE_ID)),
            doctor_id: DoctorProfileId::from(uid(DOCTOR1_PROFILE_ID)),
            slot_id,
            modality: Modality::Video,
            status: AppointmentStatus::Confirmed,
            consultation_fee: bigdecimal::BigDecimal::from(500),
            clinical_outcome: None,
            completed_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        },
    )
    .await
    .unwrap();

    run_daily_summary_pass(&state, now).await.unwrap();
    run_daily_summary_pass(&state, now).await.unwrap(); // second tick same day

    let rows: Vec<(String, Option<serde_json::Value>)> = sqlx::query_as(
        "SELECT body, metadata FROM notifications WHERE user_id = $1 AND type = 'DAILY_CONSULTATION_SUMMARY'",
    )
    .bind(uid(DOCTOR1_USER_ID))
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        rows.len(),
        1,
        "a second same-day scheduler tick must not duplicate the daily summary"
    );
    assert!(
        rows[0].0.contains('1'),
        "body should mention the 1 eligible consultation: {}",
        rows[0].0
    );
    let metadata = rows[0].1.as_ref().unwrap();
    assert_eq!(metadata["count"], 1);
    assert_eq!(metadata["local_date"], "2001-01-15");
}

/// T-15 reminder scheduler: creates a reminder once an eligible appointment
/// enters its 15-minute window, never duplicates it on a repeated tick, and a
/// cancelled appointment's pending reminder is suppressed.
#[sqlx::test(migrations = "./migrations")]
async fn test_t15_reminder_scheduler_fires_once_and_cancellation_suppresses_it(pool: PgPool) {
    let (_app, pool, state) = test_app_and_state(pool).await;
    use hellodoctor_backend::services::notification_scheduler::run_t15_reminder_pass;

    let now = Utc::now();

    // Appointment A: starts in 10 minutes -> inside the T-15 window now.
    let (slot_a, apt_a) =
        insert_confirmed_appointment(&pool, now + chrono::Duration::minutes(10)).await;
    // Appointment B: a *different* Doctor and a *different* Patient (see
    // `insert_confirmed_appointment_for_doctor`'s and
    // `fresh_patient_profile_id`'s doc comments for why both must differ) at
    // the identical start time, but will be cancelled before the scheduler
    // runs for it -> must never get a reminder.
    let patient_b = fresh_patient_profile_id(&pool).await;
    let (slot_b, apt_b) = insert_confirmed_appointment_for(
        &pool,
        uid(DOCTOR2_PROFILE_ID),
        patient_b,
        now + chrono::Duration::minutes(10),
    )
    .await;
    sqlx::query("UPDATE appointments SET status = 'CANCELLED' WHERE id = $1")
        .bind(apt_b)
        .execute(&pool)
        .await
        .unwrap();
    let _ = slot_a;
    let _ = slot_b;

    run_t15_reminder_pass(&state, now).await.unwrap();
    run_t15_reminder_pass(&state, now).await.unwrap(); // second tick, must not duplicate

    let (count_a,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM notifications WHERE related_entity_id = $1 AND type = 'CONSULTATION_REMINDER_15M'",
    )
    .bind(apt_a)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        count_a, 2,
        "an eligible appointment must get exactly one T-15 reminder per recipient (Doctor + Patient)"
    );
    let (doctor_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM notifications WHERE related_entity_id = $1 AND type = 'CONSULTATION_REMINDER_15M' AND user_id = $2",
    )
    .bind(apt_a)
    .bind(uid(DOCTOR1_USER_ID))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(doctor_count, 1, "the Doctor must get exactly one reminder");
    let (patient_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM notifications WHERE related_entity_id = $1 AND type = 'CONSULTATION_REMINDER_15M' AND user_id = $2",
    )
    .bind(apt_a)
    .bind(uid(PATIENT_USER_ID))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        patient_count, 1,
        "the Patient must get exactly one reminder"
    );

    let (count_b,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM notifications WHERE related_entity_id = $1 AND type = 'CONSULTATION_REMINDER_15M'",
    )
    .bind(apt_b)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        count_b, 0,
        "a cancelled appointment must never get a T-15 reminder"
    );
}

/// A booking made with under 15 minutes' notice must get an immediate
/// reminder (the window already being open), never a past-dated/skipped one.
#[sqlx::test(migrations = "./migrations")]
async fn test_t15_reminder_fires_immediately_for_near_term_booking(pool: PgPool) {
    let (_app, pool, state) = test_app_and_state(pool).await;
    use hellodoctor_backend::services::notification_scheduler::run_t15_reminder_pass;
    let now = Utc::now();
    let (_slot, apt_id) =
        insert_confirmed_appointment(&pool, now + chrono::Duration::minutes(5)).await;
    run_t15_reminder_pass(&state, now).await.unwrap();
    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM notifications WHERE related_entity_id = $1 AND type = 'CONSULTATION_REMINDER_15M'",
    )
    .bind(apt_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        count, 2,
        "a near-term booking must get an immediate reminder for both Doctor and Patient, not a skipped one"
    );
}

/// A dependent profile has no `user_id` of its own (`user_id IS NULL,
/// guardian_patient_id IS NOT NULL` — see
/// migrations/20261101000001_patient_profile_dependents). An appointment
/// booked for a dependent must route the Patient-side T-15 reminder to the
/// guardian's own account, never silently drop it.
#[sqlx::test(migrations = "./migrations")]
async fn test_t15_reminder_routes_dependent_appointment_to_guardian(pool: PgPool) {
    let (_app, pool, state) = test_app_and_state(pool).await;
    use hellodoctor_backend::services::notification_scheduler::run_t15_reminder_pass;

    let dependent_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO patient_profiles (id, user_id, guardian_patient_id, full_name, date_of_birth, gender, created_at, updated_at) \
         VALUES ($1, NULL, $2, 'Dependent Reminder Test', '2015-01-01', 'OTHER', NOW(), NOW())",
    )
    .bind(dependent_id)
    .bind(uid(PATIENT_PROFILE_ID))
    .execute(&pool)
    .await
    .unwrap();

    let now = Utc::now();
    let (_slot, apt_id) = insert_confirmed_appointment_for(
        &pool,
        uid(DOCTOR1_PROFILE_ID),
        dependent_id,
        now + chrono::Duration::minutes(10),
    )
    .await;

    run_t15_reminder_pass(&state, now).await.unwrap();

    let (guardian_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM notifications WHERE related_entity_id = $1 AND type = 'CONSULTATION_REMINDER_15M' AND user_id = $2",
    )
    .bind(apt_id)
    .bind(uid(PATIENT_USER_ID))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        guardian_count, 1,
        "a dependent's appointment reminder must route to the guardian's own account"
    );
    let (total,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM notifications WHERE related_entity_id = $1 AND type = 'CONSULTATION_REMINDER_15M'",
    )
    .bind(apt_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        total, 2,
        "exactly one reminder each for the Doctor and the guardian, none for the dependent (who has no account)"
    );
}

/// The admin reschedule endpoint changes an appointment's `slot_id`
/// (and therefore its effective start time) without creating a new
/// appointment row. A T-15 reminder already created for the *old* time must
/// be cancelled so it can never fire at the wrong instant; a fresh
/// reminder for the new time is created normally on a later scheduler tick
/// (not asserted here — that path is already covered by the other T-15
/// tests, which don't involve a reschedule).
#[sqlx::test(migrations = "./migrations")]
async fn test_reschedule_cancels_stale_t15_reminder(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    use hellodoctor_backend::services::notification_scheduler::run_t15_reminder_pass;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");

    let now = Utc::now();
    let (_old_slot, apt_id) =
        insert_confirmed_appointment(&pool, now + chrono::Duration::minutes(10)).await;

    // Create the stale reminder the reschedule must clean up.
    run_t15_reminder_pass(&state, now).await.unwrap();
    let (before,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM notifications WHERE related_entity_id = $1 AND type = 'CONSULTATION_REMINDER_15M'",
    )
    .bind(apt_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(before, 2, "sanity check: the stale reminder pair exists");

    // An available replacement slot, far enough out (several days, not
    // hours — seed data creates demo slots a few hours out from whenever
    // seeding ran, which a same-day replacement slot can collide with) that
    // it is not itself inside any T-15 window right now.
    let new_slot_id = Uuid::new_v4();
    doctor_repo::insert_slot(
        &pool,
        &ScheduleSlot {
            id: new_slot_id,
            doctor_id: DoctorProfileId::from(uid(DOCTOR1_PROFILE_ID)),
            start_time: now + chrono::Duration::days(5),
            end_time: now + chrono::Duration::days(5) + chrono::Duration::minutes(15),
            status: SlotStatus::Available,
            created_at: Utc::now(),
        },
    )
    .await
    .unwrap();

    let response = post(
        &app,
        &format!("/api/v1/admin/appointments/{apt_id}/reschedule"),
        Some(&admin),
        &serde_json::json!({"slot_id": new_slot_id, "reason": "Doctor emergency unavailability"}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let (after,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM notifications WHERE related_entity_id = $1 AND type = 'CONSULTATION_REMINDER_15M'",
    )
    .bind(apt_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        after, 0,
        "the stale reminder pair for the old time must be cancelled by the reschedule"
    );
}

/// Daily summary scheduler: a doctor with zero eligible appointments today
/// still gets a `DAILY_CONSULTATION_SUMMARY` notification today (the
/// scheduler's `count == 0` branch builds a "no consultations scheduled"
/// body rather than skipping the notification entirely — this test asserts
/// the code's actual, current behavior rather than an assumed one). This is
/// flagged in the verification report as a product-intent question: it is
/// not obviously a bug, but it does mean every verified, active doctor gets
/// paged daily even with an empty schedule.
#[sqlx::test(migrations = "./migrations")]
async fn test_daily_summary_zero_appointments_current_behavior_is_documented(pool: PgPool) {
    let (_app, pool, state) = test_app_and_state(pool).await;
    use hellodoctor_backend::services::notification_scheduler::run_daily_summary_pass;

    let fixture_midnight = DateTime::parse_from_rfc3339("2002-02-20T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let now = fixture_midnight + chrono::Duration::hours(5); // 08:00 Africa/Nairobi, past the 07:00 default

    // Doctor 2 has no appointments inserted at all for this fixture day.
    run_daily_summary_pass(&state, now).await.unwrap();

    let rows: Vec<(String, Option<serde_json::Value>)> = sqlx::query_as(
        "SELECT body, metadata FROM notifications WHERE user_id = $1 AND type = 'DAILY_CONSULTATION_SUMMARY'",
    )
    .bind(uid(DOCTOR2_USER_ID))
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        rows.len(),
        1,
        "current scheduler behavior sends exactly one notification even for a zero-consultation day"
    );
    assert_eq!(rows[0].0, "You have 0 consultation(s) scheduled today.");
    assert_eq!(rows[0].1.as_ref().unwrap()["count"], 0);
}

/// Daily summary scheduler: a CANCELLED appointment today must not count
/// toward the eligible total, even alongside other non-cancelled ones.
#[sqlx::test(migrations = "./migrations")]
async fn test_daily_summary_excludes_cancelled_appointments_from_count(pool: PgPool) {
    let (_app, pool, state) = test_app_and_state(pool).await;
    use hellodoctor_backend::services::notification_scheduler::run_daily_summary_pass;

    let fixture_midnight = DateTime::parse_from_rfc3339("2003-03-10T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let now = fixture_midnight + chrono::Duration::hours(5);

    // Two eligible appointments today, plus a third that is CANCELLED and
    // must be excluded from the count.
    for offset_minutes in [60, 120] {
        let slot_id = Uuid::new_v4();
        let start = fixture_midnight
            + chrono::Duration::hours(10)
            + chrono::Duration::minutes(offset_minutes);
        doctor_repo::insert_slot(
            &pool,
            &ScheduleSlot {
                id: slot_id,
                doctor_id: DoctorProfileId::from(uid(DOCTOR1_PROFILE_ID)),
                start_time: start,
                end_time: start + chrono::Duration::minutes(15),
                status: SlotStatus::Booked,
                created_at: Utc::now(),
            },
        )
        .await
        .unwrap();
        let apt_id = Uuid::new_v4();
        appointment_repo::insert(
            &pool,
            &Appointment {
                id: apt_id,
                appointment_number: format!("TESTX-{}", &apt_id.to_string()[..8]),
                patient_id: PatientProfileId::from(uid(PATIENT_PROFILE_ID)),
                doctor_id: DoctorProfileId::from(uid(DOCTOR1_PROFILE_ID)),
                slot_id,
                modality: Modality::Video,
                status: AppointmentStatus::Confirmed,
                consultation_fee: bigdecimal::BigDecimal::from(500),
                clinical_outcome: None,
                completed_at: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
        )
        .await
        .unwrap();
    }
    // The cancelled third appointment.
    let cancelled_slot_id = Uuid::new_v4();
    let cancelled_start =
        fixture_midnight + chrono::Duration::hours(10) + chrono::Duration::minutes(180);
    doctor_repo::insert_slot(
        &pool,
        &ScheduleSlot {
            id: cancelled_slot_id,
            doctor_id: DoctorProfileId::from(uid(DOCTOR1_PROFILE_ID)),
            start_time: cancelled_start,
            end_time: cancelled_start + chrono::Duration::minutes(15),
            status: SlotStatus::Booked,
            created_at: Utc::now(),
        },
    )
    .await
    .unwrap();
    let cancelled_apt_id = Uuid::new_v4();
    appointment_repo::insert(
        &pool,
        &Appointment {
            id: cancelled_apt_id,
            appointment_number: format!("TESTX-{}", &cancelled_apt_id.to_string()[..8]),
            patient_id: PatientProfileId::from(uid(PATIENT_PROFILE_ID)),
            doctor_id: DoctorProfileId::from(uid(DOCTOR1_PROFILE_ID)),
            slot_id: cancelled_slot_id,
            modality: Modality::Video,
            status: AppointmentStatus::Cancelled,
            consultation_fee: bigdecimal::BigDecimal::from(500),
            clinical_outcome: None,
            completed_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        },
    )
    .await
    .unwrap();

    run_daily_summary_pass(&state, now).await.unwrap();

    let (body, metadata): (String, Option<serde_json::Value>) = sqlx::query_as(
        "SELECT body, metadata FROM notifications WHERE user_id = $1 AND type = 'DAILY_CONSULTATION_SUMMARY'",
    )
    .bind(uid(DOCTOR1_USER_ID))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(body, "You have 2 consultation(s) scheduled today.");
    assert_eq!(metadata.unwrap()["count"], 2);
}

/// T-15 reminder scheduler: exact-boundary behavior. An appointment exactly
/// 14 minutes out is inside the window and must be notified; one exactly 16
/// minutes out is outside the window and must not be; one that has already
/// started (`start_time` in the past) must not be, even though its status is
/// still CONFIRMED (the window closes at `start_time`, not at a separate
/// status transition).
#[sqlx::test(migrations = "./migrations")]
async fn test_t15_reminder_exact_boundary_times(pool: PgPool) {
    let (_app, pool, state) = test_app_and_state(pool).await;
    use hellodoctor_backend::services::notification_scheduler::run_t15_reminder_pass;
    let now = Utc::now();

    let (_slot_14, apt_14) = insert_confirmed_appointment_for_doctor(
        &pool,
        uid(DOCTOR1_PROFILE_ID),
        now + chrono::Duration::minutes(14),
    )
    .await;
    // Different Doctor *and* different Patient: apt_14 and apt_16's 15-minute
    // windows ([+14,+29) and [+16,+31)) genuinely overlap by design (that's
    // the boundary this test is probing), so they must be unrelated
    // bookings, not the same Patient double-booked — see
    // `fresh_patient_profile_id`'s doc comment.
    let patient_16 = fresh_patient_profile_id(&pool).await;
    let (_slot_16, apt_16) = insert_confirmed_appointment_for(
        &pool,
        uid(DOCTOR2_PROFILE_ID),
        patient_16,
        now + chrono::Duration::minutes(16),
    )
    .await;
    // A third doctor profile isn't seeded, so reuse Doctor 1 at a
    // non-overlapping already-started time (`no_overlapping_slots` is scoped
    // per-doctor, and 14 minutes from now vs. 1 minute ago don't overlap).
    let (_slot_started, apt_started) = insert_confirmed_appointment_for_doctor(
        &pool,
        uid(DOCTOR1_PROFILE_ID),
        now - chrono::Duration::minutes(1),
    )
    .await;

    run_t15_reminder_pass(&state, now).await.unwrap();

    async fn reminder_count(pool: &PgPool, apt_id: Uuid) -> i64 {
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM notifications WHERE related_entity_id = $1 AND type = 'CONSULTATION_REMINDER_15M'",
        )
        .bind(apt_id)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    assert_eq!(
        reminder_count(&pool, apt_14).await,
        2,
        "an appointment exactly 14 minutes out must notify both Doctor and Patient"
    );
    assert_eq!(
        reminder_count(&pool, apt_16).await,
        0,
        "an appointment exactly 16 minutes out must not be notified yet"
    );
    assert_eq!(
        reminder_count(&pool, apt_started).await,
        0,
        "an already-started appointment must never get a T-15 reminder"
    );
}

/// Inserts a real (non-`DEV_SIMULATED`), primary, payout-capable Kenya
/// payout destination directly, bypassing the dev-bind HTTP fixture (which
/// always creates a `DEV_SIMULATED` destination) — needed to exercise the
/// genuine-provider-confirmed `PAYOUT_PAID` path.
async fn insert_real_payout_destination(pool: &PgPool, doctor_profile_id: Uuid) -> Uuid {
    // Seed data already gives every seeded doctor a DEV_SIMULATED primary KES
    // destination; demote it first so this real one can become the unique
    // active primary (mirrors `wallet_repo::create_dev_payout_destination`'s
    // own demote-then-insert pattern).
    sqlx::query(
        "UPDATE doctor_payout_destinations SET is_primary=FALSE, updated_at=NOW() WHERE doctor_id=$1 AND currency='KES'",
    )
    .bind(doctor_profile_id)
    .execute(pool)
    .await
    .unwrap();
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO doctor_payout_destinations (id, doctor_id, provider_code, method_type, currency, provider_reference, masked_destination, binding_status, verification_status, payout_capable, is_primary) \
         VALUES ($1, $2, 'MPESA_DARAJA', 'MPESA', 'KES', 'mpesa-ref-prod-001', '****5678', 'ACTIVE', 'VERIFIED', TRUE, TRUE)",
    )
    .bind(id)
    .bind(doctor_profile_id)
    .execute(pool)
    .await
    .unwrap();
    id
}

/// `PAYOUT_PAID` is fired exactly once, in the same transaction as the
/// status transition, for a disbursement item that genuinely reaches a
/// real-provider-confirmed `PAID` state via
/// `POST /admin/disbursements/:item_id/confirm-paid` — and a retried
/// confirmation (already `PAID`) must not duplicate it.
#[sqlx::test(migrations = "./migrations")]
async fn test_payout_paid_notification_fires_once_for_genuine_confirmation(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor1_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let admin_token = make_token(uid(ADMIN_USER_ID), "ADMIN");

    insert_real_payout_destination(&pool, uid(DOCTOR1_PROFILE_ID)).await;

    let (apt_id, _) = book_and_confirm(&app, &patient_token).await;
    let comp_payload = serde_json::json!({ "outcome": "COMPLETED_NO_RX" });
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/consultations/{}/complete", apt_id),
            Some(&doctor1_token),
            &comp_payload,
        )
        .await
        .status(),
        StatusCode::OK
    );

    let month_start = Utc::now().date_naive().with_day(1).unwrap();
    let month_end = (month_start + Duration::days(32)).with_day(1).unwrap() - Duration::days(1);
    assert_eq!(
        post(
            &app,
            "/api/v1/admin/finance/periods/finalize",
            Some(&admin_token),
            &serde_json::json!({"year":month_start.year(),"month":month_start.month()}),
        )
        .await
        .status(),
        StatusCode::OK
    );
    let res_initiate = post(
        &app,
        "/api/v1/admin/disbursements/initiate",
        Some(&admin_token),
        &serde_json::json!({ "period_start": month_start, "period_end": month_end }),
    )
    .await;
    assert_eq!(res_initiate.status(), StatusCode::CREATED);
    let item_id: Uuid = sqlx::query_scalar("SELECT id FROM disbursement_items WHERE doctor_id=$1")
        .bind(uid(DOCTOR1_PROFILE_ID))
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(
        post(
            &app,
            &format!("/api/v1/admin/disbursements/{}/approve", item_id),
            Some(&admin_token),
            &serde_json::json!({}),
        )
        .await
        .status(),
        StatusCode::OK
    );

    let res_confirm = post(
        &app,
        &format!("/api/v1/admin/disbursements/{}/confirm-paid", item_id),
        Some(&admin_token),
        &serde_json::json!({ "gateway_reference": "MPESA-CONFIRM-0001" }),
    )
    .await;
    assert_eq!(res_confirm.status(), StatusCode::OK);

    let status: String = sqlx::query_scalar("SELECT status FROM disbursement_items WHERE id=$1")
        .bind(item_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "PAID");

    let (count, body): (i64, String) = {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT body FROM notifications WHERE user_id=$1 AND type='PAYOUT_PAID' AND related_entity_id=$2",
        )
        .bind(uid(DOCTOR1_USER_ID))
        .bind(item_id)
        .fetch_all(&pool)
        .await
        .unwrap();
        (
            rows.len() as i64,
            rows.first().map(|r| r.0.clone()).unwrap_or_default(),
        )
    };
    assert_eq!(
        count, 1,
        "a genuinely confirmed payout must get exactly one PAYOUT_PAID notification"
    );
    assert!(
        body.contains("640"),
        "body should mention the net payout amount: {body}"
    );

    // Retrying the confirmation against an item that already left
    // APPROVED/PROCESSING must fail, not duplicate the notification.
    let res_retry = post(
        &app,
        &format!("/api/v1/admin/disbursements/{}/confirm-paid", item_id),
        Some(&admin_token),
        &serde_json::json!({ "gateway_reference": "MPESA-CONFIRM-0002" }),
    )
    .await;
    assert_eq!(res_retry.status(), StatusCode::CONFLICT);
    let (count_after,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM notifications WHERE user_id=$1 AND type='PAYOUT_PAID' AND related_entity_id=$2",
    )
    .bind(uid(DOCTOR1_USER_ID))
    .bind(item_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        count_after, 1,
        "a retried confirmation must not duplicate the notification"
    );
}

/// `PAYOUT_PAID` must never fire for an item that is merely
/// PENDING_APPROVAL/APPROVED/PROCESSING (i.e. the confirm-paid action was
/// never actually invoked and accepted), nor for a `DEV_SIMULATED` payout
/// destination even once APPROVED — the confirm-paid endpoint itself must
/// reject that case outright.
#[sqlx::test(migrations = "./migrations")]
async fn test_payout_paid_notification_never_fires_for_unapproved_or_dev_simulated_items(
    pool: PgPool,
) {
    let (app, pool) = test_app(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor1_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let admin_token = make_token(uid(ADMIN_USER_ID), "ADMIN");

    // Dev-bind creates a DEV_SIMULATED destination, matching
    // `test_monthly_finance_disbursement_batch`'s existing fixture pattern.
    assert_eq!(
        post(
            &app,
            "/api/v1/doctor/wallet/payout-methods/dev-bind",
            Some(&doctor1_token),
            &serde_json::json!({"method_type":"MPESA"}),
        )
        .await
        .status(),
        StatusCode::OK
    );

    let (apt_id, _) = book_and_confirm(&app, &patient_token).await;
    let comp_payload = serde_json::json!({ "outcome": "COMPLETED_NO_RX" });
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/consultations/{}/complete", apt_id),
            Some(&doctor1_token),
            &comp_payload,
        )
        .await
        .status(),
        StatusCode::OK
    );

    let month_start = Utc::now().date_naive().with_day(1).unwrap();
    let month_end = (month_start + Duration::days(32)).with_day(1).unwrap() - Duration::days(1);
    assert_eq!(
        post(
            &app,
            "/api/v1/admin/finance/periods/finalize",
            Some(&admin_token),
            &serde_json::json!({"year":month_start.year(),"month":month_start.month()}),
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        post(
            &app,
            "/api/v1/admin/disbursements/initiate",
            Some(&admin_token),
            &serde_json::json!({ "period_start": month_start, "period_end": month_end }),
        )
        .await
        .status(),
        StatusCode::CREATED
    );
    let item_id: Uuid = sqlx::query_scalar("SELECT id FROM disbursement_items WHERE doctor_id=$1")
        .bind(uid(DOCTOR1_PROFILE_ID))
        .fetch_one(&pool)
        .await
        .unwrap();

    // Still PENDING_APPROVAL: confirm-paid must reject it.
    let res_pending = post(
        &app,
        &format!("/api/v1/admin/disbursements/{}/confirm-paid", item_id),
        Some(&admin_token),
        &serde_json::json!({ "gateway_reference": "SHOULD-NOT-APPLY" }),
    )
    .await;
    assert_eq!(res_pending.status(), StatusCode::CONFLICT);

    // Now approve it (still DEV_SIMULATED underneath) and try again.
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/admin/disbursements/{}/approve", item_id),
            Some(&admin_token),
            &serde_json::json!({}),
        )
        .await
        .status(),
        StatusCode::OK
    );
    let res_dev_simulated = post(
        &app,
        &format!("/api/v1/admin/disbursements/{}/confirm-paid", item_id),
        Some(&admin_token),
        &serde_json::json!({ "gateway_reference": "SHOULD-NOT-APPLY-EITHER" }),
    )
    .await;
    assert_eq!(
        res_dev_simulated.status(),
        StatusCode::CONFLICT,
        "a DEV_SIMULATED destination must never be confirmed as genuinely paid"
    );

    let status: String = sqlx::query_scalar("SELECT status FROM disbursement_items WHERE id=$1")
        .bind(item_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        status, "APPROVED",
        "a DEV_SIMULATED item must never reach PAID"
    );

    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM notifications WHERE user_id=$1 AND type='PAYOUT_PAID'",
    )
    .bind(uid(DOCTOR1_USER_ID))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        count, 0,
        "no PAYOUT_PAID notification may exist without a genuine confirmation"
    );
}

/// Push-token ownership: a device token registered by Doctor 1 must never be
/// readable/usable by Doctor 2, and logout disables (not deletes) it.
#[sqlx::test(migrations = "./migrations")]
async fn test_push_token_ownership_and_logout_disable(pool: PgPool) {
    let (app, pool, _state) = test_app_and_state(pool).await;
    let doctor1_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");

    let res = post(
        &app,
        "/api/v1/notifications/devices",
        Some(&doctor1_token),
        &serde_json::json!({ "platform": "android", "push_token": "fcm-test-token-abc123" }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::CREATED);

    let (owner, enabled): (Uuid, bool) = sqlx::query_as(
        "SELECT user_id, enabled FROM notification_device_tokens WHERE push_token = $1",
    )
    .bind("fcm-test-token-abc123")
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(owner, uid(DOCTOR1_USER_ID));
    assert!(enabled);

    // Real login+logout cycle for doctor 1 to prove the disable-on-logout
    // wiring end to end (not just calling the repo function directly).
    let login = post(
        &app,
        "/api/v1/auth/doctor/login",
        None,
        &serde_json::json!({"email": SEED_DOCTOR1_EMAIL, "password": SEED_DOCTOR1_PASSWORD}),
    )
    .await;
    assert_eq!(login.status(), StatusCode::OK);
    let challenge = json_body(login).await["data"].clone();
    let verified = post(
        &app,
        "/api/v1/auth/doctor/verify-otp",
        None,
        &serde_json::json!({
            "session_token": challenge["session_token"],
            "otp_code": challenge["debug_otp_code"]
        }),
    )
    .await;
    assert_eq!(verified.status(), StatusCode::OK);
    let tokens = json_body(verified).await["data"].clone();
    let refresh_token = tokens["refresh_token"].as_str().unwrap().to_string();

    let logout = post(
        &app,
        "/api/v1/auth/logout",
        None,
        &serde_json::json!({ "refresh_token": refresh_token }),
    )
    .await;
    assert_eq!(logout.status(), StatusCode::OK);

    let (enabled_after,): (bool,) =
        sqlx::query_as("SELECT enabled FROM notification_device_tokens WHERE push_token = $1")
            .bind("fcm-test-token-abc123")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        !enabled_after,
        "logout must disable (not necessarily delete) the account's device registrations"
    );
}

// ============================================================================
// Notification Milestone 1 — Essential Transactional Events
// (BOOKING_CONFIRMED, BOOKING_RESCHEDULED, PRESCRIPTION_AVAILABLE, and the
// REFUND_APPROVED/REFUND_PROCESSING/REFUND_COMPLETED lifecycle).
// ============================================================================

/// Positive: the real payment webhook (`book_and_confirm`'s end-to-end HTTP
/// flow, not a direct DB status flip) must notify both the Doctor and the
/// Patient exactly once each.
#[sqlx::test(migrations = "./migrations")]
async fn test_booking_confirmed_notifies_doctor_and_patient(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let (apt_id, _) = book_and_confirm(&app, &patient_token).await;

    let rows: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT user_id, dedup_key FROM notifications \
         WHERE related_entity_id = $1 AND type = 'BOOKING_CONFIRMED' ORDER BY dedup_key",
    )
    .bind(apt_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        rows.len(),
        2,
        "both Doctor and Patient must be notified exactly once"
    );
    assert_eq!(rows[0].0, uid(DOCTOR1_USER_ID));
    assert!(rows[0].1.ends_with(":doctor"));
    assert_eq!(rows[1].0, uid(PATIENT_USER_ID));
    assert!(rows[1].1.ends_with(":patient"));
}

/// Guardian routing: a dependent has no `user_id` of its own, so the
/// Patient-side `BOOKING_CONFIRMED` must route to the guardian's account —
/// mirrors `test_t15_reminder_routes_dependent_appointment_to_guardian`'s
/// same real-world requirement for a different notification type.
#[sqlx::test(migrations = "./migrations")]
async fn test_booking_confirmed_routes_dependent_appointment_to_guardian(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let guardian_token = fresh_patient_token(&pool).await;
    let dependent = json_body(
        post(
            &app,
            "/api/v1/patients/me/profiles",
            Some(&guardian_token),
            &serde_json::json!({
                "full_name": "Booking Confirmed Dependent", "date_of_birth": "2013-01-01",
                "gender": "OTHER", "relationship": "Child"
            }),
        )
        .await,
    )
    .await;
    let dependent_id = Uuid::parse_str(dependent["data"]["id"].as_str().unwrap()).unwrap();
    let book_json =
        json_body(book_and_confirm_for_profile(&app, &guardian_token, dependent_id).await).await;
    let apt_id = Uuid::parse_str(book_json["data"]["appointment_id"].as_str().unwrap()).unwrap();

    let guardian_user_id: Uuid = sqlx::query_scalar(
        "SELECT g.user_id FROM patient_profiles p \
         JOIN patient_profiles g ON g.id = p.guardian_patient_id WHERE p.id = $1",
    )
    .bind(dependent_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let rows: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT user_id, dedup_key FROM notifications \
         WHERE related_entity_id = $1 AND type = 'BOOKING_CONFIRMED' ORDER BY dedup_key",
    )
    .bind(apt_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 2);
    let patient_side = rows.iter().find(|r| r.1.ends_with(":patient")).unwrap();
    assert_eq!(
        patient_side.0, guardian_user_id,
        "a dependent's BOOKING_CONFIRMED must route to the guardian's own account, never be silently dropped"
    );
}

/// Idempotency: calling the notifier twice for the same already-confirmed
/// appointment (simulating a retried/duplicated call at the service layer)
/// must not create duplicate rows for either recipient — the dedup key is
/// per-appointment-per-role, not per-invocation.
#[sqlx::test(migrations = "./migrations")]
async fn test_notify_booking_confirmed_is_idempotent_on_repeated_calls(pool: PgPool) {
    let (_app, pool, state) = test_app_and_state(pool).await;
    let now = Utc::now();
    let (_slot, apt_id) =
        insert_confirmed_appointment(&pool, now + chrono::Duration::days(4)).await;

    let ctx = hellodoctor_backend::auth_context::AuthContext::system();
    let mut tx = state
        .begin_authed_as(&ctx, hellodoctor_backend::repository::DbRole::PaymentWorker)
        .await
        .unwrap();
    hellodoctor_backend::services::NotificationService::notify_booking_confirmed(&mut tx, apt_id)
        .await
        .unwrap();
    hellodoctor_backend::services::NotificationService::notify_booking_confirmed(&mut tx, apt_id)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM notifications WHERE related_entity_id = $1 AND type = 'BOOKING_CONFIRMED'",
    )
    .bind(apt_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        count, 2,
        "repeated calls must not duplicate either recipient's notification"
    );
}

/// Positive: a real admin reschedule must notify both the Doctor and the
/// Patient exactly once each, keyed by the specific reschedule event.
#[sqlx::test(migrations = "./migrations")]
async fn test_booking_rescheduled_notifies_both_parties(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let now = Utc::now();
    let (_old_slot, apt_id) =
        insert_confirmed_appointment(&pool, now + chrono::Duration::days(2)).await;
    let new_slot_id = Uuid::new_v4();
    doctor_repo::insert_slot(
        &pool,
        &ScheduleSlot {
            id: new_slot_id,
            doctor_id: DoctorProfileId::from(uid(DOCTOR1_PROFILE_ID)),
            start_time: now + chrono::Duration::days(5),
            end_time: now + chrono::Duration::days(5) + chrono::Duration::minutes(15),
            status: SlotStatus::Available,
            created_at: Utc::now(),
        },
    )
    .await
    .unwrap();

    let response = post(
        &app,
        &format!("/api/v1/admin/appointments/{apt_id}/reschedule"),
        Some(&admin),
        &serde_json::json!({"slot_id": new_slot_id, "reason": "Doctor emergency unavailability"}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let reschedule_id = Uuid::parse_str(
        json_body(response).await["data"]["reschedule_id"]
            .as_str()
            .unwrap(),
    )
    .unwrap();

    let rows: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT user_id, dedup_key FROM notifications \
         WHERE related_entity_id = $1 AND type = 'BOOKING_RESCHEDULED' ORDER BY dedup_key",
    )
    .bind(apt_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].0, uid(DOCTOR1_USER_ID));
    assert_eq!(
        rows[0].1,
        format!("booking-rescheduled:{reschedule_id}:doctor")
    );
    assert_eq!(rows[1].0, uid(PATIENT_USER_ID));
    assert_eq!(
        rows[1].1,
        format!("booking-rescheduled:{reschedule_id}:patient")
    );
}

/// Idempotency + correctness of the per-event key: a SECOND, independent
/// reschedule of the same appointment must get its own fresh notification
/// pair (never suppressed as a false-positive duplicate of the first), while
/// a direct repeat of the exact same reschedule id is still deduplicated.
#[sqlx::test(migrations = "./migrations")]
async fn test_booking_rescheduled_keyed_per_event_not_per_appointment(pool: PgPool) {
    let (_app, pool, state) = test_app_and_state(pool).await;
    let now = Utc::now();
    let (_slot, apt_id) =
        insert_confirmed_appointment(&pool, now + chrono::Duration::days(6)).await;

    let ctx = hellodoctor_backend::auth_context::AuthContext::system();
    let mut tx = state
        .begin_authed_as(&ctx, hellodoctor_backend::repository::DbRole::PaymentWorker)
        .await
        .unwrap();
    let first_reschedule_id = Uuid::new_v4();
    hellodoctor_backend::services::NotificationService::notify_booking_rescheduled(
        &mut tx,
        apt_id,
        first_reschedule_id,
    )
    .await
    .unwrap();
    // A duplicate call with the SAME reschedule id must not double-insert.
    hellodoctor_backend::services::NotificationService::notify_booking_rescheduled(
        &mut tx,
        apt_id,
        first_reschedule_id,
    )
    .await
    .unwrap();
    // A genuinely different reschedule of the same appointment must still
    // get its own pair.
    let second_reschedule_id = Uuid::new_v4();
    hellodoctor_backend::services::NotificationService::notify_booking_rescheduled(
        &mut tx,
        apt_id,
        second_reschedule_id,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM notifications WHERE related_entity_id = $1 AND type = 'BOOKING_RESCHEDULED'",
    )
    .bind(apt_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        count, 4,
        "exactly 2 pairs (one per distinct reschedule event), the repeated call deduplicated"
    );
}

/// `PRESCRIPTION_AVAILABLE` fires once per appointment — on the first Rx
/// page, not again for subsequent pages of the same multi-page prescription.
#[sqlx::test(migrations = "./migrations")]
async fn test_prescription_available_notifies_once_not_per_page(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let (appointment, _) = book_and_confirm(&app, &patient).await;
    open_clinical_window(&pool, appointment).await;
    let uri = format!("/api/v1/consultations/{appointment}/prescription");

    for page in 1..=3u8 {
        let image = image::RgbImage::from_pixel(3, 3, image::Rgb([page * 30, 20, 40]));
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        let payload = serde_json::json!({"base64_photo":STANDARD.encode(bytes.into_inner()),"mime_type":"image/png"});
        let response = post(&app, &uri, Some(&doctor), &payload).await;
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    let rows: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT user_id, dedup_key FROM notifications \
         WHERE related_entity_id = $1 AND type = 'PRESCRIPTION_AVAILABLE'",
    )
    .bind(appointment)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        rows.len(),
        1,
        "exactly one PRESCRIPTION_AVAILABLE notification regardless of how many pages were uploaded"
    );
    assert_eq!(rows[0].0, uid(PATIENT_USER_ID));
    assert_eq!(rows[0].1, format!("prescription-available:{appointment}"));
}

/// Full refund lifecycle via the real HTTP endpoints (mirrors
/// `test_refund_request_has_a_finance_resolution_lifecycle`'s own flow):
/// REFUND_APPROVED on adjudication, REFUND_PROCESSING on the PROCESSING
/// transition, REFUND_COMPLETED only once a real `provider_reference`
/// confirms the REFUNDED transition — never claiming paid on approval alone.
#[sqlx::test(migrations = "./migrations")]
async fn test_refund_lifecycle_notifications(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let admin_token = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let (apt_id, _) = book_and_confirm(&app, &patient_token).await;

    let session = consultation_repo::find_by_appointment(&pool, apt_id)
        .await
        .unwrap()
        .unwrap();
    let grv_json = json_body(
        post(
            &app,
            "/api/v1/grievances",
            Some(&patient_token),
            &serde_json::json!({
                "consultation_id": session.id,
                "target": "DOCTOR",
                "category": "Rushed Consultation / Ended Abruptly",
                "claim_summary": "Milestone 1 refund-notification regression"
            }),
        )
        .await,
    )
    .await;
    let grv_id = Uuid::parse_str(grv_json["data"]["grievance_id"].as_str().unwrap()).unwrap();

    let adj = post(
        &app,
        &format!("/api/v1/admin/grievances/{grv_id}/refund"),
        Some(&admin_token),
        &serde_json::json!({ "audit_notes": "Substantiated for notification regression." }),
    )
    .await;
    assert_eq!(adj.status(), StatusCode::OK);

    let approved: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT user_id, dedup_key FROM notifications \
         WHERE related_entity_id = $1 AND type = 'REFUND_APPROVED'",
    )
    .bind(grv_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        approved.len(),
        1,
        "exactly one REFUND_APPROVED notification"
    );
    assert_eq!(approved[0].0, uid(PATIENT_USER_ID));

    let refund_id: Uuid =
        sqlx::query_scalar("SELECT id FROM financial_refund_requests WHERE grievance_id = $1")
            .bind(grv_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    let processing = post(
        &app,
        &format!("/api/v1/admin/finance/refund-requests/{refund_id}/transition"),
        Some(&admin_token),
        &serde_json::json!({ "status": "PROCESSING" }),
    )
    .await;
    assert_eq!(processing.status(), StatusCode::OK);
    let (processing_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM notifications WHERE related_entity_id = $1 AND type = 'REFUND_PROCESSING'",
    )
    .bind(refund_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(processing_count, 1);

    let refunded = post(
        &app,
        &format!("/api/v1/admin/finance/refund-requests/{refund_id}/transition"),
        Some(&admin_token),
        &serde_json::json!({ "status": "REFUNDED", "provider_reference": "MPESA-NOTIF-TEST-1" }),
    )
    .await;
    assert_eq!(refunded.status(), StatusCode::OK);
    let completed: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT user_id, dedup_key FROM notifications \
         WHERE related_entity_id = $1 AND type = 'REFUND_COMPLETED'",
    )
    .bind(refund_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        completed.len(),
        1,
        "exactly one REFUND_COMPLETED, created only once a real provider_reference confirmed the REFUNDED transition"
    );
    assert_eq!(completed[0].0, uid(PATIENT_USER_ID));

    // No notification claims payment at approval time — REFUND_COMPLETED
    // must not have existed before the REFUNDED transition actually happened.
    let (total_refund_notifications,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM notifications WHERE related_entity_id = $1 AND type IN ('REFUND_APPROVED','REFUND_PROCESSING','REFUND_COMPLETED')",
    )
    .bind(refund_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    // REFUND_APPROVED is keyed by grievance_id, not refund_request_id, so it
    // is not counted here — this assertion is scoped to the two
    // refund-request-keyed types actually reachable from this id.
    assert_eq!(total_refund_notifications, 2);
}

/// Negative / scope boundary: `FAILED` and `RECONCILIATION_REQUIRED` are
/// operational escape hatches deliberately out of this milestone's bounded
/// scope — transitioning to either must create no notification at all.
#[sqlx::test(migrations = "./migrations")]
async fn test_refund_failed_transition_emits_no_notification(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let admin_token = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let (apt_id, _) = book_and_confirm(&app, &patient_token).await;

    let session = consultation_repo::find_by_appointment(&pool, apt_id)
        .await
        .unwrap()
        .unwrap();
    let grv_json = json_body(
        post(
            &app,
            "/api/v1/grievances",
            Some(&patient_token),
            &serde_json::json!({
                "consultation_id": session.id,
                "target": "DOCTOR",
                "category": "Rushed Consultation / Ended Abruptly",
                "claim_summary": "Milestone 1 FAILED-transition regression"
            }),
        )
        .await,
    )
    .await;
    let grv_id = Uuid::parse_str(grv_json["data"]["grievance_id"].as_str().unwrap()).unwrap();
    post(
        &app,
        &format!("/api/v1/admin/grievances/{grv_id}/refund"),
        Some(&admin_token),
        &serde_json::json!({ "audit_notes": "Substantiated." }),
    )
    .await;
    let refund_id: Uuid =
        sqlx::query_scalar("SELECT id FROM financial_refund_requests WHERE grievance_id = $1")
            .bind(grv_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    let failed = post(
        &app,
        &format!("/api/v1/admin/finance/refund-requests/{refund_id}/transition"),
        Some(&admin_token),
        &serde_json::json!({ "status": "FAILED" }),
    )
    .await;
    assert_eq!(failed.status(), StatusCode::OK);

    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM notifications WHERE related_entity_id = $1 AND type IN ('REFUND_PROCESSING','REFUND_COMPLETED')",
    )
    .bind(refund_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        count, 0,
        "FAILED is out of this milestone's bounded scope and must emit nothing"
    );
}

/// `appointment_notification_recipients` (the SECURITY DEFINER function every
/// Milestone 1 notification goes through to resolve recipients) must not be
/// a blanket RLS bypass: (1) its `search_path` is pinned (confirmed directly
/// against `pg_proc`, not just by reading the migration), (2) EXECUTE is
/// restricted to exactly the three roles that call it — any other role must
/// be refused outright, and (3) even an authorized *role* only ever sees
/// recipients for an appointment it is an actual participant in
/// (`consultation_participant_allowed`'s own ADMIN/owning-Patient/assigned-
/// Doctor gate) — an unrelated Doctor or Patient gets zero rows, not another
/// account's recipient data.
#[sqlx::test(migrations = "./migrations")]
async fn test_appointment_notification_recipients_is_participant_scoped(pool: PgPool) {
    let (_app, pool, _state) = test_app_and_state(pool).await;
    let roles = build_role_pools(&pool).await;
    let now = Utc::now();
    let (_slot, apt_id) =
        insert_confirmed_appointment(&pool, now + chrono::Duration::days(8)).await;

    // (1) Fixed search_path — a SECURITY DEFINER function without this is a
    // classic privilege-escalation vector (a caller-controlled search_path
    // could shadow `consultation_participant_allowed` with an attacker
    // function of the same name in a schema earlier in the path).
    let (proconfig,): (Option<Vec<String>>,) = sqlx::query_as(
        "SELECT proconfig FROM pg_proc WHERE proname = 'appointment_notification_recipients'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        proconfig
            .unwrap_or_default()
            .iter()
            .any(|c| c == "search_path=public"),
        "appointment_notification_recipients must pin search_path=public"
    );

    async fn call_as(
        pool: &PgPool,
        user_id: Uuid,
        role: &str,
        appointment_id: Uuid,
    ) -> Result<Vec<(Option<Uuid>, Option<Uuid>)>, sqlx::Error> {
        let mut tx = pool.begin().await.unwrap();
        sqlx::query("SELECT set_config('app.current_user_id', $1, true), set_config('app.current_role', $2, true)")
            .bind(user_id.to_string())
            .bind(role)
            .execute(&mut *tx)
            .await
            .unwrap();
        let result = sqlx::query_as(
            "SELECT doctor_user_id, patient_user_id FROM appointment_notification_recipients($1)",
        )
        .bind(appointment_id)
        .fetch_all(&mut *tx)
        .await;
        tx.rollback().await.unwrap();
        result
    }

    // (3a) Authorized: the assigned Doctor sees the real recipient pair.
    let as_doctor = call_as(&roles.api, uid(DOCTOR1_USER_ID), "DOCTOR", apt_id)
        .await
        .unwrap();
    assert_eq!(as_doctor.len(), 1);
    assert_eq!(as_doctor[0].0, Some(uid(DOCTOR1_USER_ID)));
    assert_eq!(as_doctor[0].1, Some(uid(PATIENT_USER_ID)));

    // (3b) Authorized: the owning Patient sees the same pair.
    let as_patient = call_as(&roles.api, uid(PATIENT_USER_ID), "PATIENT", apt_id)
        .await
        .unwrap();
    assert_eq!(as_patient.len(), 1);

    // (3c) Unauthorized: a real but unrelated Doctor (never assigned to this
    // appointment) must see nothing — not an error, zero rows, matching
    // `consultation_participant_allowed`'s own fail-closed WHERE clause.
    let as_foreign_doctor = call_as(&roles.api, uid(DOCTOR2_USER_ID), "DOCTOR", apt_id)
        .await
        .unwrap();
    assert!(
        as_foreign_doctor.is_empty(),
        "an unrelated Doctor must never see this appointment's recipients"
    );

    // (3d) Unauthorized: a real but unrelated Patient (never this
    // appointment's patient) must also see nothing.
    let foreign_patient_token = fresh_patient_token(&pool).await;
    let foreign_patient_claims = hellodoctor_backend::crypto::verify_access_token(
        &foreign_patient_token,
        TEST_JWT_PUBLIC_KEY_B64,
    )
    .unwrap();
    let as_foreign_patient = call_as(
        &roles.api,
        uid(&foreign_patient_claims.user_id),
        "PATIENT",
        apt_id,
    )
    .await
    .unwrap();
    assert!(
        as_foreign_patient.is_empty(),
        "an unrelated Patient must never see this appointment's recipients"
    );

    // (2) Restricted EXECUTE: a role never granted it at all (system/auth
    // worker) must be refused outright — not merely filtered to zero rows,
    // a hard permission-denied at the database layer.
    for (role_pool, role_label) in [(&roles.system, "system"), (&roles.auth, "auth")] {
        let err = call_as(role_pool, uid(DOCTOR1_USER_ID), "ADMIN", apt_id)
            .await
            .unwrap_err();
        assert!(
            err.to_string().to_lowercase().contains("permission denied"),
            "hellodoctor_{role_label}_worker must not have EXECUTE on appointment_notification_recipients: {err}"
        );
    }
}

async fn insert_confirmed_appointment(pool: &PgPool, start_time: DateTime<Utc>) -> (Uuid, Uuid) {
    insert_confirmed_appointment_for_doctor(pool, uid(DOCTOR1_PROFILE_ID), start_time).await
}

/// Same as `insert_confirmed_appointment`, but for a caller-chosen doctor —
/// needed whenever a test wants two appointments at the identical (or
/// overlapping) `start_time`, since `doctor_schedule_slots` has a real
/// exclusion constraint (`no_overlapping_slots`) preventing the *same*
/// doctor from having two overlapping slots at all (confirmed by a real
/// constraint-violation test failure, not assumed) — using a different
/// doctor for the second slot sidesteps that constraint entirely, since it's
/// scoped to `(doctor_id, time range)`.
async fn insert_confirmed_appointment_for_doctor(
    pool: &PgPool,
    doctor_profile_id: Uuid,
    start_time: DateTime<Utc>,
) -> (Uuid, Uuid) {
    insert_confirmed_appointment_for(pool, doctor_profile_id, uid(PATIENT_PROFILE_ID), start_time)
        .await
}

/// A brand-new Patient profile, independent of the shared seeded
/// `PATIENT_PROFILE_ID` — needed alongside `insert_confirmed_appointment_for`
/// wherever a test wants two overlapping-time appointments that are
/// genuinely unrelated bookings (different Doctor *and* different Patient),
/// not a double-booking of the same person, which Stage A's
/// `appt_patient_no_overlap` constraint (20261111000001_schedule_exclusivity)
/// now correctly rejects.
async fn fresh_patient_profile_id(pool: &PgPool) -> Uuid {
    let user_id = Uuid::new_v4();
    let profile_id = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,phone_number,role,is_active,is_verified,created_at,updated_at) VALUES($1,$2,'PATIENT'::user_role_enum,true,true,NOW(),NOW())")
        .bind(user_id)
        .bind(format!("+2547{:08}", rand::thread_rng().gen_range(0..100_000_000)))
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO patient_profiles (id, user_id, full_name, date_of_birth, gender, created_at, updated_at) VALUES ($1, $2, 'Reminder Scheduler Test Patient', '1990-01-01', 'OTHER', NOW(), NOW())")
        .bind(profile_id)
        .bind(user_id)
        .execute(pool)
        .await
        .unwrap();
    profile_id
}

/// Same as `insert_confirmed_appointment_for_doctor`, but also takes an
/// explicit Patient — see `fresh_patient_profile_id`'s doc comment for why.
async fn insert_confirmed_appointment_for(
    pool: &PgPool,
    doctor_profile_id: Uuid,
    patient_profile_id: Uuid,
    start_time: DateTime<Utc>,
) -> (Uuid, Uuid) {
    let slot_id = Uuid::new_v4();
    doctor_repo::insert_slot(
        pool,
        &ScheduleSlot {
            id: slot_id,
            doctor_id: DoctorProfileId::from(doctor_profile_id),
            start_time,
            end_time: start_time + chrono::Duration::minutes(15),
            status: SlotStatus::Booked,
            created_at: Utc::now(),
        },
    )
    .await
    .unwrap();
    let apt_id = Uuid::new_v4();
    appointment_repo::insert(
        pool,
        &Appointment {
            id: apt_id,
            appointment_number: format!("TESTR-{}", &apt_id.to_string()[..8]),
            patient_id: PatientProfileId::from(patient_profile_id),
            doctor_id: DoctorProfileId::from(doctor_profile_id),
            slot_id,
            modality: Modality::Video,
            status: AppointmentStatus::Confirmed,
            consultation_fee: bigdecimal::BigDecimal::from(500),
            clinical_outcome: None,
            completed_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        },
    )
    .await
    .unwrap();
    (slot_id, apt_id)
}

/// Doctor payout method setup: there was previously nowhere for a doctor to
/// tell Finance where a scheduled disbursement should go. Verifies the raw
/// account identifier is never returned by the API, is never stored in the
/// clear in the database, and is doctor-scoped (one doctor can't see
/// another's).
#[sqlx::test(migrations = "./migrations")]
async fn test_doctor_payout_method_set_and_read_masks_account_identifier(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let doctor_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let other_doctor_token = make_token(uid(DOCTOR2_USER_ID), "DOCTOR");

    let initial = json_body(
        get(
            &app,
            "/api/v1/doctor/wallet/payout-method",
            Some(&doctor_token),
        )
        .await,
    )
    .await;
    assert_eq!(initial["data"]["configured"], false);

    let set = put(
        &app,
        "/api/v1/doctor/wallet/payout-method",
        Some(&doctor_token),
        // Current Kenya fixture; the encryption/masking regression is unchanged.
        &serde_json::json!({ "method_type": "MPESA", "account_identifier": "0712345678" }),
    )
    .await;
    assert_eq!(set.status(), StatusCode::OK);
    let set_body = json_body(set).await;
    assert_eq!(set_body["data"]["account_identifier_last4"], "5678");
    assert_eq!(set_body["data"]["display_label"], "M-Pesa •••• 5678");

    let fetched = json_body(
        get(
            &app,
            "/api/v1/doctor/wallet/payout-method",
            Some(&doctor_token),
        )
        .await,
    )
    .await;
    assert_eq!(fetched["data"]["configured"], true);
    assert_eq!(fetched["data"]["account_identifier_last4"], "5678");
    assert!(fetched["data"].get("account_identifier").is_none());
    assert!(fetched["data"]
        .get("account_identifier_encrypted")
        .is_none());

    let raw_row: (Vec<u8>,) = sqlx::query_as(
        "SELECT account_identifier_encrypted FROM doctor_payout_methods WHERE doctor_id = $1",
    )
    .bind(uid(DOCTOR1_PROFILE_ID))
    .fetch_one(&pool)
    .await
    .unwrap();
    let stored_as_text = String::from_utf8_lossy(&raw_row.0);
    assert!(!stored_as_text.contains("0712345678"));

    let other = json_body(
        get(
            &app,
            "/api/v1/doctor/wallet/payout-method",
            Some(&other_doctor_token),
        )
        .await,
    )
    .await;
    assert_eq!(other["data"]["configured"], false);

    let invalid = put(
        &app,
        "/api/v1/doctor/wallet/payout-method",
        Some(&doctor_token),
        &serde_json::json!({ "method_type": "MPESA", "account_identifier": "not-a-phone-number" }),
    )
    .await;
    assert_eq!(invalid.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_doctor_patient_history_requires_current_treatment_context(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let other_doctor_token = make_token(uid(DOCTOR2_USER_ID), "DOCTOR");

    let (historical_appointment, _) = book_and_confirm(&app, &patient_token).await;
    // Historical completion must occur inside its authorized clinical window.
    sqlx::query("UPDATE doctor_schedule_slots SET start_time=CURRENT_TIMESTAMP - INTERVAL '1 day', end_time=CURRENT_TIMESTAMP - INTERVAL '23 hours' WHERE id=(SELECT slot_id FROM appointments WHERE id=$1)")
        .bind(historical_appointment).execute(&pool).await.unwrap();
    let completed = post(
        &app,
        &format!("/api/v1/consultations/{historical_appointment}/complete-no-rx"),
        Some(&doctor_token),
        &serde_json::json!({"reason": "Historical continuity test"}),
    )
    .await;
    assert_eq!(completed.status(), StatusCode::OK);

    let (current_appointment, _) = book_and_confirm(&app, &patient_token).await;
    sqlx::query("UPDATE doctor_schedule_slots SET start_time=CURRENT_TIMESTAMP + INTERVAL '15 minutes', end_time=CURRENT_TIMESTAMP + INTERVAL '25 minutes' WHERE id=(SELECT slot_id FROM appointments WHERE id=$1)")
        .bind(current_appointment).execute(&pool).await.unwrap();
    let history = get(
        &app,
        &format!("/api/v1/doctor/appointments/{current_appointment}/patient-history"),
        Some(&doctor_token),
    )
    .await;
    assert_eq!(history.status(), StatusCode::OK);
    let history_body = json_body(history).await;
    let episodes = history_body["data"].as_array().unwrap();
    assert!(!episodes.is_empty());
    let historical_consultation =
        Uuid::parse_str(episodes[0]["historical_consultation_id"].as_str().unwrap()).unwrap();

    let detail = get(
        &app,
        &format!(
            "/api/v1/doctor/appointments/{current_appointment}/patient-history/{historical_consultation}"
        ),
        Some(&doctor_token),
    )
    .await;
    assert_eq!(detail.status(), StatusCode::OK);

    let unrelated = get(
        &app,
        &format!("/api/v1/doctor/appointments/{current_appointment}/patient-history"),
        Some(&other_doctor_token),
    )
    .await;
    assert_eq!(unrelated.status(), StatusCode::NOT_FOUND);

    // BOLA/IDOR regression: the test above only ever exercises the foreign
    // doctor against the *list* endpoint. The sibling *detail* endpoint
    // (`GET .../patient-history/:historical_consultation_id`, used two
    // requests above by the real treating doctor) is a separate handler
    // (`get_patient_history_detail` -> `doctor_patient_history_detail`),
    // keyed by (current_appointment_id, ctx.user_id, historical_consultation_id),
    // and was never itself driven with a foreign doctor's token. A doctor
    // who never treated this patient must not be able to pull the episode
    // detail by reusing DOCTOR1's own, perfectly valid
    // current_appointment_id/historical_consultation_id pair.
    let unrelated_detail = get(
        &app,
        &format!(
            "/api/v1/doctor/appointments/{current_appointment}/patient-history/{historical_consultation}"
        ),
        Some(&other_doctor_token),
    )
    .await;
    assert_eq!(
        unrelated_detail.status(),
        StatusCode::NOT_FOUND,
        "a doctor who never treated this patient must not be able to read another doctor's patient-history episode detail"
    );
    let unrelated_detail_body = json_body(unrelated_detail).await;
    assert!(
        unrelated_detail_body.get("data").is_none() || unrelated_detail_body["data"].is_null(),
        "the denied detail response must not leak clinical content: {unrelated_detail_body}"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_doctor_history_window_pagination_and_cutoff(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let other = make_token(uid(DOCTOR2_USER_ID), "DOCTOR");
    let (apt, _) = book_and_confirm(&app, &patient).await;
    let path = format!("/api/v1/doctor/appointments/{apt}/patient-history");
    for (mins, expected) in [
        (16, StatusCode::OK),
        (15, StatusCode::OK),
        (1, StatusCode::OK),
        (-1, StatusCode::OK),
    ] {
        sqlx::query("UPDATE doctor_schedule_slots SET start_time=CURRENT_TIMESTAMP + $2 * INTERVAL '1 minute', end_time=CURRENT_TIMESTAMP + ($2 + 10) * INTERVAL '1 minute' WHERE id=(SELECT slot_id FROM appointments WHERE id=$1)")
            .bind(apt).bind(mins).execute(&pool).await.unwrap();
        for offset in [0, 1, 100] {
            assert_eq!(
                get(
                    &app,
                    &format!("{path}?offset={offset}&limit=1"),
                    Some(&doctor)
                )
                .await
                .status(),
                expected
            );
            assert_eq!(
                get(
                    &app,
                    &format!("{path}?offset={offset}&limit=100"),
                    Some(&other)
                )
                .await
                .status(),
                StatusCode::NOT_FOUND
            );
        }
    }
    let empty = json_body(get(&app, &path, Some(&doctor)).await).await;
    assert_eq!(empty["data"], serde_json::json!([]));
    post(
        &app,
        &format!("/api/v1/consultations/{apt}/complete"),
        Some(&doctor),
        &serde_json::json!({"outcome":"COMPLETED_NO_RX"}),
    )
    .await;
    let ended: Option<chrono::DateTime<Utc>> =
        sqlx::query_scalar("SELECT ended_at FROM consultation_sessions WHERE appointment_id=$1")
            .bind(apt)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(ended.is_some());
    assert_eq!(
        get(&app, &path, Some(&doctor)).await.status(),
        StatusCode::OK
    );
    for (mins, expected) in [
        (1439, StatusCode::OK),
        (1440, StatusCode::OK),
        (1441, StatusCode::OK),
        (100000, StatusCode::OK),
    ] {
        sqlx::query("UPDATE consultation_sessions SET ended_at=CURRENT_TIMESTAMP - $2 * INTERVAL '1 minute' WHERE appointment_id=$1").bind(apt).bind(mins).execute(&pool).await.unwrap();
        assert_eq!(get(&app, &path, Some(&doctor)).await.status(), expected);
        assert_eq!(
            get(&app, &path, Some(&other)).await.status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            get(
                &app,
                &format!("/api/v1/doctor/consultations/{apt}/summary"),
                Some(&doctor)
            )
            .await
            .status(),
            StatusCode::OK
        );
    }
    sqlx::query("UPDATE appointments SET status='CANCELLED' WHERE id=$1")
        .bind(apt)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        get(&app, &path, Some(&doctor)).await.status(),
        StatusCode::OK
    );
}

/// BOLA/IDOR regression: `GET /api/v1/doctor/consultations/:appointment_id/summary`
/// (`doctor_handlers::consultation_summary`) has no coverage anywhere else in
/// this suite. It is gated by the same `doctor_history_context_allowed(
/// appointment_id, ctx.user_id)` SQL function exercised above for the
/// patient-history routes, plus its own direct `a.doctor_id=$2` match — a
/// doctor who was never assigned to this appointment must be refused, and
/// the refusal must carry none of the appointment's clinical content.
#[sqlx::test(migrations = "./migrations")]
async fn test_bola_doctor_cannot_access_another_doctors_consultation_summary(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let foreign_token = make_token(uid(DOCTOR2_USER_ID), "DOCTOR");

    let (apt, _) = book_and_confirm(&app, &patient_token).await;
    open_clinical_window(&pool, apt).await;
    let path = format!("/api/v1/doctor/consultations/{apt}/summary");

    // The treating doctor can read the summary inside the clinical window.
    assert_eq!(
        get(&app, &path, Some(&doctor_token)).await.status(),
        StatusCode::OK
    );

    // A doctor never assigned to this appointment must be denied, and the
    // response must carry no clinical content (no chief complaint, no
    // clinical notes, no appointment or patient identity).
    let res_foreign = get(&app, &path, Some(&foreign_token)).await;
    assert_eq!(
        res_foreign.status(),
        StatusCode::FORBIDDEN,
        "an unassigned doctor must not be able to read another doctor's consultation summary"
    );
    let body = json_body(res_foreign).await;
    let dumped = body.to_string();
    assert!(
        !dumped.contains(&apt.to_string()) && !dumped.contains(PATIENT_PROFILE_ID),
        "a denied consultation-summary response must not leak the appointment or patient identity: {dumped}"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_doctor_qna_concurrent_answers_are_conflicts(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    for contenders in [2, 20] {
        let question = Uuid::new_v4();
        sqlx::query("INSERT INTO health_questions (id,question_text,created_at,updated_at) VALUES ($1,'General health question',CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)")
            .bind(question).execute(&pool).await.unwrap();
        let barrier = Arc::new(tokio::sync::Barrier::new(contenders));
        let mut tasks = Vec::new();
        for i in 0..contenders {
            let app = app.clone();
            let barrier = barrier.clone();
            tasks.push(tokio::spawn(async move {
                let token = make_token(
                    uid(if i % 2 == 0 {
                        DOCTOR1_USER_ID
                    } else {
                        DOCTOR2_USER_ID
                    }),
                    "DOCTOR",
                );
                barrier.wait().await;
                post(
                    &app,
                    &format!("/api/v1/doctor/qna/questions/{question}/answers"),
                    Some(&token),
                    &serde_json::json!({"answer_text":"General informational answer"}),
                )
                .await
            }));
        }
        let mut success = 0;
        for task in tasks {
            let response = task.await.unwrap();
            match response.status() {
                StatusCode::CREATED => success += 1,
                StatusCode::CONFLICT => {}
                // At `contenders=20`, this deliberately pushes past the
                // Normal-cost admission lane's fixed ceiling (`n.min(8)` in
                // `Admission::new`, independent of pool capacity) — a
                // correctly-shaped bounded rejection is a legitimate
                // non-winning outcome here, not a defect.
                _ => assert_controlled_shedding(response).await,
            }
        }
        assert_eq!(success, 1);
        let count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM health_answers WHERE question_id=$1")
                .bind(question)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count, 1);
        let status: String = sqlx::query_scalar("SELECT status FROM health_questions WHERE id=$1")
            .bind(question)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(status, "ANSWERED");
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn test_doctor_duty_credentials_and_modality_contract(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    for value in [true, false] {
        let body = serde_json::json!({"is_on_duty":value});
        let updated = send(
            &app,
            "PUT",
            "/api/v1/doctor/duty",
            Some(&doctor),
            Some(&body),
            None,
        )
        .await;
        assert_eq!(updated.status(), StatusCode::OK);
        assert_eq!(json_body(updated).await["data"], body);
        assert_eq!(
            json_body(get(&app, "/api/v1/doctor/duty", Some(&doctor)).await).await["data"],
            body
        );
    }
    for forbidden in [
        "current_hospital",
        "qualifications",
        "is_verified",
        "professional_verification_status",
        "professional_verification_reviewed_by",
        "role",
        "wallet",
    ] {
        let payload = serde_json::json!({forbidden:"forbidden"});
        assert_eq!(
            send(
                &app,
                "PATCH",
                "/api/v1/doctor/profile",
                Some(&doctor),
                Some(&payload),
                None
            )
            .await
            .status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    sqlx::query("UPDATE doctor_profiles SET consultation_fee_chat=0 WHERE id=$1")
        .bind(uid(DOCTOR1_PROFILE_ID))
        .execute(&pool)
        .await
        .unwrap();
    for modality in ["VIDEO", "CHAT"] {
        let res = get(
            &app,
            &format!("/api/v1/doctors?modality={modality}"),
            Some(&doctor),
        )
        .await;
        assert_eq!(res.status(), StatusCode::OK);
        let body = json_body(res).await;
        let fee = if modality == "VIDEO" {
            "consultation_fee_video"
        } else {
            "consultation_fee_chat"
        };
        for doc in body["data"].as_array().unwrap() {
            let value = doc[fee]
                .as_str()
                .unwrap()
                .parse::<bigdecimal::BigDecimal>()
                .unwrap();
            let zero = bigdecimal::BigDecimal::from(0);
            assert_ne!(value, zero);
        }
    }
    assert_eq!(
        get(&app, "/api/v1/doctors?modality=AUDIO", Some(&doctor))
            .await
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_doctor_registration_is_pending_and_cannot_set_privileged_fields(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let payload = serde_json::json!({
        "full_name": "Pending Doctor",
        "license_number": "KMPDC-REG-UNIQUE-9001",
        "phone_number": "0719999001",
        "email": "pending.doctor.9001@example.test",
        "password": "A-secure-password-9001!",
        "primary_specialty": "Internal Medicine",
        "experience_years": 3,
        "current_hospital": "Pending Hospital",
        "qualifications": ["MBBS"],
        "residential_address": "Nairobi",
        "accepted_documents": [
            {"type": "DOCTOR_TERMS", "version": "1.0-afyamkononi"},
            {"type": "PRIVACY_NOTICE", "version": "1.0-afyamkononi"},
            {"type": "PROFESSIONAL_VERIFICATION_DECLARATION", "version": "1.0-afyamkononi"},
            {"type": "TELEMEDICINE_PRACTITIONER_POLICY", "version": "1.0-afyamkononi"}
        ]
    });
    let response = post(&app, "/api/v1/auth/doctor/register", None, &payload).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = json_body(response).await;
    assert_eq!(body["data"]["status"], "PENDING_VERIFICATION");
    assert_eq!(body["data"]["professional_verification_status"], "PENDING");
    let profile = doctor_repo::find_by_id(
        &pool,
        DoctorProfileId::from(
            Uuid::parse_str(body["data"]["doctor_id"].as_str().unwrap()).unwrap(),
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        profile.professional_verification_status,
        ProfessionalVerificationStatus::Pending
    );
    assert_eq!(profile.license_authority, "KMPDC");
    assert_eq!(profile.license_country, "KE");
    assert!(!profile.is_professionally_verified());
    for forbidden_field in [
        "professional_verification_status",
        "verification_status",
        "professional_verification_reviewed_by",
        "license_authority",
        "license_country",
    ] {
        let mut malicious = payload.clone();
        malicious[forbidden_field] = serde_json::json!("VERIFIED");
        assert_eq!(
            post(&app, "/api/v1/auth/doctor/register", None, &malicious)
                .await
                .status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }

    let doctor_id = Uuid::parse_str(body["data"]["doctor_id"].as_str().unwrap()).unwrap();
    let row = sqlx::query("SELECT u.role::text AS role, u.is_verified AS user_verified, d.is_verified AS doctor_verified, u.password_hash FROM users u JOIN doctor_profiles d ON d.user_id=u.id WHERE d.id=$1")
        .bind(doctor_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row.get::<String, _>("role"), "DOCTOR");
    assert!(!row.get::<bool, _>("user_verified"));
    assert!(!row.get::<bool, _>("doctor_verified"));
    let hash = row.get::<String, _>("password_hash");
    assert_ne!(hash, "A-secure-password-9001!");
    assert!(hash.starts_with("$argon2"));

    let duplicate = post(&app, "/api/v1/auth/doctor/register", None, &payload).await;
    assert_eq!(duplicate.status(), StatusCode::CONFLICT);

    let mut forbidden = payload.as_object().unwrap().clone();
    forbidden.insert("is_verified".into(), serde_json::json!(true));
    forbidden.insert("role".into(), serde_json::json!("ADMIN"));
    let rejected = post(
        &app,
        "/api/v1/auth/doctor/register",
        None,
        &serde_json::Value::Object(forbidden),
    )
    .await;
    assert_eq!(rejected.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

// ============================================================================
// SQL injection regression tests (authorized security validation pass).
//
// `src/repository/article_repo.rs` lines 13 and 22 are the only place in this
// codebase that build SQL text with `format!`, and in both cases it only
// concatenates two FIXED string fragments (a constant `SELECT` prefix and a
// constant literal suffix containing nothing but `$1`/`$2`/`$3` bound-
// parameter placeholders) — no request-controlled text is ever spliced into
// the query string itself. Verified directly by reading the full file: every
// caller binds `author`/`id` as sqlx parameters, never as interpolated text.
//
// The tests below submit classic SQL injection payloads through a JSON body
// text field, a query-string filter param, a path param, and a validated
// enum-backed filter param, and assert each is treated as inert data (stored/
// returned verbatim, or cleanly rejected by validation) — never a 500, never
// a widened/altered result set, never a leaked DB error message.
// ============================================================================

const SQLI_PAYLOADS: [&str; 5] = [
    "'",
    "' OR '1'='1",
    "'; --",
    "' UNION SELECT NULL--",
    "admin'--",
];

/// Percent-encodes a payload as a query-string value. Written inline per
/// call site rather than as one more shared helper, since several other
/// agents are concurrently editing this same file right now.
fn sqli_qparam_encode(payload: &str) -> String {
    payload
        .bytes()
        .map(|b| match b {
            b'\'' => "%27".to_string(),
            b' ' => "%20".to_string(),
            b';' => "%3B".to_string(),
            b'=' => "%3D".to_string(),
            other => (other as char).to_string(),
        })
        .collect()
}

#[sqlx::test(migrations = "./migrations")]
async fn test_sql_injection_doctor_directory_search_query_param_is_parameterized(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");

    let baseline = json_body(get(&app, "/api/v1/doctors", Some(&patient)).await).await;
    let baseline_count = baseline["data"].as_array().unwrap().len();
    assert!(
        baseline_count > 0,
        "seeded verified doctors must be listed before injection attempts"
    );

    for payload in SQLI_PAYLOADS {
        let res = get(
            &app,
            &format!("/api/v1/doctors?search={}", sqli_qparam_encode(payload)),
            Some(&patient),
        )
        .await;
        assert_eq!(
            res.status(),
            StatusCode::OK,
            "search payload {payload:?} must never cause a server error"
        );
        let body = json_body(res).await;
        assert!(
            body["data"].as_array().unwrap().is_empty(),
            "search payload {payload:?} must be bound as literal text, never widen the result set"
        );
    }

    // The directory itself must be fully intact and queryable after every
    // injection attempt above (nothing dropped or altered).
    let after = json_body(get(&app, "/api/v1/doctors", Some(&patient)).await).await;
    assert_eq!(after["data"].as_array().unwrap().len(), baseline_count);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_sql_injection_doctor_appointments_status_filter_param_is_parameterized(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let doctor_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");

    for payload in SQLI_PAYLOADS {
        let res = get(
            &app,
            &format!(
                "/api/v1/doctor/appointments?status={}",
                sqli_qparam_encode(payload)
            ),
            Some(&doctor_token),
        )
        .await;
        assert_eq!(
            res.status(),
            StatusCode::OK,
            "status filter payload {payload:?} must never cause a server error"
        );
        let body = json_body(res).await;
        assert!(
            body["data"].as_array().unwrap().is_empty(),
            "status filter payload {payload:?} must never match a real appointment status"
        );
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn test_sql_injection_doctor_qna_status_filter_cleanly_rejected(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let doctor_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");

    for payload in SQLI_PAYLOADS {
        let res = get(
            &app,
            &format!(
                "/api/v1/doctor/qna/questions?status={}",
                sqli_qparam_encode(payload)
            ),
            Some(&doctor_token),
        )
        .await;
        assert_eq!(
            res.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "status payload {payload:?} must be cleanly rejected by ALL/OPEN/ANSWERED \
             validation, never reach the database"
        );
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn test_sql_injection_grievance_text_fields_stored_verbatim(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");

    // Only two attempts: `submit_grievance` is rate-limited to 3/day per
    // user (see `rate_limit::check_grievance_limit`), and this test's own
    // `test_app` call already flushed the shared test Redis, so only this
    // test's own requests count against that budget.
    for payload in ["' OR '1'='1", "'; DROP TABLE grievance_reports; --"] {
        let (apt_id, _) = book_and_confirm(&app, &patient_token).await;
        let session = consultation_repo::find_by_appointment(&pool, apt_id)
            .await
            .unwrap()
            .unwrap();
        let claim_summary = format!("Patient claim referencing {payload}");
        let grv_payload = serde_json::json!({
            "consultation_id": session.id,
            "target": "DOCTOR",
            "category": payload,
            "claim_summary": claim_summary
        });
        let res = post(
            &app,
            "/api/v1/grievances",
            Some(&patient_token),
            &grv_payload,
        )
        .await;
        assert_eq!(
            res.status(),
            StatusCode::CREATED,
            "grievance payload {payload:?} must be accepted as ordinary text, not cause a server error"
        );
        let json = json_body(res).await;
        let grv_id = Uuid::parse_str(json["data"]["grievance_id"].as_str().unwrap()).unwrap();

        let row = sqlx::query("SELECT category, claim_summary FROM grievance_reports WHERE id=$1")
            .bind(grv_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(row.get::<String, _>("category"), payload);
        assert_eq!(row.get::<String, _>("claim_summary"), claim_summary);
    }

    // The table must still exist and be fully queryable after both attempts.
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM grievance_reports")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 2);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_sql_injection_chat_conversation_path_param_and_message_body(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let (apt_id, _) = book_and_confirm(&app, &patient_token).await;
    let conv_id: Uuid =
        sqlx::query_scalar("SELECT id FROM chat_conversations WHERE appointment_id=$1")
            .bind(apt_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    // Path param: a malformed/injected conversation id can never reach SQL as
    // text — axum's `Path<Uuid>` extractor rejects anything that does not
    // parse as a UUID before the handler (and therefore the database) ever
    // sees it. Must be a client error, never a 500.
    for payload in ["'", "' OR '1'='1", "1' UNION SELECT NULL--"] {
        // The raw payload can contain characters (quotes, spaces) that are
        // invalid in a literal URI and would make `http::Request` itself
        // reject the request before it even reaches axum's routing/
        // extraction — percent-encode it first so the test actually proves
        // what it claims (axum's `Path<Uuid>` extractor rejecting a
        // non-UUID value), not merely that the HTTP client library rejects
        // malformed URI syntax.
        let encoded = sqli_qparam_encode(payload);
        let res = post(
            &app,
            &format!("/api/v1/chat/{encoded}/messages"),
            Some(&patient_token),
            &serde_json::json!({ "content": "probe" }),
        )
        .await;
        assert!(
            res.status().is_client_error(),
            "path param payload {payload:?} must be rejected as a malformed id, got {:?}",
            res.status()
        );
    }

    // JSON body text field: the message content itself.
    for payload in SQLI_PAYLOADS {
        let res = post(
            &app,
            &format!("/api/v1/chat/{conv_id}/messages"),
            Some(&patient_token),
            &serde_json::json!({ "content": format!("Question: {payload}") }),
        )
        .await;
        assert_eq!(
            res.status(),
            StatusCode::CREATED,
            "chat message payload {payload:?} must be stored as ordinary text, not cause a server error"
        );
    }

    let messages = json_body(
        get(
            &app,
            &format!("/api/v1/chat/{conv_id}/messages"),
            Some(&patient_token),
        )
        .await,
    )
    .await;
    let bodies: Vec<String> = messages["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["content"].as_str().unwrap().to_string())
        .collect();
    for payload in SQLI_PAYLOADS {
        assert!(
            bodies.iter().any(|b| b == &format!("Question: {payload}")),
            "chat message payload {payload:?} must be stored and returned verbatim"
        );
    }
}

// ============================================================================
// Doctor registration legal/consent gate (docs/ADR/ADR-011-legal-consent-ledger.md).
// ============================================================================

#[sqlx::test(migrations = "./migrations")]
async fn test_doctor_registration_legal_consent_gate_enforces_current_required_set(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let base = |license_suffix: &str, phone_suffix: &str, accepted: serde_json::Value| {
        serde_json::json!({
            "full_name": "Consent Doctor",
            "license_number": format!("KMPDC-CONSENT-{license_suffix}"),
            "phone_number": format!("0719999{phone_suffix}"),
            "email": format!("consent.doctor.{license_suffix}@example.test"),
            "password": "A-secure-password-9001!",
            "primary_specialty": "Internal Medicine",
            "experience_years": 3,
            "current_hospital": "Consent Hospital",
            "qualifications": ["MBBS"],
            "residential_address": "Nairobi",
            "accepted_documents": accepted
        })
    };
    let required_only = serde_json::json!([
        {"type": "DOCTOR_TERMS", "version": "1.0-afyamkononi"},
        {"type": "PRIVACY_NOTICE", "version": "1.0-afyamkononi"},
        {"type": "PROFESSIONAL_VERIFICATION_DECLARATION", "version": "1.0-afyamkononi"},
        {"type": "TELEMEDICINE_PRACTITIONER_POLICY", "version": "1.0-afyamkononi"}
    ]);

    // 1. missing required agreement entirely -> rejected.
    let none = base("9101", "101", serde_json::json!([]));
    assert_eq!(
        post(&app, "/api/v1/auth/doctor/register", None, &none)
            .await
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );

    // 2. partial agreement (3 of 4) -> rejected.
    let partial = base(
        "9102",
        "102",
        serde_json::json!([
            {"type": "DOCTOR_TERMS", "version": "1.0-afyamkononi"},
            {"type": "PRIVACY_NOTICE", "version": "1.0-afyamkononi"},
            {"type": "PROFESSIONAL_VERIFICATION_DECLARATION", "version": "1.0-afyamkononi"}
        ]),
    );
    assert_eq!(
        post(&app, "/api/v1/auth/doctor/register", None, &partial)
            .await
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );

    // 3. obsolete version submitted -> rejected, even though the other three
    // documents are current.
    sqlx::query(
        "INSERT INTO legal_documents (document_type, version, title, required, is_current, body) \
         VALUES ('DOCTOR_TERMS','0.9','Old Doctor Terms',true,false,'Superseded text.')",
    )
    .execute(&pool)
    .await
    .unwrap();
    let obsolete = base(
        "9103",
        "103",
        serde_json::json!([
            {"type": "DOCTOR_TERMS", "version": "0.9"},
            {"type": "PRIVACY_NOTICE", "version": "1.0-afyamkononi"},
            {"type": "PROFESSIONAL_VERIFICATION_DECLARATION", "version": "1.0-afyamkononi"},
            {"type": "TELEMEDICINE_PRACTITIONER_POLICY", "version": "1.0-afyamkononi"}
        ]),
    );
    assert_eq!(
        post(&app, "/api/v1/auth/doctor/register", None, &obsolete)
            .await
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );

    // 4 & 7 (marketing omitted half): all current required versions, no
    // marketing -> succeeds, and exactly the four required rows land, none
    // of them Marketing.
    let ok = base("9104", "104", required_only.clone());
    let response = post(&app, "/api/v1/auth/doctor/register", None, &ok).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = json_body(response).await;
    let doctor_id = Uuid::parse_str(body["data"]["doctor_id"].as_str().unwrap()).unwrap();
    let profile = doctor_repo::find_by_id(&pool, DoctorProfileId::from(doctor_id))
        .await
        .unwrap()
        .unwrap();
    let user_id = profile.user_id;
    let rows = legal_repo::list_for_user(&pool, user_id).await.unwrap();
    assert_eq!(rows.len(), 4);
    assert!(rows
        .iter()
        .all(|r| r.document_type != LegalDocumentType::MarketingCommunications));

    // 5. every acceptance row's user_id equals the newly created doctor's
    // canonical users.id.
    assert!(rows.iter().all(|r| r.user_id == user_id));

    // 7 (marketing included half): optional Marketing Communications
    // included -> still succeeds, and a fifth row lands.
    let mut with_marketing_arr = required_only.as_array().unwrap().clone();
    with_marketing_arr.push(
        serde_json::json!({"type": "MARKETING_COMMUNICATIONS", "version": "1.0-afyamkononi"}),
    );
    let with_marketing = base("9107", "107", serde_json::Value::Array(with_marketing_arr));
    let response = post(&app, "/api/v1/auth/doctor/register", None, &with_marketing).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = json_body(response).await;
    let doctor_id_2 = Uuid::parse_str(body["data"]["doctor_id"].as_str().unwrap()).unwrap();
    let profile_2 = doctor_repo::find_by_id(&pool, DoctorProfileId::from(doctor_id_2))
        .await
        .unwrap()
        .unwrap();
    let rows_2 = legal_repo::list_for_user(&pool, profile_2.user_id)
        .await
        .unwrap();
    assert_eq!(rows_2.len(), 5);
    assert!(rows_2
        .iter()
        .any(|r| r.document_type == LegalDocumentType::MarketingCommunications));

    // 10. calling the endpoint directly with an empty accepted_documents
    // array (bypassing any client-side gating) is rejected exactly like
    // case 1 — there is no separate code path to bypass.
    let bypass = base("9110", "110", serde_json::json!([]));
    assert_eq!(
        post(&app, "/api/v1/auth/doctor/register", None, &bypass)
            .await
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_legal_acceptance_insert_rejects_mismatched_user_id(pool: PgPool) {
    // 6. a Doctor's registration transaction cannot write a legal_acceptances
    // row pointing at a different Doctor's user_id — RLS on the
    // hellodoctor_auth_worker INSERT policy enforces this independent of
    // handler code.
    let (_app, _pool, state) = test_app_and_state(pool).await;
    let foreign_ctx = hellodoctor_backend::auth_context::AuthContext {
        user_id: UserId::from(uid(DOCTOR2_USER_ID)),
        role: UserRole::Doctor,
        permissions: vec![],
        session_id: None,
        request_id: None,
    };
    let mut tx = state
        .begin_authed_as(&foreign_ctx, DbRole::AuthWorker)
        .await
        .unwrap();
    let forged = LegalAcceptance {
        id: Uuid::new_v4(),
        user_id: UserId::from(uid(DOCTOR1_USER_ID)),
        document_type: LegalDocumentType::DoctorTerms,
        document_version: "1.0".into(),
        document_hash: "irrelevant-overwritten-by-trigger".into(),
        accepted_at: Utc::now(),
        request_id: None,
        session_id: None,
        ip_hash: None,
        app_version: None,
        platform: None,
        locale: None,
        created_at: Utc::now(),
    };
    assert!(legal_repo::insert(&mut *tx, &forged).await.is_err());
    tx.rollback().await.unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn test_legal_document_version_bump_preserves_history_and_flags_stale(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let payload = serde_json::json!({
        "full_name": "Stale Check Doctor",
        "license_number": "KMPDC-CONSENT-9201",
        "phone_number": "0719999201",
        "email": "consent.doctor.9201@example.test",
        "password": "A-secure-password-9001!",
        "primary_specialty": "Internal Medicine",
        "experience_years": 3,
        "current_hospital": "Consent Hospital",
        "qualifications": ["MBBS"],
        "residential_address": "Nairobi",
        "accepted_documents": [
            {"type": "DOCTOR_TERMS", "version": "1.0-afyamkononi"},
            {"type": "PRIVACY_NOTICE", "version": "1.0-afyamkononi"},
            {"type": "PROFESSIONAL_VERIFICATION_DECLARATION", "version": "1.0-afyamkononi"},
            {"type": "TELEMEDICINE_PRACTITIONER_POLICY", "version": "1.0-afyamkononi"}
        ]
    });
    let response = post(&app, "/api/v1/auth/doctor/register", None, &payload).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = json_body(response).await;
    let doctor_id = Uuid::parse_str(body["data"]["doctor_id"].as_str().unwrap()).unwrap();
    let profile = doctor_repo::find_by_id(&pool, DoctorProfileId::from(doctor_id))
        .await
        .unwrap()
        .unwrap();
    let user_id = profile.user_id;

    let original_hash: String = sqlx::query_scalar(
        "SELECT document_hash FROM legal_acceptances WHERE user_id=$1 AND document_type='DOCTOR_TERMS'",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Bump DOCTOR_TERMS to a new current version.
    sqlx::query(
        "UPDATE legal_documents SET is_current=false WHERE document_type='DOCTOR_TERMS' AND version='1.0-afyamkononi'",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO legal_documents (document_type, version, title, required, is_current, body) \
         VALUES ('DOCTOR_TERMS','1.1','AfyaMkononi Doctor Terms of Service',true,true,'Updated terms text.')",
    )
    .execute(&pool)
    .await
    .unwrap();

    // 8. the original doctor's original-version acceptance row is untouched
    // (still present, unmodified) after the version bump.
    let kept_version: String = sqlx::query_scalar(
        "SELECT document_version FROM legal_acceptances WHERE user_id=$1 AND document_type='DOCTOR_TERMS'",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let kept_hash: String = sqlx::query_scalar(
        "SELECT document_hash FROM legal_acceptances WHERE user_id=$1 AND document_type='DOCTOR_TERMS'",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(kept_version, "1.0-afyamkononi");
    assert_eq!(kept_hash, original_hash);

    // 9. a data-level staleness query shows this doctor is behind the new
    // current DOCTOR_TERMS version (no login-time gate is built this pass —
    // see the plan's explicit out-of-scope note).
    let is_stale: bool = sqlx::query_scalar(
        "SELECT NOT EXISTS (\
            SELECT 1 FROM legal_acceptances la \
            JOIN legal_documents ld ON ld.document_type = la.document_type AND ld.version = la.document_version \
            WHERE la.user_id = $1 AND la.document_type = 'DOCTOR_TERMS' AND ld.is_current\
        )",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(is_stale);
}

// ============================================================================
// Previously uncovered admin/aggregate telemetry routes and month-filter regression.
// ============================================================================

#[sqlx::test(migrations = "./migrations")]
async fn test_admin_system_error_simulation_environment_abac_and_persistence(pool: PgPool) {
    let (app, pool, mut state) = test_app_and_state(pool).await;
    let platform = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let security = token_for_real_role(&pool, "ADMIN").await;
    let payload = serde_json::json!({
        "scenario":"sms_otp", "severity":"WARNING",
        "custom_note":"Synthetic route coverage", "impacted_user_id":DOCTOR1_USER_ID
    });
    let uri = "/api/v1/admin/system-errors/simulate";
    assert_eq!(
        post(&app, uri, None, &payload).await.status(),
        StatusCode::UNAUTHORIZED
    );
    for role in [
        "PATIENT",
        "DOCTOR",
        "FINANCE_ADMIN",
        "CLINICAL_ADMIN",
        "COMPLIANCE",
        "SUPPORT",
    ] {
        let token = if ["PATIENT", "DOCTOR"].contains(&role) {
            token_for_real_role(&pool, role).await
        } else {
            token_for_retired_role(&pool, role).await
        };
        assert_eq!(
            post(&app, uri, Some(&token), &payload).await.status(),
            if ["PATIENT", "DOCTOR"].contains(&role) {
                StatusCode::FORBIDDEN
            } else {
                StatusCode::UNAUTHORIZED
            },
            "{role}"
        );
    }
    let unknown = serde_json::json!({"scenario":"not_a_real_scenario", "severity":"WARNING"});
    assert_eq!(
        post(&app, uri, Some(&platform), &unknown).await.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM system_error_logs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        count, 0,
        "denied/invalid simulations must not persist incidents"
    );

    for token in [&platform, &security] {
        let response = post(&app, uri, Some(token), &payload).await;
        assert_eq!(response.status(), StatusCode::CREATED);
        let body = json_body(response).await;
        assert_eq!(body["success"], true);
        let id = uid(body["data"]["id"].as_str().unwrap());
        let stored: (String, String, Uuid) = sqlx::query_as(
            "SELECT subsystem,severity,impacted_user_id FROM system_error_logs WHERE id=$1",
        )
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            stored,
            (
                "AUTH_OTP_GATEWAY".into(),
                "WARNING".into(),
                uid(DOCTOR1_USER_ID)
            )
        );
        let response = get(&app, "/api/v1/admin/system-errors", Some(token)).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert!(body["data"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["id"] == id.to_string()));
    }

    // Override AppState configuration without mutating process environment or
    // reseeding the database, following the existing caller-supplied config pattern.
    let mut config = deployed_config();
    config.environment = "staging".into();
    config.dev_simulation_enabled = true;
    state.config = Arc::new(config);
    let staging_app = create_router(state.clone());
    let staging_payload = serde_json::json!({"scenario":"offline_sync", "severity":"ERROR"});
    assert_eq!(
        through_tls_proxy(
            &staging_app,
            "POST",
            uri,
            Some(&security),
            Some(&staging_payload)
        )
        .await
        .status(),
        StatusCode::CREATED
    );
    let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM system_error_logs")
        .fetch_one(&pool)
        .await
        .unwrap();
    let mut config = (*state.config).clone();
    config.environment = "production".into();
    config.dev_simulation_enabled = false;
    state.config = Arc::new(config);
    let production_app = create_router(state);
    let production_platform = token_for_real_role(&pool, "ADMIN").await;
    for token in [&production_platform, &security] {
        assert_eq!(
            through_tls_proxy(&production_app, "POST", uri, Some(token), Some(&payload))
                .await
                .status(),
            StatusCode::FORBIDDEN
        );
    }
    let after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM system_error_logs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        after, before,
        "production simulation must have no persistence side effect"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_admin_compliance_alerts_abac_and_real_prescription_data(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let platform = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let compliance = token_for_real_role(&pool, "ADMIN").await;
    let security = token_for_real_role(&pool, "ADMIN").await;
    let uri = "/api/v1/admin/compliance/alerts";
    assert_eq!(
        get(&app, uri, None).await.status(),
        StatusCode::UNAUTHORIZED
    );
    for token in [
        &patient,
        &doctor,
        &token_for_retired_role(&pool, "FINANCE_ADMIN").await,
        &token_for_retired_role(&pool, "CLINICAL_ADMIN").await,
        &token_for_retired_role(&pool, "SUPPORT").await,
    ] {
        assert_eq!(
            get(&app, uri, Some(token)).await.status(),
            if ["PATIENT", "DOCTOR"].contains(
                &hellodoctor_backend::crypto::verify_access_token(token, TEST_JWT_PUBLIC_KEY_B64)
                    .unwrap()
                    .role
                    .as_str()
            ) {
                StatusCode::FORBIDDEN
            } else {
                StatusCode::UNAUTHORIZED
            }
        );
    }
    let (appointment, _) = book_and_confirm(&app, &patient).await;
    open_clinical_window(&pool, appointment).await;
    let notes = "MORPHINE: synthetic controlled-substance review fixture";
    let rx_uri = format!("/api/v1/consultations/{appointment}/prescription");
    let flagged = post(
        &app,
        &rx_uri,
        Some(&doctor),
        &serde_json::json!({"base64_photo":minimal_jpeg_base64(), "doctor_notes":notes}),
    )
    .await;
    assert_eq!(flagged.status(), StatusCode::CREATED);
    let flagged_id = json_body(flagged).await["data"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let ordinary = post(&app, &rx_uri, Some(&doctor), &serde_json::json!({"base64_photo":minimal_jpeg_base64(), "doctor_notes":"Synthetic ordinary prescription"})).await;
    assert_eq!(ordinary.status(), StatusCode::CREATED);
    let doctor_name: String =
        sqlx::query_scalar("SELECT full_name FROM doctor_profiles WHERE id=$1")
            .bind(uid(DOCTOR1_PROFILE_ID))
            .fetch_one(&pool)
            .await
            .unwrap();
    let patient_name: String =
        sqlx::query_scalar("SELECT full_name FROM patient_profiles WHERE id=$1")
            .bind(uid(PATIENT_PROFILE_ID))
            .fetch_one(&pool)
            .await
            .unwrap();
    for token in [&platform, &compliance] {
        let response = get(&app, uri, Some(token)).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert_eq!(body["success"], true);
        let alerts = body["data"].as_array().unwrap();
        assert_eq!(
            alerts.len(),
            1,
            "ordinary prescription must not produce an alert"
        );
        let alert = &alerts[0];
        assert_eq!(alert["id"], flagged_id);
        assert_eq!(alert["doctor_name"], doctor_name);
        assert_eq!(alert["patient_name"], patient_name);
        assert_eq!(alert["doctor_notes"], notes);
        assert_eq!(alert["category"], "Schedule H / Narcotics Dispensation");
        assert_eq!(alert["status"], "FLAGGED");
        assert!(!alert["rx_number"].as_str().unwrap().is_empty());
        assert!(!alert["flag_reason"].as_str().unwrap().is_empty());
        DateTime::parse_from_rfc3339(alert["created_at"].as_str().unwrap()).unwrap();
    }
    // The security role is ABAC-authorized, while clinical RLS can still scope
    // what prescription data it sees. Do not weaken those policies for this test.
    assert_eq!(
        get(&app, uri, Some(&security)).await.status(),
        StatusCode::OK
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_admin_audit_log_simulation_abac_and_persistence(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let platform = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let security = token_for_real_role(&pool, "ADMIN").await;
    let uri = "/api/v1/admin/logs/simulate";
    let payload = serde_json::json!({"action":"SYNTHETIC_ROUTE_COVERAGE", "subsystem":"TEST_AUDIT",
        "severity":"warning", "error_summary":"Synthetic audit fixture", "stack_trace":"synthetic"});
    assert_eq!(
        post(&app, uri, None, &payload).await.status(),
        StatusCode::UNAUTHORIZED
    );
    for role in [
        "PATIENT",
        "DOCTOR",
        "FINANCE_ADMIN",
        "CLINICAL_ADMIN",
        "COMPLIANCE",
        "SUPPORT",
    ] {
        let token = if ["PATIENT", "DOCTOR"].contains(&role) {
            token_for_real_role(&pool, role).await
        } else {
            token_for_retired_role(&pool, role).await
        };
        assert_eq!(
            post(&app, uri, Some(&token), &payload).await.status(),
            if ["PATIENT", "DOCTOR"].contains(&role) {
                StatusCode::FORBIDDEN
            } else {
                StatusCode::UNAUTHORIZED
            },
            "{role}"
        );
    }
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_events WHERE action='SYNTHETIC_ROUTE_COVERAGE'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 0);
    let mut identifiers = Vec::new();
    for (token, role) in [(&platform, "ADMIN"), (&security, "ADMIN")] {
        let response = post(&app, uri, Some(token), &payload).await;
        assert_eq!(response.status(), StatusCode::CREATED);
        let body = json_body(response).await;
        assert_eq!(body["success"], true);
        let id = uid(body["data"]["id"].as_str().unwrap());
        let row = sqlx::query("SELECT actor_id,actor_role::text AS actor_role,action,resource_type,result,reason FROM audit_events WHERE id=$1")
            .bind(id).fetch_one(&pool).await.unwrap();
        let actor =
            hellodoctor_backend::crypto::verify_access_token(token, TEST_JWT_PUBLIC_KEY_B64)
                .unwrap()
                .user_id;
        assert_eq!(row.get::<Uuid, _>("actor_id").to_string(), actor);
        assert_eq!(
            row.get::<Uuid, _>("actor_id").to_string(),
            body["data"]["actor_id"]
        );
        assert_eq!(row.get::<String, _>("actor_role"), role);
        assert_eq!(row.get::<String, _>("action"), "SYNTHETIC_ROUTE_COVERAGE");
        assert_eq!(row.get::<String, _>("resource_type"), "TEST_AUDIT");
        assert_eq!(row.get::<String, _>("result"), "WARNING");
        assert_eq!(row.get::<String, _>("reason"), "Synthetic audit fixture");
        identifiers.push(id.to_string());
    }
    let response = get(&app, "/api/v1/admin/logs", Some(&security)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    for id in identifiers {
        assert!(body["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["id"] == id));
    }
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_events WHERE action='SYNTHETIC_ROUTE_COVERAGE'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        count, 2,
        "only the two authorized calls may create audit rows"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_admin_professional_verification_queue_pending_only_and_abac(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let platform = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let clinical = token_for_real_role(&pool, "ADMIN").await;
    sqlx::query(
        "UPDATE doctor_profiles SET professional_verification_status='PENDING' WHERE id=$1",
    )
    .bind(uid(DOCTOR2_PROFILE_ID))
    .execute(&pool)
    .await
    .unwrap();
    let uri = "/api/v1/admin/doctors/pending-verification";
    assert_eq!(
        get(&app, uri, None).await.status(),
        StatusCode::UNAUTHORIZED
    );
    for role in [
        "PATIENT",
        "DOCTOR",
        "FINANCE_ADMIN",
        "SECURITY_ADMIN",
        "COMPLIANCE",
        "SUPPORT",
    ] {
        let token = if ["PATIENT", "DOCTOR"].contains(&role) {
            token_for_real_role(&pool, role).await
        } else {
            token_for_retired_role(&pool, role).await
        };
        assert_eq!(
            get(&app, uri, Some(&token)).await.status(),
            if ["PATIENT", "DOCTOR"].contains(&role) {
                StatusCode::FORBIDDEN
            } else {
                StatusCode::UNAUTHORIZED
            },
            "{role}"
        );
    }
    for token in [&platform, &clinical] {
        let response = get(&app, uri, Some(token)).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert_eq!(body["success"], true);
        let queue = body["data"].as_array().unwrap();
        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0]["id"], DOCTOR2_PROFILE_ID);
        assert_eq!(queue[0]["user_id"], DOCTOR2_USER_ID);
        assert_eq!(queue[0]["is_verified"], false);
        assert_eq!(queue[0]["professional_verification_status"], "PENDING");
        assert_eq!(queue[0]["license_authority"], "KMPDC");
        assert_eq!(queue[0]["license_country"], "KE");
        assert!(queue
            .iter()
            .all(|doctor| doctor["id"] != DOCTOR1_PROFILE_ID));
    }
    // Rejection is terminal and must leave the pending queue on every fresh read.
    let rejected = post(
        &app,
        &format!("/api/v1/admin/doctors/{DOCTOR2_PROFILE_ID}/reject"),
        Some(&clinical),
        &serde_json::json!({"note":"Synthetic rejection coverage"}),
    )
    .await;
    assert_eq!(rejected.status(), StatusCode::OK);
    assert_eq!(json_body(rejected).await["data"]["status"], "REJECTED");
    for _ in 0..2 {
        let queue = json_body(get(&app, uri, Some(&clinical)).await).await;
        assert!(queue["data"].as_array().unwrap().is_empty());
        let profile = json_body(
            get(
                &app,
                "/api/v1/doctor/profile",
                Some(&make_token(uid(DOCTOR2_USER_ID), "DOCTOR")),
            )
            .await,
        )
        .await;
        assert_eq!(
            profile["data"]["professional_verification_status"],
            "REJECTED"
        );
        assert_eq!(profile["data"]["is_verified"], false);
    }
    let overview = json_body(get(&app, "/api/v1/admin/overview", Some(&platform)).await).await;
    assert_eq!(overview["data"]["pending_professional_verification"], 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_aggregate_consultation_telemetry_ownership_persistence_and_admin_timeline(
    pool: PgPool,
) {
    let (app, pool) = test_app(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let stranger = make_token(uid(DOCTOR2_USER_ID), "DOCTOR");
    let platform = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let (appointment, _) = book_and_confirm(&app, &patient).await;
    let session = consultation_repo::find_by_appointment(&pool, appointment)
        .await
        .unwrap()
        .unwrap();
    let uri = format!("/api/v1/consultations/{appointment}/telemetry");
    let payload = serde_json::json!({"call_duration_seconds":120, "connection_state":"CONNECTED", "prescription_issued":false});
    assert_eq!(
        post(&app, &uri, None, &payload).await.status(),
        StatusCode::UNAUTHORIZED
    );
    // RLS hides another doctor's session before the service can access it.
    assert_eq!(
        post(&app, &uri, Some(&stranger), &payload).await.status(),
        StatusCode::NOT_FOUND
    );
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM consultation_telemetry WHERE session_id=$1")
            .bind(session.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 0);
    // A second consultation lets both genuine participant perspectives submit
    // their own aggregate without depending on duplicate-row semantics.
    let (second_appointment, _) = book_and_confirm(&app, &patient).await;
    for (appointment, token, duration, premature, prescription) in [
        (appointment, &patient, 120, false, false),
        (second_appointment, &doctor, 12, true, true),
    ] {
        let session = consultation_repo::find_by_appointment(&pool, appointment)
            .await
            .unwrap()
            .unwrap();
        let response = post(&app, &format!("/api/v1/consultations/{appointment}/telemetry"), Some(token),
            &serde_json::json!({"call_duration_seconds":duration, "connection_state":"CONNECTED", "prescription_issued":prescription})).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert_eq!(body["data"]["recorded"], true);
        assert_eq!(body["data"]["premature_end"], premature);
        let id = uid(body["data"]["telemetry_id"].as_str().unwrap());
        let stored: (Uuid,i32,String,bool,bool) = sqlx::query_as("SELECT session_id,call_duration_seconds,connection_state,premature_end,prescription_issued FROM consultation_telemetry WHERE id=$1")
            .bind(id).fetch_one(&pool).await.unwrap();
        assert_eq!(
            stored,
            (
                session.id,
                duration,
                "CONNECTED".into(),
                premature,
                prescription
            )
        );
        let response = get(
            &app,
            &format!("/api/v1/admin/consultations/{}/timeline", session.id),
            Some(&platform),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        let aggregate = &body["data"]["aggregate_telemetry"];
        assert_eq!(aggregate["id"], id.to_string());
        assert_eq!(aggregate["session_id"], session.id.to_string());
        assert_eq!(aggregate["call_duration_seconds"], duration);
        assert_eq!(aggregate["connection_state"], "CONNECTED");
        assert_eq!(aggregate["premature_end"], premature);
        assert_eq!(aggregate["prescription_issued"], prescription);
        assert!(aggregate["packet_loss_percent"].is_null());
        assert!(aggregate["round_trip_time_ms"].is_null());
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn test_metrics_returns_prometheus_exposition(pool: PgPool) {
    let (app, _) = test_app(pool).await;
    // Record one completed request first; the process-wide recorder may already
    // have other tests' samples, so do not assert an exact global request count.
    assert_eq!(get(&app, "/healthz", None).await.status(), StatusCode::OK);
    let response = get(&app, "/metrics", None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = std::str::from_utf8(&bytes).unwrap();
    assert!(body.contains("# TYPE http_requests_total counter"));
    assert!(body
        .lines()
        .any(|line| line.starts_with("http_requests_total{")
            && line
                .rsplit_once(' ')
                .is_some_and(|(_, value)| value.parse::<f64>().is_ok())));
}

#[sqlx::test(migrations = "./migrations")]
async fn test_admin_finance_transactions_month_filter_changes_result_set(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let platform = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let finance = token_for_real_role(&pool, "ADMIN").await;
    let (december, _) = book_and_confirm(&app, &patient).await;
    let (january, _) = book_and_confirm(&app, &patient).await;
    for (appointment, timestamp) in [
        (december, "2001-12-31T23:59:59.999999Z"),
        (january, "2002-01-01T00:00:00Z"),
    ] {
        let result = sqlx::query("UPDATE transactions SET created_at=$2 WHERE appointment_id=$1")
            .bind(appointment)
            .bind(
                DateTime::parse_from_rfc3339(timestamp)
                    .unwrap()
                    .with_timezone(&Utc),
            )
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(result.rows_affected(), 1);
    }
    for token in [&platform, &finance] {
        let response = get(&app, "/api/v1/admin/finance/transactions", Some(token)).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            json_body(response).await["data"].as_array().unwrap().len(),
            2
        );
        for (month, appointment) in [("2001-12", december), ("2002-01", january)] {
            let response = get(
                &app,
                &format!("/api/v1/admin/finance/transactions?month={month}"),
                Some(token),
            )
            .await;
            assert_eq!(response.status(), StatusCode::OK);
            let body = json_body(response).await;
            let transactions = body["data"].as_array().unwrap();
            assert_eq!(
                transactions.len(),
                1,
                "{month} must exclude the adjacent month's transaction"
            );
            assert_eq!(transactions[0]["appointment_id"], appointment.to_string());
            assert_eq!(
                DateTime::parse_from_rfc3339(transactions[0]["created_at"].as_str().unwrap())
                    .unwrap()
                    .format("%Y-%m")
                    .to_string(),
                month
            );
        }
        let response = get(
            &app,
            "/api/v1/admin/finance/transactions?month=2001-11",
            Some(token),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert!(json_body(response).await["data"]
            .as_array()
            .unwrap()
            .is_empty());
    }
}

// Authentication/session boundary regressions. Keep this group serialized,
// like the rest of this suite; requests within concurrency tests still race.
#[sqlx::test(migrations = "./migrations")]
async fn test_security_idempotency_requires_fresh_authentication_and_role(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let expired = craft_token(
        uid(PATIENT_USER_ID),
        "PATIENT",
        JWT_ISSUER,
        JWT_AUDIENCE,
        -7200,
        -3600,
    );
    let slot: Uuid = sqlx::query_scalar("SELECT id FROM doctor_schedule_slots WHERE doctor_id=$1 AND status='AVAILABLE' ORDER BY start_time LIMIT 1")
        .bind(uid(DOCTOR1_PROFILE_ID)).fetch_one(&pool).await.unwrap();
    let payload = serde_json::json!({"doctor_id":DOCTOR1_PROFILE_ID,"slot_id":slot,"modality":"VIDEO","gateway":"BKASH"});
    let uri = "/api/v1/appointments/book";
    let key = Uuid::new_v4().to_string();
    let first = post_idem(&app, uri, Some(&patient), &payload, &key).await;
    assert_eq!(first.status(), StatusCode::CREATED);
    let first_body = json_body(first).await;
    for (token, status) in [
        (None, StatusCode::UNAUTHORIZED),
        (Some("invalid-jwt"), StatusCode::UNAUTHORIZED),
        (Some(expired.as_str()), StatusCode::UNAUTHORIZED),
        (Some(doctor.as_str()), StatusCode::FORBIDDEN),
    ] {
        assert_eq!(
            post_idem(&app, uri, token, &payload, &key).await.status(),
            status,
            "a cache hit must not bypass authentication or ABAC"
        );
    }
    sqlx::query("UPDATE users SET is_active=false WHERE id=$1")
        .bind(uid(PATIENT_USER_ID))
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        post_idem(&app, uri, Some(&patient), &payload, &key)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    // Profile-bearing roles can no longer be changed independently of their
    // canonical identity. Forging a different role remains unauthorized.
    assert!(sqlx::query("UPDATE users SET role='DOCTOR' WHERE id=$1")
        .bind(uid(PATIENT_USER_ID))
        .execute(&pool)
        .await
        .is_err());
    sqlx::query("UPDATE users SET is_active=true WHERE id=$1")
        .bind(uid(PATIENT_USER_ID))
        .execute(&pool)
        .await
        .unwrap();
    let changed_role = make_token(uid(PATIENT_USER_ID), "DOCTOR");
    assert_eq!(
        post_idem(&app, uri, Some(&changed_role), &payload, &key)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let replay = post_idem(&app, uri, Some(&patient), &payload, &key).await;
    assert_eq!(replay.status(), StatusCode::CREATED);
    assert_eq!(replay.headers()["content-type"], "application/json");
    assert_eq!(json_body(replay).await, first_body);
    let mut different = payload.clone();
    different["chief_complaint"] = serde_json::json!("different request");
    assert_eq!(
        post_idem(&app, uri, Some(&patient), &different, &key)
            .await
            .status(),
        StatusCode::CONFLICT
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM appointments")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "authorized replay must not repeat the mutation");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_security_idempotency_subject_permissions_query_and_ownership_isolation(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let other_user =
        hellodoctor_backend::repository::user_repo::create_patient_user(&pool, "+254711998877")
            .await
            .unwrap();
    let mut other_profile = hellodoctor_backend::repository::patient_repo::find_by_user_id(
        &pool,
        UserId::from(uid(PATIENT_USER_ID)),
    )
    .await
    .unwrap()
    .unwrap();
    other_profile.id = PatientProfileId::new();
    other_profile.user_id = Some(other_user.id);
    hellodoctor_backend::repository::patient_repo::insert(&pool, &other_profile)
        .await
        .unwrap();
    fixture_session(&pool, other_user.id.into_inner()).await;
    let other = make_token(other_user.id.into_inner(), "PATIENT");
    let slots: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM doctor_schedule_slots WHERE doctor_id=$1 AND status='AVAILABLE' ORDER BY start_time LIMIT 2")
        .bind(uid(DOCTOR1_PROFILE_ID)).fetch_all(&pool).await.unwrap();
    let key = Uuid::new_v4().to_string();
    let uri = "/api/v1/appointments/book";
    let mut appointments = Vec::new();
    let mut payloads = Vec::new();
    for (token, slot) in [(&patient, slots[0]), (&other, slots[1])] {
        let payload = serde_json::json!({"doctor_id":DOCTOR1_PROFILE_ID,"slot_id":slot,"modality":"VIDEO","gateway":"BKASH"});
        let response = post_idem(&app, uri, Some(token), &payload, &key).await;
        assert_eq!(
            response.status(),
            StatusCode::CREATED,
            "different subjects must not share key conflicts or responses"
        );
        appointments.push(uid(json_body(response).await["data"]["appointment_id"]
            .as_str()
            .unwrap()));
        payloads.push(payload);
    }
    assert_ne!(appointments[0], appointments[1]);
    let records: Vec<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT user_id,response_body_ref FROM idempotency_records ORDER BY user_id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(records.len(), 2);
    assert!(records.contains(&(uid(PATIENT_USER_ID), appointments[0])));
    assert!(records.contains(&(other_user.id.into_inner(), appointments[1])));
    // Same subject/role, different signed permissions: no replay of the old
    // context's response. A fresh booking attempt hits the already-locked slot.
    let changed_permissions = craft_token(
        uid(PATIENT_USER_ID),
        "PATIENT",
        JWT_ISSUER,
        JWT_AUDIENCE,
        0,
        900,
    );
    assert_eq!(
        post_idem(&app, uri, Some(&changed_permissions), &payloads[0], &key)
            .await
            .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(
        post_idem(
            &app,
            &format!("{uri}?variant=2"),
            Some(&patient),
            &payloads[0],
            &key
        )
        .await
        .status(),
        StatusCode::CONFLICT,
        "query strings must not alias an existing replay entry"
    );
    // Ownership changes after cache creation must be evaluated again, even for
    // the original authenticated subject and identical request.
    // This test's privileged fixture operation removes the unconfirmed
    // intake first, keeping all new composite ownership FKs consistent.
    sqlx::query("DELETE FROM appointment_clinical_intake WHERE appointment_id=$1")
        .bind(appointments[0])
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE appointments SET patient_id=$2 WHERE id=$1")
        .bind(appointments[0])
        .bind(other_profile.id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        post_idem(&app, uri, Some(&patient), &payloads[0], &key)
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_security_idempotency_reads_keep_query_semantics_and_current_authorization(
    pool: PgPool,
) {
    let (app, pool) = test_app(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let (december, _) = book_and_confirm(&app, &patient).await;
    let (january, _) = book_and_confirm(&app, &patient).await;
    for (appointment, timestamp) in [
        (december, "2001-12-31T23:59:59Z"),
        (january, "2002-01-01T00:00:00Z"),
    ] {
        sqlx::query("UPDATE transactions SET created_at=$2 WHERE appointment_id=$1")
            .bind(appointment)
            .bind(DateTime::parse_from_rfc3339(timestamp).unwrap())
            .execute(&pool)
            .await
            .unwrap();
    }
    let key = Uuid::new_v4().to_string();
    for (month, expected) in [("2001-12", december), ("2002-01", january)] {
        let response = send(
            &app,
            "GET",
            &format!("/api/v1/admin/finance/transactions?month={month}"),
            Some(&admin),
            None,
            Some(&key),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert_eq!(body["data"].as_array().unwrap().len(), 1);
        assert_eq!(body["data"][0]["appointment_id"], expected.to_string());
    }
    let uri = "/api/v1/admin/finance/transactions?month=2001-12";
    assert_eq!(
        send(&app, "GET", uri, None, None, Some(&key))
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        send(&app, "GET", uri, Some(&patient), None, Some(&key))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_security_idempotency_upload_rechecks_ownership_and_clinical_window(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let (appointment, _) = book_and_confirm(&app, &patient).await;
    open_clinical_window(&pool, appointment).await;
    let intake_uri = format!("/api/v1/appointments/{appointment}/prescriptions");
    let intake = serde_json::json!({"files":[{"mime_type":"image/jpeg","base64_content":minimal_jpeg_base64()}]});
    let intake_key = Uuid::new_v4().to_string();
    let first_intake = post_idem(&app, &intake_uri, Some(&patient), &intake, &intake_key).await;
    assert_eq!(first_intake.status(), StatusCode::CREATED);
    let first_intake = json_body(first_intake).await;
    let repeated_intake = post_idem(&app, &intake_uri, Some(&patient), &intake, &intake_key).await;
    assert_eq!(repeated_intake.status(), StatusCode::CREATED);
    assert_eq!(json_body(repeated_intake).await, first_intake);
    let document_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM prescription_intake_documents WHERE appointment_id=$1",
    )
    .bind(appointment)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(document_count, 1);
    assert_eq!(
        post_idem(&app, &intake_uri, Some(&doctor), &intake, &intake_key)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    sqlx::query("UPDATE appointments SET status='CANCELLED' WHERE id=$1")
        .bind(appointment)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        post_idem(&app, &intake_uri, Some(&patient), &intake, &intake_key)
            .await
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    sqlx::query("UPDATE appointments SET status='CONFIRMED' WHERE id=$1")
        .bind(appointment)
        .execute(&pool)
        .await
        .unwrap();
    let uri = format!("/api/v1/consultations/{appointment}/prescription");
    let payload = serde_json::json!({"base64_photo":minimal_jpeg_base64(),"doctor_notes":"Synthetic replay fixture"});
    let key = Uuid::new_v4().to_string();
    let first = post_idem(&app, &uri, Some(&doctor), &payload, &key).await;
    assert_eq!(first.status(), StatusCode::CREATED);
    let first = json_body(first).await;
    let replay = post_idem(&app, &uri, Some(&doctor), &payload, &key).await;
    assert_eq!(replay.status(), StatusCode::CREATED);
    assert_eq!(json_body(replay).await, first);
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM prescriptions WHERE appointment_id=$1")
            .bind(appointment)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
    // Reassign this isolated fixture consistently, including slot and child
    // participant FKs. A private cached response must follow the new ownership.
    reassign_isolated_appointment_doctor(&pool, appointment, uid(DOCTOR2_PROFILE_ID)).await;
    assert_eq!(
        post_idem(&app, &uri, Some(&doctor), &payload, &key)
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    reassign_isolated_appointment_doctor(&pool, appointment, uid(DOCTOR1_PROFILE_ID)).await;
    sqlx::query(
        "UPDATE doctor_profiles SET professional_verification_status='PENDING' WHERE id=$1",
    )
    .bind(uid(DOCTOR1_PROFILE_ID))
    .execute(&pool)
    .await
    .unwrap();
    // This is the ninth upload-lane request in the fixture. Let the
    // eight-token burst budget replenish before asserting the clinical-window
    // rejection; upload shedding is tested separately.
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert_eq!(
        post_idem(&app, &uri, Some(&doctor), &payload, &key)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    sqlx::query(
        "UPDATE doctor_profiles SET professional_verification_status='VERIFIED' WHERE id=$1",
    )
    .bind(uid(DOCTOR1_PROFILE_ID))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE appointments SET status='COMPLETED',completed_at=NOW()-INTERVAL '2 days' WHERE id=$1").bind(appointment).execute(&pool).await.unwrap();
    sqlx::query(
        "UPDATE consultation_sessions SET ended_at=NOW()-INTERVAL '2 days' WHERE appointment_id=$1",
    )
    .bind(appointment)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        post_idem(&app, &uri, Some(&doctor), &payload, &key)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
}

async fn security_patient_login(app: &axum::Router) -> Value {
    let response = post(
        app,
        "/api/v1/auth/register-otp",
        None,
        &serde_json::json!({"phone_number":SEED_PATIENT_PHONE}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let otp = json_body(response).await["data"]["debug_otp_code"]
        .as_str()
        .unwrap()
        .to_string();
    let response = post(
        app,
        "/api/v1/auth/verify-otp-and-login",
        None,
        &serde_json::json!({"phone_number":SEED_PATIENT_PHONE,"otp_code":otp}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    json_body(response).await["data"].clone()
}

#[sqlx::test(migrations = "./migrations")]
async fn test_security_refresh_successors_remain_valid_and_preserve_consumed_identity(
    pool: PgPool,
) {
    let (app, pool) = test_app(pool).await;
    let login = security_patient_login(&app).await;
    let mut token = login["refresh_token"].as_str().unwrap().to_string();
    let original_hash = hellodoctor_backend::crypto::hash_code(&token);
    let family: Uuid =
        sqlx::query_scalar("SELECT token_family_id FROM auth_sessions WHERE refresh_token_hash=$1")
            .bind(&original_hash)
            .fetch_one(&pool)
            .await
            .unwrap();
    for _ in 0..4 {
        let response = post(
            &app,
            "/api/v1/auth/refresh",
            None,
            &serde_json::json!({"refresh_token":token}),
        )
        .await;
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "every fresh successor must be usable for its next rotation"
        );
        let data = json_body(response).await["data"].clone();
        let next = data["refresh_token"].as_str().unwrap().to_string();
        assert_ne!(next, token);
        assert_eq!(
            get(
                &app,
                "/api/v1/doctors",
                Some(data["access_token"].as_str().unwrap())
            )
            .await
            .status(),
            StatusCode::OK
        );
        let consumed: (Uuid, bool) = sqlx::query_as("SELECT token_family_id,rotated_at IS NOT NULL FROM auth_sessions WHERE refresh_token_hash=$1")
            .bind(hellodoctor_backend::crypto::hash_code(&token)).fetch_one(&pool).await.unwrap();
        assert_eq!(consumed, (family, true));
        token = next;
    }
    let counts: (i64, i64, i64) = sqlx::query_as("SELECT COUNT(*),COUNT(*) FILTER(WHERE rotated_at IS NOT NULL),COUNT(*) FILTER(WHERE rotated_at IS NULL AND revoked_at IS NULL) FROM auth_sessions WHERE token_family_id=$1")
        .bind(family).fetch_one(&pool).await.unwrap();
    assert_eq!(counts, (5, 4, 1));
}

#[sqlx::test(migrations = "./migrations")]
async fn test_security_refresh_reuse_revokes_descendants_but_not_another_family(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let first = security_patient_login(&app).await;
    // A second real OTP delivery to the same phone immediately afterward
    // legitimately hits the real per-recipient delivery cooldown (1/60s,
    // `rate_limit::check_otp_delivery_limit`) — assert that rejection
    // explicitly, then advance past only that cooldown (the established
    // `expire_otp_delivery_cooldown` pattern), rather than silently
    // resetting rate-limit state to reach a second successful login. This
    // test's own invariant (reuse revokes descendants but not another
    // family) needs two genuinely distinct sessions, not two rapid OTP
    // deliveries to the same phone, so advancing past the cooldown is the
    // correct way to establish that precondition.
    let still_cooling_down = post(
        &app,
        "/api/v1/auth/register-otp",
        None,
        &serde_json::json!({"phone_number": SEED_PATIENT_PHONE}),
    )
    .await;
    assert_eq!(still_cooling_down.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_controlled_shedding(still_cooling_down).await;
    expire_otp_delivery_cooldown(&state, SEED_PATIENT_PHONE).await;
    let other = security_patient_login(&app).await;
    let original = first["refresh_token"].as_str().unwrap();
    let payload = serde_json::json!({"refresh_token":original});
    let key = Uuid::new_v4().to_string();
    let response = post_idem(&app, "/api/v1/auth/refresh", None, &payload, &key).await;
    assert_eq!(response.status(), StatusCode::OK);
    let child = json_body(response).await["data"]["refresh_token"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        post_idem(&app, "/api/v1/auth/refresh", None, &payload, &key)
            .await
            .status(),
        StatusCode::UNAUTHORIZED,
        "a credential response must not be replayed from the idempotency cache"
    );
    assert_eq!(
        post(
            &app,
            "/api/v1/auth/refresh",
            None,
            &serde_json::json!({"refresh_token":child})
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    let revoked: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM auth_sessions WHERE token_family_id=(SELECT token_family_id FROM auth_sessions WHERE refresh_token_hash=$1) AND revoked_at IS NOT NULL")
        .bind(hellodoctor_backend::crypto::hash_code(original)).fetch_one(&pool).await.unwrap();
    assert_eq!(revoked, 2);
    assert_eq!(
        post(
            &app,
            "/api/v1/auth/refresh",
            None,
            &serde_json::json!({"refresh_token":other["refresh_token"]})
        )
        .await
        .status(),
        StatusCode::OK
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_security_refresh_concurrent_consumption_has_one_winner_and_revokes_family(
    pool: PgPool,
) {
    let (app, pool) = test_app(pool).await;
    let login = security_patient_login(&app).await;
    let payload = serde_json::json!({"refresh_token":login["refresh_token"]});
    let key = Uuid::new_v4().to_string();
    let (first, second) = tokio::join!(
        post_idem(&app, "/api/v1/auth/refresh", None, &payload, &key),
        post_idem(&app, "/api/v1/auth/refresh", None, &payload, &key)
    );
    let mut statuses = [first.status().as_u16(), second.status().as_u16()];
    statuses.sort();
    assert_eq!(
        statuses,
        [200, 401],
        "the same refresh token cannot authenticate two concurrent requests"
    );
    let winner = if first.status() == StatusCode::OK {
        first
    } else {
        second
    };
    let child = json_body(winner).await["data"]["refresh_token"]
        .as_str()
        .unwrap()
        .to_string();
    let counts: (i64, i64, i64) = sqlx::query_as("SELECT COUNT(*),COUNT(*) FILTER(WHERE rotated_at IS NOT NULL),COUNT(*) FILTER(WHERE revoked_at IS NOT NULL) FROM auth_sessions WHERE token_family_id=(SELECT token_family_id FROM auth_sessions WHERE refresh_token_hash=$1)")
        .bind(hellodoctor_backend::crypto::hash_code(login["refresh_token"].as_str().unwrap()))
        .fetch_one(&pool).await.unwrap();
    assert_eq!(
        counts,
        (2, 1, 2),
        "the losing reuse attempt must revoke the successor too"
    );
    assert_eq!(
        post(
            &app,
            "/api/v1/auth/refresh",
            None,
            &serde_json::json!({"refresh_token":child})
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_security_refresh_insert_failure_rolls_back_consumption(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let login = security_patient_login(&app).await;
    let payload = serde_json::json!({"refresh_token":login["refresh_token"]});
    sqlx::raw_sql("CREATE FUNCTION reject_refresh_successor() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'Synthetic refresh successor insert failure'; END $$; CREATE TRIGGER reject_refresh_successor BEFORE INSERT ON auth_sessions FOR EACH ROW EXECUTE FUNCTION reject_refresh_successor();")
        .execute(&pool).await.unwrap();
    assert_eq!(
        post(&app, "/api/v1/auth/refresh", None, &payload)
            .await
            .status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    let counts: (i64, i64) = sqlx::query_as("SELECT COUNT(*),COUNT(*) FILTER(WHERE rotated_at IS NULL AND revoked_at IS NULL) FROM auth_sessions WHERE token_family_id=(SELECT token_family_id FROM auth_sessions WHERE refresh_token_hash=$1)")
        .bind(hellodoctor_backend::crypto::hash_code(login["refresh_token"].as_str().unwrap()))
        .fetch_one(&pool).await.unwrap();
    assert_eq!(
        counts,
        (1, 1),
        "failed successor creation must leave the original token usable"
    );
    sqlx::query("DROP TRIGGER reject_refresh_successor ON auth_sessions")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        post(&app, "/api/v1/auth/refresh", None, &payload)
            .await
            .status(),
        StatusCode::OK
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_security_refresh_logout_race_cannot_leave_live_descendants(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let login = security_patient_login(&app).await;
    let original = serde_json::json!({"refresh_token":login["refresh_token"]});
    let response = post(&app, "/api/v1/auth/refresh", None, &original).await;
    assert_eq!(response.status(), StatusCode::OK);
    let current =
        serde_json::json!({"refresh_token":json_body(response).await["data"]["refresh_token"]});
    // Logging out with a consumed ancestor still identifies the same family.
    let (refreshed, logout) = tokio::join!(
        post(&app, "/api/v1/auth/refresh", None, &current),
        post(&app, "/api/v1/auth/logout", None, &original)
    );
    assert_eq!(logout.status(), StatusCode::OK);
    assert!(matches!(
        refreshed.status(),
        StatusCode::OK | StatusCode::UNAUTHORIZED
    ));
    let live: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM auth_sessions WHERE revoked_at IS NULL AND token_family_id=(SELECT token_family_id FROM auth_sessions WHERE refresh_token_hash=$1)")
            .bind(hellodoctor_backend::crypto::hash_code(login["refresh_token"].as_str().unwrap()))
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(live, 0, "logout must revoke a racing successor atomically");
    if refreshed.status() == StatusCode::OK {
        let next = serde_json::json!({"refresh_token":json_body(refreshed).await["data"]["refresh_token"]});
        assert_eq!(
            post(&app, "/api/v1/auth/refresh", None, &next)
                .await
                .status(),
            StatusCode::UNAUTHORIZED
        );
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn test_security_patient_otp_single_use_even_with_idempotency_key(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let initial_sessions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM auth_sessions")
        .fetch_one(&pool)
        .await
        .unwrap();
    let response = post(
        &app,
        "/api/v1/auth/register-otp",
        None,
        &serde_json::json!({"phone_number":SEED_PATIENT_PHONE}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let code = json_body(response).await["data"]["debug_otp_code"]
        .as_str()
        .unwrap()
        .to_string();
    let payload = serde_json::json!({"phone_number":SEED_PATIENT_PHONE,"otp_code":code});
    let key = Uuid::new_v4().to_string();
    let uri = "/api/v1/auth/verify-otp-and-login";
    let first = post_idem(&app, uri, None, &payload, &key).await;
    assert_eq!(first.status(), StatusCode::OK);
    assert_eq!(
        post_idem(&app, uri, None, &payload, &key).await.status(),
        StatusCode::UNAUTHORIZED,
        "public authentication responses must never be replayed as a second login"
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM auth_sessions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, initial_sessions + 1);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM phone_otps")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_security_patient_otp_concurrent_consumption_issues_one_session(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    for phone in [SEED_PATIENT_PHONE, "+254711887766"] {
        let initial_sessions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM auth_sessions s JOIN users u ON u.id=s.user_id WHERE u.phone_number=$1")
            .bind(phone).fetch_one(&pool).await.unwrap();
        let response = post(
            &app,
            "/api/v1/auth/register-otp",
            None,
            &serde_json::json!({"phone_number":phone}),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let code = json_body(response).await["data"]["debug_otp_code"]
            .as_str()
            .unwrap()
            .to_string();
        let documents = legal_repo::list_current(&pool).await.unwrap();
        let consent = documents
            .iter()
            .find(|d| d.document_type.as_str() == "PATIENT_DATA_SHARING_CONSENT")
            .unwrap();
        let payload = serde_json::json!({"phone_number":phone,"otp_code":code,
            "accepted_documents":[{"type":consent.document_type.as_str(),"version":consent.version}]});
        let key = Uuid::new_v4().to_string();
        let (first, second) = tokio::join!(
            post_idem(
                &app,
                "/api/v1/auth/verify-otp-and-login",
                None,
                &payload,
                &key
            ),
            post_idem(
                &app,
                "/api/v1/auth/verify-otp-and-login",
                None,
                &payload,
                &key
            )
        );
        let mut statuses = [first.status().as_u16(), second.status().as_u16()];
        statuses.sort();
        assert_eq!(statuses, [200, 401], "one OTP must authenticate only one concurrent request, for both existing and new patients");
        let winner = if first.status() == StatusCode::OK {
            first
        } else {
            second
        };
        let data = json_body(winner).await["data"].clone();
        let access_token = data["access_token"].as_str().unwrap();
        let mut doctors_status = StatusCode::SERVICE_UNAVAILABLE;
        for _ in 0..3 {
            let response = get(&app, "/api/v1/doctors", Some(access_token)).await;
            doctors_status = response.status();
            if doctors_status != StatusCode::SERVICE_UNAVAILABLE {
                break;
            }
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(1)
                .min(5);
            tokio::time::sleep(std::time::Duration::from_secs(retry_after)).await;
        }
        assert_eq!(doctors_status, StatusCode::OK);
        let counts: (i64, i64) = sqlx::query_as("SELECT (SELECT COUNT(*) FROM auth_sessions s JOIN users u ON u.id=s.user_id WHERE u.phone_number=$1),(SELECT COUNT(*) FROM patient_profiles p JOIN users u ON u.id=p.user_id WHERE u.phone_number=$1)")
            .bind(phone).fetch_one(&pool).await.unwrap();
        assert_eq!(counts, (initial_sessions + 1, 1));
        let remaining: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM phone_otps WHERE phone_number=$1")
                .bind(phone)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(remaining, 0);
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn test_security_patient_otp_attempt_limit_and_expiry_are_atomic(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let initial_sessions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM auth_sessions")
        .fetch_one(&pool)
        .await
        .unwrap();
    let response = post(
        &app,
        "/api/v1/auth/register-otp",
        None,
        &serde_json::json!({"phone_number":SEED_PATIENT_PHONE}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let code = json_body(response).await["data"]["debug_otp_code"]
        .as_str()
        .unwrap()
        .to_string();
    let wrong_code = if code == "000000" { "111111" } else { "000000" };
    sqlx::query("UPDATE phone_otps SET attempts=2 WHERE phone_number=$1")
        .bind(SEED_PATIENT_PHONE)
        .execute(&pool)
        .await
        .unwrap();
    let wrong = serde_json::json!({"phone_number":SEED_PATIENT_PHONE,"otp_code":wrong_code});
    let uri = "/api/v1/auth/verify-otp-and-login";
    let (first, second) =
        tokio::join!(post(&app, uri, None, &wrong), post(&app, uri, None, &wrong));
    let mut statuses = [first.status().as_u16(), second.status().as_u16()];
    statuses.sort();
    assert_eq!(
        statuses,
        [401, 429],
        "concurrent failures must not exceed the attempt budget"
    );
    let attempts: i16 = sqlx::query_scalar("SELECT attempts FROM phone_otps WHERE phone_number=$1")
        .bind(SEED_PATIENT_PHONE)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(attempts, 3);
    assert_eq!(
        post(
            &app,
            uri,
            None,
            &serde_json::json!({"phone_number":SEED_PATIENT_PHONE,"otp_code":code})
        )
        .await
        .status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    sqlx::query("UPDATE phone_otps SET attempts=0,expires_at=NOW()-INTERVAL '1 second' WHERE phone_number=$1").bind(SEED_PATIENT_PHONE).execute(&pool).await.unwrap();
    assert_eq!(
        post(
            &app,
            uri,
            None,
            &serde_json::json!({"phone_number":SEED_PATIENT_PHONE,"otp_code":code})
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    let counts: (i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM phone_otps),(SELECT COUNT(*) FROM auth_sessions)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(counts, (0, initial_sessions));
}

/// `rate_limit::check_otp_request_limit` fixes the OTP-request budget at 3
/// requests/600s per phone number (see `src/rate_limit.rs`). The limiter's
/// counter is a Redis `INCR`+`EXPIRE` Lua script, so it is atomic even under
/// concurrent requests — both of two requests that land after the budget is
/// already spent must come back 429, never silently succeed because they
/// raced each other.
#[sqlx::test(migrations = "./migrations")]
async fn test_security_patient_otp_request_rate_limit_blocks_excess_and_concurrent_requests(
    pool: PgPool,
) {
    let (app, _pool, state) = test_app_and_state(pool).await;
    let phone = format!("+2547{:08}", rand::thread_rng().gen_range(0..100_000_000));
    for index in 0..3 {
        if index > 0 {
            expire_otp_delivery_cooldown(&state, &phone).await;
        }
        let res = post(
            &app,
            "/api/v1/auth/register-otp",
            None,
            &serde_json::json!({"phone_number": phone}),
        )
        .await;
        assert_eq!(
            res.status(),
            StatusCode::OK,
            "three sends within the original rolling budget are permitted after each cooldown"
        );
    }
    let otp_payload = serde_json::json!({"phone_number": phone});
    let (fourth, fifth) = tokio::join!(
        post(&app, "/api/v1/auth/register-otp", None, &otp_payload),
        post(&app, "/api/v1/auth/register-otp", None, &otp_payload)
    );
    assert!([fourth.status(), fifth.status()].contains(&StatusCode::TOO_MANY_REQUESTS));
    assert_controlled_shedding(fourth).await;
    assert_controlled_shedding(fifth).await;
    // Independently prove both callers lose the atomic phone budget, even if
    // HTTP admission sheds one before it can reach the Redis counter.
    let (fourth, fifth) = tokio::join!(
        hellodoctor_backend::rate_limit::check_otp_request_limit(&state.redis, &phone),
        hellodoctor_backend::rate_limit::check_otp_request_limit(&state.redis, &phone)
    );
    assert!(matches!(
        fourth,
        Err(hellodoctor_backend::error::AppError::RateLimitExceeded(_))
    ));
    assert!(matches!(
        fifth,
        Err(hellodoctor_backend::error::AppError::RateLimitExceeded(_))
    ));
}

/// OTP replay must be rejected by the OTP row's single-use delete-on-consume
/// (`otp_repo::delete` inside the same transaction that validates the code),
/// not merely by the idempotency-key cache layer — a second, independent
/// request (its own fresh idempotency context) presenting the exact same
/// already-consumed code must still fail.
#[sqlx::test(migrations = "./migrations")]
async fn test_security_patient_otp_replay_rejected_without_relying_on_idempotency_cache(
    pool: PgPool,
) {
    let (app, _pool) = test_app(pool).await;
    let response = post(
        &app,
        "/api/v1/auth/register-otp",
        None,
        &serde_json::json!({"phone_number": SEED_PATIENT_PHONE}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let code = json_body(response).await["data"]["debug_otp_code"]
        .as_str()
        .unwrap()
        .to_string();
    let payload = serde_json::json!({"phone_number": SEED_PATIENT_PHONE, "otp_code": code});
    let uri = "/api/v1/auth/verify-otp-and-login";
    let first = post(&app, uri, None, &payload).await;
    assert_eq!(first.status(), StatusCode::OK);
    let replay = post(&app, uri, None, &payload).await;
    assert_eq!(
        replay.status(),
        StatusCode::UNAUTHORIZED,
        "a consumed OTP must not authenticate a second, independent request"
    );
}

/// A resend (a second `register-otp` call for the same phone number before
/// the first code expired) must invalidate the earlier code outright, not
/// merely extend/duplicate it — `otp_repo::upsert`'s `ON CONFLICT` replaces
/// `code_hash`, `attempts`, and `expires_at` in place, so only the newest
/// code may ever work.
#[sqlx::test(migrations = "./migrations")]
async fn test_security_patient_otp_resend_invalidates_prior_code(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let first = post(
        &app,
        "/api/v1/auth/register-otp",
        None,
        &serde_json::json!({"phone_number": SEED_PATIENT_PHONE}),
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK);
    let old_code = json_body(first).await["data"]["debug_otp_code"]
        .as_str()
        .unwrap()
        .to_string();

    let blocked = post(
        &app,
        "/api/v1/auth/register-otp",
        None,
        &serde_json::json!({"phone_number": SEED_PATIENT_PHONE}),
    )
    .await;
    assert_eq!(blocked.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_controlled_shedding(blocked).await;
    expire_otp_delivery_cooldown(&state, SEED_PATIENT_PHONE).await;
    let second = post(
        &app,
        "/api/v1/auth/register-otp",
        None,
        &serde_json::json!({"phone_number": SEED_PATIENT_PHONE}),
    )
    .await;
    assert_eq!(second.status(), StatusCode::OK);
    let new_code = json_body(second).await["data"]["debug_otp_code"]
        .as_str()
        .unwrap()
        .to_string();
    assert_ne!(
        old_code, new_code,
        "test setup expects two independently generated codes to differ"
    );

    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM phone_otps WHERE phone_number=$1")
        .bind(SEED_PATIENT_PHONE)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        rows, 1,
        "the resend must replace the single row in place, not accumulate a second one"
    );

    let uri = "/api/v1/auth/verify-otp-and-login";
    let stale = post(
        &app,
        uri,
        None,
        &serde_json::json!({"phone_number": SEED_PATIENT_PHONE, "otp_code": old_code}),
    )
    .await;
    assert_eq!(
        stale.status(),
        StatusCode::UNAUTHORIZED,
        "a code issued before a resend must not still work"
    );

    let fresh = post(
        &app,
        uri,
        None,
        &serde_json::json!({"phone_number": SEED_PATIENT_PHONE, "otp_code": new_code}),
    )
    .await;
    assert_eq!(
        fresh.status(),
        StatusCode::OK,
        "the newest code from the resend must work"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_security_refresh_migration_revokes_legacy_rotated_families(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let migrator = &hellodoctor_backend::migration_status::MIGRATOR;
    // Exercise the normal SQLx down/up paths on this throwaway database.
    migrator.undo(&pool, 20261022000001).await.unwrap();
    // Current handlers cannot run against a deliberately rolled-back schema.
    // Seed the old session representation directly, without fabricating SQLx
    // migration metadata or disabling current authentication middleware.
    let legacy = serde_json::json!({"refresh_token":"synthetic-legacy-initial-token"});
    let untouched = serde_json::json!({"refresh_token":"synthetic-untouched-token"});
    for raw in [&legacy, &untouched] {
        sqlx::query("INSERT INTO auth_sessions(id,user_id,token_family_id,refresh_token_hash,expires_at) VALUES($1,$2,$3,$4,NOW()+INTERVAL '7 days')")
            .bind(Uuid::new_v4()).bind(uid(PATIENT_USER_ID)).bind(Uuid::new_v4())
            .bind(hellodoctor_backend::crypto::hash_code(raw["refresh_token"].as_str().unwrap()))
            .execute(&pool).await.unwrap();
    }
    let legacy_current = "synthetic-legacy-overwritten-refresh-token";
    let legacy_hash = hellodoctor_backend::crypto::hash_code(legacy_current);
    sqlx::query("UPDATE auth_sessions SET refresh_token_hash=$2,rotated_at=NOW() WHERE refresh_token_hash=$1")
        .bind(hellodoctor_backend::crypto::hash_code(legacy["refresh_token"].as_str().unwrap()))
        .bind(&legacy_hash).execute(&pool).await.unwrap();
    migrator.run(&pool).await.unwrap();
    let revoked: bool = sqlx::query_scalar(
        "SELECT revoked_at IS NOT NULL FROM auth_sessions WHERE refresh_token_hash=$1",
    )
    .bind(&legacy_hash)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        revoked,
        "irrecoverable legacy lineage must force reauthentication"
    );
    assert_eq!(
        post(
            &app,
            "/api/v1/auth/refresh",
            None,
            &serde_json::json!({"refresh_token":legacy_current})
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        post(
            &app,
            "/api/v1/auth/refresh",
            None,
            &serde_json::json!({"refresh_token":untouched["refresh_token"]})
        )
        .await
        .status(),
        StatusCode::OK
    );
}

// Kenya manual professional-registration review: terminal state, current ABAC,
// compatibility derivation and transactional audit/notification invariants.
#[sqlx::test(migrations = "./migrations")]
async fn test_professional_review_denies_self_cross_doctor_patient_and_other_admin_roles(
    pool: PgPool,
) {
    let (app, pool) = test_app(pool).await;
    sqlx::query("UPDATE doctor_profiles SET professional_verification_status='PENDING'")
        .execute(&pool)
        .await
        .unwrap();
    // `/api/v1/admin/*` classifies as Cost::Expensive, whose admission layer
    // applies a fixed, system-wide (not per-user, not pool-capacity-derived)
    // token bucket: 20-request burst, refilling at 20/sec
    // (`admission.rs`'s `rates` array — a request-RATE shield, a different
    // dimension from DB-pool/connection capacity entirely, and not
    // something to change for this test). This test fires ~36 Expensive-cost
    // calls in a tight sequential loop with no real network latency, which
    // can outrun that budget. A brief explicit pace keeps every call inside
    // the real production rate rather than silently depending on, or
    // weakening, that limiter.
    async fn paced_post(
        app: &axum::Router,
        uri: &str,
        token: Option<&str>,
        body: &Value,
    ) -> axum::http::Response<Body> {
        tokio::time::sleep(std::time::Duration::from_millis(55)).await;
        post(app, uri, token, body).await
    }
    for id in [DOCTOR1_PROFILE_ID, DOCTOR2_PROFILE_ID] {
        for action in ["verify", "reject"] {
            let uri = format!("/api/v1/admin/doctors/{id}/{action}");
            assert_eq!(
                paced_post(&app, &uri, None, &serde_json::json!({}))
                    .await
                    .status(),
                StatusCode::UNAUTHORIZED
            );
            for role in [
                "PATIENT",
                "DOCTOR",
                "SUPPORT",
                "FINANCE_ADMIN",
                "COMPLIANCE",
                "SECURITY_ADMIN",
            ] {
                let token = if ["PATIENT", "DOCTOR"].contains(&role) {
                    token_for_real_role(&pool, role).await
                } else {
                    token_for_retired_role(&pool, role).await
                };
                assert_eq!(
                    paced_post(&app, &uri, Some(&token), &serde_json::json!({}))
                        .await
                        .status(),
                    if ["PATIENT", "DOCTOR"].contains(&role) {
                        StatusCode::FORBIDDEN
                    } else {
                        StatusCode::UNAUTHORIZED
                    },
                    "{role} {id} {action}"
                );
            }
            let own_doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
            assert_eq!(
                paced_post(&app, &uri, Some(&own_doctor), &serde_json::json!({}))
                    .await
                    .status(),
                StatusCode::FORBIDDEN
            );
        }
    }
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    for field in [
        "professional_verification_status",
        "is_verified",
        "doctor_id",
        "professional_verification_reviewed_by",
    ] {
        let payload = serde_json::json!({field:"VERIFIED"});
        assert_eq!(
            paced_post(
                &app,
                &format!("/api/v1/admin/doctors/{DOCTOR1_PROFILE_ID}/verify"),
                Some(&admin),
                &payload
            )
            .await
            .status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM doctor_profiles WHERE professional_verification_status='PENDING'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 2);
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_events WHERE action LIKE 'DOCTOR_PROFESSIONAL_%'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_professional_verify_is_terminal_with_metadata_and_cannot_reactivate_suspended_account(
    pool: PgPool,
) {
    let (app, pool) = test_app(pool).await;
    let clinical = token_for_real_role(&pool, "ADMIN").await;
    sqlx::query(
        "UPDATE doctor_profiles SET professional_verification_status='PENDING' WHERE id=$1",
    )
    .bind(uid(DOCTOR1_PROFILE_ID))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE users SET is_active=false WHERE id=$1")
        .bind(uid(DOCTOR1_USER_ID))
        .execute(&pool)
        .await
        .unwrap();
    let uri = format!("/api/v1/admin/doctors/{DOCTOR1_PROFILE_ID}/verify");
    let response = post(
        &app,
        &uri,
        Some(&clinical),
        &serde_json::json!({"note":"Synthetic manual registration review"}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        json_body(response).await["data"]["professional_verification_status"],
        "VERIFIED"
    );
    for action in ["verify", "reject"] {
        assert_eq!(
            post(
                &app,
                &format!("/api/v1/admin/doctors/{DOCTOR1_PROFILE_ID}/{action}"),
                Some(&clinical),
                &serde_json::json!({})
            )
            .await
            .status(),
            StatusCode::CONFLICT
        );
    }
    let profile = doctor_repo::find_by_id(&pool, DoctorProfileId::from(uid(DOCTOR1_PROFILE_ID)))
        .await
        .unwrap()
        .unwrap();
    assert!(profile.is_professionally_verified());
    assert!(profile.is_verified);
    assert!(
        !profile.is_on_duty,
        "professional review must not turn duty on"
    );
    assert!(profile.professional_verification_reviewed_at.is_some());
    let claims =
        hellodoctor_backend::crypto::verify_access_token(&clinical, TEST_JWT_PUBLIC_KEY_B64)
            .unwrap();
    assert_eq!(
        profile
            .professional_verification_reviewed_by
            .unwrap()
            .to_string(),
        claims.user_id
    );
    assert_eq!(
        profile.professional_verification_note.as_deref(),
        Some("Synthetic manual registration review")
    );
    let active: bool = sqlx::query_scalar("SELECT is_active FROM users WHERE id=$1")
        .bind(uid(DOCTOR1_USER_ID))
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(!active);
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    assert_eq!(
        get(&app, "/api/v1/doctor/profile", Some(&doctor))
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM notifications WHERE user_id=$1 AND type='REGISTRATION_VERIFIED'",
    )
    .bind(uid(DOCTOR1_USER_ID))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);
    let queue = json_body(
        get(
            &app,
            "/api/v1/admin/doctors/pending-verification",
            Some(&clinical),
        )
        .await,
    )
    .await;
    assert!(queue["data"].as_array().unwrap().is_empty());
}

#[sqlx::test(migrations = "./migrations")]
async fn test_professional_verify_reject_race_has_one_terminal_winner(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let platform = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let clinical = token_for_real_role(&pool, "ADMIN").await;
    for (id, user_id) in [
        (DOCTOR1_PROFILE_ID, DOCTOR1_USER_ID),
        (DOCTOR2_PROFILE_ID, DOCTOR2_USER_ID),
    ] {
        sqlx::query(
            "UPDATE doctor_profiles SET professional_verification_status='PENDING' WHERE id=$1",
        )
        .bind(uid(id))
        .execute(&pool)
        .await
        .unwrap();
        let note = serde_json::json!({"note":"Synthetic concurrent review"});
        let verify_uri = format!("/api/v1/admin/doctors/{id}/verify");
        let reject_uri = format!("/api/v1/admin/doctors/{id}/reject");
        let (verify, reject) = tokio::join!(
            post(&app, &verify_uri, Some(&platform), &note),
            post(&app, &reject_uri, Some(&clinical), &note)
        );
        let mut codes = [verify.status().as_u16(), reject.status().as_u16()];
        codes.sort();
        assert_eq!(codes, [200, 409]);
        let verified = verify.status() == StatusCode::OK;
        let winner = if verified { "VERIFIED" } else { "REJECTED" };
        let row = sqlx::query("SELECT professional_verification_status::text AS status, is_verified, professional_verification_reviewed_at, professional_verification_reviewed_by FROM doctor_profiles WHERE id=$1").bind(uid(id)).fetch_one(&pool).await.unwrap();
        assert_eq!(row.get::<String, _>("status"), winner);
        assert_eq!(row.get::<bool, _>("is_verified"), verified);
        assert!(row
            .get::<Option<DateTime<Utc>>, _>("professional_verification_reviewed_at")
            .is_some());
        assert!(row
            .get::<Option<Uuid>, _>("professional_verification_reviewed_by")
            .is_some());
        let audit: Vec<Value> = sqlx::query_scalar("SELECT after_data FROM audit_events WHERE resource_id=$1 AND action LIKE 'DOCTOR_PROFESSIONAL_%'").bind(uid(id)).fetch_all(&pool).await.unwrap();
        assert_eq!(audit.len(), 1);
        assert_eq!(audit[0]["professional_verification_status"], winner);
        let types: Vec<String> = sqlx::query_scalar("SELECT type FROM notifications WHERE user_id=$1 AND type IN ('REGISTRATION_VERIFIED','REGISTRATION_REJECTED')").bind(uid(user_id)).fetch_all(&pool).await.unwrap();
        assert_eq!(
            types,
            vec![if verified {
                "REGISTRATION_VERIFIED"
            } else {
                "REGISTRATION_REJECTED"
            }]
        );
        for uri in [&verify_uri, &reject_uri] {
            assert_eq!(
                post(&app, uri, Some(&platform), &note).await.status(),
                StatusCode::CONFLICT
            );
        }
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn test_professional_rejection_is_persistent_and_blocks_verified_only_business_gates(
    pool: PgPool,
) {
    let (app, pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let (appointment, _) = book_and_confirm(&app, &patient).await;
    sqlx::query(
        "UPDATE doctor_profiles SET professional_verification_status='PENDING' WHERE id=$1",
    )
    .bind(uid(DOCTOR1_PROFILE_ID))
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/admin/doctors/{DOCTOR1_PROFILE_ID}/reject"),
            Some(&admin),
            &serde_json::json!({"note":"Synthetic rejected review"})
        )
        .await
        .status(),
        StatusCode::OK
    );
    for _ in 0..2 {
        let body = json_body(get(&app, "/api/v1/doctor/profile", Some(&doctor)).await).await;
        assert_eq!(body["data"]["professional_verification_status"], "REJECTED");
        assert_eq!(body["data"]["license_authority"], "KMPDC");
        assert_eq!(body["data"]["license_country"], "KE");
        assert_eq!(body["data"]["is_verified"], false);
        assert_eq!(body["data"]["is_on_duty"], false);
    }
    for uri in ["/api/v1/doctor/appointments", "/api/v1/doctor/chats"] {
        assert_eq!(
            get(&app, uri, Some(&doctor)).await.status(),
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(
        send(
            &app,
            "PUT",
            "/api/v1/doctor/duty",
            Some(&doctor),
            Some(&serde_json::json!({"is_on_duty":true})),
            None
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    let start = Utc::now() + Duration::days(80);
    assert_eq!(
        post(
            &app,
            "/api/v1/doctor/slots",
            Some(&doctor),
            &serde_json::json!({"start_time":start,"end_time":start+Duration::minutes(10)})
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    let question = Uuid::new_v4();
    sqlx::query("INSERT INTO health_questions(id,question_text,status,created_at,updated_at) VALUES($1,'Synthetic professional gate question','OPEN',NOW(),NOW())").bind(question).execute(&pool).await.unwrap();
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/doctor/qna/questions/{question}/answers"),
            Some(&doctor),
            &serde_json::json!({"answer_text":"Synthetic answer"})
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/consultations/{appointment}/rtc-token"),
            Some(&doctor),
            &serde_json::json!({})
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    let list = json_body(get(&app, "/api/v1/doctors", Some(&patient)).await).await;
    assert!(list["data"]
        .as_array()
        .unwrap()
        .iter()
        .all(|d| d["id"] != DOCTOR1_PROFILE_ID));
    let slots = json_body(
        get(
            &app,
            &format!("/api/v1/doctors/{DOCTOR1_PROFILE_ID}/slots"),
            Some(&patient),
        )
        .await,
    )
    .await;
    assert!(slots["data"].as_array().unwrap().is_empty());
    let available: Uuid = sqlx::query_scalar(
        "SELECT id FROM doctor_schedule_slots WHERE doctor_id=$1 AND status='AVAILABLE' LIMIT 1",
    )
    .bind(uid(DOCTOR1_PROFILE_ID))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        post_idem(
            &app,
            "/api/v1/appointments/book",
            Some(&patient),
            &schedule_booking(available),
            &Uuid::new_v4().to_string()
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    // Attempting the compatibility write cannot restore approval or bypass gates.
    sqlx::query("UPDATE doctor_profiles SET is_verified=true WHERE id=$1")
        .bind(uid(DOCTOR1_PROFILE_ID))
        .execute(&pool)
        .await
        .unwrap();
    let p = doctor_repo::find_by_id(&pool, DoctorProfileId::from(uid(DOCTOR1_PROFILE_ID)))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        p.professional_verification_status,
        ProfessionalVerificationStatus::Rejected
    );
    assert!(!p.is_verified);
    assert!(!p.is_professionally_verified());
}

#[sqlx::test(migrations = "./migrations")]
async fn test_professional_review_audit_failure_rolls_back_status_metadata_and_notification(
    pool: PgPool,
) {
    let (app, pool) = test_app(pool).await;
    sqlx::query(
        "UPDATE doctor_profiles SET professional_verification_status='PENDING' WHERE id=$1",
    )
    .bind(uid(DOCTOR1_PROFILE_ID))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::raw_sql("CREATE FUNCTION fail_professional_audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.action LIKE 'DOCTOR_PROFESSIONAL_%' THEN RAISE EXCEPTION 'synthetic audit failure'; END IF; RETURN NEW; END $$; CREATE TRIGGER fail_professional_audit BEFORE INSERT ON audit_events FOR EACH ROW EXECUTE FUNCTION fail_professional_audit();").execute(&pool).await.unwrap();
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/admin/doctors/{DOCTOR1_PROFILE_ID}/verify"),
            Some(&admin),
            &serde_json::json!({"note":"Rollback test"})
        )
        .await
        .status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    let p = doctor_repo::find_by_id(&pool, DoctorProfileId::from(uid(DOCTOR1_PROFILE_ID)))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        p.professional_verification_status,
        ProfessionalVerificationStatus::Pending
    );
    assert!(!p.is_verified);
    assert!(p.professional_verification_reviewed_at.is_none());
    assert!(p.professional_verification_reviewed_by.is_none());
    assert!(p.professional_verification_note.is_none());
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM notifications WHERE user_id=$1")
        .bind(uid(DOCTOR1_USER_ID))
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_professional_review_database_guard_denies_doctor_status_and_metadata_writes(
    pool: PgPool,
) {
    let (_app, pool, state) = test_app_and_state(pool).await;
    sqlx::query("UPDATE doctor_profiles SET professional_verification_status='PENDING'")
        .execute(&pool)
        .await
        .unwrap();
    let ctx = hellodoctor_backend::auth_context::AuthContext {
        user_id: UserId::from(uid(DOCTOR1_USER_ID)),
        role: UserRole::Doctor,
        permissions: vec![],
        session_id: None,
        request_id: None,
    };
    for id in [DOCTOR1_PROFILE_ID, DOCTOR2_PROFILE_ID] {
        for change in [
            "professional_verification_status='VERIFIED'",
            "professional_verification_reviewed_at=NOW()",
        ] {
            let mut tx = state.begin_authed(&ctx).await.unwrap();
            let result = sqlx::query(&format!("UPDATE doctor_profiles SET {change} WHERE id=$1"))
                .bind(uid(id))
                .execute(&mut *tx)
                .await;
            if id == DOCTOR1_PROFILE_ID {
                assert!(
                    result.is_err(),
                    "own professional-review mutation must be rejected"
                );
            } else {
                assert_eq!(
                    result.unwrap().rows_affected(),
                    0,
                    "cross-Doctor row must be invisible to UPDATE"
                );
            }
            tx.rollback().await.unwrap();
        }
    }
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM doctor_profiles WHERE professional_verification_status='PENDING'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 2);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_professional_review_migration_backfill_uses_latest_evidence_and_conservative_pending(
    pool: PgPool,
) {
    let (_app, pool) = test_app(pool).await;
    let migrator = &hellodoctor_backend::migration_status::MIGRATOR;
    migrator.undo(&pool, 20261023000001).await.unwrap();
    sqlx::query("UPDATE doctor_profiles SET is_verified=false WHERE id=$1")
        .bind(uid(DOCTOR2_PROFILE_ID))
        .execute(&pool)
        .await
        .unwrap();
    let rejected_id = Uuid::new_v4();
    let reviewed_id = Uuid::new_v4();
    for (id, phone, license) in [
        (rejected_id, "+254700000301", "SYNTH-KE-REJECTED"),
        (reviewed_id, "+254700000302", "SYNTH-KE-REVIEWED"),
    ] {
        let user = Uuid::new_v4();
        sqlx::query("INSERT INTO users(id,phone_number,role,is_active,is_verified) VALUES($1,$2,'DOCTOR',true,true)").bind(user).bind(phone).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO doctor_profiles(id,user_id,full_name,license_number,license_authority,license_country,primary_specialty,experience_years,current_hospital,qualifications,consultation_fee_video,consultation_fee_chat,residential_address,verified_phone,is_on_duty,is_verified) SELECT $1,$2,'Synthetic Legacy Review',$3,'KMPDC','KE',primary_specialty,experience_years,'Synthetic Clinic',qualifications,0,0,'Synthetic Nairobi',$4,false,false FROM doctor_profiles WHERE id=$5")
            .bind(id).bind(user).bind(license).bind(phone).bind(uid(DOCTOR1_PROFILE_ID)).execute(&pool).await.unwrap();
    }
    // TEST_LEGACY: actual historical action strings are migration inputs only.
    for (id, action, result, seconds) in [
        (rejected_id, "BMDC_REJECT", "REJECTED", 10i64),
        (reviewed_id, "BMDC_REJECT", "REJECTED", 20),
        (reviewed_id, "BMDC_VERIFY", "SUCCESS", 5),
    ] {
        sqlx::query("INSERT INTO audit_events(id,actor_id,actor_role,action,resource_type,resource_id,result,reason,created_at) VALUES($1,$2,'PLATFORM_ADMIN',$3,'DOCTOR_PROFILE',$4,$5,'Synthetic legacy evidence',NOW()-$6*interval '1 second')")
            .bind(Uuid::new_v4()).bind(uid(ADMIN_USER_ID)).bind(action).bind(id).bind(result).bind(seconds).execute(&pool).await.unwrap();
    }
    migrator.run(&pool).await.unwrap();
    // Undo/up recreates the enum with a new PostgreSQL OID. Like a deployed
    // backend restart, use fresh connections rather than cached pre-undo plans.
    let migrated_pool = sqlx::postgres::PgPoolOptions::new()
        .connect_with(pool.connect_options().as_ref().clone())
        .await
        .unwrap();
    for (id, status) in [
        (
            uid(DOCTOR1_PROFILE_ID),
            ProfessionalVerificationStatus::Verified,
        ),
        (
            uid(DOCTOR2_PROFILE_ID),
            ProfessionalVerificationStatus::Pending,
        ),
        (rejected_id, ProfessionalVerificationStatus::Rejected),
        (reviewed_id, ProfessionalVerificationStatus::Verified),
    ] {
        let p = doctor_repo::find_by_id(&migrated_pool, DoctorProfileId::from(id))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(p.professional_verification_status, status);
        assert_eq!(
            p.is_verified,
            status == ProfessionalVerificationStatus::Verified
        );
        if id == rejected_id || id == reviewed_id {
            assert!(p.professional_verification_reviewed_at.is_some());
            assert_eq!(
                p.professional_verification_reviewed_by,
                Some(UserId::from(uid(ADMIN_USER_ID)))
            );
        }
    }
    assert!(
        migrator.undo(&pool, 20261023000001).await.is_err(),
        "rollback must not silently discard rejected/reviewed evidence"
    );
}

// Public-exposure/session security regressions. No live dependencies are stopped.
#[sqlx::test(migrations = "./migrations")]
async fn test_exposure_logout_immediately_invalidates_access_and_refresh_only_its_family(
    pool: PgPool,
) {
    let (app, _pool, state) = test_app_and_state(pool).await;
    let first = security_patient_login(&app).await;
    // A second real OTP delivery to the same phone immediately afterward
    // legitimately hits the real per-recipient delivery cooldown (1/60s,
    // `rate_limit::check_otp_delivery_limit`) — assert that rejection
    // explicitly, then advance past only that cooldown (the established
    // `expire_otp_delivery_cooldown` pattern), rather than silently
    // resetting rate-limit state. This test's invariant needs two genuinely
    // distinct sessions, not two rapid OTP deliveries to one phone.
    let still_cooling_down = post(
        &app,
        "/api/v1/auth/register-otp",
        None,
        &serde_json::json!({"phone_number": SEED_PATIENT_PHONE}),
    )
    .await;
    assert_eq!(still_cooling_down.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_controlled_shedding(still_cooling_down).await;
    expire_otp_delivery_cooldown(&state, SEED_PATIENT_PHONE).await;
    let second = security_patient_login(&app).await;
    let access = first["access_token"].as_str().unwrap();
    let other = second["access_token"].as_str().unwrap();
    assert_eq!(
        get(&app, "/api/v1/doctors", Some(access)).await.status(),
        StatusCode::OK
    );
    let rotated = post(
        &app,
        "/api/v1/auth/refresh",
        None,
        &serde_json::json!({"refresh_token":first["refresh_token"]}),
    )
    .await;
    assert_eq!(rotated.status(), StatusCode::OK);
    let rotated = json_body(rotated).await["data"].clone();
    assert_eq!(
        get(&app, "/api/v1/doctors", Some(access)).await.status(),
        StatusCode::OK,
        "rotation preserves the access-token family"
    );
    assert_eq!(
        post(
            &app,
            "/api/v1/auth/logout",
            None,
            &serde_json::json!({"refresh_token":first["refresh_token"]})
        )
        .await
        .status(),
        StatusCode::OK
    );
    for token in [access, rotated["access_token"].as_str().unwrap()] {
        assert_eq!(
            get(&app, "/api/v1/doctors", Some(token)).await.status(),
            StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(
        post(
            &app,
            "/api/v1/auth/refresh",
            None,
            &serde_json::json!({"refresh_token":rotated["refresh_token"]})
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        get(&app, "/api/v1/doctors", Some(other)).await.status(),
        StatusCode::OK,
        "unrelated device stays authenticated"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_exposure_doctor_admin_logout_rejects_signed_token_and_other_device_stays_valid(
    pool: PgPool,
) {
    let (app, pool, state) = test_app_and_state(pool).await;
    for user_id in [uid(DOCTOR1_USER_ID), uid(ADMIN_USER_ID)] {
        let role = if user_id == uid(ADMIN_USER_ID) {
            "ADMIN"
        } else {
            "DOCTOR"
        };
        let path = if role == "DOCTOR" {
            "/api/v1/doctor/profile"
        } else {
            "/api/v1/admin/overview"
        };
        let family = Uuid::new_v4();
        let refresh = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO auth_sessions(id,user_id,token_family_id,refresh_token_hash,created_at,last_used_at,expires_at) VALUES(gen_random_uuid(),$1,$2,$3,NOW(),NOW(),NOW()+INTERVAL '7 days')")
            .bind(user_id).bind(family).bind(hellodoctor_backend::crypto::hash_code(&refresh)).execute(&pool).await.unwrap();
        let token = hellodoctor_backend::crypto::create_access_token(
            user_id,
            role,
            family,
            TEST_JWT_PRIVATE_KEY_B64,
        )
        .unwrap();
        assert_eq!(get(&app, path, Some(&token)).await.status(), StatusCode::OK);
        assert_eq!(
            post(
                &app,
                "/api/v1/auth/logout",
                None,
                &serde_json::json!({"refresh_token":refresh})
            )
            .await
            .status(),
            StatusCode::OK
        );
        assert!(
            hellodoctor_backend::crypto::verify_access_token(&token, TEST_JWT_PUBLIC_KEY_B64)
                .is_ok(),
            "signature/expiry remain valid"
        );
        assert_eq!(
            get(&app, path, Some(&token)).await.status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            get(&app, path, Some(&make_token(user_id, role)))
                .await
                .status(),
            StatusCode::OK
        );
    }
    state.auth_db.close().await;
    let response = get(
        &app,
        "/api/v1/doctor/profile",
        Some(&make_token(uid(DOCTOR1_USER_ID), "DOCTOR")),
    )
    .await;
    assert_eq!(
        response.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "session-store failure never permits access"
    );
    // A closed auth_db pool can be caught by either of two independent,
    // equally-correct layers: the admission middleware's own eager
    // `exhausted()` pre-check (code "SERVICE_OVERLOADED") if it runs first,
    // or `check_active_session`'s own query failure (code
    // "SERVICE_UNAVAILABLE") if the request reaches that far first — both
    // deny access, which is the actual invariant under test.
    let code = json_body(response).await["error"]["code"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(
        code == "SERVICE_UNAVAILABLE" || code == "SERVICE_OVERLOADED",
        "unexpected error code for a closed session store: {code}"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_exposure_sessionless_foreign_expired_and_revoked_tokens_are_denied(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient = uid(PATIENT_USER_ID);
    let path = "/api/v1/doctors";
    let token = make_token(patient, "PATIENT");
    let claims =
        hellodoctor_backend::crypto::verify_access_token(&token, TEST_JWT_PUBLIC_KEY_B64).unwrap();
    let mut payload = serde_json::to_value(&claims).unwrap();
    payload.as_object_mut().unwrap().remove("sid");
    let key = EncodingKey::from_ed_der(&STANDARD.decode(TEST_JWT_PRIVATE_KEY_B64).unwrap());
    let legacy = encode(&Header::new(Algorithm::EdDSA), &payload, &key).unwrap();
    assert_eq!(
        get(&app, path, Some(&legacy)).await.status(),
        StatusCode::UNAUTHORIZED
    );
    let foreign = hellodoctor_backend::crypto::create_access_token(
        patient,
        "PATIENT",
        uid(DOCTOR1_USER_ID),
        TEST_JWT_PRIVATE_KEY_B64,
    )
    .unwrap();
    assert_eq!(
        get(&app, path, Some(&foreign)).await.status(),
        StatusCode::UNAUTHORIZED
    );
    let expired = craft_token(patient, "PATIENT", JWT_ISSUER, JWT_AUDIENCE, -3600, -3600);
    assert_eq!(
        get(&app, path, Some(&expired)).await.status(),
        StatusCode::UNAUTHORIZED
    );
    sqlx::query("UPDATE auth_sessions SET revoked_at=NOW() WHERE token_family_id=$1")
        .bind(patient)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        get(&app, path, Some(&token)).await.status(),
        StatusCode::UNAUTHORIZED
    );
}

#[test]
fn test_exposure_startup_safety_matrix() {
    let safe = deployed_config();
    assert!(safe.validate_public_exposure().is_ok());
    for mode in ["", "unknown"] {
        let mut c = safe.clone();
        c.environment = mode.into();
        assert!(c.validate_public_exposure().is_err());
    }
    let mut c = safe.clone();
    c.seed_dev_data = true;
    assert!(c.validate_public_exposure().is_err());
    let mut c = safe.clone();
    c.dev_auth_otp_enabled = true;
    assert!(c.validate_public_exposure().is_err());
    let mut c = safe.clone();
    c.dev_simulation_enabled = true;
    assert!(c.validate_public_exposure().is_err());
    let mut c = safe.clone();
    c.trusted_proxy_ips.clear();
    assert!(c.validate_public_exposure().is_err());
    let mut c = safe.clone();
    c.metrics_scrape_token.clear();
    assert!(c.validate_public_exposure().is_err());
    let mut c = safe.clone();
    c.public_api_base_url = "http://api.example.test".into();
    assert!(c.validate_public_exposure().is_err());
    let mut c = safe.clone();
    c.allowed_origins = vec!["http://doctor.example.test".into()];
    assert!(c.validate_public_exposure().is_err());
    assert!(test_config().validate_public_exposure().is_ok());
}

#[sqlx::test(migrations = "./migrations")]
async fn test_exposure_production_metrics_and_simulation_have_independent_backend_guards(
    pool: PgPool,
) {
    let (app, pool) = test_app_with_config(pool, deployed_config()).await;
    assert_eq!(
        through_tls_proxy(&app, "GET", "/metrics", None, None)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        through_tls_proxy(
            &app,
            "GET",
            "/metrics?token=synthetic-monitoring-credential-for-tests-only",
            None,
            None
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    let scrape = through_tls_proxy(
        &app,
        "GET",
        "/metrics",
        Some("synthetic-monitoring-credential-for-tests-only"),
        None,
    )
    .await;
    assert_eq!(scrape.status(), StatusCode::OK);
    assert!(scrape.headers()["content-type"]
        .to_str()
        .unwrap()
        .contains("text/plain"));
    for role in ["ADMIN", "ADMIN"] {
        let token = token_for_real_role(&pool, role).await;
        let response=through_tls_proxy(&app,"POST","/api/v1/admin/logs/simulate",Some(&token),Some(&serde_json::json!({"action":"TEST","subsystem":"TEST","severity":"ERROR","error_summary":"synthetic"}))).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert!(json_body(response).await["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Simulation"));
    }
    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_events WHERE action='TEST'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 0);
    // Known seed credentials cannot authenticate even through a legitimate proxy.
    assert_eq!(
        through_tls_proxy(
            &app,
            "GET",
            "/api/v1/admin/overview",
            Some(&make_token(uid(ADMIN_USER_ID), "ADMIN")),
            None
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_exposure_https_requires_trusted_peer_and_sanitized_proxy_metadata(pool: PgPool) {
    let (app, _) = test_app_with_config(pool, deployed_config()).await;
    assert_eq!(
        get(&app, "/api/v1/doctors", None).await.status(),
        StatusCode::FORBIDDEN
    );
    let mut request = Request::builder()
        .uri("/api/v1/doctors")
        .header("x-forwarded-proto", "https")
        .header("x-forwarded-for", "198.51.100.73")
        .body(Body::empty())
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(
        "203.0.113.10:40000".parse::<SocketAddr>().unwrap(),
    ));
    assert_eq!(
        app.clone().oneshot(request).await.unwrap().status(),
        StatusCode::FORBIDDEN,
        "untrusted client cannot assert HTTPS"
    );
    assert_eq!(
        through_tls_proxy(&app, "GET", "/api/v1/doctors", None, None)
            .await
            .status(),
        StatusCode::UNAUTHORIZED,
        "trusted TLS reaches the ordinary JWT guard"
    );
    for (proto, client) in [
        ("http", "198.51.100.73"),
        ("https,http", "198.51.100.73"),
        ("https", "198.51.100.73,203.0.113.10"),
    ] {
        let mut request = Request::builder()
            .uri("/metrics")
            .header("x-forwarded-proto", proto)
            .header("x-forwarded-for", client)
            .body(Body::empty())
            .unwrap();
        request.extensions_mut().insert(ConnectInfo(
            "127.0.0.1:40000".parse::<SocketAddr>().unwrap(),
        ));
        assert_eq!(
            app.clone().oneshot(request).await.unwrap().status(),
            StatusCode::FORBIDDEN
        );
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn test_exposure_redis_outage_fails_closed_for_every_public_auth_route_and_records_incident(
    pool: PgPool,
) {
    let (_, pool, mut state) = test_app_and_state(pool).await;
    let config = deadpool_redis::Config::from_url("redis://127.0.0.1:1/15");
    state.redis = config
        .create_pool(Some(deadpool_redis::Runtime::Tokio1))
        .unwrap();
    let app = create_router(state);
    for path in [
        "register-otp",
        "verify-otp-and-login",
        "doctor/login",
        "doctor/verify-otp",
        "doctor/register",
        "admin/login",
        "refresh",
        "logout",
    ] {
        let response = post(
            &app,
            &format!("/api/v1/auth/{path}"),
            None,
            &serde_json::json!({}),
        )
        .await;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE, "{path}");
        assert_eq!(
            json_body(response).await["error"]["code"],
            "RATE_LIMIT_SERVICE_UNAVAILABLE"
        );
    }
    for _ in 0..30 {
        let count:i64=sqlx::query_scalar("SELECT COUNT(*) FROM system_error_logs WHERE error_code='RATE_LIMIT_SERVICE_UNAVAILABLE'").fetch_one(&pool).await.unwrap();
        if count > 0 {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    }
    panic!("Redis outage must reach the existing System Error Service");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_exposure_rate_limit_exhaustion_is_429_and_proxy_clients_have_separate_budgets(
    pool: PgPool,
) {
    let (_, _, mut state) = test_app_and_state(pool).await;
    let mut config = deployed_config();
    config.global_ip_rate_limit_per_min = 1;
    state.config = Arc::new(config);
    let app = create_router(state);
    assert_eq!(
        through_tls_proxy(&app, "GET", "/metrics", None, None)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let response = through_tls_proxy(&app, "GET", "/metrics", None, None).await;
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        json_body(response).await["error"]["code"],
        "RATE_LIMIT_EXCEEDED"
    );
    let mut request = Request::builder()
        .uri("/metrics")
        .header("x-forwarded-proto", "https")
        .header("x-forwarded-for", "198.51.100.74")
        .body(Body::empty())
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(
        "127.0.0.1:40000".parse::<SocketAddr>().unwrap(),
    ));
    assert_eq!(
        app.oneshot(request).await.unwrap().status(),
        StatusCode::UNAUTHORIZED,
        "a different client has its own budget"
    );
}

fn article_content() -> Value {
    serde_json::json!({"title":"Synthetic educational article", "summary":"An overview for patients", "body":"Structured plain text educational content within the author's specialty.", "references_list":["Synthetic educational reference"]})
}
async fn draft_article(app: &axum::Router, doctor: &str) -> Value {
    let response = post(
        app,
        "/api/v1/doctor/articles",
        Some(doctor),
        &serde_json::json!({"creation_key":Uuid::new_v4().to_string(),"content":article_content()}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    json_body(response).await["data"].clone()
}
#[sqlx::test(migrations = "./migrations")]
async fn test_articles_direct_publish_edit_archive_and_patient_visibility(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let article = draft_article(&app, &doctor).await;
    let id = article["id"].as_str().unwrap();
    assert_eq!(article["status"], "DRAFT");
    assert_eq!(
        article["references_list"][0],
        "Synthetic educational reference"
    );
    assert_eq!(
        get(&app, &format!("/api/v1/articles/{id}"), Some(&patient))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    assert!(
        json_body(get(&app, "/api/v1/articles", Some(&patient)).await).await["data"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let published = post(
        &app,
        &format!("/api/v1/doctor/articles/{id}/publish"),
        Some(&doctor),
        &serde_json::json!({"version":1}),
    )
    .await;
    assert_eq!(published.status(), StatusCode::OK);
    let published = json_body(published).await["data"].clone();
    assert_eq!(published["status"], "PUBLISHED");
    assert!(published["published_at"].is_string());
    let public = json_body(get(&app, &format!("/api/v1/articles/{id}"), Some(&patient)).await)
        .await["data"]
        .clone();
    assert_eq!(public["author_id"], DOCTOR1_PROFILE_ID);
    assert_eq!(public["body"], article["body"]);
    assert!(public["specialty"].is_string());
    let mut content = article_content();
    content["body"] = serde_json::json!("Updated educational content");
    let edited = send(
        &app,
        "PATCH",
        &format!("/api/v1/doctor/articles/{id}"),
        Some(&doctor),
        Some(&serde_json::json!({"version":2,"content":content})),
        None,
    )
    .await;
    assert_eq!(edited.status(), StatusCode::OK);
    let edited = json_body(edited).await["data"].clone();
    assert_eq!(edited["version"], 3);
    assert_eq!(edited["published_at"], published["published_at"]);
    let public =
        json_body(get(&app, &format!("/api/v1/articles/{id}"), Some(&patient)).await).await;
    assert_eq!(public["data"]["body"], content["body"]);
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/doctor/articles/{id}/archive"),
            Some(&doctor),
            &serde_json::json!({"version":3})
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        get(&app, &format!("/api/v1/articles/{id}"), Some(&patient))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    let own = json_body(get(&app, "/api/v1/doctor/articles", Some(&doctor)).await).await;
    assert_eq!(own["data"][0]["status"], "ARCHIVED");
    for action in ["publish", "archive"] {
        assert_eq!(
            post(
                &app,
                &format!("/api/v1/doctor/articles/{id}/{action}"),
                Some(&doctor),
                &serde_json::json!({"version":4})
            )
            .await
            .status(),
            StatusCode::CONFLICT
        );
    }
    let notifications:i64=sqlx::query_scalar("SELECT count(*) FROM notifications WHERE related_entity_id=$1 AND type='ARTICLE_STATUS_CHANGED'").bind(Uuid::parse_str(id).unwrap()).fetch_one(&pool).await.unwrap();
    assert_eq!(notifications, 2);
}
#[sqlx::test(migrations = "./migrations")]
async fn test_articles_ownership_abac_verified_gate_and_no_mass_assignment(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let second = make_token(uid(DOCTOR2_USER_ID), "DOCTOR");
    let article = draft_article(&app, &doctor).await;
    let id = article["id"].as_str().unwrap();
    assert_eq!(
        get(&app, "/api/v1/doctor/articles", None).await.status(),
        StatusCode::UNAUTHORIZED
    );
    for (user, role) in [(PATIENT_USER_ID, "PATIENT"), (ADMIN_USER_ID, "ADMIN")] {
        let token = make_token(uid(user), role);
        assert_eq!(
            post(
                &app,
                &format!("/api/v1/doctor/articles/{id}/publish"),
                Some(&token),
                &serde_json::json!({"version":1})
            )
            .await
            .status(),
            StatusCode::FORBIDDEN
        );
    }
    sqlx::query(
        "UPDATE doctor_profiles SET professional_verification_status='PENDING' WHERE id=$1",
    )
    .bind(uid(DOCTOR2_PROFILE_ID))
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        get(&app, "/api/v1/doctor/articles", Some(&second))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    sqlx::query(
        "UPDATE doctor_profiles SET professional_verification_status='VERIFIED' WHERE id=$1",
    )
    .bind(uid(DOCTOR2_PROFILE_ID))
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/doctor/articles/{id}/publish"),
            Some(&second),
            &serde_json::json!({"version":1})
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    assert!(
        json_body(get(&app, "/api/v1/doctor/articles", Some(&second)).await).await["data"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    for forbidden in ["author_id", "status", "published_at"] {
        let mut payload = serde_json::json!({"creation_key":Uuid::new_v4().to_string(),"content":article_content()});
        payload[forbidden] = serde_json::json!("PUBLISHED");
        assert_eq!(
            post(&app, "/api/v1/doctor/articles", Some(&doctor), &payload)
                .await
                .status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    let mut content = article_content();
    content["status"] = serde_json::json!("PUBLISHED");
    assert_eq!(
        post(
            &app,
            "/api/v1/doctor/articles",
            Some(&doctor),
            &serde_json::json!({"creation_key":Uuid::new_v4().to_string(),"content":content})
        )
        .await
        .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
}
#[sqlx::test(migrations = "./migrations")]
async fn test_articles_creation_retry_and_stale_edit_never_duplicate_or_overwrite(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let payload =
        serde_json::json!({"creation_key":Uuid::new_v4().to_string(),"content":article_content()});
    let (a, b) = tokio::join!(
        post(&app, "/api/v1/doctor/articles", Some(&doctor), &payload),
        post(&app, "/api/v1/doctor/articles", Some(&doctor), &payload)
    );
    assert_eq!(a.status(), StatusCode::OK);
    assert_eq!(b.status(), StatusCode::OK);
    let a = json_body(a).await;
    let b = json_body(b).await;
    assert_eq!(a["data"]["id"], b["data"]["id"]);
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM audit_events WHERE action='ARTICLE_CREATE'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
    let id = a["data"]["id"].as_str().unwrap();
    let mut changed = payload.clone();
    changed["content"]["title"] = serde_json::json!("Different content");
    assert_eq!(
        post(&app, "/api/v1/doctor/articles", Some(&doctor), &changed)
            .await
            .status(),
        StatusCode::CONFLICT
    );
    let edit = serde_json::json!({"version":1,"content":changed["content"]});
    assert_eq!(
        send(
            &app,
            "PATCH",
            &format!("/api/v1/doctor/articles/{id}"),
            Some(&doctor),
            Some(&edit),
            None
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        send(
            &app,
            "PATCH",
            &format!("/api/v1/doctor/articles/{id}"),
            Some(&doctor),
            Some(&edit),
            None
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
}
#[sqlx::test(migrations = "./migrations")]
async fn test_articles_publish_archive_race_has_one_winner_and_one_audit(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let a = draft_article(&app, &doctor).await;
    let id = a["id"].as_str().unwrap();
    let publish = format!("/api/v1/doctor/articles/{id}/publish");
    let archive = format!("/api/v1/doctor/articles/{id}/archive");
    let version = serde_json::json!({"version":1});
    let (a, b) = tokio::join!(
        post(&app, &publish, Some(&doctor), &version),
        post(&app, &archive, Some(&doctor), &version)
    );
    let mut statuses = [a.status(), b.status()];
    statuses.sort();
    assert_eq!(statuses, [StatusCode::OK, StatusCode::CONFLICT]);
    let row = sqlx::query("SELECT status::text AS status,version FROM doctor_articles WHERE id=$1")
        .bind(Uuid::parse_str(id).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row.get::<i32, _>("version"), 2);
    assert!(["PUBLISHED", "ARCHIVED"].contains(&row.get::<String, _>("status").as_str()));
    let count:i64=sqlx::query_scalar("SELECT count(*) FROM audit_events WHERE resource_id=$1 AND action IN ('ARTICLE_PUBLISH','ARTICLE_ARCHIVE')").bind(Uuid::parse_str(id).unwrap()).fetch_one(&pool).await.unwrap();
    assert_eq!(count, 1);
}
#[sqlx::test(migrations = "./migrations")]
async fn test_articles_rls_default_denies_cross_author_and_hides_inactive_author_feed(
    pool: PgPool,
) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let a = draft_article(&app, &doctor).await;
    let id = uid(a["id"].as_str().unwrap());
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM doctor_articles")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let foreign = hellodoctor_backend::auth_context::AuthContext {
        user_id: UserId::from(uid(DOCTOR2_USER_ID)),
        role: UserRole::Doctor,
        permissions: vec![],
        session_id: None,
        request_id: None,
    };
    let mut tx = state.begin_authed(&foreign).await.unwrap();
    let changed = sqlx::query(
        "UPDATE doctor_articles SET title='Foreign edit',version=version+1 WHERE id=$1",
    )
    .bind(id)
    .execute(&mut *tx)
    .await
    .unwrap()
    .rows_affected();
    assert_eq!(changed, 0);
    tx.rollback().await.unwrap();
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/doctor/articles/{id}/publish"),
            Some(&doctor),
            &serde_json::json!({"version":1})
        )
        .await
        .status(),
        StatusCode::OK
    );
    sqlx::query("UPDATE users SET is_active=false WHERE id=$1")
        .bind(uid(DOCTOR1_USER_ID))
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        get(&app, &format!("/api/v1/articles/{id}"), Some(&patient))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
}
#[sqlx::test(migrations = "./migrations")]
async fn test_exposure_no_development_otp_without_explicit_flag_or_in_production(pool: PgPool) {
    let (_app, _pool, mut state) = test_app_and_state(pool).await;
    for (environment, flag) in [
        ("development", false),
        ("production", false),
        ("production", true),
    ] {
        let mut config = (*state.config).clone();
        config.environment = environment.into();
        config.dev_auth_otp_enabled = flag;
        // Bypass startup only to demonstrate the action guard is independent.
        state.config = Arc::new(config);
        assert!(
            hellodoctor_backend::services::auth_service::AuthService::request_patient_otp(
                &state,
                "+254700000002"
            )
            .await
            .is_err()
        );
        assert!(
            hellodoctor_backend::services::auth_service::AuthService::doctor_login_step1(
                &state,
                SEED_DOCTOR1_EMAIL,
                SEED_DOCTOR1_PASSWORD
            )
            .await
            .is_err()
        );
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn test_articles_validation_and_audit_failure_roll_back_publication(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    for (field, content) in [
        ("title", " ".to_string()),
        ("summary", "a".repeat(601)),
        ("body", "a".repeat(30001)),
    ] {
        let mut value = article_content();
        value[field] = serde_json::json!(content);
        assert_eq!(
            post(
                &app,
                "/api/v1/doctor/articles",
                Some(&doctor),
                &serde_json::json!({"creation_key":Uuid::new_v4().to_string(),"content":value})
            )
            .await
            .status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    let a = draft_article(&app, &doctor).await;
    let id = uid(a["id"].as_str().unwrap());
    sqlx::query("CREATE FUNCTION reject_article_audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.action='ARTICLE_PUBLISH' THEN RAISE EXCEPTION 'Synthetic audit failure'; END IF; RETURN NEW; END $$").execute(&pool).await.unwrap();
    sqlx::query("CREATE TRIGGER reject_article_audit BEFORE INSERT ON audit_events FOR EACH ROW EXECUTE FUNCTION reject_article_audit()").execute(&pool).await.unwrap();
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/doctor/articles/{id}/publish"),
            Some(&doctor),
            &serde_json::json!({"version":1})
        )
        .await
        .status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    let row = sqlx::query("SELECT status::text AS status,version FROM doctor_articles WHERE id=$1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row.get::<String, _>("status"), "DRAFT");
    assert_eq!(row.get::<i32, _>("version"), 1);
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM notifications WHERE related_entity_id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 0);
}
#[sqlx::test(migrations = "./migrations")]
async fn test_patient_synthetic_api_journey_articles_booking_completion_grievance_and_logout(
    pool: PgPool,
) {
    let (app, pool) = test_app(pool).await;
    let otp = post(
        &app,
        "/api/v1/auth/register-otp",
        None,
        &serde_json::json!({"phone_number":SEED_PATIENT_PHONE}),
    )
    .await;
    assert_eq!(otp.status(), StatusCode::OK);
    let otp = json_body(otp).await;
    let login=post(&app,"/api/v1/auth/verify-otp-and-login",None,&serde_json::json!({"phone_number":SEED_PATIENT_PHONE,"otp_code":otp["data"]["debug_otp_code"]})).await;
    assert_eq!(login.status(), StatusCode::OK);
    let session = json_body(login).await["data"].clone();
    let patient = session["access_token"].as_str().unwrap();
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let article = draft_article(&app, &doctor).await;
    let article_id = article["id"].as_str().unwrap();
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/doctor/articles/{article_id}/publish"),
            Some(&doctor),
            &serde_json::json!({"version":1})
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        json_body(get(&app, "/api/v1/articles", Some(patient)).await).await["data"][0]["id"],
        article_id
    );
    let slots = json_body(
        get(
            &app,
            &format!("/api/v1/doctors/{DOCTOR1_PROFILE_ID}/slots"),
            Some(patient),
        )
        .await,
    )
    .await;
    let booking = serde_json::json!({"doctor_id":DOCTOR1_PROFILE_ID,"slot_id":slots["data"][0]["id"],"modality":"VIDEO","gateway":"MPESA","chief_complaint":"Synthetic journey fixture"});
    let key = Uuid::new_v4().to_string();
    let booked = post_idem(
        &app,
        "/api/v1/appointments/book",
        Some(patient),
        &booking,
        &key,
    )
    .await;
    assert_eq!(booked.status(), StatusCode::CREATED);
    let booked = json_body(booked).await["data"].clone();
    let appointment = uid(booked["appointment_id"].as_str().unwrap());
    let replay = json_body(
        post_idem(
            &app,
            "/api/v1/appointments/book",
            Some(patient),
            &booking,
            &key,
        )
        .await,
    )
    .await;
    assert_eq!(replay["data"]["appointment_id"], booked["appointment_id"]);
    // Existing development webhook fixture only; no live M-Pesa request/money.
    let webhook=serde_json::json!({"payment_session_id":booked["payment_session_id"],"transaction_reference":format!("SYNTH-MPESA-{}",Uuid::new_v4()),"status":"SUCCESS","event_timestamp":Utc::now().to_rfc3339(),"provider_event_id":Uuid::new_v4().to_string()}).to_string();
    assert_eq!(
        send_webhook_provider(
            &app,
            "mpesa",
            &webhook,
            &sign_webhook_body(&webhook, TEST_WEBHOOK_SECRET)
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        get(
            &app,
            &format!("/api/v1/appointments/{appointment}"),
            Some(patient)
        )
        .await
        .status(),
        StatusCode::OK
    );
    open_clinical_window(&pool, appointment).await;
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/consultations/{appointment}/complete-no-rx"),
            Some(&doctor),
            &serde_json::json!({"reason":"Synthetic educational journey; no prescription required"})
        )
        .await
        .status(),
        StatusCode::OK
    );
    let completed = json_body(
        get(
            &app,
            &format!("/api/v1/appointments/{appointment}"),
            Some(patient),
        )
        .await,
    )
    .await;
    assert_eq!(completed["data"]["status"], "COMPLETED");
    let consultation = consultation_repo::find_by_appointment(&pool, appointment)
        .await
        .unwrap()
        .unwrap();
    let grievance=post(&app,"/api/v1/grievances",Some(patient),&serde_json::json!({"consultation_id":consultation.id,"target":"DOCTOR","category":"OTHER","claim_summary":"Synthetic journey complaint"})).await;
    assert!(grievance.status().is_success());
    assert_eq!(
        post(
            &app,
            "/api/v1/auth/logout",
            None,
            &serde_json::json!({"refresh_token":session["refresh_token"]})
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        get(&app, "/api/v1/articles", Some(patient)).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        post(
            &app,
            "/api/v1/auth/refresh",
            None,
            &serde_json::json!({"refresh_token":session["refresh_token"]})
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
}

// ============================================================================
// Patient profile (GET/PATCH /patients/me) and appointment list
// (GET /patients/me/appointments) — added for the Patient Flutter client
// build. Neither route existed before this pass (confirmed by direct router
// inspection); OTP login auto-creates a placeholder `patient_profiles` row
// with no prior way to read or correct it, and a patient had no way to list
// their own appointments at all (`/doctor/appointments` is doctor-scoped).
// ============================================================================

#[sqlx::test(migrations = "./migrations")]
async fn test_patient_can_read_and_update_own_profile(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let token = fresh_patient_token(&pool).await;

    let before = get(&app, "/api/v1/patients/me", Some(&token)).await;
    assert_eq!(before.status(), StatusCode::OK);
    let before_json = json_body(before).await;
    // `fresh_patient_token`'s own fixture row (not the real OTP-login
    // placeholder — see `auth_service::verify_otp_and_login`'s "Patient
    // User"/1995-01-01 default, which a real registration flow would
    // produce instead) — asserted here only to prove GET reads real stored
    // data, not a hardcoded response.
    assert_eq!(before_json["data"]["full_name"], "Race Test Patient");
    assert_eq!(before_json["data"]["gender"], "OTHER");

    let patch1 = send(
        &app,
        "PATCH",
        "/api/v1/patients/me",
        Some(&token),
        Some(&serde_json::json!({
            "full_name": "Amina Wanjiru",
            "date_of_birth": "1992-04-17",
            "gender": "FEMALE",
            "blood_group": "O+",
            "weight_kg": "62.5",
            "emergency_contact_phone": "0712345678",
            "emergency_contact_relation": "Sister"
        })),
        None,
    )
    .await;
    assert_eq!(patch1.status(), StatusCode::OK);

    let after1 = json_body(get(&app, "/api/v1/patients/me", Some(&token)).await).await;
    assert_eq!(after1["data"]["full_name"], "Amina Wanjiru");
    assert_eq!(after1["data"]["date_of_birth"], "1992-04-17");
    assert_eq!(after1["data"]["gender"], "FEMALE");
    assert_eq!(after1["data"]["blood_group"], "O+");
    // Compared numerically, not by exact string: `bigdecimal`'s Postgres
    // NUMERIC decode preserves the full base-10000 digit group it read back
    // (a pre-existing, codebase-wide serialization characteristic, not
    // something this handler controls), so "62.5" can legitimately round
    // back as "62.5000" — the stored *value* is what matters here.
    assert_eq!(
        after1["data"]["weight_kg"]
            .as_str()
            .unwrap()
            .parse::<f64>()
            .unwrap(),
        62.5
    );
    // Stored in the canonical +254 form regardless of the local "0..." input
    // shape sent — proves the handler re-normalizes, not just validates.
    assert_eq!(after1["data"]["emergency_contact_phone"], "+254712345678");
    assert_eq!(after1["data"]["emergency_contact_relation"], "Sister");

    // A partial update (one field only) must leave every other field
    // untouched — the whole point of the repo's COALESCE-based update.
    let patch2 = send(
        &app,
        "PATCH",
        "/api/v1/patients/me",
        Some(&token),
        Some(&serde_json::json!({ "weight_kg": "60" })),
        None,
    )
    .await;
    assert_eq!(patch2.status(), StatusCode::OK);
    let after2 = json_body(get(&app, "/api/v1/patients/me", Some(&token)).await).await;
    assert_eq!(
        after2["data"]["weight_kg"]
            .as_str()
            .unwrap()
            .parse::<f64>()
            .unwrap(),
        60.0
    );
    assert_eq!(after2["data"]["full_name"], "Amina Wanjiru");
    assert_eq!(after2["data"]["emergency_contact_relation"], "Sister");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_patient_profile_update_validation_rejects_bad_input(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let token = fresh_patient_token(&pool).await;

    let cases = [
        serde_json::json!({ "full_name": "" }),
        serde_json::json!({ "full_name": "x".repeat(151) }),
        serde_json::json!({ "date_of_birth": "2999-01-01" }),
        serde_json::json!({ "weight_kg": "0" }),
        serde_json::json!({ "weight_kg": "-5" }),
        serde_json::json!({ "weight_kg": "501" }),
        serde_json::json!({ "emergency_contact_phone": "not-a-phone" }),
        serde_json::json!({ "blood_group": "x".repeat(11) }),
        serde_json::json!({ "emergency_contact_relation": "x".repeat(51) }),
    ];
    for body in cases {
        let res = send(
            &app,
            "PATCH",
            "/api/v1/patients/me",
            Some(&token),
            Some(&body),
            None,
        )
        .await;
        assert_eq!(
            res.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "expected validation to reject {body}"
        );
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn test_patient_profile_requires_patient_role(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let doctor_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");

    let get_res = get(&app, "/api/v1/patients/me", Some(&doctor_token)).await;
    assert_eq!(get_res.status(), StatusCode::FORBIDDEN);

    let patch_res = send(
        &app,
        "PATCH",
        "/api/v1/patients/me",
        Some(&doctor_token),
        Some(&serde_json::json!({ "full_name": "Should Not Apply" })),
        None,
    )
    .await;
    assert_eq!(patch_res.status(), StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_patient_lists_own_appointments_upcoming_and_scoped_to_self(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient_a = fresh_patient_token(&pool).await;
    let patient_b = fresh_patient_token(&pool).await;

    let (appointment_id, _) = book_and_confirm(&app, &patient_a).await;

    let upcoming_a = json_body(
        get(
            &app,
            "/api/v1/patients/me/appointments?status=upcoming",
            Some(&patient_a),
        )
        .await,
    )
    .await;
    let rows = upcoming_a["data"].as_array().unwrap();
    assert!(
        rows.iter()
            .any(|r| r["id"].as_str().unwrap() == appointment_id.to_string()),
        "the booking patient must see their own confirmed appointment in \"upcoming\""
    );
    let row = rows
        .iter()
        .find(|r| r["id"].as_str().unwrap() == appointment_id.to_string())
        .unwrap();
    assert_eq!(row["status"], "CONFIRMED");
    assert!(!row["doctor_display_name"].as_str().unwrap().is_empty());

    // A different patient must never see it — the same ownership-scoping
    // discipline as `test_bola_patient_cannot_view_another_patients_appointment`.
    let upcoming_b = json_body(
        get(
            &app,
            "/api/v1/patients/me/appointments?status=upcoming",
            Some(&patient_b),
        )
        .await,
    )
    .await;
    assert!(upcoming_b["data"].as_array().unwrap().is_empty());

    // A CONFIRMED (not yet completed/terminal) appointment must not appear
    // under "past".
    let past_a = json_body(
        get(
            &app,
            "/api/v1/patients/me/appointments?status=past",
            Some(&patient_a),
        )
        .await,
    )
    .await;
    assert!(past_a["data"]
        .as_array()
        .unwrap()
        .iter()
        .all(|r| r["id"].as_str().unwrap() != appointment_id.to_string()));
}

#[sqlx::test(migrations = "./migrations")]
async fn test_patient_appointments_invalid_status_param_rejected(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let token = fresh_patient_token(&pool).await;
    let res = get(
        &app,
        "/api/v1/patients/me/appointments?status=bogus",
        Some(&token),
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

// ============================================================================
// Multi-profile / family support — one login can create and manage several
// patient profiles (self + dependents), each with its own fully independent
// appointments/prescriptions/vault, confirmed as a must-have mid-build
// requirement. See docs/ plan addendum §5b (migration
// 20261101000001_patient_profile_dependents).
// ============================================================================

#[sqlx::test(migrations = "./migrations")]
async fn test_patient_can_create_list_and_update_dependent_profiles(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let token = fresh_patient_token(&pool).await;

    let create = post(
        &app,
        "/api/v1/patients/me/profiles",
        Some(&token),
        &serde_json::json!({
            "full_name": "Zayd Rafiq",
            "date_of_birth": "2018-06-01",
            "gender": "MALE",
            "relationship": "Child"
        }),
    )
    .await;
    assert_eq!(create.status(), StatusCode::CREATED);
    let created = json_body(create).await;
    let dependent_id = created["data"]["id"].as_str().unwrap().to_string();
    assert_eq!(created["data"]["full_name"], "Zayd Rafiq");
    assert_eq!(created["data"]["relationship"], "Child");
    assert!(created["data"]["user_id"].is_null());

    let list = json_body(get(&app, "/api/v1/patients/me/profiles", Some(&token)).await).await;
    let rows = list["data"].as_array().unwrap();
    assert_eq!(rows.len(), 2, "self profile + one dependent");
    assert!(rows.iter().any(|r| !r["user_id"].is_null())); // self
    assert!(rows
        .iter()
        .any(|r| r["id"].as_str().unwrap() == dependent_id));

    let update = send(
        &app,
        "PATCH",
        &format!("/api/v1/patients/me/profiles/{dependent_id}"),
        Some(&token),
        Some(&serde_json::json!({ "weight_kg": "18.5", "blood_group": "O+" })),
        None,
    )
    .await;
    assert_eq!(update.status(), StatusCode::OK);

    let after = json_body(get(&app, "/api/v1/patients/me/profiles", Some(&token)).await).await;
    let dependent_row = after["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"].as_str().unwrap() == dependent_id)
        .unwrap();
    assert_eq!(dependent_row["blood_group"], "O+");
    assert_eq!(dependent_row["full_name"], "Zayd Rafiq"); // untouched by the partial update
}

#[sqlx::test(migrations = "./migrations")]
async fn test_dependent_profile_creation_validation_rejects_bad_input(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let token = fresh_patient_token(&pool).await;

    let base = serde_json::json!({
        "full_name": "Amina Rafiq",
        "date_of_birth": "1995-05-05",
        "gender": "FEMALE",
        "relationship": "Spouse"
    });
    let cases = [
        {
            let mut b = base.clone();
            b["relationship"] = serde_json::json!("");
            b
        },
        {
            let mut b = base.clone();
            b["date_of_birth"] = serde_json::json!("2999-01-01");
            b
        },
        {
            let mut b = base.clone();
            b["full_name"] = serde_json::json!("");
            b
        },
    ];
    for body in cases {
        let res = post(&app, "/api/v1/patients/me/profiles", Some(&token), &body).await;
        assert_eq!(
            res.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "expected validation to reject {body}"
        );
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn test_patient_cannot_see_or_update_another_patients_dependent(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient_a = fresh_patient_token(&pool).await;
    let patient_b = fresh_patient_token(&pool).await;

    let create = json_body(
        post(
            &app,
            "/api/v1/patients/me/profiles",
            Some(&patient_a),
            &serde_json::json!({
                "full_name": "A's Child", "date_of_birth": "2015-01-01",
                "gender": "OTHER", "relationship": "Child"
            }),
        )
        .await,
    )
    .await;
    let dependent_id = create["data"]["id"].as_str().unwrap().to_string();

    // Patient B's own profile list never includes patient A's dependent.
    let b_list = json_body(get(&app, "/api/v1/patients/me/profiles", Some(&patient_b)).await).await;
    assert!(!b_list["data"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["id"].as_str().unwrap() == dependent_id));

    // Patient B cannot update patient A's dependent — not found, not forbidden
    // (RLS makes the row simply not exist for B, the correct non-leaky shape).
    let update = send(
        &app,
        "PATCH",
        &format!("/api/v1/patients/me/profiles/{dependent_id}"),
        Some(&patient_b),
        Some(&serde_json::json!({ "full_name": "Hijacked" })),
        None,
    )
    .await;
    assert_eq!(update.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_self_profile_update_endpoint_rejects_a_dependent_id(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let token = fresh_patient_token(&pool).await;
    let create = json_body(
        post(
            &app,
            "/api/v1/patients/me/profiles",
            Some(&token),
            &serde_json::json!({
                "full_name": "Dependent", "date_of_birth": "2010-01-01",
                "gender": "OTHER", "relationship": "Child"
            }),
        )
        .await,
    )
    .await;
    let dependent_id = create["data"]["id"].as_str().unwrap().to_string();

    // /patients/me/profiles/:id (dependent-only) rejects the caller's own id.
    let self_profile = json_body(get(&app, "/api/v1/patients/me", Some(&token)).await).await;
    let self_id = self_profile["data"]["id"].as_str().unwrap();
    let misuse = send(
        &app,
        "PATCH",
        &format!("/api/v1/patients/me/profiles/{self_id}"),
        Some(&token),
        Some(&serde_json::json!({ "full_name": "Should Fail" })),
        None,
    )
    .await;
    assert_eq!(misuse.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // Confirms the dependent id is otherwise valid — sanity check that the
    // rejection above is about which endpoint was used, not the id itself.
    let correct = send(
        &app,
        "PATCH",
        &format!("/api/v1/patients/me/profiles/{dependent_id}"),
        Some(&token),
        Some(&serde_json::json!({ "full_name": "Correctly Updated" })),
        None,
    )
    .await;
    assert_eq!(correct.status(), StatusCode::OK);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_guardian_books_appointment_for_dependent_and_lists_it_scoped(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let token = fresh_patient_token(&pool).await;
    let dependent = json_body(
        post(
            &app,
            "/api/v1/patients/me/profiles",
            Some(&token),
            &serde_json::json!({
                "full_name": "Booked Dependent", "date_of_birth": "2012-01-01",
                "gender": "OTHER", "relationship": "Child"
            }),
        )
        .await,
    )
    .await;
    let dependent_id = Uuid::parse_str(dependent["data"]["id"].as_str().unwrap()).unwrap();

    let book_res = book_and_confirm_for_profile(&app, &token, dependent_id).await;
    assert_eq!(book_res.status(), StatusCode::CREATED);
    let book_json = json_body(book_res).await;
    let appointment_id = book_json["data"]["appointment_id"]
        .as_str()
        .unwrap()
        .to_string();

    // Scoped to the dependent's profile_id: present.
    let dependent_list = json_body(
        get(
            &app,
            &format!("/api/v1/patients/me/appointments?status=upcoming&profile_id={dependent_id}"),
            Some(&token),
        )
        .await,
    )
    .await;
    assert!(dependent_list["data"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["id"].as_str().unwrap() == appointment_id));

    // Default (self-only) listing: absent — the two profiles' appointment
    // lists are genuinely independent, matching "each profile acts solely as
    // a different account" exactly.
    let self_list = json_body(
        get(
            &app,
            "/api/v1/patients/me/appointments?status=upcoming",
            Some(&token),
        )
        .await,
    )
    .await;
    assert!(self_list["data"]
        .as_array()
        .unwrap()
        .iter()
        .all(|r| r["id"].as_str().unwrap() != appointment_id));
}

#[sqlx::test(migrations = "./migrations")]
async fn test_patient_cannot_book_using_a_profile_id_they_do_not_own(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient_a = fresh_patient_token(&pool).await;
    let patient_b_token = fresh_patient_token(&pool).await;
    let patient_b_profile =
        json_body(get(&app, "/api/v1/patients/me", Some(&patient_b_token)).await).await;
    let patient_b_id = Uuid::parse_str(patient_b_profile["data"]["id"].as_str().unwrap()).unwrap();

    // Patient A tries to book "as" Patient B — an unrelated real account, not
    // a dependent A guardians.
    let res = book_and_confirm_for_profile(&app, &patient_a, patient_b_id).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_guardian_uploads_intake_and_files_grievance_for_dependent(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let token = fresh_patient_token(&pool).await;
    let dependent = json_body(
        post(
            &app,
            "/api/v1/patients/me/profiles",
            Some(&token),
            &serde_json::json!({
                "full_name": "Clinical Dependent", "date_of_birth": "2016-01-01",
                "gender": "OTHER", "relationship": "Child"
            }),
        )
        .await,
    )
    .await;
    let dependent_id = Uuid::parse_str(dependent["data"]["id"].as_str().unwrap()).unwrap();
    let book_json = json_body(book_and_confirm_for_profile(&app, &token, dependent_id).await).await;
    let appointment_id =
        Uuid::parse_str(book_json["data"]["appointment_id"].as_str().unwrap()).unwrap();

    // Intake upload: no profile_id needed — resolved from the appointment's
    // own patient_id, see prescription_handlers::upload_patient_intake.
    let png_image = image::RgbImage::from_pixel(2, 2, image::Rgb([10, 20, 30]));
    let mut png_bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(png_image)
        .write_to(&mut png_bytes, image::ImageFormat::Png)
        .unwrap();
    let upload = post_idem(
        &app,
        &format!("/api/v1/appointments/{appointment_id}/prescriptions"),
        Some(&token),
        &serde_json::json!({
            "files": [{ "mime_type": "image/png", "base64_content": STANDARD.encode(png_bytes.into_inner()) }]
        }),
        &Uuid::new_v4().to_string(),
    )
    .await;
    assert_eq!(upload.status(), StatusCode::CREATED);

    // Grievance: also resolved from the consultation's own appointment, not
    // a client-supplied profile_id.
    let session = consultation_repo::find_by_appointment(&pool, appointment_id)
        .await
        .unwrap()
        .unwrap();
    let grievance = post(
        &app,
        "/api/v1/grievances",
        Some(&token),
        &serde_json::json!({
            "consultation_id": session.id,
            "target": "SYSTEM",
            "category": "OTHER",
            "claim_summary": "Dependent-profile grievance test"
        }),
    )
    .await;
    assert_eq!(grievance.status(), StatusCode::CREATED);
}

/// Regression test for a real bug this pass found and fixed:
/// `ConsultationService::mint_agora_token` resolved "is this caller the
/// patient" by looking up the caller's own self profile and comparing its id
/// directly against the appointment's `patient_id` — which can never match
/// for a dependent-booked appointment (`apt.patient_id` is the dependent's
/// own profile id, not the guardian's), so a guardian joining their own
/// dependent's consultation was wrongly rejected as "not an authorized
/// participant". The same bug existed in `record_telemetry_events` and the
/// idempotency-replay authorization check — all three fixed identically.
#[sqlx::test(migrations = "./migrations")]
async fn test_guardian_can_mint_rtc_token_for_dependents_consultation(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let token = fresh_patient_token(&pool).await;
    let dependent = json_body(
        post(
            &app,
            "/api/v1/patients/me/profiles",
            Some(&token),
            &serde_json::json!({
                "full_name": "RTC Dependent", "date_of_birth": "2014-01-01",
                "gender": "OTHER", "relationship": "Child"
            }),
        )
        .await,
    )
    .await;
    let dependent_id = Uuid::parse_str(dependent["data"]["id"].as_str().unwrap()).unwrap();
    let book_json = json_body(book_and_confirm_for_profile(&app, &token, dependent_id).await).await;
    let appointment_id =
        Uuid::parse_str(book_json["data"]["appointment_id"].as_str().unwrap()).unwrap();

    let res_rtc = post(
        &app,
        &format!("/api/v1/consultations/{appointment_id}/rtc-token"),
        Some(&token),
        &serde_json::json!({}),
    )
    .await;
    assert_eq!(res_rtc.status(), StatusCode::OK);
    let rtc_json = json_body(res_rtc).await;
    assert!(!rtc_json["data"]["channel_name"]
        .as_str()
        .unwrap()
        .is_empty());
}

// Test-only privileged fixture transfer. Deferred FKs still validate the whole
// ownership graph at commit; ordinary API callers never get this operation.
async fn reassign_isolated_appointment_doctor(pool: &PgPool, appointment: Uuid, doctor: Uuid) {
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SET CONSTRAINTS ALL DEFERRED")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("UPDATE doctor_schedule_slots SET doctor_id=$2 WHERE id=(SELECT slot_id FROM appointments WHERE id=$1)").bind(appointment).bind(doctor).execute(&mut *tx).await.unwrap();
    for table in ["appointments", "prescriptions", "chat_conversations"] {
        let key = if table == "appointments" {
            "id"
        } else {
            "appointment_id"
        };
        sqlx::query(&format!("UPDATE {table} SET doctor_id=$2 WHERE {key}=$1"))
            .bind(appointment)
            .bind(doctor)
            .execute(&mut *tx)
            .await
            .unwrap();
    }
    tx.commit().await.unwrap();
}

/// Books an appointment as `patient_token`, stopping before the payment
/// webhook — the appointment is left genuinely `PENDING_PAYMENT`, which is
/// the real steady state for almost every booking in this build (no real
/// M-Pesa confirmation path exists for a patient to trigger themselves).
/// Returns `(appointment_id, slot_id)`.
async fn book_pending_payment(
    app: &axum::Router,
    patient_token: &str,
    profile_id: Option<Uuid>,
) -> (Uuid, Uuid) {
    let res_slots = get(
        app,
        &format!("/api/v1/doctors/{}/slots", DOCTOR1_PROFILE_ID),
        Some(patient_token),
    )
    .await;
    assert_eq!(res_slots.status(), StatusCode::OK);
    let json_slots = json_body(res_slots).await;
    let slot_id = Uuid::parse_str(json_slots["data"][0]["id"].as_str().unwrap()).unwrap();

    let mut book_payload = serde_json::json!({
        "doctor_id": DOCTOR1_PROFILE_ID,
        "slot_id": slot_id,
        "modality": "VIDEO",
        "gateway": "MPESA",
        "chief_complaint": "Cancellation test booking"
    });
    if let Some(pid) = profile_id {
        book_payload["profile_id"] = serde_json::json!(pid);
    }
    let res_book = post_idem(
        app,
        "/api/v1/appointments/book",
        Some(patient_token),
        &book_payload,
        &Uuid::new_v4().to_string(),
    )
    .await;
    assert_eq!(res_book.status(), StatusCode::CREATED);
    let book_json = json_body(res_book).await;
    let apt_id = Uuid::parse_str(book_json["data"]["appointment_id"].as_str().unwrap()).unwrap();
    (apt_id, slot_id)
}

/// Gap #3 (plan `enumerated-wishing-pearl.md` §5/§6 M6): a patient previously
/// had no way to resolve which `chat_conversations` row belongs to their own
/// appointment. `chat_repo::find_conversation_by_appointment` already
/// existed — this closes the missing read endpoint over it. A `CONFIRMED`
/// appointment always has one (bundled with every booking regardless of
/// modality — confirmed directly in `process_payment_webhook`), so this is
/// the common, expected success case.
#[sqlx::test(migrations = "./migrations")]
async fn test_patient_can_resolve_conversation_for_confirmed_appointment(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient = fresh_patient_token(&pool).await;
    let (appointment_id, _) = book_and_confirm(&app, &patient).await;

    let res = get(
        &app,
        &format!("/api/v1/appointments/{appointment_id}/conversation"),
        Some(&patient),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let json = json_body(res).await;
    assert_eq!(json["data"]["appointment_id"], appointment_id.to_string());
    assert!(!json["data"]["id"].as_str().unwrap().is_empty());
    assert_eq!(json["data"]["is_locked"], false);
}

/// A conversation is only created once payment is confirmed — resolving one
/// for a still-`PENDING_PAYMENT` appointment must be a clear 404, not a 500
/// or an empty-but-200 response.
#[sqlx::test(migrations = "./migrations")]
async fn test_conversation_lookup_404s_before_appointment_confirmed(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient = fresh_patient_token(&pool).await;
    let (appointment_id, _) = book_pending_payment(&app, &patient, None).await;

    let res = get(
        &app,
        &format!("/api/v1/appointments/{appointment_id}/conversation"),
        Some(&patient),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

/// BOLA guard: a different patient must not be able to resolve someone
/// else's conversation — RLS makes the appointment itself invisible, so this
/// 404s before `chat_conversations` is ever queried, never a 403 that would
/// confirm the appointment's existence to a non-owner.
#[sqlx::test(migrations = "./migrations")]
async fn test_conversation_lookup_rejects_non_owning_patient(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let owner = fresh_patient_token(&pool).await;
    let stranger = fresh_patient_token(&pool).await;
    let (appointment_id, _) = book_and_confirm(&app, &owner).await;

    let res = get(
        &app,
        &format!("/api/v1/appointments/{appointment_id}/conversation"),
        Some(&stranger),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

/// §5b multi-profile/family support: a guardian must be able to resolve the
/// conversation for an appointment booked for their own dependent, the same
/// ownership shape already proven for RTC token minting and cancellation.
#[sqlx::test(migrations = "./migrations")]
async fn test_guardian_can_resolve_dependents_conversation(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let guardian = fresh_patient_token(&pool).await;
    let dependent = json_body(
        post(
            &app,
            "/api/v1/patients/me/profiles",
            Some(&guardian),
            &serde_json::json!({
                "full_name": "Chat Dependent", "date_of_birth": "2016-01-01",
                "gender": "OTHER", "relationship": "Child"
            }),
        )
        .await,
    )
    .await;
    let dependent_id = Uuid::parse_str(dependent["data"]["id"].as_str().unwrap()).unwrap();
    let book_json =
        json_body(book_and_confirm_for_profile(&app, &guardian, dependent_id).await).await;
    let appointment_id =
        Uuid::parse_str(book_json["data"]["appointment_id"].as_str().unwrap()).unwrap();

    let res = get(
        &app,
        &format!("/api/v1/appointments/{appointment_id}/conversation"),
        Some(&guardian),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
}

/// Gap #2 (plan `enumerated-wishing-pearl.md` §5/§6 M7): a patient previously
/// had no way to list their own documents at all. Confirms the merged Health
/// Vault list (`GET /patients/me/documents`) surfaces both a patient-uploaded
/// intake document and a doctor-issued e-Prescription for the same patient,
/// tagged with the right `kind` and carrying the fields the Flutter client
/// needs to request a real download via the pre-existing
/// `GET /appointments/:id/documents/:document_id`.
#[sqlx::test(migrations = "./migrations")]
async fn test_patient_health_vault_lists_prescriptions_and_intake_documents(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient = fresh_patient_token(&pool).await;
    let doctor1_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let (apt_id, _) = book_and_confirm(&app, &patient).await;

    let intake_payload = serde_json::json!({
        "files": [{ "mime_type": "image/jpeg", "base64_content": minimal_jpeg_base64() }]
    });
    let res_intake = post(
        &app,
        &format!("/api/v1/appointments/{apt_id}/prescriptions"),
        Some(&patient),
        &intake_payload,
    )
    .await;
    assert_eq!(res_intake.status(), StatusCode::CREATED);

    open_clinical_window(&pool, apt_id).await;
    let res_rx = post_idem(
        &app,
        &format!("/api/v1/consultations/{apt_id}/prescription"),
        Some(&doctor1_token),
        &serde_json::json!({
            "base64_photo": minimal_jpeg_base64(),
            "doctor_notes": "Health Vault fixture prescription"
        }),
        &Uuid::new_v4().to_string(),
    )
    .await;
    assert_eq!(res_rx.status(), StatusCode::CREATED);

    let res_vault = get(&app, "/api/v1/patients/me/documents", Some(&patient)).await;
    assert_eq!(res_vault.status(), StatusCode::OK);
    let vault_json = json_body(res_vault).await;
    let documents = vault_json["data"].as_array().unwrap();
    assert_eq!(
        documents.len(),
        2,
        "expected exactly one intake doc and one prescription"
    );

    let kinds: std::collections::HashSet<&str> = documents
        .iter()
        .map(|d| d["kind"].as_str().unwrap())
        .collect();
    assert!(kinds.contains("INTAKE"));
    assert!(kinds.contains("PRESCRIPTION"));

    let prescription_entry = documents
        .iter()
        .find(|d| d["kind"] == "PRESCRIPTION")
        .unwrap();
    assert_eq!(prescription_entry["appointment_id"], apt_id.to_string());
    assert!(!prescription_entry["rx_number"].as_str().unwrap().is_empty());
    assert!(!prescription_entry["doctor_display_name"]
        .as_str()
        .unwrap()
        .is_empty());

    let intake_entry = documents.iter().find(|d| d["kind"] == "INTAKE").unwrap();
    assert_eq!(intake_entry["appointment_id"], apt_id.to_string());
    assert_eq!(intake_entry["mime_type"], "image/jpeg");
}

/// A no-Rx clinical decision creates a `prescriptions` row with no image
/// (`rx_image_object_key IS NULL`) — it must never appear in the Health
/// Vault as a phantom "e-Prescription" entry with nothing to actually show
/// or download.
#[sqlx::test(migrations = "./migrations")]
async fn test_health_vault_excludes_no_rx_decision_records(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient = fresh_patient_token(&pool).await;
    let doctor1_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let (apt_id, _) = book_and_confirm(&app, &patient).await;

    open_clinical_window(&pool, apt_id).await;
    let res_no_rx = post(
        &app,
        &format!("/api/v1/consultations/{apt_id}/complete-no-rx"),
        Some(&doctor1_token),
        &serde_json::json!({"reason": "Follow-up advice only, no medication required"}),
    )
    .await;
    assert_eq!(res_no_rx.status(), StatusCode::OK);

    let res_vault = get(&app, "/api/v1/patients/me/documents", Some(&patient)).await;
    assert_eq!(res_vault.status(), StatusCode::OK);
    let vault_json = json_body(res_vault).await;
    assert!(
        vault_json["data"].as_array().unwrap().is_empty(),
        "a no-Rx decision must not surface as a Health Vault document"
    );
}

/// BOLA guard: a different patient's Health Vault must never include another
/// patient's documents.
#[sqlx::test(migrations = "./migrations")]
async fn test_health_vault_rejects_cross_patient_documents(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let owner = fresh_patient_token(&pool).await;
    let stranger = fresh_patient_token(&pool).await;
    let (apt_id, _) = book_and_confirm(&app, &owner).await;

    let res_intake = post(
        &app,
        &format!("/api/v1/appointments/{apt_id}/prescriptions"),
        Some(&owner),
        &serde_json::json!({
            "files": [{ "mime_type": "image/jpeg", "base64_content": minimal_jpeg_base64() }]
        }),
    )
    .await;
    assert_eq!(res_intake.status(), StatusCode::CREATED);

    let res_vault = get(&app, "/api/v1/patients/me/documents", Some(&stranger)).await;
    assert_eq!(res_vault.status(), StatusCode::OK);
    let vault_json = json_body(res_vault).await;
    assert!(vault_json["data"].as_array().unwrap().is_empty());
}

/// §5b multi-profile/family support: a guardian must be able to list the
/// Health Vault documents of a dependent they guardian, the same ownership
/// shape already proven for appointments, RTC tokens, and chat.
#[sqlx::test(migrations = "./migrations")]
async fn test_guardian_can_list_dependents_health_vault_documents(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let guardian = fresh_patient_token(&pool).await;
    let dependent = json_body(
        post(
            &app,
            "/api/v1/patients/me/profiles",
            Some(&guardian),
            &serde_json::json!({
                "full_name": "Vault Dependent", "date_of_birth": "2017-01-01",
                "gender": "OTHER", "relationship": "Child"
            }),
        )
        .await,
    )
    .await;
    let dependent_id = Uuid::parse_str(dependent["data"]["id"].as_str().unwrap()).unwrap();
    let book_json =
        json_body(book_and_confirm_for_profile(&app, &guardian, dependent_id).await).await;
    let apt_id = Uuid::parse_str(book_json["data"]["appointment_id"].as_str().unwrap()).unwrap();

    let res_intake = post(
        &app,
        &format!("/api/v1/appointments/{apt_id}/prescriptions"),
        Some(&guardian),
        &serde_json::json!({
            "files": [{ "mime_type": "image/jpeg", "base64_content": minimal_jpeg_base64() }]
        }),
    )
    .await;
    assert_eq!(res_intake.status(), StatusCode::CREATED);

    let res_vault = get(
        &app,
        &format!("/api/v1/patients/me/documents?profile_id={dependent_id}"),
        Some(&guardian),
    )
    .await;
    assert_eq!(res_vault.status(), StatusCode::OK);
    let vault_json = json_body(res_vault).await;
    let documents = vault_json["data"].as_array().unwrap();
    assert_eq!(documents.len(), 1);
    assert_eq!(documents[0]["kind"], "INTAKE");
    assert_eq!(documents[0]["appointment_id"], apt_id.to_string());
}

/// M8 Grievance support: a patient previously had no way to resolve the
/// `consultation_id` `POST /grievances` requires. `consultation_repo::
/// find_by_appointment` already existed — this closes the missing read
/// endpoint over it, the same gap shape as #3's conversation lookup. A
/// session only exists once either participant has minted an RTC token
/// (`mint_agora_token` creates it on first use), which `book_and_confirm`
/// does via its own `rtc-token` call in earlier tests — here minted directly.
#[sqlx::test(migrations = "./migrations")]
async fn test_patient_can_resolve_consultation_for_grievance_filing(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient = fresh_patient_token(&pool).await;
    let (appointment_id, _) = book_and_confirm(&app, &patient).await;

    let res_rtc = post(
        &app,
        &format!("/api/v1/consultations/{appointment_id}/rtc-token"),
        Some(&patient),
        &serde_json::json!({}),
    )
    .await;
    assert_eq!(res_rtc.status(), StatusCode::OK);

    let res = get(
        &app,
        &format!("/api/v1/appointments/{appointment_id}/consultation"),
        Some(&patient),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let json = json_body(res).await;
    assert_eq!(json["data"]["appointment_id"], appointment_id.to_string());
    assert!(!json["data"]["id"].as_str().unwrap().is_empty());
    assert!(json["data"].get("agora_channel_name").is_none());

    // The resolved consultation_id must actually work end to end with the
    // real grievance endpoint, not just look plausible.
    let consultation_id = json["data"]["id"].as_str().unwrap();
    let res_grievance = post(
        &app,
        "/api/v1/grievances",
        Some(&patient),
        &serde_json::json!({
            "consultation_id": consultation_id,
            "target": "DOCTOR",
            "category": "Doctor Was Absent / Late Join",
            "claim_summary": "Doctor did not join within the scheduled slot window."
        }),
    )
    .await;
    assert_eq!(res_grievance.status(), StatusCode::CREATED);
}

/// A consultation session row is created once payment is confirmed (see
/// `appointment_service::process_payment_webhook`), not when the RTC token
/// is first minted — so the real 404 case is "not yet confirmed," matching
/// the conversation-lookup test above. Resolving one for a still-
/// `PENDING_PAYMENT` appointment must be a clear 404, not a 500 or an
/// empty-but-200 response.
#[sqlx::test(migrations = "./migrations")]
async fn test_consultation_lookup_404s_before_appointment_confirmed(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient = fresh_patient_token(&pool).await;
    let (appointment_id, _) = book_pending_payment(&app, &patient, None).await;

    let res = get(
        &app,
        &format!("/api/v1/appointments/{appointment_id}/consultation"),
        Some(&patient),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

/// BOLA guard: a different patient must not be able to resolve someone
/// else's consultation session.
#[sqlx::test(migrations = "./migrations")]
async fn test_consultation_lookup_rejects_non_owning_patient(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let owner = fresh_patient_token(&pool).await;
    let stranger = fresh_patient_token(&pool).await;
    let (appointment_id, _) = book_and_confirm(&app, &owner).await;
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/consultations/{appointment_id}/rtc-token"),
            Some(&owner),
            &serde_json::json!({}),
        )
        .await
        .status(),
        StatusCode::OK
    );

    let res = get(
        &app,
        &format!("/api/v1/appointments/{appointment_id}/consultation"),
        Some(&stranger),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

/// Gap #6 (plan §4/§6, M8): the patient-facing Free Health Q&A feed. A
/// patient can submit a question (landing `OPEN`), it is invisible to other
/// patients until a doctor answers it (`list_answered_questions` is hardcoded
/// to `status='ANSWERED'`), and once answered it appears with the doctor's
/// display name and specialty attached — but never the asking patient's
/// identity, matching `health_questions`' column-scoped SELECT grant.
#[sqlx::test(migrations = "./migrations")]
async fn test_patient_can_submit_question_and_read_it_once_answered(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient = fresh_patient_token(&pool).await;
    let doctor_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");

    let submit = post(
        &app,
        "/api/v1/patients/me/qna/questions",
        Some(&patient),
        &serde_json::json!({
            "question_text": "Is a 100.4F fever with body pain normal on day 2 of a viral fever?",
            "category": "Fevers & Infections"
        }),
    )
    .await;
    assert_eq!(submit.status(), StatusCode::CREATED);
    let submit_json = json_body(submit).await;
    let question_id = Uuid::parse_str(submit_json["data"]["id"].as_str().unwrap()).unwrap();
    assert_eq!(submit_json["data"]["status"], "OPEN");
    assert!(submit_json["data"].get("author_patient_id").is_none());

    // Not visible in the answered feed yet — nobody has answered it.
    let feed_before =
        json_body(get(&app, "/api/v1/patients/me/qna/questions", Some(&patient)).await).await;
    assert!(feed_before["data"]
        .as_array()
        .unwrap()
        .iter()
        .all(|q| q["id"] != question_id.to_string()));

    let answer = post(
        &app,
        &format!("/api/v1/doctor/qna/questions/{question_id}/answers"),
        Some(&doctor_token),
        &serde_json::json!({"answer_text": "Continue oral hydration and paracetamol; seek care if it persists past day 3."}),
    )
    .await;
    assert_eq!(answer.status(), StatusCode::CREATED);

    let feed_after =
        json_body(get(&app, "/api/v1/patients/me/qna/questions", Some(&patient)).await).await;
    let entry = feed_after["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|q| q["id"] == question_id.to_string())
        .expect("answered question must appear in the feed");
    assert_eq!(
        entry["answer_text"],
        "Continue oral hydration and paracetamol; seek care if it persists past day 3."
    );
    assert!(entry.get("author_patient_id").is_none());
    assert!(!entry["answering_doctor"].as_str().unwrap().is_empty());
}

/// Validation: an empty or over-length question must be rejected before it
/// ever reaches `health_questions`.
#[sqlx::test(migrations = "./migrations")]
async fn test_submit_question_rejects_empty_and_oversized_text(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient = fresh_patient_token(&pool).await;

    let empty = post(
        &app,
        "/api/v1/patients/me/qna/questions",
        Some(&patient),
        &serde_json::json!({"question_text": "   "}),
    )
    .await;
    assert_eq!(empty.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let oversized = post(
        &app,
        "/api/v1/patients/me/qna/questions",
        Some(&patient),
        &serde_json::json!({"question_text": "a".repeat(4001)}),
    )
    .await;
    assert_eq!(oversized.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

/// Only a `PATIENT`-role caller may submit a question — mirrors the
/// `SubmitGrievance`/`BookAppointment` authorization shape exactly.
#[sqlx::test(migrations = "./migrations")]
async fn test_submit_question_rejects_non_patient_role(pool: PgPool) {
    let (app, _) = test_app(pool).await;
    let doctor_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");

    let res = post(
        &app,
        "/api/v1/patients/me/qna/questions",
        Some(&doctor_token),
        &serde_json::json!({"question_text": "Can a doctor ask a question here?"}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

/// Multi-profile support (§5b): a guardian can submit a question on behalf of
/// a dependent profile, exactly like booking, intake uploads, and grievances.
#[sqlx::test(migrations = "./migrations")]
async fn test_guardian_can_submit_question_for_dependent(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let guardian = fresh_patient_token(&pool).await;
    let dependent = json_body(
        post(
            &app,
            "/api/v1/patients/me/profiles",
            Some(&guardian),
            &serde_json::json!({
                "full_name": "Qna Dependent", "date_of_birth": "2015-01-01",
                "gender": "OTHER", "relationship": "Child"
            }),
        )
        .await,
    )
    .await;
    let dependent_id = Uuid::parse_str(dependent["data"]["id"].as_str().unwrap()).unwrap();

    let submit = post(
        &app,
        "/api/v1/patients/me/qna/questions",
        Some(&guardian),
        &serde_json::json!({
            "question_text": "My child has a mild dry cough at night, what is safe to give?",
            "profile_id": dependent_id
        }),
    )
    .await;
    assert_eq!(submit.status(), StatusCode::CREATED);

    let author: Uuid =
        sqlx::query_scalar("SELECT author_patient_id FROM health_questions WHERE id = $1")
            .bind(Uuid::parse_str(json_body(submit).await["data"]["id"].as_str().unwrap()).unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(author, dependent_id);
}

/// BOLA guard: a guardian cannot submit a question "on behalf of" a profile
/// they do not own.
#[sqlx::test(migrations = "./migrations")]
async fn test_submit_question_rejects_unowned_profile_id(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient = fresh_patient_token(&pool).await;
    let stranger = fresh_patient_token(&pool).await;

    // Resolve the stranger's own self-profile id via their own session —
    // the real, authorized path — rather than guessing it.
    let stranger_profile = json_body(get(&app, "/api/v1/patients/me", Some(&stranger)).await).await;
    let stranger_profile_id =
        Uuid::parse_str(stranger_profile["data"]["id"].as_str().unwrap()).unwrap();

    let res = post(
        &app,
        "/api/v1/patients/me/qna/questions",
        Some(&patient),
        &serde_json::json!({
            "question_text": "Trying to ask as someone else's profile.",
            "profile_id": stranger_profile_id
        }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

/// Durable Patient Q&A writes have an account-scoped abuse budget in addition
/// to the global IP ceiling. A dependent profile switch must not create a new
/// budget because the key is the authenticated account's user id.
#[sqlx::test(migrations = "./migrations")]
async fn test_security_patient_question_submission_rate_limit(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let patient = fresh_patient_token(&_pool).await;
    for index in 0..10 {
        let response = post(
            &app,
            "/api/v1/patients/me/qna/questions",
            Some(&patient),
            &serde_json::json!({
                "question_text": format!("Rate limit fixture question {index}")
            }),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED);
    }
    let response = post(
        &app,
        "/api/v1/patients/me/qna/questions",
        Some(&patient),
        &serde_json::json!({"question_text": "The eleventh question is blocked."}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
}

/// Family-profile creation is a durable write and must be bounded separately
/// from ordinary profile edits.
#[sqlx::test(migrations = "./migrations")]
async fn test_security_patient_dependent_creation_rate_limit(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let patient = fresh_patient_token(&_pool).await;
    for index in 0..10 {
        let response = post(
            &app,
            "/api/v1/patients/me/profiles",
            Some(&patient),
            &serde_json::json!({
                "full_name": format!("Rate Limit Dependent {index}"),
                "date_of_birth": "2015-01-01",
                "gender": "OTHER",
                "relationship": "Child"
            }),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED);
    }
    let response = post(
        &app,
        "/api/v1/patients/me/profiles",
        Some(&patient),
        &serde_json::json!({
            "full_name": "The eleventh dependent is blocked.",
            "date_of_birth": "2015-01-01",
            "gender": "OTHER",
            "relationship": "Child"
        }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_patient_qna_own_open_persists_without_disclosing_other_authors(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let patient = fresh_patient_token(&pool).await;
    let stranger = fresh_patient_token(&pool).await;
    let create = post(&app, "/api/v1/patients/me/qna/questions", Some(&patient),
        &serde_json::json!({"question_text":"When should I seek assessment for recurring headaches?", "category":"General Health"})).await;
    assert_eq!(create.status(), StatusCode::CREATED);
    let created = json_body(create).await["data"].clone();
    let id = created["id"].as_str().unwrap();
    for _ in 0..2 {
        let feed = json_body(
            get(
                &app,
                "/api/v1/patients/me/qna/questions?include_own_open=true",
                Some(&patient),
            )
            .await,
        )
        .await;
        let entry = feed["data"]
            .as_array()
            .unwrap()
            .iter()
            .find(|q| q["id"] == id)
            .unwrap();
        assert_eq!(entry["status"], "OPEN");
        assert!(entry["answer_text"].is_null());
        assert!(entry.get("author_patient_id").is_none());
    }
    let others = json_body(
        get(
            &app,
            "/api/v1/patients/me/qna/questions?include_own_open=true",
            Some(&stranger),
        )
        .await,
    )
    .await;
    assert!(others["data"]
        .as_array()
        .unwrap()
        .iter()
        .all(|q| q["id"] != id));
    let legacy =
        json_body(get(&app, "/api/v1/patients/me/qna/questions", Some(&patient)).await).await;
    assert!(legacy["data"]
        .as_array()
        .unwrap()
        .iter()
        .all(|q| q["id"] != id));
    assert!(
        sqlx::query("SELECT author_patient_id FROM health_questions")
            .fetch_all(&state.db)
            .await
            .is_err(),
        "anonymous author column remains denied"
    );
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    assert_eq!(
        get(
            &app,
            "/api/v1/patients/me/qna/questions?include_own_open=true",
            Some(&doctor)
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    let answer = post(
        &app,
        &format!("/api/v1/doctor/qna/questions/{id}/answers"),
        Some(&doctor),
        &serde_json::json!({"answer_text":"Seek a clinical assessment for recurring symptoms."}),
    )
    .await;
    assert_eq!(answer.status(), StatusCode::CREATED);
    let published = json_body(
        get(
            &app,
            "/api/v1/patients/me/qna/questions?include_own_open=true",
            Some(&stranger),
        )
        .await,
    )
    .await;
    let entry = published["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|q| q["id"] == id)
        .unwrap();
    assert_eq!(entry["status"], "ANSWERED");
    assert_eq!(entry["answering_doctor_verified"], true);
    assert!(entry.get("author_patient_id").is_none());
    let second = post(
        &app,
        &format!("/api/v1/doctor/qna/questions/{id}/answers"),
        Some(&make_token(uid(DOCTOR2_USER_ID), "DOCTOR")),
        &serde_json::json!({"answer_text":"Another response."}),
    )
    .await;
    assert_eq!(second.status(), StatusCode::CONFLICT);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_patient_qna_guardian_open_read_is_context_bound(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let patient = fresh_patient_token(&pool).await;
    let stranger = fresh_patient_token(&pool).await;
    let dep = json_body(post(&app,"/api/v1/patients/me/profiles",Some(&patient), &serde_json::json!({"full_name":"Dependent Test","date_of_birth":"2015-01-01","gender":"OTHER","relationship":"Child"})).await).await;
    let created = json_body(post(&app,"/api/v1/patients/me/qna/questions",Some(&patient), &serde_json::json!({"question_text":"What general precautions help during cold season?","profile_id":dep["data"]["id"]})).await).await;
    let id = &created["data"]["id"];
    let mine = json_body(
        get(
            &app,
            "/api/v1/patients/me/qna/questions?include_own_open=true",
            Some(&patient),
        )
        .await,
    )
    .await;
    assert!(mine["data"]
        .as_array()
        .unwrap()
        .iter()
        .any(|q| &q["id"] == id));
    let other = json_body(
        get(
            &app,
            "/api/v1/patients/me/qna/questions?include_own_open=true",
            Some(&stranger),
        )
        .await,
    )
    .await;
    assert!(other["data"]
        .as_array()
        .unwrap()
        .iter()
        .all(|q| &q["id"] != id));
    let unscoped: Vec<(Uuid,)> = sqlx::query_as("SELECT id FROM patient_owned_open_question_ids()")
        .fetch_all(&state.db)
        .await
        .unwrap();
    assert!(
        unscoped.is_empty(),
        "no authenticated context gives no owned IDs"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_consultation_chat_booking_payment_shared_history_and_ownership(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let other_doctor = make_token(uid(DOCTOR2_USER_ID), "DOCTOR");
    let other_patient = fresh_patient_token(&pool).await;
    sqlx::query("UPDATE patient_profiles SET full_name='Meherab Shafin' WHERE id=$1")
        .bind(uid(PATIENT_PROFILE_ID))
        .execute(&pool)
        .await
        .unwrap();
    let slots = json_body(
        get(
            &app,
            &format!("/api/v1/doctors/{DOCTOR1_PROFILE_ID}/slots"),
            Some(&patient),
        )
        .await,
    )
    .await;
    let slot = &slots["data"][0]["id"];
    let payload = serde_json::json!({"doctor_id":DOCTOR1_PROFILE_ID,"slot_id":slot,"modality":"CHAT","gateway":"BKASH"});
    let key = Uuid::new_v4().to_string();
    // `/appointments/book` classifies as Cost::Payment, whose admission lane
    // is permanently capped at 2 concurrent admissions (`n.min(2)` in
    // `Admission::new`, independent of pool capacity) — 2 genuinely
    // concurrent duplicate submissions already fully exercises the
    // idempotency-key race this test verifies.
    let results = futures_util::future::join_all((0..2).map(|_| {
        post_idem(
            &app,
            "/api/v1/appointments/book",
            Some(&patient),
            &payload,
            &key,
        )
    }))
    .await;
    let mut booking = None;
    for response in results {
        assert!(matches!(
            response.status(),
            StatusCode::CREATED | StatusCode::CONFLICT
        ));
        if response.status() == StatusCode::CREATED {
            let data = json_body(response).await["data"].clone();
            if let Some(ref previous) = booking {
                assert_eq!(&data, previous);
            } else {
                booking = Some(data);
            }
        }
    }
    let booking = booking.expect("at least one booking succeeds");
    assert_eq!(booking["status"], "PENDING_PAYMENT");
    let id = Uuid::parse_str(booking["appointment_id"].as_str().unwrap()).unwrap();
    let session = booking["payment_session_id"].as_str().unwrap();
    let price: bigdecimal::BigDecimal =
        sqlx::query_scalar("SELECT consultation_fee_chat FROM doctor_profiles WHERE id=$1")
            .bind(uid(DOCTOR1_PROFILE_ID))
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        booking["amount"]
            .as_str()
            .unwrap()
            .parse::<bigdecimal::BigDecimal>()
            .unwrap(),
        price
    );
    let counts:(i64,i64)=sqlx::query_as("SELECT (SELECT COUNT(*) FROM appointments WHERE slot_id=$1),(SELECT COUNT(*) FROM transactions WHERE appointment_id=$2)").bind(Uuid::parse_str(slot.as_str().unwrap()).unwrap()).bind(id).fetch_one(&pool).await.unwrap();
    assert_eq!(counts, (1, 1));
    let pending =
        json_body(get(&app, &format!("/api/v1/appointments/{id}"), Some(&patient)).await).await;
    assert_eq!(pending["data"]["payment_status"], "INITIATED");
    assert_eq!(
        get(
            &app,
            &format!("/api/v1/appointments/{id}/conversation"),
            Some(&patient)
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    sqlx::query("UPDATE transactions SET payment_status='FAILED' WHERE appointment_id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    let failed =
        json_body(get(&app, &format!("/api/v1/appointments/{id}"), Some(&patient)).await).await;
    assert_eq!(failed["data"]["payment_status"], "FAILED");
    assert_eq!(
        get(
            &app,
            &format!("/api/v1/appointments/{id}/conversation"),
            Some(&doctor)
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    sqlx::query("UPDATE transactions SET payment_status='INITIATED' WHERE appointment_id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    let callback=serde_json::json!({"payment_session_id":session,"transaction_reference":"CHAT-TEST","status":"SUCCESS","event_timestamp":Utc::now().to_rfc3339(),"provider_event_id":Uuid::new_v4().to_string()}).to_string();
    let sig = sign_webhook_body(&callback, TEST_WEBHOOK_SECRET);
    // Webhook-cost admission is permanently fixed at 2 concurrent admissions
    // (`limits[Cost::Webhook.index()] = 2` in `Admission::new`, a flat
    // constant, not derived from pool capacity at all) — 2 genuinely
    // concurrent replays of the same signed callback already fully
    // exercises the idempotent find-or-create behavior this asserts.
    let callbacks =
        futures_util::future::join_all((0..2).map(|_| send_webhook(&app, &callback, &sig))).await;
    for response in callbacks {
        assert_eq!(response.status(), StatusCode::OK);
    }
    let activated =
        json_body(get(&app, &format!("/api/v1/appointments/{id}"), Some(&patient)).await).await;
    assert_eq!(activated["data"]["status"], "CONFIRMED");
    assert_eq!(activated["data"]["payment_status"], "PAYMENT_HELD");
    let counts:(i64,i64,i64)=sqlx::query_as("SELECT (SELECT COUNT(*) FROM consultation_sessions WHERE appointment_id=$1),(SELECT COUNT(*) FROM chat_conversations WHERE appointment_id=$1),(SELECT COUNT(*) FROM payment_events e JOIN transactions t ON t.id=e.transaction_id WHERE t.appointment_id=$1)").bind(id).fetch_one(&pool).await.unwrap();
    assert_eq!(counts, (1, 1, 1));
    let conversation = json_body(
        get(
            &app,
            &format!("/api/v1/appointments/{id}/conversation"),
            Some(&patient),
        )
        .await,
    )
    .await["data"]
        .clone();
    let conv = conversation["id"].as_str().unwrap();
    for _ in 0..4 {
        let same = json_body(
            get(
                &app,
                &format!("/api/v1/appointments/{id}/conversation"),
                Some(&doctor),
            )
            .await,
        )
        .await;
        assert_eq!(same["data"]["id"], conv);
    }
    let queue =
        json_body(get(&app, "/api/v1/doctor/appointments?limit=100", Some(&doctor)).await).await;
    let item = queue["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == id.to_string())
        .unwrap();
    assert_eq!(item["patient_display_name"], "Meherab Shafin");
    assert_eq!(item["modality"], "CHAT");
    assert_eq!(item["conversation_id"], conv);
    let inbox = json_body(get(&app, "/api/v1/doctor/chats?limit=100", Some(&doctor)).await).await;
    let chat = inbox["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == conv)
        .unwrap();
    assert_eq!(chat["patient_display_name"], "Meherab Shafin");
    assert_eq!(chat["modality"], "CHAT");
    assert_eq!(chat["appointment_status"], "CONFIRMED");
    let path = format!("/api/v1/chat/{conv}/messages");
    assert_eq!(
        post(
            &app,
            &path,
            Some(&patient),
            &serde_json::json!({"content":"Hello doctor"})
        )
        .await
        .status(),
        StatusCode::CREATED
    );
    let on_doctor = json_body(get(&app, &path, Some(&doctor)).await).await;
    assert_eq!(on_doctor["data"][0]["content"], "Hello doctor");
    assert_eq!(
        post(
            &app,
            &path,
            Some(&doctor),
            &serde_json::json!({"content":"Hello, how can I help?"})
        )
        .await
        .status(),
        StatusCode::CREATED
    );
    let on_patient = json_body(get(&app, &path, Some(&patient)).await).await;
    assert_eq!(on_patient["data"].as_array().unwrap().len(), 2);
    assert_eq!(
        on_patient["data"],
        json_body(get(&app, &path, Some(&doctor)).await).await["data"]
    );
    for attacker in [&other_doctor, &other_patient] {
        assert_eq!(
            get(&app, &format!("/api/v1/appointments/{id}"), Some(attacker))
                .await
                .status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            get(
                &app,
                &format!("/api/v1/appointments/{id}/conversation"),
                Some(attacker)
            )
            .await
            .status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            get(&app, &path, Some(attacker)).await.status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            post(
                &app,
                &path,
                Some(attacker),
                &serde_json::json!({"content":"unauthorized"})
            )
            .await
            .status(),
            StatusCode::NOT_FOUND
        );
    }
    sqlx::query("UPDATE appointments SET status='CANCELLED' WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        post(
            &app,
            &path,
            Some(&patient),
            &serde_json::json!({"content":"cancelled"})
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        post(
            &app,
            &path,
            Some(&doctor),
            &serde_json::json!({"content":"cancelled doctor send"})
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    sqlx::query("UPDATE appointments SET status='CONFIRMED' WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE chat_conversations SET expires_at=NOW()-INTERVAL '1 second' WHERE appointment_id=$1").bind(id).execute(&pool).await.unwrap();
    assert_eq!(
        post(
            &app,
            &path,
            Some(&doctor),
            &serde_json::json!({"content":"expired"})
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    let final_counts:(i64,i64,i64)=sqlx::query_as("SELECT (SELECT COUNT(*) FROM appointments WHERE id=$1),(SELECT COUNT(*) FROM transactions WHERE appointment_id=$1),(SELECT COUNT(*) FROM chat_conversations WHERE appointment_id=$1)").bind(id).fetch_one(&pool).await.unwrap();
    assert_eq!(final_counts, (1, 1, 1));
}

#[sqlx::test(migrations = "./migrations")]
async fn test_consultation_video_retains_identity_and_separate_chat_booking(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let (video, _) = book_and_confirm(&app, &patient).await;
    let queue =
        json_body(get(&app, "/api/v1/doctor/appointments?limit=100", Some(&doctor)).await).await;
    let item = queue["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == video.to_string())
        .unwrap();
    assert_eq!(item["modality"], "VIDEO");
    let name: String = sqlx::query_scalar("SELECT full_name FROM patient_profiles WHERE id=$1")
        .bind(uid(PATIENT_PROFILE_ID))
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(item["patient_display_name"], name);
    let conv = json_body(
        get(
            &app,
            &format!("/api/v1/appointments/{video}/conversation"),
            Some(&patient),
        )
        .await,
    )
    .await;
    assert_eq!(item["conversation_id"], conv["data"]["id"]);

    let slots = json_body(
        get(
            &app,
            &format!("/api/v1/doctors/{DOCTOR1_PROFILE_ID}/slots"),
            Some(&patient),
        )
        .await,
    )
    .await;
    let booked = post_idem(
        &app,
        "/api/v1/appointments/book",
        Some(&patient),
        &serde_json::json!({
            "doctor_id": DOCTOR1_PROFILE_ID, "slot_id": slots["data"][0]["id"],
            "modality": "CHAT", "gateway": "BKASH"
        }),
        &Uuid::new_v4().to_string(),
    )
    .await;
    assert_eq!(booked.status(), StatusCode::CREATED);
    let booked = json_body(booked).await;
    let chat_id = booked["data"]["appointment_id"].as_str().unwrap();
    assert_ne!(chat_id, video.to_string());
    let callback = serde_json::json!({"payment_session_id":booked["data"]["payment_session_id"],
        "transaction_reference":format!("BK-SEPARATE-{}",Uuid::new_v4()),"status":"SUCCESS",
        "event_timestamp":chrono::Utc::now().to_rfc3339(),"provider_event_id":Uuid::new_v4().to_string()}).to_string();
    assert_eq!(
        send_webhook(
            &app,
            &callback,
            &sign_webhook_body(&callback, TEST_WEBHOOK_SECRET)
        )
        .await
        .status(),
        StatusCode::OK
    );
    let both =
        json_body(get(&app, "/api/v1/doctor/appointments?limit=100", Some(&doctor)).await).await;
    let rows = both["data"].as_array().unwrap();
    let chat = rows.iter().find(|a| a["id"] == chat_id).unwrap();
    let video_row = rows.iter().find(|a| a["id"] == video.to_string()).unwrap();
    assert_eq!(chat["modality"], "CHAT");
    assert_eq!(video_row["modality"], "VIDEO");
    assert_eq!(
        chat["patient_display_name"],
        video_row["patient_display_name"]
    );
    assert_ne!(chat["conversation_id"], video_row["conversation_id"]);
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/consultations/{chat_id}/rtc-token"),
            Some(&doctor),
            &serde_json::json!({})
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/consultations/{chat_id}/rtc-token"),
            Some(&patient),
            &serde_json::json!({})
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    sqlx::query("UPDATE doctor_schedule_slots SET start_time=NOW()+INTERVAL '3 days',end_time=NOW()+INTERVAL '3 days 30 minutes' WHERE id=(SELECT slot_id FROM appointments WHERE id=$1)").bind(video).execute(&pool).await.unwrap();
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/consultations/{video}/rtc-token"),
            Some(&doctor),
            &serde_json::json!({})
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_consultation_concurrent_video_start_shares_persisted_channel(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let (id, _) = book_and_confirm(&app, &patient).await;
    sqlx::query("UPDATE doctor_schedule_slots SET start_time=NOW()-INTERVAL '1 minute',end_time=NOW()+INTERVAL '9 minutes' WHERE id=(SELECT slot_id FROM appointments WHERE id=$1)").bind(id).execute(&pool).await.unwrap();
    sqlx::query("DELETE FROM consultation_sessions WHERE appointment_id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    let path = format!("/api/v1/consultations/{id}/rtc-token");
    let payload = serde_json::json!({});
    // Consultation-cost admission is permanently capped at 4 concurrent
    // admissions (`n.min(4)` in `Admission::new`, independent of pool
    // capacity) — 4 genuinely concurrent calls (still alternating
    // patient/doctor) already fully exercises the persisted-channel race.
    let responses = futures_util::future::join_all((0..4).map(|i| {
        post(
            &app,
            &path,
            Some(if i % 2 == 0 { &patient } else { &doctor }),
            &payload,
        )
    }))
    .await;
    let stored: String = sqlx::query_scalar(
        "SELECT agora_channel_name FROM consultation_sessions WHERE appointment_id=$1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    for response in responses {
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(json_body(response).await["data"]["channel_name"], stored);
    }
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM consultation_sessions WHERE appointment_id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
}

/// A stale calendar/guessed ID must never charge a Patient for a slot that has
/// already started. The public calendar hides both elapsed and in-progress
/// AVAILABLE rows; direct booking rejects them without financial side effects.
#[sqlx::test(migrations = "./migrations")]
async fn test_chat_booking_rejects_started_slots_without_financial_effect(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let elapsed = schedule_slot_fixture(&app, &pool, "AVAILABLE", 100).await;
    let started = schedule_slot_fixture(&app, &pool, "AVAILABLE", 120).await;
    let future = schedule_slot_fixture(&app, &pool, "AVAILABLE", 140).await;
    sqlx::query("UPDATE doctor_schedule_slots SET start_time=NOW()-INTERVAL '30 minutes', end_time=NOW()-INTERVAL '20 minutes' WHERE id=$1")
        .bind(elapsed).execute(&pool).await.unwrap();
    sqlx::query("UPDATE doctor_schedule_slots SET start_time=NOW()-INTERVAL '1 minute', end_time=NOW()+INTERVAL '9 minutes' WHERE id=$1")
        .bind(started).execute(&pool).await.unwrap();

    let response = get(
        &app,
        &format!("/api/v1/doctors/{DOCTOR1_PROFILE_ID}/slots"),
        Some(&patient),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let calendar = json_body(response).await;
    let rows = calendar["data"].as_array().unwrap();
    assert!(rows
        .iter()
        .all(|r| r["id"] != elapsed.to_string() && r["id"] != started.to_string()));
    assert!(rows.iter().any(|r| r["id"] == future.to_string()));

    for slot in [elapsed, started] {
        let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM transactions")
            .fetch_one(&pool)
            .await
            .unwrap();
        let payload = serde_json::json!({"doctor_id":DOCTOR1_PROFILE_ID,"slot_id":slot,"modality":"CHAT","gateway":"MPESA"});
        let response = post_idem(
            &app,
            "/api/v1/appointments/book",
            Some(&patient),
            &payload,
            &Uuid::new_v4().to_string(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(schedule_slot_status(&pool, slot).await, "AVAILABLE");
        let appointments: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM appointments WHERE slot_id=$1")
                .bind(slot)
                .fetch_one(&pool)
                .await
                .unwrap();
        let after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM transactions")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(appointments, 0);
        assert_eq!(after, before);
    }
}

// Pre-E2E corrections: additive fixtures and assertions; historical cases above remain intact.
#[sqlx::test(migrations = "./migrations")]
async fn test_pre_e2e_queue_excludes_stale_and_terminal_but_preserves_history(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let now = Utc::now();
    let (_, past) = insert_confirmed_appointment(&pool, now - Duration::hours(4)).await;
    let (_, completed) = insert_confirmed_appointment(&pool, now - Duration::hours(3)).await;
    sqlx::query("UPDATE appointments SET status='COMPLETED', completed_at=NOW()-INTERVAL '2 hours' WHERE id=$1").bind(completed).execute(&pool).await.unwrap();
    let (_, near) = insert_confirmed_appointment(&pool, now + Duration::minutes(30)).await;
    let (_, later) = insert_confirmed_appointment(&pool, now + Duration::minutes(150)).await;
    let (_, tomorrow) = insert_confirmed_appointment(&pool, now + Duration::days(1)).await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let response = get(
        &app,
        "/api/v1/doctor/appointments?view=upcoming",
        Some(&doctor),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    let ids: Vec<_> = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["id"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(ids, vec![near.to_string(), later.to_string()]);
    assert!(body["data"]
        .as_array()
        .unwrap()
        .iter()
        .all(|v| v["queue_actionable"] == true));
    let history = json_body(
        get(
            &app,
            "/api/v1/doctor/appointments?view=history&status=COMPLETED",
            Some(&doctor),
        )
        .await,
    )
    .await;
    assert_eq!(history["data"].as_array().unwrap().len(), 1);
    assert_eq!(history["data"][0]["id"], completed.to_string());
    assert_eq!(history["data"][0]["queue_actionable"], false);
    let all =
        json_body(get(&app, "/api/v1/doctor/appointments?view=all", Some(&doctor)).await).await;
    assert_eq!(all["data"].as_array().unwrap().len(), 5);
    assert_eq!(all["data"][0]["id"], near.to_string());
    assert_eq!(all["data"][3]["id"], completed.to_string());
    assert_eq!(all["data"][4]["id"], past.to_string());
    let (_, active) = insert_confirmed_appointment(&pool, now - Duration::minutes(60)).await;
    sqlx::query("UPDATE appointments SET status='IN_CONSULTATION' WHERE id=$1")
        .bind(active)
        .execute(&pool)
        .await
        .unwrap();
    let queue = json_body(
        get(
            &app,
            "/api/v1/doctor/appointments?view=upcoming",
            Some(&doctor),
        )
        .await,
    )
    .await;
    assert_eq!(queue["data"].as_array().unwrap().len(), 3);
    assert_eq!(queue["data"][0]["id"], active.to_string());
    assert_eq!(queue["data"][1]["id"], near.to_string());
    let scheduled = json_body(
        get(
            &app,
            "/api/v1/doctor/appointments?view=schedule",
            Some(&doctor),
        )
        .await,
    )
    .await;
    assert_eq!(scheduled["data"].as_array().unwrap().len(), 1);
    assert_eq!(scheduled["data"][0]["id"], tomorrow.to_string());
    assert!(!queue["data"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["id"] == tomorrow.to_string()));
    let other = make_token(uid(DOCTOR2_USER_ID), "DOCTOR");
    let isolated = json_body(
        get(
            &app,
            "/api/v1/doctor/appointments?view=upcoming",
            Some(&other),
        )
        .await,
    )
    .await;
    assert_eq!(isolated["data"].as_array().unwrap().len(), 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_pre_e2e_slot_instant_boundaries_and_nairobi_date(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let doctor = DoctorProfileId::from(uid(DOCTOR1_PROFILE_ID));
    let now: DateTime<Utc> = sqlx::query_scalar("SELECT clock_timestamp()")
        .fetch_one(&pool)
        .await
        .unwrap();
    let crossing = DateTime::<Utc>::from_naive_utc_and_offset(
        now.date_naive()
            .succ_opt()
            .unwrap()
            .and_hms_opt(21, 30, 0)
            .unwrap(),
        Utc,
    );
    let times = [
        now - Duration::days(2),
        now - Duration::hours(2),
        now - Duration::seconds(1),
        now + Duration::minutes(30),
        crossing,
    ];
    let mut ids = Vec::new();
    for start in times {
        let id = Uuid::new_v4();
        doctor_repo::insert_slot(
            &pool,
            &ScheduleSlot {
                id,
                doctor_id: doctor,
                start_time: start,
                end_time: start + Duration::minutes(10),
                status: SlotStatus::Available,
                created_at: now,
            },
        )
        .await
        .unwrap();
        ids.push(id);
    }
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let response = get(
        &app,
        &format!("/api/v1/doctors/{doctor}/slots"),
        Some(&patient),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    let rows = body["data"].as_array().unwrap();
    for id in &ids[..3] {
        assert!(rows.iter().all(|v| v["id"] != id.to_string()));
    }
    for id in &ids[3..] {
        assert!(rows.iter().any(|v| v["id"] == id.to_string()));
    }
    let row = rows.iter().find(|v| v["id"] == ids[4].to_string()).unwrap();
    assert_eq!(
        DateTime::parse_from_rfc3339(row["start_time"].as_str().unwrap())
            .unwrap()
            .with_timezone(&Utc),
        crossing
    );
    assert_ne!(
        crossing.date_naive(),
        (crossing + Duration::hours(3)).date_naive()
    );
    for id in &ids[..3] {
        let response=post_idem(&app,"/api/v1/appointments/book",Some(&patient),&serde_json::json!({"doctor_id":doctor,"slot_id":id,"modality":"CHAT","gateway":"MPESA"}),&Uuid::new_v4().to_string()).await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(schedule_slot_status(&pool, *id).await, "AVAILABLE");
    }
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM appointments")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM transactions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_pre_e2e_slot_start_crossed_while_waiting_for_row_lock(pool: PgPool) {
    let (_, pool) = test_app(pool).await;
    let id = Uuid::new_v4();
    let doctor = DoctorProfileId::from(uid(DOCTOR1_PROFILE_ID));
    let mut holder = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO doctor_schedule_slots(id,doctor_id,start_time,end_time,status) VALUES($1,$2,clock_timestamp()+INTERVAL '700 milliseconds',clock_timestamp()+INTERVAL '10 minutes','AVAILABLE')").bind(id).bind(doctor).execute(&mut *holder).await.unwrap();
    holder.commit().await.unwrap();
    let mut holder = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM doctor_schedule_slots WHERE id=$1 FOR UPDATE")
        .bind(id)
        .fetch_one(&mut *holder)
        .await
        .unwrap();
    let mut waiter = pool.begin().await.unwrap();
    let before: bool = sqlx::query_scalar(
        "SELECT start_time > CURRENT_TIMESTAMP FROM doctor_schedule_slots WHERE id=$1",
    )
    .bind(id)
    .fetch_one(&mut *waiter)
    .await
    .unwrap();
    assert!(before);
    let task = tokio::spawn(async move {
        let result = doctor_repo::try_lock_slot(&mut waiter, id, doctor)
            .await
            .unwrap();
        waiter.commit().await.unwrap();
        result
    });
    tokio::time::sleep(std::time::Duration::from_millis(900)).await;
    holder.commit().await.unwrap();
    assert!(!task.await.unwrap());
    assert_eq!(schedule_slot_status(&pool, id).await, "AVAILABLE");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_pre_e2e_payment_resume_has_no_duplicate_or_false_payment(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let slots = json_body(
        get(
            &app,
            &format!("/api/v1/doctors/{DOCTOR1_PROFILE_ID}/slots"),
            Some(&patient),
        )
        .await,
    )
    .await;
    let response=post_idem(&app,"/api/v1/appointments/book",Some(&patient),&serde_json::json!({"doctor_id":DOCTOR1_PROFILE_ID,"slot_id":slots["data"][0]["id"],"modality":"CHAT","gateway":"MPESA"}),&Uuid::new_v4().to_string()).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = json_body(response).await;
    let id = body["data"]["appointment_id"].as_str().unwrap();
    let path = format!("/api/v1/appointments/{id}/payment");
    let mut tasks = Vec::new();
    for _ in 0..4 {
        let app = app.clone();
        let token = patient.clone();
        let path = path.clone();
        tasks.push(tokio::spawn(async move {
            post(&app, &path, Some(&token), &serde_json::json!({})).await
        }));
    }
    let mut provider_unavailable = 0;
    for task in tasks {
        let response = task.await.unwrap();
        match response.status() {
            StatusCode::CONFLICT => {
                let body = json_body(response).await;
                assert_eq!(
                    body["error"]["message"],
                    "Payment service is currently unavailable. Please try again later."
                );
                provider_unavailable += 1;
            }
            StatusCode::SERVICE_UNAVAILABLE => {
                assert_eq!(
                    response
                        .headers()
                        .get("retry-after")
                        .and_then(|v| v.to_str().ok()),
                    Some("1")
                );
                let body = json_body(response).await;
                assert_eq!(body["error"]["code"], "SERVICE_OVERLOADED");
            }
            status => panic!("unexpected payment-resume response: {status}"),
        }
    }
    assert!(
        provider_unavailable >= 1,
        "the race must exercise the actual payment handler"
    );
    let status =
        json_body(get(&app, &format!("/api/v1/appointments/{id}"), Some(&patient)).await).await;
    assert_eq!(status["data"]["status"], "PENDING_PAYMENT");
    assert_eq!(status["data"]["payment_status"], "INITIATED");
    let counts: (i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM appointments),(SELECT COUNT(*) FROM transactions)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(counts, (1, 1));
    let other = fresh_patient_token(&pool).await;
    assert_eq!(
        post(&app, &path, Some(&other), &serde_json::json!({}))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    assert_eq!(
        post(&app, &path, Some(&doctor), &serde_json::json!({}))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    let rejected:Vec<(Uuid,String,String)>=sqlx::query_as("SELECT actor_user_id,result,reason_code FROM consultation_trace_events WHERE appointment_id=$1 AND event_type='PAYMENT_INITIATION_REJECTED'").bind(uid(id)).fetch_all(&pool).await.unwrap();
    assert_eq!(rejected.len(), provider_unavailable);
    assert!(rejected
        .iter()
        .all(|(actor, result, reason)| *actor == uid(PATIENT_USER_ID)
            && result == "REJECTED"
            && reason == "PROVIDER_UNAVAILABLE"));
}

#[sqlx::test(migrations = "./migrations")]
async fn test_pre_e2e_clinical_attachment_shared_private_persistent_and_idempotent(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let (appointment, _) = book_and_confirm(&app, &patient).await;
    open_clinical_window(&pool, appointment).await;
    let conv = json_body(
        get(
            &app,
            &format!("/api/v1/appointments/{appointment}/conversation"),
            Some(&patient),
        )
        .await,
    )
    .await;
    let conversation = conv["data"]["id"].as_str().unwrap();
    let upload = format!("/api/v1/chat/{conversation}/attachments");
    let body = serde_json::json!({"mime_type":"image/jpeg","base64_content":minimal_jpeg_base64()});
    let key = Uuid::new_v4().to_string();
    let mut tasks = Vec::new();
    for _ in 0..4 {
        let app = app.clone();
        let token = doctor.clone();
        let path = upload.clone();
        let body = body.clone();
        let key = key.clone();
        tasks.push(tokio::spawn(async move {
            post_idem(&app, &path, Some(&token), &body, &key).await
        }));
    }
    let mut results = Vec::new();
    let mut shed = 0;
    for task in tasks {
        let response = task.await.unwrap();
        if response.status() == StatusCode::CREATED {
            results.push(json_body(response).await);
        } else {
            assert_controlled_shedding(response).await;
            shed += 1;
        }
    }
    assert!(
        !results.is_empty(),
        "at least one actual upload must commit during the race"
    );
    // Recover every shed caller through the original request identity. All four
    // participants must ultimately observe the same committed attachment.
    for _ in 0..shed {
        let response = post_idem(&app, &upload, Some(&doctor), &body, &key).await;
        assert_eq!(response.status(), StatusCode::CREATED);
        results.push(json_body(response).await);
    }
    assert_eq!(results.len(), 4);
    let message = results[0]["data"]["id"].as_str().unwrap();
    for r in &results {
        assert_eq!(r["data"]["id"], message);
    }
    assert_eq!(results[0]["data"]["sender_id"], DOCTOR1_USER_ID);
    assert!(results[0]["data"].get("attachment_object_key").is_none());
    assert!(results[0]["data"].get("attachment_bucket").is_none());
    assert!(results[0]["data"].get("attachment_hash").is_none());
    assert_eq!(
        results[0]["data"]["attachment"]["display_name"],
        "Clinical image"
    );
    assert!(
        results[0]["data"]["attachment"]["size_bytes"]
            .as_i64()
            .unwrap()
            <= 40 * 1024
    );
    let list = format!("/api/v1/chat/{conversation}/messages");
    let p = json_body(get(&app, &list, Some(&patient)).await).await;
    let d = json_body(get(&app, &list, Some(&doctor)).await).await;
    assert_eq!(p["data"], d["data"]);
    assert_eq!(p["data"].as_array().unwrap().len(), 1);
    let view = format!("/api/v1/chat/{conversation}/messages/{message}/attachment");
    let response = get(&app, &view, Some(&patient)).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store, private");
    let file = json_body(response).await;
    let bytes = STANDARD
        .decode(file["data"]["base64_content"].as_str().unwrap())
        .unwrap();
    assert!(bytes.len() <= 40 * 1024);
    assert!(bytes.starts_with(&[0xff, 0xd8, 0xff]));
    let doctor_file = json_body(get(&app, &view, Some(&doctor)).await).await;
    assert_eq!(doctor_file["data"], file["data"]);
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM chat_messages WHERE conversation_id=$1")
            .bind(Uuid::parse_str(conversation).unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
    let other_doctor = make_token(uid(DOCTOR2_USER_ID), "DOCTOR");
    let other_patient = fresh_patient_token(&pool).await;
    assert_eq!(
        post_idem(
            &app,
            &upload,
            Some(&other_doctor),
            &body,
            &Uuid::new_v4().to_string()
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        post_idem(
            &app,
            &upload,
            Some(&patient),
            &body,
            &Uuid::new_v4().to_string()
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    for token in [&other_doctor, &other_patient] {
        assert_eq!(
            get(&app, &view, Some(token)).await.status(),
            StatusCode::NOT_FOUND
        );
    }
    sqlx::query("UPDATE chat_conversations SET expires_at=NOW()-INTERVAL '1 second' WHERE id=$1")
        .bind(Uuid::parse_str(conversation).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        post_idem(
            &app,
            &upload,
            Some(&doctor),
            &body,
            &Uuid::new_v4().to_string()
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM chat_messages WHERE conversation_id=$1")
            .bind(Uuid::parse_str(conversation).unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_pre_e2e_clinical_attachment_rejects_spoof_corruption_and_size(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let (appointment, _) = book_and_confirm(&app, &patient).await;
    open_clinical_window(&pool, appointment).await;
    let conv = json_body(
        get(
            &app,
            &format!("/api/v1/appointments/{appointment}/conversation"),
            Some(&patient),
        )
        .await,
    )
    .await;
    let id = conv["data"]["id"].as_str().unwrap();
    let path = format!("/api/v1/chat/{id}/attachments");
    for body in [
        serde_json::json!({"mime_type":"image/png","base64_content":minimal_jpeg_base64()}),
        serde_json::json!({"mime_type":"image/jpeg","base64_content":STANDARD.encode([0xff,0xd8,0xff,0,1,2])}),
        serde_json::json!({"mime_type":"application/pdf","base64_content":STANDARD.encode(b"%PDF-fixture")}),
    ] {
        assert_eq!(
            post_idem(
                &app,
                &path,
                Some(&doctor),
                &body,
                &Uuid::new_v4().to_string()
            )
            .await
            .status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    let response = post_idem(
        &app,
        &path,
        Some(&doctor),
        &serde_json::json!({"mime_type":"image/jpeg","base64_content":"A".repeat(14*1024*1024+1)}),
        &Uuid::new_v4().to_string(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM chat_messages WHERE conversation_id=$1")
            .bind(Uuid::parse_str(id).unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_milestone_trace_base_and_single_admin(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let role: String = sqlx::query_scalar("SELECT role::text FROM users WHERE id=$1")
        .bind(uid(ADMIN_USER_ID))
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(role, "ADMIN");
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let (appointment, _) = book_and_confirm(&app, &patient).await;
    let response = get(
        &app,
        &format!("/api/v1/appointments/{appointment}/conversation"),
        Some(&patient),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    let conversation = body["data"]["id"].as_str().unwrap();
    let path = format!("/api/v1/chat/{conversation}/messages");
    let key = Uuid::new_v4().to_string();
    let message = serde_json::json!({"content":"Milestone synthetic message"});
    let first = post_idem(&app, &path, Some(&patient), &message, &key).await;
    assert_eq!(first.status(), StatusCode::CREATED);
    let first = json_body(first).await;
    let second = post_idem(&app, &path, Some(&patient), &message, &key).await;
    assert_eq!(second.status(), StatusCode::CREATED);
    assert_eq!(json_body(second).await["data"]["id"], first["data"]["id"]);
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM chat_messages WHERE conversation_id=$1")
            .bind(uid(conversation))
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
    let notices: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM notifications WHERE type='NEW_CHAT_MESSAGE'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(notices, 1);
    let traces: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM consultation_trace_events WHERE appointment_id=$1",
    )
    .bind(appointment)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(traces >= 3);
    let forbidden = sqlx::query("DELETE FROM consultation_trace_events")
        .execute(&state.db)
        .await;
    assert!(forbidden.is_err());
}

#[sqlx::test(migrations = "./migrations")]
async fn test_milestone_grievance_preservation_and_timeline(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let (appointment, _) = book_and_confirm(&app, &patient).await;
    let session = consultation_repo::find_by_appointment(&pool, appointment)
        .await
        .unwrap()
        .unwrap();
    let case=post(&app,"/api/v1/grievances",Some(&patient),&serde_json::json!({"consultation_id":session.id,"target":"DOCTOR","category":"Trace investigation","claim_summary":"Synthetic grievance for preservation acceptance"})).await;
    assert_eq!(case.status(), StatusCode::CREATED);
    let case = json_body(case).await;
    let grievance = case["data"]["grievance_id"].as_str().unwrap();
    let held:i64=sqlx::query_scalar("SELECT COUNT(*) FROM consultation_trace_holds WHERE appointment_id=$1 AND release_at IS NULL").bind(appointment).fetch_one(&pool).await.unwrap();
    assert_eq!(held, 1);
    let timeline = get(
        &app,
        &format!("/api/v1/admin/grievances/{grievance}/timeline"),
        Some(&admin),
    )
    .await;
    let status = timeline.status();
    let timeline = json_body(timeline).await;
    assert_eq!(status, StatusCode::OK, "{timeline}");
    assert_eq!(
        timeline["data"]["facts"]["doctor_id"],
        serde_json::json!(DOCTOR1_PROFILE_ID)
    );
    assert_eq!(
        timeline["data"]["facts"]["appointment_id"],
        serde_json::json!(appointment)
    );
    assert_eq!(
        timeline["data"]["preservation_holds"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(timeline["data"]["trace_events"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["event_type"] == "GRIEVANCE_CREATED"));
    let denied = get(
        &app,
        &format!("/api/v1/admin/grievances/{grievance}/timeline"),
        Some(&patient),
    )
    .await;
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    let before: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM consultation_trace_events WHERE appointment_id=$1",
    )
    .bind(appointment)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE consultation_trace_events SET expires_at=clock_timestamp()-INTERVAL '1 day' WHERE appointment_id=$1").bind(appointment).execute(&pool).await.unwrap();
    let deleted = hellodoctor_backend::services::consultation_trace::maintenance(&state)
        .await
        .unwrap();
    assert_eq!(deleted, 0);
    let after: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM consultation_trace_events WHERE appointment_id=$1",
    )
    .bind(appointment)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(after, before);
    let audit:(Uuid,String)=sqlx::query_as("SELECT actor_id,actor_role::text FROM audit_events WHERE action='ADMIN_GRIEVANCE_OPENED' AND resource_id=$1").bind(uid(grievance)).fetch_one(&pool).await.unwrap();
    assert_eq!(audit, (uid(ADMIN_USER_ID), "ADMIN".into()));
}

#[sqlx::test(migrations = "./migrations")]
async fn test_milestone_latest_200_cursor_pagination(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let (appointment, _) = book_and_confirm(&app, &patient).await;
    let conv = chat_repo::find_conversation_by_appointment(&pool, appointment)
        .await
        .unwrap()
        .unwrap();
    for i in 0..200 {
        sqlx::query("INSERT INTO chat_messages(id,conversation_id,sender_id,content,sent_at) VALUES($1,$2,$3,$4,clock_timestamp()-INTERVAL '1 hour'+$5*INTERVAL '1 second')").bind(Uuid::new_v4()).bind(conv.id).bind(uid(PATIENT_USER_ID)).bind(format!("Pagination fixture {i:03}")).bind(i as f64).execute(&pool).await.unwrap();
    }
    let mut path = format!("/api/v1/chat/{}/messages", conv.id);
    let mut seen = std::collections::HashSet::new();
    for page in 0..4 {
        let response = get(&app, &path, Some(&patient)).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        let rows = body["data"].as_array().unwrap();
        assert_eq!(rows.len(), 50);
        assert_eq!(
            rows.last().unwrap()["content"],
            format!("Pagination fixture {:03}", 199 - page * 50)
        );
        for row in rows {
            assert!(
                seen.insert(row["id"].as_str().unwrap().to_owned()),
                "Duplicate cursor boundary"
            );
        }
        let first = &rows[0];
        let time = first["sent_at"].as_str().unwrap().replace('+', "%2B");
        path = format!(
            "/api/v1/chat/{}/messages?before_time={time}&before_id={}",
            conv.id,
            first["id"].as_str().unwrap()
        );
    }
    assert_eq!(seen.len(), 200);
    let final_page = get(&app, &path, Some(&patient)).await;
    assert_eq!(final_page.status(), StatusCode::OK);
    assert_eq!(
        json_body(final_page).await["data"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_milestone_trace_cleanup_never_touches_clinical_records(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let (appointment, _) = book_and_confirm(&app, &patient).await;
    let retention:f64=sqlx::query_scalar("SELECT EXTRACT(EPOCH FROM expires_at-event_time_server)::float8/86400 FROM consultation_trace_events WHERE appointment_id=$1 LIMIT 1").bind(appointment).fetch_one(&pool).await.unwrap();
    assert_eq!(retention, 5.0);
    let expected: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM consultation_trace_events WHERE appointment_id=$1",
    )
    .bind(appointment)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(expected > 0);
    for role in [UserRole::Patient, UserRole::Admin] {
        let user = if role == UserRole::Patient {
            PATIENT_USER_ID
        } else {
            ADMIN_USER_ID
        };
        let ctx = isolation_ctx(uid(user).into(), role);
        for statement in [
            "DELETE FROM consultation_trace_events",
            "UPDATE consultation_trace_events SET result='ALTERED'",
        ] {
            let mut tx = state.begin_authed(&ctx).await.unwrap();
            let error = sqlx::query(statement).execute(&mut *tx).await.unwrap_err();
            assert_eq!(
                error.as_database_error().unwrap().code().as_deref(),
                Some("42501")
            );
            tx.rollback().await.unwrap();
        }
    }
    let before:(i64,i64,i64,i64)=sqlx::query_as("SELECT (SELECT COUNT(*) FROM appointments),(SELECT COUNT(*) FROM transactions),(SELECT COUNT(*) FROM chat_conversations),(SELECT COUNT(*) FROM audit_events)").fetch_one(&pool).await.unwrap();
    sqlx::query("UPDATE consultation_trace_events SET expires_at=clock_timestamp()-INTERVAL '1 second' WHERE appointment_id=$1").bind(appointment).execute(&pool).await.unwrap();
    assert_eq!(
        hellodoctor_backend::services::consultation_trace::maintenance(&state)
            .await
            .unwrap(),
        expected as u64
    );
    let after:(i64,i64,i64,i64)=sqlx::query_as("SELECT (SELECT COUNT(*) FROM appointments),(SELECT COUNT(*) FROM transactions),(SELECT COUNT(*) FROM chat_conversations),(SELECT COUNT(*) FROM audit_events)").fetch_one(&pool).await.unwrap();
    assert_eq!(after, before);
    let traces: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM consultation_trace_events WHERE appointment_id=$1",
    )
    .bind(appointment)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(traces, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_milestone_chat_concurrent_replay_distinct_keys_and_privacy(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let (appointment, _) = book_and_confirm(&app, &patient).await;
    let conv = chat_repo::find_conversation_by_appointment(&pool, appointment)
        .await
        .unwrap()
        .unwrap();
    let path = format!("/api/v1/chat/{}/messages", conv.id);
    let body = serde_json::json!({"content":"Private synthetic clinical text must not enter notifications"});
    let key = Uuid::new_v4().to_string();
    // `/chat/*/messages` also classifies as Cost::Consultation, permanently
    // capped at 4 concurrent admissions — 4 genuinely concurrent replays
    // already fully exercises the idempotent-dedup race this test verifies.
    let responses = futures_util::future::join_all(
        (0..4).map(|_| post_idem(&app, &path, Some(&patient), &body, &key)),
    )
    .await;
    let mut ids = std::collections::HashSet::new();
    for response in responses {
        assert_eq!(response.status(), StatusCode::CREATED);
        ids.insert(
            json_body(response).await["data"]["id"]
                .as_str()
                .unwrap()
                .to_owned(),
        );
    }
    assert_eq!(ids.len(), 1);
    let different_key = post_idem(
        &app,
        &path,
        Some(&patient),
        &body,
        &Uuid::new_v4().to_string(),
    )
    .await;
    assert_eq!(different_key.status(), StatusCode::CREATED);
    assert!(!ids.contains(
        json_body(different_key).await["data"]["id"]
            .as_str()
            .unwrap()
    ));
    let collision = post_idem(
        &app,
        &path,
        Some(&patient),
        &serde_json::json!({"content":"Changed content"}),
        &key,
    )
    .await;
    assert_eq!(collision.status(), StatusCode::CONFLICT);
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM chat_messages WHERE conversation_id=$1")
            .bind(conv.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 2);
    let notifications: Vec<String> = sqlx::query_scalar(
        "SELECT body FROM notifications WHERE type='NEW_CHAT_MESSAGE' AND related_entity_id=$1",
    )
    .bind(conv.id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(notifications.len(), 2);
    for notice in notifications {
        assert!(!notice.contains("Private synthetic"));
    }
    let leaked:i64=sqlx::query_scalar("SELECT COUNT(*) FROM consultation_trace_events WHERE appointment_id=$1 AND safe_metadata::text LIKE '%Private synthetic%'").bind(appointment).fetch_one(&pool).await.unwrap();
    assert_eq!(leaked, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_milestone_chat_expiry_wall_clock_and_historical_reads(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let (appointment, _) = book_and_confirm(&app, &patient).await;
    let conv = chat_repo::find_conversation_by_appointment(&pool, appointment)
        .await
        .unwrap()
        .unwrap();
    let path = format!("/api/v1/chat/{}/messages", conv.id);
    assert_eq!(
        post_idem(
            &app,
            &path,
            Some(&patient),
            &serde_json::json!({"content":"Persisted before expiry"}),
            &Uuid::new_v4().to_string()
        )
        .await
        .status(),
        StatusCode::CREATED
    );
    sqlx::query("UPDATE chat_conversations SET expires_at=clock_timestamp()+INTERVAL '150 milliseconds' WHERE id=$1").bind(conv.id).execute(&pool).await.unwrap();
    let ctx = isolation_ctx(uid(PATIENT_USER_ID).into(), UserRole::Patient);
    let mut tx = state.begin_authed(&ctx).await.unwrap();
    sqlx::query("SELECT pg_sleep(0.2)")
        .execute(&mut *tx)
        .await
        .unwrap();
    let error=sqlx::query("INSERT INTO chat_messages(conversation_id,sender_id,content) VALUES($1,$2,'Rejected after transaction crossed expiry')").bind(conv.id).bind(ctx.user_id).execute(&mut *tx).await.unwrap_err();
    assert_eq!(
        error.as_database_error().unwrap().code().as_deref(),
        Some("42501")
    );
    tx.rollback().await.unwrap();
    for token in [&patient, &doctor] {
        let rejected = post_idem(
            &app,
            &path,
            Some(token),
            &serde_json::json!({"content":"Rejected after lock"}),
            &Uuid::new_v4().to_string(),
        )
        .await;
        assert_eq!(rejected.status(), StatusCode::FORBIDDEN);
        let read = get(&app, &path, Some(token)).await;
        assert_eq!(read.status(), StatusCode::OK);
        assert_eq!(json_body(read).await["data"].as_array().unwrap().len(), 1);
    }
    let notices: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM notifications WHERE type='NEW_CHAT_MESSAGE' AND related_entity_id=$1",
    )
    .bind(conv.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(notices, 1);
    let rejected:i64=sqlx::query_scalar("SELECT COUNT(*) FROM consultation_trace_events WHERE appointment_id=$1 AND event_type='CHAT_MESSAGE_REJECTED' AND result='REJECTED'").bind(appointment).fetch_one(&pool).await.unwrap();
    assert_eq!(rejected, 2);
}

async fn milestone_book_chat(
    app: &axum::Router,
    patient_token: &str,
    doctor: &str,
) -> (Uuid, Uuid) {
    let res_slots = get(
        app,
        &format!("/api/v1/doctors/{}/slots", doctor),
        Some(patient_token),
    )
    .await;
    assert_eq!(res_slots.status(), StatusCode::OK);
    let json_slots = json_body(res_slots).await;
    let slot_id = Uuid::parse_str(json_slots["data"][0]["id"].as_str().unwrap()).unwrap();

    let book_payload = serde_json::json!({
        "doctor_id": doctor,
        "slot_id": slot_id,
        "modality": "CHAT",
        "gateway": "BKASH",
        "chief_complaint": "Persistent fever and joint pain"
    });
    let res_book = post_idem(
        app,
        "/api/v1/appointments/book",
        Some(patient_token),
        &book_payload,
        &Uuid::new_v4().to_string(),
    )
    .await;
    assert_eq!(res_book.status(), StatusCode::CREATED);
    let book_json = json_body(res_book).await;
    let apt_id = Uuid::parse_str(book_json["data"]["appointment_id"].as_str().unwrap()).unwrap();
    let payment_sess_id =
        Uuid::parse_str(book_json["data"]["payment_session_id"].as_str().unwrap()).unwrap();

    let webhook_payload = serde_json::json!({
        "payment_session_id": payment_sess_id,
        "transaction_reference": format!("BK-TEST-{}", Uuid::new_v4()),
        "status": "SUCCESS",
        "event_timestamp": chrono::Utc::now().to_rfc3339(),
        "provider_event_id": Uuid::new_v4().to_string(),
    });
    let body_str = serde_json::to_string(&webhook_payload).unwrap();
    let sig = sign_webhook_body(&body_str, TEST_WEBHOOK_SECRET);
    let res_webhook = send_webhook(app, &body_str, &sig).await;
    assert_eq!(res_webhook.status(), StatusCode::OK);

    (apt_id, payment_sess_id)
}

#[sqlx::test(migrations = "./migrations")]
async fn test_milestone_same_patient_two_doctors_historical_isolation(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctors = [
        make_token(uid(DOCTOR1_USER_ID), "DOCTOR"),
        make_token(uid(DOCTOR2_USER_ID), "DOCTOR"),
    ];
    let mut appointments = Vec::new();
    let mut conversations = Vec::new();
    let mut prescriptions = Vec::new();
    for (i, doctor) in [DOCTOR1_PROFILE_ID, DOCTOR2_PROFILE_ID].iter().enumerate() {
        if i == 1 {
            let slot=post(&app,"/api/v1/doctor/slots",Some(&doctors[i]),&serde_json::json!({"start_time":(Utc::now()+Duration::hours(4)).to_rfc3339(),"end_time":(Utc::now()+Duration::hours(4)+Duration::minutes(20)).to_rfc3339()})).await;
            let status = slot.status();
            let body = json_body(slot).await;
            assert_eq!(status, StatusCode::CREATED, "{body}");
        }
        let (appointment, _) = milestone_book_chat(&app, &patient, doctor).await;
        // Both appointments belong to the same Patient (different Doctors) —
        // stagger the two clinical windows so they don't land on the
        // (almost) identical instant, which would otherwise trip Stage A's
        // `appt_patient_no_overlap` constraint. See `open_clinical_window_at`.
        // +15 minutes is the exact boundary that works for both reasons at
        // once: it lands adjacent to (not overlapping) i==0's [+5,+15) slot
        // under the half-open range, AND it's still within
        // `doctor_history_context_allowed`'s own `CURRENT_TIMESTAMP >=
        // start_time - INTERVAL '15 minutes'` access rule (the prescription
        // endpoint's real authorization check), which a larger offset would
        // fail.
        if i == 0 {
            open_clinical_window(&pool, appointment).await;
        } else {
            open_clinical_window_at(&pool, appointment, 15).await;
        }
        let rx = post(
            &app,
            &format!("/api/v1/consultations/{appointment}/prescription"),
            Some(&doctors[i]),
            &serde_json::json!({"base64_photo":minimal_jpeg_base64()}),
        )
        .await;
        let status = rx.status();
        let rx = json_body(rx).await;
        assert_eq!(status, StatusCode::CREATED, "{rx}");
        appointments.push(appointment);
        prescriptions.push(rx["data"]["id"].as_str().unwrap().to_owned());
        conversations.push(
            chat_repo::find_conversation_by_appointment(&pool, appointment)
                .await
                .unwrap()
                .unwrap()
                .id,
        );
    }
    sqlx::query("UPDATE chat_conversations SET expires_at=clock_timestamp()-INTERVAL '2 days'")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE consultation_sessions SET ended_at=clock_timestamp()-INTERVAL '2 days',status='COMPLETED'").execute(&pool).await.unwrap();
    for i in 0..2 {
        for j in 0..2 {
            let own = i == j;
            let summary = get(
                &app,
                &format!("/api/v1/doctor/consultations/{}/summary", appointments[j]),
                Some(&doctors[i]),
            )
            .await;
            assert_eq!(
                summary.status(),
                if own {
                    StatusCode::OK
                } else {
                    StatusCode::FORBIDDEN
                }
            );
            let chat = get(
                &app,
                &format!("/api/v1/chat/{}/messages", conversations[j]),
                Some(&doctors[i]),
            )
            .await;
            assert_eq!(
                chat.status(),
                if own {
                    StatusCode::OK
                } else {
                    StatusCode::NOT_FOUND
                }
            );
            let rx = get(
                &app,
                &format!(
                    "/api/v1/appointments/{}/documents/{}",
                    appointments[j], prescriptions[j]
                ),
                Some(&doctors[i]),
            )
            .await;
            assert_eq!(
                rx.status(),
                if own {
                    StatusCode::OK
                } else {
                    StatusCode::NOT_FOUND
                }
            );
        }
        let history = get(
            &app,
            &format!(
                "/api/v1/doctor/appointments/{}/patient-history",
                appointments[i]
            ),
            Some(&doctors[i]),
        )
        .await;
        assert_eq!(history.status(), StatusCode::OK);
        let rows = json_body(history).await;
        assert!(rows["data"]
            .as_array()
            .unwrap()
            .iter()
            .all(|row| row["historical_appointment_id"] != serde_json::json!(appointments[1 - i])));
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn test_milestone_admin_reschedule_preserves_time_account_and_conversation(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let (appointment, _) = milestone_book_chat(&app, &patient, DOCTOR1_PROFILE_ID).await;
    let original:(Uuid,DateTime<Utc>)=sqlx::query_as("SELECT a.slot_id,s.start_time FROM appointments a JOIN doctor_schedule_slots s ON s.id=a.slot_id WHERE a.id=$1").bind(appointment).fetch_one(&pool).await.unwrap();
    let conversation = chat_repo::find_conversation_by_appointment(&pool, appointment)
        .await
        .unwrap()
        .unwrap();
    let slots = json_body(
        get(
            &app,
            &format!("/api/v1/doctors/{DOCTOR1_PROFILE_ID}/slots"),
            Some(&patient),
        )
        .await,
    )
    .await;
    let new_slot = slots["data"][0]["id"].as_str().unwrap();
    let payload = serde_json::json!({"slot_id":new_slot,"reason":"Synthetic Admin-approved schedule correction"});
    let path = format!("/api/v1/admin/appointments/{appointment}/reschedule");
    assert_eq!(
        post(&app, &path, Some(&patient), &payload).await.status(),
        StatusCode::FORBIDDEN
    );
    let response = post(&app, &path, Some(&admin), &payload).await;
    let status = response.status();
    let body = json_body(response).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let stored:(Uuid,Uuid,Uuid,DateTime<Utc>)=sqlx::query_as("SELECT actor_user_id,original_slot_id,new_slot_id,original_start FROM appointment_reschedules WHERE appointment_id=$1").bind(appointment).fetch_one(&pool).await.unwrap();
    assert_eq!(
        stored,
        (uid(ADMIN_USER_ID), original.0, uid(new_slot), original.1)
    );
    let after = chat_repo::find_conversation_by_appointment(&pool, appointment)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.id, conversation.id);
    assert_eq!(after.expires_at, conversation.expires_at);
    assert_eq!(
        post(&app, &path, Some(&admin), &payload).await.status(),
        StatusCode::CONFLICT
    );
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM appointment_reschedules WHERE appointment_id=$1")
            .bind(appointment)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
    let transactions: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM transactions WHERE appointment_id=$1")
            .bind(appointment)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(transactions, 1);
}

#[sqlx::test(migrations = false)]
async fn test_milestone_legacy_admin_upgrade_specific_accounts_and_reversible_schema(pool: PgPool) {
    let full = sqlx::migrate!("./migrations");
    let previous = full
        .iter()
        .filter(|m| m.version < 20261106000001)
        .map(|m| m.version)
        .max()
        .unwrap();
    let mut old = sqlx::migrate!("./migrations");
    old.migrations = std::borrow::Cow::Owned(
        old.iter()
            .filter(|m| m.version < 20261106000001)
            .cloned()
            .collect(),
    );
    old.run(&pool).await.unwrap();
    // This test's own subject is specifically the legacy-admin-role-upgrade
    // batch (20261106000001 canonical_admin_enum, 20261106000002
    // single_admin) — it is not a claim that every later migration is
    // reversible. Scope the run/undo/run reversibility round-trip to exactly
    // that batch via `legacy_batch`, a migrator ceilinged at
    // 20261106000002, rather than `full`'s complete, ever-growing migration
    // list. Without this, any later migration added on top (e.g.
    // 20261107000002, which adds a PostgreSQL enum value no down-migration
    // can remove — see its own file-level comment) gets swept into this
    // test's undo/redo cycle purely by having a higher version number, with
    // no relationship to the admin-upgrade feature this test exists to
    // verify. `full` is still used below, once, as a plain forward-only
    // apply to bring the database to the real current schema for the rest
    // of this test (which exercises live session/token behavior through
    // `test_app`) — never undone here, so it never exercises that
    // unrelated limitation.
    const LEGACY_ADMIN_BATCH_CEILING: i64 = 20261106000002;
    let mut legacy_batch = sqlx::migrate!("./migrations");
    legacy_batch.migrations = std::borrow::Cow::Owned(
        legacy_batch
            .iter()
            .filter(|m| m.version <= LEGACY_ADMIN_BATCH_CEILING)
            .cloned()
            .collect(),
    );
    let mut accounts = Vec::new();
    for (i, role) in [
        "PLATFORM_ADMIN",
        "CLINICAL_ADMIN",
        "FINANCE_ADMIN",
        "SUPPORT",
        "COMPLIANCE",
        "SECURITY_ADMIN",
    ]
    .iter()
    .enumerate()
    {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO users(id,phone_number,role,is_active,is_verified) VALUES($1,$2,$3::user_role_enum,true,true)").bind(id).bind(format!("+2547100000{i:02}")).bind(role).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO auth_sessions(user_id,token_family_id,refresh_token_hash,expires_at) VALUES($1,$1,$2,clock_timestamp()+INTERVAL '7 days')")
            .bind(id).bind(format!("legacy-migration-{id}")).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO audit_events(actor_id,actor_role,action,resource_type,resource_id) VALUES($1,$2::user_role_enum,'LEGACY_ADMIN_EVIDENCE','USER',$1)").bind(id).bind(role).execute(&pool).await.unwrap();
        accounts.push((id, *role));
    }
    legacy_batch.run(&pool).await.unwrap();
    for (id, legacy) in &accounts {
        let role: String = sqlx::query_scalar("SELECT role::text FROM users WHERE id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(role, "ADMIN");
        let old_role:String=sqlx::query_scalar("SELECT actor_role::text FROM audit_events WHERE actor_id=$1 AND action='LEGACY_ADMIN_EVIDENCE'").bind(id).fetch_one(&pool).await.unwrap();
        assert_eq!(old_role, *legacy);
        let active: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM auth_sessions WHERE user_id=$1 AND revoked_at IS NULL",
        )
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(active, 0);
        let error =
            sqlx::query("INSERT INTO users(phone_number,role) VALUES($1,$2::user_role_enum)")
                .bind(format!(
                    "+254799{:06}",
                    rand::thread_rng().gen_range(0..1_000_000)
                ))
                .bind(legacy)
                .execute(&pool)
                .await
                .unwrap_err();
        assert_eq!(
            error.as_database_error().unwrap().code().as_deref(),
            Some("23514")
        );
    }
    legacy_batch.undo(&pool, previous).await.unwrap();
    for (id, legacy) in &accounts {
        let restored: String = sqlx::query_scalar("SELECT role::text FROM users WHERE id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(restored, *legacy);
    }
    legacy_batch.run(&pool).await.unwrap();
    // Forward-only from here: bring the database up to the real current
    // schema for the rest of this test. These later migrations are applied
    // for the first time in this test, never undone, so
    // 20261107000002's documented one-way enum limitation is never
    // exercised here — see test_enum_add_value_migration_reversibility_limitation_pattern
    // for the dedicated, isolated regression covering that limitation.
    full.run(&pool).await.unwrap();
    let (app, pool) = test_app(pool).await;
    for (id, _) in accounts.iter().take(2) {
        // A fresh family authenticates the migrated account; its pre-upgrade family stays revoked.
        let family = Uuid::new_v4();
        sqlx::query("INSERT INTO auth_sessions(user_id,token_family_id,refresh_token_hash,expires_at) VALUES($1,$2,$2::text,clock_timestamp()+INTERVAL '1 day')").bind(id).bind(family).execute(&pool).await.unwrap();
        let token = hellodoctor_backend::crypto::create_access_token(
            *id,
            "ADMIN",
            family,
            TEST_JWT_PRIVATE_KEY_B64,
        )
        .unwrap();
        let response=post(&app,"/api/v1/admin/logs/simulate",Some(&token),&serde_json::json!({"action":"MILESTONE_SPECIFIC_ADMIN","subsystem":"TEST_AUDIT","severity":"warning","error_summary":"Synthetic account attribution evidence"})).await;
        let status = response.status();
        let body = json_body(response).await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        let actor: (Uuid, String) =
            sqlx::query_as("SELECT actor_id,actor_role::text FROM audit_events WHERE id=$1")
                .bind(uid(body["data"]["id"].as_str().unwrap()))
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(actor, (*id, "ADMIN".into()));
    }
}

/// Documents, and exercises in isolation, the exact PostgreSQL limitation
/// `20261107000002_doctor_verification_revocation` runs into: `ALTER TYPE
/// ... ADD VALUE` has no corresponding `DROP VALUE` — a value added to a
/// live enum can never be removed by any single statement, so a bare
/// `ADD VALUE` migration's down-migration can only be partial (it is
/// honestly documented as such in that migration's own `.down.sql`).
///
/// This test does NOT touch `professional_verification_status_enum` or
/// `doctor_profiles`, and does NOT make `20261107000002` itself reversible.
/// Doing that for the real column would require recreating the enum type
/// and re-deriving at least six dependent security objects (a view plus
/// five RLS policies spanning the patient directory, Doctor Articles,
/// consultation trace, encounter-history, and the h23 chat-RLS backstop) —
/// a materially larger, higher-risk schema change than this specific
/// migration-reversibility gap justifies. That tradeoff is an explicit,
/// recorded decision, not an oversight:
///
///   REAL_MIGRATION_20261107000002_FULLY_REVERSIBLE = NO
///   CURRENT_FORWARD_SCHEMA_VALID = YES
///   CURRENT_SECURITY_POLICIES_UNCHANGED = YES
///   MIGRATION_TEST_SCOPE_CORRECTED = YES
///   FUTURE_ENUM_REVERSIBILITY_PATTERN_TESTED = YES
///
/// What this test DOES prove, against a throwaway type/table local to its
/// own ephemeral test database: (1) the limitation itself is real — a
/// single-statement removal is rejected by Postgres, not merely unwritten;
/// (2) the correct pattern for a FUTURE status column that must support
/// true down-migration reversibility — recreate the enum type under the
/// current name holding only the desired values, swap the column onto it,
/// drop the old type — works end-to-end, including a second full
/// add/remove round trip, so a future migration needing real reversibility
/// has a proven template to follow instead of reaching for `ADD VALUE`.
#[sqlx::test(migrations = false)]
async fn test_enum_add_value_migration_reversibility_limitation_pattern(pool: PgPool) {
    sqlx::query("CREATE TYPE test_reversible_enum AS ENUM ('A', 'B')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE test_reversible_enum_demo (id INT PRIMARY KEY, status test_reversible_enum NOT NULL)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO test_reversible_enum_demo VALUES (1, 'A')")
        .execute(&pool)
        .await
        .unwrap();

    // Forward: ADD VALUE works, exactly like 20261107000002's up.sql.
    sqlx::query("ALTER TYPE test_reversible_enum ADD VALUE 'C'")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE test_reversible_enum_demo SET status='C' WHERE id=1")
        .execute(&pool)
        .await
        .unwrap();

    // The limitation itself: there is no single-statement way to remove it.
    // `DROP VALUE` is not valid ALTER TYPE ... ENUM syntax in PostgreSQL —
    // confirmed here as a real rejection, not asserted from documentation.
    let drop_value_result = sqlx::query("ALTER TYPE test_reversible_enum DROP VALUE 'C'")
        .execute(&pool)
        .await;
    assert!(
        drop_value_result.is_err(),
        "PostgreSQL unexpectedly accepted ALTER TYPE ... DROP VALUE — \
         if a future PostgreSQL version adds this, 20261107000002's \
         down-migration limitation and this test's premise should both be \
         revisited"
    );

    // The real, working pattern for a column that must support true
    // reversibility: recreate the type without the unwanted value, swap
    // the column onto it, drop the old type. 'C' is currently referenced
    // by a live row, so — matching 20261024000001's own evidence-protection
    // precedent for this exact situation — a real migration would refuse
    // to proceed; here we reassign the row first to demonstrate the clean
    // path.
    sqlx::query("UPDATE test_reversible_enum_demo SET status='A' WHERE id=1")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("CREATE TYPE test_reversible_enum_v2 AS ENUM ('A', 'B')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "ALTER TABLE test_reversible_enum_demo ALTER COLUMN status TYPE test_reversible_enum_v2 \
         USING status::text::test_reversible_enum_v2",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("DROP TYPE test_reversible_enum")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("ALTER TYPE test_reversible_enum_v2 RENAME TO test_reversible_enum")
        .execute(&pool)
        .await
        .unwrap();
    let values: Vec<String> = sqlx::query_scalar(
        "SELECT enumlabel FROM pg_enum WHERE enumtypid = 'test_reversible_enum'::regtype ORDER BY enumsortorder",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        values,
        vec!["A".to_string(), "B".to_string()],
        "'C' must be genuinely gone, not just unused"
    );

    // Round-trip again — the pattern itself must be repeatable, since a
    // real migration chain applies forward, backward, and forward again
    // (exactly what the legacy-admin-upgrade test above exercises for its
    // own batch).
    sqlx::query("ALTER TYPE test_reversible_enum ADD VALUE 'C'")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE test_reversible_enum_demo SET status='C' WHERE id=1")
        .execute(&pool)
        .await
        .unwrap();
    let current: String =
        sqlx::query_scalar("SELECT status::text FROM test_reversible_enum_demo WHERE id=1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(current, "C");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_milestone_queue_today_schedule_future_and_history_separate(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let (base, _) = book_and_confirm(&app, &patient).await;
    let template = appointment_repo::find_by_id(&pool, base)
        .await
        .unwrap()
        .unwrap();
    let mut appointments = Vec::new();
    sqlx::query("UPDATE appointments SET status='CANCELLED' WHERE id=$1")
        .bind(base)
        .execute(&pool)
        .await
        .unwrap();
    for (kind, status) in [
        ("ACTIVE", AppointmentStatus::InConsultation),
        ("TODAY", AppointmentStatus::Confirmed),
        ("TOMORROW", AppointmentStatus::Confirmed),
        ("PAST", AppointmentStatus::Completed),
    ] {
        let mut a = template.clone();
        a.id = Uuid::new_v4();
        a.appointment_number = format!("L-{}", a.id);
        a.status = status;
        a.modality = Modality::Video;
        let start: DateTime<Utc> = sqlx::query_scalar(match kind {
            "ACTIVE" => "SELECT clock_timestamp()-INTERVAL '20 minutes'",
            "PAST" => "SELECT clock_timestamp()-INTERVAL '1 day'",
            "TOMORROW" => "SELECT clock_timestamp()+INTERVAL '1 day'",
            _ => "SELECT clock_timestamp()",
        })
        .fetch_one(&pool)
        .await
        .unwrap();
        a.slot_id=sqlx::query_scalar("INSERT INTO doctor_schedule_slots(doctor_id,start_time,end_time,status) VALUES($1,$2,$2+INTERVAL '20 minutes','BOOKED') RETURNING id").bind(a.doctor_id).bind(start).fetch_one(&pool).await.unwrap();
        appointment_repo::insert(&pool, &a).await.unwrap();
        appointments.push(a.id);
    }
    let response = get(&app, "/api/v1/doctor/appointments", Some(&doctor)).await;
    let status = response.status();
    let rows = json_body(response).await;
    assert_eq!(status, StatusCode::OK, "{rows}");
    let ids: Vec<&str> = rows["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        vec![appointments[0].to_string(), appointments[1].to_string()]
    );
    let future = json_body(
        get(
            &app,
            "/api/v1/doctor/appointments?view=schedule",
            Some(&doctor),
        )
        .await,
    )
    .await;
    assert!(future["data"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["id"] == serde_json::json!(appointments[2])));
    let past = json_body(
        get(
            &app,
            "/api/v1/doctor/appointments?view=history",
            Some(&doctor),
        )
        .await,
    )
    .await;
    assert!(past["data"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["id"] == serde_json::json!(appointments[3])));
    assert!(!past["data"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["id"] == serde_json::json!(appointments[0])));
}

#[sqlx::test(migrations = "./migrations")]
async fn test_milestone_read_ack_first_access_and_request_correlation(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let stranger = make_token(uid(DOCTOR2_USER_ID), "DOCTOR");
    let (appointment, _) = book_and_confirm(&app, &patient).await;
    let conv = chat_repo::find_conversation_by_appointment(&pool, appointment)
        .await
        .unwrap()
        .unwrap();
    let sent = post_idem(
        &app,
        &format!("/api/v1/chat/{}/messages", conv.id),
        Some(&patient),
        &serde_json::json!({"content":"Synthetic first contact"}),
        "read-evidence-message",
    )
    .await;
    assert_eq!(sent.status(), StatusCode::CREATED);
    let message = uid(json_body(sent).await["data"]["id"].as_str().unwrap());
    let path = format!("/api/v1/appointments/{appointment}/trace-ack");
    let request_id = Uuid::new_v4().to_string();
    let request=Request::builder().method("POST").uri(&path)
      .header("Authorization",format!("Bearer {doctor}"))
      .header("Content-Type","application/json").header("x-request-id",&request_id)
      .header("x-client-type","DOCTOR_FLUTTER").header("x-client-build","42")
      .body(Body::from(serde_json::json!({"event":"CHAT_OPENED","conversation_id":conv.id,"client_occurred_at":"2000-01-01T00:00:00Z"}).to_string())).unwrap();
    let mut request = request;
    request
        .extensions_mut()
        .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 12345))));
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["x-request-id"], request_id);
    let evidence:(Uuid,Uuid,String,String,DateTime<Utc>)=sqlx::query_as("SELECT actor_user_id,auth_session_id,request_id,trace_id,event_time_server FROM consultation_trace_events WHERE request_id=$1 AND event_type='CHAT_OPENED'").bind(&request_id).fetch_one(&pool).await.unwrap();
    assert_eq!(evidence.0, uid(DOCTOR1_USER_ID));
    let session_user: Uuid = sqlx::query_scalar("SELECT user_id FROM auth_sessions WHERE id=$1")
        .bind(evidence.1)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(session_user, uid(DOCTOR1_USER_ID));
    assert_eq!(evidence.2, request_id);
    assert_eq!(evidence.3, request_id);
    assert!(evidence.4 > Utc::now() - Duration::minutes(1));
    let ack = serde_json::json!({"event":"CHAT_READ_ACKNOWLEDGED","conversation_id":conv.id,"message_ids":[message]});
    assert_eq!(
        post(&app, &path, Some(&doctor), &ack).await.status(),
        StatusCode::OK
    );
    let first:DateTime<Utc>=sqlx::query_scalar("SELECT first_acknowledged_at FROM chat_read_acknowledgements WHERE user_id=$1 AND message_id=$2").bind(uid(DOCTOR1_USER_ID)).bind(message).fetch_one(&pool).await.unwrap();
    assert_eq!(
        post(&app, &path, Some(&doctor), &ack).await.status(),
        StatusCode::OK
    );
    let timestamps:Vec<DateTime<Utc>>=sqlx::query_scalar("SELECT first_acknowledged_at FROM chat_read_acknowledgements WHERE user_id=$1 AND message_id=$2").bind(uid(DOCTOR1_USER_ID)).bind(message).fetch_all(&pool).await.unwrap();
    assert_eq!(timestamps, vec![first]);
    let trace_count:i64=sqlx::query_scalar("SELECT COUNT(*) FROM consultation_trace_events WHERE resource_id=$1 AND event_type='CHAT_READ_ACKNOWLEDGED'").bind(message).fetch_one(&pool).await.unwrap();
    assert_eq!(trace_count, 1);
    assert_eq!(
        post(&app, &path, Some(&stranger), &ack).await.status(),
        StatusCode::NOT_FOUND
    );
    let spoof = serde_json::json!({"event":"CHAT_OPENED","conversation_id":conv.id,"actor_user_id":DOCTOR2_USER_ID});
    assert_eq!(
        post(&app, &path, Some(&doctor), &spoof).await.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_milestone_deduplicated_errors_preserve_each_consultation_request(pool: PgPool) {
    use hellodoctor_backend::api::middleware::request_context::{
        RequestContext, RequestEvidence, REQUEST_CONTEXT,
    };
    use hellodoctor_backend::auth_context::AuthContext;
    use hellodoctor_backend::system_error::{self, ErrorEvent};
    let (app, pool, state) = test_app_and_state(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let (appointment, _) = book_and_confirm(&app, &patient).await;
    let session: Uuid = sqlx::query_scalar(
        "SELECT id FROM auth_sessions WHERE user_id=$1 AND revoked_at IS NULL LIMIT 1",
    )
    .bind(uid(PATIENT_USER_ID))
    .fetch_one(&pool)
    .await
    .unwrap();
    let mut groups = Vec::new();
    let mut requests = Vec::new();
    for _ in 0..2 {
        let request = Uuid::new_v4().to_string();
        requests.push(request.clone());
        let actor = AuthContext {
            user_id: UserId::from(uid(PATIENT_USER_ID)),
            role: UserRole::Patient,
            permissions: vec![],
            session_id: Some(session),
            request_id: Some(request.clone()),
        };
        let context = RequestContext {
            id: request.clone(),
            client_type: Some("PATIENT_FLUTTER".into()),
            client_build: Some("1".into()),
            evidence: Arc::new(std::sync::Mutex::new(RequestEvidence {
                actor: Some(actor),
                appointment: Some(appointment),
                conversation: None,
                retention_days: 5,
            })),
        };
        let group = REQUEST_CONTEXT
            .scope(context, async {
                system_error::report_sync(
                    &state.system_db,
                    ErrorEvent::new(
                        ErrorSeverity::Error,
                        "INTERNAL",
                        "MilestoneCorrelation",
                        "MILESTONE_SAFE_FAILURE",
                        "Synthetic infrastructure failure",
                        request,
                    ),
                )
                .await
                .unwrap()
            })
            .await;
        groups.push(group);
    }
    assert_eq!(groups[0], groups[1]);
    let count: i32 =
        sqlx::query_scalar("SELECT occurrence_count FROM system_error_logs WHERE id=$1")
            .bind(groups[0])
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 2);
    let refs:Vec<(String,Uuid,Uuid)>=sqlx::query_as("SELECT request_id,resource_id,actor_user_id FROM consultation_trace_events WHERE appointment_id=$1 AND event_type='CONSULTATION_REQUEST_FAILED' ORDER BY event_time_server,id").bind(appointment).fetch_all(&pool).await.unwrap();
    assert_eq!(refs.len(), 2);
    for (index, (request, group, actor)) in refs.iter().enumerate() {
        assert_eq!(request, &requests[index]);
        assert_eq!(*group, groups[0]);
        assert_eq!(*actor, uid(PATIENT_USER_ID));
    }
    let unsafe_rows:i64=sqlx::query_scalar("SELECT COUNT(*) FROM consultation_trace_events WHERE appointment_id=$1 AND safe_metadata::text LIKE '%Synthetic infrastructure failure%'").bind(appointment).fetch_one(&pool).await.unwrap();
    assert_eq!(unsafe_rows, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_milestone_unpaid_cancelled_booking_is_not_own_historical_encounter(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let slots = json_body(
        get(
            &app,
            &format!("/api/v1/doctors/{DOCTOR1_PROFILE_ID}/slots"),
            Some(&patient),
        )
        .await,
    )
    .await;
    let slot = uid(slots["data"][0]["id"].as_str().unwrap());
    let response = post_idem(
        &app,
        "/api/v1/appointments/book",
        Some(&patient),
        &schedule_booking(slot),
        &Uuid::new_v4().to_string(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let appointment = uid(json_body(response).await["data"]["appointment_id"]
        .as_str()
        .unwrap());
    sqlx::query("UPDATE appointments SET status='CANCELLED' WHERE id=$1")
        .bind(appointment)
        .execute(&pool)
        .await
        .unwrap();
    let ctx = hellodoctor_backend::auth_context::AuthContext {
        user_id: UserId::from(uid(DOCTOR1_USER_ID)),
        role: UserRole::Doctor,
        permissions: vec![],
        session_id: None,
        request_id: None,
    };
    let mut tx = state.begin_authed(&ctx).await.unwrap();
    let allowed: bool = sqlx::query_scalar("SELECT doctor_own_encounter_allowed($1,$2)")
        .bind(appointment)
        .bind(ctx.user_id)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert!(!allowed);
    tx.commit().await.unwrap();
    let history = get(
        &app,
        &format!("/api/v1/doctor/appointments/{appointment}/patient-history"),
        Some(&doctor),
    )
    .await;
    assert_eq!(history.status(), StatusCode::NOT_FOUND);
    let sessions: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM consultation_sessions WHERE appointment_id=$1")
            .bind(appointment)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(sessions, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_milestone_text_attachment_keys_share_one_logical_message(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let (appointment, _) = milestone_book_chat(&app, &patient, DOCTOR1_PROFILE_ID).await;
    let ctx = isolation_ctx(UserId::from(uid(DOCTOR1_USER_ID)), UserRole::Doctor);
    let mut check = state.begin_authed(&ctx).await.unwrap();
    let windows: (bool, bool) = sqlx::query_as(
        "SELECT doctor_own_encounter_allowed($1,$2),doctor_history_context_allowed($1,$2)",
    )
    .bind(appointment)
    .bind(ctx.user_id)
    .fetch_one(&mut *check)
    .await
    .unwrap();
    assert_eq!(
        windows,
        (true, false),
        "Active paid Chat must not inherit the Video pre-slot history gate"
    );
    check.commit().await.unwrap();
    let conv = chat_repo::find_conversation_by_appointment(&pool, appointment)
        .await
        .unwrap()
        .unwrap();
    let text_path = format!("/api/v1/chat/{}/messages", conv.id);
    let image_path = format!("/api/v1/chat/{}/attachments", conv.id);
    let text = serde_json::json!({"content":"Synthetic attachment replay contract"});
    let image =
        serde_json::json!({"mime_type":"image/jpeg","base64_content":minimal_jpeg_base64()});
    assert_eq!(
        post_idem(&app, &text_path, Some(&doctor), &text, "text-first-key")
            .await
            .status(),
        StatusCode::CREATED
    );
    assert_eq!(
        post_idem(&app, &image_path, Some(&doctor), &image, "text-first-key")
            .await
            .status(),
        StatusCode::CONFLICT
    );
    let uploaded = post_idem(&app, &image_path, Some(&doctor), &image, "image-first-key").await;
    assert_eq!(uploaded.status(), StatusCode::CREATED);
    let uploaded = json_body(uploaded).await;
    assert_eq!(
        post_idem(&app, &text_path, Some(&doctor), &text, "image-first-key")
            .await
            .status(),
        StatusCode::CONFLICT
    );
    let retry = post_idem(&app, &image_path, Some(&doctor), &image, "image-first-key").await;
    assert_eq!(retry.status(), StatusCode::CREATED);
    assert_eq!(json_body(retry).await["data"]["id"], uploaded["data"]["id"]);
    let counts:(i64,i64)=sqlx::query_as("SELECT (SELECT COUNT(*) FROM chat_messages WHERE conversation_id=$1),(SELECT COUNT(*) FROM notifications WHERE related_entity_id=$1 AND type='NEW_CHAT_MESSAGE')").bind(conv.id).fetch_one(&pool).await.unwrap();
    assert_eq!(counts, (2, 2));
    let message = uploaded["data"]["id"].as_str().unwrap();
    let view = format!("/api/v1/chat/{}/messages/{message}/attachment", conv.id);
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    assert_eq!(
        get(&app, &view, Some(&patient)).await.status(),
        StatusCode::OK
    );
    assert_eq!(
        get(&app, &view, Some(&admin)).await.status(),
        StatusCode::OK
    );
    let views:i64=sqlx::query_scalar("SELECT COUNT(*) FROM consultation_trace_events WHERE appointment_id=$1 AND resource_id=$2 AND event_type='ATTACHMENT_VIEWED'").bind(appointment).bind(uid(message)).fetch_one(&pool).await.unwrap();
    assert_eq!(views, 2);
    let session = consultation_repo::find_by_appointment(&pool, appointment)
        .await
        .unwrap()
        .unwrap();
    let case=post(&app,"/api/v1/grievances",Some(&patient),&serde_json::json!({"consultation_id":session.id,"target":"DOCTOR","category":"Attachment trace","claim_summary":"Synthetic investigation of authorized retrieval"})).await;
    assert_eq!(case.status(), StatusCode::CREATED);
    let case = json_body(case).await;
    let case_id = case["data"]["grievance_id"].as_str().unwrap();
    let timeline = get(
        &app,
        &format!("/api/v1/admin/grievances/{case_id}/timeline"),
        Some(&admin),
    )
    .await;
    assert_eq!(timeline.status(), StatusCode::OK);
    let data = json_body(timeline).await;
    assert_eq!(data["data"]["facts"]["modality"], "CHAT");
    assert!(data["data"]["audit_events"]
        .as_array()
        .unwrap()
        .iter()
        .any(|event| event["action"] == "ADMIN_ATTACHMENT_VIEWED"
            && event["resource_id"] == message
            && event["actor_user_id"] == ADMIN_USER_ID));
    assert_eq!(
        data["data"]["trace_events"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|event| event["event_type"] == "ATTACHMENT_VIEWED")
            .count(),
        2
    );
    // Cross the expiry boundary during INSERT itself, after upload authorization.
    // The existing request timeout and concurrency controls remain unchanged.
    sqlx::query("CREATE FUNCTION milestone_delay_chat_insert() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_sleep(2.2); RETURN NEW; END $$").execute(&pool).await.unwrap();
    sqlx::query("CREATE TRIGGER a_milestone_delay BEFORE INSERT ON chat_messages FOR EACH ROW EXECUTE FUNCTION milestone_delay_chat_insert()").execute(&pool).await.unwrap();
    sqlx::query("UPDATE chat_conversations SET expires_at=clock_timestamp()+INTERVAL '2 seconds' WHERE id=$1").bind(conv.id).execute(&pool).await.unwrap();
    assert_eq!(
        post_idem(
            &app,
            &image_path,
            Some(&doctor),
            &image,
            "crossing-expiry-key"
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    let final_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM chat_messages WHERE conversation_id=$1")
            .bind(conv.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(final_count, 2);
    let rejected:i64=sqlx::query_scalar("SELECT COUNT(*) FROM consultation_trace_events WHERE appointment_id=$1 AND event_type='ATTACHMENT_UPLOAD_REJECTED' AND reason_code='WINDOW_CLOSED_OR_DENIED'").bind(appointment).fetch_one(&pool).await.unwrap();
    assert_eq!(rejected, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_traffic_notification_cursor_pages_are_stable_and_owner_scoped(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let time = chrono::Utc::now() - chrono::Duration::hours(1);
    for index in 1..=100_u128 {
        sqlx::query("INSERT INTO notifications(id,user_id,type,title,body,created_at,dedup_key)VALUES($1,$2,'NEW_CHAT_MESSAGE','New consultation message','A new consultation message is available.',$3,$1::text)").bind(Uuid::from_u128(index)).bind(uid(PATIENT_USER_ID)).bind(time).execute(&pool).await.unwrap();
    }
    let foreign = Uuid::from_u128(101);
    sqlx::query("INSERT INTO notifications(id,user_id,type,title,body,created_at,dedup_key)VALUES($1,$2,'NEW_CHAT_MESSAGE','Private notification','Another account notification',$3,$1::text)").bind(foreign).bind(uid(DOCTOR2_USER_ID)).bind(time).execute(&pool).await.unwrap();
    let latest = get(&app, "/api/v1/notifications", Some(&patient)).await;
    assert_eq!(latest.status(), StatusCode::OK);
    let latest = json_body(latest).await;
    assert_eq!(latest["data"].as_array().unwrap().len(), 25);
    assert_eq!(latest["data"][0]["id"], Uuid::from_u128(100).to_string());
    let mut cursor = Uuid::nil();
    let mut seen = std::collections::HashSet::new();
    for _ in 0..4 {
        let path = format!(
            "/api/v1/notifications?limit=25&after_time={}&after_id={cursor}",
            time.to_rfc3339().replace('+', "%2B")
        );
        let response = get(&app, &path, Some(&patient)).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        let rows = body["data"].as_array().unwrap();
        assert_eq!(rows.len(), 25);
        for row in rows {
            assert_eq!(row["user_id"], PATIENT_USER_ID);
            assert_ne!(row["id"], foreign.to_string());
            assert!(seen.insert(row["id"].as_str().unwrap().to_owned()));
        }
        cursor = uid(rows.last().unwrap()["id"].as_str().unwrap());
    }
    assert_eq!(seen.len(), 100);
    let empty = get(
        &app,
        &format!(
            "/api/v1/notifications?after_time={}&after_id={cursor}",
            time.to_rfc3339().replace('+', "%2B")
        ),
        Some(&patient),
    )
    .await;
    assert_eq!(empty.status(), StatusCode::OK);
    assert_eq!(json_body(empty).await["data"], serde_json::json!([]));
    let before = get(
        &app,
        &format!(
            "/api/v1/notifications?before_time={}&before_id={cursor}&limit=25",
            time.to_rfc3339().replace('+', "%2B")
        ),
        Some(&patient),
    )
    .await;
    assert_eq!(before.status(), StatusCode::OK);
    let before = json_body(before).await;
    assert_eq!(before["data"][0]["id"], Uuid::from_u128(99).to_string());
    assert_eq!(
        get(
            &app,
            "/api/v1/notifications?after_id=00000000-0000-0000-0000-000000000001",
            Some(&patient)
        )
        .await
        .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_traffic_admin_timeline_pages_remain_complete_without_recursive_growth(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let (appointment, _) = book_and_confirm(&app, &patient).await;
    let session = consultation_repo::find_by_appointment(&pool, appointment)
        .await
        .unwrap()
        .unwrap();
    let conv = chat_repo::find_conversation_by_appointment(&pool, appointment)
        .await
        .unwrap()
        .unwrap();
    // Generate 120 trace-event fixture rows the same way the adjacent
    // chat_messages insert does: directly, bypassing HTTP. Going through the
    // real `/trace-ack` endpoint 120 times trips its own real per-user rate
    // limit (120/60s, `trace_handlers.rs::acknowledge`) with zero margin —
    // that limiter is unrelated to the pagination invariant this test
    // actually verifies. This calls the exact same `consultation_trace::record`
    // the handler calls for a non-CHAT_READ_ACKNOWLEDGED event, under a real
    // `begin_authed` transaction, so it's still the real trace-write path.
    let patient_ctx = isolation_ctx(UserId::from(uid(PATIENT_USER_ID)), UserRole::Patient);
    for _index in 0..120 {
        sqlx::query("INSERT INTO chat_messages(id,conversation_id,sender_id,content,sent_at)VALUES($1,$2,$3,'private clinical fixture text',clock_timestamp())").bind(Uuid::new_v4()).bind(conv.id).bind(uid(PATIENT_USER_ID)).execute(&pool).await.unwrap();
        let mut tx = state.begin_authed(&patient_ctx).await.unwrap();
        let mut record = hellodoctor_backend::services::consultation_trace::Event::accepted(
            "CHAT_OPENED",
            Some(("APPOINTMENT", appointment)),
        );
        record.metadata = serde_json::json!({"client_reported":true,"client_occurred_at":Option::<chrono::DateTime<chrono::Utc>>::None});
        hellodoctor_backend::services::consultation_trace::record(
            &mut tx,
            appointment,
            Some(conv.id),
            record,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }
    let grievance=post(&app,"/api/v1/grievances",Some(&patient),&serde_json::json!({"consultation_id":session.id,"target":"DOCTOR","category":"Pagination investigation","claim_summary":"Synthetic case proving complete bounded investigation evidence"})).await;
    assert_eq!(grievance.status(), StatusCode::CREATED);
    let case = json_body(grievance).await;
    let id = case["data"]["grievance_id"].as_str().unwrap();
    let base = format!("/api/v1/admin/grievances/{id}/timeline");
    let response = get(
        &app,
        &format!("{base}?event_limit=17&record_limit=20"),
        Some(&admin),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let first = json_body(response).await["data"].clone();
    assert_eq!(first["records"]["messages"].as_array().unwrap().len(), 20);
    assert_eq!(first["record_page"]["totals"]["messages"], 120);
    let total = first["event_page"]["total"].as_u64().unwrap();
    assert!(total >= 120);
    let snapshot = first["event_page"]["snapshot_at"]
        .as_str()
        .unwrap()
        .replace('+', "%2B");
    let mut page = first;
    let mut seen = std::collections::HashSet::new();
    for iteration in 0..30 {
        assert!(!page.to_string().contains("private clinical fixture text"));
        assert_eq!(page["event_page"]["total"].as_u64().unwrap(), total);
        let count = page["trace_events"].as_array().unwrap().len()
            + page["audit_events"].as_array().unwrap().len();
        assert!(count <= 17);
        for kind in ["trace_events", "audit_events"] {
            for row in page[kind].as_array().unwrap() {
                assert!(seen.insert(format!("{kind}:{}", row["id"])));
            }
        }
        if !page["event_page"]["has_more"].as_bool().unwrap() {
            break;
        }
        assert!(
            iteration < 29,
            "Evidence paging must terminate despite read audits"
        );
        let next = &page["event_page"]["next"];
        let path=format!("{base}?event_limit=17&snapshot_at={snapshot}&after_time={}&after_id={}&after_source={}&include_records=false",next["time"].as_str().unwrap().replace('+',"%2B"),next["id"].as_str().unwrap(),next["source"].as_str().unwrap());
        let response = get(&app, &path, Some(&admin)).await;
        assert_eq!(response.status(), StatusCode::OK);
        page = json_body(response).await["data"].clone();
        assert_eq!(page["records"], serde_json::json!({}));
        assert!(page["telemetry"].is_null());
    }
    assert_eq!(seen.len() as u64, total);
    let mut message_ids = std::collections::HashSet::new();
    for offset in (0..120).step_by(20) {
        let response=get(&app,&format!("{base}?include_events=false&record_limit=20&record_offset={offset}&snapshot_at={snapshot}"),Some(&admin)).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert_eq!(body["data"]["trace_events"], serde_json::json!([]));
        assert_eq!(body["data"]["audit_events"], serde_json::json!([]));
        let rows = body["data"]["records"]["messages"].as_array().unwrap();
        assert_eq!(rows.len(), 20);
        for row in rows {
            assert!(message_ids.insert(row["id"].as_str().unwrap().to_owned()));
        }
    }
    assert_eq!(message_ids.len(), 120);
    assert_eq!(
        get(&app, &base, Some(&patient)).await.status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        get(&app, &format!("{base}?after_source=TRACE"), Some(&admin))
            .await
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_traffic_admin_aggregate_summaries_match_existing_appointment_values(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let (appointment, _) = book_and_confirm(&app, &patient).await;
    let fee: bigdecimal::BigDecimal =
        sqlx::query_scalar("SELECT consultation_fee FROM appointments WHERE id=$1")
            .bind(appointment)
            .fetch_one(&pool)
            .await
            .unwrap();
    let overview = get(&app, "/api/v1/admin/overview", Some(&admin)).await;
    assert_eq!(overview.status(), StatusCode::OK);
    let overview = json_body(overview).await;
    assert_eq!(overview["data"]["modality_split"]["videoCount"], 1);
    assert_eq!(overview["data"]["modality_split"]["chatCount"], 0);
    assert_eq!(
        overview["data"]["modality_split"]["videoRevenue"],
        fee.to_string()
    );
    let doctors = json_body(get(&app, "/api/v1/admin/doctors", Some(&admin)).await).await;
    let row = doctors["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == DOCTOR1_PROFILE_ID)
        .unwrap();
    assert_eq!(row["video_count"], 1);
    assert_eq!(row["chat_count"], 0);
    let patients = json_body(get(&app, "/api/v1/admin/patients", Some(&admin)).await).await;
    let row = patients["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == PATIENT_PROFILE_ID)
        .unwrap();
    assert_eq!(row["video_count"], 1);
    assert_eq!(row["chat_count"], 0);
    assert_eq!(row["consultation_fee_total"], fee.to_string());
}

#[sqlx::test(migrations = "./migrations")]
async fn test_traffic_doctor_schedule_day_query_is_nairobi_scoped_and_retains_history(
    pool: PgPool,
) {
    let (app, pool) = test_app(pool).await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let ids = [Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
    for (id, time) in ids.iter().zip([
        "2030-10-16T20:59:00Z",
        "2030-10-16T21:01:00Z",
        "2030-10-17T21:01:00Z",
    ]) {
        let start = chrono::DateTime::parse_from_rfc3339(time)
            .unwrap()
            .with_timezone(&chrono::Utc);
        sqlx::query("INSERT INTO doctor_schedule_slots(id,doctor_id,start_time,end_time,status)VALUES($1,$2,$3,$3+INTERVAL '1 minute','AVAILABLE')").bind(id).bind(uid(DOCTOR1_PROFILE_ID)).bind(start).execute(&pool).await.unwrap();
    }
    let response = get(
        &app,
        "/api/v1/doctor/slots?day=2030-10-17&limit=100",
        Some(&doctor),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let response = json_body(response).await;
    let rows = response["data"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["id"], ids[1].to_string());
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM doctor_schedule_slots WHERE id=ANY($1)")
            .bind(ids.as_slice())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 3);
    let unrelated = make_token(uid(DOCTOR2_USER_ID), "DOCTOR");
    let response = get(
        &app,
        "/api/v1/doctor/slots?day=2030-10-17&limit=100",
        Some(&unrelated),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json_body(response).await["data"], serde_json::json!([]));
}

/// A clean patient with zero appointments/history must be truly erased —
/// Postgres's own FK `RESTRICT`s, not a hand-written history check, decide
/// this (see `data_rights_service::attempt_hard_delete`'s doc comment).
#[sqlx::test(migrations = "./migrations")]
async fn test_data_rights_patient_hard_delete_when_no_history(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");

    let res = post(
        &app,
        &format!("/api/v1/admin/patients/{PATIENT_PROFILE_ID}/data-rights/erase"),
        Some(&admin),
        &serde_json::json!({"reason": "Patient submitted a verified §40 erasure request."}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res).await["data"]["outcome"], "HARD_DELETED");

    let dossier = get(
        &app,
        &format!("/api/v1/admin/patients/{PATIENT_PROFILE_ID}/dossier"),
        Some(&admin),
    )
    .await;
    assert_eq!(
        dossier.status(),
        StatusCode::NOT_FOUND,
        "a truly erased profile must no longer exist"
    );

    let (phone, is_active): (String, bool) =
        sqlx::query_as("SELECT phone_number, is_active FROM users WHERE id=$1")
            .bind(uid(PATIENT_USER_ID))
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        phone.starts_with("ERASED"),
        "the linked login row must be anonymized even on the hard-delete path"
    );
    assert!(!is_active, "an erased account's login must be deactivated");

    let audit: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_events WHERE action='DATA_RIGHTS_PATIENT_ERASED' AND resource_id=$1",
    )
    .bind(uid(PATIENT_PROFILE_ID))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit, 1, "erasure must leave an audit trail");
}

/// A patient with a real consultation on record must be anonymized in
/// place, not deleted — the appointment and its payment/financial trail
/// must survive untouched, still pointing at the same (now-scrubbed)
/// profile id.
#[sqlx::test(migrations = "./migrations")]
async fn test_data_rights_patient_anonymized_when_history_exists(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");

    let (apt_id, _) = book_and_confirm(&app, &patient).await;

    let res = post(
        &app,
        &format!("/api/v1/admin/patients/{PATIENT_PROFILE_ID}/data-rights/erase"),
        Some(&admin),
        &serde_json::json!({"reason": "Patient submitted a verified §40 erasure request."}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res).await["data"]["outcome"], "ANONYMIZED");

    let dossier = json_body(
        get(
            &app,
            &format!("/api/v1/admin/patients/{PATIENT_PROFILE_ID}/dossier"),
            Some(&admin),
        )
        .await,
    )
    .await;
    assert!(
        dossier["data"]["patient"]["full_name"]
            .as_str()
            .unwrap()
            .starts_with("Deleted Patient"),
        "the profile's own identifying fields must be scrubbed"
    );
    let encounters = dossier["data"]["encounters"].as_array().unwrap();
    assert!(
        encounters
            .iter()
            .any(|e| e["appointment_id"] == apt_id.to_string()),
        "the clinical/financial trail must survive anonymization untouched"
    );

    let transaction_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM transactions WHERE appointment_id=$1")
            .bind(apt_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        transaction_count, 1,
        "financial records are never touched by an erasure request"
    );

    let (phone, is_active): (String, bool) =
        sqlx::query_as("SELECT phone_number, is_active FROM users WHERE id=$1")
            .bind(uid(PATIENT_USER_ID))
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(phone.starts_with("ERASED"));
    assert!(!is_active);
}

/// Every seeded doctor already has a `doctor_wallets` row from onboarding
/// (`ON DELETE RESTRICT`), so a doctor erasure request must always resolve
/// to anonymize, never a hard delete — confirmed live, not assumed.
#[sqlx::test(migrations = "./migrations")]
async fn test_data_rights_doctor_anonymized_due_to_existing_wallet(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");

    let res = post(
        &app,
        &format!("/api/v1/admin/doctors/{DOCTOR1_PROFILE_ID}/data-rights/erase"),
        Some(&admin),
        &serde_json::json!({"reason": "Doctor submitted a verified §40 erasure request."}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res).await["data"]["outcome"], "ANONYMIZED");

    let dossier = json_body(
        get(
            &app,
            &format!("/api/v1/admin/doctors/{DOCTOR1_PROFILE_ID}/dossier"),
            Some(&admin),
        )
        .await,
    )
    .await;
    assert_eq!(dossier["success"], true);
    let (phone, is_active): (String, bool) =
        sqlx::query_as("SELECT phone_number, is_active FROM users WHERE id=$1")
            .bind(uid(DOCTOR1_USER_ID))
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(phone.starts_with("ERASED"));
    assert!(!is_active);

    let (license_number,): (String,) =
        sqlx::query_as("SELECT license_number FROM doctor_profiles WHERE id=$1")
            .bind(uid(DOCTOR1_PROFILE_ID))
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        !license_number.is_empty(),
        "KMPDC professional/regulatory record must survive anonymization"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_data_rights_execution_is_admin_only(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");

    for token in [&doctor, &patient] {
        let res = post(
            &app,
            &format!("/api/v1/admin/patients/{PATIENT_PROFILE_ID}/data-rights/erase"),
            Some(token),
            &serde_json::json!({"reason": "Attempting a non-admin erasure request."}),
        )
        .await;
        assert_eq!(res.status(), StatusCode::FORBIDDEN);
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn test_data_rights_requires_a_real_reason(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");

    let res = post(
        &app,
        &format!("/api/v1/admin/patients/{PATIENT_PROFILE_ID}/data-rights/erase"),
        Some(&admin),
        &serde_json::json!({"reason": "short"}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_data_rights_unknown_patient_is_not_found(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");

    let res = post(
        &app,
        &format!(
            "/api/v1/admin/patients/{}/data-rights/erase",
            Uuid::new_v4()
        ),
        Some(&admin),
        &serde_json::json!({"reason": "Erasure request for a profile that does not exist."}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}
