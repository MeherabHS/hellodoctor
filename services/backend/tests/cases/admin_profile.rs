//! The signed-in Admin's own profile page (2026-11-22): `GET /admin/me`, the
//! Admin profile photo (`/admin/me/photo`), and changing the password with
//! the shared `POST /auth/change-password`.
use super::*;
use serde_json::json;

fn admin_token() -> String {
    make_token(uid(ADMIN_USER_ID), "ADMIN")
}

async fn body_of(res: axum::http::Response<Body>) -> (StatusCode, Value) {
    let status = res.status();
    (status, json_body(res).await)
}

#[sqlx::test(migrations = "./migrations")]
async fn admin_sees_own_account_details_without_the_full_phone_number(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let (status, body) = body_of(get(&app, "/api/v1/admin/me", Some(&admin_token())).await).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["email"], SEED_ADMIN_EMAIL);
    let phone = body["data"]["phone"].as_str().unwrap();
    assert!(
        phone.starts_with("•••• ") && phone.chars().count() <= 8,
        "{phone}"
    );
    assert_eq!(body["data"]["has_photo"], false);
}

#[sqlx::test(migrations = "./migrations")]
async fn admin_photo_upload_read_replace_and_remove(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let token = admin_token();

    let (status, _) = body_of(get(&app, "/api/v1/admin/me/photo", Some(&token)).await).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "no photo before the first upload"
    );

    let upload = json!({"mime_type": "image/jpeg", "base64_content": minimal_jpeg_base64()});
    let (status, body) =
        body_of(post(&app, "/api/v1/admin/me/photo", Some(&token), &upload).await).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, body) = body_of(get(&app, "/api/v1/admin/me/photo", Some(&token)).await).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["mime_type"], "image/jpeg");
    let bytes = STANDARD
        .decode(body["data"]["base64_content"].as_str().unwrap())
        .unwrap();
    assert!(bytes.len() <= 40 * 1024);
    assert!(image::load_from_memory(&bytes).is_ok());
    let (_, me) = body_of(get(&app, "/api/v1/admin/me", Some(&token)).await).await;
    assert_eq!(me["data"]["has_photo"], true);

    // Replacing keeps exactly one row.
    let (status, _) =
        body_of(post(&app, "/api/v1/admin/me/photo", Some(&token), &upload).await).await;
    assert_eq!(status, StatusCode::OK);
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM admin_profile_pictures")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 1);

    let (status, _) = body_of(delete(&app, "/api/v1/admin/me/photo", Some(&token)).await).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = body_of(get(&app, "/api/v1/admin/me/photo", Some(&token)).await).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (_, me) = body_of(get(&app, "/api/v1/admin/me", Some(&token)).await).await;
    assert_eq!(me["data"]["has_photo"], false);
}

#[sqlx::test(migrations = "./migrations")]
async fn admin_photo_rejects_non_images(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let pdf = STANDARD.encode(b"%PDF-1.4\n%%EOF");
    let (status, _) = body_of(
        post(
            &app,
            "/api/v1/admin/me/photo",
            Some(&admin_token()),
            &json!({"mime_type": "application/pdf", "base64_content": pdf}),
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrations = "./migrations")]
async fn only_admins_reach_the_admin_profile(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    for token in [&patient, &doctor] {
        for path in ["/api/v1/admin/me", "/api/v1/admin/me/photo"] {
            let status = get(&app, path, Some(token)).await.status();
            assert_eq!(status, StatusCode::FORBIDDEN, "{path}");
        }
    }
    assert_eq!(
        get(&app, "/api/v1/admin/me", None).await.status(),
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn admin_can_change_own_password(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let token = admin_token();
    let (status, body) = body_of(
        post(
            &app,
            "/api/v1/auth/change-password",
            Some(&token),
            &json!({"current_password": "not-the-password", "new_password": "Str0ng!NewPassw0rd"}),
        )
        .await,
    )
    .await;
    // 403, never 401: a 401 would make the Admin Web proxy sign the Admin out.
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");

    let (status, body) = body_of(
        post(
            &app,
            "/api/v1/auth/change-password",
            Some(&token),
            &json!({"current_password": SEED_ADMIN_PASSWORD, "new_password": "Str0ng!NewPassw0rd"}),
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

/// The nightly dump's status card: admin-only, and "not configured" until
/// the VM mounts the status directory (BACKUP_STATUS_DIR is unset in tests).
#[sqlx::test(migrations = "./migrations")]
async fn backup_status_is_admin_only_and_reports_not_configured(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let (status, body) = body_of(
        get(
            &app,
            "/api/v1/admin/system/backup-status",
            Some(&admin_token()),
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["state"], "NOT_CONFIGURED");
    assert_eq!(body["data"]["overdue_after_hours"], 26);
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    assert_eq!(
        get(&app, "/api/v1/admin/system/backup-status", Some(&doctor))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
}
