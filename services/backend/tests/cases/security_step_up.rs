//! Admin step-up re-authentication for sensitive actions (2026-11-21).
use super::*;

/// A second, unmarked session for the seeded Admin (the fixture session is
/// pre-marked by the harness).
async fn fresh_admin_token(pool: &PgPool) -> String {
    let admin = uid(ADMIN_USER_ID);
    let family = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO auth_sessions (user_id, token_family_id, refresh_token_hash, expires_at) \
         VALUES ($1, $2, 'step-up-test-'||$2::text, now() + interval '1 day')",
    )
    .bind(admin)
    .bind(family)
    .execute(pool)
    .await
    .unwrap();
    hellodoctor_backend::crypto::create_access_token(
        admin,
        "ADMIN",
        family,
        TEST_JWT_PRIVATE_KEY_B64,
    )
    .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn sensitive_admin_actions_require_a_recent_password_reentry(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let token = fresh_admin_token(&pool).await;
    let empty = serde_json::json!({"version": 1});
    let approve = format!("/api/v1/admin/campaigns/{}/approve", Uuid::new_v4());

    // Without step-up: refused with the code Admin Web turns into a prompt.
    let res = post(&app, &approve, Some(&token), &empty).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    assert_eq!(json_body(res).await["error"]["code"], "STEP_UP_REQUIRED");
    let erase = format!(
        "/api/v1/admin/doctors/{}/data-rights/erase",
        uid(DOCTOR2_USER_ID)
    );
    let res = post(
        &app,
        &erase,
        Some(&token),
        &serde_json::json!({"reason": "test reason here"}),
    )
    .await;
    assert_eq!(json_body(res).await["error"]["code"], "STEP_UP_REQUIRED");

    // A wrong password does not step up (and is not the step-up code).
    let res = post(
        &app,
        "/api/v1/admin/step-up",
        Some(&token),
        &serde_json::json!({"current_password": "not-the-password"}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    assert_eq!(json_body(res).await["error"]["code"], "FORBIDDEN");

    // The right password covers sensitive actions for this session.
    let res = post(
        &app,
        "/api/v1/admin/step-up",
        Some(&token),
        &serde_json::json!({"current_password": SEED_ADMIN_PASSWORD}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res).await["data"]["valid_for_seconds"], 120);
    let res = post(&app, &approve, Some(&token), &empty).await;
    assert_ne!(
        res.status(),
        StatusCode::FORBIDDEN,
        "past the guard (unknown campaign)"
    );

    // Another session of the same Admin is not covered.
    let other = fresh_admin_token(&pool).await;
    let res = post(&app, &approve, Some(&other), &empty).await;
    assert_eq!(json_body(res).await["error"]["code"], "STEP_UP_REQUIRED");

    let audited: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_events WHERE action IN ('ADMIN_STEP_UP', 'ADMIN_REAUTH_FAILED')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audited, 2, "one success and one wrong password are audited");
}

#[sqlx::test(migrations = "./migrations")]
async fn only_admins_can_step_up(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let res = post(
        &app,
        "/api/v1/admin/step-up",
        Some(&patient),
        &serde_json::json!({"current_password": "anything"}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}
