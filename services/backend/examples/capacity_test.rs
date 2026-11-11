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
//!
//! Milestone 8.0 session-ramp remediation (2026-10-31): the original
//! sessions mode started every one of `CAPACITY_ACTIVE_SESSIONS` tasks, and
//! had each dispatch its first action, at the same instant (t=0) — a real,
//! measured thundering-herd artifact (hundreds of 5xx in the first 10s
//! bucket alone, varying run to run) that is a property of this test
//! harness's startup, not of organic Patient/Doctor ramp-up, and inflated
//! headline error-rate figures in a way unrelated to whatever the run was
//! actually trying to measure. `CAPACITY_RAMP_SECS` (default 0, preserving
//! the exact original immediate-start behavior) spreads each session's
//! first start uniformly across a `[0, CAPACITY_RAMP_SECS]` window instead.
//! Two explicitly named profiles:
//!   REALISTIC_RAMP (sessions introduced gradually, realistic ongoing use):
//!     CAPACITY_MODE=sessions CAPACITY_PROFILE=REALISTIC_RAMP \
//!     CAPACITY_ACTIVE_SESSIONS=1200 CAPACITY_RAMP_SECS=120 \
//!     CAPACITY_THINK_MIN_SECS=5 CAPACITY_THINK_MAX_SECS=20 \
//!     CAPACITY_DURATION_SECS=300 cargo run --release --example capacity_test
//!   SIMULTANEOUS_BURST (explicit stress test — the original behavior, now
//!   named rather than silently the only option):
//!     CAPACITY_MODE=sessions CAPACITY_PROFILE=SIMULTANEOUS_BURST \
//!     CAPACITY_ACTIVE_SESSIONS=1200 CAPACITY_RAMP_SECS=0 \
//!     CAPACITY_THINK_MIN_SECS=5 CAPACITY_THINK_MAX_SECS=20 \
//!     CAPACITY_DURATION_SECS=300 cargo run --release --example capacity_test
//! A `SESSION_START` marker sample is recorded the instant each session
//! actually begins dispatching (after its ramp delay, if any), reusing the
//! existing sample/time-series plumbing rather than a parallel mechanism —
//! the printed time series's `sessions_active` column and the final
//! `sessions_started`/`sessions_never_started` line are both derived from
//! these markers, so "how many sessions were really concurrently active,
//! and when" is reported directly from observed events, never assumed equal
//! to `CAPACITY_ACTIVE_SESSIONS` or to the registered-patient count. A
//! session whose ramp-assigned start time falls after `CAPACITY_DURATION_SECS`
//! (only possible if `CAPACITY_RAMP_SECS` > `CAPACITY_DURATION_SECS`) never
//! starts at all and is counted in `sessions_never_started`, not silently
//! dropped.

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
use tokio::sync::{mpsc, Mutex, RwLock};
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
    patient_tokens: Vec<RwLock<(String, Instant)>>,
    patient_user_ids: Vec<Uuid>,
    patient_consultation_ids: Vec<Option<Uuid>>, // one pre-existing historical consultation per patient, for grievances
    patient_state: Vec<Mutex<PatientRuntimeState>>,
    booking_attempts: Vec<AtomicU32>, // per-patient-index counter, gates well under the real 10/hour limit
    doctor_tokens: Vec<RwLock<(String, Instant)>>,
    doctor_user_ids: Vec<Uuid>,
    admin_tokens: Vec<RwLock<(String, Instant)>>,
    admin_user_ids: Vec<Uuid>,
    jwt_private_key: String,
    available_slots: Mutex<Vec<(Uuid, Uuid)>>, // (slot_id, doctor_id), consumed front-to-back
    slot_cursor: AtomicUsize,
    rr_patient: AtomicUsize,
    rr_doctor: AtomicUsize,
    rr_admin: AtomicUsize,
    webhook_secret: String,
}

const MAX_BOOKING_ATTEMPTS_PER_PATIENT: u32 = 8;

// Milestone 8 Workstream 2 fix: the real backend's access-token TTL
// (hellodoctor_backend::crypto::ACCESS_TOKEN_TTL_SECONDS, private to that
// module -- this is this file's own mirror, keep in sync if that ever
// changes) is a genuine 15 minutes, correctly enforced. This harness
// previously minted every identity's token ONCE at startup into a plain
// immutable Vec<String> and never renewed it; `refresh_action` read a
// `patient_state.refresh_token` field that was NEVER SET anywhere (always
// None), so it silently fell back to a filler GET every single time and
// never actually exercised `/api/v1/auth/refresh`. Confirmed via a 40-minute
// run: ~15 minutes in, every token expired near-simultaneously and the
// backend correctly started returning real 401s for the rest of the run --
// a benchmark-tooling defect, not a backend one.
//
// Fix: re-mint locally via the same signing key this harness already
// legitimately holds (the same bypass already used for *initial*
// authentication, for the same reason -- this is a load generator, not a
// test of the login/argon2id path), proactively before the real TTL
// elapses, per identity, with a small index-derived jitter so ~2,000
// sessions minted at nearly the same instant don't all re-check in the
// exact same tick. This does NOT touch the backend's real TTL, the real
// `/api/v1/auth/refresh` endpoint's reuse-detection/device-proof rotation
// logic, or any production authentication code -- it only fixes the
// harness's own failure to ever renew what it hands to the backend.
// Deliberately NOT implemented: replicating the real refresh-token
// rotation protocol (crypto::hash_code-based, with family reuse detection
// and device-proof binding -- see src/services/auth_service.rs) would be
// real additional backend-security-sensitive surface to get exactly right
// under 2,000-way concurrency, and getting it subtly wrong would itself be
// a new failure mode. This harness still never exercises
// `/api/v1/auth/refresh` under load -- a disclosed, deliberate limitation,
// not a silent gap.
const ACCESS_TOKEN_TTL_SECS_MIRROR: u64 = 15 * 60;
const TOKEN_REFRESH_MARGIN_SECS: u64 = 180;

async fn fresh_token(
    pool: &[RwLock<(String, Instant)>],
    user_ids: &[Uuid],
    role: &str,
    jwt_private_key: &str,
    idx: usize,
) -> String {
    if pool.is_empty() {
        return String::new();
    }
    let i = idx % pool.len();
    // Deterministic per-identity jitter (0-119s) so a pool minted together
    // doesn't all cross its refresh threshold in the same instant.
    let jitter_secs = (i as u64 * 37) % 120;
    let refresh_at_elapsed = ACCESS_TOKEN_TTL_SECS_MIRROR
        .saturating_sub(TOKEN_REFRESH_MARGIN_SECS)
        .saturating_sub(jitter_secs);
    {
        let guard = pool[i].read().await;
        if guard.1.elapsed().as_secs() < refresh_at_elapsed {
            return guard.0.clone();
        }
    }
    let mut guard = pool[i].write().await;
    // Re-check: another task may have already refreshed while we waited.
    if guard.1.elapsed().as_secs() < refresh_at_elapsed {
        return guard.0.clone();
    }
    let new_token = create_access_token(user_ids[i], role, user_ids[i], jwt_private_key)
        .expect("re-mint access token");
    *guard = (new_token.clone(), Instant::now());
    new_token
}

async fn patient_token(f: &Fixtures, idx: usize) -> String {
    fresh_token(
        &f.patient_tokens,
        &f.patient_user_ids,
        "PATIENT",
        &f.jwt_private_key,
        idx,
    )
    .await
}
async fn doctor_token(f: &Fixtures, idx: usize) -> String {
    fresh_token(
        &f.doctor_tokens,
        &f.doctor_user_ids,
        "DOCTOR",
        &f.jwt_private_key,
        idx,
    )
    .await
}
async fn admin_token(f: &Fixtures, idx: usize) -> String {
    fresh_token(
        &f.admin_tokens,
        &f.admin_user_ids,
        "ADMIN",
        &f.jwt_private_key,
        idx,
    )
    .await
}

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
    let ramp_secs: u64 = env::var("CAPACITY_RAMP_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let profile = env::var("CAPACITY_PROFILE").unwrap_or_else(|_| {
        if ramp_secs > 0 {
            "REALISTIC_RAMP".to_string()
        } else {
            "SIMULTANEOUS_BURST".to_string()
        }
    });
    let database_url = env::var("CAPACITY_DATABASE_URL").unwrap_or_else(|_| {
        "postgres://hellodoctor:hellodoctor_dev_password@localhost:5532/hellodoctor".to_string()
    });
    let jwt_private_key =
        env::var("JWT_ED25519_PRIVATE_KEY_B64").expect("JWT_ED25519_PRIVATE_KEY_B64 must be set");
    let webhook_secret = env::var("WEBHOOK_HMAC_SECRET").unwrap_or_default();

    println!("=== HelloDoctor Constrained-VPS Capacity Test (open-loop) ===");
    println!("target={base_url} target_rps={target_rps} duration={duration_secs}s");
    if mode == "sessions" {
        if profile == "SIMULTANEOUS_BURST" {
            println!(
                "PROFILE={profile} (ramp_secs={ramp_secs}) -- THIS IS A STRESS/BURST TEST: \
                 every session starts at the same instant. Not representative of organic \
                 Patient/Doctor ramp-up; use REALISTIC_RAMP (CAPACITY_RAMP_SECS > 0) for a \
                 steady-state capacity conclusion.\n"
            );
        } else {
            println!(
                "PROFILE={profile} (ramp_secs={ramp_secs}) -- sessions are introduced \
                 gradually and uniformly over the ramp window.\n"
            );
        }
    } else {
        println!();
    }

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

    // Milestone 8 benchmark-methodology correction (2026-11-04): a fixed-dump
    // restore's doctor_schedule_slots.start_time values were real future times
    // AT SEED TIME, but every day that passes after the dump is captured turns
    // more of its earliest AVAILABLE slots into the past relative to "now" at
    // restore time. This query used to have no time filter at all, so
    // next_slot()'s front-to-back cursor (ordered by start_time ASC) always
    // drained those stale slots first; the real booking lock (doctor_repo.rs's
    // `UPDATE ... WHERE id=$1 AND start_time > clock_timestamp()`) then
    // silently affects 0 rows for each one, surfacing as a business rejection
    // -- confirmed live: 0/29 (0%) booking success one day after the dump was
    // captured, purely this artifact, not a capacity signal. Fixed by matching
    // the same `start_time > clock_timestamp()` filter production code already
    // uses correctly for this exact purpose (see doctor_repo.rs's own
    // available-slots query) -- zero data mutation required, and correct no
    // matter how much time has elapsed since the dump was captured.
    let available_slots: Vec<(Uuid, Uuid)> = sqlx::query(
        "SELECT s.id, s.doctor_id FROM doctor_schedule_slots s \
         JOIN doctor_profiles dp ON dp.id = s.doctor_id \
         WHERE s.status = 'AVAILABLE' AND s.start_time > clock_timestamp() \
         AND dp.license_number LIKE 'SYNTH-KE-CAP-%' \
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
    let patient_user_ids: Vec<Uuid> = patient_rows.iter().map(|(uid, _)| *uid).collect();
    let now = Instant::now();
    let patient_tokens: Vec<RwLock<(String, Instant)>> = patient_user_ids
        .iter()
        .map(|uid| {
            RwLock::new((
                create_access_token(*uid, "PATIENT", *uid, &jwt_private_key).unwrap(),
                now,
            ))
        })
        .collect();
    let doctor_tokens: Vec<RwLock<(String, Instant)>> = doctor_user_ids
        .iter()
        .map(|uid| {
            RwLock::new((
                create_access_token(*uid, "DOCTOR", *uid, &jwt_private_key).unwrap(),
                now,
            ))
        })
        .collect();
    let admin_tokens: Vec<RwLock<(String, Instant)>> = admin_user_ids
        .iter()
        .map(|uid| {
            RwLock::new((
                create_access_token(*uid, "ADMIN", *uid, &jwt_private_key).unwrap(),
                now,
            ))
        })
        .collect();

    let patient_count = patient_tokens.len();
    // M8 six-service benchmark: when CAPACITY_BASE_URL points at a reverse
    // proxy by IP:port (e.g. Caddy) rather than directly at the backend, the
    // proxy's Host-based vhost routing needs an explicit Host header — the
    // IP:port alone never matches a named `http://api.localhost { ... }`
    // site block. Optional and unused by the existing direct-to-backend
    // invocations (CAPACITY_HOST_HEADER unset leaves behavior unchanged).
    let mut client_builder = reqwest::Client::builder().timeout(Duration::from_secs(10));
    if let Ok(host_header) = env::var("CAPACITY_HOST_HEADER") {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::HOST,
            reqwest::header::HeaderValue::from_str(&host_header)
                .expect("CAPACITY_HOST_HEADER must be a valid header value"),
        );
        client_builder = client_builder.default_headers(headers);
    }
    let fixtures = Arc::new(Fixtures {
        base_url,
        client: client_builder.build().unwrap(),
        pg: pg.clone(),
        patient_tokens,
        patient_user_ids,
        patient_consultation_ids,
        patient_state: (0..patient_count)
            .map(|_| Mutex::new(PatientRuntimeState::default()))
            .collect(),
        booking_attempts: (0..patient_count).map(|_| AtomicU32::new(0)).collect(),
        doctor_tokens,
        doctor_user_ids,
        admin_tokens,
        admin_user_ids,
        jwt_private_key: jwt_private_key.clone(),
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
            "registered_patients={} active_sessions_target={active_sessions} ramp_secs={ramp_secs} think_time_secs=[{think_min_secs},{think_max_secs}]\n",
            patient_count
        );
        let mut handles = Vec::new();
        for session_idx in 0..active_sessions {
            let f = fixtures.clone();
            let tx = tx.clone();
            let patient_idx = session_idx % patient_count.max(1);
            // Milestone 8.0 session-ramp remediation: spread each session's
            // first start uniformly across [0, ramp_secs] instead of every
            // session starting at the same instant. ramp_secs=0 (the
            // SIMULTANEOUS_BURST default) reduces this to the original,
            // unchanged immediate-start behavior exactly (start_delay is
            // always zero).
            let start_delay = if ramp_secs > 0 {
                Duration::from_secs_f64(
                    ramp_secs as f64 * session_idx as f64 / active_sessions.max(1) as f64,
                )
            } else {
                Duration::ZERO
            };
            handles.push(tokio::spawn(async move {
                if !start_delay.is_zero() {
                    tokio::time::sleep(start_delay).await;
                }
                // A session whose ramp-assigned start lands at/after the
                // deadline (only possible when ramp_secs > duration_secs)
                // never dispatches anything — counted via the complete
                // absence of its SESSION_START marker, never silently
                // treated as having run.
                if Instant::now() >= deadline {
                    return;
                }
                let _ = tx.send(Sample {
                    label: "SESSION_START",
                    elapsed: Duration::ZERO,
                    status: 0,
                    sent_at: Instant::now(),
                });
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

    // SESSION_START markers are bookkeeping, not real dispatched requests —
    // excluded here so achieved_rps reflects actual traffic, not the ramp.
    let real_sample_count = samples
        .iter()
        .filter(|s| !is_session_start_marker(s))
        .count();
    let achieved_rps = real_sample_count as f64 / duration_secs as f64;
    println!(
        "configured_target_rps={target_rps:.1} requested_count={requested_count} achieved_response_rps={achieved_rps:.1} achieved_count={}\n",
        real_sample_count
    );

    report_time_series(&samples, test_start, duration_secs);
    let active_sessions_target = if mode == "sessions" {
        active_sessions
    } else {
        0
    };
    report(
        &samples,
        duration_secs,
        target_rps,
        requested_count,
        active_sessions_target,
    );
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
    // Milestone 8 Workstream 1 isolated-profile addition: when set, every
    // session dispatches ONLY this one action instead of the normal weighted
    // mix below, so a profile's memory/CPU footprint can be measured without
    // the other endpoints' traffic diluting it. Unset (the default) preserves
    // the original weighted-mix behavior exactly, so existing REALISTIC_RAMP/
    // SIMULTANEOUS_BURST runs and their recorded baselines are unaffected.
    if let Ok(isolated) = env::var("CAPACITY_ISOLATED_ACTION") {
        match isolated.as_str() {
            "booking_payment" => {
                booking_action(f, tx, patient_idx).await;
                return;
            }
            "chat" => {
                // chat_action requires a conversation, which only exists
                // after a successful booking_action (it reads
                // patient_state.booked_appointment's conv_id, populated at
                // the end of booking_action). An isolated chat-only run
                // never calls booking_action otherwise, so every session
                // would fall through to chat_action's own "no conversation"
                // fallback (a GET /doctors) for its entire lifetime and
                // measure nothing but that — confirmed happened on the first
                // attempt (2,079/2,079 samples were "GET /doctors", zero
                // "POST chat message"). Book once per session, then chat
                // repeatedly once a conversation exists.
                let has_booking = f.patient_state[patient_idx]
                    .lock()
                    .await
                    .booked_appointment
                    .is_some();
                if has_booking {
                    chat_action(f, tx, patient_idx).await;
                } else {
                    booking_action(f, tx, patient_idx).await;
                }
                return;
            }
            other => panic!(
                "Unknown CAPACITY_ISOLATED_ACTION={other}; expected \"booking_payment\" or \"chat\""
            ),
        }
    }

    let roll: f64 = {
        use rand::Rng;
        rand::thread_rng().gen::<f64>()
    };
    let token = &patient_token(f, patient_idx).await;

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
        // A virtual patient may create one new booking during a run. Once the
        // booking exists, keep the session on read/consultation traffic rather
        // than repeatedly creating fresh appointments for the same identity;
        // repeated bookings were a synthetic burst source that overwhelmed
        // Payment's bounded 20/s token bucket at the 250-RPS preparation run.
        let already_booked = f.patient_state[patient_idx]
            .lock()
            .await
            .booked_appointment
            .is_some();
        if already_booked {
            chat_action(f, tx, patient_idx).await;
        } else {
            booking_action(f, tx, patient_idx).await;
        }
    } else if roll < 0.86 {
        let idx = f.rr_doctor.fetch_add(1, Ordering::Relaxed) % f.doctor_tokens.len().max(1);
        send_sample(
            f,
            tx,
            "GET wallet summary",
            timed_get(
                f,
                "/api/v1/doctor/wallet/summary",
                &doctor_token(f, idx).await,
            )
            .await,
        );
    } else if roll < 0.91 {
        telemetry_action(f, tx, patient_idx).await;
    } else if roll < 0.95 {
        let idx = f.rr_admin.fetch_add(1, Ordering::Relaxed) % f.admin_tokens.len().max(1);
        send_sample(
            f,
            tx,
            "GET admin telemetry",
            timed_get(
                f,
                "/api/v1/admin/command/telemetry",
                &admin_token(f, idx).await,
            )
            .await,
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
/// `SESSION_START` is a bookkeeping marker (§ module doc), never a real
/// request/response — it must never be counted as throughput, latency, or
/// (status=0, which `Outcome::classify` maps to `SystemError`) a failure.
fn is_session_start_marker(s: &Sample) -> bool {
    s.label == "SESSION_START"
}

fn report_time_series(samples: &[Sample], test_start: Instant, duration_secs: u64) {
    const BUCKET_SECS: u64 = 10;
    let num_buckets = duration_secs.div_ceil(BUCKET_SECS).max(1) as usize;
    let mut buckets: Vec<Vec<Duration>> = vec![Vec::new(); num_buckets];
    let mut error_buckets: Vec<usize> = vec![0; num_buckets];
    // Milestone 8.0 session-ramp remediation: how many sessions had actually
    // started (sent their SESSION_START marker) by the END of each bucket —
    // the real, observed concurrent-activity curve, never assumed to equal
    // CAPACITY_ACTIVE_SESSIONS before the ramp window completes.
    let mut sessions_started_by_bucket_end: Vec<usize> = vec![0; num_buckets];
    let mut sessions_started_so_far = 0usize;
    let mut session_start_offsets: Vec<u64> = samples
        .iter()
        .filter(|s| is_session_start_marker(s))
        .map(|s| s.sent_at.saturating_duration_since(test_start).as_secs())
        .collect();
    session_start_offsets.sort_unstable();
    let mut start_cursor = 0usize;

    for s in samples {
        if is_session_start_marker(s) {
            continue;
        }
        let offset_secs = s.sent_at.saturating_duration_since(test_start).as_secs();
        let idx = ((offset_secs / BUCKET_SECS) as usize).min(num_buckets - 1);
        buckets[idx].push(s.elapsed);
        if Outcome::classify(s.status) == Outcome::SystemError {
            error_buckets[idx] += 1;
        }
    }
    for (i, slot) in sessions_started_by_bucket_end.iter_mut().enumerate() {
        let bucket_end_secs = (i as u64 + 1) * BUCKET_SECS;
        while start_cursor < session_start_offsets.len()
            && session_start_offsets[start_cursor] < bucket_end_secs
        {
            sessions_started_so_far += 1;
            start_cursor += 1;
        }
        *slot = sessions_started_so_far;
    }

    println!("--- Time series (10s buckets) ---");
    println!(
        "{:>10} {:>12} {:>8} {:>8} {:>8} {:>8} {:>15}",
        "window_s", "achieved_rps", "p50_ms", "p95_ms", "p99_ms", "5xx", "sessions_active"
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
            "{:>10} {:>12.1} {:>8} {:>8} {:>8} {:>8} {:>15}",
            format!(
                "{}-{}",
                i as u64 * BUCKET_SECS,
                (i as u64 + 1) * BUCKET_SECS
            ),
            rps,
            p50,
            p95,
            p99,
            error_buckets[i],
            sessions_started_by_bucket_end[i]
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
            timed_get(f, "/api/v1/doctors", &patient_token(f, idx).await).await,
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
                &patient_token(f, idx).await,
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
            timed_get(
                f,
                "/api/v1/doctor/wallet/summary",
                &doctor_token(f, idx).await,
            )
            .await,
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
            timed_get(
                f,
                "/api/v1/admin/command/telemetry",
                &admin_token(f, idx).await,
            )
            .await,
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
            timed_get(f, "/api/v1/doctors", &patient_token(f, patient_idx).await).await,
        );
        return;
    }
    let Some((slot_id, doctor_id)) = next_slot(f).await else {
        send_sample(
            f,
            tx,
            "GET /doctors",
            timed_get(f, "/api/v1/doctors", &patient_token(f, patient_idx).await).await,
        );
        return;
    };
    let token = patient_token(f, patient_idx).await;

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

    // Reserve this patient's single booking slot before the webhook call. If
    // the provider confirmation is shed or fails, the run records that honest
    // payment outcome instead of issuing a second booking for the same user.
    {
        let mut state = f.patient_state[patient_idx].lock().await;
        state.booked_appointment = Some((appointment_id, Uuid::nil()));
    }

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
            timed_get(f, "/api/v1/doctors", &patient_token(f, patient_idx).await).await,
        );
        return;
    };
    if conv_id.is_nil() {
        send_sample(
            f,
            tx,
            "GET /doctors",
            timed_get(f, "/api/v1/doctors", &patient_token(f, patient_idx).await).await,
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
            &patient_token(f, patient_idx).await,
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
            timed_get(f, "/api/v1/doctors", &patient_token(f, patient_idx).await).await,
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
            &patient_token(f, patient_idx).await,
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
            timed_get(f, "/api/v1/doctors", &patient_token(f, patient_idx).await).await,
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
            &patient_token(f, patient_idx).await,
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
            timed_get(f, "/api/v1/doctors", &patient_token(f, patient_idx).await).await,
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
            timed_get(f, "/api/v1/doctors", &patient_token(f, patient_idx).await).await,
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
            &patient_token(f, patient_idx).await,
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

fn report(
    samples: &[Sample],
    duration_secs: u64,
    target_rps: f64,
    requested_count: u64,
    active_sessions_target: usize,
) {
    let _ = requested_count;
    let sessions_started = samples
        .iter()
        .filter(|s| is_session_start_marker(s))
        .count();
    if active_sessions_target > 0 {
        println!(
            "SESSIONS: target={active_sessions_target} started={sessions_started} never_started={} \
             -- \"started\" means this session actually began dispatching real requests before the \
             test deadline; never_started > 0 only happens when CAPACITY_RAMP_SECS exceeds \
             CAPACITY_DURATION_SECS (the ramp window outlived the test). Registered-account count is \
             reported separately above and must never be read as this number.\n",
            active_sessions_target.saturating_sub(sessions_started)
        );
    }
    let mut by_label: HashMap<&str, EndpointAgg> = HashMap::new();
    let mut total_business_reject = 0usize;
    let mut total_auth_reject = 0usize;
    let mut total_rate_limited = 0usize;
    let mut total_system_error = 0usize;
    let mut total_success = 0usize;
    let total = samples.len() - sessions_started;

    for s in samples {
        if is_session_start_marker(s) {
            continue;
        }
        let outcome = Outcome::classify(s.status);
        let agg = by_label.entry(s.label).or_default();
        agg.total += 1;
        agg.latencies.push(s.elapsed);
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

    let observed_rps = total as f64 / duration_secs.max(1) as f64;
    let successful_rps = total_success as f64 / duration_secs.max(1) as f64;
    println!(
        "LOAD_ACCOUNTING: configured_target_rps={target_rps:.1} observed_offered_rps={observed_rps:.3} achieved_response_rps={observed_rps:.3} successful_response_rps={successful_rps:.3} total_requests={total} successful_requests={total_success}\n"
    );

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
    // Milestone 8.0: the clearest "successful end-to-end business outcome"
    // for the booking flow, not just a per-step status code. `booking_action`
    // only ever sends a "POST webhook confirm" sample for an attempt whose
    // preceding "POST /appointments/book" already succeeded, so this success
    // count already IS the number of fully completed book+pay flows — no
    // separate correlation tracking needed.
    if let Some(booking) = by_label.get("POST /appointments/book") {
        let completed = by_label
            .get("POST webhook confirm")
            .map(|a| a.success)
            .unwrap_or(0);
        println!(
            "\nEND_TO_END_BOOKING_SUCCESS: {completed} of {} booking attempts fully completed \
             (booked AND payment confirmed) -- {:.1}%",
            booking.total,
            completed as f64 / booking.total.max(1) as f64 * 100.0
        );
    }
    let _ = Utc::now();
}
