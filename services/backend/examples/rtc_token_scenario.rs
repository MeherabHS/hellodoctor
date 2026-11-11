//! Legitimate, in-window RTC-token mint scenario -- separate from, and never
//! folded into, the main 166-170 RPS capacity_test workload.
//!
//! WHY THIS EXISTS: `capacity_test.rs`'s `rtc_token_action` books against
//! whichever slot its shared `available_slots` cursor hands out from the
//! seeded capacity dataset, which is NOT scheduled to start within the real
//! RTC mint window (`AGORA_TOKEN_WINDOW_BEFORE_MINS`/`_AFTER_MINS` relative
//! to the appointment's own slot `start_time`/`end_time` --
//! `ConsultationService::mint_agora_token`, services/consultation_service.rs).
//! So under that harness, every `POST .../rtc-token` attempt legitimately
//! gets 401/403 from the backend's own correct authorization logic -- this
//! was never a backend bug, and this scenario does NOT bypass, relax, or
//! shortcut that logic, the G-30/G-31 live-reservation model
//! (docs/MILESTONE_8_G30_G31_IMPLEMENTATION.md), or consultation-state
//! checks in any way. It exercises the exact same real APIs a real doctor
//! and patient would use, timed so the real window is genuinely open:
//!   1. Mint a DOCTOR + PATIENT bearer token for two real synthetic
//!      capacity-test identities (same documented bypass as capacity_test.rs
//!      and web_workload.rs -- no real password login).
//!   2. POST /api/v1/doctor/slots with start_time ~90s in the future (passes
//!      the handler's only real constraint, `start_time > now()`) and
//!      end_time 15 minutes later.
//!   3. POST /api/v1/appointments/book against that real slot (VIDEO
//!      modality, same as capacity_test's own booking shape).
//!   4. POST /api/v1/webhooks/payment/bkash with a real HMAC signature to
//!      confirm payment (same mechanism as capacity_test.rs's
//!      `booking_action` -- an unmodified, already-reviewed flow).
//!   5. POST /api/v1/consultations/{appointment_id}/rtc-token as BOTH the
//!      patient and the doctor -- the window is open immediately because
//!      the slot's own start_time (now+90s) already falls inside
//!      [start_time - AGORA_TOKEN_WINDOW_BEFORE_MINS, end_time +
//!      AGORA_TOKEN_WINDOW_AFTER_MINS] the instant the appointment is
//!      CONFIRMED.
//!
//! Usage (six-service local-verification stack already up):
//!     RTC_ITERATIONS=5 RTC_BASE_URL=http://127.0.0.1:58080 \
//!     RTC_HOST_HEADER=api.localhost \
//!     RTC_DATABASE_URL=postgres://postgres:m8-local-password@127.0.0.1:55432/hellodoctor \
//!         cargo run --release --example rtc_token_scenario
//!
//! Deliberately NOT run as part of the main capacity_test/web_workload
//! comparisons -- this is its own small, separately-reported scenario (see
//! module doc rationale above and the "Phase 2" section of
//! docs/MILESTONE_8_SIX_SERVICE_PERFORMANCE_REMEDIATION.md).

use chrono::{Duration as ChronoDuration, Utc};
use hellodoctor_backend::crypto::create_access_token;
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use sqlx::Row;
use std::env;
use std::time::{Duration, Instant};
use uuid::Uuid;

fn hmac_sign(body: &str, secret: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(body.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let base_url =
        env::var("RTC_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:58080".to_string());
    let host_header = env::var("RTC_HOST_HEADER").unwrap_or_else(|_| "api.localhost".to_string());
    let iterations: usize = env::var("RTC_ITERATIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);
    let database_url = env::var("RTC_DATABASE_URL").unwrap_or_else(|_| {
        "postgres://postgres:m8-local-password@127.0.0.1:55432/hellodoctor".to_string()
    });
    let jwt_private_key =
        env::var("JWT_ED25519_PRIVATE_KEY_B64").expect("JWT_ED25519_PRIVATE_KEY_B64 must be set");
    let webhook_secret = env::var("WEBHOOK_HMAC_SECRET").expect("WEBHOOK_HMAC_SECRET must be set");

    let pg = PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await
        .expect("connect to Postgres");

    // One doctor PER ITERATION (not one doctor reused `iterations` times) --
    // each doctor's own `doctor_slots` table is independent, so distinct
    // doctors never collide on the overlap constraint even when every
    // iteration's slot starts at the same near-term offset.
    // OFFSET rotates which doctors/patients this run uses, so repeated
    // manual runs against the same long-lived dataset don't keep colliding
    // with `doctor_slots` rows a previous run already created nearby in time.
    let rotation = (chrono::Utc::now().timestamp() / 60) % 30;
    let doctor_rows = sqlx::query(
        "SELECT u.id AS user_id, dp.id AS profile_id FROM users u \
         JOIN doctor_profiles dp ON dp.user_id = u.id \
         WHERE u.role = 'DOCTOR' AND u.email LIKE '%@capacity-test.local' \
         ORDER BY u.id OFFSET $1 LIMIT $2",
    )
    .bind(rotation)
    .bind(iterations as i64)
    .fetch_all(&pg)
    .await
    .expect("fetch synthetic DOCTORs");
    assert!(
        doctor_rows.len() >= iterations,
        "need at least {iterations} synthetic doctors"
    );

    // One patient PER ITERATION too -- every iteration uses the same
    // near-term start_time, and a patient legitimately cannot hold two
    // overlapping appointments (with any doctor), so distinct patients
    // avoid that real, correct business-rule conflict rather than needing
    // to stagger times (which would risk drifting outside the mint window).
    let patient_rotation = (chrono::Utc::now().timestamp() / 60) % 5000;
    let patient_rows =
        sqlx::query("SELECT id FROM users WHERE role = 'PATIENT' ORDER BY id OFFSET $1 LIMIT $2")
            .bind(patient_rotation)
            .bind(iterations as i64)
            .fetch_all(&pg)
            .await
            .expect("fetch PATIENTs");

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .default_headers({
            let mut h = reqwest::header::HeaderMap::new();
            h.insert(
                reqwest::header::HOST,
                reqwest::header::HeaderValue::from_str(&host_header).unwrap(),
            );
            h
        })
        .build()
        .expect("build client");

    println!("=== RTC-token in-window legitimacy scenario ===");
    println!("iterations={iterations}");

    let mut successes = 0usize;
    let mut failures = 0usize;
    let mut latencies: Vec<Duration> = Vec::new();

    for i in 0..iterations {
        let doctor_user_id: Uuid = doctor_rows[i].get("user_id");
        let doctor_profile_id: Uuid = doctor_rows[i].get("profile_id");
        let doctor_token =
            create_access_token(doctor_user_id, "DOCTOR", doctor_user_id, &jwt_private_key)
                .expect("mint DOCTOR token");
        let patient_user_id: Uuid = patient_rows[i].get("id");
        let patient_token = create_access_token(
            patient_user_id,
            "PATIENT",
            patient_user_id,
            &jwt_private_key,
        )
        .expect("mint PATIENT token");

        let start_time = Utc::now() + ChronoDuration::seconds(90);
        let end_time = start_time + ChronoDuration::minutes(15);

        let slot_res = client
            .post(format!("{base_url}/api/v1/doctor/slots"))
            .bearer_auth(&doctor_token)
            .json(&json!({ "start_time": start_time, "end_time": end_time }))
            .send()
            .await
            .expect("create slot request");
        let slot_status = slot_res.status();
        let slot_body: serde_json::Value = slot_res.json().await.unwrap_or_default();
        if !slot_status.is_success() {
            eprintln!("[iter {i}] FAIL create slot: status={slot_status} body={slot_body}");
            failures += 1;
            continue;
        }
        let slot_id: Uuid = slot_body["data"]["id"]
            .as_str()
            .and_then(|s| Uuid::parse_str(s).ok())
            .expect("slot id in response");

        let book_res = client
            .post(format!("{base_url}/api/v1/appointments/book"))
            .bearer_auth(&patient_token)
            .header("Idempotency-Key", Uuid::new_v4().to_string())
            .json(&json!({
                "doctor_id": doctor_profile_id, "slot_id": slot_id, "modality": "VIDEO",
                "gateway": "BKASH", "chief_complaint": "RTC-token legitimacy scenario"
            }))
            .send()
            .await
            .expect("book request");
        let book_status = book_res.status();
        let book_body: serde_json::Value = book_res.json().await.unwrap_or_default();
        if !book_status.is_success() {
            eprintln!("[iter {i}] FAIL booking: status={book_status} body={book_body}");
            failures += 1;
            continue;
        }
        let appointment_id: Uuid = book_body["data"]["appointment_id"]
            .as_str()
            .and_then(|s| Uuid::parse_str(s).ok())
            .expect("appointment_id in response");
        let payment_session_id = book_body["data"]["payment_session_id"]
            .as_str()
            .expect("payment_session_id in response");

        let webhook_body = json!({
            "payment_session_id": payment_session_id,
            "transaction_reference": format!("RTC-{}", Uuid::new_v4()),
            "status": "SUCCESS",
            "event_timestamp": Utc::now().to_rfc3339(),
            "provider_event_id": Uuid::new_v4().to_string(),
        });
        let body_str = serde_json::to_string(&webhook_body).unwrap();
        let sig = hmac_sign(&body_str, &webhook_secret);
        let wh_status = client
            .post(format!("{base_url}/api/v1/webhooks/payment/bkash"))
            .header("Content-Type", "application/json")
            .header("X-Signature", sig)
            .body(body_str)
            .send()
            .await
            .expect("webhook request")
            .status();
        if !wh_status.is_success() {
            eprintln!("[iter {i}] FAIL webhook confirm: status={wh_status}");
            failures += 1;
            continue;
        }

        for (role, token) in [("PATIENT", &patient_token), ("DOCTOR", &doctor_token)] {
            let start = Instant::now();
            let res = client
                .post(format!(
                    "{base_url}/api/v1/consultations/{appointment_id}/rtc-token"
                ))
                .bearer_auth(token)
                .json(&json!({}))
                .send()
                .await
                .expect("rtc-token request");
            let elapsed = start.elapsed();
            let status = res.status();
            if status.is_success() {
                successes += 1;
                latencies.push(elapsed);
                println!("[iter {i}] {role} rtc-token: SUCCESS ({elapsed:?})");
            } else {
                failures += 1;
                let body: serde_json::Value = res.json().await.unwrap_or_default();
                println!("[iter {i}] {role} rtc-token: FAIL status={status} body={body}");
            }
        }
    }

    latencies.sort();
    let n = latencies.len();
    let (p50, p95) = if n == 0 {
        (0, 0)
    } else {
        (
            latencies[n * 50 / 100].as_millis(),
            latencies[(n * 95 / 100).min(n - 1)].as_millis(),
        )
    };
    println!(
        "\nRTC_TOKEN_SCENARIO_RESULT: successes={successes} failures={failures} \
         success_rate={:.1}% p50_ms={p50} p95_ms={p95}",
        successes as f64 / (successes + failures).max(1) as f64 * 100.0
    );
    if successes == 0 {
        std::process::exit(1);
    }
}
