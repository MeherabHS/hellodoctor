//! Dedicated auth-stress benchmark (PRE_PATIENT_AZURE_4X8_BENCHMARK §7) —
//! separate from capacity_test.rs's mixed workload, which mints JWTs
//! directly and only lightly exercises real login. This harness drives the
//! real end-to-end lifecycles: Patient OTP request+verify, Doctor real
//! Argon2id password + MFA challenge, refresh-token rotation, logout, and
//! re-login. Open-loop, same Outcome/percentile reporting shape as
//! capacity_test.rs (duplicated rather than shared, matching how
//! load_test.rs/booking_concurrency_test.rs/capacity_test.rs are already
//! independent self-contained examples in this codebase).
//!
//! Does NOT disable Argon2 cost, mint replacement JWTs directly, or bypass
//! session-family checks — every token in this harness comes from a real
//! `/auth/*` response. Prerequisites: scripts/seed_capacity_dataset.sql (for
//! Patient identities) and scripts/seed_auth_stress_doctors.sql (for Doctor
//! identities with a real, documented benchmark-only password) already
//! applied to the target database, and the target backend started with
//! ENVIRONMENT=development + DEV_AUTH_OTP_ENABLED=true (already true for
//! every benchmark compose file via env_file: .env) — real OTP/MFA delivery
//! is not configured locally, and the backend fails closed without these
//! flags rather than silently succeeding (see auth_service.rs).
//!
//! Usage:
//!     AUTH_BASE_URL=http://localhost:8280 AUTH_TARGET_RPS=20 AUTH_DURATION_SECS=120 \
//!         cargo run --release --example auth_stress_test

use serde_json::json;
use std::collections::HashMap;
use std::env;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

const BENCHMARK_DOCTOR_PASSWORD: &str = "AuthStressBenchmark-2026!";

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
}

struct Fixtures {
    base_url: String,
    client: reqwest::Client,
    patient_phones: Vec<String>,
    doctor_emails: Vec<String>,
    rr_patient: AtomicUsize,
    rr_doctor: AtomicUsize,
}

async fn timed_post(
    f: &Fixtures,
    path: &str,
    body: &serde_json::Value,
) -> (u16, Duration, serde_json::Value) {
    let start = Instant::now();
    let res = f
        .client
        .post(format!("{}{}", f.base_url, path))
        .json(body)
        .send()
        .await;
    match res {
        Ok(r) => {
            let status = r.status().as_u16();
            let body: serde_json::Value = r.json().await.unwrap_or(serde_json::Value::Null);
            (status, start.elapsed(), body)
        }
        Err(_) => (0, start.elapsed(), serde_json::Value::Null),
    }
}

/// One full Patient lifecycle: OTP request -> verify+login -> refresh ->
/// logout -> re-login (a second OTP request+verify). Every one of these is a
/// real backend round trip against the real otp_repo/auth_sessions tables.
async fn patient_lifecycle(f: &Fixtures, tx: &mpsc::UnboundedSender<Sample>) {
    let idx = f.rr_patient.fetch_add(1, Ordering::Relaxed) % f.patient_phones.len();
    let phone = &f.patient_phones[idx];

    let (status, elapsed, body) = timed_post(
        f,
        "/api/v1/auth/register-otp",
        &json!({"phone_number": phone}),
    )
    .await;
    let _ = tx.send(Sample {
        label: "patient_otp_request",
        elapsed,
        status,
    });
    if Outcome::classify(status) != Outcome::Success {
        return;
    }
    let Some(code) = body["data"]["debug_otp_code"]
        .as_str()
        .map(|s| s.to_string())
    else {
        return; // dev-otp flags not enabled on the target — nothing more to exercise.
    };

    let (status, elapsed, body) = timed_post(
        f,
        "/api/v1/auth/verify-otp-and-login",
        &json!({"phone_number": phone, "otp_code": code}),
    )
    .await;
    let _ = tx.send(Sample {
        label: "patient_otp_verify",
        elapsed,
        status,
    });
    if Outcome::classify(status) != Outcome::Success {
        return;
    }
    let Some(refresh_token) = body["data"]["refresh_token"]
        .as_str()
        .map(|s| s.to_string())
    else {
        return;
    };

    let (status, elapsed, body) = timed_post(
        f,
        "/api/v1/auth/refresh",
        &json!({"refresh_token": refresh_token}),
    )
    .await;
    let _ = tx.send(Sample {
        label: "refresh_rotate",
        elapsed,
        status,
    });
    let rotated_refresh = body["data"]["refresh_token"]
        .as_str()
        .map(|s| s.to_string());

    let logout_token = rotated_refresh.unwrap_or(refresh_token);
    let (status, elapsed, _) = timed_post(
        f,
        "/api/v1/auth/logout",
        &json!({"refresh_token": logout_token}),
    )
    .await;
    let _ = tx.send(Sample {
        label: "logout",
        elapsed,
        status,
    });

    // Re-login: a fresh OTP cycle proves the account can log back in after
    // the previous session was revoked, not just that the first login worked.
    let (status, elapsed, body) = timed_post(
        f,
        "/api/v1/auth/register-otp",
        &json!({"phone_number": phone}),
    )
    .await;
    if Outcome::classify(status) == Outcome::Success {
        if let Some(code) = body["data"]["debug_otp_code"].as_str() {
            let (status, elapsed, _) = timed_post(
                f,
                "/api/v1/auth/verify-otp-and-login",
                &json!({"phone_number": phone, "otp_code": code}),
            )
            .await;
            let _ = tx.send(Sample {
                label: "relogin",
                elapsed,
                status,
            });
            return;
        }
    }
    let _ = tx.send(Sample {
        label: "relogin",
        elapsed,
        status,
    });
}

/// One full Doctor lifecycle: real Argon2id password login -> MFA challenge
/// verify -> refresh -> logout -> re-login (password + MFA again).
async fn doctor_lifecycle(f: &Fixtures, tx: &mpsc::UnboundedSender<Sample>) {
    let idx = f.rr_doctor.fetch_add(1, Ordering::Relaxed) % f.doctor_emails.len();
    let email = &f.doctor_emails[idx];

    let (status, elapsed, body) = timed_post(
        f,
        "/api/v1/auth/doctor/login",
        &json!({"email": email, "password": BENCHMARK_DOCTOR_PASSWORD}),
    )
    .await;
    let _ = tx.send(Sample {
        label: "doctor_password_login",
        elapsed,
        status,
    });
    if Outcome::classify(status) != Outcome::Success {
        return;
    }
    let Some(session_token) = body["data"]["session_token"]
        .as_str()
        .map(|s| s.to_string())
    else {
        return;
    };
    let Some(code) = body["data"]["debug_otp_code"]
        .as_str()
        .map(|s| s.to_string())
    else {
        return;
    };

    let (status, elapsed, body) = timed_post(
        f,
        "/api/v1/auth/doctor/verify-otp",
        &json!({"session_token": session_token, "otp_code": code}),
    )
    .await;
    let _ = tx.send(Sample {
        label: "doctor_mfa_verify",
        elapsed,
        status,
    });
    if Outcome::classify(status) != Outcome::Success {
        return;
    }
    let Some(refresh_token) = body["data"]["refresh_token"]
        .as_str()
        .map(|s| s.to_string())
    else {
        return;
    };

    let (status, elapsed, body) = timed_post(
        f,
        "/api/v1/auth/refresh",
        &json!({"refresh_token": refresh_token}),
    )
    .await;
    let _ = tx.send(Sample {
        label: "refresh_rotate",
        elapsed,
        status,
    });
    let rotated_refresh = body["data"]["refresh_token"]
        .as_str()
        .map(|s| s.to_string());

    let logout_token = rotated_refresh.unwrap_or(refresh_token);
    let (status, elapsed, _) = timed_post(
        f,
        "/api/v1/auth/logout",
        &json!({"refresh_token": logout_token}),
    )
    .await;
    let _ = tx.send(Sample {
        label: "logout",
        elapsed,
        status,
    });

    let (status, elapsed, body) = timed_post(
        f,
        "/api/v1/auth/doctor/login",
        &json!({"email": email, "password": BENCHMARK_DOCTOR_PASSWORD}),
    )
    .await;
    if Outcome::classify(status) == Outcome::Success {
        if let (Some(st), Some(code)) = (
            body["data"]["session_token"]
                .as_str()
                .map(|s| s.to_string()),
            body["data"]["debug_otp_code"]
                .as_str()
                .map(|s| s.to_string()),
        ) {
            let (status, elapsed, _) = timed_post(
                f,
                "/api/v1/auth/doctor/verify-otp",
                &json!({"session_token": st, "otp_code": code}),
            )
            .await;
            let _ = tx.send(Sample {
                label: "relogin",
                elapsed,
                status,
            });
            return;
        }
    }
    let _ = tx.send(Sample {
        label: "relogin",
        elapsed,
        status,
    });
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let base_url =
        env::var("AUTH_BASE_URL").unwrap_or_else(|_| "http://localhost:8280".to_string());
    let target_rps: f64 = env::var("AUTH_TARGET_RPS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10.0);
    let duration_secs: u64 = env::var("AUTH_DURATION_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60);
    let database_url = env::var("AUTH_DATABASE_URL").unwrap_or_else(|_| {
        "postgres://hellodoctor:hellodoctor_dev_password@localhost:5633/hellodoctor".to_string()
    });

    println!(
        "=== HelloDoctor Auth-Stress Test (open-loop, real OTP/password/MFA/refresh/logout) ==="
    );
    println!("target={base_url} target_rps={target_rps} duration={duration_secs}s\n");

    let pg = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("connect to benchmark Postgres");

    let patient_phones: Vec<String> = sqlx::query_scalar(
        "SELECT phone_number FROM users WHERE phone_number LIKE '+25472%' ORDER BY phone_number",
    )
    .fetch_all(&pg)
    .await
    .expect("load patient phones");
    let doctor_emails: Vec<String> = sqlx::query_scalar(
        "SELECT email FROM users WHERE phone_number LIKE '+25475%' AND email IS NOT NULL ORDER BY email",
    )
    .fetch_all(&pg)
    .await
    .expect("load auth-stress doctor emails (run scripts/seed_auth_stress_doctors.sql first)");

    if doctor_emails.is_empty() {
        eprintln!("No auth-stress doctors found (+25475% phone prefix) — run scripts/seed_auth_stress_doctors.sql against this database first.");
        std::process::exit(1);
    }

    println!(
        "Identities: {} patients (OTP), {} doctors (password+MFA)\n",
        patient_phones.len(),
        doctor_emails.len()
    );

    let fixtures = Arc::new(Fixtures {
        base_url,
        client: reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap(),
        patient_phones,
        doctor_emails,
        rr_patient: AtomicUsize::new(0),
        rr_doctor: AtomicUsize::new(0),
    });

    let (tx, mut rx) = mpsc::unbounded_channel::<Sample>();
    let interval = Duration::from_secs_f64(1.0 / target_rps);
    let deadline = Instant::now() + Duration::from_secs(duration_secs);

    let collector = tokio::spawn(async move {
        let mut samples = Vec::new();
        while let Some(s) = rx.recv().await {
            samples.push(s);
        }
        samples
    });

    let mut ticker = tokio::time::interval(interval);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Burst);
    let mut handles = Vec::new();
    let mut requested_count: u64 = 0;
    while Instant::now() < deadline {
        ticker.tick().await;
        requested_count += 1;
        let f = fixtures.clone();
        let tx = tx.clone();
        // 70% Patient OTP lifecycle, 30% Doctor password+MFA lifecycle —
        // patients log in far more often than doctors in real usage.
        handles.push(tokio::spawn(async move {
            let roll: f64 = {
                use rand::Rng;
                rand::thread_rng().gen()
            };
            if roll < 0.70 {
                patient_lifecycle(&f, &tx).await;
            } else {
                doctor_lifecycle(&f, &tx).await;
            }
        }));
    }
    drop(tx);
    for h in handles {
        let _ = h.await;
    }
    let samples = collector.await.unwrap();

    println!(
        "requested_count={requested_count} total_samples={}\n",
        samples.len()
    );

    type EndpointAgg = (usize, usize, usize, usize, usize, Vec<Duration>);
    let mut by_label: HashMap<&str, EndpointAgg> = HashMap::new();
    let mut total_sys_err = 0usize;
    let mut total_rate_limited = 0usize;
    let total = samples.len();
    for s in &samples {
        let entry = by_label.entry(s.label).or_default();
        entry.0 += 1;
        match Outcome::classify(s.status) {
            Outcome::Success => entry.1 += 1,
            Outcome::BusinessReject => entry.2 += 1,
            Outcome::AuthReject => entry.3 += 1,
            Outcome::RateLimited => {
                entry.4 += 1;
                total_rate_limited += 1;
            }
            Outcome::SystemError => {
                total_sys_err += 1;
            }
        }
        entry.5.push(s.elapsed);
    }
    println!(
        "TRUE_SYSTEM_ERROR_RATE={:.4}%  rate_limited={:.3}%\n",
        total_sys_err as f64 / total.max(1) as f64 * 100.0,
        total_rate_limited as f64 / total.max(1) as f64 * 100.0
    );
    println!(
        "{:<24} {:>8} {:>8} {:>8} {:>8} {:>8} {:>7} {:>7} {:>7}",
        "stage", "total", "succ", "bizrej", "authrej", "ratelim", "p50", "p95", "p99"
    );
    let mut labels: Vec<&&str> = by_label.keys().collect();
    labels.sort();
    for label in labels {
        let (total, succ, bizrej, authrej, ratelim, durs) = &by_label[*label];
        let mut durs = durs.clone();
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
            "{:<24} {:>8} {:>8} {:>8} {:>8} {:>8} {:>7} {:>7} {:>7}",
            label, total, succ, bizrej, authrej, ratelim, p50, p95, p99
        );
    }
}
