//! Controlled A/B measurement of `Admission::new()`'s capacity policy —
//! deliberately NOT part of the regular suite (both functions are
//! `#[ignore]`d). Run explicitly, one at a time, so each gets its own
//! process/pool lifetime for clean resource sampling:
//!   cargo test --test api_tests admission_capacity_bench::bench_admission_capacity_baseline_x1 -- --ignored --exact --test-threads=1 --nocapture
//!   cargo test --test api_tests admission_capacity_bench::bench_admission_capacity_x5 -- --ignored --exact --test-threads=1 --nocapture
//!
//! Exists to answer, with real measurements instead of guesswork, whether
//! scaling `Admission::new()`'s capacity by the number of independent
//! per-role DB pools (5) is safe. Reuses
//! `test_identity_concurrent_http_requests_no_cross_user_pool_bleed`'s exact
//! interleaved-identity round-robin workload, because that test already is
//! the most demanding real concurrency / RLS-context-isolation invariant
//! this change could endanger — the comparison exercises correctness and
//! capacity in the same run, not two disconnected probes. The real `db`/
//! `auth_db` pool sizes are left at `test_config()`'s unchanged tiny value
//! (2 per role) in both runs; only the `Admission` semaphore capacity passed
//! in differs, isolating exactly the variable under test.
use super::*;
use std::time::Instant;

const BENCH_CONCURRENCY: usize = 128;

struct BenchOutcome {
    label: &'static str,
    admitted_capacity: u32,
    total: usize,
    ok: usize,
    rate_limited_429: usize,
    overloaded_503: usize,
    other_failures: usize,
    cross_user_bleed_detected: bool,
    latency_ms: Vec<f64>,
    db_pool_size_samples: Vec<u32>,
    db_pool_idle_samples: Vec<u32>,
    auth_pool_size_samples: Vec<u32>,
    auth_pool_idle_samples: Vec<u32>,
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = (((sorted.len() - 1) as f64) * p).round() as usize;
    sorted[idx]
}

async fn run_identity_bleed_burst(
    admission_capacity: u32,
    concurrency: usize,
    label: &'static str,
    pool: PgPool,
) -> BenchOutcome {
    let (_app, pool, mut state) = test_app_and_state(pool).await;
    state.admission = Arc::new(hellodoctor_backend::admission::Admission::new(
        admission_capacity,
    ));
    let app = create_router(state.clone());

    let patient_b_user =
        hellodoctor_backend::repository::user_repo::create_patient_user(&pool, "+254700009994")
            .await
            .unwrap();
    fixture_session(&pool, patient_b_user.id.into_inner()).await;
    let identities: [(Uuid, &str); 4] = [
        (uid(PATIENT_USER_ID), "PATIENT"),
        (patient_b_user.id.into_inner(), "PATIENT"),
        (uid(DOCTOR1_USER_ID), "DOCTOR"),
        (uid(ADMIN_USER_ID), "ADMIN"),
    ];
    for (user_id, _) in identities {
        sqlx::query("INSERT INTO notifications(user_id,type,title,body,dedup_key) VALUES($1,'REGISTRATION_VERIFIED','Bench marker','Synthetic bench notification',$2)")
            .bind(user_id)
            .bind(format!("BENCH-{label}-{user_id}"))
            .execute(&pool)
            .await
            .unwrap();
    }
    let tokens: Vec<String> = identities
        .iter()
        .map(|(id, role)| make_token(*id, role))
        .collect();

    // Background pool-pressure sampler, running for the whole burst.
    let sample_db = state.db.clone();
    let sample_auth = state.auth_db.clone();
    let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel::<()>();
    let sampler = tokio::spawn(async move {
        let mut db_size = Vec::new();
        let mut db_idle = Vec::new();
        let mut auth_size = Vec::new();
        let mut auth_idle = Vec::new();
        loop {
            db_size.push(sample_db.size());
            db_idle.push(sample_db.num_idle() as u32);
            auth_size.push(sample_auth.size());
            auth_idle.push(sample_auth.num_idle() as u32);
            if stop_rx.try_recv().is_ok() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        (db_size, db_idle, auth_size, auth_idle)
    });

    let mut handles = Vec::new();
    for i in 0..concurrency {
        let app = app.clone();
        let expected_user = identities[i % 4].0;
        let token = tokens[i % 4].clone();
        handles.push(tokio::spawn(async move {
            let start = Instant::now();
            let res = get(&app, "/api/v1/notifications", Some(&token)).await;
            let elapsed = start.elapsed();
            let status = res.status();
            let bled = if status == StatusCode::OK {
                let body = json_body(res).await;
                let items = body["data"].as_array().cloned().unwrap_or_default();
                items.iter().any(|item| {
                    Uuid::parse_str(item["user_id"].as_str().unwrap()).unwrap() != expected_user
                })
            } else {
                false
            };
            (status, elapsed, bled)
        }));
    }

    let mut ok = 0;
    let mut r429 = 0;
    let mut r503 = 0;
    let mut other = 0;
    let mut latency_ms = Vec::new();
    let mut bleed = false;
    for h in handles {
        let (status, elapsed, bled) = h.await.unwrap();
        if bled {
            bleed = true;
        }
        match status {
            StatusCode::OK => {
                ok += 1;
                latency_ms.push(elapsed.as_secs_f64() * 1000.0);
            }
            StatusCode::TOO_MANY_REQUESTS => r429 += 1,
            StatusCode::SERVICE_UNAVAILABLE => r503 += 1,
            _ => other += 1,
        }
    }
    let _ = stop_tx.send(());
    let (db_size, db_idle, auth_size, auth_idle) = sampler.await.unwrap();
    latency_ms.sort_by(|a, b| a.partial_cmp(b).unwrap());

    BenchOutcome {
        label,
        admitted_capacity: admission_capacity,
        total: concurrency,
        ok,
        rate_limited_429: r429,
        overloaded_503: r503,
        other_failures: other,
        cross_user_bleed_detected: bleed,
        latency_ms,
        db_pool_size_samples: db_size,
        db_pool_idle_samples: db_idle,
        auth_pool_size_samples: auth_size,
        auth_pool_idle_samples: auth_idle,
    }
}

fn report(o: &BenchOutcome) {
    let p50 = percentile(&o.latency_ms, 0.50);
    let p95 = percentile(&o.latency_ms, 0.95);
    let p99 = percentile(&o.latency_ms, 0.99);
    let max_db_size = o.db_pool_size_samples.iter().max().copied().unwrap_or(0);
    let min_db_idle = o.db_pool_idle_samples.iter().min().copied().unwrap_or(0);
    let max_auth_size = o.auth_pool_size_samples.iter().max().copied().unwrap_or(0);
    let min_auth_idle = o.auth_pool_idle_samples.iter().min().copied().unwrap_or(0);
    println!(
        "BENCH_RESULT label={} admitted_capacity={} total={} ok={} status_429={} status_503={} other={} \
cross_user_bleed={} p50_ms={:.2} p95_ms={:.2} p99_ms={:.2} max_db_pool_size={} min_db_pool_idle={} \
max_auth_pool_size={} min_auth_pool_idle={}",
        o.label,
        o.admitted_capacity,
        o.total,
        o.ok,
        o.rate_limited_429,
        o.overloaded_503,
        o.other_failures,
        o.cross_user_bleed_detected,
        p50,
        p95,
        p99,
        max_db_size,
        min_db_idle,
        max_auth_size,
        min_auth_idle
    );
}

#[sqlx::test(migrations = "./migrations")]
#[ignore = "explicit A/B admission-capacity measurement, not part of the regular suite"]
async fn bench_admission_capacity_baseline_x1(pool: PgPool) {
    // Pre-fix production behavior: Admission sized from ONE role's pool
    // capacity only (`config.db_pool_max_connections_per_role`, no ×5).
    let outcome = run_identity_bleed_burst(2, BENCH_CONCURRENCY, "baseline_x1", pool).await;
    report(&outcome);
    assert!(
        !outcome.cross_user_bleed_detected,
        "cross-user pool bleed under baseline capacity"
    );
}

#[sqlx::test(migrations = "./migrations")]
#[ignore = "explicit A/B admission-capacity measurement, not part of the regular suite"]
async fn bench_admission_capacity_x5(pool: PgPool) {
    let outcome = run_identity_bleed_burst(10, BENCH_CONCURRENCY, "x5", pool).await;
    report(&outcome);
    assert!(
        !outcome.cross_user_bleed_detected,
        "cross-user pool bleed under x5 capacity"
    );
}

// The two runs above use `test_config()`'s deliberately tiny base (2), which
// is below several of `Admission::new`'s internal per-lane ceilings (2, 4, 8)
// — exaggerating ×5's effect relative to what it does at PRODUCTION's actual
// default (`DB_POOL_MAX_CONNECTIONS_PER_ROLE` unset → 10, see `config.rs`),
// which already meets or exceeds every one of those ceilings on its own.
// These two runs hold the admission capacity at the production-realistic
// base/×5 values instead, to measure what ×5 actually changes for a real
// deployment rather than for this deliberately-starved test harness.
#[sqlx::test(migrations = "./migrations")]
#[ignore = "explicit A/B admission-capacity measurement, not part of the regular suite"]
async fn bench_admission_capacity_prod_base(pool: PgPool) {
    let outcome = run_identity_bleed_burst(10, BENCH_CONCURRENCY, "prod_base_x1", pool).await;
    report(&outcome);
    assert!(
        !outcome.cross_user_bleed_detected,
        "cross-user pool bleed under production-base capacity"
    );
}

#[sqlx::test(migrations = "./migrations")]
#[ignore = "explicit A/B admission-capacity measurement, not part of the regular suite"]
async fn bench_admission_capacity_prod_x5(pool: PgPool) {
    let outcome = run_identity_bleed_burst(50, BENCH_CONCURRENCY, "prod_x5", pool).await;
    report(&outcome);
    assert!(
        !outcome.cross_user_bleed_detected,
        "cross-user pool bleed under production ×5 capacity"
    );
}
