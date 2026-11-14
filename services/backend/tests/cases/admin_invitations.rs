//! Admin invitations (migrations/20261216000001): invite by email, accept via
//! the single-use link to create an Admin, revoke, resend and expiry.
use super::*;
use serde_json::json;

fn token_of(body: &Value) -> String {
    let url = body["data"]["dev_invite_url"]
        .as_str()
        .expect("development returns the link");
    assert!(url.starts_with("https://admin.afyamkononi.app/invite?token="));
    url.rsplit('=').next().unwrap().to_string()
}

async fn invite(app: &axum::Router, admin: &str, email: &str) -> (StatusCode, Value) {
    let res = post(
        app,
        "/api/v1/admin/team/invitations",
        Some(admin),
        &json!({"email": email, "current_password": SEED_ADMIN_PASSWORD}),
    )
    .await;
    let status = res.status();
    (status, json_body(res).await)
}

#[sqlx::test(migrations = "./migrations")]
async fn test_admin_invitation_lifecycle_creates_a_working_admin(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");

    // Only Admins.
    let res = post(
        &app,
        "/api/v1/admin/team/invitations",
        Some(&doctor),
        &json!({"email": "new.admin@example.test", "current_password": SEED_ADMIN_PASSWORD}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        get(&app, "/api/v1/admin/team", Some(&doctor))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );

    // Validation and existing accounts.
    let (status, _) = invite(&app, &admin, "not-an-email").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let taken: String = sqlx::query_scalar("SELECT email FROM users WHERE id=$1")
        .bind(uid(DOCTOR1_USER_ID))
        .fetch_one(&pool)
        .await
        .unwrap();
    let (status, _) = invite(&app, &admin, &taken).await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "an existing account cannot be invited"
    );

    let (status, body) = invite(&app, &admin, "  New.Admin@Example.test ").await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(
        body["data"]["invitation"]["email"],
        "new.admin@example.test"
    );
    let token = token_of(&body);
    let (status, _) = invite(&app, &admin, "new.admin@example.test").await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "one open invitation per email"
    );

    // Only the hash is stored.
    let stored: String = sqlx::query_scalar(
        "SELECT token_hash FROM admin_invitations WHERE email='new.admin@example.test'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_ne!(stored, token);

    let team = json_body(get(&app, "/api/v1/admin/team", Some(&admin)).await).await;
    let inv = team["data"]["invitations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["email"] == "new.admin@example.test")
        .cloned()
        .unwrap();
    assert_eq!(inv["status"], "PENDING");
    assert!(team["data"]["admins"]
        .as_array()
        .unwrap()
        .iter()
        .any(|a| a["is_you"] == true));

    // Public lookup.
    let res = post(
        &app,
        "/api/v1/auth/admin/invitations/lookup",
        None,
        &json!({"token": token}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        json_body(res).await["data"]["email"],
        "new.admin@example.test"
    );
    let bad = "0".repeat(64);
    let res = post(
        &app,
        "/api/v1/auth/admin/invitations/lookup",
        None,
        &json!({"token": bad}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // Accept: weak password refused, then success.
    let res = post(
        &app,
        "/api/v1/auth/admin/invitations/accept",
        None,
        &json!({"token": token, "phone_number": "0712000111", "password": "short"}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let password = "InvitedAdmin!2026";
    let res = post(
        &app,
        "/api/v1/auth/admin/invitations/accept",
        None,
        &json!({"token": token, "phone_number": "0712000111", "password": password}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::CREATED);

    let (role, active): (String, bool) = sqlx::query_as(
        "SELECT role::text, is_active FROM users WHERE email='new.admin@example.test'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((role.as_str(), active), ("ADMIN", true));
    let res = post(
        &app,
        "/api/v1/auth/admin/invitations/accept",
        None,
        &json!({"token": token, "phone_number": "0712000112", "password": password}),
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::UNAUTHORIZED,
        "the link is single use"
    );

    // The new Admin can start a normal sign-in (password step; OTP follows).
    let res = post(
        &app,
        "/api/v1/auth/admin/login",
        None,
        &json!({"email": "new.admin@example.test", "password": password}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);

    let team = json_body(get(&app, "/api/v1/admin/team", Some(&admin)).await).await;
    let inv = team["data"]["invitations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["email"] == "new.admin@example.test")
        .cloned()
        .unwrap();
    assert_eq!(inv["status"], "ACCEPTED");
    let audit: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_events WHERE action IN ('ADMIN_INVITATION_SENT','ADMIN_INVITATION_ACCEPTED')").fetch_one(&pool).await.unwrap();
    assert_eq!(audit, 2);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_revoke_resend_and_expiry_invalidate_old_links(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let lookup = |token: String| {
        let app = app.clone();
        async move {
            post(
                &app,
                "/api/v1/auth/admin/invitations/lookup",
                None,
                &json!({"token": token}),
            )
            .await
            .status()
        }
    };

    // Revoke.
    let (_, body) = invite(&app, &admin, "revoke.me@example.test").await;
    let token = token_of(&body);
    let id = body["data"]["invitation"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/admin/team/invitations/{id}/revoke"),
            Some(&admin),
            &json!({})
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(lookup(token).await, StatusCode::UNAUTHORIZED);
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/admin/team/invitations/{id}/revoke"),
            Some(&admin),
            &json!({})
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );

    // Resend replaces the link.
    let (_, body) = invite(&app, &admin, "resend.me@example.test").await;
    let old = token_of(&body);
    let id = body["data"]["invitation"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let res = post(
        &app,
        &format!("/api/v1/admin/team/invitations/{id}/resend"),
        Some(&admin),
        &json!({"current_password": SEED_ADMIN_PASSWORD}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let new = token_of(&json_body(res).await);
    assert_ne!(old, new);
    assert_eq!(lookup(old).await, StatusCode::UNAUTHORIZED);
    assert_eq!(lookup(new).await, StatusCode::OK);

    // Expiry: an expired link stops working and no longer blocks a new invitation.
    let (_, body) = invite(&app, &admin, "expire.me@example.test").await;
    let token = token_of(&body);
    // Simulate time passing. The guard trigger (rightly) forbids changing
    // expiry, so bypass triggers for this one test-only statement.
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SET LOCAL session_replication_role = replica")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("UPDATE admin_invitations SET created_at=now()-interval '4 days', expires_at=now()-interval '1 hour' WHERE email='expire.me@example.test'")
        .execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    assert_eq!(lookup(token).await, StatusCode::UNAUTHORIZED);
    let (status, _) = invite(&app, &admin, "expire.me@example.test").await;
    assert_eq!(status, StatusCode::CREATED);
}

/// Security: strict email (no markup / header injection / SQL-shaped input),
/// SQL-shaped tokens are plain invalid links, role checks on every Admin
/// endpoint, RLS hides invitations from non-Admin roles, and the database
/// trigger refuses tampering with a finished invitation.
#[sqlx::test(migrations = "./migrations")]
async fn test_invitation_security_controls(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");

    for email in [
        "<script>alert(1)</script>@example.test",
        "x\"onmouseover=alert(1)@example.test",
        "a\r\nBcc: victim@example.test",
        "a,b@example.test",
        "' OR '1'='1@example.test",
        "robert'); DROP TABLE users;--@example.test",
        "a..b@example.test",
    ] {
        let (status, _) = invite(&app, &admin, email).await;
        assert_eq!(
            status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "rejected: {email:?}"
        );
    }
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM admin_invitations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);

    for token in [
        "' OR '1'='1",
        "x' UNION SELECT email FROM users --",
        &"g".repeat(64),
        "",
    ] {
        let res = post(
            &app,
            "/api/v1/auth/admin/invitations/lookup",
            None,
            &json!({"token": token}),
        )
        .await;
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED, "token {token:?}");
        let res = post(
            &app,
            "/api/v1/auth/admin/invitations/accept",
            None,
            &json!({"token": token, "phone_number": "0712000222", "password": "InvitedAdmin!2026"}),
        )
        .await;
        assert!(
            res.status() == StatusCode::UNAUTHORIZED || res.status() == StatusCode::FORBIDDEN,
            "token {token:?}"
        );
    }
    let res = post(&app, "/api/v1/auth/admin/invitations/accept", None, &json!({"token": "a".repeat(64), "phone_number": "0712000222", "password": "InvitedAdmin!2026", "role": "ADMIN"})).await;
    assert!(
        res.status().is_client_error(),
        "unknown fields are rejected"
    );

    // Role checks (BOLA / function-level authorisation).
    let (_, body) = invite(&app, &admin, "secure.check@example.test").await;
    let id = body["data"]["invitation"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        get(&app, "/api/v1/admin/team", Some(&patient))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    for path in [
        format!("/api/v1/admin/team/invitations/{id}/revoke"),
        format!("/api/v1/admin/team/invitations/{id}/resend"),
    ] {
        assert_eq!(
            post(
                &app,
                &path,
                Some(&patient),
                &json!({"current_password": "x"})
            )
            .await
            .status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            post(&app, &path, None, &json!({"current_password": "x"}))
                .await
                .status(),
            StatusCode::UNAUTHORIZED
        );
    }
    let random = format!("/api/v1/admin/team/invitations/{}/revoke", Uuid::new_v4());
    assert_eq!(
        post(&app, &random, Some(&admin), &json!({})).await.status(),
        StatusCode::CONFLICT
    );

    // RLS: the API role sees no invitations unless the request context is an Admin.
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SET LOCAL ROLE hellodoctor_api")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("SELECT set_config('app.current_role', 'PATIENT', true)")
        .execute(&mut *tx)
        .await
        .unwrap();
    let visible: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM admin_invitations")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(visible, 0, "non-Admin roles cannot read invitations");
    tx.rollback().await.unwrap();

    // Trigger: a finished invitation cannot be changed or re-opened.
    post(
        &app,
        &format!("/api/v1/admin/team/invitations/{id}/revoke"),
        Some(&admin),
        &json!({}),
    )
    .await;
    let reopen = sqlx::query(
        "UPDATE admin_invitations SET revoked_at=NULL, revoked_by=NULL WHERE id=$1::uuid",
    )
    .bind(&id)
    .execute(&pool)
    .await;
    assert!(reopen.is_err(), "a revoked invitation is final");
    let (_, body) = invite(&app, &admin, "extend.me@example.test").await;
    let open_id = body["data"]["invitation"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let extend = sqlx::query(
        "UPDATE admin_invitations SET expires_at=now()+interval '30 days' WHERE id=$1::uuid",
    )
    .bind(&open_id)
    .execute(&pool)
    .await;
    assert!(extend.is_err(), "expiry and identity columns are immutable");
}

/// Activity Log and who-invited-whom (product owner, 2026-11-14).
#[sqlx::test(migrations = "./migrations")]
async fn test_activity_log_and_invited_by(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let admin_email: String = sqlx::query_scalar("SELECT email FROM users WHERE id=$1")
        .bind(uid(ADMIN_USER_ID))
        .fetch_one(&pool)
        .await
        .unwrap();

    let (_, body) = invite(&app, &admin, "logged.admin@example.test").await;
    let token = token_of(&body);
    let res = post(
        &app,
        "/api/v1/auth/admin/invitations/accept",
        None,
        &json!({"token": token, "phone_number": "0712000333", "password": "InvitedAdmin!2026"}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::CREATED);

    // Who invited whom.
    let team = json_body(get(&app, "/api/v1/admin/team", Some(&admin)).await).await;
    let new_admin = team["data"]["admins"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["email"] == "logged.admin@example.test")
        .cloned()
        .unwrap();
    assert_eq!(new_admin["invited_by"], json!(admin_email));
    let original = team["data"]["admins"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["is_you"] == true)
        .cloned()
        .unwrap();
    assert!(
        original["invited_by"].is_null(),
        "the first Admin was not invited"
    );

    // Activity log: newest first, Admin named by email, no sensitive fields.
    let log = json_body(get(&app, "/api/v1/admin/activity?area=team", Some(&admin)).await).await;
    let items = log["data"]["items"].as_array().unwrap();
    let sent = items
        .iter()
        .find(|i| i["action"] == "ADMIN_INVITATION_SENT")
        .cloned()
        .unwrap();
    assert_eq!(sent["actor_email"], json!(admin_email));
    assert_eq!(sent["is_you"], true);
    for forbidden in [
        "before_data",
        "after_data",
        "reason",
        "ip_hash",
        "request_id",
        "session_id",
    ] {
        assert!(
            sent.get(forbidden).is_none(),
            "{forbidden} must not be returned"
        );
    }
    let all = json_body(
        get(
            &app,
            "/api/v1/admin/activity?search=invitation",
            Some(&admin),
        )
        .await,
    )
    .await;
    assert!(all["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|i| i["action"] == "ADMIN_INVITATION_ACCEPTED"));

    assert_eq!(
        get(&app, "/api/v1/admin/activity", Some(&patient))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        get(&app, "/api/v1/admin/activity", None).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        get(&app, "/api/v1/admin/activity?area=everything", Some(&admin))
            .await
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let injected = get(
        &app,
        "/api/v1/admin/activity?search=x%27%20OR%201%3D1--",
        Some(&admin),
    )
    .await;
    assert_eq!(
        injected.status(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "quotes and SQL fragments are refused"
    );
    let script = get(
        &app,
        "/api/v1/admin/activity?search=%3Cscript%3Ealert(1)%3C%2Fscript%3E",
        Some(&admin),
    )
    .await;
    assert_eq!(script.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        get(&app, "/api/v1/admin/activity?unknown=1", Some(&admin))
            .await
            .status(),
        StatusCode::BAD_REQUEST,
        "unknown parameters are refused"
    );

    // Only my actions.
    let mine = json_body(get(&app, "/api/v1/admin/activity?mine=true", Some(&admin)).await).await;
    let mine = mine["data"]["items"].as_array().unwrap();
    assert!(mine.iter().any(|i| i["action"] == "ADMIN_INVITATION_SENT"));
    assert!(mine.iter().all(|i| i["is_you"] == true));
}

#[sqlx::test(migrations = "./migrations")]
async fn test_inviting_requires_the_admins_current_password(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");

    // Missing password: refused before anything happens.
    let res = post(
        &app,
        "/api/v1/admin/team/invitations",
        Some(&admin),
        &json!({"email": "pw.check@example.test"}),
    )
    .await;
    assert!(res.status().is_client_error());
    // Wrong password: 403 (the session stays valid), audited, nothing created.
    let res = post(
        &app,
        "/api/v1/admin/team/invitations",
        Some(&admin),
        &json!({"email": "pw.check@example.test", "current_password": "WrongPass123!"}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let created: i64 = sqlx::query_scalar("SELECT count(*) FROM admin_invitations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(created, 0);
    let failed: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_events WHERE action='ADMIN_REAUTH_FAILED' AND actor_id=$1",
    )
    .bind(uid(ADMIN_USER_ID))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(failed, 1, "the wrong password is in the activity log");
    assert_eq!(
        get(&app, "/api/v1/admin/team", Some(&admin)).await.status(),
        StatusCode::OK,
        "still signed in"
    );

    // Right password works; resend needs it too.
    let (status, body) = invite(&app, &admin, "pw.check@example.test").await;
    assert_eq!(status, StatusCode::CREATED);
    let id = body["data"]["invitation"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let path = format!("/api/v1/admin/team/invitations/{id}/resend");
    assert!(post(&app, &path, Some(&admin), &json!({}))
        .await
        .status()
        .is_client_error());
    assert_eq!(
        post(
            &app,
            &path,
            Some(&admin),
            &json!({"current_password": "WrongPass123!"})
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );

    // Correct passwords never count toward the cap.
    for _ in 0..6 {
        assert_eq!(
            get(&app, "/api/v1/admin/team", Some(&admin)).await.status(),
            StatusCode::OK
        );
    }
    let (status, _) = invite(&app, &admin, "pw.check2@example.test").await;
    assert_eq!(status, StatusCode::CREATED);
    // Guessing is capped: after 5 wrong passwords in 15 minutes, 429 even with the right one.
    for _ in 0..3 {
        let _ = post(
            &app,
            &path,
            Some(&admin),
            &json!({"current_password": "WrongPass123!"}),
        )
        .await;
    }
    assert_eq!(
        post(
            &app,
            &path,
            Some(&admin),
            &json!({"current_password": SEED_ADMIN_PASSWORD})
        )
        .await
        .status(),
        StatusCode::TOO_MANY_REQUESTS
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_admin_notifications_feed(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");

    // Another Admin's action (accepting an invitation) reaches this Admin; this
    // Admin's own invitation does not.
    let (_, body) = invite(&app, &admin, "notify.me@example.test").await;
    let token = token_of(&body);
    let res = post(
        &app,
        "/api/v1/auth/admin/invitations/accept",
        None,
        &json!({"token": token, "phone_number": "0712000444", "password": "InvitedAdmin!2026"}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::CREATED);
    // A new app incident reaches every Admin.
    let res = post(
        &app,
        "/api/v1/admin/system-errors/simulate",
        Some(&admin),
        &json!({"scenario": "mpesa_timeout", "severity": "CRITICAL", "custom_note": "Notification feed check"}),
    )
    .await;
    assert!(res.status().is_success());

    let feed = json_body(get(&app, "/api/v1/admin/notifications", Some(&admin)).await).await;
    let items = feed["data"]["items"].as_array().unwrap();
    assert!(items
        .iter()
        .any(|i| i["action"] == "ADMIN_INVITATION_ACCEPTED"
            && i["actor_email"] == "notify.me@example.test"));
    assert!(
        !items.iter().any(|i| i["action"] == "ADMIN_INVITATION_SENT"),
        "your own actions do not notify you"
    );
    let incident = items.iter().find(|i| i["kind"] == "incident").unwrap();
    assert_eq!(incident["severity"], "CRITICAL");
    for forbidden in [
        "error_message",
        "stack_trace",
        "impacted_user_id",
        "custom_note",
        "claim_summary",
        "patient_id",
    ] {
        assert!(
            !feed.to_string().contains(forbidden),
            "{forbidden} must not be returned"
        );
    }

    // The cursor: nothing new since the returned server time.
    let since = feed["data"]["server_time"].as_str().unwrap().to_string();
    let next = json_body(
        get(
            &app,
            &format!(
                "/api/v1/admin/notifications?since={}",
                since.replace('+', "%2B")
            ),
            Some(&admin),
        )
        .await,
    )
    .await;
    assert!(next["data"]["items"].as_array().unwrap().is_empty());

    assert_eq!(
        get(&app, "/api/v1/admin/notifications", Some(&patient))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        get(&app, "/api/v1/admin/notifications?since=x", Some(&admin))
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
}
