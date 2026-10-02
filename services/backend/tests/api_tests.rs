use axum::body::Body;
use axum::http::{Request, StatusCode};
use hellodoctor_backend::api::create_router;
use hellodoctor_backend::domain::models::*;
use hellodoctor_backend::repository::AppState;
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;
use uuid::Uuid;

fn test_app() -> (axum::Router, AppState) {
    let state = AppState::new_with_seeds();
    let app = create_router(state.clone());
    (app, state)
}

#[tokio::test]
async fn test_health_check() {
    let (app, _) = test_app();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_patient_otp_auth_flow_and_token_refresh() {
    let (app, _) = test_app();

    // 1. Request OTP
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/register-otp")
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({
                "phone_number": "+8801712345678"
            }))
            .unwrap(),
        ))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 2. Verify OTP and login
    let req2 = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/verify-otp-and-login")
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({
                "phone_number": "+8801712345678",
                "otp_code": "584920"
            }))
            .unwrap(),
        ))
        .unwrap();

    let res2 = app.clone().oneshot(req2).await.unwrap();
    assert_eq!(res2.status(), StatusCode::OK);

    let body_bytes = res2.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert!(json["success"].as_bool().unwrap());
    let access_token = json["data"]["access_token"].as_str().unwrap();
    let refresh_token = json["data"]["refresh_token"].as_str().unwrap();
    assert!(!access_token.is_empty());
    assert!(!refresh_token.is_empty());

    // 3. Refresh Token Rotation
    let req3 = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/refresh")
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({
                "refresh_token": refresh_token
            }))
            .unwrap(),
        ))
        .unwrap();

    let res3 = app.clone().oneshot(req3).await.unwrap();
    assert_eq!(res3.status(), StatusCode::OK);

    // 4. Token Reuse Detection Test: Presenting old rotated token must be rejected
    let req4 = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/refresh")
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({
                "refresh_token": refresh_token
            }))
            .unwrap(),
        ))
        .unwrap();

    let res4 = app.clone().oneshot(req4).await.unwrap();
    assert_eq!(res4.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_doctor_discovery_and_slot_booking_lifecycle() {
    let (app, state) = test_app();

    // 1. Get doctor list
    let req = Request::builder()
        .uri("/api/v1/doctors?specialty=Internal%20Medicine")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();
    let doc_id_str = json["data"][0]["id"].as_str().unwrap();
    let doc_id = Uuid::parse_str(doc_id_str).unwrap();

    // 2. Get available slots
    let req_slots = Request::builder()
        .uri(format!("/api/v1/doctors/{}/slots", doc_id))
        .body(Body::empty())
        .unwrap();
    let res_slots = app.clone().oneshot(req_slots).await.unwrap();
    assert_eq!(res_slots.status(), StatusCode::OK);

    let body_bytes = res_slots.into_body().collect().await.unwrap().to_bytes();
    let json_slots: Value = serde_json::from_slice(&body_bytes).unwrap();
    let slot_id_str = json_slots["data"][0]["id"].as_str().unwrap();
    let slot_id = Uuid::parse_str(slot_id_str).unwrap();

    // 3. Atomically Book Slot (Stage 1: PENDING_PAYMENT & LOCKED_IN_PAYMENT)
    let book_payload = serde_json::json!({
        "doctor_id": doc_id,
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

    let body_bytes = res_book.into_body().collect().await.unwrap().to_bytes();
    let book_json: Value = serde_json::from_slice(&body_bytes).unwrap();
    let apt_id_str = book_json["data"]["appointment_id"].as_str().unwrap();
    let apt_id = Uuid::parse_str(apt_id_str).unwrap();
    let payment_sess_str = book_json["data"]["payment_session_id"].as_str().unwrap();
    let payment_sess_id = Uuid::parse_str(payment_sess_str).unwrap();

    assert_eq!(book_json["data"]["status"].as_str().unwrap(), "PENDING_PAYMENT");

    // 4. Verify Collision / Race Condition Protection: Second booking for same slot must 409 Conflict
    let req_collision = Request::builder()
        .method("POST")
        .uri("/api/v1/appointments/book")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&book_payload).unwrap()))
        .unwrap();

    let res_collision = app.clone().oneshot(req_collision).await.unwrap();
    assert_eq!(res_collision.status(), StatusCode::CONFLICT);

    // 5. Payment Webhook Callback (Stage 2: CONFIRMED & PAYMENT_HELD)
    let webhook_payload = serde_json::json!({
        "payment_session_id": payment_sess_id,
        "transaction_reference": "BK-TEST-TXN-99881",
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

    // 6. Verify appointment is now CONFIRMED
    let apt = state.appointments.read().get(&apt_id).unwrap().clone();
    assert_eq!(apt.status, AppointmentStatus::Confirmed);

    // 7. Verify transaction is PAYMENT_HELD with exact 20% platform charge math
    let txn = state
        .transactions
        .read()
        .values()
        .find(|t| t.appointment_id == apt_id)
        .unwrap()
        .clone();
    assert_eq!(txn.payment_status, PaymentStatus::PaymentHeld);
    assert_eq!(txn.gross_amount, bigdecimal::BigDecimal::from(800));
    assert_eq!(txn.platform_fee_amount, bigdecimal::BigDecimal::from(160)); // 20%
    assert_eq!(txn.net_amount, bigdecimal::BigDecimal::from(640)); // 80%

    // 8. Mint Agora Video RTC Token
    let req_rtc = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/consultations/{}/rtc-token", apt_id))
        .body(Body::empty())
        .unwrap();

    let res_rtc = app.clone().oneshot(req_rtc).await.unwrap();
    assert_eq!(res_rtc.status(), StatusCode::OK);

    let body_bytes = res_rtc.into_body().collect().await.unwrap().to_bytes();
    let rtc_json: Value = serde_json::from_slice(&body_bytes).unwrap();
    let channel_name = rtc_json["data"]["channel_name"].as_str().unwrap();
    assert!(channel_name.starts_with("rtc_"));

    // 9. Doctor Uploads Handwritten Prescription Photo
    let rx_payload = serde_json::json!({
        "base64_photo": "/9j/4AAQSkZJRg==",
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

    // 10. Complete Consultation and Settle Funds to Doctor Wallet
    let comp_payload = serde_json::json!({
        "outcome": "COMPLETED_WITH_RX"
    });

    let req_comp = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/consultations/{}/complete", apt_id))
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&comp_payload).unwrap()))
        .unwrap();

    let res_comp = app.clone().oneshot(req_comp).await.unwrap();
    assert_eq!(res_comp.status(), StatusCode::OK);

    // 11. Check Doctor Wallet: Withheld 20% platform charge and settled net
    let req_wallet = Request::builder()
        .uri("/api/v1/doctor/wallet/summary")
        .body(Body::empty())
        .unwrap();

    let res_wallet = app.clone().oneshot(req_wallet).await.unwrap();
    assert_eq!(res_wallet.status(), StatusCode::OK);

    let body_bytes = res_wallet.into_body().collect().await.unwrap().to_bytes();
    let wallet_json: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(wallet_json["data"]["total_gross"].as_str().unwrap(), "800");
    assert_eq!(wallet_json["data"]["withheld_fee"].as_str().unwrap(), "160.00");
    assert_eq!(wallet_json["data"]["final_net"].as_str().unwrap(), "640.00");
}

#[tokio::test]
async fn test_multi_prescription_intake_and_5_photo_limit() {
    let (app, _) = test_app();
    let apt_id = Uuid::new_v4();

    // Upload 3 valid prescription images
    let upload_3 = serde_json::json!({
        "files": [
            { "mime_type": "image/jpeg", "base64_content": "/9j/test1" },
            { "mime_type": "image/jpeg", "base64_content": "/9j/test2" },
            { "mime_type": "image/jpeg", "base64_content": "/9j/test3" }
        ]
    });

    let req1 = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/appointments/{}/prescriptions", apt_id))
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&upload_3).unwrap()))
        .unwrap();

    let res1 = app.clone().oneshot(req1).await.unwrap();
    assert_eq!(res1.status(), StatusCode::CREATED);

    // Attempt to upload 3 more (total 6 > max 5 limit) -> must be rejected with 422
    let upload_more = serde_json::json!({
        "files": [
            { "mime_type": "image/jpeg", "base64_content": "/9j/test4" },
            { "mime_type": "image/jpeg", "base64_content": "/9j/test5" },
            { "mime_type": "image/jpeg", "base64_content": "/9j/test6" }
        ]
    });

    let req2 = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/appointments/{}/prescriptions", apt_id))
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&upload_more).unwrap()))
        .unwrap();

    let res2 = app.clone().oneshot(req2).await.unwrap();
    assert_eq!(res2.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_clinical_grievance_and_board_refund_adjudication() {
    let (app, _) = test_app();
    let consult_id = Uuid::new_v4();

    // 1. Patient files dispute (star-rating-free)
    let grv_payload = serde_json::json!({
        "consultation_id": consult_id,
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

    let body_bytes = res_grv.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();
    let grv_id_str = json["data"]["grievance_id"].as_str().unwrap();
    let grv_id = Uuid::parse_str(grv_id_str).unwrap();

    // 2. Admin Medical Board Adjudicates Refund
    let adj_payload = serde_json::json!({
        "audit_notes": "Call telemetry confirmed 45s duration; claim substantiated."
    });

    let req_adj = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/admin/grievances/{}/refund", grv_id))
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&adj_payload).unwrap()))
        .unwrap();

    let res_adj = app.clone().oneshot(req_adj).await.unwrap();
    assert_eq!(res_adj.status(), StatusCode::OK);

    let body_bytes = res_adj.into_body().collect().await.unwrap().to_bytes();
    let adj_json: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(adj_json["data"]["status"].as_str().unwrap(), "REFUNDED");
}

#[tokio::test]
async fn test_monthly_finance_disbursement_batch() {
    let (app, _) = test_app();

    let disb_payload = serde_json::json!({
        "period_start": "2026-09-01",
        "period_end": "2026-09-30"
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/admin/disbursements/initiate")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&disb_payload).unwrap()))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn test_doctor_mfa_login_two_step_verification() {
    let (app, _) = test_app();

    // Step 1: License + Password
    let req1 = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/doctor/login")
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({
                "license_number": "BMDC #45821",
                "password": "Password123!"
            }))
            .unwrap(),
        ))
        .unwrap();

    let res1 = app.clone().oneshot(req1).await.unwrap();
    assert_eq!(res1.status(), StatusCode::OK);

    let body_bytes = res1.into_body().collect().await.unwrap().to_bytes();
    let json1: Value = serde_json::from_slice(&body_bytes).unwrap();
    let session_token = json1["data"]["session_token"].as_str().unwrap();

    // Step 2: Second-factor OTP verification
    let req2 = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/doctor/verify-otp")
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({
                "session_token": session_token,
                "otp_code": "837194"
            }))
            .unwrap(),
        ))
        .unwrap();

    let res2 = app.clone().oneshot(req2).await.unwrap();
    assert_eq!(res2.status(), StatusCode::OK);

    let body_bytes2 = res2.into_body().collect().await.unwrap().to_bytes();
    let json2: Value = serde_json::from_slice(&body_bytes2).unwrap();
    assert_eq!(json2["data"]["user"]["role"].as_str().unwrap(), "DOCTOR");
}

#[tokio::test]
async fn test_24h_clinical_chat_messaging_and_expiration_locking() {
    let (app, state) = test_app();
    let apt_id = Uuid::new_v4();
    let conv_id = Uuid::new_v4();

    // Setup active conversation
    state.chat_conversations.write().insert(
        conv_id,
        ChatConversation {
            id: conv_id,
            appointment_id: apt_id,
            patient_id: Uuid::new_v4(),
            doctor_id: Uuid::new_v4(),
            expires_at: chrono::Utc::now() + chrono::Duration::hours(24),
            is_locked: false,
            created_at: chrono::Utc::now(),
        },
    );

    // 1. Post message during active 24h window
    let msg_payload = serde_json::json!({
        "content": "Doctor, can I take paracetamol after food?"
    });

    let req1 = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/chat/{}/messages", conv_id))
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&msg_payload).unwrap()))
        .unwrap();

    let res1 = app.clone().oneshot(req1).await.unwrap();
    assert_eq!(res1.status(), StatusCode::CREATED);

    // 2. Simulate 24-hour expiration by setting expires_at in the past
    {
        let mut convs = state.chat_conversations.write();
        let conv = convs.get_mut(&conv_id).unwrap();
        conv.expires_at = chrono::Utc::now() - chrono::Duration::seconds(10);
    }

    // 3. Attempt posting after 24h -> Must be locked and rejected with 403 Forbidden
    let req2 = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/chat/{}/messages", conv_id))
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&msg_payload).unwrap()))
        .unwrap();

    let res2 = app.clone().oneshot(req2).await.unwrap();
    assert_eq!(res2.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_complete_consultation_without_prescription() {
    let (app, state) = test_app();
    let apt_id = Uuid::new_v4();
    let doc_id = Uuid::parse_str("da000001-0000-0000-0000-000000000001").unwrap();

    state.appointments.write().insert(
        apt_id,
        Appointment {
            id: apt_id,
            appointment_number: "APT-TEST-NO-RX".into(),
            patient_id: Uuid::new_v4(),
            doctor_id: doc_id,
            slot_id: Uuid::new_v4(),
            modality: Modality::Video,
            status: AppointmentStatus::InConsultation,
            consultation_fee: bigdecimal::BigDecimal::from(500),
            clinical_outcome: None,
            completed_at: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        },
    );

    // Doctor confirms no prescription required for this session
    let no_rx_payload = serde_json::json!({
        "reason": "Follow-up consultation — patient recovering well, lifestyle advice provided"
    });

    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/consultations/{}/complete-no-rx", apt_id))
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&no_rx_payload).unwrap()))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let rx = state.prescriptions.read().get(&apt_id).unwrap().clone();
    assert!(rx.is_no_rx_required);
    assert_eq!(
        rx.no_rx_reason.unwrap(),
        "Follow-up consultation — patient recovering well, lifestyle advice provided"
    );
}

