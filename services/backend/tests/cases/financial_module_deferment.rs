//! Regression coverage for `FINANCIAL_MODULE_MODE=deferred|enabled`
//! (`config.rs::FinancialModuleMode`, `AppointmentService::book_appointment`/
//! `book_appointment_deferred`, migrations 20261210000001/20261210000002).
//! `use super::*;` pulls in `test_app`/`test_app_with_config`, `test_config`,
//! the HTTP helpers, and the seeded `DOCTOR1_*`/`PATIENT_*` identities, the
//! same way `profile_pictures.rs`/`schedule_exclusivity.rs` do.
use super::*;

fn deferred_config() -> AppConfig {
    AppConfig {
        financial_module_mode: hellodoctor_backend::config::FinancialModuleMode::Deferred,
        ..test_config()
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn test_deferred_booking_requires_no_gateway_and_creates_no_transaction(pool: PgPool) {
    let (app, pool) = test_app_with_config(pool, deferred_config()).await;
    let patient_token = fresh_patient_token(&pool).await;

    let res_slots = get(
        &app,
        &format!("/api/v1/doctors/{DOCTOR1_PROFILE_ID}/slots"),
        Some(&patient_token),
    )
    .await;
    assert_eq!(res_slots.status(), StatusCode::OK);
    let slot_id = Uuid::parse_str(
        json_body(res_slots).await["data"][0]["id"]
            .as_str()
            .unwrap(),
    )
    .unwrap();

    // Deliberately no "gateway" and no "payment_method_id" at all.
    let res_book = post_idem(
        &app,
        "/api/v1/appointments/book",
        Some(&patient_token),
        &serde_json::json!({
            "doctor_id": DOCTOR1_PROFILE_ID,
            "slot_id": slot_id,
            "modality": "VIDEO",
            "chief_complaint": "Deferred-mode booking"
        }),
        &Uuid::new_v4().to_string(),
    )
    .await;
    assert_eq!(res_book.status(), StatusCode::CREATED);
    let body = json_body(res_book).await;
    assert_eq!(body["data"]["status"], "CONFIRMED");
    assert_eq!(body["data"]["payment_policy"], "NOT_REQUIRED");
    assert!(body["data"]["payment_session_id"].is_null());
    assert!(body["data"]["payment_redirect_url"].is_null());
    assert!(body["data"]["hold_expires_at"].is_null());

    let apt_id = Uuid::parse_str(body["data"]["appointment_id"].as_str().unwrap()).unwrap();
    let apt = appointment_repo::find_by_id(&pool, apt_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(apt.status, AppointmentStatus::Confirmed);
    assert_eq!(apt.payment_policy, PaymentPolicy::NotRequired);

    // No transaction of any kind was ever created for this appointment — not
    // even a placeholder. Never forge a paid status or fabricate a payment
    // event.
    let txn = hellodoctor_backend::repository::transaction_repo::find_by_appointment(&pool, apt_id)
        .await
        .unwrap();
    assert!(txn.is_none());

    let slot_status: String =
        sqlx::query_scalar("SELECT status::text FROM doctor_schedule_slots WHERE id=$1")
            .bind(slot_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(slot_status, "BOOKED");

    // The listing endpoint surfaces the same policy for history display.
    let res_list = get(
        &app,
        "/api/v1/patients/me/appointments?status=upcoming",
        Some(&patient_token),
    )
    .await;
    let list = json_body(res_list).await;
    let row = list["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == apt_id.to_string())
        .unwrap();
    assert_eq!(row["payment_policy"], "NOT_REQUIRED");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_deferred_booking_still_rejects_double_booking_the_same_slot(pool: PgPool) {
    let (app, pool) = test_app_with_config(pool, deferred_config()).await;
    let patient_token = fresh_patient_token(&pool).await;
    let res_slots = get(
        &app,
        &format!("/api/v1/doctors/{DOCTOR1_PROFILE_ID}/slots"),
        Some(&patient_token),
    )
    .await;
    let slot_id = Uuid::parse_str(
        json_body(res_slots).await["data"][0]["id"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let payload = serde_json::json!({
        "doctor_id": DOCTOR1_PROFILE_ID,
        "slot_id": slot_id,
        "modality": "VIDEO",
    });
    let first = post(
        &app,
        "/api/v1/appointments/book",
        Some(&patient_token),
        &payload,
    )
    .await;
    assert_eq!(first.status(), StatusCode::CREATED);
    let second = post(
        &app,
        "/api/v1/appointments/book",
        Some(&patient_token),
        &payload,
    )
    .await;
    assert_eq!(second.status(), StatusCode::CONFLICT);
}

/// Full deferred-mode clinical lifecycle: booking is immediately eligible
/// for Agora joining under the same existing consultation-authorization
/// rules, and completes with no earnings/ledger/settlement record created.
#[sqlx::test(migrations = "./migrations")]
async fn test_deferred_booking_can_join_agora_and_complete_with_no_earnings(pool: PgPool) {
    let (app, pool) = test_app_with_config(pool, deferred_config()).await;
    let patient_token = fresh_patient_token(&pool).await;
    let doctor1_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");

    let wallet_before: Option<(sqlx::types::BigDecimal,)> =
        sqlx::query_as("SELECT lifetime_gross FROM doctor_wallets WHERE doctor_id=$1")
            .bind(uid(DOCTOR1_PROFILE_ID))
            .fetch_optional(&pool)
            .await
            .unwrap();

    let res_slots = get(
        &app,
        &format!("/api/v1/doctors/{DOCTOR1_PROFILE_ID}/slots"),
        Some(&patient_token),
    )
    .await;
    let slot_id = Uuid::parse_str(
        json_body(res_slots).await["data"][0]["id"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let res_book = post(
        &app,
        "/api/v1/appointments/book",
        Some(&patient_token),
        &serde_json::json!({"doctor_id": DOCTOR1_PROFILE_ID, "slot_id": slot_id, "modality": "VIDEO"}),
    )
    .await;
    assert_eq!(res_book.status(), StatusCode::CREATED);
    let apt_id = Uuid::parse_str(
        json_body(res_book).await["data"]["appointment_id"]
            .as_str()
            .unwrap(),
    )
    .unwrap();

    let res_rtc = post(
        &app,
        &format!("/api/v1/consultations/{apt_id}/rtc-token"),
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

    let res_comp = post(
        &app,
        &format!("/api/v1/consultations/{apt_id}/complete"),
        Some(&doctor1_token),
        &serde_json::json!({ "outcome": "COMPLETED_NO_RX" }),
    )
    .await;
    assert_eq!(res_comp.status(), StatusCode::OK);

    let apt = appointment_repo::find_by_id(&pool, apt_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(apt.status, AppointmentStatus::Completed);

    // No transaction, no wallet credit — the existing earnings pipeline is
    // keyed strictly off real transactions.payment_status rows, so a
    // payment-less appointment is naturally invisible to it; assert that
    // invariant directly rather than trusting it implicitly.
    assert!(
        hellodoctor_backend::repository::transaction_repo::find_by_appointment(&pool, apt_id)
            .await
            .unwrap()
            .is_none()
    );
    let wallet_after: Option<(sqlx::types::BigDecimal,)> =
        sqlx::query_as("SELECT lifetime_gross FROM doctor_wallets WHERE doctor_id=$1")
            .bind(uid(DOCTOR1_PROFILE_ID))
            .fetch_optional(&pool)
            .await
            .unwrap();
    assert_eq!(
        wallet_before, wallet_after,
        "completing a NOT_REQUIRED appointment must never move the doctor's wallet balance"
    );

    // Retrying completion with the same outcome stays idempotent (ALREADY_
    // COMPLETED maps to a plain success, same as the paid path) — not a
    // second attempted settlement.
    let res_retry = post(
        &app,
        &format!("/api/v1/consultations/{apt_id}/complete"),
        Some(&doctor1_token),
        &serde_json::json!({ "outcome": "COMPLETED_NO_RX" }),
    )
    .await;
    assert_eq!(res_retry.status(), StatusCode::OK);

    let wallet_after_retry: Option<(sqlx::types::BigDecimal,)> =
        sqlx::query_as("SELECT lifetime_gross FROM doctor_wallets WHERE doctor_id=$1")
            .bind(uid(DOCTOR1_PROFILE_ID))
            .fetch_optional(&pool)
            .await
            .unwrap();
    assert_eq!(
        wallet_before, wallet_after_retry,
        "an idempotent retry must never move the wallet balance either"
    );
}

/// `FinancialModuleMode::Enabled` (the default) must keep today's exact
/// behavior: a booking with neither `gateway` nor `payment_method_id` is
/// still rejected, never silently treated as payment-not-required.
#[sqlx::test(migrations = "./migrations")]
async fn test_enabled_mode_still_requires_gateway_or_payment_method(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient_token = fresh_patient_token(&pool).await;
    let res_slots = get(
        &app,
        &format!("/api/v1/doctors/{DOCTOR1_PROFILE_ID}/slots"),
        Some(&patient_token),
    )
    .await;
    let slot_id = Uuid::parse_str(
        json_body(res_slots).await["data"][0]["id"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let res = post(
        &app,
        "/api/v1/appointments/book",
        Some(&patient_token),
        &serde_json::json!({"doctor_id": DOCTOR1_PROFILE_ID, "slot_id": slot_id, "modality": "VIDEO"}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_booking_policy_endpoint_reports_deferred(pool: PgPool) {
    let token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let (app, _pool) = test_app_with_config(pool, deferred_config()).await;
    let res = get(&app, "/api/v1/config/booking-policy", Some(&token)).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res).await["data"]["payment_required"], false);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_booking_policy_endpoint_reports_enabled_by_default(pool: PgPool) {
    let token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let (app, _pool) = test_app(pool).await;
    let res = get(&app, "/api/v1/config/booking-policy", Some(&token)).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res).await["data"]["payment_required"], true);
}

/// Structural defense-in-depth: a replayed/forged payment webhook can never
/// mutate a deferred-mode appointment, because one never has a
/// `payment_session_id`/`transactions` row for the webhook to match against
/// in the first place — not because of an extra check, but because none was
/// ever created.
#[sqlx::test(migrations = "./migrations")]
async fn test_payment_webhook_cannot_match_a_deferred_appointment(pool: PgPool) {
    let (app, pool) = test_app_with_config(pool, deferred_config()).await;
    let patient_token = fresh_patient_token(&pool).await;
    let res_slots = get(
        &app,
        &format!("/api/v1/doctors/{DOCTOR1_PROFILE_ID}/slots"),
        Some(&patient_token),
    )
    .await;
    let slot_id = Uuid::parse_str(
        json_body(res_slots).await["data"][0]["id"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    post(
        &app,
        "/api/v1/appointments/book",
        Some(&patient_token),
        &serde_json::json!({"doctor_id": DOCTOR1_PROFILE_ID, "slot_id": slot_id, "modality": "VIDEO"}),
    )
    .await;

    // No real payment_session_id ever existed for this booking; a forged
    // one naturally finds no transaction to confirm.
    let forged_session = Uuid::new_v4();
    let webhook_payload = serde_json::json!({
        "payment_session_id": forged_session,
        "transaction_reference": format!("FORGED-{}", Uuid::new_v4()),
        "status": "SUCCESS",
        "event_timestamp": chrono::Utc::now().to_rfc3339(),
        "provider_event_id": Uuid::new_v4().to_string(),
    });
    let body_str = serde_json::to_string(&webhook_payload).unwrap();
    let sig = sign_webhook_body(&body_str, TEST_WEBHOOK_SECRET);
    let res_webhook = send_webhook(&app, &body_str, &sig).await;
    assert_eq!(res_webhook.status(), StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// P0 regression: "This doctor hasn't set up consultation booking yet."
// Self-registered doctors are created with consultation fees of 0
// (auth_handlers.rs). In deferred mode no fee is charged, so such a doctor
// must still be discoverable (VIDEO filter) and bookable; in enabled mode the
// fee remains the offering signal, exactly as before.
// ---------------------------------------------------------------------------

async fn unprice_doctor1(pool: &PgPool) {
    sqlx::query(
        "UPDATE doctor_profiles SET consultation_fee_video = 0, consultation_fee_chat = 0 WHERE id = $1",
    )
    .bind(Uuid::parse_str(DOCTOR1_PROFILE_ID).unwrap())
    .execute(pool)
    .await
    .unwrap();
}

async fn first_bookable_slot(app: &axum::Router, token: &str) -> Uuid {
    let res = get(
        app,
        &format!("/api/v1/doctors/{DOCTOR1_PROFILE_ID}/slots"),
        Some(token),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    Uuid::parse_str(json_body(res).await["data"][0]["id"].as_str().unwrap()).unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn test_deferred_unpriced_doctor_is_listed_and_bookable(pool: PgPool) {
    let (app, pool) = test_app_with_config(pool, deferred_config()).await;
    unprice_doctor1(&pool).await;
    let patient_token = fresh_patient_token(&pool).await;

    let listed = get(&app, "/api/v1/doctors?modality=VIDEO", Some(&patient_token)).await;
    assert_eq!(listed.status(), StatusCode::OK);
    assert!(
        json_body(listed).await["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["id"] == DOCTOR1_PROFILE_ID),
        "deferred mode must not hide an unpriced verified doctor from the VIDEO directory"
    );

    let slot_id = first_bookable_slot(&app, &patient_token).await;
    let res = post(
        &app,
        "/api/v1/appointments/book",
        Some(&patient_token),
        &serde_json::json!({"doctor_id": DOCTOR1_PROFILE_ID, "slot_id": slot_id, "modality": "VIDEO"}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let body = json_body(res).await;
    assert_eq!(body["data"]["status"], "CONFIRMED");
    assert_eq!(body["data"]["payment_policy"], "NOT_REQUIRED");
    let apt_id = Uuid::parse_str(body["data"]["appointment_id"].as_str().unwrap()).unwrap();
    assert!(
        hellodoctor_backend::repository::transaction_repo::find_by_appointment(&pool, apt_id)
            .await
            .unwrap()
            .is_none(),
        "deferred booking must never create a transaction"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_enabled_mode_unpriced_doctor_stays_unlisted_for_video(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    unprice_doctor1(&pool).await;
    let patient_token = fresh_patient_token(&pool).await;
    let listed = get(&app, "/api/v1/doctors?modality=VIDEO", Some(&patient_token)).await;
    assert_eq!(listed.status(), StatusCode::OK);
    assert!(!json_body(listed).await["data"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["id"] == DOCTOR1_PROFILE_ID));
}

#[sqlx::test(migrations = "./migrations")]
async fn test_deferred_booking_rejects_expired_cancelled_and_blocked_slots(pool: PgPool) {
    let (app, pool) = test_app_with_config(pool, deferred_config()).await;
    let patient_token = fresh_patient_token(&pool).await;
    let doctor = Uuid::parse_str(DOCTOR1_PROFILE_ID).unwrap();

    // One slot per non-bookable condition, far from any seeded slot.
    let mut ids = Vec::new();
    for (offset_days, status) in [(-3i64, "AVAILABLE"), (40, "CANCELLED"), (41, "BLOCKED")] {
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO doctor_schedule_slots (id, doctor_id, start_time, end_time, status, created_at) \
             VALUES ($1, $2, now() + make_interval(days => $3::int), now() + make_interval(days => $3::int, mins => 30), $4::slot_status_enum, now())",
        )
        .bind(id)
        .bind(doctor)
        .bind(offset_days as i32)
        .bind(status)
        .execute(&pool)
        .await
        .unwrap();
        ids.push(id);
    }

    // The patient-facing listing never offers any of them...
    let listing = json_body(
        get(
            &app,
            &format!("/api/v1/doctors/{DOCTOR1_PROFILE_ID}/slots"),
            Some(&patient_token),
        )
        .await,
    )
    .await;
    for id in &ids {
        assert!(!listing["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["id"] == id.to_string()));
    }
    // ...and the booking transaction itself rejects them (server-authoritative,
    // not merely hidden by the client).
    for id in ids {
        let res = post(
            &app,
            "/api/v1/appointments/book",
            Some(&patient_token),
            &serde_json::json!({"doctor_id": DOCTOR1_PROFILE_ID, "slot_id": id, "modality": "VIDEO"}),
        )
        .await;
        assert_eq!(res.status(), StatusCode::CONFLICT, "slot {id}");
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn test_deferred_booking_requires_authentication(pool: PgPool) {
    let (app, pool) = test_app_with_config(pool, deferred_config()).await;
    let patient_token = fresh_patient_token(&pool).await;
    let slot_id = first_bookable_slot(&app, &patient_token).await;
    let res = post(
        &app,
        "/api/v1/appointments/book",
        None,
        &serde_json::json!({"doctor_id": DOCTOR1_PROFILE_ID, "slot_id": slot_id, "modality": "VIDEO"}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

// ---------------------------------------------------------------------------
// Payment-free consultation activation (P0-B, 2026-11-13). Before this, the
// chat conversation was created only by the payment webhook, so every
// deferred booking had none: chat consultations could never start and the
// post-video chat was missing. `AppointmentService::ensure_consultation_activated`
// is now shared by the webhook, the deferred booking and the
// participant-authorized repair in `GET /appointments/:id/conversation`.
// ---------------------------------------------------------------------------

async fn book_deferred(app: &axum::Router, token: &str, modality: &str) -> (Uuid, Uuid) {
    let slot_id = first_bookable_slot(app, token).await;
    let res = post_idem(
        app,
        "/api/v1/appointments/book",
        Some(token),
        &serde_json::json!({"doctor_id": DOCTOR1_PROFILE_ID, "slot_id": slot_id, "modality": modality}),
        &Uuid::new_v4().to_string(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let body = json_body(res).await;
    assert_eq!(body["data"]["status"], "CONFIRMED");
    assert_eq!(body["data"]["payment_policy"], "NOT_REQUIRED");
    (
        Uuid::parse_str(body["data"]["appointment_id"].as_str().unwrap()).unwrap(),
        slot_id,
    )
}

/// Fixture-only: returns an appointment to the state of one confirmed before
/// payment-free activation existed (no conversation, no activation trace).
async fn remove_conversation_as_if_legacy(pool: &PgPool, apt_id: Uuid) {
    sqlx::query(
        "DELETE FROM consultation_trace_events WHERE conversation_id IN \
         (SELECT id FROM chat_conversations WHERE appointment_id = $1)",
    )
    .bind(apt_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("DELETE FROM chat_conversations WHERE appointment_id = $1")
        .bind(apt_id)
        .execute(pool)
        .await
        .unwrap();
}

async fn conversation_count(pool: &PgPool, apt_id: Uuid) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM chat_conversations WHERE appointment_id = $1")
        .bind(apt_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn conversation_id_for(app: &axum::Router, token: &str, apt_id: Uuid) -> Uuid {
    let res = get(
        app,
        &format!("/api/v1/appointments/{apt_id}/conversation"),
        Some(token),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    Uuid::parse_str(json_body(res).await["data"]["id"].as_str().unwrap()).unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn test_deferred_chat_booking_activates_conversation_and_both_parties_exchange_messages(
    pool: PgPool,
) {
    let (app, pool) = test_app_with_config(pool, deferred_config()).await;
    unprice_doctor1(&pool).await;
    let patient = fresh_patient_token(&pool).await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");

    // A zero-fee verified doctor is discoverable for CHAT in deferred mode.
    let listed = get(&app, "/api/v1/doctors?modality=CHAT", Some(&patient)).await;
    assert!(json_body(listed).await["data"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["id"] == DOCTOR1_PROFILE_ID));

    let (apt_id, slot_id) = book_deferred(&app, &patient, "CHAT").await;
    assert_eq!(conversation_count(&pool, apt_id).await, 1);

    // The messaging window outlives the booked slot.
    let (expires_at, slot_end): (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>) =
        sqlx::query_as(
            "SELECT c.expires_at, s.end_time FROM chat_conversations c, doctor_schedule_slots s \
             WHERE c.appointment_id = $1 AND s.id = $2",
        )
        .bind(apt_id)
        .bind(slot_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(expires_at >= slot_end + chrono::Duration::hours(24) - chrono::Duration::seconds(5));

    let conv = conversation_id_for(&app, &patient, apt_id).await;
    assert_eq!(conversation_id_for(&app, &doctor, apt_id).await, conv);

    for (token, text) in [(&patient, "Patient hello"), (&doctor, "Doctor reply")] {
        let res = post_idem(
            &app,
            &format!("/api/v1/chat/{conv}/messages"),
            Some(token),
            &serde_json::json!({"content": text}),
            &Uuid::new_v4().to_string(),
        )
        .await;
        assert!(res.status().is_success(), "send failed: {}", res.status());
    }
    for token in [&patient, &doctor] {
        let res = get(&app, &format!("/api/v1/chat/{conv}/messages"), Some(token)).await;
        assert_eq!(res.status(), StatusCode::OK);
        let messages = json_body(res).await["data"].as_array().unwrap().clone();
        let contents: Vec<&str> = messages
            .iter()
            .filter_map(|m| m["content"].as_str())
            .collect();
        assert!(contents.contains(&"Patient hello") && contents.contains(&"Doctor reply"));
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn test_deferred_conversation_is_not_reachable_by_unrelated_accounts(pool: PgPool) {
    let (app, pool) = test_app_with_config(pool, deferred_config()).await;
    let patient = fresh_patient_token(&pool).await;
    let (apt_id, _) = book_deferred(&app, &patient, "CHAT").await;
    let conv = conversation_id_for(&app, &patient, apt_id).await;

    let stranger = fresh_patient_token(&pool).await;
    let other_doctor = make_token(uid(DOCTOR2_USER_ID), "DOCTOR");
    for token in [&stranger, &other_doctor] {
        let lookup = get(
            &app,
            &format!("/api/v1/appointments/{apt_id}/conversation"),
            Some(token),
        )
        .await;
        assert_eq!(lookup.status(), StatusCode::NOT_FOUND);
        let read = get(&app, &format!("/api/v1/chat/{conv}/messages"), Some(token)).await;
        assert!(
            !read.status().is_success(),
            "unrelated account read the thread"
        );
        let send = post_idem(
            &app,
            &format!("/api/v1/chat/{conv}/messages"),
            Some(token),
            &serde_json::json!({"content": "intrusion"}),
            &Uuid::new_v4().to_string(),
        )
        .await;
        assert!(
            !send.status().is_success(),
            "unrelated account posted into the thread"
        );
    }
    let intrusions: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM chat_messages WHERE conversation_id = $1")
            .bind(conv)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(intrusions, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_deferred_video_booking_gets_session_and_post_video_conversation(pool: PgPool) {
    let (app, pool) = test_app_with_config(pool, deferred_config()).await;
    let patient = fresh_patient_token(&pool).await;
    let (apt_id, _) = book_deferred(&app, &patient, "VIDEO").await;
    assert_eq!(conversation_count(&pool, apt_id).await, 1);
    let sessions: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM consultation_sessions WHERE appointment_id = $1")
            .bind(apt_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(sessions, 1);
    let conv = conversation_id_for(&app, &patient, apt_id).await;
    let send = post_idem(
        &app,
        &format!("/api/v1/chat/{conv}/messages"),
        Some(&patient),
        &serde_json::json!({"content": "Follow-up question after the call"}),
        &Uuid::new_v4().to_string(),
    )
    .await;
    assert!(send.status().is_success());
}

#[sqlx::test(migrations = "./migrations")]
async fn test_conversation_activation_is_idempotent_and_repairs_missing_conversations(
    pool: PgPool,
) {
    let (app, pool, state) = test_app_and_state_with_config(pool, deferred_config()).await;
    let patient = fresh_patient_token(&pool).await;
    let (apt_id, _) = book_deferred(&app, &patient, "CHAT").await;

    // Simulate an appointment confirmed before payment-free activation existed.
    remove_conversation_as_if_legacy(&pool, apt_id).await;

    // An unrelated account cannot trigger activation through this appointment.
    let stranger = fresh_patient_token(&pool).await;
    let res = get(
        &app,
        &format!("/api/v1/appointments/{apt_id}/conversation"),
        Some(&stranger),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(conversation_count(&pool, apt_id).await, 0);

    // Concurrent activation converges on a single conversation.
    let (a, b, c) = tokio::join!(
        hellodoctor_backend::services::AppointmentService::ensure_consultation_activated(
            &state, apt_id
        ),
        hellodoctor_backend::services::AppointmentService::ensure_consultation_activated(
            &state, apt_id
        ),
        hellodoctor_backend::services::AppointmentService::ensure_consultation_activated(
            &state, apt_id
        ),
    );
    a.unwrap();
    b.unwrap();
    c.unwrap();
    assert_eq!(conversation_count(&pool, apt_id).await, 1);

    // The participant's lookup repairs it if missing, and is stable afterwards.
    remove_conversation_as_if_legacy(&pool, apt_id).await;
    let first = conversation_id_for(&app, &patient, apt_id).await;
    let second = conversation_id_for(&app, &patient, apt_id).await;
    assert_eq!(first, second);
    assert_eq!(conversation_count(&pool, apt_id).await, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_enabled_mode_unpriced_doctor_stays_unlisted_for_chat(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    unprice_doctor1(&pool).await;
    let patient = fresh_patient_token(&pool).await;
    let listed = get(&app, "/api/v1/doctors?modality=CHAT", Some(&patient)).await;
    assert_eq!(listed.status(), StatusCode::OK);
    assert!(!json_body(listed).await["data"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["id"] == DOCTOR1_PROFILE_ID));
}

#[sqlx::test(migrations = "./migrations")]
async fn test_appointment_detail_carries_routing_fields_for_owner_only(pool: PgPool) {
    let (app, pool) = test_app_with_config(pool, deferred_config()).await;
    let patient = fresh_patient_token(&pool).await;
    let (apt_id, slot_id) = book_deferred(&app, &patient, "CHAT").await;

    let res = get(
        &app,
        &format!("/api/v1/appointments/{apt_id}"),
        Some(&patient),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let data = json_body(res).await["data"].clone();
    assert_eq!(data["status"], "CONFIRMED");
    assert_eq!(data["modality"], "CHAT");
    assert!(data["doctor_display_name"]
        .as_str()
        .is_some_and(|s| !s.is_empty()));
    assert!(data["primary_specialty"].is_string());
    let slot_start: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT start_time FROM doctor_schedule_slots WHERE id = $1")
            .bind(slot_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let returned = chrono::DateTime::parse_from_rfc3339(data["start_time"].as_str().unwrap())
        .unwrap()
        .with_timezone(&chrono::Utc);
    assert_eq!(returned, slot_start);
    // Chat has no RTC room: the server's own join policy says so.
    assert_eq!(data["join"]["reason"], "NOT_VIDEO");
    assert!(data["join"]["server_time"].is_string());

    let stranger = fresh_patient_token(&pool).await;
    let res = get(
        &app,
        &format!("/api/v1/appointments/{apt_id}"),
        Some(&stranger),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

/// With payments deferred the booking still emails both sides: the patient's
/// confirmation and the Doctor's "new consultation booked". The Doctor's
/// email shows the time and reference, never the complaint or patient name.
#[sqlx::test(migrations = "./migrations")]
async fn test_deferred_booking_emails_the_patient_and_the_doctor(pool: PgPool) {
    let (_, pool, mut state) = test_app_and_state_with_config(pool, deferred_config()).await;
    let transport = capture_transport();
    state.email_transport = transport.clone();
    let app = hellodoctor_backend::api::create_router(state);
    let patient_token = fresh_patient_token(&pool).await;
    sqlx::query(
        "UPDATE users SET email = 'deferred.patient@example.test' \
          WHERE id = (SELECT user_id FROM patient_profiles WHERE full_name = 'Race Test Patient' \
                      ORDER BY created_at DESC LIMIT 1)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let doctor_email: String = sqlx::query_scalar(
        "SELECT u.email FROM doctor_profiles d JOIN users u ON u.id = d.user_id WHERE d.id = $1",
    )
    .bind(uid(DOCTOR1_PROFILE_ID))
    .fetch_one(&pool)
    .await
    .unwrap();
    let slot_id = first_bookable_slot(&app, &patient_token).await;
    let booked = post_idem(
        &app,
        "/api/v1/appointments/book",
        Some(&patient_token),
        &serde_json::json!({
            "doctor_id": DOCTOR1_PROFILE_ID,
            "slot_id": slot_id,
            "modality": "VIDEO",
            "chief_complaint": "Persistent cough at night"
        }),
        &Uuid::new_v4().to_string(),
    )
    .await;
    assert_eq!(booked.status(), StatusCode::CREATED);
    wait_for_sent_emails(
        &pool,
        &["CONSULTATION_CONFIRMED", "CONSULTATION_BOOKED_DOCTOR"],
    )
    .await;

    let mut queued: Vec<(String, String)> = sqlx::query_as(
        "SELECT email_type, recipient_email FROM email_outbox \
          WHERE email_type IN ('CONSULTATION_CONFIRMED', 'CONSULTATION_BOOKED_DOCTOR')",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    queued.sort();
    assert_eq!(
        queued,
        vec![
            (
                "CONSULTATION_BOOKED_DOCTOR".to_string(),
                doctor_email.clone()
            ),
            (
                "CONSULTATION_CONFIRMED".to_string(),
                "deferred.patient@example.test".to_string()
            ),
        ]
    );
    let sent = transport.sent.lock().unwrap().clone();
    let doctor_mail = sent
        .iter()
        .find(|m| m.0 == doctor_email)
        .expect("the Doctor's email was sent");
    assert_eq!(
        doctor_mail.1,
        "A new consultation has been booked with you."
    );
    for body in [&doctor_mail.2, &doctor_mail.3] {
        assert!(
            !body.contains("cough"),
            "no complaint in the Doctor's email"
        );
        assert!(
            !body.contains("Race Test Patient"),
            "no patient name in the Doctor's email"
        );
        assert!(body.contains("Video consultation") && body.contains("EAT"));
    }
    assert!(sent
        .iter()
        .any(|m| m.0 == "deferred.patient@example.test"
            && m.1 == "You're booked! Your care, your way."));
}
