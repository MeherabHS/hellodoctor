//! Phase 2: Secure Email OTP Password Recovery — end-to-end security and
//! regression tests against the real HTTP handlers, the real
//! `password_reset_requests` table, and (for a subset of tests) the real
//! `email_outbox` durable-tracking table. Every test explicitly enables
//! `password_reset_enabled` on its own `AppState` clone (the shared
//! `test_config()` default is `false`, matching production's fail-closed
//! default), so this file also proves the feature stays off unless a test
//! deliberately turns it on.
use super::*;
use async_trait::async_trait;
use hellodoctor_backend::services::email_transport::{EmailMessage, EmailOutcome, EmailTransport};
use std::sync::Mutex;

struct AlwaysSendTransport {
    sent: Mutex<Vec<(String, String)>>,
}

#[async_trait]
impl EmailTransport for AlwaysSendTransport {
    fn is_configured(&self) -> bool {
        true
    }
    async fn send_email(&self, message: &EmailMessage<'_>) -> EmailOutcome {
        self.sent
            .lock()
            .unwrap()
            .push((message.recipient.to_string(), message.subject.to_string()));
        EmailOutcome::SentToProvider {
            provider_message_id: None,
        }
    }
}

fn always_send() -> std::sync::Arc<AlwaysSendTransport> {
    std::sync::Arc::new(AlwaysSendTransport {
        sent: Mutex::new(Vec::new()),
    })
}

/// Enables the feature on a per-test `AppState` clone, exactly as
/// `AppConfig::validate_public_exposure` would only ever accept in
/// development/test — never mutating the shared `test_config()` default.
fn with_password_reset_enabled(state: &mut AppState) {
    let mut cfg = (*state.config).clone();
    cfg.password_reset_enabled = true;
    state.config = std::sync::Arc::new(cfg);
}

async fn outbox_row_for(pool: &PgPool, email_type: &str, recipient: &str) -> Option<String> {
    sqlx::query_scalar(
        "SELECT delivery_status FROM email_outbox \
         WHERE email_type=$1 AND recipient_email=$2 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(email_type)
    .bind(recipient)
    .fetch_optional(pool)
    .await
    .unwrap()
}

async fn request_reset(app: &axum::Router, email: &str) -> Value {
    let response = post(
        app,
        "/api/v1/auth/password-reset/request",
        None,
        &serde_json::json!({ "email": email }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    json_body(response).await
}

async fn verify_otp(app: &axum::Router, email: &str, otp_code: &str) -> axum::http::Response<Body> {
    post(
        app,
        "/api/v1/auth/password-reset/verify-otp",
        None,
        &serde_json::json!({ "email": email, "otp_code": otp_code }),
    )
    .await
}

async fn complete_reset(
    app: &axum::Router,
    reset_authorization: &str,
    new_password: &str,
) -> axum::http::Response<Body> {
    post(
        app,
        "/api/v1/auth/password-reset/complete",
        None,
        &serde_json::json!({
            "reset_authorization": reset_authorization,
            "new_password": new_password,
        }),
    )
    .await
}

#[sqlx::test(migrations = "./migrations")]
async fn disabled_by_default_returns_not_found(pool: PgPool) {
    // No `with_password_reset_enabled` call: proves production's fail-closed
    // default (`test_config().password_reset_enabled == false`) actually
    // blocks every step of this flow, not just the request step.
    let (_, _pool, state) = test_app_and_state(pool).await;
    let app = hellodoctor_backend::api::create_router(state);

    let response = post(
        &app,
        "/api/v1/auth/password-reset/request",
        None,
        &serde_json::json!({ "email": hellodoctor_backend::seed::SEED_DOCTOR1_EMAIL }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let response = verify_otp(
        &app,
        hellodoctor_backend::seed::SEED_DOCTOR1_EMAIL,
        "000000",
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let response = complete_reset(&app, "any-authorization", "Str0ng!NewPassw0rd").await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "./migrations")]
async fn doctor_happy_path_changes_password_and_revokes_the_old_session(pool: PgPool) {
    let (_, pool, mut state) = test_app_and_state(pool).await;
    with_password_reset_enabled(&mut state);
    let redis = state.redis.clone();
    reset_owned_redis(&redis).await;
    let app = hellodoctor_backend::api::create_router(state);

    // Establish a real, pre-reset session so we can prove it gets revoked.
    let login = post(
        &app,
        "/api/v1/auth/doctor/login",
        None,
        &serde_json::json!({
            "email": hellodoctor_backend::seed::SEED_DOCTOR1_EMAIL,
            "password": hellodoctor_backend::seed::SEED_DOCTOR1_PASSWORD,
        }),
    )
    .await;
    let login_body = json_body(login).await;
    let verify_login = post(
        &app,
        "/api/v1/auth/doctor/verify-otp",
        None,
        &serde_json::json!({
            "session_token": login_body["data"]["session_token"].as_str().unwrap(),
            "otp_code": login_body["data"]["debug_otp_code"].as_str().unwrap(),
        }),
    )
    .await;
    let verify_login_body = json_body(verify_login).await;
    let pre_reset_access = verify_login_body["data"]["access_token"]
        .as_str()
        .unwrap()
        .to_string();
    let pre_reset_refresh = verify_login_body["data"]["refresh_token"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        get(&app, "/api/v1/doctor/profile", Some(&pre_reset_access))
            .await
            .status(),
        StatusCode::OK
    );

    reset_owned_redis(&redis).await;

    let requested = request_reset(&app, hellodoctor_backend::seed::SEED_DOCTOR1_EMAIL).await;
    let code = requested["data"]["debug_otp_code"]
        .as_str()
        .expect("development + DEV_AUTH_OTP_ENABLED must echo the code back")
        .to_string();

    let verified = verify_otp(&app, hellodoctor_backend::seed::SEED_DOCTOR1_EMAIL, &code).await;
    assert_eq!(verified.status(), StatusCode::OK);
    let verified_body = json_body(verified).await;
    let authorization = verified_body["data"]["reset_authorization"]
        .as_str()
        .unwrap()
        .to_string();

    let completed = complete_reset(&app, &authorization, "Str0ng!NewPassw0rd").await;
    assert_eq!(completed.status(), StatusCode::OK);

    // Old password no longer works; new password does.
    let old_login = post(
        &app,
        "/api/v1/auth/doctor/login",
        None,
        &serde_json::json!({
            "email": hellodoctor_backend::seed::SEED_DOCTOR1_EMAIL,
            "password": hellodoctor_backend::seed::SEED_DOCTOR1_PASSWORD,
        }),
    )
    .await;
    assert_eq!(old_login.status(), StatusCode::UNAUTHORIZED);

    // The real per-recipient OTP-delivery cooldown (1 per 60s) would
    // otherwise reject this login's own second-factor code send, since the
    // reset OTP sent moments ago already spent it for this recipient — the
    // same rationale `a_new_request_invalidates_the_previous_one` and the
    // reference `change_password_sends_email_and_revokes_other_sessions`
    // both document.
    reset_owned_redis(&redis).await;
    let new_login = post(
        &app,
        "/api/v1/auth/doctor/login",
        None,
        &serde_json::json!({
            "email": hellodoctor_backend::seed::SEED_DOCTOR1_EMAIL,
            "password": "Str0ng!NewPassw0rd",
        }),
    )
    .await;
    assert_eq!(new_login.status(), StatusCode::OK);

    // The session that existed before the reset is now dead everywhere.
    assert_eq!(
        get(&app, "/api/v1/doctor/profile", Some(&pre_reset_access))
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let refresh_attempt = post(
        &app,
        "/api/v1/auth/refresh",
        None,
        &serde_json::json!({ "refresh_token": pre_reset_refresh }),
    )
    .await;
    assert_eq!(refresh_attempt.status(), StatusCode::UNAUTHORIZED);

    let row = outbox_row_for(
        &pool,
        "PASSWORD_CHANGED",
        hellodoctor_backend::seed::SEED_DOCTOR1_EMAIL,
    )
    .await;
    assert_eq!(row, None, "no email_transport override: PASSWORD_CHANGED is attempted but has nowhere configured to go, so no outbox row for it is expected without `always_send`");
}

#[sqlx::test(migrations = "./migrations")]
async fn patient_happy_path_via_a_freshly_registered_email_account(pool: PgPool) {
    let (_, _pool, mut state) = test_app_and_state(pool).await;
    with_password_reset_enabled(&mut state);
    let redis = state.redis.clone();
    reset_owned_redis(&redis).await;
    let app = hellodoctor_backend::api::create_router(state);

    let register = post(
        &app,
        "/api/v1/auth/patient/register",
        None,
        &serde_json::json!({
            "email": "reset.fixture.patient@example.invalid",
            "phone_number": "+254711000111",
            "password": "Str0ng!OldPassw0rd",
            "full_name": "Reset Fixture Patient",
            "date_of_birth": "1992-04-01",
            "gender": "FEMALE",
        }),
    )
    .await;
    assert_eq!(register.status(), StatusCode::OK);
    let register_body = json_body(register).await;
    let verify_register = post(
        &app,
        "/api/v1/auth/patient/verify-otp",
        None,
        &serde_json::json!({
            "challenge_token": register_body["data"]["challenge_token"].as_str().unwrap(),
            "otp_code": register_body["data"]["debug_otp_code"].as_str().unwrap(),
            "accepted_documents": [
                {"type": "PATIENT_DATA_SHARING_CONSENT", "version": "1.0-afyamkononi"},
                {"type": "PATIENT_TERMS", "version": "1.0-afyamkononi"},
                {"type": "PATIENT_PRIVACY_NOTICE", "version": "1.0-afyamkononi"},
                {"type": "PATIENT_DATA_RIGHTS", "version": "1.0"},
            ],
        }),
    )
    .await;
    assert_eq!(verify_register.status(), StatusCode::OK);
    reset_owned_redis(&redis).await;

    let requested = request_reset(&app, "reset.fixture.patient@example.invalid").await;
    let code = requested["data"]["debug_otp_code"]
        .as_str()
        .unwrap()
        .to_string();
    let verified = verify_otp(&app, "reset.fixture.patient@example.invalid", &code).await;
    assert_eq!(verified.status(), StatusCode::OK);
    let authorization = json_body(verified).await["data"]["reset_authorization"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        complete_reset(&app, &authorization, "Str0ng!NewPassw0rd")
            .await
            .status(),
        StatusCode::OK
    );

    let old_login = post(
        &app,
        "/api/v1/auth/patient/login",
        None,
        &serde_json::json!({
            "email": "reset.fixture.patient@example.invalid",
            "password": "Str0ng!OldPassw0rd",
        }),
    )
    .await;
    assert_eq!(old_login.status(), StatusCode::UNAUTHORIZED);

    // Same shared per-recipient OTP-delivery cooldown as
    // `doctor_happy_path_changes_password_and_revokes_the_old_session` —
    // the reset OTP just sent already spent it for this recipient.
    reset_owned_redis(&redis).await;
    let new_login = post(
        &app,
        "/api/v1/auth/patient/login",
        None,
        &serde_json::json!({
            "email": "reset.fixture.patient@example.invalid",
            "password": "Str0ng!NewPassw0rd",
        }),
    )
    .await;
    assert_eq!(new_login.status(), StatusCode::OK);
}

#[sqlx::test(migrations = "./migrations")]
async fn admin_accounts_are_excluded_and_indistinguishable_from_no_account_at_all(pool: PgPool) {
    let (_, pool, mut state) = test_app_and_state(pool).await;
    with_password_reset_enabled(&mut state);
    let redis = state.redis.clone();
    reset_owned_redis(&redis).await;
    let app = hellodoctor_backend::api::create_router(state);

    let admin_response = request_reset(&app, hellodoctor_backend::seed::SEED_ADMIN_EMAIL).await;
    reset_owned_redis(&redis).await;
    let nonexistent_response = request_reset(&app, "no-such-account@example.invalid").await;

    // Same generic message, same absent debug code, for a real (but
    // ineligible) account and an account that does not exist at all.
    assert_eq!(
        admin_response["data"]["message"],
        nonexistent_response["data"]["message"]
    );
    assert!(admin_response["data"]["debug_otp_code"].is_null());
    assert!(nonexistent_response["data"]["debug_otp_code"].is_null());

    let admin_row = outbox_row_for(
        &pool,
        "PASSWORD_RESET_OTP",
        hellodoctor_backend::seed::SEED_ADMIN_EMAIL,
    )
    .await;
    assert_eq!(
        admin_row, None,
        "Admin must never receive a reset OTP email"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn wrong_otp_code_is_rejected_and_grants_no_authorization(pool: PgPool) {
    let (_, _pool, mut state) = test_app_and_state(pool).await;
    with_password_reset_enabled(&mut state);
    reset_owned_redis(&state.redis.clone()).await;
    let app = hellodoctor_backend::api::create_router(state);

    request_reset(&app, hellodoctor_backend::seed::SEED_DOCTOR1_EMAIL).await;
    let response = verify_otp(
        &app,
        hellodoctor_backend::seed::SEED_DOCTOR1_EMAIL,
        "000000",
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "./migrations")]
async fn otp_cannot_be_reused_after_a_successful_verification(pool: PgPool) {
    let (_, _pool, mut state) = test_app_and_state(pool).await;
    with_password_reset_enabled(&mut state);
    reset_owned_redis(&state.redis.clone()).await;
    let app = hellodoctor_backend::api::create_router(state);

    let requested = request_reset(&app, hellodoctor_backend::seed::SEED_DOCTOR1_EMAIL).await;
    let code = requested["data"]["debug_otp_code"]
        .as_str()
        .unwrap()
        .to_string();

    let first = verify_otp(&app, hellodoctor_backend::seed::SEED_DOCTOR1_EMAIL, &code).await;
    assert_eq!(first.status(), StatusCode::OK);

    let replay = verify_otp(&app, hellodoctor_backend::seed::SEED_DOCTOR1_EMAIL, &code).await;
    assert_eq!(
        replay.status(),
        StatusCode::UNAUTHORIZED,
        "an already-verified request must not accept the OTP a second time"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn too_many_wrong_attempts_locks_out_the_request_even_for_the_correct_code(pool: PgPool) {
    let (_, _pool, mut state) = test_app_and_state(pool).await;
    with_password_reset_enabled(&mut state);
    reset_owned_redis(&state.redis.clone()).await;
    let app = hellodoctor_backend::api::create_router(state);

    let requested = request_reset(&app, hellodoctor_backend::seed::SEED_DOCTOR1_EMAIL).await;
    let code = requested["data"]["debug_otp_code"]
        .as_str()
        .unwrap()
        .to_string();

    for _ in 0..5 {
        let response = verify_otp(
            &app,
            hellodoctor_backend::seed::SEED_DOCTOR1_EMAIL,
            "000000",
        )
        .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    // By now the shared account-level brute-force jail (`ip_jail`,
    // independent of and in addition to this request's own 5-attempt
    // ceiling) has also tripped from the same 5 wrong attempts, so the
    // correct code is rejected at that earlier layer (403) rather than
    // reaching this request's own exhausted-attempts check (401) — either
    // way, the correct code must never succeed here.
    let final_attempt =
        verify_otp(&app, hellodoctor_backend::seed::SEED_DOCTOR1_EMAIL, &code).await;
    assert_eq!(
        final_attempt.status(),
        StatusCode::FORBIDDEN,
        "the correct code must still be rejected once max_attempts is exhausted"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn reset_authorization_cannot_be_reused_after_completion(pool: PgPool) {
    let (_, _pool, mut state) = test_app_and_state(pool).await;
    with_password_reset_enabled(&mut state);
    reset_owned_redis(&state.redis.clone()).await;
    let app = hellodoctor_backend::api::create_router(state);

    let requested = request_reset(&app, hellodoctor_backend::seed::SEED_DOCTOR1_EMAIL).await;
    let code = requested["data"]["debug_otp_code"]
        .as_str()
        .unwrap()
        .to_string();
    let verified = verify_otp(&app, hellodoctor_backend::seed::SEED_DOCTOR1_EMAIL, &code).await;
    let authorization = json_body(verified).await["data"]["reset_authorization"]
        .as_str()
        .unwrap()
        .to_string();

    assert_eq!(
        complete_reset(&app, &authorization, "Str0ng!FirstPassw0rd")
            .await
            .status(),
        StatusCode::OK
    );
    let replay = complete_reset(&app, &authorization, "Str0ng!SecondPassw0rd").await;
    assert_eq!(replay.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "./migrations")]
async fn expired_otp_is_rejected(pool: PgPool) {
    let (_, pool, mut state) = test_app_and_state(pool).await;
    with_password_reset_enabled(&mut state);
    reset_owned_redis(&state.redis.clone()).await;
    let app = hellodoctor_backend::api::create_router(state);

    let requested = request_reset(&app, hellodoctor_backend::seed::SEED_DOCTOR1_EMAIL).await;
    let code = requested["data"]["debug_otp_code"]
        .as_str()
        .unwrap()
        .to_string();

    sqlx::query(
        "UPDATE password_reset_requests SET otp_expires_at = NOW() - INTERVAL '1 minute' \
         WHERE user_id = $1",
    )
    .bind(uid(hellodoctor_backend::seed::DOCTOR1_USER_ID))
    .execute(&pool)
    .await
    .unwrap();

    let response = verify_otp(&app, hellodoctor_backend::seed::SEED_DOCTOR1_EMAIL, &code).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "./migrations")]
async fn a_new_request_invalidates_the_previous_one(pool: PgPool) {
    let (_, _pool, mut state) = test_app_and_state(pool).await;
    with_password_reset_enabled(&mut state);
    let redis = state.redis.clone();
    reset_owned_redis(&redis).await;
    let app = hellodoctor_backend::api::create_router(state);

    let first = request_reset(&app, hellodoctor_backend::seed::SEED_DOCTOR1_EMAIL).await;
    let first_code = first["data"]["debug_otp_code"]
        .as_str()
        .unwrap()
        .to_string();
    // The real per-recipient OTP-delivery cooldown (1 per 60s) would
    // otherwise reject this second request for the same account within the
    // same test — correct production behavior, not something to bypass via
    // a code change (see the identical rationale in
    // `transactional_email_triggers.rs::change_password_sends_email_and_revokes_other_sessions`).
    reset_owned_redis(&redis).await;
    let second = request_reset(&app, hellodoctor_backend::seed::SEED_DOCTOR1_EMAIL).await;
    let second_code = second["data"]["debug_otp_code"]
        .as_str()
        .unwrap()
        .to_string();
    assert_ne!(first_code, second_code);

    let stale_attempt = verify_otp(
        &app,
        hellodoctor_backend::seed::SEED_DOCTOR1_EMAIL,
        &first_code,
    )
    .await;
    assert_eq!(stale_attempt.status(), StatusCode::UNAUTHORIZED);

    let fresh_attempt = verify_otp(
        &app,
        hellodoctor_backend::seed::SEED_DOCTOR1_EMAIL,
        &second_code,
    )
    .await;
    assert_eq!(fresh_attempt.status(), StatusCode::OK);
}

#[sqlx::test(migrations = "./migrations")]
async fn weak_new_password_is_rejected_at_the_complete_step(pool: PgPool) {
    let (_, _pool, mut state) = test_app_and_state(pool).await;
    with_password_reset_enabled(&mut state);
    reset_owned_redis(&state.redis.clone()).await;
    let app = hellodoctor_backend::api::create_router(state);

    let requested = request_reset(&app, hellodoctor_backend::seed::SEED_DOCTOR1_EMAIL).await;
    let code = requested["data"]["debug_otp_code"]
        .as_str()
        .unwrap()
        .to_string();
    let verified = verify_otp(&app, hellodoctor_backend::seed::SEED_DOCTOR1_EMAIL, &code).await;
    let authorization = json_body(verified).await["data"]["reset_authorization"]
        .as_str()
        .unwrap()
        .to_string();

    let response = complete_reset(&app, &authorization, "weak").await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

/// Forces the real send path (no `DEV_AUTH_OTP_ENABLED` bypass), proving the
/// branded template actually reaches `email_outbox` as
/// `PASSWORD_RESET_OTP`/`SENT_TO_PROVIDER` — not just that the dev shortcut
/// makes the HTTP flow pass.
#[sqlx::test(migrations = "./migrations")]
async fn reset_otp_and_completion_emails_travel_through_the_real_outbox(pool: PgPool) {
    let (_, pool, mut state) = test_app_and_state(pool).await;
    with_password_reset_enabled(&mut state);
    reset_owned_redis(&state.redis.clone()).await;
    let mut cfg = (*state.config).clone();
    cfg.dev_auth_otp_enabled = false;
    state.config = std::sync::Arc::new(cfg);
    state.email_transport = always_send();
    let app = hellodoctor_backend::api::create_router(state);

    let requested = request_reset(&app, hellodoctor_backend::seed::SEED_DOCTOR1_EMAIL).await;
    assert!(
        requested["data"]["debug_otp_code"].is_null(),
        "with the dev bypass off, no debug code should be echoed back"
    );
    let row = outbox_row_for(
        &pool,
        "PASSWORD_RESET_OTP",
        hellodoctor_backend::seed::SEED_DOCTOR1_EMAIL,
    )
    .await;
    assert_eq!(row.as_deref(), Some("SENT_TO_PROVIDER"));

    // Pull the real code back out of the hashed challenge row is not
    // possible (by design); instead confirm the full completion path still
    // emits PASSWORD_CHANGED once we drive it via the known seed password
    // and a freshly reset one, using the service directly to obtain the
    // plaintext code the way a real recipient would read it from email.
    let code = hellodoctor_backend::crypto::generate_numeric_code(6);
    sqlx::query("UPDATE password_reset_requests SET code_hash = $2 WHERE user_id = $1")
        .bind(uid(hellodoctor_backend::seed::DOCTOR1_USER_ID))
        .bind(hellodoctor_backend::crypto::hash_code(&code))
        .execute(&pool)
        .await
        .unwrap();

    let verified = verify_otp(&app, hellodoctor_backend::seed::SEED_DOCTOR1_EMAIL, &code).await;
    assert_eq!(verified.status(), StatusCode::OK);
    let authorization = json_body(verified).await["data"]["reset_authorization"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        complete_reset(&app, &authorization, "Str0ng!NewPassw0rd")
            .await
            .status(),
        StatusCode::OK
    );

    let changed_row = outbox_row_for(
        &pool,
        "PASSWORD_CHANGED",
        hellodoctor_backend::seed::SEED_DOCTOR1_EMAIL,
    )
    .await;
    assert_eq!(changed_row.as_deref(), Some("SENT_TO_PROVIDER"));
}

/// Atomicity regression: `request_reset`'s own invalidate-then-insert
/// (two separate statements, not one transaction) is supposed to guarantee
/// at most one live `password_reset_requests` row per user, but a race
/// between two concurrent `request_reset` calls for the same account could
/// in principle leave a second, stray row behind. `complete_reset` must
/// invalidate every outstanding row for the account inside its own
/// transaction — not just the one its authorization matched — so a stray
/// row never survives a completed reset. This directly exercises the fix
/// in `PasswordResetService::complete_reset` (password update + reset
/// authorization consumption + session revocation + invalidation of other
/// outstanding recovery challenges, all inside one `tx`).
#[sqlx::test(migrations = "./migrations")]
async fn completing_a_reset_invalidates_every_outstanding_request_row_for_the_account(
    pool: PgPool,
) {
    let (_, pool, mut state) = test_app_and_state(pool).await;
    with_password_reset_enabled(&mut state);
    reset_owned_redis(&state.redis.clone()).await;
    let app = hellodoctor_backend::api::create_router(state);

    let requested = request_reset(&app, hellodoctor_backend::seed::SEED_DOCTOR1_EMAIL).await;
    let code = requested["data"]["debug_otp_code"]
        .as_str()
        .unwrap()
        .to_string();
    let verified = verify_otp(&app, hellodoctor_backend::seed::SEED_DOCTOR1_EMAIL, &code).await;
    let authorization = json_body(verified).await["data"]["reset_authorization"]
        .as_str()
        .unwrap()
        .to_string();

    // A second row for the same account can no longer even be created — the
    // `password_reset_requests_user_id_unique` constraint
    // (migrations/20261208000001) now enforces "at most one live reset flow
    // per user" at the database level, closing the issuance race this test
    // originally simulated by directly inserting a stray row here. Proving
    // the constraint itself fires is the stronger, still-relevant half of
    // this test's original intent.
    let direct_insert_result = sqlx::query(
        "INSERT INTO password_reset_requests (token, user_id, code_hash, otp_expires_at, created_at) \
         VALUES ($1, $2, $3, NOW() + INTERVAL '5 minutes', NOW())",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(uid(hellodoctor_backend::seed::DOCTOR1_USER_ID))
    .bind(hellodoctor_backend::crypto::hash_code("999999"))
    .execute(&pool)
    .await;
    assert!(
        direct_insert_result.is_err(),
        "a second row for the same user_id must be rejected by the unique constraint"
    );

    assert_eq!(
        complete_reset(&app, &authorization, "Str0ng!NewPassw0rd")
            .await
            .status(),
        StatusCode::OK
    );

    let remaining_after: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM password_reset_requests WHERE user_id = $1")
            .bind(uid(hellodoctor_backend::seed::DOCTOR1_USER_ID))
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        remaining_after, 0,
        "completion must invalidate every outstanding recovery challenge for the account"
    );
}

/// The issuance-race fix itself, isolated from the (separate, unrelated,
/// already-correct) per-recipient OTP-delivery rate limiter: two genuinely
/// concurrent calls to `password_reset_repo::upsert_for_user` for the same
/// user_id — the exact statement `request_reset` now uses instead of its
/// old separate invalidate-then-insert — must never leave two live rows
/// behind and must never surface the unique constraint as a raw error to
/// either caller. Calling the repo function directly (bypassing
/// `PasswordResetService::request_reset` and its HTTP handler) tests
/// precisely this fix rather than conflating it with rate-limiting, which
/// would otherwise make one of two truly concurrent HTTP calls for the same
/// recipient legitimately 429 regardless of this fix (see
/// `a_new_request_invalidates_the_previous_one`'s identical rationale for
/// why this codebase's tests already treat that cooldown as a separate
/// concern to reset around, not bypass).
#[sqlx::test(migrations = "./migrations")]
async fn concurrent_upserts_for_the_same_account_never_leave_two_live_rows(pool: PgPool) {
    let (_, pool, state) = test_app_and_state(pool).await;
    let user_id = hellodoctor_backend::domain::ids::UserId::from(uid(
        hellodoctor_backend::seed::DOCTOR1_USER_ID,
    ));

    let pool_a = state.auth_db.clone();
    let pool_b = state.auth_db.clone();
    let token_a = Uuid::new_v4().to_string();
    let token_b = Uuid::new_v4().to_string();
    let hash_a = hellodoctor_backend::crypto::hash_code("111111");
    let hash_b = hellodoctor_backend::crypto::hash_code("222222");
    let expires_at = Utc::now() + Duration::seconds(300);
    let (result_a, result_b) = tokio::join!(
        hellodoctor_backend::repository::password_reset_repo::upsert_for_user(
            &pool_a, &token_a, user_id, &hash_a, expires_at,
        ),
        hellodoctor_backend::repository::password_reset_repo::upsert_for_user(
            &pool_b, &token_b, user_id, &hash_b, expires_at,
        ),
    );
    assert!(result_a.is_ok(), "{result_a:?}");
    assert!(result_b.is_ok(), "{result_b:?}");

    let remaining: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM password_reset_requests WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        remaining, 1,
        "two concurrent upserts for the same account must leave exactly one live row, never two"
    );
}

/// Concurrency/rollback regression: two requests racing to complete the
/// *same* authorization must not both succeed and must not corrupt state —
/// `find_by_authorization_hash_for_update`'s `FOR UPDATE` row lock should
/// serialize them so exactly one wins (deletes the row, updates the
/// password, revokes sessions, all inside one transaction) and the other
/// sees the row already gone (same "invalid or expired" response as any
/// other replay), never a partial or double application of either change.
#[sqlx::test(migrations = "./migrations")]
async fn concurrent_completions_of_the_same_authorization_only_one_succeeds(pool: PgPool) {
    let (_, pool, mut state) = test_app_and_state(pool).await;
    with_password_reset_enabled(&mut state);
    let redis = state.redis.clone();
    reset_owned_redis(&redis).await;
    let app = hellodoctor_backend::api::create_router(state);

    let requested = request_reset(&app, hellodoctor_backend::seed::SEED_DOCTOR1_EMAIL).await;
    let code = requested["data"]["debug_otp_code"]
        .as_str()
        .unwrap()
        .to_string();
    let verified = verify_otp(&app, hellodoctor_backend::seed::SEED_DOCTOR1_EMAIL, &code).await;
    let authorization = json_body(verified).await["data"]["reset_authorization"]
        .as_str()
        .unwrap()
        .to_string();

    let auth_a = authorization.clone();
    let auth_b = authorization.clone();
    let (status_a, status_b) = tokio::join!(
        async {
            complete_reset(&app, &auth_a, "Str0ng!RaceWinnerPassw0rd")
                .await
                .status()
        },
        async {
            complete_reset(&app, &auth_b, "Str0ng!RaceLoserPassw0rd")
                .await
                .status()
        },
    );

    let statuses = [status_a, status_b];
    assert_eq!(
        statuses.iter().filter(|s| **s == StatusCode::OK).count(),
        1,
        "exactly one of two concurrent completions of the same authorization must succeed: {statuses:?}"
    );
    assert_eq!(
        statuses
            .iter()
            .filter(|s| **s == StatusCode::UNAUTHORIZED)
            .count(),
        1,
        "the loser must be cleanly rejected, never silently ignored or double-applied: {statuses:?}"
    );

    // No stray row survives either outcome.
    let remaining: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM password_reset_requests WHERE user_id = $1")
            .bind(uid(hellodoctor_backend::seed::DOCTOR1_USER_ID))
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(remaining, 0);

    // Exactly one of the two candidate passwords now works, proving the
    // winner's password update, not some mix of both, is what committed.
    // Each successful-password login attempt spends the same shared
    // per-recipient OTP-delivery cooldown the reset flow itself used (see
    // `a_new_request_invalidates_the_previous_one`'s identical rationale),
    // so Redis must be reset immediately before each one.
    async fn try_login(
        app: &axum::Router,
        redis: &deadpool_redis::Pool,
        password: &str,
    ) -> StatusCode {
        reset_owned_redis(redis).await;
        post(
            app,
            "/api/v1/auth/doctor/login",
            None,
            &serde_json::json!({
                "email": hellodoctor_backend::seed::SEED_DOCTOR1_EMAIL,
                "password": password,
            }),
        )
        .await
        .status()
    }
    let winner_login = try_login(&app, &redis, "Str0ng!RaceWinnerPassw0rd").await;
    let loser_login = try_login(&app, &redis, "Str0ng!RaceLoserPassw0rd").await;
    let successes = [winner_login, loser_login]
        .iter()
        .filter(|s| **s == StatusCode::OK)
        .count();
    assert_eq!(
        successes, 1,
        "exactly one of the two candidate passwords must now be the account's real password"
    );
}
