//! Milestone 8.0R: regression coverage for the admission-control correctness
//! remediation in `src/admission.rs`. A Milestone 8.0 load benchmark measured
//! 46-56% of ordinary `GET /doctors`-class ("Normal" cost class) requests
//! rejected at 150-250 offered RPS from independent synthetic Patient/Doctor
//! accounts, with real error rate under 0.06% and flat 3-5ms p50 latency
//! throughout — a pure, fully account-agnostic policy ceiling, not hardware
//! or DB saturation. Root cause and fix are documented in `admission.rs`'s
//! own module doc comment; this file exercises the corrected behavior
//! end-to-end through the real HTTP/DB/Redis stack, not a unit-level mock.
//!
//! Coverage against Milestone 8.0R Phase 4's 15-scenario list: scenarios
//! 1-8 and 15 are exercised below. 9 (booking conflicts/idempotency) is
//! already covered by `schedule_exclusivity.rs`/`booking_concurrency_test.rs`
//! and unaffected by this change (Payment-class budgets/lanes are
//! untouched). 10 (Consultation/Webhook reserved lanes under load) and 11
//! (CPU/DB capacity exhaustion) are exercised by `admission.rs`'s own
//! `expensive_flood_does_not_take_ordinary_slots` unit test and
//! `pool_pressure.rs` respectively, neither of which this change touches.
//! 12/13 (bounded waiting, safe rejection and recovery) follow structurally
//! from `try_acquire_owned` (no queue — admit or reject immediately) and are
//! not re-tested here. 14 (no auth/RLS bypass) is scenario 15 below plus the
//! full `cargo test --test api_tests` run this file is part of.
//!
//! Concurrency-capacity note: `test_config()` deliberately uses a tiny
//! `db_pool_max_connections_per_role` (2 — see that config's own comment),
//! which `AppState::new` turns into a tiny `Admission` concurrency ceiling
//! (combined-pools × 5 = 10) — appropriate for most of this suite, wrong for
//! tests that need to fire many requests at once. Instantaneous concurrency
//! capacity is a separate, legitimate, already-covered-elsewhere dimension
//! (`pool_pressure.rs`, `admission.rs`'s own unit tests); it is deliberately
//! raised to a production-realistic value here (`Admission::new(50)`,
//! matching `DB_POOL_MAX_CONNECTIONS_PER_ROLE=10` × 5 real pools) so these
//! tests isolate and prove the *rate/fairness* fix without being confounded
//! by that separate, correctly-tiny-by-default test setting.
use super::*;

async fn fresh_account(pool: &PgPool, role: &str) -> (Uuid, String) {
    let user_id = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,phone_number,role,is_active,is_verified,created_at,updated_at) VALUES($1,$2,$3::user_role_enum,true,true,NOW(),NOW())")
        .bind(user_id)
        .bind(format!("+2547{:08}", rand::thread_rng().gen_range(0..100_000_000)))
        .bind(role)
        .execute(pool)
        .await
        .unwrap();
    if role == "PATIENT" {
        sqlx::query(
            "INSERT INTO patient_profiles (id, user_id, full_name, date_of_birth, gender, created_at, updated_at) \
             VALUES (gen_random_uuid(), $1, 'Admission Fairness Test Patient', '1990-01-01', 'OTHER', NOW(), NOW())",
        )
        .bind(user_id)
        .execute(pool)
        .await
        .unwrap();
    }
    if role == "DOCTOR" {
        sqlx::query(
            "INSERT INTO doctor_profiles (id, user_id, full_name, license_number, license_authority, license_country, \
             primary_specialty, experience_years, current_hospital, consultation_fee_video, consultation_fee_chat, \
             residential_address, verified_phone, is_verified, professional_verification_status, created_at, updated_at) \
             VALUES (gen_random_uuid(), $1, 'Admission Fairness Test Doctor', $2, 'KMPDC', 'KE', 'General Practice', \
             5, 'Test Hospital', 500.00, 300.00, 'Test Address', '+254700000000', true, 'VERIFIED', NOW(), NOW())",
        )
        .bind(user_id)
        .bind(format!("ADM-FAIR-{}", user_id.simple()))
        .execute(pool)
        .await
        .unwrap();
    }
    fixture_session(pool, user_id).await;
    (user_id, make_token(user_id, role))
}

/// Builds the router with a production-realistic admission concurrency
/// capacity instead of `test_config()`'s deliberately tiny default — see
/// module doc comment.
async fn realistic_capacity_app(pool: PgPool) -> (axum::Router, PgPool, AppState) {
    let (_, pool, mut state) = test_app_and_state(pool).await;
    state.admission = Arc::new(hellodoctor_backend::admission::Admission::new(50));
    let app = create_router(state.clone());
    (app, pool, state)
}

/// Scenario 1 + 5: one legitimate Patient making ordinary requests succeeds
/// every time well below any budget.
#[sqlx::test(migrations = "./migrations")]
async fn one_patient_ordinary_browsing_always_succeeds(pool: PgPool) {
    let (app, pool, _state) = realistic_capacity_app(pool).await;
    let (_, token) = fresh_account(&pool, "PATIENT").await;
    for _ in 0..10 {
        assert_eq!(
            get(&app, "/api/v1/doctors", Some(&token)).await.status(),
            StatusCode::OK
        );
    }
}

/// Scenario 2: many *independent* Patients browsing concurrently must not
/// collectively exhaust a shared policy quota. 40 concurrent requests from
/// 40 distinct accounts would already have been throttled by the old,
/// removed, fully account-agnostic ~100/s *combined* ceiling once blended
/// with any other traffic; more fundamentally, none of these accounts'
/// *individual* traffic is anywhere near either the per-account budget
/// (`auth_middleware`, 120/60s) or the new per-principal budget
/// (`ratelimit:principal:normal`, 1800/60s) — the only way this could have
/// failed before the fix was by competing for the one shared global bucket.
#[sqlx::test(migrations = "./migrations")]
async fn many_independent_patients_browse_concurrently_without_rejection(pool: PgPool) {
    let (app, pool, _state) = realistic_capacity_app(pool).await;
    let mut tokens = Vec::new();
    for _ in 0..40 {
        let (_, token) = fresh_account(&pool, "PATIENT").await;
        tokens.push(token);
    }
    let results = futures_util::future::join_all(
        tokens.iter().map(|t| get(&app, "/api/v1/doctors", Some(t))),
    )
    .await;
    let statuses: Vec<_> = results.iter().map(|r| r.status()).collect();
    let ok = statuses.iter().filter(|s| **s == StatusCode::OK).count();
    assert_eq!(
        ok, 40,
        "all 40 independent Patients must succeed concurrently, not compete for one shared budget; got {statuses:?}"
    );
}

/// Scenario 3: many independent Doctors concurrently accessing their own
/// ordinary (Normal-class) endpoint succeed together, same as Patients.
#[sqlx::test(migrations = "./migrations")]
async fn many_independent_doctors_access_dashboards_concurrently(pool: PgPool) {
    let (app, pool, _state) = realistic_capacity_app(pool).await;
    let mut tokens = Vec::new();
    for _ in 0..40 {
        let (_, token) = fresh_account(&pool, "DOCTOR").await;
        tokens.push(token);
    }
    let results = futures_util::future::join_all(
        tokens
            .iter()
            .map(|t| get(&app, "/api/v1/doctor/appointments", Some(t))),
    )
    .await;
    let statuses: Vec<_> = results.iter().map(|r| r.status()).collect();
    let ok = statuses.iter().filter(|s| **s == StatusCode::OK).count();
    assert_eq!(
        ok, 40,
        "all 40 independent Doctors must succeed concurrently; got {statuses:?}"
    );
}

/// Scenario 4: a Patient and a Doctor sharing the same public IP (NAT) must
/// both succeed independently — the fix is keyed by authenticated account
/// identity, falling back to IP only for anonymous/invalid-token traffic,
/// so two *different, valid* accounts behind one IP must never throttle
/// each other.
#[sqlx::test(migrations = "./migrations")]
async fn patient_and_doctor_sharing_one_ip_both_succeed(pool: PgPool) {
    let (app, pool, _state) = realistic_capacity_app(pool).await;
    let (_, patient_token) = fresh_account(&pool, "PATIENT").await;
    let (_, doctor_token) = fresh_account(&pool, "DOCTOR").await;
    let shared_addr: SocketAddr = "203.0.113.50:443".parse().unwrap();

    let mut req = Request::builder()
        .method("GET")
        .uri("/api/v1/doctors")
        .header("Authorization", format!("Bearer {patient_token}"))
        .body(Body::empty())
        .unwrap();
    req.extensions_mut().insert(ConnectInfo(shared_addr));
    let patient_res = app.clone().oneshot(req).await.unwrap();

    let mut req = Request::builder()
        .method("GET")
        .uri("/api/v1/doctor/appointments")
        .header("Authorization", format!("Bearer {doctor_token}"))
        .body(Body::empty())
        .unwrap();
    req.extensions_mut().insert(ConnectInfo(shared_addr));
    let doctor_res = app.clone().oneshot(req).await.unwrap();

    assert_eq!(patient_res.status(), StatusCode::OK);
    assert_eq!(doctor_res.status(), StatusCode::OK);
}

/// Scenario 6: the *same* account issuing abnormally frequent Normal-class
/// requests still gets rate-limited — per-account fairness must never
/// become an unlimited bypass for one account's abuse. This is
/// `auth_middleware`'s existing, untouched `ratelimit:authenticated:*`
/// budget (120/60s for Normal). `check_and_increment` is a fixed-window
/// Redis counter (INCR+EXPIRE, not a smoothed rate), so this pre-seeds the
/// counter to one below the limit instead of firing 121 real requests —
/// the same pattern `patient_quizzes.rs` uses for its own rate-limit test.
#[sqlx::test(migrations = "./migrations")]
async fn same_account_abnormally_frequent_requests_still_rate_limited(pool: PgPool) {
    let (app, pool, state) = realistic_capacity_app(pool).await;
    let (user_id, token) = fresh_account(&pool, "PATIENT").await;
    let mut redis = state.redis.get().await.unwrap();
    let _: () = deadpool_redis::redis::cmd("SET")
        .arg(format!("ratelimit:authenticated:{user_id}:normal"))
        .arg(120)
        .arg("EX")
        .arg(60)
        .query_async(&mut redis)
        .await
        .unwrap();
    assert_eq!(
        get(&app, "/api/v1/doctors", Some(&token)).await.status(),
        StatusCode::TOO_MANY_REQUESTS,
        "the 121st request in one window from one account must still be throttled"
    );
}

/// Scenario 7 + 8: anonymous (no bearer token) high-volume traffic against a
/// Normal-shaped path is bounded per-source (the new
/// `ratelimit:principal:normal:anon:{ip}` check), and — critically — must
/// not be able to consume shared capacity that starves a concurrent,
/// legitimate, distinct authenticated account from a different address.
/// Previously nothing bounded an anonymous flood per-source before it
/// reached the (post-auth) 401, so one anonymous source could exhaust the
/// old shared global budget alongside real accounts. Pre-seeds the counter
/// for the same fixed-window reason as above (limit is 1800/60s).
#[sqlx::test(migrations = "./migrations")]
async fn anonymous_flood_is_bounded_and_does_not_starve_a_real_account(pool: PgPool) {
    let (app, pool, state) = realistic_capacity_app(pool).await;
    let (_, legit_token) = fresh_account(&pool, "PATIENT").await;
    let flood_addr: SocketAddr = "198.51.100.77:9000".parse().unwrap();
    let mut redis = state.redis.get().await.unwrap();
    let _: () = deadpool_redis::redis::cmd("SET")
        .arg(format!(
            "ratelimit:principal:normal:anon:{}",
            flood_addr.ip()
        ))
        .arg(1800)
        .arg("EX")
        .arg(60)
        .query_async(&mut redis)
        .await
        .unwrap();

    let mut req = Request::builder()
        .method("GET")
        .uri("/api/v1/doctors")
        .body(Body::empty())
        .unwrap();
    req.extensions_mut().insert(ConnectInfo(flood_addr));
    let flood_res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(
        flood_res.status(),
        StatusCode::TOO_MANY_REQUESTS,
        "an anonymous source past its own per-source budget must be throttled"
    );

    // A real, distinct, authenticated account from a *different* address
    // must still succeed, unaffected by the anonymous flood above.
    assert_eq!(
        get(&app, "/api/v1/doctors", Some(&legit_token))
            .await
            .status(),
        StatusCode::OK,
        "a legitimate authenticated account must not be starved by an unrelated anonymous flood"
    );
}

/// Milestone 8.0 verification item 1 ("JWT admission identity"): `principal()`
/// (admission.rs) must verify the bearer token's signature/expiry via the
/// same `crypto::verify_access_token` primitive `auth_middleware` uses,
/// never merely parse unverified claims — otherwise an attacker could claim
/// an arbitrary `user_id` in an unsigned/mis-signed token and either (a)
/// exhaust a victim account's `ratelimit:principal:normal:acct:{id}` budget
/// from outside (poisoning), or (b) ride on a victim's budget state to get
/// treated more favorably than their own anonymous/IP bucket deserves. A
/// request with a tampered signature must be indistinguishable, from
/// admission's perspective, from having no token at all: it falls to the
/// `anon:{ip}` bucket, not the claimed account's bucket, and is rejected
/// downstream by `auth_middleware` with 401 — never 429 attributed to the
/// victim's account. Each call below goes through `get()`, which (per
/// `send()`) assigns a fresh random peer IP per request, so the forged/
/// expired/malformed requests never coincidentally share the victim's own
/// `anon:` bucket either.
#[sqlx::test(migrations = "./migrations")]
async fn forged_signature_token_cannot_poison_or_use_victim_accounts_budget(pool: PgPool) {
    let (app, pool, state) = realistic_capacity_app(pool).await;
    let (victim_id, victim_token) = fresh_account(&pool, "PATIENT").await;

    // Exhaust the victim's own principal budget directly, so that if a
    // forged token were (incorrectly) attributed to this account, the
    // forged request would itself come back 429 instead of 401.
    let mut redis = state.redis.get().await.unwrap();
    let _: () = deadpool_redis::redis::cmd("SET")
        .arg(format!("ratelimit:principal:normal:acct:{victim_id}"))
        .arg(1800)
        .arg("EX")
        .arg(60)
        .query_async(&mut redis)
        .await
        .unwrap();

    // Sanity check: the real victim, using their real token, is genuinely
    // blocked by their own exhausted budget.
    assert_eq!(
        get(&app, "/api/v1/doctors", Some(&victim_token))
            .await
            .status(),
        StatusCode::TOO_MANY_REQUESTS,
        "sanity check: the victim's own exhausted principal budget must bind for their real token"
    );

    // Tamper with the last character of the signature segment — still a
    // syntactically well-formed `header.payload.signature` JWT, but the
    // signature no longer verifies against the server's public key.
    let mut forged = victim_token.clone();
    let last = forged.pop().unwrap();
    forged.push(if last == 'A' { 'B' } else { 'A' });
    assert_ne!(forged, victim_token);

    let forged_res = get(&app, "/api/v1/doctors", Some(&forged)).await;
    assert_eq!(
        forged_res.status(),
        StatusCode::UNAUTHORIZED,
        "a forged-signature token claiming the victim's identity must be rejected (401) by \
         downstream auth, not short-circuited by admission attributing it to the victim's \
         exhausted budget (which would surface as 429 instead)"
    );
}

/// Same claim, for an expired-but-genuinely-signed token (real private key,
/// `exp` in the past): `verify_access_token`'s `validate_exp = true` must
/// still reject it in `principal()`, falling back to the (fresh) anon/IP
/// bucket rather than the victim's exhausted account bucket.
#[sqlx::test(migrations = "./migrations")]
async fn expired_token_cannot_poison_or_use_victim_accounts_budget(pool: PgPool) {
    let (app, pool, state) = realistic_capacity_app(pool).await;
    let (victim_id, _) = fresh_account(&pool, "PATIENT").await;

    let mut redis = state.redis.get().await.unwrap();
    let _: () = deadpool_redis::redis::cmd("SET")
        .arg(format!("ratelimit:principal:normal:acct:{victim_id}"))
        .arg(1800)
        .arg("EX")
        .arg(60)
        .query_async(&mut redis)
        .await
        .unwrap();

    let expired = craft_token(
        victim_id,
        "PATIENT",
        JWT_ISSUER,
        JWT_AUDIENCE,
        -7200,
        -3600, // expired one hour ago
    );
    let res = get(&app, "/api/v1/doctors", Some(&expired)).await;
    assert_eq!(
        res.status(),
        StatusCode::UNAUTHORIZED,
        "an expired token for the victim's account must be rejected (401), not treated as the \
         victim's identity by admission (which would surface as 429 from their exhausted budget)"
    );
}

/// A structurally invalid bearer value (not a JWT at all) must fail closed
/// the same way — no panic, no 5xx, no account/bucket attribution, just the
/// ordinary 401 an absent/garbage token already produces.
#[sqlx::test(migrations = "./migrations")]
async fn malformed_bearer_token_fails_closed_without_crashing_or_attribution(pool: PgPool) {
    let (app, pool, _state) = realistic_capacity_app(pool).await;
    let _ = fresh_account(&pool, "PATIENT").await; // ensure the route itself works at all
    let res = get(&app, "/api/v1/doctors", Some("not.a.jwt")).await;
    assert_eq!(
        res.status(),
        StatusCode::UNAUTHORIZED,
        "a malformed bearer token must fail closed with 401, not a panic/5xx"
    );
}

/// Milestone 8.0 verification item 1, positive case: two distinct *valid*
/// accounts are keyed independently by `ratelimit:principal:normal:acct:*`
/// — exhausting one real account's budget must never block a different real
/// account, confirming the new principal budget (not just `auth_middleware`'s
/// pre-existing per-account budget, already covered by
/// `same_account_abnormally_frequent_requests_still_rate_limited`) is
/// correctly scoped to verified account identity.
#[sqlx::test(migrations = "./migrations")]
async fn exhausting_one_valid_accounts_principal_budget_does_not_affect_another(pool: PgPool) {
    let (app, pool, state) = realistic_capacity_app(pool).await;
    let (exhausted_id, exhausted_token) = fresh_account(&pool, "PATIENT").await;
    let (_, other_token) = fresh_account(&pool, "PATIENT").await;

    let mut redis = state.redis.get().await.unwrap();
    let _: () = deadpool_redis::redis::cmd("SET")
        .arg(format!("ratelimit:principal:normal:acct:{exhausted_id}"))
        .arg(1800)
        .arg("EX")
        .arg(60)
        .query_async(&mut redis)
        .await
        .unwrap();

    assert_eq!(
        get(&app, "/api/v1/doctors", Some(&exhausted_token))
            .await
            .status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(
        get(&app, "/api/v1/doctors", Some(&other_token)).await.status(),
        StatusCode::OK,
        "a distinct valid account must be unaffected by another account's exhausted principal budget"
    );
}

/// Scenario 15: the fix touches only admission/rate-limit plumbing — RLS,
/// ABAC and cross-account data isolation must be exactly as before. A
/// Patient must still never see another Patient's or Doctor's private rows
/// through the now-wider admission ceilings.
#[sqlx::test(migrations = "./migrations")]
async fn wider_admission_ceilings_do_not_weaken_authorization_or_rls(pool: PgPool) {
    let (app, pool, _state) = realistic_capacity_app(pool).await;
    let (_, patient_token) = fresh_account(&pool, "PATIENT").await;
    // Directory browsing never exposes another account's private profile
    // fields; still gated by `AnyAuthenticated`, never by an unauthenticated
    // caller regardless of admission capacity.
    assert_eq!(
        get(&app, "/api/v1/doctors", None).await.status(),
        StatusCode::UNAUTHORIZED,
        "admission must never admit an unauthenticated caller past auth_middleware"
    );
    assert_eq!(
        get(&app, "/api/v1/doctor/appointments", Some(&patient_token))
            .await
            .status(),
        StatusCode::FORBIDDEN,
        "a Patient token must still be rejected by Doctor-only ABAC regardless of admission capacity"
    );
}
