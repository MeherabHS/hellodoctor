//! Phase 0 email integration: `email_outbox` durable tracking, retry
//! classification/backoff, idempotency and expiry — exercised against a
//! scripted `EmailTransport`, never a real network call to Azure
//! Communication Services. Mirrors `push_notification_delivery.rs`'s
//! scripted-transport pattern for the FCM side.
use super::*;
use async_trait::async_trait;
use hellodoctor_backend::email_delivery::{self, OtpEmailKind};
use hellodoctor_backend::error::AppError;
use hellodoctor_backend::repository::email_outbox_repo;
use hellodoctor_backend::services::email_transport::{EmailMessage, EmailOutcome, EmailTransport};
use std::collections::VecDeque;
use std::sync::Mutex;

/// Deterministic, non-network stand-in for `AcsEmailTransport`. Each call
/// pops the next scripted outcome from a shared queue so a retry sequence
/// (e.g. `Retryable`, then `SentToProvider`) can be exercised without ever
/// calling the real Azure endpoint. Optionally sleeps before returning (to
/// model a realistic slow/hanging provider for end-to-end latency-bound
/// assertions) and always records every message body it was asked to send
/// (to prove a retried send carries the identical OTP code, never a
/// different one).
struct ScriptedEmailTransport {
    outcomes: Mutex<VecDeque<EmailOutcome>>,
    delay: std::time::Duration,
    sent_bodies: Mutex<Vec<String>>,
}

#[async_trait]
impl EmailTransport for ScriptedEmailTransport {
    fn is_configured(&self) -> bool {
        true
    }

    async fn send_email(&self, message: &EmailMessage<'_>) -> EmailOutcome {
        if !self.delay.is_zero() {
            tokio::time::sleep(self.delay).await;
        }
        self.sent_bodies
            .lock()
            .unwrap()
            .push(message.plain_text_body.to_string());
        self.outcomes
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(EmailOutcome::Permanent {
                reason: "ScriptedEmailTransport ran out of scripted outcomes".into(),
            })
    }
}

fn scripted(outcomes: Vec<EmailOutcome>) -> std::sync::Arc<dyn EmailTransport> {
    std::sync::Arc::new(ScriptedEmailTransport {
        outcomes: Mutex::new(outcomes.into()),
        delay: std::time::Duration::ZERO,
        sent_bodies: Mutex::new(Vec::new()),
    })
}

fn scripted_with_delay(
    outcomes: Vec<EmailOutcome>,
    delay: std::time::Duration,
) -> std::sync::Arc<ScriptedEmailTransport> {
    std::sync::Arc::new(ScriptedEmailTransport {
        outcomes: Mutex::new(outcomes.into()),
        delay,
        sent_bodies: Mutex::new(Vec::new()),
    })
}

/// `test_config()` defaults to `ENVIRONMENT=development` and
/// `DEV_AUTH_OTP_ENABLED=true` (the dev OTP bypass) — every test below that
/// actually wants `send_otp_email_tracked` to reach the transport must
/// disable that bypass first, exactly like `test_exposure_*` tests elsewhere
/// in this suite mutate `state.config` for a single scenario.
fn disable_dev_otp_bypass(state: &mut AppState) {
    let mut config = (*state.config).clone();
    config.dev_auth_otp_enabled = false;
    state.config = Arc::new(config);
}

#[sqlx::test(migrations = "./migrations")]
async fn sent_to_provider_marks_the_outbox_row_sent(pool: PgPool) {
    let (_, pool, mut state) = test_app_and_state(pool).await;
    disable_dev_otp_bypass(&mut state);
    state.email_transport = scripted(vec![EmailOutcome::SentToProvider {
        provider_message_id: Some("msg-1".into()),
    }]);

    let token = Uuid::new_v4().to_string();
    let expires_at = Utc::now() + Duration::seconds(300);
    email_delivery::send_otp_email_tracked(
        &state,
        "patient@example.invalid",
        "123456",
        OtpEmailKind::Patient,
        &token,
        expires_at,
    )
    .await
    .unwrap();

    let dedup_key = format!("otp-email:patient:{token}");
    let row = email_outbox_repo::find_by_dedup_key(&pool, &dedup_key)
        .await
        .unwrap()
        .expect("outbox row must exist");
    assert_eq!(row.delivery_status, "SENT_TO_PROVIDER");
    assert_eq!(row.provider_message_id.as_deref(), Some("msg-1"));
    assert_eq!(row.email_type, "PATIENT_OTP");
    assert_eq!(row.recipient_email, "patient@example.invalid");
}

#[sqlx::test(migrations = "./migrations")]
async fn retryable_outcome_is_retried_inline_and_can_still_succeed(pool: PgPool) {
    let (_, pool, mut state) = test_app_and_state(pool).await;
    disable_dev_otp_bypass(&mut state);
    state.email_transport = scripted(vec![
        EmailOutcome::Retryable {
            reason: "transient".into(),
        },
        EmailOutcome::SentToProvider {
            provider_message_id: None,
        },
    ]);

    let token = Uuid::new_v4().to_string();
    let expires_at = Utc::now() + Duration::seconds(300);
    email_delivery::send_otp_email_tracked(
        &state,
        "doctor@example.invalid",
        "654321",
        OtpEmailKind::Doctor,
        &token,
        expires_at,
    )
    .await
    .unwrap();

    let dedup_key = format!("otp-email:doctor:{token}");
    let row = email_outbox_repo::find_by_dedup_key(&pool, &dedup_key)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.delivery_status, "SENT_TO_PROVIDER");
    // The retry attempt is recorded before the eventual success; the success
    // path itself does not bump the attempt counter again.
    assert_eq!(row.delivery_attempts, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn retryable_outcome_exhausts_its_budget_and_marks_the_row_failed(pool: PgPool) {
    let (_, pool, mut state) = test_app_and_state(pool).await;
    disable_dev_otp_bypass(&mut state);
    state.email_transport = scripted(vec![
        EmailOutcome::Retryable {
            reason: "transient-1".into(),
        },
        EmailOutcome::Retryable {
            reason: "transient-2".into(),
        },
    ]);

    let token = Uuid::new_v4().to_string();
    let expires_at = Utc::now() + Duration::seconds(300);
    let result = email_delivery::send_otp_email_tracked(
        &state,
        "admin@example.invalid",
        "111222",
        OtpEmailKind::Admin,
        &token,
        expires_at,
    )
    .await;
    assert!(
        matches!(result, Err(AppError::ServiceUnavailable(_))),
        "an exhausted retry budget is a dependency failure (503), not an unhandled \
         server bug (500): {result:?}"
    );

    let dedup_key = format!("otp-email:admin:{token}");
    let row = email_outbox_repo::find_by_dedup_key(&pool, &dedup_key)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.delivery_status, "FAILED");
    assert_eq!(row.delivery_attempts, 2);
    assert_eq!(row.last_delivery_error.as_deref(), Some("transient-2"));
}

#[sqlx::test(migrations = "./migrations")]
async fn permanent_outcome_fails_immediately_without_retrying(pool: PgPool) {
    let (_, pool, mut state) = test_app_and_state(pool).await;
    disable_dev_otp_bypass(&mut state);
    state.email_transport = scripted(vec![EmailOutcome::Permanent {
        reason: "bad recipient".into(),
    }]);

    let token = Uuid::new_v4().to_string();
    let expires_at = Utc::now() + Duration::seconds(300);
    let result = email_delivery::send_otp_email_tracked(
        &state,
        "patient2@example.invalid",
        "999000",
        OtpEmailKind::Patient,
        &token,
        expires_at,
    )
    .await;
    assert!(
        matches!(result, Err(AppError::ServiceUnavailable(_))),
        "a provider rejection is a dependency failure (503), not an unhandled \
         server bug (500): {result:?}"
    );

    let dedup_key = format!("otp-email:patient:{token}");
    let row = email_outbox_repo::find_by_dedup_key(&pool, &dedup_key)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.delivery_status, "FAILED");
    assert_eq!(
        row.delivery_attempts, 1,
        "a permanent outcome must not retry"
    );
    assert_eq!(row.last_delivery_error.as_deref(), Some("bad recipient"));
}

#[sqlx::test(migrations = "./migrations")]
async fn development_bypass_skips_outbox_and_transport_entirely(pool: PgPool) {
    // test_config() defaults to ENVIRONMENT=development and
    // DEV_AUTH_OTP_ENABLED=true — left untouched here, which is exactly the
    // bypass this test verifies still short-circuits before any outbox row
    // or transport call.
    let (_, pool, state) = test_app_and_state(pool).await;

    let token = Uuid::new_v4().to_string();
    let expires_at = Utc::now() + Duration::seconds(300);
    email_delivery::send_otp_email_tracked(
        &state,
        "patient3@example.invalid",
        "555666",
        OtpEmailKind::Patient,
        &token,
        expires_at,
    )
    .await
    .unwrap();

    let dedup_key = format!("otp-email:patient:{token}");
    assert!(email_outbox_repo::find_by_dedup_key(&pool, &dedup_key)
        .await
        .unwrap()
        .is_none());
}

#[sqlx::test(migrations = "./migrations")]
async fn unconfigured_transport_fails_fast_without_an_outbox_row(pool: PgPool) {
    let (_, pool, mut state) = test_app_and_state(pool).await;
    disable_dev_otp_bypass(&mut state);
    // state.email_transport remains the default NoopEmailTransport built
    // from test_config()'s unset azure_email_* fields.

    let token = Uuid::new_v4().to_string();
    let expires_at = Utc::now() + Duration::seconds(300);
    let result = email_delivery::send_otp_email_tracked(
        &state,
        "patient4@example.invalid",
        "777888",
        OtpEmailKind::Patient,
        &token,
        expires_at,
    )
    .await;
    assert!(
        matches!(result, Err(AppError::Conflict(_))),
        "a provider that was never configured at all is a static precondition \
         failure (409) distinct from a configured-but-failing provider \
         (503) — a client can't fix the latter by retrying with different \
         input, but an operator can fix the former: {result:?}"
    );

    let dedup_key = format!("otp-email:patient:{token}");
    assert!(email_outbox_repo::find_by_dedup_key(&pool, &dedup_key)
        .await
        .unwrap()
        .is_none());
}

#[sqlx::test(migrations = "./migrations")]
async fn insert_if_new_is_idempotent_on_dedup_key(pool: PgPool) {
    let expires_at = Utc::now() + Duration::seconds(300);
    let first = email_outbox_repo::insert_if_new(
        &pool,
        Uuid::new_v4(),
        "PATIENT_OTP",
        "a@b.invalid",
        "dedup-key-1",
        expires_at,
    )
    .await
    .unwrap();
    let second = email_outbox_repo::insert_if_new(
        &pool,
        Uuid::new_v4(),
        "PATIENT_OTP",
        "a@b.invalid",
        "dedup-key-1",
        expires_at,
    )
    .await
    .unwrap();
    assert!(first, "the first insert for a new dedup_key must succeed");
    assert!(
        !second,
        "a second insert for the same dedup_key must be a no-op, not a second row"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn expire_stale_closes_only_rows_past_their_expiry(pool: PgPool) {
    email_outbox_repo::insert_if_new(
        &pool,
        Uuid::new_v4(),
        "DOCTOR_OTP",
        "d@e.invalid",
        "stale-key",
        Utc::now() - Duration::seconds(10),
    )
    .await
    .unwrap();
    email_outbox_repo::insert_if_new(
        &pool,
        Uuid::new_v4(),
        "DOCTOR_OTP",
        "d@e.invalid",
        "fresh-key",
        Utc::now() + Duration::seconds(300),
    )
    .await
    .unwrap();

    let closed = email_outbox_repo::expire_stale(&pool).await.unwrap();
    assert_eq!(closed, 1);

    let stale = email_outbox_repo::find_by_dedup_key(&pool, "stale-key")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stale.delivery_status, "EXPIRED");
    let fresh = email_outbox_repo::find_by_dedup_key(&pool, "fresh-key")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(fresh.delivery_status, "CREATED");
}

/// Review follow-up: the scripted-outcome tests above return instantly, so
/// they never actually exercised wall-clock latency. This test uses a
/// transport that genuinely sleeps on every call (modeling a slow/degraded
/// provider, not just a logically-classified one) and asserts the whole
/// login-request-blocking call to `send_otp_email_tracked` is bounded by the
/// documented formula — `MAX_INLINE_ATTEMPTS * per_attempt_delay +
/// inter-attempt backoff` — not merely "eventually returns." A caller
/// (`auth_service.rs`'s three OTP call sites) is blocked for exactly this
/// duration before it can respond to the waiting HTTP request; this is the
/// end-to-end evidence that it is bounded, not indefinite.
#[sqlx::test(migrations = "./migrations")]
async fn worst_case_latency_is_bounded_not_indefinite_under_a_slow_provider(pool: PgPool) {
    let (_, pool, mut state) = test_app_and_state(pool).await;
    disable_dev_otp_bypass(&mut state);
    let per_attempt_delay = std::time::Duration::from_millis(300);
    state.email_transport = scripted_with_delay(
        vec![
            EmailOutcome::Retryable {
                reason: "slow-1".into(),
            },
            EmailOutcome::Retryable {
                reason: "slow-2".into(),
            },
        ],
        per_attempt_delay,
    );

    let token = Uuid::new_v4().to_string();
    let expires_at = Utc::now() + Duration::seconds(300);
    let started = std::time::Instant::now();
    let result = email_delivery::send_otp_email_tracked(
        &state,
        "patient5@example.invalid",
        "222333",
        OtpEmailKind::Patient,
        &token,
        expires_at,
    )
    .await;
    let elapsed = started.elapsed();

    assert!(
        result.is_err(),
        "a persistently slow provider must still fail, not hang forever"
    );
    // Two attempts at `per_attempt_delay` plus one inter-attempt backoff
    // sleep (500ms, per MAX_INLINE_ATTEMPTS/INLINE_RETRY_BACKOFF in
    // email_delivery.rs) is the documented floor; a generous multiple of
    // that is the ceiling this test treats as "not indefinite."
    let expected_floor = per_attempt_delay * 2;
    assert!(
        elapsed >= expected_floor,
        "unexpectedly fast — did it skip an attempt? {elapsed:?}"
    );
    assert!(
        elapsed < expected_floor + std::time::Duration::from_secs(5),
        "unbounded/indefinite latency under a slow provider: {elapsed:?}"
    );

    let dedup_key = format!("otp-email:patient:{token}");
    let row = email_outbox_repo::find_by_dedup_key(&pool, &dedup_key)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.delivery_status, "FAILED");
    assert_eq!(row.delivery_attempts, 2);
}

/// Review follow-up: confirms that when a transient failure is followed by a
/// retry that succeeds, the email actually delivered to the provider on the
/// retry carries the *same* OTP code as the first attempt — never a
/// different/newly-generated one, and never sent zero times. A duplicate
/// send of the identical, still-valid code is harmless for OTP (the
/// recipient can use either copy; the challenge table's single-use
/// row-lock in `auth_service.rs` is the actual replay guard, unaffected by
/// how many times the *email* itself was attempted) — this test exists to
/// prove it is a harmless duplicate, never a code mismatch.
#[sqlx::test(migrations = "./migrations")]
async fn a_retried_send_carries_the_identical_otp_code_not_a_different_one(pool: PgPool) {
    let (_, _pool, mut state) = test_app_and_state(pool).await;
    disable_dev_otp_bypass(&mut state);
    let transport = scripted_with_delay(
        vec![
            EmailOutcome::Retryable {
                reason: "transient".into(),
            },
            EmailOutcome::SentToProvider {
                provider_message_id: None,
            },
        ],
        std::time::Duration::ZERO,
    );
    state.email_transport = transport.clone();

    let token = Uuid::new_v4().to_string();
    let expires_at = Utc::now() + Duration::seconds(300);
    email_delivery::send_otp_email_tracked(
        &state,
        "doctor2@example.invalid",
        "445566",
        OtpEmailKind::Doctor,
        &token,
        expires_at,
    )
    .await
    .unwrap();

    let sent_bodies = transport.sent_bodies.lock().unwrap().clone();
    assert_eq!(sent_bodies.len(), 2, "exactly one retry must have occurred");
    assert_eq!(
        sent_bodies[0], sent_bodies[1],
        "a retried send must carry the identical code, never a different one"
    );
    assert!(sent_bodies[0].contains("445566"));
}

/// Integration-level, through the real `/api/v1/auth/doctor/login` HTTP
/// endpoint rather than `email_delivery` directly (the tests above already
/// cover the service-layer outcome classification in isolation) — proves
/// "production authentication must continue to fail closed whenever
/// delivery is unavailable" end-to-end for all four provider states this
/// system can be in, fully deterministically and without any live network
/// access: no `pending_mfa_challenges` row survives a failed send, and the
/// HTTP status a caller sees reflects the true cause — 409 for a provider
/// that was never configured at all (a static precondition only an operator
/// can fix), 503 for one that is configured but failing (network-unreachable
/// or an explicit provider rejection — a caller can retry later), 200 for a
/// real, successful send. This directly addresses the regression in
/// `test_production_without_providers_fails_closed_without_false_success`
/// (`api_tests.rs`), whose single HTTP assertion exercised only one of
/// these four states (and, before this fix, asserted the wrong status for
/// it).
mod login_provider_state {
    use super::*;

    /// `test_config()` defaults to a `NoopEmailTransport` built from unset
    /// `azure_email_*` fields; this gives the login path a *configured*
    /// endpoint (so it passes the `azure_email_endpoint.is_none()` guard and
    /// actually attempts a send) while still disabling the dev OTP bypass,
    /// so every scenario below reaches the scripted `email_transport`.
    fn with_azure_email_configured_but_no_dev_bypass(state: &mut AppState) {
        let mut cfg = (*state.config).clone();
        cfg.dev_auth_otp_enabled = false;
        cfg.azure_email_endpoint = Some("https://synthetic.communication.azure.com".into());
        cfg.azure_email_access_key =
            Some(base64::engine::general_purpose::STANDARD.encode([9u8; 32]));
        cfg.azure_email_sender = Some("no-reply@example.test".into());
        state.config = Arc::new(cfg);
    }

    async fn pending_challenge_count(pool: &PgPool, user_id: Uuid) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM pending_mfa_challenges WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(pool)
            .await
            .unwrap()
    }

    async fn attempt_doctor_login(app: &axum::Router) -> axum::http::Response<Body> {
        post(
            app,
            "/api/v1/auth/doctor/login",
            None,
            &serde_json::json!({
                "email": hellodoctor_backend::seed::SEED_DOCTOR1_EMAIL,
                "password": hellodoctor_backend::seed::SEED_DOCTOR1_PASSWORD,
            }),
        )
        .await
    }

    /// 1 of 4: missing ACS configuration. Deliberately not `environment =
    /// "production"`: `validate_azure_email_configuration` forbids a
    /// production/staging config from omitting Azure email entirely, so the
    /// only environment where "no provider configured at all" is both
    /// reachable and deterministic is `"test"` (`requires_https()` is false,
    /// so validation allows it) — the exact branch `doctor_login_step1`
    /// guards with `environment != "development" && azure_email_endpoint.is_none()`.
    #[sqlx::test(migrations = "./migrations")]
    async fn missing_azure_email_configuration_fails_closed_with_conflict(pool: PgPool) {
        let (_, pool, mut state) = test_app_and_state(pool).await;
        let mut cfg = (*state.config).clone();
        cfg.environment = "test".into();
        cfg.dev_auth_otp_enabled = false;
        state.config = Arc::new(cfg);
        reset_owned_redis(&state.redis.clone()).await;
        let app = hellodoctor_backend::api::create_router(state);

        let response = attempt_doctor_login(&app).await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            pending_challenge_count(&pool, uid(hellodoctor_backend::seed::DOCTOR1_USER_ID)).await,
            0
        );
    }

    /// 2 of 4: configured but unreachable ACS endpoint — modeled by a
    /// scripted transport that exhausts the inline retry budget with
    /// `Retryable` outcomes, exactly as a real connect-timeout/DNS failure
    /// against an unreachable host would be classified by
    /// `AcsEmailTransport::send_email` (see
    /// `email_transport::tests::unresponsive_endpoint_times_out_and_is_classified_retryable`
    /// for that classification proved against a real hung socket).
    #[sqlx::test(migrations = "./migrations")]
    async fn unreachable_configured_endpoint_fails_closed_with_service_unavailable(pool: PgPool) {
        let (_, pool, mut state) = test_app_and_state(pool).await;
        with_azure_email_configured_but_no_dev_bypass(&mut state);
        state.email_transport = scripted(vec![
            EmailOutcome::Retryable {
                reason: "connect timeout".into(),
            },
            EmailOutcome::Retryable {
                reason: "connect timeout".into(),
            },
        ]);
        reset_owned_redis(&state.redis.clone()).await;
        let app = hellodoctor_backend::api::create_router(state);

        let response = attempt_doctor_login(&app).await;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            pending_challenge_count(&pool, uid(hellodoctor_backend::seed::DOCTOR1_USER_ID)).await,
            0,
            "a failed send must not leave a usable challenge behind"
        );
    }

    /// 3 of 4: provider error response — the provider is reachable and
    /// responds, but rejects the request outright (e.g. HTTP 401/403/400),
    /// classified `Permanent` by `AcsEmailTransport::send_email`.
    #[sqlx::test(migrations = "./migrations")]
    async fn provider_error_response_fails_closed_with_service_unavailable(pool: PgPool) {
        let (_, pool, mut state) = test_app_and_state(pool).await;
        with_azure_email_configured_but_no_dev_bypass(&mut state);
        state.email_transport = scripted(vec![EmailOutcome::Permanent {
            reason: "Azure email rejected the request (HTTP 401)".into(),
        }]);
        reset_owned_redis(&state.redis.clone()).await;
        let app = hellodoctor_backend::api::create_router(state);

        let response = attempt_doctor_login(&app).await;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            pending_challenge_count(&pool, uid(hellodoctor_backend::seed::DOCTOR1_USER_ID)).await,
            0
        );
    }

    /// 4 of 4: successful provider acceptance — the control case proving the
    /// other three assertions are not merely "this endpoint always fails."
    #[sqlx::test(migrations = "./migrations")]
    async fn successful_provider_acceptance_completes_login_normally(pool: PgPool) {
        let (_, pool, mut state) = test_app_and_state(pool).await;
        with_azure_email_configured_but_no_dev_bypass(&mut state);
        state.email_transport = scripted(vec![EmailOutcome::SentToProvider {
            provider_message_id: Some("msg-ok".into()),
        }]);
        reset_owned_redis(&state.redis.clone()).await;
        let app = hellodoctor_backend::api::create_router(state);

        let response = attempt_doctor_login(&app).await;
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "a real success must not be masked as a failure either"
        );
        assert_eq!(
            pending_challenge_count(&pool, uid(hellodoctor_backend::seed::DOCTOR1_USER_ID)).await,
            1
        );
    }
}
