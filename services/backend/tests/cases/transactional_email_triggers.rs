//! Phase 2: proves the new trigger wiring actually fires, not just that it
//! compiles — each test drives the real HTTP handler (or, for the new SQL
//! join, calls it directly against seeded data) and asserts on the real
//! `email_outbox` table, using a scripted transport so nothing touches a
//! real network.
use super::*;
use async_trait::async_trait;
use hellodoctor_backend::services::appointment_service::AppointmentService;
use hellodoctor_backend::services::email_transport::{EmailMessage, EmailOutcome, EmailTransport};
use std::sync::Mutex;

/// Always succeeds, records every message it was asked to send — enough for
/// these tests, which only need to confirm an outbox row was created with
/// the right `email_type`/recipient, not exercise retry/backoff (already
/// covered by `email_outbox_delivery.rs`).
struct AlwaysSendTransport {
    sent: Mutex<Vec<(String, String)>>, // (recipient, subject)
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

async fn outbox_row_for(
    pool: &PgPool,
    email_type: &str,
    recipient: &str,
) -> Option<(String, String)> {
    sqlx::query_as(
        "SELECT delivery_status, dedup_key FROM email_outbox \
         WHERE email_type=$1 AND recipient_email=$2 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(email_type)
    .bind(recipient)
    .fetch_optional(pool)
    .await
    .unwrap()
}

/// Phase 1B: the triggering handler's response no longer waits for delivery
/// (the thin outbox row is enqueued atomically with the business
/// transaction; the actual send happens on a detached `tokio::spawn` fired
/// right after commit — see `email_outbox_repo::enqueue_transactional`'s doc
/// comment). A test asserting `SENT_TO_PROVIDER` immediately after the HTTP
/// call returns would be racing that spawned task, not exercising a real
/// bug — this polls instead, exactly like a well-behaved caller that cannot
/// know in advance which of two independent completions (the fast path or
/// the periodic worker) will land first.
async fn wait_for_outbox_status(
    pool: &PgPool,
    email_type: &str,
    recipient: &str,
) -> (String, String) {
    for _ in 0..100 {
        if let Some(row) = outbox_row_for(pool, email_type, recipient).await {
            if row.0 != "CREATED" {
                return row;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!("{email_type} outbox row for {recipient} never reached a terminal status");
}

#[sqlx::test(migrations = "./migrations")]
async fn doctor_registration_sends_application_received_email(pool: PgPool) {
    let (_, pool, mut state) = test_app_and_state(pool).await;
    state.email_transport = always_send();
    let app = hellodoctor_backend::api::create_router(state);

    let payload = serde_json::json!({
        "full_name": "Dr. Fictional Newcomer",
        "license_number": "SYNTH-KE-9999",
        "phone_number": "0719999099",
        "email": "fictional.newcomer@example.invalid",
        "password": "Str0ng!Passw0rd9",
        "primary_specialty": "General Practice",
        "experience_years": 3,
        "current_hospital": "Fictional Clinic",
        "qualifications": ["MBChB"],
        "residential_address": "Nairobi",
        "accepted_documents": [
            {"type": "DOCTOR_TERMS", "version": "1.0-afyamkononi"},
            {"type": "PRIVACY_NOTICE", "version": "1.0-afyamkononi"},
            {"type": "PROFESSIONAL_VERIFICATION_DECLARATION", "version": "1.0-afyamkononi"},
            {"type": "TELEMEDICINE_PRACTITIONER_POLICY", "version": "1.0-afyamkononi"}
        ]
    });
    let response = post(&app, "/api/v1/auth/doctor/register", None, &payload).await;
    assert_eq!(
        response.status(),
        StatusCode::CREATED,
        "registration must succeed for this to be a real test of the email trigger"
    );

    let (status, _dedup_key) = wait_for_outbox_status(
        &pool,
        "DOCTOR_APPLICATION_RECEIVED",
        "fictional.newcomer@example.invalid",
    )
    .await;
    assert_eq!(status, "SENT_TO_PROVIDER");
}

#[sqlx::test(migrations = "./migrations")]
async fn doctor_approval_sends_approved_email(pool: PgPool) {
    let (_, pool, mut state) = test_app_and_state(pool).await;
    state.email_transport = always_send();
    let app = hellodoctor_backend::api::create_router(state);

    sqlx::query(
        "UPDATE doctor_profiles SET professional_verification_status='PENDING' WHERE id=$1",
    )
    .bind(uid(DOCTOR1_PROFILE_ID))
    .execute(&pool)
    .await
    .unwrap();
    let admin_token = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let response = post(
        &app,
        &format!("/api/v1/admin/doctors/{DOCTOR1_PROFILE_ID}/verify"),
        Some(&admin_token),
        &serde_json::json!({ "note": "license cross-checked" }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let (status, _) = wait_for_outbox_status(
        &pool,
        "DOCTOR_APPROVED",
        hellodoctor_backend::seed::SEED_DOCTOR1_EMAIL,
    )
    .await;
    assert_eq!(status, "SENT_TO_PROVIDER");
}

#[sqlx::test(migrations = "./migrations")]
async fn doctor_rejection_sends_rejected_email(pool: PgPool) {
    let (_, pool, mut state) = test_app_and_state(pool).await;
    state.email_transport = always_send();
    let app = hellodoctor_backend::api::create_router(state);

    sqlx::query(
        "UPDATE doctor_profiles SET professional_verification_status='PENDING' WHERE id=$1",
    )
    .bind(uid(DOCTOR1_PROFILE_ID))
    .execute(&pool)
    .await
    .unwrap();
    let admin_token = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let response = post(
        &app,
        &format!("/api/v1/admin/doctors/{DOCTOR1_PROFILE_ID}/reject"),
        Some(&admin_token),
        &serde_json::json!({ "note": "credentials did not match KMPDC records" }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let (status, _) = wait_for_outbox_status(
        &pool,
        "DOCTOR_REJECTED",
        hellodoctor_backend::seed::SEED_DOCTOR1_EMAIL,
    )
    .await;
    assert_eq!(status, "SENT_TO_PROVIDER");
}

/// The new, real "sign out everywhere else" behavior: a second session is
/// established first; after changing the password from the first session,
/// the second session's access token must be rejected (its family was
/// revoked), while the first (the one that made the change) keeps working.
#[sqlx::test(migrations = "./migrations")]
async fn change_password_sends_email_and_revokes_other_sessions(pool: PgPool) {
    let (_, pool, mut state) = test_app_and_state(pool).await;
    state.email_transport = always_send();
    let redis = state.redis.clone();
    let app = hellodoctor_backend::api::create_router(state);

    // Session A: the real doctor login/verify-otp flow, so it has a real
    // auth_sessions row with a real `sid` family — a bare `make_token`
    // fixture has no backing row at all and can't prove revocation.
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
    assert_eq!(login.status(), StatusCode::OK);
    let login_body = json_body(login).await;
    let session_token = login_body["data"]["session_token"]
        .as_str()
        .unwrap()
        .to_string();
    let debug_code = login_body["data"]["debug_otp_code"]
        .as_str()
        .unwrap()
        .to_string();
    let verify = post(
        &app,
        "/api/v1/auth/doctor/verify-otp",
        None,
        &serde_json::json!({ "session_token": session_token, "otp_code": debug_code }),
    )
    .await;
    assert_eq!(verify.status(), StatusCode::OK);
    let verify_body = json_body(verify).await;
    let session_a_access = verify_body["data"]["access_token"]
        .as_str()
        .unwrap()
        .to_string();
    let session_a_refresh = verify_body["data"]["refresh_token"]
        .as_str()
        .unwrap()
        .to_string();

    // The real per-recipient OTP-delivery cooldown (1 per 60s) would
    // otherwise reject this second login for the same account within the
    // same test — correct, intentional production behavior (see
    // rate_limit::check_otp_delivery_limit), not something to work around
    // via a code change. Clearing the fixture's own disposable Redis
    // between the two logins is the test-side equivalent of "60 seconds
    // later," not a bypass of any real control.
    reset_owned_redis(&redis).await;

    // Session B: an independent second login for the same account.
    let login2 = post(
        &app,
        "/api/v1/auth/doctor/login",
        None,
        &serde_json::json!({
            "email": hellodoctor_backend::seed::SEED_DOCTOR1_EMAIL,
            "password": hellodoctor_backend::seed::SEED_DOCTOR1_PASSWORD,
        }),
    )
    .await;
    let login2_body = json_body(login2).await;
    let verify2 = post(
        &app,
        "/api/v1/auth/doctor/verify-otp",
        None,
        &serde_json::json!({
            "session_token": login2_body["data"]["session_token"].as_str().unwrap(),
            "otp_code": login2_body["data"]["debug_otp_code"].as_str().unwrap(),
        }),
    )
    .await;
    let verify2_body = json_body(verify2).await;
    let session_b_refresh = verify2_body["data"]["refresh_token"]
        .as_str()
        .unwrap()
        .to_string();

    // Change the password from session A.
    let change = post(
        &app,
        "/api/v1/auth/change-password",
        Some(&session_a_access),
        &serde_json::json!({
            "current_password": hellodoctor_backend::seed::SEED_DOCTOR1_PASSWORD,
            "new_password": "Str0ng!NewPassw0rd",
        }),
    )
    .await;
    assert_eq!(change.status(), StatusCode::OK);
    let change_body = json_body(change).await;
    // At least session B; the test harness's own fixture session (inserted
    // once per seeded user so `make_token` JWTs have a backing row) is also
    // a real, independent session for this account and correctly gets
    // revoked too — not asserting an exact count tied to that fixture
    // detail.
    assert!(
        change_body["data"]["other_sessions_revoked"]
            .as_u64()
            .unwrap()
            >= 1
    );

    // Session A is untouched: a normal authenticated call still succeeds.
    let still_works = get(&app, "/api/v1/doctor/profile", Some(&session_a_access)).await;
    assert_eq!(still_works.status(), StatusCode::OK);

    // Session B's refresh token is now revoked.
    let b_refresh_attempt = post(
        &app,
        "/api/v1/auth/refresh",
        None,
        &serde_json::json!({ "refresh_token": session_b_refresh }),
    )
    .await;
    assert_eq!(b_refresh_attempt.status(), StatusCode::UNAUTHORIZED);

    // Session A's own refresh token is explicitly NOT revoked by this change.
    let a_refresh_attempt = post(
        &app,
        "/api/v1/auth/refresh",
        None,
        &serde_json::json!({ "refresh_token": session_a_refresh }),
    )
    .await;
    assert_eq!(a_refresh_attempt.status(), StatusCode::OK);

    let row = outbox_row_for(
        &pool,
        "PASSWORD_CHANGED",
        hellodoctor_backend::seed::SEED_DOCTOR1_EMAIL,
    )
    .await;
    let (status, _) = row.expect("PASSWORD_CHANGED outbox row must exist");
    assert_eq!(status, "SENT_TO_PROVIDER");
}

#[sqlx::test(migrations = "./migrations")]
async fn change_password_rejects_wrong_current_password_and_sends_nothing(pool: PgPool) {
    let (_, pool, mut state) = test_app_and_state(pool).await;
    state.email_transport = always_send();
    let app = hellodoctor_backend::api::create_router(state);

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
    let verify = post(
        &app,
        "/api/v1/auth/doctor/verify-otp",
        None,
        &serde_json::json!({
            "session_token": login_body["data"]["session_token"].as_str().unwrap(),
            "otp_code": login_body["data"]["debug_otp_code"].as_str().unwrap(),
        }),
    )
    .await;
    let verify_body = json_body(verify).await;
    let access = verify_body["data"]["access_token"].as_str().unwrap();

    let change = post(
        &app,
        "/api/v1/auth/change-password",
        Some(access),
        &serde_json::json!({
            "current_password": "DefinitelyWrongPassword!1",
            "new_password": "Str0ng!NewPassw0rd",
        }),
    )
    .await;
    assert_eq!(change.status(), StatusCode::UNAUTHORIZED);

    let row = outbox_row_for(
        &pool,
        "PASSWORD_CHANGED",
        hellodoctor_backend::seed::SEED_DOCTOR1_EMAIL,
    )
    .await;
    assert!(row.is_none(), "no email must be sent for a rejected change");
}

/// Direct coverage of the new SQL join (`consultation_email_details`) — the
/// single riskiest new piece of hand-written SQL in this phase. Seeds a
/// CONFIRMED appointment against an existing doctor schedule slot and
/// confirms the resolved name/date fields are correct.
#[sqlx::test(migrations = "./migrations")]
async fn consultation_email_details_resolves_real_names_and_slot_time(pool: PgPool) {
    let (_, pool, state) = test_app_and_state(pool).await;

    sqlx::query("UPDATE users SET email='amani.fictional@example.invalid' WHERE id=$1")
        .bind(uid(PATIENT_USER_ID))
        .execute(&pool)
        .await
        .unwrap();

    let appointment_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO appointments(id,appointment_number,patient_id,doctor_id,slot_id,modality,status,consultation_fee) \
         SELECT $1,$1::text,$2,$3,id,'CHAT','CONFIRMED',100 FROM doctor_schedule_slots WHERE doctor_id=$3 ORDER BY start_time LIMIT 1",
    )
    .bind(appointment_id)
    .bind(uid(PATIENT_PROFILE_ID))
    .bind(uid(DOCTOR1_PROFILE_ID))
    .execute(&pool)
    .await
    .unwrap();

    let details = AppointmentService::consultation_email_details(&state, appointment_id)
        .await
        .unwrap()
        .expect("appointment must resolve to details");

    assert_eq!(details.patient_name, "Patient User");
    assert_eq!(details.recipient_email, "amani.fictional@example.invalid");
    assert!(details.doctor_name.contains("Synthetic Doctor"));
    assert_eq!(details.appointment_number, appointment_id.to_string());
    assert!(!details.date_label.is_empty());
    assert!(!details.time_label.is_empty());
}

#[sqlx::test(migrations = "./migrations")]
async fn consultation_email_details_returns_empty_recipient_when_patient_has_no_email(
    pool: PgPool,
) {
    let (_, pool, state) = test_app_and_state(pool).await;
    // Seeded patient has no email by default (phone/OTP registration).

    let appointment_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO appointments(id,appointment_number,patient_id,doctor_id,slot_id,modality,status,consultation_fee) \
         SELECT $1,$1::text,$2,$3,id,'CHAT','CONFIRMED',100 FROM doctor_schedule_slots WHERE doctor_id=$3 ORDER BY start_time LIMIT 1",
    )
    .bind(appointment_id)
    .bind(uid(PATIENT_PROFILE_ID))
    .bind(uid(DOCTOR1_PROFILE_ID))
    .execute(&pool)
    .await
    .unwrap();

    let details = AppointmentService::consultation_email_details(&state, appointment_id)
        .await
        .unwrap()
        .expect("appointment must still resolve to details");
    assert_eq!(
        details.recipient_email, "",
        "no email on file must resolve to an empty recipient, not an error"
    );
}
