//! Stage D (FCM) regression coverage not already exercised by
//! `test_push_token_ownership_and_logout_disable` in `api_tests.rs`:
//! the `/notifications/devices/revoke` endpoint itself, and the
//! scheduler's claim/send/retry/permanent-failure state machine against a
//! scripted provider transport (never a real network call to FCM).
use super::*;
use async_trait::async_trait;
use hellodoctor_backend::repository::notification_repo;
use hellodoctor_backend::services::notification_scheduler;
use hellodoctor_backend::services::notification_transport::{
    NotificationTransport, PushOptions, PushOutcome,
};
use std::collections::HashMap;

/// Revoking by `push_token` must only ever disable rows owned by the
/// authenticated caller — the handler never accepts a client-supplied
/// recipient, and `device_tokens::disable_device` scopes its `UPDATE` by
/// `user_id` from `AuthContext`, not from the request body.
#[sqlx::test(migrations = "./migrations")]
async fn test_revoke_device_endpoint_is_owner_scoped(pool: PgPool) {
    let (app, pool, _state) = test_app_and_state(pool).await;
    let doctor1 = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let doctor2 = make_token(uid(DOCTOR2_USER_ID), "DOCTOR");

    let register = post(
        &app,
        "/api/v1/notifications/devices",
        Some(&doctor1),
        &serde_json::json!({"platform": "android", "push_token": "revoke-test-token-1"}),
    )
    .await;
    assert_eq!(register.status(), StatusCode::CREATED);

    // Doctor 2 tries to revoke doctor 1's token by push_token — must be a
    // no-op (0 rows disabled), never an error that would disclose the
    // token's existence or ownership.
    let cross_account_attempt = post(
        &app,
        "/api/v1/notifications/devices/revoke",
        Some(&doctor2),
        &serde_json::json!({"push_token": "revoke-test-token-1"}),
    )
    .await;
    assert_eq!(cross_account_attempt.status(), StatusCode::OK);
    let cross_body = json_body(cross_account_attempt).await;
    assert_eq!(cross_body["data"]["disabled_count"], 0);

    let (still_enabled,): (bool,) =
        sqlx::query_as("SELECT enabled FROM notification_device_tokens WHERE push_token = $1")
            .bind("revoke-test-token-1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        still_enabled,
        "a different account's revoke request must never disable this token"
    );

    // Doctor 1 revoking their own token must succeed and actually disable it.
    let own_revoke = post(
        &app,
        "/api/v1/notifications/devices/revoke",
        Some(&doctor1),
        &serde_json::json!({"push_token": "revoke-test-token-1"}),
    )
    .await;
    assert_eq!(own_revoke.status(), StatusCode::OK);
    let own_body = json_body(own_revoke).await;
    assert_eq!(own_body["data"]["disabled_count"], 1);

    let (enabled_after,): (bool,) =
        sqlx::query_as("SELECT enabled FROM notification_device_tokens WHERE push_token = $1")
            .bind("revoke-test-token-1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!enabled_after);
}

/// Deterministic, non-network stand-in for the real FCM transport. Each
/// push_token is scripted to a fixed outcome so the scheduler's three state
/// transitions (sent / permanently rejected / transiently retried) can be
/// exercised without ever calling the real `fcm.googleapis.com` endpoint.
struct ScriptedTransport {
    outcomes: HashMap<String, PushOutcome>,
}

#[async_trait]
impl NotificationTransport for ScriptedTransport {
    fn is_configured(&self) -> bool {
        true
    }

    async fn send_push(
        &self,
        push_token: &str,
        _title: &str,
        _body: &str,
        _options: &PushOptions,
    ) -> PushOutcome {
        self.outcomes
            .get(push_token)
            .cloned()
            .unwrap_or(PushOutcome::Retryable {
                reason: "unscripted token in test".into(),
            })
    }
}

/// Exercises `notification_scheduler::run_push_delivery_pass` directly
/// (not through the 60-second background loop) against a scripted
/// transport covering all three provider outcomes in one pass: a
/// provider-accepted send marks the row `SENT_TO_PROVIDER`; an
/// `UNREGISTERED`-style permanent rejection disables that device token but
/// leaves the row retryable-exhausted as `FAILED`; a transient failure
/// leaves the row `FAILED` with a future `next_delivery_at` rather than
/// disabling anything.
#[sqlx::test(migrations = "./migrations")]
async fn test_push_delivery_pass_sent_permanent_and_retry_paths(pool: PgPool) {
    let (app, pool, mut state) = test_app_and_state(pool).await;

    let sent_user = UserId::from(uid(DOCTOR1_USER_ID));
    let permanent_user = UserId::from(uid(DOCTOR2_USER_ID));
    let retry_user = UserId::from(uid(PATIENT_USER_ID));

    // Registered through the real HTTP endpoint (as the registration test
    // suite already does) rather than by calling the repo layer directly
    // against `system_db` — that pool authenticates as
    // `hellodoctor_system_worker`, which only has UPDATE/SELECT on
    // `notification_device_tokens` (the delivery pass's own job), never
    // INSERT; only `hellodoctor_api`, via a user's own authenticated
    // transaction, may create a new device-token row.
    for (token_owner, push_token) in [
        (make_token(uid(DOCTOR1_USER_ID), "DOCTOR"), "token-sent"),
        (
            make_token(uid(DOCTOR2_USER_ID), "DOCTOR"),
            "token-permanent",
        ),
        (make_token(uid(PATIENT_USER_ID), "PATIENT"), "token-retry"),
    ] {
        let register = post(
            &app,
            "/api/v1/notifications/devices",
            Some(&token_owner),
            &serde_json::json!({"platform": "android", "push_token": push_token}),
        )
        .await;
        assert_eq!(register.status(), StatusCode::CREATED);
    }

    let sent_id = Uuid::new_v4();
    let permanent_id = Uuid::new_v4();
    let retry_id = Uuid::new_v4();
    for (id, user, key) in [
        (sent_id, sent_user, "scripted-sent"),
        (permanent_id, permanent_user, "scripted-permanent"),
        (retry_id, retry_user, "scripted-retry"),
    ] {
        assert!(notification_repo::insert_if_new(
            &pool,
            id,
            user,
            "SERVICE_ANNOUNCEMENT",
            "Test",
            "Test body",
            None,
            None,
            None,
            key,
            None,
        )
        .await
        .unwrap());
    }

    let mut outcomes = HashMap::new();
    outcomes.insert(
        "token-sent".to_string(),
        PushOutcome::SentToProvider {
            provider_message_id: Some("projects/x/messages/1".into()),
        },
    );
    outcomes.insert(
        "token-permanent".to_string(),
        PushOutcome::Permanent {
            reason: "FCM rejected the registration token (HTTP 404)".into(),
        },
    );
    outcomes.insert(
        "token-retry".to_string(),
        PushOutcome::Retryable {
            reason: "FCM transport is unavailable".into(),
        },
    );
    state.notification_transport = std::sync::Arc::new(ScriptedTransport { outcomes });

    notification_scheduler::run_push_delivery_pass(&state)
        .await
        .unwrap();

    let (sent_status,): (String,) =
        sqlx::query_as("SELECT delivery_status FROM notifications WHERE id = $1")
            .bind(sent_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(sent_status, "SENT_TO_PROVIDER");

    let (permanent_status, permanent_token_enabled): (String, bool) = sqlx::query_as(
        "SELECT n.delivery_status, t.enabled FROM notifications n \
         JOIN notification_device_tokens t ON t.push_token = 'token-permanent' \
         WHERE n.id = $1",
    )
    .bind(permanent_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(permanent_status, "FAILED");
    assert!(
        !permanent_token_enabled,
        "a provider-confirmed UNREGISTERED token must be disabled"
    );

    let (retry_status, retry_next_delivery): (String, Option<DateTime<Utc>>) =
        sqlx::query_as("SELECT delivery_status, next_delivery_at FROM notifications WHERE id = $1")
            .bind(retry_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(retry_status, "FAILED");
    assert!(
        retry_next_delivery.is_some() && retry_next_delivery.unwrap() > Utc::now(),
        "a transient failure must be rescheduled, not treated as permanent"
    );
    let (retry_token_enabled,): (bool,) =
        sqlx::query_as("SELECT enabled FROM notification_device_tokens WHERE push_token = $1")
            .bind("token-retry")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        retry_token_enabled,
        "a transient provider failure must never disable the device token"
    );
}

/// A short TTL alone would not have caught this: a `CONSULTATION_READY` row
/// that was fine when queued but whose appointment has since actually moved
/// to `IN_CONSULTATION` (someone already joined) must be suppressed at
/// dispatch time, never sent, and never retried — while the in-app inbox
/// row itself must still exist (history is retained).
#[sqlx::test(migrations = "./migrations")]
async fn test_push_delivery_pass_suppresses_stale_consultation_ready(pool: PgPool) {
    let (app, pool, mut state) = test_app_and_state(pool).await;
    let doctor_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let register = post(
        &app,
        "/api/v1/notifications/devices",
        Some(&doctor_token),
        &serde_json::json!({"platform": "android", "push_token": "ready-stale-token"}),
    )
    .await;
    assert_eq!(register.status(), StatusCode::CREATED);

    let (_slot, appointment_id) =
        insert_confirmed_appointment(&pool, Utc::now() + chrono::Duration::minutes(5)).await;
    let notification_id = Uuid::new_v4();
    assert!(notification_repo::insert_if_new(
        &pool,
        notification_id,
        UserId::from(uid(DOCTOR1_USER_ID)),
        "CONSULTATION_READY",
        "Consultation ready",
        "Your consultation is ready to join.",
        Some("APPOINTMENT"),
        Some(appointment_id),
        None,
        "stale-ready-test",
        None,
    )
    .await
    .unwrap());

    // The authoritative state changed after this was queued: someone already
    // joined, exactly the race the dispatch-time recheck exists to catch.
    sqlx::query("UPDATE appointments SET status='IN_CONSULTATION' WHERE id=$1")
        .bind(appointment_id)
        .execute(&pool)
        .await
        .unwrap();

    let mut outcomes = HashMap::new();
    outcomes.insert(
        "ready-stale-token".to_string(),
        PushOutcome::SentToProvider {
            provider_message_id: Some("should-never-be-used".into()),
        },
    );
    state.notification_transport = std::sync::Arc::new(ScriptedTransport { outcomes });
    notification_scheduler::run_push_delivery_pass(&state)
        .await
        .unwrap();

    let (status, error): (String, Option<String>) = sqlx::query_as(
        "SELECT delivery_status, last_delivery_error FROM notifications WHERE id = $1",
    )
    .bind(notification_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "SUPPRESSED");
    assert!(error.unwrap().contains("Suppressed"));
    // The row must still exist for in-app history — it was never deleted.
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM notifications WHERE id=$1)")
        .bind(notification_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(exists, "a suppressed dispatch must retain the inbox row");
}

/// Same concern for `CONSULTATION_REJOIN`: if the session has already
/// recovered (the participant rejoined) by the time a queued push is about
/// to go out, it must be suppressed rather than misleadingly inviting a
/// rejoin that already happened.
#[sqlx::test(migrations = "./migrations")]
async fn test_push_delivery_pass_suppresses_stale_consultation_rejoin(pool: PgPool) {
    let (app, pool, mut state) = test_app_and_state(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let register = post(
        &app,
        "/api/v1/notifications/devices",
        Some(&patient_token),
        &serde_json::json!({"platform": "android", "push_token": "rejoin-stale-token"}),
    )
    .await;
    assert_eq!(register.status(), StatusCode::CREATED);

    let (appointment_id, _) = book_and_confirm(&app, &patient_token).await;
    let notification_id = Uuid::new_v4();
    assert!(notification_repo::insert_if_new(
        &pool,
        notification_id,
        UserId::from(uid(PATIENT_USER_ID)),
        "CONSULTATION_REJOIN",
        "Rejoin available",
        "Both participants have left the consultation room. Rejoin before the grace period ends to continue, or it will be marked interrupted.",
        Some("APPOINTMENT"),
        Some(appointment_id),
        None,
        "stale-rejoin-test",
        None,
    )
    .await
    .unwrap());

    // The session was never actually in GRACE_PERIOD here (recovery_state
    // defaults to 'NONE') — simulating "already recovered by dispatch time."

    let mut outcomes = HashMap::new();
    outcomes.insert(
        "rejoin-stale-token".to_string(),
        PushOutcome::SentToProvider {
            provider_message_id: Some("should-never-be-used".into()),
        },
    );
    state.notification_transport = std::sync::Arc::new(ScriptedTransport { outcomes });
    notification_scheduler::run_push_delivery_pass(&state)
        .await
        .unwrap();

    let (status, error): (String, Option<String>) = sqlx::query_as(
        "SELECT delivery_status, last_delivery_error FROM notifications WHERE id = $1",
    )
    .bind(notification_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "SUPPRESSED");
    assert!(error.unwrap().contains("Suppressed"));
}

/// Positive control: a `CONSULTATION_READY` notification whose appointment
/// is still genuinely eligible at dispatch time must still be sent normally
/// — the recheck must not false-positive-block the ordinary case.
#[sqlx::test(migrations = "./migrations")]
async fn test_push_delivery_pass_still_sends_genuinely_eligible_ready(pool: PgPool) {
    let (app, pool, mut state) = test_app_and_state(pool).await;
    let doctor_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let register = post(
        &app,
        "/api/v1/notifications/devices",
        Some(&doctor_token),
        &serde_json::json!({"platform": "android", "push_token": "ready-live-token"}),
    )
    .await;
    assert_eq!(register.status(), StatusCode::CREATED);

    let (_slot, appointment_id) =
        insert_confirmed_appointment(&pool, Utc::now() + chrono::Duration::minutes(5)).await;
    let notification_id = Uuid::new_v4();
    assert!(notification_repo::insert_if_new(
        &pool,
        notification_id,
        UserId::from(uid(DOCTOR1_USER_ID)),
        "CONSULTATION_READY",
        "Consultation ready",
        "Your consultation is ready to join.",
        Some("APPOINTMENT"),
        Some(appointment_id),
        None,
        "live-ready-test",
        None,
    )
    .await
    .unwrap());

    let mut outcomes = HashMap::new();
    outcomes.insert(
        "ready-live-token".to_string(),
        PushOutcome::SentToProvider {
            provider_message_id: Some("projects/x/messages/live".into()),
        },
    );
    state.notification_transport = std::sync::Arc::new(ScriptedTransport { outcomes });
    notification_scheduler::run_push_delivery_pass(&state)
        .await
        .unwrap();

    let (status,): (String,) =
        sqlx::query_as("SELECT delivery_status FROM notifications WHERE id = $1")
            .bind(notification_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "SENT_TO_PROVIDER");
}
