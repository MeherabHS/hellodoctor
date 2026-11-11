//! Reproducible Admin-Web/Doctor-Web page-load workload generator, built to
//! replace the ad hoc, never-persisted `web_page_workload.sh` script used in
//! the original (contaminated) six-service 60-minute run. That script lived
//! nowhere in version control, so nobody could review it, reuse it
//! identically, or even confirm later what it actually did — exactly the
//! failure mode Milestone 8 Step 3's "rebuild it as a reviewable artifact"
//! directive exists to close.
//!
//! Usage (six-service local-verification stack already up):
//!     WEB_ADMIN_WORKERS=5 WEB_DOCTOR_WORKERS=5 WEB_DURATION_SECS=360 \
//!     WEB_BASE_URL=http://127.0.0.1:58080 \
//!     WEB_DATABASE_URL=postgres://postgres:m8-local-password@127.0.0.1:55432/hellodoctor \
//!         cargo run --release --example web_workload
//!
//! Pair with `capacity_test` (CAPACITY_MODE=sessions) for the API workload —
//! launch both from `services/backend/scripts/run_six_service_comparison.ps1`,
//! which starts them within the same second and logs both observed start
//! times, so "one explicit measurement start and end" is a measured fact in
//! the evidence, not an unverified claim.
//!
//! AUTHENTICATION: before the measurement window, the harness performs the
//! existing Admin and Doctor password-plus-development-OTP login flows using
//! WEB_ADMIN_EMAIL/WEB_ADMIN_PASSWORD and WEB_DOCTOR_EMAIL/WEB_DOCTOR_PASSWORD.
//! It places the returned active-session access tokens in the same BFF cookie
//! names (hd_admin_access / hd_doctor_access, plus each app's marker cookie),
//! so real backend session validation remains exercised without adding login
//! traffic to the dashboard measurement window.
//!
//! NO REAL EXTERNAL TRAFFIC: every endpoint this harness calls is a
//! read-only GET (dashboard pages, `/admin/overview`, `/admin/doctors`,
//! `/admin/finance/summary`, `/admin/command/telemetry`, `/admin/logs`,
//! `/admin/grievances`, `/admin/patients` for Admin; `doctor/profile`,
//! `doctor/duty`, `doctor/slots`, `doctor/appointments`, `doctor/chats`,
//! `doctor/wallet/summary`, `notifications` for Doctor — the exact
//! read-only subset `apps/doctor-web/src/lib/contracts.ts::safeBackendPath`
//! already allowlists). None of these routes mutate state, send payment
//! instructions, send email, or call FCM/Agora — confirmed by reading each
//! handler's own route table in `services/backend/src/api/router.rs` before
//! selecting this list. This harness deliberately never calls a POST/PATCH
//! mutating endpoint for that reason.
//!
//! FAIL-FAST PREFLIGHT: before the measurement window starts, `preflight()`
//! makes one real request to each app's dashboard root AND one authenticated
//! API call, and hard-asserts: HTTP status, a non-trivial `Content-Length`
//! (a blank `200 OK, Content-Length: 0` is exactly what Caddy's own fallback
//! returned when the prior benchmark run's `CAPACITY_HOST_HEADER` was never
//! set — so a zero-length body is treated as a ROUTING FAILURE, not a
//! success), the expected `Content-Type`, and — for the page route — a
//! known HTML marker string that only the real app shell renders. Any
//! failure here aborts with a labelled reason and a nonzero exit code before
//! a single measurement sample is taken, instead of silently producing a
//!100%-"success" fake run the way the original bug did.
//!
//! EARLY-TERMINATION DETECTION: each worker task tracks its own consecutive
//! transport-failure count (status 0 = connection refused/timeout/etc.). If
//! any worker accumulates `MAX_CONSECUTIVE_FAILURES` in a row, it marks the
//! shared `run_invalid` flag with a reason and stops early rather than
//! silently degrading the sample count. The final report checks this flag
//! and exits with status 3 (printing `RUN_INVALID: ...`) if it was ever set,
//! so a partial/broken run can never be mistaken for a complete one by
//! anything that scripts around this binary's exit code.

use parking_lot::Mutex as SyncMutex;
use serde_json::Value;
use std::collections::HashMap;
use std::env;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use uuid::Uuid;

/// Milestone 8 endurance-run harness correction (2026-11-05): mirrors
/// `crypto::ACCESS_TOKEN_TTL_SECONDS` (15 minutes) -- a 60-minute endurance
/// run authenticated once at startup and reused that one access token for
/// the full hour, so ~73-74% of every Admin/Doctor Web request after the
/// first ~15 minutes was auth-rejected (confirmed: (60-15)/60 ~= 75%, almost
/// exactly the observed rate). The real admin-web/doctor-web apps already
/// implement proper session refresh (see apps/admin-web/src/lib/refresh.ts)
/// -- this harness simply never exercised it. Fixed by proactively refreshing
/// well before expiry using the backend's own `/auth/refresh` endpoint,
/// exactly the mechanism real sessions use -- no new auth bypass, no change
/// to token lifetime, no weakening of any check.
const ACCESS_TOKEN_TTL_SECS: u64 = 15 * 60;
/// Refresh this long before the token would actually expire, so request
/// latency/scheduling jitter can never let a worker present an already-
/// expired token. Comfortably inside the TTL (10 of 15 minutes used).
const REFRESH_SAFETY_MARGIN_SECS: u64 = 5 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Outcome {
    Success,
    AuthReject,
    RateLimited,
    SystemError,
    Other,
}
impl Outcome {
    fn classify(status: u16) -> Outcome {
        match status {
            200..=299 => Outcome::Success,
            401 | 403 => Outcome::AuthReject,
            429 => Outcome::RateLimited,
            500..=599 | 0 => Outcome::SystemError,
            _ => Outcome::Other,
        }
    }
}

#[derive(Debug, Clone)]
struct Sample {
    role: &'static str, // "ADMIN" | "DOCTOR"
    label: &'static str,
    elapsed: Duration,
    status: u16,
}

const MAX_CONSECUTIVE_FAILURES: u32 = 10;

struct RunState {
    invalid: AtomicBool,
    reason: SyncMutex<Option<String>>,
}
impl RunState {
    fn mark_invalid(&self, reason: String) {
        if !self.invalid.swap(true, Ordering::SeqCst) {
            *self.reason.lock() = Some(reason);
        }
    }
}

struct AdminIdentity {
    cookie: Arc<RwLock<String>>,
}
struct DoctorIdentity {
    cookie: Arc<RwLock<String>>,
}

/// Pure, network-free: should a refresh fire yet? Kept separate from the
/// actual sleep/refresh loop so it's directly unit-testable.
fn should_refresh(token_age: Duration, ttl_secs: u64, safety_margin_secs: u64) -> bool {
    let refresh_at = ttl_secs.saturating_sub(safety_margin_secs);
    token_age.as_secs() >= refresh_at
}

/// Pure, network-free: extracts the rotated (access_token, refresh_token)
/// pair from a `/auth/refresh` response body, or a descriptive error --
/// never panics, so a refresh failure can be handled (logged, retried) by
/// the caller instead of crashing the whole harness mid-run.
fn parse_refresh_response(body: &Value) -> Result<(String, String), String> {
    if body["success"] != true {
        return Err(format!("refresh response had success=false: {body}"));
    }
    let access_token = body["data"]["access_token"]
        .as_str()
        .filter(|v| !v.is_empty())
        .ok_or_else(|| format!("refresh response missing access_token: {body}"))?
        .to_string();
    let refresh_token = body["data"]["refresh_token"]
        .as_str()
        .filter(|v| !v.is_empty())
        .ok_or_else(|| format!("refresh response missing refresh_token: {body}"))?
        .to_string();
    Ok((access_token, refresh_token))
}

/// Calls the real `/auth/refresh` endpoint -- the same rotating-refresh-token
/// mechanism the real admin-web/doctor-web apps use, never a bypass. Returns
/// Err on any transport/HTTP/parse failure rather than panicking, so the
/// caller can log and retry on the next cycle instead of tearing down the run.
async fn refresh_access_token(
    client: &reqwest::Client,
    base_url: &str,
    refresh_token: &str,
) -> Result<(String, String), String> {
    let resp = client
        .post(format!("{base_url}/api/v1/auth/refresh"))
        .header(reqwest::header::HOST, "api.localhost")
        .json(&serde_json::json!({ "refresh_token": refresh_token }))
        .send()
        .await
        .map_err(|e| format!("refresh transport failed: {e}"))?;
    let status = resp.status();
    let body: Value = resp
        .json()
        .await
        .map_err(|e| format!("refresh returned invalid JSON (HTTP {status}): {e}"))?;
    if !status.is_success() {
        return Err(format!("refresh failed with HTTP {status}: {body}"));
    }
    parse_refresh_response(&body)
}

enum RefreshCookieKind {
    Admin,
    Doctor(Uuid),
}
impl RefreshCookieKind {
    fn build(&self, access_token: &str) -> String {
        match self {
            RefreshCookieKind::Admin => admin_cookie(access_token),
            RefreshCookieKind::Doctor(scope) => doctor_cookie(access_token, *scope),
        }
    }
}

/// Runs for the whole measurement window, refreshing the shared cookie
/// proactively before the access token's TTL would otherwise let it expire
/// mid-run. A failed refresh is logged (never silently swallowed) and
/// retried on the next cycle -- workers keep using whatever cookie value is
/// currently published, stale or not, rather than the run crashing.
async fn refresh_loop(
    client: reqwest::Client,
    base_url: String,
    role: &'static str,
    mut refresh_token: String,
    cookie: Arc<RwLock<String>>,
    kind: RefreshCookieKind,
    deadline: Instant,
) {
    let mut last_refresh = Instant::now();
    loop {
        let age = last_refresh.elapsed();
        if should_refresh(age, ACCESS_TOKEN_TTL_SECS, REFRESH_SAFETY_MARGIN_SECS) {
            match refresh_access_token(&client, &base_url, &refresh_token).await {
                Ok((new_access_token, new_refresh_token)) => {
                    *cookie.write().await = kind.build(&new_access_token);
                    refresh_token = new_refresh_token;
                    last_refresh = Instant::now();
                    println!("{role} session refreshed (proactive, before TTL expiry)");
                }
                Err(reason) => {
                    eprintln!("{role} session refresh FAILED, will retry next cycle: {reason}");
                    // Deliberately do not advance `last_refresh` -- retry
                    // sooner rather than waiting a full cycle again, but
                    // still bounded by the sleep below so this can't spin.
                }
            }
        }
        if Instant::now() >= deadline {
            return;
        }
        tokio::time::sleep(Duration::from_secs(30)).await;
    }
}

fn admin_cookie(access_token: &str) -> String {
    format!("hd_admin_access={access_token}; hd_admin_session=1")
}
fn doctor_cookie(access_token: &str, scope: Uuid) -> String {
    format!("hd_doctor_access={access_token}; hd_doctor_scope={scope}")
}

async fn timed_get(
    client: &reqwest::Client,
    base_url: &str,
    host: &str,
    path: &str,
    cookie: &str,
    origin: Option<&str>,
) -> (u16, Duration, usize, String) {
    let start = Instant::now();
    let mut req = client
        .get(format!("{base_url}{path}"))
        .header(reqwest::header::HOST, host)
        .header(reqwest::header::COOKIE, cookie);
    if let Some(o) = origin {
        req = req.header(reqwest::header::ORIGIN, o);
    }
    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let content_type = resp
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();
            let body = resp.bytes().await.unwrap_or_default();
            if std::env::var("WEB_DEBUG_BODIES").is_ok() && (status == 401 || status == 403) {
                eprintln!(
                    "[DEBUG] {status} {path} cookie={cookie} body={}",
                    String::from_utf8_lossy(&body)
                );
            }
            (status, start.elapsed(), body.len(), content_type)
        }
        Err(_) => (0, start.elapsed(), 0, String::new()),
    }
}

async fn authenticate_with_otp(
    client: &reqwest::Client,
    base_url: &str,
    role: &str,
    email: &str,
    password: &str,
) -> (String, String) {
    let (login_path, verify_path) = match role {
        "ADMIN" => ("/api/v1/auth/admin/login", "/api/v1/auth/admin/verify-otp"),
        "DOCTOR" => (
            "/api/v1/auth/doctor/login",
            "/api/v1/auth/doctor/verify-otp",
        ),
        _ => panic!("unsupported web workload role: {role}"),
    };
    let login = client
        .post(format!("{base_url}{login_path}"))
        .header(reqwest::header::HOST, "api.localhost")
        .json(&serde_json::json!({ "email": email, "password": password }))
        .send()
        .await
        .unwrap_or_else(|error| panic!("{role} password login transport failed: {error}"));
    let login_status = login.status();
    let login_body: Value = login
        .json()
        .await
        .unwrap_or_else(|error| panic!("{role} password login returned invalid JSON: {error}"));
    if !login_status.is_success() || login_body["success"] != true {
        panic!("{role} password login failed with HTTP {login_status}");
    }
    let session_token = login_body["data"]["session_token"]
        .as_str()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| panic!("{role} password login returned no MFA session"));
    let otp_code = login_body["data"]["debug_otp_code"]
        .as_str()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| panic!("{role} login returned no development OTP code"));

    let verify = client
        .post(format!("{base_url}{verify_path}"))
        .header(reqwest::header::HOST, "api.localhost")
        .json(&serde_json::json!({ "session_token": session_token, "otp_code": otp_code }))
        .send()
        .await
        .unwrap_or_else(|error| panic!("{role} OTP verification transport failed: {error}"));
    let verify_status = verify.status();
    let verify_body: Value = verify
        .json()
        .await
        .unwrap_or_else(|error| panic!("{role} OTP verification returned invalid JSON: {error}"));
    if !verify_status.is_success() || verify_body["success"] != true {
        panic!("{role} OTP verification failed with HTTP {verify_status}");
    }
    let access_token = verify_body["data"]["access_token"]
        .as_str()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| panic!("{role} OTP verification returned no access token"))
        .to_string();
    let refresh_token = verify_body["data"]["refresh_token"]
        .as_str()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| panic!("{role} OTP verification returned no refresh token"))
        .to_string();
    (access_token, refresh_token)
}

/// Hard-fails (process exit) before any measurement sample is taken if
/// routing or auth minting is broken — see module doc "FAIL-FAST PREFLIGHT".
async fn preflight(
    client: &reqwest::Client,
    base_url: &str,
    admin_host: &str,
    doctor_host: &str,
    admin_cookie_hdr: &str,
    doctor_cookie_hdr: &str,
) {
    let mut failures: Vec<String> = Vec::new();

    let (status, _elapsed, len, ctype) =
        timed_get(client, base_url, admin_host, "/", admin_cookie_hdr, None).await;
    if status != 200 {
        failures.push(format!(
            "ADMIN root page: expected HTTP 200, got {status} (routing/auth failure)"
        ));
    } else if len == 0 {
        failures.push(
            "ADMIN root page: HTTP 200 but Content-Length 0 -- this is Caddy's own fallback \
             response, not the real admin-web app (the exact CAPACITY_HOST_HEADER-missing bug \
             class this preflight exists to catch)"
                .to_string(),
        );
    } else if !ctype.contains("text/html") {
        failures.push(format!(
            "ADMIN root page: expected text/html, got content-type '{ctype}'"
        ));
    }

    let (status, _elapsed, len, ctype) = timed_get(
        client,
        base_url,
        admin_host,
        "/api/proxy/admin/overview",
        admin_cookie_hdr,
        None,
    )
    .await;
    if status != 200 {
        failures.push(format!(
            "ADMIN /api/proxy/admin/overview: expected HTTP 200, got {status} \
             (minted admin token rejected -- auth bypass is broken, not a real backend policy \
             rejection, since this is the admin's own overview endpoint)"
        ));
    } else if len == 0 || !ctype.contains("application/json") {
        failures.push(format!(
            "ADMIN /api/proxy/admin/overview: expected non-empty application/json, got \
             content-type='{ctype}' len={len}"
        ));
    }

    let (status, _elapsed, len, ctype) = timed_get(
        client,
        base_url,
        doctor_host,
        "/doctor/chats",
        doctor_cookie_hdr,
        None,
    )
    .await;
    if status != 200 {
        failures.push(format!(
            "DOCTOR /doctor/chats page: expected HTTP 200, got {status} (routing/auth failure)"
        ));
    } else if len == 0 {
        failures.push(
            "DOCTOR /doctor/chats page: HTTP 200 but Content-Length 0 -- Caddy fallback, not \
             the real doctor-web app"
                .to_string(),
        );
    } else if !ctype.contains("text/html") {
        failures.push(format!(
            "DOCTOR /doctor/chats page: expected text/html, got content-type '{ctype}'"
        ));
    }

    let (status, _elapsed, len, ctype) = timed_get(
        client,
        base_url,
        doctor_host,
        "/api/proxy/doctor/profile",
        doctor_cookie_hdr,
        None,
    )
    .await;
    if status != 200 {
        failures.push(format!(
            "DOCTOR /api/proxy/doctor/profile: expected HTTP 200, got {status} \
             (minted doctor token rejected -- auth bypass is broken)"
        ));
    } else if len == 0 || !ctype.contains("application/json") {
        failures.push(format!(
            "DOCTOR /api/proxy/doctor/profile: expected non-empty application/json, got \
             content-type='{ctype}' len={len}"
        ));
    }

    if !failures.is_empty() {
        eprintln!("=== PREFLIGHT FAILED -- aborting before any measurement window ===");
        for f in &failures {
            eprintln!("  - {f}");
        }
        std::process::exit(2);
    }
    println!(
        "=== PREFLIGHT PASSED: routing + real Admin/Doctor auth confirmed for both apps ===\n"
    );
}

async fn admin_worker(
    client: reqwest::Client,
    base_url: String,
    admin_host: String,
    cookie: Arc<RwLock<String>>,
    origin: String,
    think_min_ms: u64,
    think_max_ms: u64,
    deadline: Instant,
    tx: std::sync::mpsc::Sender<Sample>,
    state: Arc<RunState>,
    worker_id: usize,
) {
    const ENDPOINTS: &[(&str, &str)] = &[
        ("GET admin /", "/"),
        ("GET admin/overview", "/api/proxy/admin/overview"),
        ("GET admin/doctors", "/api/proxy/admin/doctors"),
        ("GET admin/patients", "/api/proxy/admin/patients"),
        (
            "GET admin/finance/summary",
            "/api/proxy/admin/finance/summary",
        ),
        (
            "GET admin/command/telemetry",
            "/api/proxy/admin/command/telemetry",
        ),
        ("GET admin/logs", "/api/proxy/admin/logs"),
        ("GET admin/grievances", "/api/proxy/admin/grievances"),
    ];
    let mut consecutive_failures: u32 = 0;
    let mut i = worker_id;
    while Instant::now() < deadline {
        if state.invalid.load(Ordering::SeqCst) {
            return;
        }
        i = i.wrapping_add(1);
        let (label, path) = ENDPOINTS[i % ENDPOINTS.len()];
        let current_cookie = cookie.read().await.clone();
        let (status, elapsed, _len, _ctype) = timed_get(
            &client,
            &base_url,
            &admin_host,
            path,
            &current_cookie,
            Some(&origin),
        )
        .await;
        if status == 0 {
            consecutive_failures += 1;
            if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                state.mark_invalid(format!(
                    "ADMIN worker {worker_id} hit {MAX_CONSECUTIVE_FAILURES} consecutive \
                     transport failures on '{label}' -- treating run as invalid"
                ));
                return;
            }
        } else {
            consecutive_failures = 0;
        }
        let _ = tx.send(Sample {
            role: "ADMIN",
            label,
            elapsed,
            status,
        });
        let think = think_min_ms + (rand_jitter(worker_id, i) % (think_max_ms - think_min_ms + 1));
        tokio::time::sleep(Duration::from_millis(think)).await;
    }
}

async fn doctor_worker(
    client: reqwest::Client,
    base_url: String,
    doctor_host: String,
    cookie: Arc<RwLock<String>>,
    origin: String,
    think_min_ms: u64,
    think_max_ms: u64,
    deadline: Instant,
    tx: std::sync::mpsc::Sender<Sample>,
    state: Arc<RunState>,
    worker_id: usize,
) {
    const ENDPOINTS: &[(&str, &str)] = &[
        ("GET doctor /chats", "/doctor/chats"),
        ("GET doctor/profile", "/api/proxy/doctor/profile"),
        ("GET doctor/duty", "/api/proxy/doctor/duty"),
        ("GET doctor/slots", "/api/proxy/doctor/slots"),
        ("GET doctor/appointments", "/api/proxy/doctor/appointments"),
        ("GET doctor/chats(api)", "/api/proxy/doctor/chats"),
        (
            "GET doctor/wallet/summary",
            "/api/proxy/doctor/wallet/summary",
        ),
        ("GET notifications", "/api/proxy/notifications"),
    ];
    let mut consecutive_failures: u32 = 0;
    let mut i = worker_id;
    while Instant::now() < deadline {
        if state.invalid.load(Ordering::SeqCst) {
            return;
        }
        i = i.wrapping_add(1);
        let (label, path) = ENDPOINTS[i % ENDPOINTS.len()];
        let current_cookie = cookie.read().await.clone();
        let (status, elapsed, _len, _ctype) = timed_get(
            &client,
            &base_url,
            &doctor_host,
            path,
            &current_cookie,
            Some(&origin),
        )
        .await;
        if status == 0 {
            consecutive_failures += 1;
            if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                state.mark_invalid(format!(
                    "DOCTOR worker {worker_id} hit {MAX_CONSECUTIVE_FAILURES} consecutive \
                     transport failures on '{label}' -- treating run as invalid"
                ));
                return;
            }
        } else {
            consecutive_failures = 0;
        }
        let _ = tx.send(Sample {
            role: "DOCTOR",
            label,
            elapsed,
            status,
        });
        let think = think_min_ms + (rand_jitter(worker_id, i) % (think_max_ms - think_min_ms + 1));
        tokio::time::sleep(Duration::from_millis(think)).await;
    }
}

/// Deterministic, dependency-free jitter (no extra `rand` crate dependency
/// for one small helper) -- good enough to avoid every worker requesting in
/// lockstep, not meant to be cryptographically random.
fn rand_jitter(worker_id: usize, tick: usize) -> u64 {
    let x = (worker_id as u64).wrapping_mul(2654435761) ^ (tick as u64).wrapping_mul(40503);
    (x ^ (x >> 13)) % 1000
}

#[derive(Default)]
struct Agg {
    total: usize,
    success: usize,
    auth_reject: usize,
    rate_limited: usize,
    system_error: usize,
    other: usize,
    latencies: Vec<Duration>,
}

fn report(role: &str, samples: &[&Sample], wall_secs: f64) {
    let mut by_label: HashMap<&str, Agg> = HashMap::new();
    for s in samples {
        let agg = by_label.entry(s.label).or_default();
        agg.total += 1;
        agg.latencies.push(s.elapsed);
        match Outcome::classify(s.status) {
            Outcome::Success => agg.success += 1,
            Outcome::AuthReject => agg.auth_reject += 1,
            Outcome::RateLimited => agg.rate_limited += 1,
            Outcome::SystemError => agg.system_error += 1,
            Outcome::Other => agg.other += 1,
        }
    }
    let total: usize = samples.len();
    println!(
        "=== {role} web workload summary ===\nrequests={total} achieved_rps={:.2}",
        total as f64 / wall_secs.max(0.001)
    );
    println!(
        "{:<26} {:>7} {:>7} {:>8} {:>8} {:>7} {:>7} {:>7} {:>7} {:>7}",
        "endpoint", "total", "succ", "authrej", "ratelim", "syserr", "other", "p50", "p95", "p99"
    );
    let mut labels: Vec<&&str> = by_label.keys().collect();
    labels.sort();
    for label in labels {
        let agg = &by_label[*label];
        let mut durs = agg.latencies.clone();
        durs.sort();
        let n = durs.len();
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
            "{:<26} {:>7} {:>7} {:>8} {:>8} {:>7} {:>7} {:>7} {:>7} {:>7}",
            label,
            agg.total,
            agg.success,
            agg.auth_reject,
            agg.rate_limited,
            agg.system_error,
            agg.other,
            p50,
            p95,
            p99
        );
    }
    println!();
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let base_url =
        env::var("WEB_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:58080".to_string());
    let admin_host = env::var("WEB_ADMIN_HOST").unwrap_or_else(|_| "admin.localhost".to_string());
    let doctor_host =
        env::var("WEB_DOCTOR_HOST").unwrap_or_else(|_| "doctor.localhost".to_string());
    let admin_origin =
        env::var("WEB_ADMIN_ORIGIN").unwrap_or_else(|_| format!("http://{admin_host}"));
    let doctor_origin =
        env::var("WEB_DOCTOR_ORIGIN").unwrap_or_else(|_| format!("http://{doctor_host}"));
    let admin_workers: usize = env::var("WEB_ADMIN_WORKERS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);
    let doctor_workers: usize = env::var("WEB_DOCTOR_WORKERS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);
    let duration_secs: u64 = env::var("WEB_DURATION_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(300);
    let think_min_ms: u64 = env::var("WEB_THINK_MIN_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(500);
    let think_max_ms: u64 = env::var("WEB_THINK_MAX_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2000)
        .max(think_min_ms + 1);
    let admin_email = env::var("WEB_ADMIN_EMAIL").expect("WEB_ADMIN_EMAIL must be set");
    let admin_password = env::var("WEB_ADMIN_PASSWORD").expect("WEB_ADMIN_PASSWORD must be set");
    let doctor_email = env::var("WEB_DOCTOR_EMAIL").expect("WEB_DOCTOR_EMAIL must be set");
    let doctor_password = env::var("WEB_DOCTOR_PASSWORD").expect("WEB_DOCTOR_PASSWORD must be set");
    let start_epoch_ms: Option<u128> = env::var("WEB_START_EPOCH_MS")
        .ok()
        .and_then(|v| v.parse().ok());

    println!("=== HelloDoctor Admin/Doctor Web Workload Generator ===");
    println!(
        "base_url={base_url} admin_host={admin_host} doctor_host={doctor_host} \
         admin_workers={admin_workers} doctor_workers={doctor_workers} \
         duration={duration_secs}s think=[{think_min_ms},{think_max_ms}]ms"
    );

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .expect("build reqwest client");

    let (admin_access_token, admin_refresh_token) =
        authenticate_with_otp(&client, &base_url, "ADMIN", &admin_email, &admin_password).await;
    let (doctor_access_token, doctor_refresh_token) = authenticate_with_otp(
        &client,
        &base_url,
        "DOCTOR",
        &doctor_email,
        &doctor_password,
    )
    .await;
    // Stable for the whole run, same as a real doctor session's scope
    // selection -- refreshing the access token never changes which doctor
    // scope the session is bound to, so this must not be regenerated.
    let doctor_scope = Uuid::new_v4();
    let admin_cookie_initial = admin_cookie(&admin_access_token);
    let doctor_cookie_initial = doctor_cookie(&doctor_access_token, doctor_scope);
    let admin_identities = vec![AdminIdentity {
        cookie: Arc::new(RwLock::new(admin_cookie_initial.clone())),
    }];
    let doctor_identities = vec![DoctorIdentity {
        cookie: Arc::new(RwLock::new(doctor_cookie_initial.clone())),
    }];
    println!(
        "Authenticated test identities: 1 admin, 1 doctor (reused round-robin across {} admin_workers / {} doctor_workers)\n",
        admin_workers, doctor_workers
    );

    preflight(
        &client,
        &base_url,
        &admin_host,
        &doctor_host,
        &admin_cookie_initial,
        &doctor_cookie_initial,
    )
    .await;

    if let Some(target_ms) = start_epoch_ms {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        if target_ms > now_ms {
            tokio::time::sleep(Duration::from_millis((target_ms - now_ms) as u64)).await;
        }
    }
    let wall_start = Instant::now();
    let wall_start_epoch_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    println!("MEASUREMENT_WINDOW_START_EPOCH_MS={wall_start_epoch_ms}");
    let deadline = wall_start + Duration::from_secs(duration_secs);

    // Background session-refresh tasks: one per role, proactively refreshing
    // the access token well before its 15-minute TTL so a run longer than
    // that doesn't degrade into mostly-auth-rejected traffic (see the module
    // doc comment on ACCESS_TOKEN_TTL_SECS for the full rationale). Each task
    // owns its own refresh_token locally (rotates on every successful
    // refresh, exactly like a real session) and only ever writes the shared
    // cookie on success -- a failed refresh is logged and retried next cycle,
    // never crashes the run or falls back to a stale/bypassed token.
    let admin_refresh_handle = tokio::spawn(refresh_loop(
        client.clone(),
        base_url.clone(),
        "ADMIN",
        admin_refresh_token,
        admin_identities[0].cookie.clone(),
        RefreshCookieKind::Admin,
        deadline,
    ));
    let doctor_refresh_handle = tokio::spawn(refresh_loop(
        client.clone(),
        base_url.clone(),
        "DOCTOR",
        doctor_refresh_token,
        doctor_identities[0].cookie.clone(),
        RefreshCookieKind::Doctor(doctor_scope),
        deadline,
    ));

    let (tx, rx) = std::sync::mpsc::channel::<Sample>();
    let state = Arc::new(RunState {
        invalid: AtomicBool::new(false),
        reason: SyncMutex::new(None),
    });
    let alive = Arc::new(AtomicUsize::new(admin_workers + doctor_workers));

    let mut handles = Vec::new();
    for w in 0..admin_workers {
        let identity = &admin_identities[w % admin_identities.len()];
        let h = tokio::spawn(admin_worker(
            client.clone(),
            base_url.clone(),
            admin_host.clone(),
            identity.cookie.clone(),
            admin_origin.clone(),
            think_min_ms,
            think_max_ms,
            deadline,
            tx.clone(),
            state.clone(),
            w,
        ));
        handles.push(h);
    }
    for w in 0..doctor_workers {
        let identity = &doctor_identities[w % doctor_identities.len()];
        let h = tokio::spawn(doctor_worker(
            client.clone(),
            base_url.clone(),
            doctor_host.clone(),
            identity.cookie.clone(),
            doctor_origin.clone(),
            think_min_ms,
            think_max_ms,
            deadline,
            tx.clone(),
            state.clone(),
            w,
        ));
        handles.push(h);
    }
    drop(tx);
    let alive_clone = alive.clone();
    let collector = std::thread::spawn(move || {
        let mut samples = Vec::new();
        while let Ok(s) = rx.recv() {
            samples.push(s);
        }
        let _ = alive_clone;
        samples
    });

    for h in handles {
        let _ = h.await;
    }
    // Both refresh_loop tasks already respect the same `deadline` and exit on
    // their own -- aborting here is just defensive cleanup against ever
    // leaking a background task past the end of the run, not something
    // expected to fire in the normal case.
    admin_refresh_handle.abort();
    doctor_refresh_handle.abort();
    let samples = collector.join().unwrap_or_default();
    let wall_secs = wall_start.elapsed().as_secs_f64();
    let wall_end_epoch_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    println!("MEASUREMENT_WINDOW_END_EPOCH_MS={wall_end_epoch_ms}");
    println!(
        "MEASUREMENT_WINDOW_ACTUAL_SECS={:.1} (requested {duration_secs}s)\n",
        wall_secs
    );

    let admin_samples: Vec<&Sample> = samples.iter().filter(|s| s.role == "ADMIN").collect();
    let doctor_samples: Vec<&Sample> = samples.iter().filter(|s| s.role == "DOCTOR").collect();
    report("ADMIN", &admin_samples, wall_secs);
    report("DOCTOR", &doctor_samples, wall_secs);
    println!(
        "COMBINED_WEB_RPS={:.2} (admin={:.2} doctor={:.2})",
        samples.len() as f64 / wall_secs.max(0.001),
        admin_samples.len() as f64 / wall_secs.max(0.001),
        doctor_samples.len() as f64 / wall_secs.max(0.001)
    );
    if state.invalid.load(Ordering::SeqCst) {
        let reason = state
            .reason
            .lock()
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        eprintln!("\nRUN_INVALID: {reason}");
        std::process::exit(3);
    }
    println!("\nRUN_VALID: no worker hit the early-termination threshold.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn does_not_refresh_while_well_within_ttl() {
        assert!(!should_refresh(
            Duration::from_secs(60),
            ACCESS_TOKEN_TTL_SECS,
            REFRESH_SAFETY_MARGIN_SECS
        ));
        assert!(!should_refresh(
            Duration::from_secs(9 * 60),
            ACCESS_TOKEN_TTL_SECS,
            REFRESH_SAFETY_MARGIN_SECS
        ));
    }

    #[test]
    fn refreshes_once_inside_the_safety_margin_before_expiry() {
        // TTL=15min, margin=5min -> refresh fires at >=10min of age, which
        // still leaves a full 5-minute buffer before the token would
        // actually expire at 15min -- this is "token expiry" handling:
        // proactive, never reactive to an already-expired token.
        assert!(should_refresh(
            Duration::from_secs(10 * 60),
            ACCESS_TOKEN_TTL_SECS,
            REFRESH_SAFETY_MARGIN_SECS
        ));
        assert!(should_refresh(
            Duration::from_secs(14 * 60),
            ACCESS_TOKEN_TTL_SECS,
            REFRESH_SAFETY_MARGIN_SECS
        ));
    }

    #[test]
    fn parses_a_successful_refresh_response() {
        let body = serde_json::json!({
            "success": true,
            "data": { "access_token": "new-access", "refresh_token": "new-refresh" }
        });
        let (access, refresh) = parse_refresh_response(&body).expect("should parse");
        assert_eq!(access, "new-access");
        assert_eq!(refresh, "new-refresh");
    }

    #[test]
    fn rejects_a_refresh_response_with_success_false() {
        // "refresh failure" case: the backend explicitly rejected the
        // refresh token (expired/revoked/reused family) -- must be an Err,
        // never panic or silently treat it as success.
        let body = serde_json::json!({ "success": false, "error": { "code": "UNAUTHORIZED" } });
        let result = parse_refresh_response(&body);
        assert!(result.is_err());
    }

    #[test]
    fn rejects_a_malformed_refresh_response_missing_fields() {
        // "refresh failure" case: HTTP 200 success=true but a missing field
        // (unexpected shape) must still be a clean Err, not a panic and not
        // a silently-empty token string presented as a cookie.
        let missing_refresh = serde_json::json!({
            "success": true,
            "data": { "access_token": "new-access" }
        });
        assert!(parse_refresh_response(&missing_refresh).is_err());

        let empty_access = serde_json::json!({
            "success": true,
            "data": { "access_token": "", "refresh_token": "new-refresh" }
        });
        assert!(parse_refresh_response(&empty_access).is_err());
    }

    #[test]
    fn refresh_cookie_kind_builds_the_expected_cookie_shape() {
        assert_eq!(
            RefreshCookieKind::Admin.build("tok123"),
            "hd_admin_access=tok123; hd_admin_session=1"
        );
        let scope = Uuid::nil();
        assert_eq!(
            RefreshCookieKind::Doctor(scope).build("tok456"),
            format!("hd_doctor_access=tok456; hd_doctor_scope={scope}")
        );
    }
}
