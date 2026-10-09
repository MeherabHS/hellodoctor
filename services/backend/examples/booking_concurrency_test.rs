//! Booking-concurrency CORRECTNESS test — not a capacity benchmark (see
//! examples/load_test.rs for that). Two controlled scenarios prove the booking
//! path is safe under concurrency without the "everyone grabs the same slot"
//! noise that inflated the 409 rate in the mixed load test, followed by a
//! direct Postgres integrity audit and an idempotency-replay check.
//!
//! Usage (server + `docker compose up -d` already running, same as load_test.rs):
//!     cargo run --release --example booking_concurrency_test
//!
//! Security controls are NOT weakened for this test: real ABAC, real RLS, real
//! `check_booking_limit` (10/hour/user_id) all apply. Each scenario provisions
//! enough distinct real patient accounts that no one account's quota is at risk
//! of being exhausted by this test itself.

use chrono::{Duration as ChronoDuration, Utc};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use std::env;
use std::time::{Duration, Instant};
use uuid::Uuid;

fn base_url() -> String {
    env::var("CAPACITY_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".to_string())
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set (see .env)");
    let pg = PgPoolOptions::new()
        .max_connections(20)
        .connect(&database_url)
        .await
        .expect("connect to Postgres");

    let doctor_id = fetch_one_doctor_id(&pg).await;
    let jwt_private_key =
        env::var("JWT_ED25519_PRIVATE_KEY_B64").expect("JWT_ED25519_PRIVATE_KEY_B64 must be set");
    let webhook_secret = env::var("WEBHOOK_HMAC_SECRET").unwrap_or_default();

    println!("=== HelloDoctor Booking Concurrency & Integrity Test ===");
    println!("Target: {}  (see CAPACITY_BASE_URL env var)\n", base_url());

    run_non_contended_test(&client, &pg, doctor_id).await;
    run_contention_test(&client, &pg, doctor_id).await;
    run_idempotency_replay_check(&client, &pg, doctor_id, &webhook_secret).await;
    run_db_integrity_check(&pg).await;

    let _ = jwt_private_key; // reserved: not needed once booking goes through real patient tokens only
}

async fn fetch_one_doctor_id(pg: &sqlx::PgPool) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM doctor_profiles WHERE deleted_at IS NULL ORDER BY id LIMIT 1",
    )
    .fetch_one(pg)
    .await
    .expect("at least one seeded doctor")
}

/// Fresh, collision-free slots anchored 500 days out (distinct from load_test.rs's
/// 365-day anchor and from any prior run of this file) via a *truly random* jitter
/// offset — a wall-clock-timestamp-derived jitter (the first version of this
/// function) collides when two sections of the same process call this within the
/// same second, confirmed: section 3 and section 4's provisioning calls landed the
/// same anchor and hit `no_overlapping_slots` against each other, not just against
/// a different run.
async fn provision_unique_slots(pg: &sqlx::PgPool, doctor_id: Uuid, count: usize) -> Vec<Uuid> {
    let jitter_minutes: i64 = rand::Rng::gen_range(&mut rand::thread_rng(), 0..50_000_000);
    let anchor = Utc::now() + ChronoDuration::days(500) + ChronoDuration::minutes(jitter_minutes);
    let mut ids = Vec::with_capacity(count);
    let mut tx = pg.begin().await.unwrap();
    for i in 0..count {
        let id = Uuid::new_v4();
        let start = anchor + ChronoDuration::minutes(20 * i as i64);
        let end = start + ChronoDuration::minutes(15);
        sqlx::query(
            "INSERT INTO doctor_schedule_slots (id, doctor_id, start_time, end_time, status, created_at) \
             VALUES ($1, $2, $3, $4, 'AVAILABLE', now())",
        )
        .bind(id)
        .bind(doctor_id)
        .bind(start)
        .bind(end)
        .execute(&mut *tx)
        .await
        .unwrap();
        ids.push(id);
    }
    tx.commit().await.unwrap();
    ids
}

async fn provision_patients(client: &reqwest::Client, count: usize) -> Vec<String> {
    let phone_salt = 200_000_000 + (Utc::now().timestamp() as u64 % 500_000_000);
    let mut tokens = Vec::with_capacity(count);
    for i in 0..count {
        let phone = format!("+2547{:08}", (phone_salt + i as u64) % 100_000_000);
        if let Some(token) = real_patient_login(client, &phone).await {
            tokens.push(token);
        }
    }
    tokens
}

async fn real_patient_login(client: &reqwest::Client, phone: &str) -> Option<String> {
    let res = client
        .post(format!("{}/api/v1/auth/register-otp", base_url()))
        .json(&json!({ "phone_number": phone }))
        .send()
        .await
        .ok()?;
    if !res.status().is_success() {
        return None;
    }
    let body: serde_json::Value = res.json().await.ok()?;
    let code = body["data"]["debug_otp_code"].as_str()?.to_string();

    let res = client
        .post(format!("{}/api/v1/auth/verify-otp-and-login", base_url()))
        .json(&json!({ "phone_number": phone, "otp_code": code }))
        .send()
        .await
        .ok()?;
    if !res.status().is_success() {
        return None;
    }
    let body: serde_json::Value = res.json().await.ok()?;
    body["data"]["access_token"].as_str().map(String::from)
}

struct BookAttemptResult {
    status: u16,
    elapsed: Duration,
}

async fn attempt_book(
    client: &reqwest::Client,
    token: &str,
    doctor_id: Uuid,
    slot_id: Uuid,
) -> BookAttemptResult {
    let start = Instant::now();
    let status = client
        .post(format!("{}/api/v1/appointments/book", base_url()))
        .bearer_auth(token)
        .header("Idempotency-Key", Uuid::new_v4().to_string())
        .json(&json!({
            "doctor_id": doctor_id,
            "slot_id": slot_id,
            "modality": "VIDEO",
            "gateway": "BKASH",
            "chief_complaint": "Concurrency-test booking"
        }))
        .send()
        .await
        .map(|r| r.status().as_u16())
        .unwrap_or(0);
    BookAttemptResult {
        status,
        elapsed: start.elapsed(),
    }
}

fn percentiles(mut durs: Vec<Duration>) -> (u128, u128, u128, u128) {
    if durs.is_empty() {
        return (0, 0, 0, 0);
    }
    durs.sort();
    let n = durs.len();
    (
        durs[n * 50 / 100].as_millis(),
        durs[(n * 95 / 100).min(n - 1)].as_millis(),
        durs[(n * 99 / 100).min(n - 1)].as_millis(),
        durs[n - 1].as_millis(),
    )
}

// ============================================================================
// SECTION 3 — non-contended booking test: every patient gets a unique slot,
// so no two patients can ever race for the same row. Measures real booking
// performance without slot-race noise.
// ============================================================================
async fn run_non_contended_test(client: &reqwest::Client, pg: &sqlx::PgPool, doctor_id: Uuid) {
    const N: usize = 30;
    println!("--- SECTION 3: Non-contended booking test (N={N}, one unique slot per patient) ---");

    let slots = provision_unique_slots(pg, doctor_id, N).await;
    let patients = provision_patients(client, N).await;
    println!(
        "Provisioned {} unique slots, {} patient sessions.",
        slots.len(),
        patients.len()
    );

    let n = slots.len().min(patients.len());
    let mut handles = Vec::with_capacity(n);
    for i in 0..n {
        let client = client.clone();
        let token = patients[i].clone();
        let slot_id = slots[i];
        handles.push(tokio::spawn(async move {
            attempt_book(&client, &token, doctor_id, slot_id).await
        }));
    }

    let mut c201 = 0u32;
    let mut c409 = 0u32;
    let mut c429 = 0u32;
    let mut c5xx = 0u32;
    let mut durs = Vec::new();
    for h in handles {
        let r = h.await.unwrap();
        durs.push(r.elapsed);
        match r.status {
            201 => c201 += 1,
            409 => c409 += 1,
            429 => c429 += 1,
            500..=599 => c5xx += 1,
            other => println!("  (unexpected status {other} in non-contended test)"),
        }
    }
    let (p50, p95, p99, max) = percentiles(durs);

    println!("attempts={n} 201={c201} 409={c409} 429={c429} 5xx={c5xx}");
    println!("latency_ms: p50={p50} p95={p95} p99={p99} max={max}");
    println!(
        "Expected near-zero 409 / zero 5xx: {}\n",
        if c409 == 0 && c5xx == 0 {
            "CONFIRMED"
        } else if c5xx == 0 {
            "409 > 0 — investigate (should be ~0 with unique slots)"
        } else {
            "FAILED — 5xx present"
        }
    );
}

// ============================================================================
// SECTION 4 — intentional contention test: many patients target a small,
// shared pool of slots. Proves booking concurrency safety: exactly one winner
// per slot, everyone else a clean 409, zero duplicates, zero 500s.
// ============================================================================
async fn run_contention_test(client: &reqwest::Client, pg: &sqlx::PgPool, doctor_id: Uuid) {
    const NUM_SLOTS: usize = 5;
    const CONTENDERS_PER_SLOT: usize = 6;
    const NUM_CONTENDERS: usize = NUM_SLOTS * CONTENDERS_PER_SLOT;
    println!(
        "--- SECTION 4: Intentional contention test ({NUM_SLOTS} slots, {NUM_CONTENDERS} contenders, {CONTENDERS_PER_SLOT} per slot) ---"
    );

    let slots = provision_unique_slots(pg, doctor_id, NUM_SLOTS).await;
    let patients = provision_patients(client, NUM_CONTENDERS).await;
    println!(
        "Provisioned {} slots, {} contender patient sessions.",
        slots.len(),
        patients.len()
    );

    let mut handles = Vec::with_capacity(patients.len());
    for (i, token) in patients.iter().enumerate() {
        let slot_id = slots[i % slots.len()];
        let client = client.clone();
        let token = token.clone();
        handles.push(tokio::spawn(async move {
            attempt_book(&client, &token, doctor_id, slot_id).await
        }));
    }

    let mut c201 = 0u32;
    let mut c409 = 0u32;
    let mut unexpected = 0u32;
    for h in handles {
        let r = h.await.unwrap();
        match r.status {
            201 => c201 += 1,
            409 => c409 += 1,
            _ => unexpected += 1,
        }
    }

    // Ground truth, not just the HTTP responses: ask Postgres directly how many
    // live (non-cancelled) appointments actually landed on each contended slot.
    let per_slot_counts: Vec<(Uuid, i64)> = {
        let mut out = Vec::new();
        for &slot_id in &slots {
            let count: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM appointments WHERE slot_id = $1 AND status NOT IN ('CANCELLED','EXPIRED')",
            )
            .bind(slot_id)
            .fetch_one(pg)
            .await
            .unwrap();
            out.push((slot_id, count));
        }
        out
    };
    let duplicate_slots = per_slot_counts.iter().filter(|(_, c)| *c > 1).count();
    let unwon_slots = per_slot_counts.iter().filter(|(_, c)| *c == 0).count();

    println!(
        "slots={} contenders={} winners(201)={c201} losers(409)={c409} unexpected={unexpected}",
        slots.len(),
        patients.len()
    );
    println!(
        "DB ground truth — duplicate_bookings_for_one_slot={duplicate_slots} unwon_slots={unwon_slots}"
    );
    println!(
        "Expected exactly one winner per slot, zero duplicates, zero 500s: {}\n",
        if duplicate_slots == 0 && unexpected == 0 && c201 == slots.len() as u32 {
            "CONFIRMED"
        } else {
            "FAILED — see counts above"
        }
    );
}

// ============================================================================
// Idempotency replay check — resend the exact same booking request (same
// Idempotency-Key, same body) and confirm it replays the cached response
// instead of creating a second appointment.
// ============================================================================
async fn run_idempotency_replay_check(
    client: &reqwest::Client,
    pg: &sqlx::PgPool,
    doctor_id: Uuid,
    _webhook_secret: &str,
) {
    println!("--- IDEMPOTENCY REPLAY CHECK ---");
    let slots = provision_unique_slots(pg, doctor_id, 1).await;
    let slot_id = slots[0];
    let patients = provision_patients(client, 1).await;
    let Some(token) = patients.into_iter().next() else {
        println!("Could not provision a patient for the idempotency check — skipping.\n");
        return;
    };

    let idem_key = Uuid::new_v4().to_string();
    let body = json!({
        "doctor_id": doctor_id,
        "slot_id": slot_id,
        "modality": "VIDEO",
        "gateway": "BKASH",
        "chief_complaint": "Idempotency-replay-test booking"
    });

    let first = client
        .post(format!("{}/api/v1/appointments/book", base_url()))
        .bearer_auth(&token)
        .header("Idempotency-Key", idem_key.clone())
        .json(&body)
        .send()
        .await
        .expect("first booking request");
    let first_status = first.status().as_u16();
    let first_body = first.text().await.unwrap_or_default();

    let second = client
        .post(format!("{}/api/v1/appointments/book", base_url()))
        .bearer_auth(&token)
        .header("Idempotency-Key", idem_key)
        .json(&body)
        .send()
        .await
        .expect("replayed booking request");
    let second_status = second.status().as_u16();
    let second_body = second.text().await.unwrap_or_default();

    let replay_matched = first_status == second_status && first_body == second_body;

    let appointment_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM appointments WHERE slot_id = $1")
            .bind(slot_id)
            .fetch_one(pg)
            .await
            .unwrap();

    println!("first_status={first_status} second_status={second_status} replay_body_matched={replay_matched}");
    println!("appointments_created_for_this_slot={appointment_count} (expected: 1)");
    println!(
        "Idempotency replay correctness: {}\n",
        if replay_matched && appointment_count == 1 {
            "CONFIRMED"
        } else {
            "FAILED — see counts above"
        }
    );
}

// ============================================================================
// SECTION 5 — post-test database integrity check. Exact counts, not a verdict
// string — every query below should return 0.
// ============================================================================
async fn run_db_integrity_check(pg: &sqlx::PgPool) {
    println!(
        "--- SECTION 5: Post-test database integrity check (exact counts, expect 0 everywhere) ---"
    );

    let checks: Vec<(&str, &str)> = vec![
        (
            "duplicate_booked_slots (same slot_id, >1 live appointment)",
            "SELECT count(*) FROM (SELECT slot_id FROM appointments \
             WHERE status NOT IN ('CANCELLED','EXPIRED') GROUP BY slot_id HAVING count(*) > 1) t",
        ),
        (
            "multiple_confirmed_for_same_slot",
            "SELECT count(*) FROM (SELECT slot_id FROM appointments \
             WHERE status = 'CONFIRMED' GROUP BY slot_id HAVING count(*) > 1) t",
        ),
        (
            "orphan_appointments (no clinical intake row)",
            "SELECT count(*) FROM appointments a \
             LEFT JOIN appointment_clinical_intake c ON c.appointment_id = a.id \
             WHERE c.appointment_id IS NULL",
        ),
        (
            "orphan_clinical_intake_rows (no parent appointment)",
            "SELECT count(*) FROM appointment_clinical_intake c \
             LEFT JOIN appointments a ON a.id = c.appointment_id WHERE a.id IS NULL",
        ),
        (
            "appointments_with_no_transaction (partially-created bookings)",
            "SELECT count(*) FROM appointments a \
             LEFT JOIN transactions t ON t.appointment_id = a.id WHERE t.id IS NULL",
        ),
        (
            "orphan_transactions (no parent appointment)",
            "SELECT count(*) FROM transactions t \
             LEFT JOIN appointments a ON a.id = t.appointment_id WHERE a.id IS NULL",
        ),
        (
            "duplicate_gateway_reference",
            "SELECT count(*) FROM (SELECT gateway, gateway_reference FROM transactions \
             WHERE gateway_reference IS NOT NULL \
             GROUP BY gateway, gateway_reference HAVING count(*) > 1) t",
        ),
        (
            "duplicate_provider_event_id",
            "SELECT count(*) FROM (SELECT gateway, provider_event_id FROM payment_events \
             GROUP BY gateway, provider_event_id HAVING count(*) > 1) t",
        ),
        (
            "invalid_state_confirmed_without_held_payment",
            "SELECT count(*) FROM appointments a JOIN transactions t ON t.appointment_id = a.id \
             WHERE a.status = 'CONFIRMED' \
             AND t.payment_status NOT IN ('PAYMENT_HELD','SETTLED_TO_DOCTOR','DISBURSED')",
        ),
        (
            "invalid_state_pending_with_held_payment",
            "SELECT count(*) FROM appointments a JOIN transactions t ON t.appointment_id = a.id \
             WHERE a.status = 'PENDING_PAYMENT' \
             AND t.payment_status IN ('PAYMENT_HELD','SETTLED_TO_DOCTOR','DISBURSED')",
        ),
        (
            "invalid_slot_booked_with_no_live_appointment",
            "SELECT count(*) FROM doctor_schedule_slots s WHERE s.status = 'BOOKED' \
             AND NOT EXISTS (SELECT 1 FROM appointments a WHERE a.slot_id = s.id \
             AND a.status NOT IN ('CANCELLED','EXPIRED'))",
        ),
        (
            "invalid_slot_available_with_live_appointment",
            "SELECT count(*) FROM doctor_schedule_slots s WHERE s.status = 'AVAILABLE' \
             AND EXISTS (SELECT 1 FROM appointments a WHERE a.slot_id = s.id \
             AND a.status NOT IN ('CANCELLED','EXPIRED'))",
        ),
    ];

    let mut all_zero = true;
    for (label, sql) in &checks {
        let count: i64 = sqlx::query_scalar(sql).fetch_one(pg).await.unwrap_or(-1);
        if count != 0 {
            all_zero = false;
        }
        println!("{label:<60} {count}");
    }

    println!(
        "\nDATABASE_INTEGRITY_RESULT: {}\n",
        if all_zero {
            "PASSED (all checks = 0)"
        } else {
            "FAILED — see non-zero counts above"
        }
    );
}
