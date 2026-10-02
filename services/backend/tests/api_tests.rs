use axum::body::Body;
use axum::http::{Request, StatusCode};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use hellodoctor_backend::api::create_router;
use hellodoctor_backend::config::AppConfig;
use hellodoctor_backend::domain::models::*;
use hellodoctor_backend::repository::{appointment_repo, consultation_repo, doctor_repo, AppState};
use hellodoctor_backend::seed::{self, DOCTOR1_PROFILE_ID, PATIENT_PROFILE_ID, SEED_ADMIN_EMAIL, SEED_ADMIN_PASSWORD, SEED_ADMIN_TOTP_SECRET_BASE32, SEED_DOCTOR1_LICENSE, SEED_DOCTOR1_PASSWORD, SEED_PATIENT_PHONE};
use http_body_util::BodyExt;
use serde_json::Value;
use sqlx::PgPool;
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

fn test_config() -> AppConfig {
    AppConfig {
        port: 0,
        host: "127.0.0.1".into(),
        database_url: String::new(),
        jwt_ed25519_private_key_b64: "MC4CAQAwBQYDK2VwBCIEIAT/WOtPd4fQqc9E/PQBXq2WXSXcAKdcGsvGqg7reaCS".into(),
        jwt_ed25519_public_key_b64: "MCowBQYDK2VwAyEAKioTLvfEFI4gtalwnRy5/Y2u+Q4Wdz1sSYldnwk1Ubs=".into(),
        totp_encryption_key_b64: "zjw5FesMAwveYp3JKXqYJhbZ4O1s2yHgTAp9xgvfqq0=".into(),
        agora_app_id: "73ddf1abee12ac89822eecb0e7df9e28".into(),
        agora_app_certificate: "12a7c64e686cb880ee5cb57ac9eed39d".into(),
        storage_bucket: "helodoc-medical-records".into(),
        environment: "development".into(),
        allowed_origins: vec!["http://localhost:3000".into()],
    }
}

async fn test_app(pool: PgPool) -> (axum::Router, PgPool) {
    hellodoctor_backend::telemetry::init_tracing();
    seed::seed_dev_data(&pool).await.expect("seed dev data");
    let state = AppState::new(pool.clone(), Arc::new(test_config()));
    (create_router(state), pool)
}

fn uid(s: &str) -> Uuid {
    Uuid::parse_str(s).unwrap()
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

async fn json_body(res: axum::http::Response<Body>) -> Value {
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

/// Books an appointment with seeded doctor 1 and confirms payment via webhook,
/// returning (appointment_id, payment_session_id).
async fn book_and_confirm(app: &axum::Router) -> (Uuid, Uuid) {
    let req_slots = Request::builder()
        .uri(format!("/api/v1/doctors/{}/slots", DOCTOR1_PROFILE_ID))
        .body(Body::empty())
        .unwrap();
    let res_slots = app.clone().oneshot(req_slots).await.unwrap();
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

    let req_book = Request::builder()
        .method("POST")
        .uri("/api/v1/appointments/book")
        .header("Content-Type", "application/json")
        .header("Idempotency-Key", Uuid::new_v4().to_string())
        .body(Body::from(serde_json::to_string(&book_payload).unwrap()))
        .unwrap();
    let res_book = app.clone().oneshot(req_book).await.unwrap();
    assert_eq!(res_book.status(), StatusCode::CREATED);
    let book_json = json_body(res_book).await;
    let apt_id = Uuid::parse_str(book_json["data"]["appointment_id"].as_str().unwrap()).unwrap();
    let payment_sess_id = Uuid::parse_str(book_json["data"]["payment_session_id"].as_str().unwrap()).unwrap();

    let webhook_payload = serde_json::json!({
        "payment_session_id": payment_sess_id,
        "transaction_reference": format!("BK-TEST-{}", Uuid::new_v4()),
        "status": "SUCCESS"
    });
    let req_webhook = Request::builder()
        .method("POST")
        .uri("/api/v1/webhooks/payment/bkash")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&webhook_payload).unwrap()))
        .unwrap();
    let res_webhook = app.clone().oneshot(req_webhook).await.unwrap();
    assert_eq!(res_webhook.status(), StatusCode::OK);

    (apt_id, payment_sess_id)
}

#[sqlx::test(migrations = "./migrations")]
async fn test_health_check(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let response = app.oneshot(Request::builder().uri("/healthz").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_patient_otp_auth_flow_and_token_refresh(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/register-otp")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&serde_json::json!({ "phone_number": SEED_PATIENT_PHONE })).unwrap()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json = json_body(res).await;
    let real_otp = json["data"]["debug_otp_code"].as_str().unwrap().to_string();

    // A wrong OTP must now be genuinely rejected — no more universal magic-code bypass.
    let req_wrong = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/verify-otp-and-login")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&serde_json::json!({ "phone_number": SEED_PATIENT_PHONE, "otp_code": "000000" })).unwrap()))
        .unwrap();
    let res_wrong = app.clone().oneshot(req_wrong).await.unwrap();
    assert_eq!(res_wrong.status(), StatusCode::UNAUTHORIZED);

    let req2 = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/verify-otp-and-login")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&serde_json::json!({ "phone_number": SEED_PATIENT_PHONE, "otp_code": real_otp })).unwrap()))
        .unwrap();
    let res2 = app.clone().oneshot(req2).await.unwrap();
    assert_eq!(res2.status(), StatusCode::OK);

    let json2 = json_body(res2).await;
    assert!(json2["success"].as_bool().unwrap());
    let access_token = json2["data"]["access_token"].as_str().unwrap();
    let refresh_token = json2["data"]["refresh_token"].as_str().unwrap();
    assert!(!access_token.is_empty());
    assert!(!refresh_token.is_empty());

    let req3 = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/refresh")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&serde_json::json!({ "refresh_token": refresh_token })).unwrap()))
        .unwrap();
    let res3 = app.clone().oneshot(req3).await.unwrap();
    assert_eq!(res3.status(), StatusCode::OK);

    // Reuse of the already-rotated refresh token must be rejected.
    let req4 = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/refresh")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&serde_json::json!({ "refresh_token": refresh_token })).unwrap()))
        .unwrap();
    let res4 = app.clone().oneshot(req4).await.unwrap();
    assert_eq!(res4.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_doctor_discovery_and_slot_booking_lifecycle(pool: PgPool) {
    let (app, pool) = test_app(pool).await;

    let req = Request::builder().uri("/api/v1/doctors?specialty=Internal%20Medicine").body(Body::empty()).unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json = json_body(res).await;
    let doc_id = Uuid::parse_str(json["data"][0]["id"].as_str().unwrap()).unwrap();
    assert_eq!(doc_id, uid(DOCTOR1_PROFILE_ID));

    let (apt_id, _payment_sess_id) = book_and_confirm(&app).await;

    let apt = appointment_repo::find_by_id(&pool, apt_id).await.unwrap().unwrap();
    assert_eq!(apt.status, AppointmentStatus::Confirmed);

    // Collision: the same slot is now BOOKED, a second booking attempt on it must 409.
    let book_payload_again = serde_json::json!({
        "doctor_id": DOCTOR1_PROFILE_ID,
        "slot_id": apt.slot_id,
        "modality": "VIDEO",
        "gateway": "BKASH",
        "chief_complaint": "Follow-up"
    });
    let req_collision = Request::builder()
        .method("POST")
        .uri("/api/v1/appointments/book")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&book_payload_again).unwrap()))
        .unwrap();
    let res_collision = app.clone().oneshot(req_collision).await.unwrap();
    assert_eq!(res_collision.status(), StatusCode::CONFLICT);

    let txn = hellodoctor_backend::repository::transaction_repo::find_by_appointment(&pool, apt_id).await.unwrap().unwrap();
    assert_eq!(txn.payment_status, PaymentStatus::PaymentHeld);
    assert_eq!(txn.gross_amount, bigdecimal::BigDecimal::from(800));
    assert_eq!(txn.platform_fee_amount, bigdecimal::BigDecimal::from(160));
    assert_eq!(txn.net_amount, bigdecimal::BigDecimal::from(640));

    let req_rtc = Request::builder().method("POST").uri(format!("/api/v1/consultations/{}/rtc-token", apt_id)).body(Body::empty()).unwrap();
    let res_rtc = app.clone().oneshot(req_rtc).await.unwrap();
    assert_eq!(res_rtc.status(), StatusCode::OK);
    let rtc_json = json_body(res_rtc).await;
    assert!(rtc_json["data"]["channel_name"].as_str().unwrap().starts_with("rtc_"));

    let rx_payload = serde_json::json!({
        "base64_photo": minimal_jpeg_base64(),
        "doctor_notes": "Paracetamol 500mg 1+0+1 for 3 days after food"
    });
    let req_rx = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/consultations/{}/prescription", apt_id))
        .header("Content-Type", "application/json")
        .header("Idempotency-Key", Uuid::new_v4().to_string())
        .body(Body::from(serde_json::to_string(&rx_payload).unwrap()))
        .unwrap();
    let res_rx = app.clone().oneshot(req_rx).await.unwrap();
    assert_eq!(res_rx.status(), StatusCode::CREATED);

    let comp_payload = serde_json::json!({ "outcome": "COMPLETED_WITH_RX" });
    let req_comp = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/consultations/{}/complete", apt_id))
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&comp_payload).unwrap()))
        .unwrap();
    let res_comp = app.clone().oneshot(req_comp).await.unwrap();
    assert_eq!(res_comp.status(), StatusCode::OK);

    let req_wallet = Request::builder().uri("/api/v1/doctor/wallet/summary").body(Body::empty()).unwrap();
    let res_wallet = app.clone().oneshot(req_wallet).await.unwrap();
    assert_eq!(res_wallet.status(), StatusCode::OK);
    let wallet_json = json_body(res_wallet).await;
    assert_money_eq(&wallet_json["data"]["total_gross"], "800");
    assert_money_eq(&wallet_json["data"]["withheld_fee"], "160.00");
    assert_money_eq(&wallet_json["data"]["final_net"], "640.00");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_multi_prescription_intake_and_5_photo_limit(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let (apt_id, _) = book_and_confirm(&app).await;

    let three_files: Vec<_> = (0..3)
        .map(|_| serde_json::json!({ "mime_type": "image/jpeg", "base64_content": minimal_jpeg_base64() }))
        .collect();
    let upload_3 = serde_json::json!({ "files": three_files });

    let req1 = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/appointments/{}/prescriptions", apt_id))
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&upload_3).unwrap()))
        .unwrap();
    let res1 = app.clone().oneshot(req1).await.unwrap();
    assert_eq!(res1.status(), StatusCode::CREATED);

    let three_more: Vec<_> = (0..3)
        .map(|_| serde_json::json!({ "mime_type": "image/jpeg", "base64_content": minimal_jpeg_base64() }))
        .collect();
    let upload_more = serde_json::json!({ "files": three_more });

    let req2 = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/appointments/{}/prescriptions", apt_id))
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&upload_more).unwrap()))
        .unwrap();
    let res2 = app.clone().oneshot(req2).await.unwrap();
    assert_eq!(res2.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_clinical_grievance_and_board_refund_adjudication(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let (apt_id, _) = book_and_confirm(&app).await;

    let session = consultation_repo::find_by_appointment(&pool, apt_id).await.unwrap().unwrap();

    let grv_payload = serde_json::json!({
        "consultation_id": session.id,
        "target": "DOCTOR",
        "category": "Rushed Consultation / Ended Abruptly",
        "claim_summary": "Doctor disconnected after 45 seconds"
    });
    let req_grv = Request::builder()
        .method("POST")
        .uri("/api/v1/grievances")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&grv_payload).unwrap()))
        .unwrap();
    let res_grv = app.clone().oneshot(req_grv).await.unwrap();
    assert_eq!(res_grv.status(), StatusCode::CREATED);
    let json = json_body(res_grv).await;
    let grv_id = Uuid::parse_str(json["data"]["grievance_id"].as_str().unwrap()).unwrap();

    let adj_payload = serde_json::json!({ "audit_notes": "Call telemetry confirmed 45s duration; claim substantiated." });
    let req_adj = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/admin/grievances/{}/refund", grv_id))
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&adj_payload).unwrap()))
        .unwrap();
    let res_adj = app.clone().oneshot(req_adj).await.unwrap();
    assert_eq!(res_adj.status(), StatusCode::OK);
    let adj_json = json_body(res_adj).await;
    assert_eq!(adj_json["data"]["status"].as_str().unwrap(), "REFUNDED");

    let txn = hellodoctor_backend::repository::transaction_repo::find_by_appointment(&pool, apt_id).await.unwrap().unwrap();
    assert_eq!(txn.payment_status, PaymentStatus::RefundedToPatient);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_grievance_dismiss(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let (apt_id, _) = book_and_confirm(&app).await;
    let session = consultation_repo::find_by_appointment(&pool, apt_id).await.unwrap().unwrap();

    let grv_payload = serde_json::json!({
        "consultation_id": session.id,
        "target": "SYSTEM",
        "category": "App crashed mid-call",
        "claim_summary": "Lost connection, unclear if doctor's fault"
    });
    let req_grv = Request::builder()
        .method("POST")
        .uri("/api/v1/grievances")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&grv_payload).unwrap()))
        .unwrap();
    let res_grv = app.clone().oneshot(req_grv).await.unwrap();
    let json = json_body(res_grv).await;
    let grv_id = Uuid::parse_str(json["data"]["grievance_id"].as_str().unwrap()).unwrap();

    let req_dismiss = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/admin/grievances/{}/dismiss", grv_id))
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&serde_json::json!({ "audit_notes": "Telemetry inconclusive; no fault found." })).unwrap()))
        .unwrap();
    let res_dismiss = app.clone().oneshot(req_dismiss).await.unwrap();
    assert_eq!(res_dismiss.status(), StatusCode::OK);
    let dismiss_json = json_body(res_dismiss).await;
    assert_eq!(dismiss_json["data"]["status"].as_str().unwrap(), "DISMISSED");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_monthly_finance_disbursement_batch(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let (apt_id, _) = book_and_confirm(&app).await;

    let comp_payload = serde_json::json!({ "outcome": "COMPLETED_NO_RX" });
    let req_comp = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/consultations/{}/complete-no-rx", apt_id))
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&serde_json::json!({ "reason": "Routine follow-up" })).unwrap()))
        .unwrap();
    // This appointment already has a settled PAYMENT_HELD transaction from book_and_confirm;
    // settle it to the wallet via the outcome-completion path used elsewhere.
    let _ = req_comp; // complete-no-rx only records the prescription outcome, not payment settlement.

    let comp2 = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/consultations/{}/complete", apt_id))
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&comp_payload).unwrap()))
        .unwrap();
    let res_comp2 = app.clone().oneshot(comp2).await.unwrap();
    assert_eq!(res_comp2.status(), StatusCode::OK);

    let disb_payload = serde_json::json!({ "period_start": "2026-09-01", "period_end": "2026-09-30" });
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/admin/disbursements/initiate")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&disb_payload).unwrap()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    let json = json_body(res).await;
    assert_eq!(json["data"]["total_doctors"].as_i64().unwrap(), 1);
    assert_money_eq(&json["data"]["total_net_disbursed"], "640.00");
    assert_money_eq(&json["data"]["total_platform_fee"], "160.00");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_doctor_mfa_login_two_step_verification(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;

    let req1 = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/doctor/login")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&serde_json::json!({ "license_number": SEED_DOCTOR1_LICENSE, "password": SEED_DOCTOR1_PASSWORD })).unwrap()))
        .unwrap();
    let res1 = app.clone().oneshot(req1).await.unwrap();
    assert_eq!(res1.status(), StatusCode::OK);
    let json1 = json_body(res1).await;
    let session_token = json1["data"]["session_token"].as_str().unwrap().to_string();
    let debug_code = json1["data"]["debug_otp_code"].as_str().unwrap().to_string();

    // Wrong password must be rejected (real Argon2id verification, not bypassed).
    let req_wrong = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/doctor/login")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&serde_json::json!({ "license_number": SEED_DOCTOR1_LICENSE, "password": "wrong-password" })).unwrap()))
        .unwrap();
    let res_wrong = app.clone().oneshot(req_wrong).await.unwrap();
    assert_eq!(res_wrong.status(), StatusCode::UNAUTHORIZED);

    let req2 = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/doctor/verify-otp")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&serde_json::json!({ "session_token": session_token, "otp_code": debug_code })).unwrap()))
        .unwrap();
    let res2 = app.clone().oneshot(req2).await.unwrap();
    assert_eq!(res2.status(), StatusCode::OK);
    let json2 = json_body(res2).await;
    assert_eq!(json2["data"]["user"]["role"].as_str().unwrap(), "DOCTOR");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_admin_login_with_real_totp(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;

    let totp = hellodoctor_backend::crypto::build_totp(SEED_ADMIN_TOTP_SECRET_BASE32).unwrap();
    let code = totp.generate_current().unwrap();

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/admin/login")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&serde_json::json!({ "email": SEED_ADMIN_EMAIL, "password": SEED_ADMIN_PASSWORD, "totp_code": code })).unwrap()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json = json_body(res).await;
    assert_eq!(json["data"]["user"]["role"].as_str().unwrap(), "PLATFORM_ADMIN");

    // A stale/incorrect TOTP code must be rejected.
    let req_bad = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/admin/login")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&serde_json::json!({ "email": SEED_ADMIN_EMAIL, "password": SEED_ADMIN_PASSWORD, "totp_code": "000000" })).unwrap()))
        .unwrap();
    let res_bad = app.clone().oneshot(req_bad).await.unwrap();
    assert_eq!(res_bad.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_24h_clinical_chat_messaging_and_expiration_locking(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let (apt_id, _) = book_and_confirm(&app).await;

    let apt = appointment_repo::find_by_id(&pool, apt_id).await.unwrap().unwrap();
    let _ = apt;
    let conv = sqlx::query_as::<_, (Uuid,)>("SELECT id FROM chat_conversations WHERE appointment_id = $1")
        .bind(apt_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let conv_id = conv.0;

    let msg_payload = serde_json::json!({ "content": "Doctor, can I take paracetamol after food?" });
    let req1 = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/chat/{}/messages", conv_id))
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&msg_payload).unwrap()))
        .unwrap();
    let res1 = app.clone().oneshot(req1).await.unwrap();
    assert_eq!(res1.status(), StatusCode::CREATED);

    sqlx::query("UPDATE chat_conversations SET expires_at = now() - interval '10 seconds' WHERE id = $1")
        .bind(conv_id)
        .execute(&pool)
        .await
        .unwrap();

    let req2 = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/chat/{}/messages", conv_id))
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&msg_payload).unwrap()))
        .unwrap();
    let res2 = app.clone().oneshot(req2).await.unwrap();
    assert_eq!(res2.status(), StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_admin_create_doctor_issues_temporary_password(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;

    let payload = serde_json::json!({
        "full_name": "Dr. Tanvir Hasan",
        "license_number": "BMDC #A-77102",
        "primary_specialty": "Dermatology",
        "experience_years": 6,
        "current_hospital": "Square Hospital",
        "verified_phone": "+880 1611-223344",
        "consultation_fee_video": 700,
        "consultation_fee_chat": 450
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/admin/doctors")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&payload).unwrap()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    let json = json_body(res).await;
    let temp_password = json["data"]["temporary_password"].as_str().unwrap();
    assert_eq!(temp_password.len(), 10);
    assert!(temp_password.chars().all(|c| c.is_ascii_digit()));
}

#[sqlx::test(migrations = "./migrations")]
async fn test_complete_consultation_without_prescription(pool: PgPool) {
    let (app, pool) = test_app(pool).await;

    let doctor_id = uid(DOCTOR1_PROFILE_ID);
    let slot = doctor_repo::list_slots_for_doctor(&pool, doctor_id).await.unwrap().into_iter().next().unwrap();

    let apt_id = Uuid::new_v4();
    appointment_repo::insert(
        &pool,
        &Appointment {
            id: apt_id,
            appointment_number: "APT-TEST-NO-RX".into(),
            patient_id: uid(PATIENT_PROFILE_ID),
            doctor_id,
            slot_id: slot.id,
            modality: Modality::Video,
            status: AppointmentStatus::InConsultation,
            consultation_fee: bigdecimal::BigDecimal::from(500),
            clinical_outcome: None,
            completed_at: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        },
    )
    .await
    .unwrap();

    let no_rx_payload = serde_json::json!({ "reason": "Follow-up consultation — patient recovering well, lifestyle advice provided" });
    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/consultations/{}/complete-no-rx", apt_id))
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&no_rx_payload).unwrap()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let rx = hellodoctor_backend::repository::prescription_repo::find_prescription_by_appointment(&pool, apt_id)
        .await
        .unwrap()
        .unwrap();
    assert!(rx.is_no_rx_required);
    assert_eq!(rx.no_rx_reason.unwrap(), "Follow-up consultation — patient recovering well, lifestyle advice provided");
}
