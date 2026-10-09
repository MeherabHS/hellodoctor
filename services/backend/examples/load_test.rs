//! Realistic multi-endpoint load test against a *running* `cargo run` server
//! (not a unit/integration test — this hits real HTTP over the network stack,
//! same as a real client would). Exercises the flows named in the user's
//! load-testing brief: login, directory, slots, booking + payment init, RTC
//! token issuance, chat, wallet reads, and an admin/"queue" read — not just
//! `/healthz`.
//!
//! Usage (from services/backend/, with `cargo run` already serving on :8080,
//! and `docker compose up -d` providing Postgres/Redis):
//!     cargo run --release --example load_test
//!
//! Env overrides: LOAD_TEST_CONCURRENCY (default 50), LOAD_TEST_DURATION_SECS
//! (default 45), LOAD_TEST_PATIENTS (default = concurrency, one distinct patient
//! identity per concurrent worker, each provisioned via a real OTP register+verify
//! call before the timed window starts).
//!
//! IMPORTANT CAVEAT, not to be lost in the numbers this prints: this runs
//! against a local development machine, not representative production
//! infrastructure. Treat the output as "does the architecture behave
//! sanely under concurrent load and where does it bend first", not as a
//! production capacity number.

use chrono::{Duration as ChronoDuration, Utc};
use rand::Rng;
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use uuid::Uuid;

const BASE_URL: &str = "http://localhost:8080";
// Safely under the real `check_booking_limit` cap of 10/hour/user_id, leaving margin
// for the fact that a 409 (lost a slot race) still consumes one unit of that budget.
const MAX_BOOKINGS_PER_VIRTUAL_USER: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Outcome {
    Success,
    BusinessReject,
    AuthReject,
    RateLimited,
    SystemError,
}

impl Outcome {
    /// `0` is this harness's own sentinel for "request never reached the server"
    /// (connection refused/timeout/transport error) — classified as a system
    /// error since, on a loopback test against our own server, that's exactly
    /// what it is, not a legitimate business outcome.
    fn classify(status: u16) -> Outcome {
        match status {
            200..=299 => Outcome::Success,
            401 | 403 => Outcome::AuthReject,
            429 => Outcome::RateLimited,
            500..=599 | 0 => Outcome::SystemError,
            _ => Outcome::BusinessReject, // 400, 404, 409, 422, etc.
        }
    }
}

#[derive(Debug, Clone)]
struct Sample {
    label: &'static str,
    elapsed: Duration,
    status: u16,
}

struct VirtualUser {
    token: String,
    booked_appointments: Vec<(Uuid, Uuid)>, // (appointment_id, conversation_id-or-nil)
    // Counts every booking *attempt*, success or not — `check_booking_limit` on the
    // server charges its 10/hour quota per attempt (a lost-slot-race 409 still costs
    // a unit), so capping on `booked_appointments.len()` alone (successes only) lets
    // a user who keeps losing slot races keep attempting and exhaust the real quota
    // entirely on 409s before this cap ever has a chance to trip.
    booking_attempts: usize,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let concurrency: usize = env::var("LOAD_TEST_CONCURRENCY")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(50);
    let duration_secs: u64 = env::var("LOAD_TEST_DURATION_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(45);
    // Defaults to `concurrency`, not a flat number: each concurrent worker is meant to
    // model one distinct real user, and `check_booking_limit` is a real, deliberately
    // un-weakened per-account anti-abuse cap (10 bookings/hour/user_id) — sharing one
    // patient identity across multiple workers means several workers hammer the same
    // account's booking budget at once and trip it almost immediately, which looks
    // like a capacity failure but is actually just too few distinct test identities.
    let num_patients: usize = env::var("LOAD_TEST_PATIENTS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(concurrency);

    println!("=== HelloDoctor Load Test ===");
    println!("concurrency={concurrency} duration={duration_secs}s patients={num_patients}");
    println!("Target: {BASE_URL}  (local dev machine — see module doc caveat)\n");

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();

    // ---- Setup: provision enough slots that booking doesn't starve under load ----
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set (see .env)");
    let pg = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("connect to Postgres for slot provisioning");

    let doctor_ids = fetch_doctor_ids(&pg).await;
    if doctor_ids.is_empty() {
        eprintln!("No doctors found — run with SEED_DEV_DATA=true at least once first.");
        std::process::exit(1);
    }
    let slots_per_doctor = (concurrency * 20).max(500);
    provision_slots(&pg, &doctor_ids, slots_per_doctor).await;
    println!(
        "Provisioned {} slots across {} doctor(s).",
        slots_per_doctor * doctor_ids.len(),
        doctor_ids.len()
    );

    // ---- Setup: real doctor/admin tokens (minted via the real signing key,
    // equivalent in shape to what a real login would hand back) ----
    let jwt_private_key =
        env::var("JWT_ED25519_PRIVATE_KEY_B64").expect("JWT_ED25519_PRIVATE_KEY_B64 must be set");
    // The JWT subject must be the doctor's `users.id`, not `doctor_profiles.id` —
    // every handler resolves `doctor_profiles` by `user_id = ctx.user_id`, so minting
    // with the profile id here silently 404s every doctor-scoped request (confirmed:
    // this made 100% of "GET wallet summary" requests fail in an earlier run).
    let doctor_user_id = fetch_doctor_user_id(&pg, doctor_ids[0]).await;
    sqlx::query("INSERT INTO auth_sessions(id,user_id,token_family_id,refresh_token_hash,created_at,last_used_at,expires_at) SELECT gen_random_uuid(),id,id,'load-fixture-session-'||id::text,NOW(),NOW(),NOW()+INTERVAL '7 days' FROM users WHERE id=$1 OR id=$2 ON CONFLICT DO NOTHING")
        .bind(doctor_user_id).bind(Uuid::parse_str(hellodoctor_backend::seed::ADMIN_USER_ID).unwrap()).execute(&pg).await.expect("synthetic load-test sessions");
    let doctor_token = hellodoctor_backend::crypto::create_access_token(
        doctor_user_id,
        "DOCTOR",
        doctor_user_id,
        &jwt_private_key,
    )
    .unwrap();
    let admin_user_id = Uuid::parse_str(hellodoctor_backend::seed::ADMIN_USER_ID).unwrap();
    let admin_token = hellodoctor_backend::crypto::create_access_token(
        admin_user_id,
        "PLATFORM_ADMIN",
        admin_user_id,
        &jwt_private_key,
    )
    .unwrap();

    // ---- Setup: provision N real patients via the actual OTP login flow —
    // this both sets up virtual users AND gives us a real measurement of
    // login-flow latency under the same conditions as the main run. ----
    println!("Provisioning {num_patients} patients via real OTP login flow...");
    let mut patient_tokens = Vec::with_capacity(num_patients);
    let mut login_samples = Vec::new();
    // Salted by wall-clock time, not a fixed base, so repeated runs within the same
    // `check_otp_request_limit` window (3 requests / phone / 10 minutes) don't reuse
    // the prior run's phone numbers and get silently capped down to however many
    // hadn't yet hit that quota (confirmed: a same-session rerun provisioned only
    // 10/50 patients this way before this fix).
    let phone_salt = 100_000_000 + (Utc::now().timestamp() as u64 % 700_000_000);
    for i in 0..num_patients {
        let phone = format!("+2547{:08}", (phone_salt + i as u64) % 100_000_000);
        if let Some((token, samples)) = real_patient_login(&client, &phone).await {
            patient_tokens.push(token);
            login_samples.extend(samples);
        }
    }
    println!("Provisioned {} patient sessions.\n", patient_tokens.len());

    if patient_tokens.is_empty() {
        eprintln!("No patient sessions could be provisioned — aborting.");
        std::process::exit(1);
    }
    // Never run more concurrent workers than distinct provisioned patients: each
    // worker is meant to model one real concurrent user, and `worker_id %
    // patient_tokens.len()` would otherwise silently hand the same account to
    // several workers, multiplying their combined hits against that one account's
    // real `check_booking_limit` quota well past what the per-worker cap accounts for.
    let concurrency = if concurrency > patient_tokens.len() {
        eprintln!(
            "Only {} of {num_patients} requested patients were provisioned — \
             clamping concurrency down to match so no account is shared across workers.",
            patient_tokens.len()
        );
        patient_tokens.len()
    } else {
        concurrency
    };

    // ---- Timed load phase ----
    let samples: Arc<Mutex<Vec<Sample>>> = Arc::new(Mutex::new(login_samples));
    let deadline = Instant::now() + Duration::from_secs(duration_secs);
    let doctor_ids = Arc::new(doctor_ids);

    println!("Running timed load for {duration_secs}s at concurrency={concurrency}...");
    let mut handles = Vec::new();
    for worker_id in 0..concurrency {
        let client = client.clone();
        let samples = samples.clone();
        let patient_token = patient_tokens[worker_id % patient_tokens.len()].clone();
        let doctor_token = doctor_token.clone();
        let admin_token = admin_token.clone();
        let doctor_ids = doctor_ids.clone();
        let pg_for_lookup = pg.clone();

        handles.push(tokio::spawn(async move {
            let mut vu = VirtualUser {
                token: patient_token,
                booked_appointments: Vec::new(),
                booking_attempts: 0,
            };
            let mut local_samples = Vec::new();

            while Instant::now() < deadline {
                // A fresh `ThreadRng` per iteration rather than one held
                // across the loop: `ThreadRng` is `!Send`, and a value held
                // live across an `.await` point inside a spawned task must
                // be `Send` — recreating it each iteration sidesteps that
                // without switching to a different RNG type.
                let (doctor_id, roll) = {
                    let mut rng = rand::thread_rng();
                    (
                        doctor_ids[rng.gen_range(0..doctor_ids.len())],
                        rng.gen::<f64>(),
                    )
                };

                if roll < 0.30 {
                    local_samples.push(
                        timed_get(&client, "GET /doctors", "/api/v1/doctors", &vu.token).await,
                    );
                } else if roll < 0.50 {
                    local_samples.push(
                        timed_get(
                            &client,
                            "GET /doctors/:id/slots",
                            &format!("/api/v1/doctors/{doctor_id}/slots"),
                            &vu.token,
                        )
                        .await,
                    );
                } else if roll < 0.60 {
                    // `check_booking_limit` is a real, deliberately un-weakened per-account
                    // cap of 10 bookings/hour/user_id (see rate_limit.rs) — a tight closed
                    // loop re-rolling this branch for the whole run would blow through that
                    // budget in seconds regardless of how many distinct patients exist,
                    // which measures the abuse guard firing correctly, not booking capacity.
                    // Capping attempts per virtual user well under the real limit, and
                    // falling back to a realistic read, keeps this endpoint's numbers
                    // meaningful instead of ~100% 429.
                    if vu.booking_attempts < MAX_BOOKINGS_PER_VIRTUAL_USER {
                        vu.booking_attempts += 1;
                        book_and_pay(
                            &client,
                            &pg_for_lookup,
                            &vu.token,
                            doctor_id,
                            &mut vu.booked_appointments,
                            &mut local_samples,
                        )
                        .await;
                    } else {
                        local_samples.push(
                            timed_get(&client, "GET /doctors", "/api/v1/doctors", &vu.token).await,
                        );
                    }
                } else if roll < 0.70 {
                    if let Some((apt_id, _)) = vu.booked_appointments.last() {
                        local_samples.push(
                            timed_post(
                                &client,
                                "POST rtc-token",
                                &format!("/api/v1/consultations/{apt_id}/rtc-token"),
                                &vu.token,
                                &json!({}),
                            )
                            .await,
                        );
                    }
                } else if roll < 0.80 {
                    if let Some((_, conv_id)) = vu.booked_appointments.last() {
                        if !conv_id.is_nil() {
                            local_samples.push(
                                timed_post(
                                    &client,
                                    "POST chat message",
                                    &format!("/api/v1/chat/{conv_id}/messages"),
                                    &vu.token,
                                    &json!({ "content": "Load-test message" }),
                                )
                                .await,
                            );
                        }
                    }
                } else if roll < 0.90 {
                    local_samples.push(
                        timed_get(
                            &client,
                            "GET wallet summary",
                            "/api/v1/doctor/wallet/summary",
                            &doctor_token,
                        )
                        .await,
                    );
                } else {
                    local_samples.push(
                        timed_get(
                            &client,
                            "GET admin telemetry (queue proxy)",
                            "/api/v1/admin/command/telemetry",
                            &admin_token,
                        )
                        .await,
                    );
                }
            }

            let mut guard = samples.lock().await;
            guard.extend(local_samples);
        }));
    }

    for h in handles {
        let _ = h.await;
    }

    let samples = Arc::try_unwrap(samples).unwrap().into_inner();
    report(&samples, duration_secs);
}

async fn fetch_doctor_ids(pg: &sqlx::PgPool) -> Vec<Uuid> {
    sqlx::query_scalar::<_, Uuid>("SELECT id FROM doctor_profiles WHERE deleted_at IS NULL")
        .fetch_all(pg)
        .await
        .unwrap_or_default()
}

async fn fetch_doctor_user_id(pg: &sqlx::PgPool, doctor_profile_id: Uuid) -> Uuid {
    sqlx::query_scalar::<_, Uuid>("SELECT user_id FROM doctor_profiles WHERE id = $1")
        .bind(doctor_profile_id)
        .fetch_one(pg)
        .await
        .expect("doctor_profiles.user_id lookup for the load-test doctor token")
}

async fn provision_slots(pg: &sqlx::PgPool, doctor_ids: &[Uuid], count_per_doctor: usize) {
    let mut tx = pg.begin().await.unwrap();
    // Anchor far in the future so these never collide with anything a human
    // tester books interactively around "now".
    let anchor = Utc::now() + ChronoDuration::days(365);
    for &doctor_id in doctor_ids {
        for i in 0..count_per_doctor {
            let start = anchor + ChronoDuration::minutes(20 * i as i64);
            let end = start + ChronoDuration::minutes(15);
            // ON CONFLICT ON CONSTRAINT, not a plain INSERT, because re-running this
            // harness on the same day lands the same `now() + 365d` anchor window as
            // a prior run and collides with `no_overlapping_slots` otherwise — this
            // makes provisioning idempotent across repeated runs instead of panicking.
            sqlx::query(
                "INSERT INTO doctor_schedule_slots (id, doctor_id, start_time, end_time, status, created_at) \
                 VALUES ($1, $2, $3, $4, 'AVAILABLE', now()) \
                 ON CONFLICT ON CONSTRAINT no_overlapping_slots DO NOTHING",
            )
            .bind(Uuid::new_v4())
            .bind(doctor_id)
            .bind(start)
            .bind(end)
            .execute(&mut *tx)
            .await
            .unwrap();
        }
    }
    tx.commit().await.unwrap();
}

async fn real_patient_login(
    client: &reqwest::Client,
    phone: &str,
) -> Option<(String, Vec<Sample>)> {
    let mut samples = Vec::new();

    let start = Instant::now();
    let res = client
        .post(format!("{BASE_URL}/api/v1/auth/register-otp"))
        .json(&json!({ "phone_number": phone }))
        .send()
        .await
        .ok()?;
    let status = res.status().as_u16();
    let ok = res.status().is_success();
    let body: serde_json::Value = res.json().await.ok()?;
    samples.push(Sample {
        label: "POST register-otp",
        elapsed: start.elapsed(),
        status,
    });
    if !ok {
        return None;
    }
    let code = body["data"]["debug_otp_code"].as_str()?.to_string();

    let start = Instant::now();
    let res = client
        .post(format!("{BASE_URL}/api/v1/auth/verify-otp-and-login"))
        .json(&json!({ "phone_number": phone, "otp_code": code }))
        .send()
        .await
        .ok()?;
    let status = res.status().as_u16();
    let ok = res.status().is_success();
    let body: serde_json::Value = res.json().await.ok()?;
    samples.push(Sample {
        label: "POST verify-otp-login",
        elapsed: start.elapsed(),
        status,
    });
    if !ok {
        return None;
    }
    let token = body["data"]["access_token"].as_str()?.to_string();
    Some((token, samples))
}

async fn book_and_pay(
    client: &reqwest::Client,
    pg: &sqlx::PgPool,
    token: &str,
    doctor_id: Uuid,
    booked: &mut Vec<(Uuid, Uuid)>,
    samples: &mut Vec<Sample>,
) {
    let start = Instant::now();
    let Ok(res) = client
        .get(format!("{BASE_URL}/api/v1/doctors/{doctor_id}/slots"))
        .bearer_auth(token)
        .send()
        .await
    else {
        samples.push(Sample {
            label: "GET /doctors/:id/slots",
            elapsed: start.elapsed(),
            status: 0,
        });
        return;
    };
    let status = res.status().as_u16();
    samples.push(Sample {
        label: "GET /doctors/:id/slots",
        elapsed: start.elapsed(),
        status,
    });
    let Ok(body) = res.json::<serde_json::Value>().await else {
        return;
    };
    let Some(slot_id) = body["data"]
        .as_array()
        .and_then(|a| a.first())
        .and_then(|s| s["id"].as_str())
    else {
        return;
    };

    let start = Instant::now();
    let Ok(res) = client
        .post(format!("{BASE_URL}/api/v1/appointments/book"))
        .bearer_auth(token)
        .header("Idempotency-Key", Uuid::new_v4().to_string())
        .json(&json!({ "doctor_id": doctor_id, "slot_id": slot_id, "modality": "VIDEO", "gateway": "BKASH", "chief_complaint": "Load-test booking" }))
        .send()
        .await
    else {
        samples.push(Sample {
            label: "POST /appointments/book",
            elapsed: start.elapsed(),
            status: 0,
        });
        return;
    };
    let status = res.status().as_u16();
    let ok = res.status().is_success();
    samples.push(Sample {
        label: "POST /appointments/book",
        elapsed: start.elapsed(),
        status,
    });
    if !ok {
        // Expected under real concurrency: two virtual users racing for the
        // same slot produces a genuine 409, which is correct behavior, not a
        // load-test bug — it's counted as an error for this label on purpose.
        return;
    }
    let Ok(book_body) = res.json::<serde_json::Value>().await else {
        return;
    };
    let Some(apt_id) = book_body["data"]["appointment_id"]
        .as_str()
        .and_then(|s| Uuid::parse_str(s).ok())
    else {
        return;
    };
    let Some(payment_session_id) = book_body["data"]["payment_session_id"].as_str() else {
        return;
    };

    let webhook_secret = env::var("WEBHOOK_HMAC_SECRET").unwrap_or_default();
    let webhook_body = json!({
        "payment_session_id": payment_session_id,
        "transaction_reference": format!("LOADTEST-{}", Uuid::new_v4()),
        "status": "SUCCESS",
        "event_timestamp": Utc::now().to_rfc3339(),
        "provider_event_id": Uuid::new_v4().to_string(),
    });
    let body_str = serde_json::to_string(&webhook_body).unwrap();
    let sig = hmac_sign(&body_str, &webhook_secret);

    let start = Instant::now();
    let Ok(res) = client
        .post(format!("{BASE_URL}/api/v1/webhooks/payment/bkash"))
        .header("Content-Type", "application/json")
        .header("X-Signature", sig)
        .body(body_str)
        .send()
        .await
    else {
        samples.push(Sample {
            label: "POST webhook confirm",
            elapsed: start.elapsed(),
            status: 0,
        });
        return;
    };
    let status = res.status().as_u16();
    let ok = res.status().is_success();
    samples.push(Sample {
        label: "POST webhook confirm",
        elapsed: start.elapsed(),
        status,
    });
    if !ok {
        return;
    }

    // The webhook handler creates the chat conversation as a side effect —
    // look it up directly (this is test-harness orchestration, not something
    // a real client would do; a real client would discover it via its own
    // appointment-detail view).
    let conv_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM chat_conversations WHERE appointment_id = $1",
    )
    .bind(apt_id)
    .fetch_optional(pg)
    .await
    .ok()
    .flatten()
    .unwrap_or(Uuid::nil());

    booked.push((apt_id, conv_id));
}

fn hmac_sign(body: &str, secret: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(body.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

async fn timed_get(
    client: &reqwest::Client,
    label: &'static str,
    path: &str,
    token: &str,
) -> Sample {
    let start = Instant::now();
    let status = client
        .get(format!("{BASE_URL}{path}"))
        .bearer_auth(token)
        .send()
        .await
        .map(|r| r.status().as_u16())
        .unwrap_or(0);
    Sample {
        label,
        elapsed: start.elapsed(),
        status,
    }
}

async fn timed_post(
    client: &reqwest::Client,
    label: &'static str,
    path: &str,
    token: &str,
    body: &serde_json::Value,
) -> Sample {
    let start = Instant::now();
    let status = client
        .post(format!("{BASE_URL}{path}"))
        .bearer_auth(token)
        .json(body)
        .send()
        .await
        .map(|r| r.status().as_u16())
        .unwrap_or(0);
    Sample {
        label,
        elapsed: start.elapsed(),
        status,
    }
}

/// Outcome taxonomy, not a generic "errors" column: a `409` slot conflict and a
/// `500` unhandled panic are not the same kind of event, and collapsing them
/// into one "error" bucket (the previous format) makes it impossible to tell
/// "the architecture is rejecting demand correctly" apart from "the architecture
/// is broken" without manually grepping server logs after every run.
#[derive(Default)]
struct EndpointAgg {
    total: usize,
    success: usize,
    business_reject: usize,
    auth_reject: usize,
    rate_limited: usize,
    system_error: usize,
    latency_by_outcome: HashMap<Outcome, Vec<Duration>>,
}

fn report(samples: &[Sample], duration_secs: u64) {
    let mut by_label: HashMap<&str, EndpointAgg> = HashMap::new();
    let mut total = 0usize;
    let mut total_success = 0usize;
    let mut total_business_reject = 0usize;
    let mut total_auth_reject = 0usize;
    let mut total_rate_limited = 0usize;
    let mut total_system_error = 0usize;

    for s in samples {
        total += 1;
        let outcome = Outcome::classify(s.status);
        let agg = by_label.entry(s.label).or_default();
        agg.total += 1;
        agg.latency_by_outcome
            .entry(outcome)
            .or_default()
            .push(s.elapsed);
        match outcome {
            Outcome::Success => {
                agg.success += 1;
                total_success += 1;
            }
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

    println!("\n=== Results ===");
    println!("total_requests={total}");
    println!("overall_rps={:.1}", total as f64 / duration_secs as f64);
    println!(
        "success_rate={:.2}%",
        total_success as f64 / total.max(1) as f64 * 100.0
    );
    println!(
        "BUSINESS_REJECTION_RATE={:.2}%  (expected rejections — 409 slot conflict, 404/422 validation, etc. — NOT backend failures)",
        total_business_reject as f64 / total.max(1) as f64 * 100.0
    );
    println!(
        "TRUE_SYSTEM_ERROR_RATE={:.4}%  (5xx or unreachable only)",
        total_system_error as f64 / total.max(1) as f64 * 100.0
    );
    println!(
        "auth_reject_rate={:.2}%  rate_limited_rate={:.2}%\n",
        total_auth_reject as f64 / total.max(1) as f64 * 100.0,
        total_rate_limited as f64 / total.max(1) as f64 * 100.0
    );

    println!(
        "{:<32} {:>7} {:>7} {:>8} {:>8} {:>8} {:>8}",
        "endpoint", "total", "success", "bizrej", "authrej", "ratelim", "syserr"
    );
    let mut labels: Vec<&&str> = by_label.keys().collect();
    labels.sort();
    for label in &labels {
        let agg = &by_label[*label];
        println!(
            "{:<32} {:>7} {:>7} {:>8} {:>8} {:>8} {:>8}",
            label,
            agg.total,
            agg.success,
            agg.business_reject,
            agg.auth_reject,
            agg.rate_limited,
            agg.system_error
        );
    }

    println!("\n--- Per-outcome latency (ms) ---");
    println!(
        "{:<32} {:<15} {:>6} {:>7} {:>7} {:>7} {:>7}",
        "endpoint", "outcome", "n", "p50", "p95", "p99", "max"
    );
    let ordered_outcomes = [
        Outcome::Success,
        Outcome::BusinessReject,
        Outcome::AuthReject,
        Outcome::RateLimited,
        Outcome::SystemError,
    ];
    for label in &labels {
        let agg = &by_label[*label];
        for outcome in ordered_outcomes {
            let Some(durs) = agg.latency_by_outcome.get(&outcome) else {
                continue;
            };
            let mut durs = durs.clone();
            durs.sort();
            let n = durs.len();
            if n == 0 {
                continue;
            }
            let p50 = durs[n * 50 / 100].as_millis();
            let p95 = durs[(n * 95 / 100).min(n - 1)].as_millis();
            let p99 = durs[(n * 99 / 100).min(n - 1)].as_millis();
            let max = durs[n - 1].as_millis();
            println!(
                "{:<32} {:<15} {:>6} {:>7} {:>7} {:>7} {:>7}",
                label,
                format!("{outcome:?}"),
                n,
                p50,
                p95,
                p99,
                max
            );
        }
    }
}
