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
