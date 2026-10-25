//! Milestones 4 + 5 coverage: good-morning/birthday/holiday/welcome/
//! first-consultation-milestone/health-education/new-article engagement
//! notifications, the maintained Kenyan holiday calendar, the
//! greeting/religious opt-in preferences, and concurrency-safety of the
//! atomic daily frequency cap across multiple simulated delivery workers.
use super::*;
use async_trait::async_trait;
use hellodoctor_backend::repository::notification_repo;
use hellodoctor_backend::services::notification_scheduler;
use hellodoctor_backend::services::notification_transport::{
    NotificationTransport, PushOptions, PushOutcome,
};
use std::collections::HashMap;
use std::sync::Arc;

struct AlwaysSendTransport;

#[async_trait]
impl NotificationTransport for AlwaysSendTransport {
    fn is_configured(&self) -> bool {
        true
    }
    async fn send_push(
        &self,
        _push_token: &str,
        _title: &str,
        _body: &str,
        _options: &PushOptions,
    ) -> PushOutcome {
        PushOutcome::SentToProvider {
            provider_message_id: Some("concurrency-test-sent".into()),
        }
    }
}

/// Requirement: "Verify frequency-cap concurrency across multiple delivery
/// workers." Ten same-type, same-user `ARTICLE_STATUS_CHANGED` rows (cap
/// lowered to 3) are claimed by five genuinely concurrent `tokio::spawn`
/// tasks, each running its own `run_push_delivery_pass` against the SAME
/// shared pool/state — simulating five backend instances racing on the
/// same claim. `claim_due_notifications`'s `FOR UPDATE SKIP LOCKED` already
/// partitions which rows each task claims; the atomic
/// `claim_frequency_slot` upsert is what must still hold the cap across
/// that race, which a plain `SELECT COUNT(*)` (Milestone 3's original
/// design) could not guarantee.
#[sqlx::test(migrations = "./migrations")]
async fn test_frequency_cap_holds_under_concurrent_delivery_workers(pool: PgPool) {
    let (app, pool, mut state) = test_app_and_state(pool).await;
    let doctor_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let user = UserId::from(uid(DOCTOR1_USER_ID));

    let register = post(
        &app,
        "/api/v1/notifications/devices",
        Some(&doctor_token),
        &serde_json::json!({"platform": "android", "push_token": "concurrency-token"}),
    )
    .await;
    assert_eq!(register.status(), StatusCode::CREATED);

    sqlx::query("UPDATE notification_type_registry SET max_per_day = 3 WHERE notification_type = 'ARTICLE_STATUS_CHANGED'")
        .execute(&pool)
        .await
        .unwrap();

    for i in 0..10 {
        assert!(notification_repo::insert_if_new(
            &pool,
            Uuid::new_v4(),
            user,
            "ARTICLE_STATUS_CHANGED",
            "Article published",
            "Your article is now available in the published feed.",
            None,
            None,
            None,
            &format!("concurrency-cap-test-{i}"),
            None,
        )
        .await
        .unwrap());
    }

    state.notification_transport = Arc::new(AlwaysSendTransport);

    let mut handles = Vec::new();
    for _ in 0..5 {
        let worker_state = state.clone();
        handles.push(tokio::spawn(async move {
            notification_scheduler::run_push_delivery_pass(&worker_state)
                .await
                .unwrap();
        }));
    }
    for h in handles {
        h.await.unwrap();
    }

    let (sent_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM notifications WHERE user_id = $1 AND type = 'ARTICLE_STATUS_CHANGED' AND delivery_status = 'SENT_TO_PROVIDER'",
    )
    .bind(user)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        sent_count, 3,
        "exactly the cap must be sent even when multiple workers race on the same claim"
    );

    let (suppressed_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM notifications WHERE user_id = $1 AND type = 'ARTICLE_STATUS_CHANGED' AND last_delivery_error LIKE '%frequency limit%'",
    )
    .bind(user)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        suppressed_count, 7,
        "the remaining 7 must be suppressed, not silently dropped or double-sent"
    );

    let (total_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM notifications WHERE user_id = $1 AND type = 'ARTICLE_STATUS_CHANGED'",
    )
    .bind(user)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        total_count, 10,
        "every row's in-app history must survive regardless of push outcome"
    );
}

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

/// Patients default to greetings OFF, Doctors default ON — the stricter,
/// separate opt-in axis from `engagement_enabled` (role-aware default with
/// no preference row yet). `DOCTOR_GREETING`/`PATIENT_GREETING` only fire
/// on a Nairobi-local Monday at/after the configured time, and the gate
/// decides per-recipient from each one's own actual preference state.
#[sqlx::test(migrations = "./migrations")]
async fn test_greeting_defaults_doctor_on_patient_off_and_weekly_dedup(pool: PgPool) {
    let (app, pool, mut state) = test_app_and_state(pool).await;
    let doctor_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");

    // The seeded fixture patient happens to use the exact OTP-login
    // placeholder shape (`full_name: "Patient User"`, `date_of_birth:
    // 1995-01-01`) as its own test data — give it a real completed profile
    // first so this test exercises "non-placeholder Patient defaults
    // greetings off", not the separate placeholder-exclusion rule.
    sqlx::query(
        "UPDATE patient_profiles SET full_name = 'Wanjiku Kamau', date_of_birth = '1988-05-20', gender = 'FEMALE' WHERE id = $1",
    )
    .bind(uid(PATIENT_PROFILE_ID))
    .execute(&pool)
    .await
    .unwrap();

    for (token, push_token) in [
        (&doctor_token, "greeting-doctor-token"),
        (&patient_token, "greeting-patient-token"),
    ] {
        let register = post(
            &app,
            "/api/v1/notifications/devices",
            Some(token),
            &serde_json::json!({"platform": "android", "push_token": push_token}),
        )
        .await;
        assert_eq!(register.status(), StatusCode::CREATED);
    }

    let mut outcomes = HashMap::new();
    outcomes.insert(
        "greeting-doctor-token".to_string(),
        PushOutcome::SentToProvider {
            provider_message_id: Some("doctor-greeting-sent".into()),
        },
    );
    outcomes.insert(
        "greeting-patient-token".to_string(),
        PushOutcome::SentToProvider {
            provider_message_id: Some("patient-greeting-should-never-be-sent".into()),
        },
    );
    state.notification_transport = Arc::new(ScriptedTransport { outcomes });

    // A Monday at/after the configured 08:00 Nairobi-local greeting time.
    let monday_morning = DateTime::parse_from_rfc3339("2026-11-02T06:30:00Z")
        .unwrap()
        .with_timezone(&Utc);
    notification_scheduler::run_greeting_pass(&state, monday_morning)
        .await
        .unwrap();
    notification_scheduler::run_push_delivery_pass(&state)
        .await
        .unwrap();

    let (doctor_status,): (String,) = sqlx::query_as(
        "SELECT delivery_status FROM notifications WHERE user_id = $1 AND type = 'DOCTOR_GREETING'",
    )
    .bind(uid(DOCTOR1_USER_ID))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        doctor_status, "SENT_TO_PROVIDER",
        "Doctors default to greetings on"
    );

    let (patient_status, patient_error): (String, Option<String>) = sqlx::query_as(
        "SELECT delivery_status, last_delivery_error FROM notifications WHERE user_id = $1 AND type = 'PATIENT_GREETING'",
    )
    .bind(uid(PATIENT_USER_ID))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(patient_status, "FAILED");
    assert!(
        patient_error.unwrap().contains("greeting"),
        "Patients default to greetings off"
    );

    // Re-running the same pass for the SAME week must never create a
    // second row (dedup_key is keyed by ISO week, not by tick).
    notification_scheduler::run_greeting_pass(&state, monday_morning + chrono::Duration::hours(2))
        .await
        .unwrap();
    let (doctor_row_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM notifications WHERE user_id = $1 AND type = 'DOCTOR_GREETING'",
    )
    .bind(uid(DOCTOR1_USER_ID))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        doctor_row_count, 1,
        "at most one greeting per ISO week, even across multiple ticks"
    );

    // A Tuesday must never fire a greeting at all.
    let tuesday = DateTime::parse_from_rfc3339("2026-11-03T06:30:00Z")
        .unwrap()
        .with_timezone(&Utc);
    notification_scheduler::run_greeting_pass(&state, tuesday)
        .await
        .unwrap();
    let (still_one,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM notifications WHERE user_id = $1 AND type = 'DOCTOR_GREETING'",
    )
    .bind(uid(DOCTOR1_USER_ID))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        still_one, 1,
        "only Monday triggers the good-morning greeting"
    );
}

/// A Patient who explicitly opts into greetings receives `BIRTHDAY_GREETING`
/// only on the exact Nairobi-local calendar day matching their real
/// `date_of_birth` — never a day before/after, and never for the OTP-login
/// placeholder profile shape.
#[sqlx::test(migrations = "./migrations")]
async fn test_birthday_greeting_fires_only_on_exact_date_and_excludes_placeholder(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");

    let opt_in = put(
        &app,
        "/api/v1/notifications/preferences",
        Some(&patient_token),
        &serde_json::json!({
            "operational_enabled": true,
            "engagement_enabled": true,
            "quiet_hours_enabled": false,
            "quiet_hours_start_local": "22:00",
            "quiet_hours_end_local": "07:00",
            "greeting_notifications_enabled": true,
            "religious_greetings_enabled": false
        }),
    )
    .await;
    assert_eq!(opt_in.status(), StatusCode::OK);

    sqlx::query("UPDATE patient_profiles SET date_of_birth = '1990-03-15' WHERE id = $1")
        .bind(uid(PATIENT_PROFILE_ID))
        .execute(&pool)
        .await
        .unwrap();

    let not_birthday = DateTime::parse_from_rfc3339("2026-03-14T09:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    notification_scheduler::run_birthday_pass(&state, not_birthday)
        .await
        .unwrap();
    let (count_before,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM notifications WHERE type = 'BIRTHDAY_GREETING'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count_before, 0, "must not fire a day early");

    let birthday = DateTime::parse_from_rfc3339("2026-03-15T09:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    notification_scheduler::run_birthday_pass(&state, birthday)
        .await
        .unwrap();
    let (body,): (String,) = sqlx::query_as(
        "SELECT body FROM notifications WHERE user_id = $1 AND type = 'BIRTHDAY_GREETING'",
    )
    .bind(uid(PATIENT_USER_ID))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(body.contains("Birthday") || body.contains("Heri"));

    // Re-running the same exact date must not duplicate (per-year dedup).
    notification_scheduler::run_birthday_pass(&state, birthday + chrono::Duration::hours(3))
        .await
        .unwrap();
    let (count_after,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM notifications WHERE type = 'BIRTHDAY_GREETING'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count_after, 1);

    // A dependent still in the exact OTP-placeholder shape must never get a
    // birthday greeting even if its (fabricated) date happens to match.
    sqlx::query(
        "UPDATE patient_profiles SET full_name = 'Patient User', date_of_birth = '1995-01-01', gender = 'OTHER' WHERE id = $1",
    )
    .bind(uid(PATIENT_PROFILE_ID))
    .execute(&pool)
    .await
    .unwrap();
    let placeholder_birthday = DateTime::parse_from_rfc3339("2026-01-01T09:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    notification_scheduler::run_birthday_pass(&state, placeholder_birthday)
        .await
        .unwrap();
    // Still exactly the one real (1990-03-15) birthday row from earlier —
    // the placeholder-shaped profile's fabricated 1995-01-01 "birthday"
    // must never add a second row.
    let (placeholder_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM notifications WHERE type = 'BIRTHDAY_GREETING'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        placeholder_count, 1,
        "an OTP-login placeholder profile must never receive a birthday greeting"
    );
}

/// Kenyan holiday calendar: fires only on the exact gazetted date, never a
/// day late (simulating a scheduler outage that resumes after the holiday
/// already passed), and a religious-origin holiday (Eid) additionally
/// requires the separate `religious_greetings_enabled` opt-in that a purely
/// civic holiday (New Year) never needs.
#[sqlx::test(migrations = "./migrations")]
async fn test_holiday_pass_exact_date_and_religious_opt_in(pool: PgPool) {
    let (app, pool, mut state) = test_app_and_state(pool).await;
    let doctor_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");

    let opt_in_greetings_only = put(
        &app,
        "/api/v1/notifications/preferences",
        Some(&doctor_token),
        &serde_json::json!({
            "operational_enabled": true,
            "engagement_enabled": true,
            "quiet_hours_enabled": false,
            "quiet_hours_start_local": "22:00",
            "quiet_hours_end_local": "07:00",
            "greeting_notifications_enabled": true,
            "religious_greetings_enabled": false
        }),
    )
    .await;
    assert_eq!(opt_in_greetings_only.status(), StatusCode::OK);

    let register = post(
        &app,
        "/api/v1/notifications/devices",
        Some(&doctor_token),
        &serde_json::json!({"platform": "android", "push_token": "holiday-token"}),
    )
    .await;
    assert_eq!(register.status(), StatusCode::CREATED);
    let mut outcomes = HashMap::new();
    outcomes.insert(
        "holiday-token".to_string(),
        PushOutcome::SentToProvider {
            provider_message_id: Some("holiday-sent".into()),
        },
    );
    state.notification_transport = Arc::new(ScriptedTransport { outcomes });

    // Real seeded civic holiday: New Year's Day 2026.
    let new_year = DateTime::parse_from_rfc3339("2026-01-01T09:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    notification_scheduler::run_holiday_pass(&state, new_year)
        .await
        .unwrap();
    notification_scheduler::run_push_delivery_pass(&state)
        .await
        .unwrap();
    let (new_year_status,): (String,) = sqlx::query_as(
        "SELECT delivery_status FROM notifications WHERE user_id = $1 AND type = 'HOLIDAY_GREETING' AND dedup_key LIKE 'holiday:new_year:%'",
    )
    .bind(uid(DOCTOR1_USER_ID))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        new_year_status, "SENT_TO_PROVIDER",
        "a civic holiday never needs the religious opt-in"
    );

    // Real seeded religious holiday: Eid al-Fitr 2026 (2026-03-20).
    let eid = DateTime::parse_from_rfc3339("2026-03-20T09:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    notification_scheduler::run_holiday_pass(&state, eid)
        .await
        .unwrap();
    notification_scheduler::run_push_delivery_pass(&state)
        .await
        .unwrap();
    let (eid_status, eid_error): (String, Option<String>) = sqlx::query_as(
        "SELECT delivery_status, last_delivery_error FROM notifications WHERE user_id = $1 AND type = 'HOLIDAY_GREETING' AND dedup_key LIKE 'holiday:eid_al_fitr:%'",
    )
    .bind(uid(DOCTOR1_USER_ID))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(eid_status, "FAILED");
    assert!(
        eid_error.unwrap().contains("religious"),
        "a religious holiday must require the separate opt-in"
    );

    // A scheduler "catching up" a day after Eid must never send it late.
    let (count_before_late_check,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM notifications WHERE dedup_key LIKE 'holiday:eid_al_fitr:%'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let day_after_eid = DateTime::parse_from_rfc3339("2026-03-21T09:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    notification_scheduler::run_holiday_pass(&state, day_after_eid)
        .await
        .unwrap();
    let (count_after_late_check,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM notifications WHERE dedup_key LIKE 'holiday:eid_al_fitr:%'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        count_before_late_check, count_after_late_check,
        "a holiday must never be sent late after the real date has passed"
    );
}

/// `WELCOME_GREETING` fires exactly once, the moment `PATCH /patients/me`
/// genuinely takes the caller's profile out of the OTP-login placeholder
/// shape — never on an edit that keeps it a placeholder, and never twice
/// for the same account.
#[sqlx::test(migrations = "./migrations")]
async fn test_welcome_greeting_fires_once_on_real_profile_completion(pool: PgPool) {
    let (app, pool, _state) = test_app_and_state(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");

    // Force the fixture into the exact placeholder shape first (the seeded
    // fixture patient is already a real, completed profile).
    sqlx::query(
        "UPDATE patient_profiles SET full_name = 'Patient User', date_of_birth = '1995-01-01', gender = 'OTHER' WHERE id = $1",
    )
    .bind(uid(PATIENT_PROFILE_ID))
    .execute(&pool)
    .await
    .unwrap();

    // An edit that keeps every placeholder field unchanged must not fire.
    let noop_edit = send(
        &app,
        "PATCH",
        "/api/v1/patients/me",
        Some(&patient_token),
        Some(&serde_json::json!({"blood_group": "O+"})),
        None,
    )
    .await;
    assert_eq!(noop_edit.status(), StatusCode::OK);
    let (count_after_noop,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM notifications WHERE type = 'WELCOME_GREETING'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count_after_noop, 0);

    let real_completion = send(
        &app,
        "PATCH",
        "/api/v1/patients/me",
        Some(&patient_token),
        Some(&serde_json::json!({"full_name": "Asha Wanjiru", "date_of_birth": "1992-07-04", "gender": "FEMALE"})),
        None,
    )
    .await;
    assert_eq!(real_completion.status(), StatusCode::OK);
    let (count_after_real,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM notifications WHERE type = 'WELCOME_GREETING'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count_after_real, 1);

    // A further unrelated edit must never fire a second welcome greeting.
    let another_edit = send(
        &app,
        "PATCH",
        "/api/v1/patients/me",
        Some(&patient_token),
        Some(&serde_json::json!({"blood_group": "AB-"})),
        None,
    )
    .await;
    assert_eq!(another_edit.status(), StatusCode::OK);
    let (count_after_second_edit,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM notifications WHERE type = 'WELCOME_GREETING'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count_after_second_edit, 1);
}

/// `FIRST_CONSULTATION_MILESTONE` fires once, the moment the Patient's
/// first-ever consultation actually completes — never on a second.
#[sqlx::test(migrations = "./migrations")]
async fn test_first_consultation_milestone_fires_once(pool: PgPool) {
    let (app, pool, _state) = test_app_and_state(pool).await;
    let doctor_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");

    let (apt_id_1, _) = book_and_confirm(&app, &patient_token).await;
    let complete_1 = post(
        &app,
        &format!("/api/v1/consultations/{apt_id_1}/complete"),
        Some(&doctor_token),
        &serde_json::json!({"outcome": "COMPLETED_NO_RX"}),
    )
    .await;
    assert_eq!(complete_1.status(), StatusCode::OK);

    let (count_after_first,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM notifications WHERE user_id = $1 AND type = 'FIRST_CONSULTATION_MILESTONE'",
    )
    .bind(uid(PATIENT_USER_ID))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count_after_first, 1);

    let (apt_id_2, _) = book_and_confirm(&app, &patient_token).await;
    let complete_2 = post(
        &app,
        &format!("/api/v1/consultations/{apt_id_2}/complete"),
        Some(&doctor_token),
        &serde_json::json!({"outcome": "COMPLETED_NO_RX"}),
    )
    .await;
    assert_eq!(complete_2.status(), StatusCode::OK);

    let (count_after_second,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM notifications WHERE user_id = $1 AND type = 'FIRST_CONSULTATION_MILESTONE'",
    )
    .bind(uid(PATIENT_USER_ID))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        count_after_second, 1,
        "a second consultation must never fire a second milestone"
    );
}

/// `NEW_ARTICLE_PUBLISHED` fans out once per newly-published article to
/// every active Patient, then marks the article processed so a later pass
/// (simulating a Patient who registers afterward) never re-notifies for an
/// old article ("prevent historical events from being sent late").
#[sqlx::test(migrations = "./migrations")]
async fn test_new_article_published_fans_out_once_and_never_rescans(pool: PgPool) {
    let (_app, pool, state) = test_app_and_state(pool).await;

    let article_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO doctor_articles (id, author_id, title, summary, body, status, creation_key, creation_hash, published_at) \
         VALUES ($1, $2, 'Managing Seasonal Allergies', 'A short guide', 'Full body text here.', 'PUBLISHED', $3, '', clock_timestamp())",
    )
    .bind(article_id)
    .bind(uid(DOCTOR1_PROFILE_ID))
    .bind(format!("test-key-{article_id}"))
    .execute(&pool)
    .await
    .unwrap();

    notification_scheduler::run_new_article_pass(&state)
        .await
        .unwrap();

    let (body,): (String,) = sqlx::query_as(
        "SELECT body FROM notifications WHERE user_id = $1 AND type = 'NEW_ARTICLE_PUBLISHED'",
    )
    .bind(uid(PATIENT_USER_ID))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(body.contains("new health article") || body.contains("Makala mpya"));
    assert!(
        !body.contains("Managing Seasonal Allergies"),
        "article titles stay in authenticated inbox metadata, not lock-screen push copy"
    );

    let (notified_at,): (Option<DateTime<Utc>>,) =
        sqlx::query_as("SELECT engagement_notified_at FROM doctor_articles WHERE id = $1")
            .bind(article_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(notified_at.is_some());

    // A newly-activated patient appearing AFTER the fan-out must never be
    // retroactively notified about this already-processed article.
    sqlx::query("UPDATE patient_profiles SET full_name = 'Brand New Patient' WHERE id = $1")
        .bind(uid(PATIENT_PROFILE_ID))
        .execute(&pool)
        .await
        .unwrap();
    notification_scheduler::run_new_article_pass(&state)
        .await
        .unwrap();
    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM notifications WHERE type = 'NEW_ARTICLE_PUBLISHED'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        count, 1,
        "an already-processed article must never be rescanned"
    );
}

/// The Doctor's own notification-content language is configurable through
/// the same generic `/notifications/preferences` endpoint Milestone 3 built
/// — no Doctor-specific backend gap exists; this just confirms it directly
/// rather than assuming.
#[sqlx::test(migrations = "./migrations")]
async fn test_doctor_language_preference_is_configurable(pool: PgPool) {
    let (app, pool, _state) = test_app_and_state(pool).await;
    let doctor_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");

    let update = put(
        &app,
        "/api/v1/notifications/preferences",
        Some(&doctor_token),
        &serde_json::json!({
            "operational_enabled": true,
            "engagement_enabled": true,
            "quiet_hours_enabled": false,
            "quiet_hours_start_local": "22:00",
            "quiet_hours_end_local": "07:00",
            "greeting_notifications_enabled": true,
            "religious_greetings_enabled": false,
            "language": "sw"
        }),
    )
    .await;
    assert_eq!(update.status(), StatusCode::OK);

    let (language,): (String,) =
        sqlx::query_as("SELECT preferred_language FROM users WHERE id = $1")
            .bind(uid(DOCTOR1_USER_ID))
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(language, "sw");
}
