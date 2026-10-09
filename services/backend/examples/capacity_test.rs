//! Open-loop capacity test against the constrained-VPS capacity stack
//! (docker-compose.capacity.yml + scripts/seed_capacity_dataset.sql).
//!
//! Unlike load_test.rs/booking_concurrency_test.rs (closed-loop: a worker
//! fires its next request only after the previous one returns), this fires
//! requests at a configured TARGET ARRIVAL RATE independent of response
//! latency — if the backend slows down, offered load does not drop with it,
//! which is what's needed to see real queueing/saturation instead of the
//! load generator accidentally self-throttling in sympathy with the backend.
//!
//! Usage (capacity stack already up — see docs/CONSTRAINED_CAPACITY_TEST.md):
//!     CAPACITY_TARGET_RPS=300 CAPACITY_DURATION_SECS=600 \
//!         cargo run --release --example capacity_test
//!
//! Env overrides: CAPACITY_BASE_URL (default http://localhost:8180),
//! CAPACITY_TARGET_RPS (default 100), CAPACITY_DURATION_SECS (default 60),
//! CAPACITY_DATABASE_URL (default the capacity-test postgres on :5532).
//!
//! PRE_PATIENT_AZURE_4X8_BENCHMARK §6 "active session" mode — a second drive
//! mode added alongside the original open-loop RPS mode, reusing the exact
//! same weighted-action dispatch and outcome/percentile reporting rather
//! than duplicating a parallel harness:
//!     CAPACITY_MODE=sessions CAPACITY_ACTIVE_SESSIONS=1200 \
//!     CAPACITY_THINK_MIN_SECS=5 CAPACITY_THINK_MAX_SECS=20 \
//!     CAPACITY_DURATION_SECS=300 cargo run --release --example capacity_test
//! Each session is one long-lived task bound to one fixed patient identity
//! for the test's duration (continuity — its own booked appointment, its own
//! chat/rtc/telemetry actions build on that, exactly like a real user's
//! session), looping: dispatch one weighted action, sleep a random
//! think-time interval in [MIN,MAX] seconds, repeat until the deadline. This
//! is deliberately NOT a tight loop — "realistic idle/think intervals rather
//! than continuously looping each user" per the spec. Registered-user
//! cardinality (how many patients exist in the dataset) and active-session
//! count (how many of them are simulated as concurrently "logged in and
//! doing things" right now) are reported separately, never conflated.

use chrono::Utc;
use hellodoctor_backend::crypto::create_access_token;
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use sqlx::Row;
use std::collections::HashMap;
use std::env;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Outcome {
    Success,
    BusinessReject,
    AuthReject,
    RateLimited,
    SystemError,
}
impl Outcome {
    fn classify(status: u16) -> Outcome {
        match status {
            200..=299 => Outcome::Success,
            401 | 403 => Outcome::AuthReject,
            429 => Outcome::RateLimited,
            500..=599 | 0 => Outcome::SystemError,
            _ => Outcome::BusinessReject,
        }
    }
}

#[derive(Debug, Clone)]
struct Sample {
    label: &'static str,
    elapsed: Duration,
    status: u16,
    sent_at: Instant,
}

/// Per-patient mutable state a booking/rtc-token/telemetry/chat action can
/// build on across separate dispatched requests, giving the open-loop model
/// some continuity (the same virtual patient's own just-booked appointment)
/// without needing a persistent per-user task/loop.
#[derive(Default)]
struct PatientRuntimeState {
    booked_appointment: Option<(Uuid, Uuid)>, // (appointment_id, conversation_id)
    refresh_token: Option<String>,
}

struct Fixtures {
    base_url: String,
    client: reqwest::Client,
    pg: sqlx::PgPool,
    patient_tokens: Vec<String>,
    patient_consultation_ids: Vec<Option<Uuid>>, // one pre-existing historical consultation per patient, for grievances
    patient_state: Vec<Mutex<PatientRuntimeState>>,
    booking_attempts: Vec<AtomicU32>, // per-patient-index counter, gates well under the real 10/hour limit
    doctor_tokens: Vec<String>,
    admin_tokens: Vec<String>,
    available_slots: Mutex<Vec<(Uuid, Uuid)>>, // (slot_id, doctor_id), consumed front-to-back
    slot_cursor: AtomicUsize,
    rr_patient: AtomicUsize,
    rr_doctor: AtomicUsize,
    rr_admin: AtomicUsize,
    webhook_secret: String,
}

const MAX_BOOKING_ATTEMPTS_PER_PATIENT: u32 = 8;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let base_url =
        env::var("CAPACITY_BASE_URL").unwrap_or_else(|_| "http://localhost:8180".to_string());
    let target_rps: f64 = env::var("CAPACITY_TARGET_RPS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(100.0);
    let duration_secs: u64 = env::var("CAPACITY_DURATION_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60);
    let mode = env::var("CAPACITY_MODE").unwrap_or_else(|_| "rps".to_string());
    let active_sessions: usize = env::var("CAPACITY_ACTIVE_SESSIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(300);
    let think_min_secs: u64 = env::var("CAPACITY_THINK_MIN_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);
    let think_max_secs: u64 = env::var("CAPACITY_THINK_MAX_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20);
    let database_url = env::var("CAPACITY_DATABASE_URL").unwrap_or_else(|_| {
        "postgres://hellodoctor:hellodoctor_dev_password@localhost:5532/hellodoctor".to_string()
    });
    let jwt_private_key =
        env::var("JWT_ED25519_PRIVATE_KEY_B64").expect("JWT_ED25519_PRIVATE_KEY_B64 must be set");
    let webhook_secret = env::var("WEBHOOK_HMAC_SECRET").unwrap_or_default();

    println!("=== HelloDoctor Constrained-VPS Capacity Test (open-loop) ===");
    println!("target={base_url} target_rps={target_rps} duration={duration_secs}s\n");

    let pg = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
        .expect("connect to capacity-test Postgres (is the stack up on :5532?)");

    let patient_rows: Vec<(Uuid, Uuid)> = sqlx::query(
        "SELECT u.id, pp.id FROM users u JOIN patient_profiles pp ON pp.user_id = u.id \
         WHERE u.phone_number LIKE '+25472%' ORDER BY u.phone_number",
    )
    .fetch_all(&pg)
    .await
    .expect("load capacity-test patients")
    .into_iter()
    .map(|r| (r.get(0), r.get(1)))
    .collect();

    let doctor_user_ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT u.id FROM users u JOIN doctor_profiles dp ON dp.user_id = u.id \
         WHERE u.phone_number LIKE '+25471%' ORDER BY u.phone_number",
    )
    .fetch_all(&pg)
    .await
    .expect("load capacity-test doctors");

    let admin_user_ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM users WHERE phone_number LIKE '+25474%' ORDER BY phone_number",
    )
    .fetch_all(&pg)
    .await
    .expect("load capacity-test admins");

    let available_slots: Vec<(Uuid, Uuid)> = sqlx::query(
        "SELECT s.id, s.doctor_id FROM doctor_schedule_slots s \
         JOIN doctor_profiles dp ON dp.id = s.doctor_id \
         WHERE s.status = 'AVAILABLE' AND dp.license_number LIKE 'SYNTH-KE-CAP-%' \
         ORDER BY s.start_time",
    )
    .fetch_all(&pg)
    .await
    .expect("load available slots")
    .into_iter()
    .map(|r| (r.get(0), r.get(1)))
    .collect();

    // One pre-existing historical consultation per patient, if they have one
    // (most do, from the seeded dataset), used by the grievance-submission
    // action — real historical data, not something created during the run.
    let patient_consultation_ids: Vec<Option<Uuid>> = {
        let mut map: HashMap<Uuid, Uuid> = HashMap::new();
        let rows: Vec<(Uuid, Uuid)> = sqlx::query(
            "SELECT a.patient_id, cs.id FROM consultation_sessions cs \
             JOIN appointments a ON a.id = cs.appointment_id \
             JOIN patient_profiles pp ON pp.id = a.patient_id \
             JOIN users u ON u.id = pp.user_id \
             WHERE u.phone_number LIKE '+25472%'",
        )
        .fetch_all(&pg)
        .await
        .expect("load patient consultation history")
        .into_iter()
        .map(|r| (r.get(0), r.get(1)))
        .collect();
        for (patient_profile_id, consultation_id) in rows {
            map.entry(patient_profile_id).or_insert(consultation_id);
        }
        patient_rows
            .iter()
            .map(|(_, profile_id)| map.get(profile_id).copied())
            .collect()
    };

    println!(
        "Identities: {} patients, {} doctors, {} admins, {} available slots\n",
        patient_rows.len(),
        doctor_user_ids.len(),
        admin_user_ids.len(),
        available_slots.len()
    );

    sqlx::query("INSERT INTO auth_sessions(id,user_id,token_family_id,refresh_token_hash,created_at,last_used_at,expires_at) SELECT gen_random_uuid(),id,id,'capacity-fixture-session-'||id::text,NOW(),NOW(),NOW()+INTERVAL '7 days' FROM users WHERE phone_number LIKE '+25471%' OR phone_number LIKE '+25472%' OR phone_number LIKE '+25473%' OR phone_number LIKE '+25474%' ON CONFLICT DO NOTHING")
        .execute(&pg).await.expect("synthetic capacity-test sessions");
    let patient_tokens: Vec<String> = patient_rows
        .iter()
        .map(|(uid, _)| create_access_token(*uid, "PATIENT", *uid, &jwt_private_key).unwrap())
        .collect();
    let doctor_tokens: Vec<String> = doctor_user_ids
        .iter()
        .map(|uid| create_access_token(*uid, "DOCTOR", *uid, &jwt_private_key).unwrap())
        .collect();
    let admin_tokens: Vec<String> = admin_user_ids
        .iter()
        .map(|uid| create_access_token(*uid, "PLATFORM_ADMIN", *uid, &jwt_private_key).unwrap())
        .collect();

    let patient_count = patient_tokens.len();
    let fixtures = Arc::new(Fixtures {
        base_url,
        client: reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap(),
        pg: pg.clone(),
        patient_tokens,
        patient_consultation_ids,
        patient_state: (0..patient_count)
            .map(|_| Mutex::new(PatientRuntimeState::default()))
            .collect(),
        booking_attempts: (0..patient_count).map(|_| AtomicU32::new(0)).collect(),
        doctor_tokens,
        admin_tokens,
        available_slots: Mutex::new(available_slots),
        slot_cursor: AtomicUsize::new(0),
        rr_patient: AtomicUsize::new(0),
        rr_doctor: AtomicUsize::new(0),
        rr_admin: AtomicUsize::new(0),
        webhook_secret,
    });

    let (tx, mut rx) = mpsc::unbounded_channel::<Sample>();
    let test_start = Instant::now();
    let deadline = test_start + Duration::from_secs(duration_secs);

    let collector = tokio::spawn(async move {
        let mut samples = Vec::new();
        while let Some(s) = rx.recv().await {
            samples.push(s);
        }
        samples
    });

    let requested_count: u64 = if mode == "sessions" {
        // ---- Active-session (think-time) dispatch — §6 ----
        println!(
            "registered_patients={} active_sessions={active_sessions} think_time_secs=[{think_min_secs},{think_max_secs}]\n",
            patient_count
        );
        let mut handles = Vec::new();
        for session_idx in 0..active_sessions {
            let f = fixtures.clone();
            let tx = tx.clone();
            let patient_idx = session_idx % patient_count.max(1);
            handles.push(tokio::spawn(async move {
                loop {
                    if Instant::now() >= deadline {
                        break;
                    }
                    dispatch_one_for_session(&f, &tx, patient_idx).await;
                    let think = {
                        use rand::Rng;
                        rand::thread_rng()
                            .gen_range(think_min_secs..=think_max_secs.max(think_min_secs))
                    };
                    tokio::time::sleep(Duration::from_secs(think)).await;
                }
            }));
        }
        for h in handles {
            let _ = h.await;
        }
        0
    } else {
        // ---- Open-loop RPS dispatch (original mode) ----
        let interval = Duration::from_secs_f64(1.0 / target_rps);
        let mut requested_count: u64 = 0;
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Burst);
        let mut handles = Vec::new();
        while Instant::now() < deadline {
            ticker.tick().await;
            requested_count += 1;
            let f = fixtures.clone();
            let tx = tx.clone();
            handles.push(tokio::spawn(async move {
                dispatch_one(&f, tx).await;
            }));
        }
        for h in handles {
            let _ = h.await;
        }
        requested_count
    };
    drop(tx);
    let samples = collector.await.unwrap();

    let achieved_rps = samples.len() as f64 / duration_secs as f64;
    println!(
        "requested_rps={target_rps:.1} requested_count={requested_count} achieved_rps={achieved_rps:.1} achieved_count={}\n",
        samples.len()
    );

    report_time_series(&samples, test_start, duration_secs);
    report(&samples, duration_secs, target_rps, requested_count);
}

/// Same weighted action mix as `dispatch_one`, but bound to a fixed
/// `patient_idx` for the session's whole lifetime instead of round-robining
/// — the continuity ("this session's own booked appointment") an active
/// session test needs. Doctor/admin reads still round-robin globally since
/// they are not the session identity being modeled here.
async fn dispatch_one_for_session(
    f: &Fixtures,
    tx: &mpsc::UnboundedSender<Sample>,
    patient_idx: usize,
) {
    let roll: f64 = {
        use rand::Rng;
        rand::thread_rng().gen::<f64>()
    };
    let token = &f.patient_tokens[patient_idx % f.patient_tokens.len()];

    if roll < 0.33 {
        send_sample(
            f,
            tx,
            "GET /doctors",
            timed_get(f, "/api/v1/doctors", token).await,
        );
    } else if roll < 0.53 {
        let didx = f.rr_doctor.fetch_add(1, Ordering::Relaxed) % f.doctor_tokens.len().max(1);
        let doctor_id = doctor_id_for_index(f, didx).await;
        send_sample(
            f,
            tx,
            "GET /doctors/:id/slots",
            timed_get(f, &format!("/api/v1/doctors/{doctor_id}/slots"), token).await,
        );
    } else if roll < 0.63 {
        chat_action(f, tx, patient_idx).await;
    } else if roll < 0.71 {
        rtc_token_action(f, tx, patient_idx).await;
    } else if roll < 0.79 {
        booking_action(f, tx, patient_idx).await;
    } else if roll < 0.86 {
        let idx = f.rr_doctor.fetch_add(1, Ordering::Relaxed) % f.doctor_tokens.len().max(1);
        send_sample(
            f,
            tx,
            "GET wallet summary",
            timed_get(f, "/api/v1/doctor/wallet/summary", &f.doctor_tokens[idx]).await,
        );
    } else if roll < 0.91 {
        telemetry_action(f, tx, patient_idx).await;
    } else if roll < 0.95 {
        let idx = f.rr_admin.fetch_add(1, Ordering::Relaxed) % f.admin_tokens.len().max(1);
        send_sample(
            f,
            tx,
            "GET admin telemetry",
            timed_get(f, "/api/v1/admin/command/telemetry", &f.admin_tokens[idx]).await,
        );
    } else if roll < 0.98 {
        refresh_action(f, tx, patient_idx).await;
    } else {
        grievance_action(f, tx, patient_idx).await;
    }
}

/// Section 11's explicit requirement: capture requested-vs-achieved RPS and
/// latency every 10 seconds, so queue growth/saturation shows up as a trend,
/// not just a single end-of-run average that could hide a mid-run collapse.
fn report_time_series(samples: &[Sample], test_start: Instant, duration_secs: u64) {
    const BUCKET_SECS: u64 = 10;
    let num_buckets = duration_secs.div_ceil(BUCKET_SECS).max(1) as usize;
    let mut buckets: Vec<Vec<Duration>> = vec![Vec::new(); num_buckets];
    let mut error_buckets: Vec<usize> = vec![0; num_buckets];

    for s in samples {
        let offset_secs = s.sent_at.saturating_duration_since(test_start).as_secs();
        let idx = ((offset_secs / BUCKET_SECS) as usize).min(num_buckets - 1);
        buckets[idx].push(s.elapsed);
        if Outcome::classify(s.status) == Outcome::SystemError {
            error_buckets[idx] += 1;
        }
    }

    println!("--- Time series (10s buckets) ---");
    println!(
        "{:>10} {:>12} {:>8} {:>8} {:>8} {:>8}",
        "window_s", "achieved_rps", "p50_ms", "p95_ms", "p99_ms", "5xx"
    );
    for (i, durs) in buckets.iter().enumerate() {
        let mut durs = durs.clone();
        durs.sort();
        let n = durs.len();
        let rps = n as f64 / BUCKET_SECS as f64;
        let (p50, p95, p99) = if n == 0 {
            (0, 0, 0)
        } else {
            (
                durs[n * 50 / 100].as_millis(),
                durs[(n * 95 / 100).min(n - 1)].as_millis(),
                durs[(n * 99 / 100).min(n - 1)].as_millis(),
            )
        };
        println!(
            "{:>10} {:>12.1} {:>8} {:>8} {:>8} {:>8}",
            format!(
                "{}-{}",
                i as u64 * BUCKET_SECS,
                (i as u64 + 1) * BUCKET_SECS
            ),
            rps,
            p50,
            p95,
            p99,
            error_buckets[i]
        );
    }
    println!();
}

async fn dispatch_one(f: &Fixtures, tx: mpsc::UnboundedSender<Sample>) {
    let roll: f64 = {
        use rand::Rng;
        rand::thread_rng().gen::<f64>()
    };

    if roll < 0.33 {
        let idx = f.rr_patient.fetch_add(1, Ordering::Relaxed) % f.patient_tokens.len();
        send_sample(
            f,
            &tx,
            "GET /doctors",
            timed_get(f, "/api/v1/doctors", &f.patient_tokens[idx]).await,
        );
    } else if roll < 0.53 {
        let idx = f.rr_patient.fetch_add(1, Ordering::Relaxed) % f.patient_tokens.len();
        let didx = f.rr_doctor.fetch_add(1, Ordering::Relaxed) % f.doctor_tokens.len().max(1);
        let doctor_id = doctor_id_for_index(f, didx).await;
        send_sample(
            f,
            &tx,
            "GET /doctors/:id/slots",
            timed_get(
                f,
                &format!("/api/v1/doctors/{doctor_id}/slots"),
                &f.patient_tokens[idx],
            )
            .await,
        );
    } else if roll < 0.63 {
        let idx = f.rr_patient.fetch_add(1, Ordering::Relaxed) % f.patient_tokens.len();
        chat_action(f, &tx, idx).await;
    } else if roll < 0.71 {
        let idx = f.rr_patient.fetch_add(1, Ordering::Relaxed) % f.patient_tokens.len();
        rtc_token_action(f, &tx, idx).await;
    } else if roll < 0.79 {
        let idx = f.rr_patient.fetch_add(1, Ordering::Relaxed) % f.patient_tokens.len();
        booking_action(f, &tx, idx).await;
    } else if roll < 0.86 {
        let idx = f.rr_doctor.fetch_add(1, Ordering::Relaxed) % f.doctor_tokens.len().max(1);
        send_sample(
            f,
            &tx,
            "GET wallet summary",
            timed_get(f, "/api/v1/doctor/wallet/summary", &f.doctor_tokens[idx]).await,
        );
    } else if roll < 0.91 {
        let idx = f.rr_patient.fetch_add(1, Ordering::Relaxed) % f.patient_tokens.len();
        telemetry_action(f, &tx, idx).await;
    } else if roll < 0.95 {
        let idx = f.rr_admin.fetch_add(1, Ordering::Relaxed) % f.admin_tokens.len().max(1);
        send_sample(
            f,
            &tx,
            "GET admin telemetry",
            timed_get(f, "/api/v1/admin/command/telemetry", &f.admin_tokens[idx]).await,
        );
    } else if roll < 0.98 {
        let idx = f.rr_patient.fetch_add(1, Ordering::Relaxed) % f.patient_tokens.len();
        refresh_action(f, &tx, idx).await;
    } else {
        let idx = f.rr_patient.fetch_add(1, Ordering::Relaxed) % f.patient_tokens.len();
        grievance_action(f, &tx, idx).await;
    }
}

async fn doctor_id_for_index(f: &Fixtures, idx: usize) -> Uuid {
    // Doctor *profile* id is needed for the slots route, not the user id we
    // minted tokens from — look it up once via the available_slots pool's
    // own doctor_id values (already profile ids) when possible, else fall
    // back to the first available slot's doctor.
    let slots = f.available_slots.lock().await;
    slots
        .get(idx % slots.len().max(1))
        .map(|(_, d)| *d)
        .unwrap_or(Uuid::nil())
}

async fn next_slot(f: &Fixtures) -> Option<(Uuid, Uuid)> {
    let idx = f.slot_cursor.fetch_add(1, Ordering::Relaxed);
    let slots = f.available_slots.lock().await;
    slots.get(idx).copied()
}

/// Full booking → webhook-confirm chain (mirrors the proven pattern in
/// load_test.rs/booking_concurrency_test.rs), and — unlike those — also
/// stores the resulting (appointment_id, conversation_id) into this virtual
/// patient's own runtime state, so the chat/rtc-token/telemetry mix buckets
/// have something real to act on instead of silently falling back to filler
/// reads for the rest of the run.
async fn booking_action(f: &Fixtures, tx: &mpsc::UnboundedSender<Sample>, patient_idx: usize) {
    if f.booking_attempts[patient_idx].fetch_add(1, Ordering::Relaxed)
        >= MAX_BOOKING_ATTEMPTS_PER_PATIENT
    {
        // Stay safely under the real 10/hour/user_id `check_booking_limit` —
        // substitute a harmless read rather than let this patient's own
        // quota (shared across every test stage run within the same hour)
        // produce artificial 429s that look like a capacity failure.
        send_sample(
            f,
            tx,
            "GET /doctors",
            timed_get(f, "/api/v1/doctors", &f.patient_tokens[patient_idx]).await,
        );
        return;
    }
    let Some((slot_id, doctor_id)) = next_slot(f).await else {
        send_sample(
            f,
            tx,
            "GET /doctors",
            timed_get(f, "/api/v1/doctors", &f.patient_tokens[patient_idx]).await,
        );
        return;
    };
    let token = f.patient_tokens[patient_idx].clone();

    let start = Instant::now();
    let Ok(res) = f
        .client
        .post(format!("{}/api/v1/appointments/book", f.base_url))
        .bearer_auth(&token)
        .header("Idempotency-Key", Uuid::new_v4().to_string())
        .json(&json!({
            "doctor_id": doctor_id, "slot_id": slot_id, "modality": "VIDEO",
            "gateway": "BKASH", "chief_complaint": "Capacity-test booking"
        }))
        .send()
        .await
    else {
        let _ = tx.send(Sample {
            label: "POST /appointments/book",
            elapsed: start.elapsed(),
            status: 0,
            sent_at: start,
        });
        return;
    };
    let status = res.status().as_u16();
    let elapsed = start.elapsed();
    let ok = res.status().is_success();
    let body: Option<serde_json::Value> = res.json().await.ok();
    let _ = tx.send(Sample {
        label: "POST /appointments/book",
        elapsed,
        status,
        sent_at: start,
    });
    if !ok {
        return;
    }
    let Some(body) = body else { return };
    let Some(appointment_id) = body["data"]["appointment_id"]
        .as_str()
        .and_then(|s| Uuid::parse_str(s).ok())
    else {
        return;
    };
    let Some(payment_session_id) = body["data"]["payment_session_id"].as_str() else {
        return;
    };

    let webhook_body = json!({
        "payment_session_id": payment_session_id,
        "transaction_reference": format!("CAP-{}", Uuid::new_v4()),
        "status": "SUCCESS",
        "event_timestamp": Utc::now().to_rfc3339(),
        "provider_event_id": Uuid::new_v4().to_string(),
    });
    let body_str = serde_json::to_string(&webhook_body).unwrap();
    let sig = hmac_sign(&body_str, &f.webhook_secret);
    let wh_start = Instant::now();
    let wh_status = f
        .client
        .post(format!("{}/api/v1/webhooks/payment/bkash", f.base_url))
        .header("Content-Type", "application/json")
        .header("X-Signature", sig)
        .body(body_str)
        .send()
        .await
        .map(|r| r.status().as_u16())
        .unwrap_or(0);
    let _ = tx.send(Sample {
        label: "POST webhook confirm",
        elapsed: wh_start.elapsed(),
        status: wh_status,
        sent_at: wh_start,
    });
    if wh_status != 200 {
        return;
    }

    let conv_id: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM chat_conversations WHERE appointment_id = $1")
            .bind(appointment_id)
            .fetch_optional(&f.pg)
            .await
            .ok()
            .flatten();

    let mut state = f.patient_state[patient_idx].lock().await;
    state.booked_appointment = Some((appointment_id, conv_id.unwrap_or(Uuid::nil())));
}

fn hmac_sign(body: &str, secret: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(body.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

async fn chat_action(f: &Fixtures, tx: &mpsc::UnboundedSender<Sample>, patient_idx: usize) {
    let conv = { f.patient_state[patient_idx].lock().await.booked_appointment };
    let Some((_, conv_id)) = conv else {
        send_sample(
            f,
            tx,
            "GET /doctors",
            timed_get(f, "/api/v1/doctors", &f.patient_tokens[patient_idx]).await,
        );
        return;
    };
    if conv_id.is_nil() {
        send_sample(
            f,
            tx,
            "GET /doctors",
            timed_get(f, "/api/v1/doctors", &f.patient_tokens[patient_idx]).await,
        );
        return;
    }
    send_sample(
        f,
        tx,
        "POST chat message",
        timed_post(
            f,
            &format!("/api/v1/chat/{conv_id}/messages"),
            &f.patient_tokens[patient_idx],
            &json!({ "content": "Capacity-test message" }),
        )
        .await,
    );
}

async fn rtc_token_action(f: &Fixtures, tx: &mpsc::UnboundedSender<Sample>, patient_idx: usize) {
    let apt = { f.patient_state[patient_idx].lock().await.booked_appointment };
    let Some((apt_id, _)) = apt else {
        send_sample(
            f,
            tx,
            "GET /doctors",
            timed_get(f, "/api/v1/doctors", &f.patient_tokens[patient_idx]).await,
        );
        return;
    };
    send_sample(
        f,
        tx,
        "POST rtc-token",
        timed_post(
            f,
            &format!("/api/v1/consultations/{apt_id}/rtc-token"),
            &f.patient_tokens[patient_idx],
            &json!({}),
        )
        .await,
    );
}

async fn telemetry_action(f: &Fixtures, tx: &mpsc::UnboundedSender<Sample>, patient_idx: usize) {
    let apt = { f.patient_state[patient_idx].lock().await.booked_appointment };
    let Some((apt_id, _)) = apt else {
        send_sample(
            f,
            tx,
            "GET /doctors",
            timed_get(f, "/api/v1/doctors", &f.patient_tokens[patient_idx]).await,
        );
        return;
    };
    send_sample(
        f,
        tx,
        "POST telemetry",
        timed_post(
            f,
            &format!("/api/v1/consultations/{apt_id}/telemetry"),
            &f.patient_tokens[patient_idx],
            &json!({ "call_duration_seconds": 300, "connection_state": "CONNECTED", "prescription_issued": false }),
        )
        .await,
    );
}

async fn refresh_action(f: &Fixtures, tx: &mpsc::UnboundedSender<Sample>, patient_idx: usize) {
    let refresh = {
        f.patient_state[patient_idx]
            .lock()
            .await
            .refresh_token
            .clone()
    };
    let Some(refresh_token) = refresh else {
        send_sample(
            f,
            tx,
            "GET /doctors",
            timed_get(f, "/api/v1/doctors", &f.patient_tokens[patient_idx]).await,
        );
        return;
    };
    send_sample(
        f,
        tx,
        "POST auth/refresh",
        timed_post(
            f,
            "/api/v1/auth/refresh",
            "",
            &json!({ "refresh_token": refresh_token }),
        )
        .await,
    );
}

async fn grievance_action(f: &Fixtures, tx: &mpsc::UnboundedSender<Sample>, patient_idx: usize) {
    let Some(consultation_id) = f.patient_consultation_ids[patient_idx] else {
        send_sample(
            f,
            tx,
            "GET /doctors",
            timed_get(f, "/api/v1/doctors", &f.patient_tokens[patient_idx]).await,
        );
        return;
    };
    send_sample(
        f,
        tx,
        "POST grievance",
        timed_post(
            f,
            "/api/v1/grievances",
            &f.patient_tokens[patient_idx],
            &json!({
                "consultation_id": consultation_id, "target": "SYSTEM",
                "category": "SERVICE_QUALITY", "claim_summary": "Capacity-test synthetic grievance"
            }),
        )
        .await,
    );
}

fn send_sample(
    _f: &Fixtures,
    tx: &mpsc::UnboundedSender<Sample>,
    label: &'static str,
    sample: (u16, Duration, Instant),
) {
    let _ = tx.send(Sample {
        label,
        elapsed: sample.1,
        status: sample.0,
        sent_at: sample.2,
    });
}

async fn timed_get(f: &Fixtures, path: &str, token: &str) -> (u16, Duration, Instant) {
    let start = Instant::now();
    let status = f
        .client
        .get(format!("{}{}", f.base_url, path))
        .bearer_auth(token)
        .send()
        .await
        .map(|r| r.status().as_u16())
        .unwrap_or(0);
    (status, start.elapsed(), start)
}

async fn timed_post(
    f: &Fixtures,
    path: &str,
    token: &str,
    body: &serde_json::Value,
) -> (u16, Duration, Instant) {
    let start = Instant::now();
    let mut req = f.client.post(format!("{}{}", f.base_url, path)).json(body);
    if !token.is_empty() {
        req = req.bearer_auth(token);
    }
    let status = req.send().await.map(|r| r.status().as_u16()).unwrap_or(0);
    (status, start.elapsed(), start)
}

#[derive(Default)]
struct EndpointAgg {
    total: usize,
    success: usize,
    business_reject: usize,
    auth_reject: usize,
    rate_limited: usize,
    system_error: usize,
    latencies: Vec<Duration>,
}

fn report(samples: &[Sample], duration_secs: u64, target_rps: f64, requested_count: u64) {
    let _ = (duration_secs, target_rps, requested_count);
    let mut by_label: HashMap<&str, EndpointAgg> = HashMap::new();
    let mut total_business_reject = 0usize;
    let mut total_auth_reject = 0usize;
    let mut total_rate_limited = 0usize;
    let mut total_system_error = 0usize;
    let total = samples.len();

    for s in samples {
        let outcome = Outcome::classify(s.status);
        let agg = by_label.entry(s.label).or_default();
        agg.total += 1;
        agg.latencies.push(s.elapsed);
        match outcome {
            Outcome::Success => agg.success += 1,
            Outcome::BusinessReject => {
                agg.business_reject += 1;
                total_business_reject += 1;
            }
            Outcome::AuthReject => {
                agg.auth_reject += 1;
                total_auth_reject += 1;
            }
            Outcome::RateLimited => {
                agg.rate_limited += 1;
                total_rate_limited += 1;
            }
            Outcome::SystemError => {
                agg.system_error += 1;
                total_system_error += 1;
            }
        }
    }

    println!(
        "BUSINESS_REJECTION_RATE={:.3}%  TRUE_SYSTEM_ERROR_RATE={:.4}%  auth_reject={:.3}%  rate_limited={:.3}%\n",
        total_business_reject as f64 / total.max(1) as f64 * 100.0,
        total_system_error as f64 / total.max(1) as f64 * 100.0,
        total_auth_reject as f64 / total.max(1) as f64 * 100.0,
        total_rate_limited as f64 / total.max(1) as f64 * 100.0,
    );

    println!(
        "{:<24} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>7} {:>7} {:>7} {:>7}",
        "endpoint",
        "total",
        "succ",
        "bizrej",
        "authrej",
        "ratelim",
        "syserr",
        "p50",
        "p95",
        "p99",
        "max"
    );
    let mut labels: Vec<&&str> = by_label.keys().collect();
    labels.sort();
    for label in labels {
        let agg = &by_label[*label];
        let mut durs = agg.latencies.clone();
        durs.sort();
        let n = durs.len();
        let (p50, p95, p99, max) = if n == 0 {
            (0, 0, 0, 0)
        } else {
            (
                durs[n * 50 / 100].as_millis(),
                durs[(n * 95 / 100).min(n - 1)].as_millis(),
                durs[(n * 99 / 100).min(n - 1)].as_millis(),
                durs[n - 1].as_millis(),
            )
        };
        println!(
            "{:<24} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>7} {:>7} {:>7} {:>7}",
            label,
            agg.total,
            agg.success,
            agg.business_reject,
            agg.auth_reject,
            agg.rate_limited,
            agg.system_error,
            p50,
            p95,
            p99,
            max
        );
    }
    let _ = Utc::now();
}
