//! Milestone 3 coverage: English/Swahili template rendering (including the
//! English fallback when a locale's template row is missing), the
//! `/notifications/preferences` endpoint (category toggles, quiet hours,
//! `preferred_language`), and `run_push_delivery_pass`'s delivery-time
//! gating (category opt-out, quiet-hours deferral exempting time-sensitive
//! types, and the per-type daily frequency cap) — all while confirming the
//! in-app inbox row is never deleted by any of this.
use super::*;
use async_trait::async_trait;
use hellodoctor_backend::repository::notification_repo;
use hellodoctor_backend::services::notification_scheduler;
use hellodoctor_backend::services::notification_transport::{
    NotificationTransport, PushOptions, PushOutcome,
};
use std::collections::HashMap;

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

/// `GET /notifications/preferences` must return the documented safe
/// defaults for a user who has never touched settings, and `PUT` must both
/// persist category/quiet-hours changes and drive `users.preferred_language`
/// — the real write path this milestone had to add (previously nothing in
/// the backend ever updated that column).
#[sqlx::test(migrations = "./migrations")]
async fn test_preferences_endpoint_defaults_and_round_trip(pool: PgPool) {
    let (app, _pool, _state) = test_app_and_state(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");

    let defaults = json_body(
        get(
            &app,
            "/api/v1/notifications/preferences",
            Some(&patient_token),
        )
        .await,
    )
    .await;
    assert_eq!(defaults["data"]["operational_enabled"], true);
    assert_eq!(defaults["data"]["engagement_enabled"], true);
    assert_eq!(defaults["data"]["quiet_hours_enabled"], false);
    assert_eq!(defaults["data"]["quiet_hours_start_local"], "22:00");
    assert_eq!(defaults["data"]["quiet_hours_end_local"], "07:00");
    assert_eq!(defaults["data"]["preferred_language"], "en");

    let update = put(
        &app,
        "/api/v1/notifications/preferences",
        Some(&patient_token),
        &serde_json::json!({
            "operational_enabled": false,
            "engagement_enabled": false,
            "quiet_hours_enabled": true,
            "quiet_hours_start_local": "21:30",
            "quiet_hours_end_local": "06:15",
            "language": "sw"
        }),
    )
    .await;
    assert_eq!(update.status(), StatusCode::OK);
    let updated = json_body(update).await;
    assert_eq!(updated["data"]["operational_enabled"], false);
    assert_eq!(updated["data"]["engagement_enabled"], false);
    assert_eq!(updated["data"]["quiet_hours_enabled"], true);
    assert_eq!(updated["data"]["quiet_hours_start_local"], "21:30");
    assert_eq!(updated["data"]["quiet_hours_end_local"], "06:15");
    assert_eq!(updated["data"]["preferred_language"], "sw");

    // A second GET confirms it was actually persisted, not just echoed back.
    let reread = json_body(
        get(
            &app,
            "/api/v1/notifications/preferences",
            Some(&patient_token),
        )
        .await,
    )
    .await;
    assert_eq!(reread["data"]["preferred_language"], "sw");
    assert_eq!(reread["data"]["quiet_hours_start_local"], "21:30");
}

/// An invalid `language` value must be rejected (400), never silently
/// ignored or stored — the allowed-locale set is a product rule enforced in
/// `NotificationService::update_preferences`.
#[sqlx::test(migrations = "./migrations")]
async fn test_preferences_endpoint_rejects_unsupported_language(pool: PgPool) {
    let (app, _pool, _state) = test_app_and_state(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let res = put(
        &app,
        "/api/v1/notifications/preferences",
        Some(&patient_token),
        &serde_json::json!({
            "operational_enabled": true,
            "engagement_enabled": true,
            "quiet_hours_enabled": false,
            "quiet_hours_start_local": "22:00",
            "quiet_hours_end_local": "07:00",
            "language": "fr"
        }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

/// End-to-end localization: a Patient who has set their language to Swahili
/// (via the real `/notifications/preferences` endpoint, not a raw SQL
/// update) must receive their own `BOOKING_CONFIRMED` notification in
/// Swahili, while the Doctor (still on the 'en' default) receives the
/// English wording — the same real event renders differently per recipient.
#[sqlx::test(migrations = "./migrations")]
async fn test_booking_confirmed_localizes_per_recipient_language(pool: PgPool) {
    let (app, pool, _state) = test_app_and_state(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");

    let set_language = put(
        &app,
        "/api/v1/notifications/preferences",
        Some(&patient_token),
        &serde_json::json!({
            "operational_enabled": true,
            "engagement_enabled": true,
            "quiet_hours_enabled": false,
            "quiet_hours_start_local": "22:00",
            "quiet_hours_end_local": "07:00",
            "language": "sw"
        }),
    )
    .await;
    assert_eq!(set_language.status(), StatusCode::OK);

    let (_appointment_id, _) = book_and_confirm(&app, &patient_token).await;

    let (patient_body,): (String,) = sqlx::query_as(
        "SELECT body FROM notifications WHERE user_id = $1 AND type = 'BOOKING_CONFIRMED'",
    )
    .bind(uid(PATIENT_USER_ID))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(patient_body, "Uhifadhi wako wa ushauri umethibitishwa.");

    let (doctor_body,): (String,) = sqlx::query_as(
        "SELECT body FROM notifications WHERE user_id = $1 AND type = 'BOOKING_CONFIRMED'",
    )
    .bind(uid(DOCTOR1_USER_ID))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(doctor_body, "A consultation booking has been confirmed.");
}

/// If a locale's template row is ever missing/disabled, rendering must fall
/// back to the caller-supplied English default rather than failing the
/// notification (and therefore the booking-confirmation transaction)
/// outright.
#[sqlx::test(migrations = "./migrations")]
async fn test_render_falls_back_to_english_when_locale_template_missing(pool: PgPool) {
    let (app, pool, _state) = test_app_and_state(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");

    let set_language = put(
        &app,
        "/api/v1/notifications/preferences",
        Some(&patient_token),
        &serde_json::json!({
            "operational_enabled": true,
            "engagement_enabled": true,
            "quiet_hours_enabled": false,
            "quiet_hours_start_local": "22:00",
            "quiet_hours_end_local": "07:00",
            "language": "sw"
        }),
    )
    .await;
    assert_eq!(set_language.status(), StatusCode::OK);

    sqlx::query(
        "DELETE FROM notification_templates WHERE template_key = 'booking_confirmed_patient' AND locale = 'sw'",
    )
    .execute(&pool)
    .await
    .unwrap();

    let (_appointment_id, _) = book_and_confirm(&app, &patient_token).await;

    let (patient_body,): (String,) = sqlx::query_as(
        "SELECT body FROM notifications WHERE user_id = $1 AND type = 'BOOKING_CONFIRMED'",
    )
    .bind(uid(PATIENT_USER_ID))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        patient_body, "Your consultation booking has been confirmed.",
        "a missing sw template must fall back to the English default, not fail or render blank"
    );
}

/// `ARTICLE_STATUS_CHANGED` is OPERATIONAL and `quiet_hours_allowed = TRUE`
/// in the seeded registry — with the recipient's quiet hours covering the
/// whole day, it must be deferred (not suppressed, not sent) with a future
/// `next_delivery_at`. `CONSULTATION_REMINDER_15M` is CLINICAL with
/// `quiet_hours_allowed = FALSE` for the same recipient under the same
/// quiet-hours window and must NOT be deferred — proving requirement 13
/// (existing scheduled reminders are never delayed by quiet hours).
#[sqlx::test(migrations = "./migrations")]
async fn test_quiet_hours_defers_operational_but_exempts_time_sensitive_type(pool: PgPool) {
    let (_app, pool, state) = test_app_and_state(pool).await;
    let user = UserId::from(uid(DOCTOR1_USER_ID));

    notification_repo::upsert_preferences(
        &pool,
        user,
        true,
        true,
        true,
        chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
        chrono::NaiveTime::from_hms_opt(23, 59, 59).unwrap(),
        false,
        false,
    )
    .await
    .unwrap();

    let deferred_id = Uuid::new_v4();
    assert!(notification_repo::insert_if_new(
        &pool,
        deferred_id,
        user,
        "ARTICLE_STATUS_CHANGED",
        "Article published",
        "Your article is now available in the published feed.",
        None,
        None,
        None,
        "quiet-hours-deferred-test",
        None,
    )
    .await
    .unwrap());

    let exempt_id = Uuid::new_v4();
    assert!(notification_repo::insert_if_new(
        &pool,
        exempt_id,
        user,
        "CONSULTATION_REMINDER_15M",
        "Consultation starting soon",
        "Your consultation starts in 15 minutes.",
        None,
        None,
        None,
        "quiet-hours-exempt-test",
        None,
    )
    .await
    .unwrap());

    // Deliberately no device token registered: a deferred row must never
    // reach the token/send loop at all, while the exempt row proceeding past
    // the gate shows up as "no enabled device token" rather than
    // "Deferred" — that distinction is how this test tells the two paths
    // apart without needing a real transport outcome.
    let outcomes = HashMap::new();
    let mut state = state;
    state.notification_transport = std::sync::Arc::new(ScriptedTransport { outcomes });
    notification_scheduler::run_push_delivery_pass(&state)
        .await
        .unwrap();

    let (deferred_status, deferred_error, deferred_next): (String, Option<String>, Option<DateTime<Utc>>) =
        sqlx::query_as(
            "SELECT delivery_status, last_delivery_error, next_delivery_at FROM notifications WHERE id = $1",
        )
        .bind(deferred_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(deferred_status, "FAILED");
    assert!(deferred_error.unwrap().contains("Deferred"));
    assert!(
        deferred_next.is_some() && deferred_next.unwrap() > Utc::now(),
        "a quiet-hours deferral must be rescheduled to a future instant"
    );

    let (exempt_status, exempt_error): (String, Option<String>) = sqlx::query_as(
        "SELECT delivery_status, last_delivery_error FROM notifications WHERE id = $1",
    )
    .bind(exempt_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(exempt_status, "FAILED");
    assert!(
        !exempt_error.unwrap().contains("Deferred"),
        "a type with quiet_hours_allowed = FALSE must never be deferred by quiet hours"
    );
}

/// Disabling the OPERATIONAL/ENGAGEMENT category preferences must suppress
/// the push for those categories only — a SECURITY-category notification for
/// the SAME user, with BOTH toggles off, must still be delivered, proving
/// requirement 9 ("never suppress essential security or transactional
/// records due to marketing opt-outs"). In every case the in-app inbox row
/// itself must survive.
#[sqlx::test(migrations = "./migrations")]
async fn test_category_preferences_never_suppress_security_notifications(pool: PgPool) {
    let (app, pool, mut state) = test_app_and_state(pool).await;
    let doctor_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let user = UserId::from(uid(DOCTOR1_USER_ID));

    let register = post(
        &app,
        "/api/v1/notifications/devices",
        Some(&doctor_token),
        &serde_json::json!({"platform": "android", "push_token": "prefs-security-token"}),
    )
    .await;
    assert_eq!(register.status(), StatusCode::CREATED);

    notification_repo::upsert_preferences(
        &pool,
        user,
        false,
        false,
        false,
        chrono::NaiveTime::from_hms_opt(22, 0, 0).unwrap(),
        chrono::NaiveTime::from_hms_opt(7, 0, 0).unwrap(),
        false,
        false,
    )
    .await
    .unwrap();

    let operational_id = Uuid::new_v4();
    assert!(notification_repo::insert_if_new(
        &pool,
        operational_id,
        user,
        "ARTICLE_STATUS_CHANGED",
        "Article published",
        "Your article is now available in the published feed.",
        None,
        None,
        None,
        "prefs-operational-test",
        None,
    )
    .await
    .unwrap());

    let engagement_id = Uuid::new_v4();
    assert!(notification_repo::insert_if_new(
        &pool,
        engagement_id,
        user,
        "PATIENT_GREETING",
        "Hello",
        "A friendly greeting.",
        None,
        None,
        None,
        "prefs-engagement-test",
        None,
    )
    .await
    .unwrap());

    let security_id = Uuid::new_v4();
    assert!(notification_repo::insert_if_new(
        &pool,
        security_id,
        user,
        "REGISTRATION_VERIFIED",
        "Registration verified",
        "AfyaMkononi has approved your submitted KMPDC registration information.",
        None,
        None,
        None,
        "prefs-security-test",
        None,
    )
    .await
    .unwrap());

    let mut outcomes = HashMap::new();
    outcomes.insert(
        "prefs-security-token".to_string(),
        PushOutcome::SentToProvider {
            provider_message_id: Some("security-sent".into()),
        },
    );
    state.notification_transport = std::sync::Arc::new(ScriptedTransport { outcomes });
    notification_scheduler::run_push_delivery_pass(&state)
        .await
        .unwrap();

    for (id, expect_suppressed) in [
        (operational_id, true),
        (engagement_id, true),
        (security_id, false),
    ] {
        let (status, error): (String, Option<String>) = sqlx::query_as(
            "SELECT delivery_status, last_delivery_error FROM notifications WHERE id = $1",
        )
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
        if expect_suppressed {
            assert_eq!(status, "SUPPRESSED");
            assert!(error.unwrap().contains("disabled by user preference"));
        } else {
            assert_eq!(
                status, "SENT_TO_PROVIDER",
                "security notifications must never be suppressed by category opt-outs"
            );
        }
        // History is retained regardless of push outcome.
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM notifications WHERE id = $1)")
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(exists);
    }
}

/// The per-type `max_per_day` cap applies to successfully-sent pushes, not
/// raw row creation: of three same-day `ARTICLE_STATUS_CHANGED` events for
/// one user with the registry's cap lowered to 2 for this test, exactly the
/// first two must be delivered and the third suppressed for "daily
/// frequency limit reached" — while all three in-app rows remain.
#[sqlx::test(migrations = "./migrations")]
async fn test_frequency_limit_caps_after_max_per_day_sent(pool: PgPool) {
    let (app, pool, mut state) = test_app_and_state(pool).await;
    let doctor_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let user = UserId::from(uid(DOCTOR1_USER_ID));

    let register = post(
        &app,
        "/api/v1/notifications/devices",
        Some(&doctor_token),
        &serde_json::json!({"platform": "android", "push_token": "freq-cap-token"}),
    )
    .await;
    assert_eq!(register.status(), StatusCode::CREATED);

    sqlx::query("UPDATE notification_type_registry SET max_per_day = 2 WHERE notification_type = 'ARTICLE_STATUS_CHANGED'")
        .execute(&pool)
        .await
        .unwrap();

    let mut ids = Vec::new();
    for i in 0..3 {
        let id = Uuid::new_v4();
        assert!(notification_repo::insert_if_new(
            &pool,
            id,
            user,
            "ARTICLE_STATUS_CHANGED",
            "Article published",
            "Your article is now available in the published feed.",
            None,
            None,
            None,
            &format!("freq-cap-test-{i}"),
            None,
        )
        .await
        .unwrap());
        ids.push(id);
        // A strictly increasing `created_at` keeps claim ordering
        // deterministic across however fast `clock_timestamp()` ticks on
        // this machine.
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    let mut outcomes = HashMap::new();
    outcomes.insert(
        "freq-cap-token".to_string(),
        PushOutcome::SentToProvider {
            provider_message_id: Some("freq-cap-sent".into()),
        },
    );
    state.notification_transport = std::sync::Arc::new(ScriptedTransport { outcomes });
    notification_scheduler::run_push_delivery_pass(&state)
        .await
        .unwrap();

    for (index, id) in ids.iter().enumerate() {
        let (status, error): (String, Option<String>) = sqlx::query_as(
            "SELECT delivery_status, last_delivery_error FROM notifications WHERE id = $1",
        )
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
        if index < 2 {
            assert_eq!(
                status, "SENT_TO_PROVIDER",
                "the first 2 under the cap must be delivered"
            );
        } else {
            assert_eq!(status, "SUPPRESSED");
            assert!(error.unwrap().contains("daily frequency limit reached"));
        }
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM notifications WHERE id = $1)")
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(exists, "frequency capping must never delete inbox history");
    }
}

/// Idempotency: re-running the delivery pass against an already-`SENT_TO_PROVIDER`
/// row must never re-send it or change its status — `claim_due_notifications`
/// only ever claims rows still in `CREATED`/`DUE`/`FAILED`.
#[sqlx::test(migrations = "./migrations")]
async fn test_push_delivery_pass_is_idempotent_for_already_sent_rows(pool: PgPool) {
    let (app, pool, mut state) = test_app_and_state(pool).await;
    let doctor_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let user = UserId::from(uid(DOCTOR1_USER_ID));

    let register = post(
        &app,
        "/api/v1/notifications/devices",
        Some(&doctor_token),
        &serde_json::json!({"platform": "android", "push_token": "idempotent-token"}),
    )
    .await;
    assert_eq!(register.status(), StatusCode::CREATED);

    let id = Uuid::new_v4();
    assert!(notification_repo::insert_if_new(
        &pool,
        id,
        user,
        "ARTICLE_STATUS_CHANGED",
        "Article published",
        "Your article is now available in the published feed.",
        None,
        None,
        None,
        "idempotent-pass-test",
        None,
    )
    .await
    .unwrap());

    let mut outcomes = HashMap::new();
    outcomes.insert(
        "idempotent-token".to_string(),
        PushOutcome::SentToProvider {
            provider_message_id: Some("idempotent-first".into()),
        },
    );
    state.notification_transport = std::sync::Arc::new(ScriptedTransport { outcomes });
    notification_scheduler::run_push_delivery_pass(&state)
        .await
        .unwrap();

    let (status_after_first, provider_id_after_first): (String, Option<String>) = sqlx::query_as(
        "SELECT delivery_status, provider_message_id FROM notifications WHERE id = $1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status_after_first, "SENT_TO_PROVIDER");
    assert_eq!(provider_id_after_first.as_deref(), Some("idempotent-first"));

    // A second pass, scripted so a second send WOULD be visible if it
    // happened, must change nothing.
    let mut outcomes = HashMap::new();
    outcomes.insert(
        "idempotent-token".to_string(),
        PushOutcome::SentToProvider {
            provider_message_id: Some("idempotent-second-should-never-be-recorded".into()),
        },
    );
    state.notification_transport = std::sync::Arc::new(ScriptedTransport { outcomes });
    notification_scheduler::run_push_delivery_pass(&state)
        .await
        .unwrap();

    let (status_after_second, provider_id_after_second): (String, Option<String>) = sqlx::query_as(
        "SELECT delivery_status, provider_message_id FROM notifications WHERE id = $1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status_after_second, "SENT_TO_PROVIDER");
    assert_eq!(
        provider_id_after_second.as_deref(),
        Some("idempotent-first"),
        "an already-sent row must never be reclaimed/resent"
    );
}
