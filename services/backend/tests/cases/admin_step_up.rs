//! Password step-up for sensitive Admin actions (services/admin_step_up.rs).
use super::*;
use serde_json::json;

async fn step_up_app(pool: PgPool) -> (axum::Router, PgPool) {
    let mut config = test_config();
    config.admin_step_up_required = true;
    test_app_with_config(pool, config).await
}

async fn trace(app: &axum::Router, token: &str) -> (StatusCode, Value) {
    let res = post(
        app,
        "/api/v1/admin/compliance/trace",
        Some(token),
        &json!({
            "target_type": "USER",
            "target_id": ADMIN_USER_ID,
            "reason": "Step-up regression check"
        }),
    )
    .await;
    let status = res.status();
    (status, json_body(res).await)
}

async fn reauth(app: &axum::Router, token: &str, password: &str) -> (StatusCode, Value) {
    let res = post(
        app,
        "/api/v1/admin/reauth",
        Some(token),
        &json!({"current_password": password}),
    )
    .await;
    let status = res.status();
    (status, json_body(res).await)
}

fn error_code(body: &Value) -> &str {
    body["error"]["code"]
        .as_str()
        .or_else(|| body["code"].as_str())
        .unwrap_or("")
}

#[sqlx::test(migrations = "./migrations")]
async fn sensitive_admin_actions_need_a_fresh_password(pool: PgPool) {
    let (app, pool) = step_up_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");

    // Without a confirmation the action is refused with REAUTH_REQUIRED.
    let (status, body) = trace(&app, &admin).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(error_code(&body), "REAUTH_REQUIRED", "{body}");

    // Doctor verification is gated the same way.
    let res = post(
        &app,
        &format!("/api/v1/admin/doctors/{}/verify", Uuid::new_v4()),
        Some(&admin),
        &json!({}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    assert_eq!(error_code(&json_body(res).await), "REAUTH_REQUIRED");

    // So is revoking an Admin invitation.
    let res = post(
        &app,
        &format!("/api/v1/admin/team/invitations/{}/revoke", Uuid::new_v4()),
        Some(&admin),
        &json!({}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    assert_eq!(error_code(&json_body(res).await), "REAUTH_REQUIRED");

    // A wrong password does not unlock anything and is audited.
    let (status, body) = reauth(&app, &admin, "not-the-password").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(error_code(&body), "FORBIDDEN");
    let (_, body) = trace(&app, &admin).await;
    assert_eq!(error_code(&body), "REAUTH_REQUIRED");
    let failures: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_events WHERE action='ADMIN_REAUTH_FAILED' AND resource_type='ADMIN_STEP_UP'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(failures, 1);

    // The right password unlocks this sign-in session.
    let (status, body) = reauth(&app, &admin, SEED_ADMIN_PASSWORD).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["valid_for_seconds"], 600);
    let (status, body) = trace(&app, &admin).await;
    assert_ne!(error_code(&body), "REAUTH_REQUIRED", "{status} {body}");
    assert_ne!(status, StatusCode::FORBIDDEN, "{body}");

    // ...but not another sign-in session of the same Admin.
    let other_family = Uuid::new_v4();
    sqlx::query("INSERT INTO auth_sessions(id,user_id,token_family_id,refresh_token_hash,created_at,last_used_at,expires_at) VALUES(gen_random_uuid(),$1,$2,$3,NOW(),NOW(),NOW()+INTERVAL '7 days')")
        .bind(uid(ADMIN_USER_ID))
        .bind(other_family)
        .bind(format!("step-up-other-{other_family}"))
        .execute(&pool)
        .await
        .unwrap();
    let other_session = hellodoctor_backend::crypto::create_access_token(
        uid(ADMIN_USER_ID),
        "ADMIN",
        other_family,
        TEST_JWT_PRIVATE_KEY_B64,
    )
    .unwrap();
    let (status, body) = trace(&app, &other_session).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(error_code(&body), "REAUTH_REQUIRED");
}

#[sqlx::test(migrations = "./migrations")]
async fn only_admins_can_confirm_a_password_step_up(pool: PgPool) {
    let (app, _pool) = step_up_app(pool).await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let (status, body) = reauth(&app, &doctor, "anything").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(error_code(&body), "FORBIDDEN");
}
