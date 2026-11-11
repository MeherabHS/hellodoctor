use super::*;
use hellodoctor_backend::{auth_context::AuthContext, services::article_media_service};
use serde_json::json;

async fn category(app: &axum::Router, admin: &str, sensitive: bool) -> Value {
    let res=post(app,"/api/v1/admin/article-categories",Some(admin),&json!({"name_en":if sensitive{"Medical education"}else{"Platform updates"},"name_sw":if sensitive{"Elimu ya afya"}else{"Habari za jukwaa"},"sensitive":sensitive})).await;
    assert_eq!(res.status(), StatusCode::OK);
    json_body(res).await["data"].clone()
}
fn content(category: &Value, sensitive: bool) -> Value {
    json!({"title_en":"A healthy start","title_sw":"Mwanzo wa afya","body_en":"## Guidance\n\nRead **carefully**.\n\n- First\n- Second\n\n[Reference](https://example.org)","body_sw":"## Mwongozo\n\nSoma **kwa makini**.","category_id":category["id"],"tags":["education"],"featured":true,"sensitive":sensitive})
}
async fn draft(app: &axum::Router, admin: &str, category: &Value, sensitive: bool) -> Value {
    let res = post(
        app,
        "/api/v1/admin/articles",
        Some(admin),
        &json!({"id":Uuid::new_v4(),"content":content(category,sensitive)}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    json_body(res).await["data"].clone()
}
async fn step(app: &axum::Router, token: &str, a: &Value, action: &str) -> Value {
    let res = post(
        app,
        &format!(
            "/api/v1/admin/articles/{}/{action}",
            a["id"].as_str().unwrap()
        ),
        Some(token),
        &json!({"version":a["version"]}),
    )
    .await;
    let status = res.status();
    let data = json_body(res).await;
    assert_eq!(status, StatusCode::OK, "{action}: {data}");
    data["data"].clone()
}
async fn reviewer(pool: &PgPool) -> String {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,phone_number,role,is_active,is_verified) VALUES($1,$2,'ADMIN',true,true)").bind(id).bind(format!("+2547{:08}",rand::thread_rng().gen_range(0..100_000_000))).execute(pool).await.unwrap();
    fixture_session(pool, id).await;
    make_token(id, "ADMIN")
}
fn actor(role: UserRole, id: &str) -> AuthContext {
    AuthContext {
        user_id: uid(id).into(),
        role,
        permissions: vec![],
        session_id: None,
        request_id: None,
    }
}
async fn image_upload(
    app: &axum::Router,
    id: &Value,
    version: &Value,
    token: Option<&str>,
    bytes: Vec<u8>,
    mime: &str,
) -> axum::http::Response<Body> {
    let mut request = Request::builder()
        .method("POST")
        .uri(format!(
            "/api/v1/admin/articles/{}/cover?version={}",
            id.as_str().unwrap(),
            version.as_i64().unwrap()
        ))
        .header("Content-Type", mime);
    if let Some(token) = token {
        request = request.header("Authorization", format!("Bearer {token}"));
    }
    let mut request = request.body(Body::from(bytes)).unwrap();
    request.extensions_mut().insert(random_peer());
    app.clone().oneshot(request).await.unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn test_cms_authorization_and_category_validation(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let cat = category(&app, &admin, true).await;
    for role in ["PATIENT", "DOCTOR"] {
        let token = make_token(
            uid(if role == "PATIENT" {
                PATIENT_USER_ID
            } else {
                DOCTOR1_USER_ID
            }),
            role,
        );
        for path in ["/api/v1/admin/articles", "/api/v1/admin/article-categories"] {
            assert_eq!(
                get(&app, path, Some(&token)).await.status(),
                StatusCode::FORBIDDEN
            );
        }
        assert_eq!(
            post(
                &app,
                "/api/v1/admin/articles",
                Some(&token),
                &json!({"id":Uuid::new_v4(),"content":content(&cat,true)})
            )
            .await
            .status(),
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(
        post(
            &app,
            "/api/v1/admin/articles",
            None,
            &json!({"id":Uuid::new_v4(),"content":content(&cat,true)})
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        post(
            &app,
            "/api/v1/admin/article-categories",
            Some(&admin),
            &json!({"name_en":" medical EDUCATION ","name_sw":"Other","sensitive":true})
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    let mut c = content(&cat, true);
    c["category_id"] = json!(Uuid::new_v4());
    assert_eq!(
        post(
            &app,
            "/api/v1/admin/articles",
            Some(&admin),
            &json!({"id":Uuid::new_v4(),"content":c})
        )
        .await
        .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM admin_articles")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}
#[sqlx::test(migrations = "./migrations")]
async fn test_cms_sensitive_review_and_publication_audit_without_notifications(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let reviewer = reviewer(&pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let cat = category(&app, &admin, true).await;
    let a = draft(&app, &admin, &cat, false).await;
    let path = format!("/api/v1/articles/{}", a["id"].as_str().unwrap());
    assert_eq!(
        get(&app, &path, Some(&patient)).await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        post(
            &app,
            &format!(
                "/api/v1/admin/articles/{}/publish",
                a["id"].as_str().unwrap()
            ),
            Some(&admin),
            &json!({"version":1})
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    let a = step(&app, &admin, &a, "submit").await;
    assert_eq!(a["can_approve"], false);
    assert_eq!(a["approval_block_reason"], "INDEPENDENT_REVIEW_REQUIRED");
    let admin_path = format!("/api/v1/admin/articles/{}", a["id"].as_str().unwrap());
    let reviewer_view = json_body(get(&app, &admin_path, Some(&reviewer)).await).await;
    assert_eq!(reviewer_view["data"]["can_approve"], true);
    assert!(reviewer_view["data"]["approval_block_reason"].is_null());
    assert_eq!(
        post(
            &app,
            &format!(
                "/api/v1/admin/articles/{}/approve",
                a["id"].as_str().unwrap()
            ),
            Some(&admin),
            &json!({"version":a["version"]})
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    let after_rejection = json_body(get(&app, &admin_path, Some(&admin)).await).await;
    assert_eq!(after_rejection["data"]["status"], "PENDING_REVIEW");
    assert_eq!(after_rejection["data"]["version"], a["version"]);
    let a = step(&app, &reviewer, &a, "approve").await;
    assert_eq!(a["can_approve"], false);
    assert_eq!(a["approved_version"], a["content_version"]);
    let a = step(&app, &admin, &a, "publish").await;
    let published = json_body(get(&app, &path, Some(&patient)).await).await;
    assert_eq!(published["data"]["article_type"], "ADMIN");
    assert_eq!(published["data"]["author_name"], "AfyaMkononi Editorial");
    assert_eq!(published["data"]["title_sw"], "Mwanzo wa afya");
    assert_eq!(published["data"]["content_format"], "markdown");
    assert!(published["data"].get("can_approve").is_none());
    assert!(published["data"].get("approval_block_reason").is_none());
    let a = step(&app, &admin, &a, "unpublish").await;
    assert!(a["approved_version"].is_null());
    assert_eq!(
        get(&app, &path, Some(&patient)).await.status(),
        StatusCode::NOT_FOUND
    );
    let a = step(&app, &admin, &a, "archive").await;
    assert_eq!(a["status"], "ARCHIVED");
    let events:Vec<String>=sqlx::query_scalar("SELECT action FROM audit_events WHERE resource_type='ADMIN_ARTICLE' ORDER BY created_at,id").fetch_all(&pool).await.unwrap();
    for action in [
        "CREATE",
        "SUBMIT",
        "APPROVE",
        "PUBLISH",
        "UNPUBLISH",
        "ARCHIVE",
    ] {
        assert!(events.contains(&format!("ADMIN_ARTICLE_{action}")));
    }
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM notifications WHERE related_entity_id=$1")
            .bind(uid(a["id"].as_str().unwrap()))
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 0);
}
#[sqlx::test(migrations = "./migrations")]
async fn test_cms_edit_invalidates_approval_schedule_and_stale_versions(pool: PgPool) {
    let (app, _) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let cat = category(&app, &admin, false).await;
    let a = draft(&app, &admin, &cat, false).await;
    let a = step(&app, &admin, &a, "submit").await;
    let a = step(&app, &admin, &a, "approve").await;
    let res = post(
        &app,
        &format!(
            "/api/v1/admin/articles/{}/schedule",
            a["id"].as_str().unwrap()
        ),
        Some(&admin),
        &json!({"version":a["version"],"scheduled_for":Utc::now()+Duration::hours(2)}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let a = json_body(res).await["data"].clone();
    let mut c = content(&cat, false);
    c["body_en"] = json!("## Revised\n\nNew exact content");
    let path = format!("/api/v1/admin/articles/{}", a["id"].as_str().unwrap());
    let res = send(
        &app,
        "PATCH",
        &path,
        Some(&admin),
        Some(&json!({"version":a["version"],"content":c})),
        None,
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let updated = json_body(res).await["data"].clone();
    assert_eq!(updated["status"], "DRAFT");
    assert_eq!(updated["content_version"], 2);
    assert!(updated["approved_version"].is_null());
    assert!(updated["scheduled_for"].is_null());
    assert_eq!(
        post(
            &app,
            &format!("{path}/publish"),
            Some(&admin),
            &json!({"version":a["version"]})
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(
        send(
            &app,
            "PATCH",
            &path,
            Some(&admin),
            Some(&json!({"version":1,"content":c})),
            None
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    let submitted = step(&app, &admin, &updated, "submit").await;
    let returned = step(&app, &admin, &submitted, "return").await;
    assert_eq!(returned["status"], "DRAFT");
}
#[sqlx::test(migrations = "./migrations")]
async fn test_cms_concurrent_publish_has_one_winner(pool: PgPool) {
    let (app, _) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let cat = category(&app, &admin, false).await;
    let a = draft(&app, &admin, &cat, false).await;
    let a = step(&app, &admin, &a, "submit").await;
    let a = step(&app, &admin, &a, "approve").await;
    let path = format!(
        "/api/v1/admin/articles/{}/publish",
        a["id"].as_str().unwrap()
    );
    let body = json!({"version":a["version"]});
    let (a, b) = tokio::join!(
        post(&app, &path, Some(&admin), &body),
        post(&app, &path, Some(&admin), &body)
    );
    let mut statuses = [a.status().as_u16(), b.status().as_u16()];
    statuses.sort();
    assert_eq!(statuses, [200, 409]);
}
#[sqlx::test(migrations = "./migrations")]
async fn test_cms_rls_drafts_writes_and_transaction_context(pool: PgPool) {
    let (app, _, state) = test_app_and_state(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let cat = category(&app, &admin, true).await;
    let a = draft(&app, &admin, &cat, true).await;
    let patient = actor(UserRole::Patient, PATIENT_USER_ID);
    let mut tx = state.begin_authed(&patient).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM admin_articles")
            .fetch_one(&mut *tx)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query("UPDATE admin_articles SET title_en='Intrusion',version=version+1 WHERE id=$1")
            .bind(uid(a["id"].as_str().unwrap()))
            .execute(&mut *tx)
            .await
            .unwrap()
            .rows_affected(),
        0
    );
    tx.commit().await.unwrap();
    let mut tx = state
        .begin_authed(&actor(UserRole::Admin, ADMIN_USER_ID))
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM admin_articles")
            .fetch_one(&mut *tx)
            .await
            .unwrap(),
        1
    );
    tx.commit().await.unwrap();
    // A pooled connection has no caller context outside the transaction.
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM admin_articles")
            .fetch_one(&state.db)
            .await
            .unwrap(),
        0
    );
}
#[sqlx::test(migrations = "./migrations")]
async fn test_cms_database_guards_prevent_forged_approval(pool: PgPool) {
    let (app, _, state) = test_app_and_state(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let cat = category(&app, &admin, true).await;
    let a = draft(&app, &admin, &cat, true).await;
    let id = uid(a["id"].as_str().unwrap());
    let mut tx = state
        .begin_authed(&actor(UserRole::Admin, ADMIN_USER_ID))
        .await
        .unwrap();
    assert!(sqlx::query("UPDATE admin_articles SET status='PUBLISHED',approved_version=content_version,approved_by=author_id,approved_at=now(),published_at=now(),version=version+1 WHERE id=$1").bind(id).execute(&mut *tx).await.is_err());
    tx.rollback().await.unwrap();
    let a = step(&app, &admin, &a, "submit").await;
    assert_eq!(a["status"], "PENDING_REVIEW");
    let mut tx = state
        .begin_authed(&actor(UserRole::Admin, ADMIN_USER_ID))
        .await
        .unwrap();
    assert!(sqlx::query("UPDATE admin_articles SET status='APPROVED',approved_version=content_version,approved_by=author_id,approved_at=now(),version=version+1 WHERE id=$1").bind(id).execute(&mut *tx).await.is_err());
}
#[sqlx::test(migrations = "./migrations")]
async fn test_cms_unified_feed_retains_doctor_ids_and_pagination(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let res=post(&app,"/api/v1/doctor/articles",Some(&doctor),&json!({"creation_key":Uuid::new_v4().to_string(),"content":{"title":"Doctor article","summary":"Physician summary","body":"Unchanged physician plain text","references_list":[]}})).await;
    assert_eq!(res.status(), StatusCode::OK);
    let d = json_body(res).await["data"].clone();
    assert_eq!(
        post(
            &app,
            &format!(
                "/api/v1/doctor/articles/{}/publish",
                d["id"].as_str().unwrap()
            ),
            Some(&doctor),
            &json!({"version":d["version"]})
        )
        .await
        .status(),
        StatusCode::OK
    );
    let cat = category(&app, &admin, false).await;
    let a = draft(&app, &admin, &cat, false).await;
    let a = step(&app, &admin, &a, "submit").await;
    let a = step(&app, &admin, &a, "approve").await;
    let a = step(&app, &admin, &a, "publish").await;
    let _draft = draft(&app, &admin, &cat, false).await;
    let feed = json_body(get(&app, "/api/v1/articles?limit=50", Some(&patient)).await).await;
    let rows = feed["data"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["id"], a["id"]);
    assert_eq!(rows[1]["id"], d["id"]);
    assert_eq!(rows[1]["article_type"], "DOCTOR");
    assert_eq!(rows[1]["body"], "Unchanged physician plain text");
    let page =
        json_body(get(&app, "/api/v1/articles?limit=1&offset=1", Some(&patient)).await).await;
    assert_eq!(page["data"][0]["id"], d["id"]);
    let own = json_body(get(&app, "/api/v1/doctor/articles", Some(&doctor)).await).await;
    assert_eq!(own["data"].as_array().unwrap().len(), 1);
    assert_eq!(
        get(
            &app,
            &format!("/api/v1/articles/{}", d["id"].as_str().unwrap()),
            Some(&patient)
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM admin_articles")
            .fetch_one(&pool)
            .await
            .unwrap(),
        2
    );
}
#[sqlx::test(migrations = "./migrations")]
async fn test_cms_schedule_worker_idempotency_restart_and_invalidated_approval(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let cat = category(&app, &admin, false).await;
    let a = draft(&app, &admin, &cat, false).await;
    let a = step(&app, &admin, &a, "submit").await;
    let a = step(&app, &admin, &a, "approve").await;
    let res = post(
        &app,
        &format!(
            "/api/v1/admin/articles/{}/schedule",
            a["id"].as_str().unwrap()
        ),
        Some(&admin),
        &json!({"version":a["version"],"scheduled_for":Utc::now()+Duration::seconds(1)}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    article_media_service::maintenance(&state).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM admin_articles WHERE id=$1")
            .bind(uid(a["id"].as_str().unwrap()))
            .fetch_one(&pool)
            .await
            .unwrap(),
        "SCHEDULED"
    );
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    let (left, right) = tokio::join!(
        article_media_service::maintenance(&state),
        article_media_service::maintenance(&state)
    );
    left.unwrap();
    right.unwrap();
    article_media_service::maintenance(&state).await.unwrap();
    let a = json_body(
        get(
            &app,
            &format!("/api/v1/admin/articles/{}", a["id"].as_str().unwrap()),
            Some(&admin),
        )
        .await,
    )
    .await["data"]
        .clone();
    assert_eq!(a["status"], "PUBLISHED");
    let count:i64=sqlx::query_scalar("SELECT count(*) FROM audit_events WHERE resource_id=$1 AND action='ADMIN_ARTICLE_SCHEDULED_PUBLISH'").bind(uid(a["id"].as_str().unwrap())).fetch_one(&pool).await.unwrap();
    assert_eq!(count, 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM notifications WHERE related_entity_id=$1"
        )
        .bind(uid(a["id"].as_str().unwrap()))
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
}
#[sqlx::test(migrations = "./migrations")]
async fn test_cms_bilingual_validation_listing_and_category_crud(pool: PgPool) {
    let (app, _) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let cat = category(&app, &admin, false).await;
    let mut c = content(&cat, false);
    c["body_sw"] = json!("");
    assert_eq!(
        post(
            &app,
            "/api/v1/admin/articles",
            Some(&admin),
            &json!({"id":Uuid::new_v4(),"content":c})
        )
        .await
        .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    c["title_sw"] = json!("");
    let res = post(
        &app,
        "/api/v1/admin/articles",
        Some(&admin),
        &json!({"id":Uuid::new_v4(),"content":c}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let list = json_body(
        get(
            &app,
            "/api/v1/admin/articles?search=healthy&status=DRAFT&limit=1",
            Some(&admin),
        )
        .await,
    )
    .await;
    assert_eq!(list["data"]["total"], 1);
    assert_eq!(list["data"]["items"].as_array().unwrap().len(), 1);
    let list = json_body(
        get(
            &app,
            "/api/v1/admin/articles?search=%27%20OR%20true--",
            Some(&admin),
        )
        .await,
    )
    .await;
    assert_eq!(list["data"]["total"], 0);
    assert_eq!(
        delete(
            &app,
            &format!(
                "/api/v1/admin/article-categories/{}",
                cat["id"].as_str().unwrap()
            ),
            Some(&admin)
        )
        .await
        .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let unused = category(&app, &admin, true).await;
    let path = format!(
        "/api/v1/admin/article-categories/{}",
        unused["id"].as_str().unwrap()
    );
    assert_eq!(
        send(
            &app,
            "PATCH",
            &path,
            Some(&admin),
            Some(&json!({"name_en":"Revised","name_sw":"Mpya","sensitive":true})),
            None
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        delete(&app, &path, Some(&admin)).await.status(),
        StatusCode::OK
    );
}
#[sqlx::test(migrations = "./migrations")]
async fn test_cms_cover_authorization_and_private_storage(pool: PgPool) {
    let (app, _, state) = test_app_and_state(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let cat = category(&app, &admin, false).await;
    let a = draft(&app, &admin, &cat, false).await;
    let bytes = STANDARD.decode(minimal_jpeg_base64()).unwrap();
    assert_eq!(
        image_upload(
            &app,
            &a["id"],
            &a["version"],
            Some(&patient),
            bytes.clone(),
            "image/jpeg"
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    let res = image_upload(
        &app,
        &a["id"],
        &a["version"],
        Some(&admin),
        bytes,
        "image/jpeg",
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let a = json_body(res).await["data"].clone();
    assert_eq!(a["content_version"], 2);
    let path = format!("/api/v1/articles/{}/cover", a["id"].as_str().unwrap());
    assert_eq!(
        get(&app, &path, Some(&patient)).await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        get(&app, &path, None).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        get(
            &app,
            &format!("/api/v1/admin/articles/{}/cover", a["id"].as_str().unwrap()),
            Some(&admin)
        )
        .await
        .status(),
        StatusCode::OK
    );
    let media = uid(a["cover_image_id"].as_str().unwrap());
    let object = state
        .object_storage
        .as_ref()
        .unwrap()
        .get(&object_store::path::Path::from(format!(
            "article-media/{media}.jpg"
        )))
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap();
    assert!(!object.starts_with(&[0xff, 0xd8]));
    let a = step(&app, &admin, &a, "submit").await;
    let a = step(&app, &admin, &a, "approve").await;
    let a = step(&app, &admin, &a, "publish").await;
    let res = get(&app, &path, Some(&patient)).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()["cache-control"], "private, no-store");
    assert_eq!(res.headers()["x-content-type-options"], "nosniff");
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        image::guess_format(&bytes).unwrap(),
        image::ImageFormat::Jpeg
    );
    let a = step(&app, &admin, &a, "unpublish").await;
    assert_eq!(a["status"], "DRAFT");
    assert_eq!(
        get(&app, &path, Some(&patient)).await.status(),
        StatusCode::NOT_FOUND
    );
}
#[sqlx::test(migrations = "./migrations")]
async fn test_cms_cover_rejects_malformed_spoofed_oversized_and_unconfigured_storage(pool: PgPool) {
    let (app, pool, mut state) = test_app_and_state(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let cat = category(&app, &admin, false).await;
    let a = draft(&app, &admin, &cat, false).await;
    for (bytes, mime) in [
        (b"<script>alert(1)</script>".to_vec(), "image/png"),
        (STANDARD.decode(minimal_jpeg_base64()).unwrap(), "image/png"),
        (b"\xff\xd8\xff\xe0".to_vec(), "image/jpeg"),
    ] {
        assert_eq!(
            image_upload(&app, &a["id"], &a["version"], Some(&admin), bytes, mime)
                .await
                .status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    assert_eq!(
        image_upload(
            &app,
            &a["id"],
            &a["version"],
            Some(&admin),
            vec![0; article_media_service::MAX_COVER_UPLOAD + 1],
            "image/jpeg"
        )
        .await
        .status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM article_media")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    state.object_storage = None;
    state.article_media_storage = None;
    let app = create_router(state);
    assert_eq!(
        image_upload(
            &app,
            &a["id"],
            &a["version"],
            Some(&admin),
            STANDARD.decode(minimal_jpeg_base64()).unwrap(),
            "image/jpeg"
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_cms_abandoned_media_cleanup_preserves_current_and_clinical_objects(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let cat = category(&app, &admin, false).await;
    let a = draft(&app, &admin, &cat, false).await;
    let res = image_upload(
        &app,
        &a["id"],
        &a["version"],
        Some(&admin),
        STANDARD.decode(minimal_jpeg_base64()).unwrap(),
        "image/jpeg",
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let a = json_body(res).await["data"].clone();
    let store = state.article_media_storage.as_ref().unwrap();
    let abandoned = Uuid::new_v4();
    let key = format!("article-media/{abandoned}.jpg");
    sqlx::query("INSERT INTO article_media(id,article_id,object_key,content_hash,size_bytes,created_at) VALUES($1,$2,$3,'unused',1,now()-interval '2 days')").bind(abandoned).bind(uid(a["id"].as_str().unwrap())).bind(&key).execute(&pool).await.unwrap();
    store
        .put(
            &object_store::path::Path::from(key.clone()),
            vec![0u8].into(),
        )
        .await
        .unwrap();
    let clinical = format!("clinical/{}.enc", Uuid::new_v4());
    store
        .put(
            &object_store::path::Path::from(clinical.clone()),
            vec![1u8].into(),
        )
        .await
        .unwrap();
    sqlx::query("UPDATE article_media SET created_at=now()-interval '2 days' WHERE id=$1")
        .bind(uid(a["cover_image_id"].as_str().unwrap()))
        .execute(&pool)
        .await
        .unwrap();
    article_media_service::maintenance(&state).await.unwrap();
    article_media_service::maintenance(&state).await.unwrap();
    assert!(store
        .get(&object_store::path::Path::from(key))
        .await
        .is_err());
    assert!(store
        .get(&object_store::path::Path::from(clinical))
        .await
        .is_ok());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM article_media WHERE article_id=$1")
            .bind(uid(a["id"].as_str().unwrap()))
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_cms_failed_audit_rolls_back_publication(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let cat = category(&app, &admin, false).await;
    let a = draft(&app, &admin, &cat, false).await;
    let a = step(&app, &admin, &a, "submit").await;
    let a = step(&app, &admin, &a, "approve").await;
    sqlx::raw_sql("CREATE FUNCTION cms_test_fail_audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.action='ADMIN_ARTICLE_PUBLISH' THEN RAISE EXCEPTION 'isolated audit failure'; END IF; RETURN NEW; END $$; CREATE TRIGGER cms_test_fail_audit BEFORE INSERT ON audit_events FOR EACH ROW EXECUTE FUNCTION cms_test_fail_audit();").execute(&pool).await.unwrap();
    assert_eq!(
        post(
            &app,
            &format!(
                "/api/v1/admin/articles/{}/publish",
                a["id"].as_str().unwrap()
            ),
            Some(&admin),
            &json!({"version":a["version"]})
        )
        .await
        .status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    let status: String = sqlx::query_scalar("SELECT status FROM admin_articles WHERE id=$1")
        .bind(uid(a["id"].as_str().unwrap()))
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "APPROVED");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_cms_migration_round_trip_and_doctor_id_collision_guard(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let res=post(&app,"/api/v1/doctor/articles",Some(&doctor),&json!({"creation_key":Uuid::new_v4().to_string(),"content":{"title":"Private Doctor draft","summary":"Summary","body":"Unchanged","references_list":[]}})).await;
    assert_eq!(res.status(), StatusCode::OK);
    let d = json_body(res).await["data"].clone();
    let cat = category(&app, &admin, false).await;
    assert_eq!(
        post(
            &app,
            "/api/v1/admin/articles",
            Some(&admin),
            &json!({"id":d["id"],"content":content(&cat,false)})
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    let policies:Value=sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(p) ORDER BY policyname) FROM pg_policies p WHERE tablename='doctor_articles'").fetch_one(&pool).await.unwrap();
    hellodoctor_backend::migration_status::MIGRATOR
        .undo(&pool, 20261126000002)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM doctor_articles")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    hellodoctor_backend::migration_status::MIGRATOR
        .run(&pool)
        .await
        .unwrap();
    let restored:Value=sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(p) ORDER BY policyname) FROM pg_policies p WHERE tablename='doctor_articles'").fetch_one(&pool).await.unwrap();
    assert_eq!(restored, policies);
}
