//! Admin privilege sweep (product owner, 2026-11-14): nobody gets Admin access
//! by editing a link, a token or a role claim.
//!
//! The route list is read from `src/api/router.rs` at compile time, so every
//! `/admin/...` route added later is covered automatically.
use super::*;
use serde_json::json;

const ROUTER_SOURCE: &str = include_str!("../../src/api/router.rs");

/// Every distinct `/admin/...` path in the router, with path parameters filled
/// in (random ids, so nothing real is targeted).
fn admin_paths() -> Vec<String> {
    let mut paths: Vec<String> = ROUTER_SOURCE
        .split('"')
        .filter(|s| s.starts_with("/admin/"))
        .map(|p| {
            p.split('/')
                .map(|seg| match seg {
                    ":action" => "publish".to_string(),
                    ":revision" => "1".to_string(),
                    s if s.starts_with(':') => Uuid::new_v4().to_string(),
                    s => s.to_string(),
                })
                .collect::<Vec<_>>()
                .join("/")
        })
        .map(|p| format!("/api/v1{p}"))
        .collect();
    paths.sort();
    paths.dedup();
    paths
}

/// The server's own throughput limits (an in-process token bucket per lane)
/// answer 429 when ~650 requests arrive back to back. Wait and retry like a
/// client would, so the final status is the authorisation decision itself.
async fn settled<F, Fut>(send: F) -> StatusCode
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = axum::http::Response<Body>>,
{
    for _ in 0..10 {
        let status = send().await.status();
        if status != StatusCode::TOO_MANY_REQUESTS && status != StatusCode::SERVICE_UNAVAILABLE {
            return status;
        }
        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    }
    send().await.status()
}

#[sqlx::test(migrations = "./migrations")]
async fn non_admins_are_refused_on_every_admin_route(pool: PgPool) {
    // Rate limits (per IP and per account) would start answering 429 long
    // before ~650 requests finish, hiding whether authorisation itself holds.
    // The IP ceiling is raised and the counters in this test's own Redis are
    // cleared after each route, so every route is checked for real.
    let mut config = test_config();
    config.global_ip_rate_limit_per_min = 100_000;
    let redis = deadpool_redis::Config::from_url(&config.redis_url)
        .create_pool(Some(deadpool_redis::Runtime::Tokio1))
        .unwrap();
    let (app, _pool) = test_app_with_config(pool, config).await;
    let paths = admin_paths();
    assert!(
        paths.len() >= 60,
        "router parsing found {} admin routes",
        paths.len()
    );
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    for path in &paths {
        let mut conn = redis.get().await.unwrap();
        let _: i64 = deadpool_redis::redis::cmd("EVAL")
            .arg("local n=0 for _,k in ipairs(redis.call('KEYS','ratelimit:*')) do redis.call('DEL',k) n=n+1 end return n")
            .arg(0)
            .query_async(&mut conn)
            .await
            .unwrap();
        drop(conn);
        for (who, token) in [
            ("patient", Some(patient.as_str())),
            ("doctor", Some(doctor.as_str())),
            ("anonymous", None),
            ("garbage token", Some("not.a.jwt")),
        ] {
            let g = settled(|| get(&app, path, token)).await;
            let empty = json!({});
            let p = settled(|| post(&app, path, token, &empty)).await;
            for (method, status) in [("GET", g), ("POST", p)] {
                assert!(
                    matches!(
                        status,
                        StatusCode::UNAUTHORIZED
                            | StatusCode::FORBIDDEN
                            | StatusCode::NOT_FOUND
                            | StatusCode::METHOD_NOT_ALLOWED
                            | StatusCode::BAD_REQUEST
                            | StatusCode::UNPROCESSABLE_ENTITY
                            | StatusCode::UNSUPPORTED_MEDIA_TYPE
                    ),
                    "{who} {method} {path} returned {status}"
                );
                if token.is_none() || token == Some("not.a.jwt") {
                    assert!(
                        matches!(
                            status,
                            StatusCode::UNAUTHORIZED | StatusCode::METHOD_NOT_ALLOWED
                        ),
                        "{who} {method} {path} reached past authentication ({status})"
                    );
                }
            }
        }
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn forged_admin_role_claim_is_rejected_and_audited(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    // A correctly signed token for a patient account that claims ADMIN: the
    // backend re-reads the real role from the database and refuses.
    let forged = make_token(uid(PATIENT_USER_ID), "ADMIN");
    for path in [
        "/api/v1/admin/team",
        "/api/v1/admin/patients",
        "/api/v1/admin/activity",
    ] {
        assert_eq!(
            get(&app, path, Some(&forged)).await.status(),
            StatusCode::UNAUTHORIZED,
            "{path}"
        );
    }
    let res = post(
        &app,
        "/api/v1/admin/team/invitations",
        Some(&forged),
        &json!({"email": "x@example.test"}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let invitations: i64 = sqlx::query_scalar("SELECT count(*) FROM admin_invitations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(invitations, 0, "nothing was created");
    let mismatches: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_events WHERE action='AUTH_ROLE_MISMATCH' AND actor_id=$1 AND result='DENIED'",
    )
    .bind(uid(PATIENT_USER_ID))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        mismatches >= 1,
        "the forgery attempt is recorded in the audit trail"
    );

    // A doctor token claiming ADMIN is refused the same way.
    let forged_doctor = make_token(uid(DOCTOR1_USER_ID), "ADMIN");
    assert_eq!(
        get(&app, "/api/v1/admin/team", Some(&forged_doctor))
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn deactivated_or_deleted_admin_loses_access_immediately(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    assert_eq!(
        get(&app, "/api/v1/admin/team", Some(&admin)).await.status(),
        StatusCode::OK
    );

    sqlx::query("UPDATE users SET is_active=false WHERE id=$1")
        .bind(uid(ADMIN_USER_ID))
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        get(&app, "/api/v1/admin/team", Some(&admin)).await.status(),
        StatusCode::UNAUTHORIZED,
        "a still-valid token stops working once the account is deactivated"
    );
    sqlx::query("UPDATE users SET is_active=true, deleted_at=NOW() WHERE id=$1")
        .bind(uid(ADMIN_USER_ID))
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        get(&app, "/api/v1/admin/team", Some(&admin)).await.status(),
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn revoked_admin_session_cannot_be_reused(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    assert_eq!(
        get(&app, "/api/v1/admin/team", Some(&admin)).await.status(),
        StatusCode::OK
    );
    sqlx::query("UPDATE auth_sessions SET revoked_at=NOW() WHERE user_id=$1")
        .bind(uid(ADMIN_USER_ID))
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        get(&app, "/api/v1/admin/team", Some(&admin)).await.status(),
        StatusCode::UNAUTHORIZED,
        "signing out (or a password reset) ends every copy of the token"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn api_responses_are_never_cacheable_by_shared_caches(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    for res in [
        get(&app, "/api/v1/admin/team", Some(&admin)).await,
        get(&app, "/api/v1/admin/team", None).await,
        get(
            &app,
            "/api/v1/facilities",
            Some(&make_token(uid(PATIENT_USER_ID), "PATIENT")),
        )
        .await,
    ] {
        assert_eq!(
            res.headers()
                .get(axum::http::header::CACHE_CONTROL)
                .and_then(|v| v.to_str().ok()),
            Some("no-store"),
            "status {}",
            res.status()
        );
    }
}
