use super::admin_quizzes::{
    admin, article, article_step, data, patient, publish_quiz, question, save,
};
use super::*;
use serde_json::json;

async fn doctor_article(app: &axum::Router, title: &str) -> Value {
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let row = data(post(app,"/api/v1/doctor/articles",Some(&doctor),&json!({"creation_key":Uuid::new_v4().to_string(),"content":{"title":title,"summary":"Educational reading","body":"Safe educational article","references_list":[]}})).await).await;
    data(
        post(
            app,
            &format!(
                "/api/v1/doctor/articles/{}/publish",
                row["id"].as_str().unwrap()
            ),
            Some(&doctor),
            &json!({"version":row["version"]}),
        )
        .await,
    )
    .await
}

async fn page(app: &axum::Router, suffix: &str) -> Value {
    let response = get(app, &format!("/api/v1/articles?{suffix}"), Some(&patient())).await;
    assert_eq!(response.status(), StatusCode::OK);
    json_body(response).await
}
fn cursor_query(page: &Value) -> String {
    let cursor = &page["next_cursor"];
    // RFC3339 UTC is emitted with Z; no unescaped '+' in the query string.
    format!(
        "limit=20&before_time={}&before_id={}",
        cursor["before_time"].as_str().unwrap(),
        cursor["before_id"].as_str().unwrap()
    )
}

#[sqlx::test(migrations = "./migrations")]
async fn test_d_discovery_search_uses_complete_sw_translation(pool: PgPool) {
    let (app, _) = test_app(pool).await;
    let a = article(&app, false, false).await;
    let updated = data(send(&app,"PATCH",&format!("/api/v1/admin/articles/{}",a["id"].as_str().unwrap()),Some(&admin()),Some(&json!({"version":a["version"],"content":{"title_en":"English education","body_en":"Complete English guidance.","title_sw":"Mwongozo maalumu","body_sw":"Elimu ya afya kwa wote.","category_id":a["category_id"],"sensitive":false}})),None).await).await;
    let mut published = updated;
    for action in ["submit", "approve", "publish"] {
        published = article_step(&app, &published, &admin(), action).await;
    }
    let sw = page(&app, "language=sw&search=Mwongozo%20maalumu").await;
    assert_eq!(sw["data"][0]["id"], published["id"]);
    assert_eq!(sw["data"][0]["body_sw"], "Elimu ya afya kwa wote.");
    assert!(
        page(&app, "language=en&search=Mwongozo%20maalumu").await["data"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_d_article_archive_search_and_mixed_cursor_pages(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let mut ids = std::collections::HashSet::new();
    for index in 0..25 {
        let title = if index == 0 {
            "D-archive-needle".into()
        } else {
            format!("D-guide-{index}")
        };
        let row = if index < 5 {
            // Synthetic historical rows with tied publication timestamps.
            // Never mutate immutable published timestamps or disable guards.
            let id = Uuid::new_v4();
            sqlx::query("INSERT INTO doctor_articles(id,author_id,title,summary,body,status,version,creation_key,creation_hash,published_at) VALUES($1,$2,$3,'Educational reading','Safe educational article','PUBLISHED',2,$4,'synthetic-fixture','2026-01-01T00:00:00Z')")
                .bind(id).bind(uid(DOCTOR1_PROFILE_ID)).bind(&title).bind(Uuid::new_v4().to_string()).execute(&pool).await.unwrap();
            json!({"id":id})
        } else {
            doctor_article(&app, &title).await
        };
        ids.insert(row["id"].as_str().unwrap().to_owned());
    }
    let published = article(&app, false, true).await;
    ids.insert(published["id"].as_str().unwrap().to_owned());
    let draft = article(&app, false, false).await;
    let first = page(&app, "limit=20").await;
    assert_eq!(first["data"].as_array().unwrap().len(), 20);
    assert!(first["data"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["article_type"] == "ADMIN"));
    assert!(first["data"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["article_type"] == "DOCTOR"));
    let last = page(&app, &cursor_query(&first)).await;
    let all: Vec<_> = first["data"]
        .as_array()
        .unwrap()
        .iter()
        .chain(last["data"].as_array().unwrap())
        .collect();
    assert_eq!(all.len(), 26);
    assert_eq!(
        all.iter()
            .map(|r| r["id"].as_str().unwrap().to_owned())
            .collect::<std::collections::HashSet<_>>(),
        ids
    );
    assert!(last["next_cursor"].is_null());
    assert!(!all.iter().any(|r| r["id"] == draft["id"]));
    let search = page(&app, "search=D-archive-needle").await;
    assert_eq!(search["data"].as_array().unwrap().len(), 1);
    assert_eq!(search["data"][0]["title"], "D-archive-needle");
    let legacy = page(&app, "limit=20&offset=20").await;
    assert_eq!(legacy["data"], last["data"]);
    // Literal '%' must not turn into a wildcard or change visibility.
    assert!(page(&app, "search=%25").await["data"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[sqlx::test(migrations = "./migrations")]
async fn test_d_cursor_survives_new_publication_and_archive_refresh(pool: PgPool) {
    let (app, _) = test_app(pool).await;
    for index in 0..22 {
        doctor_article(&app, &format!("D cursor {index}")).await;
    }
    let first = page(&app, "limit=20").await;
    let newest = doctor_article(&app, "D new publication").await;
    let next = page(&app, &cursor_query(&first)).await;
    assert_eq!(next["data"].as_array().unwrap().len(), 2);
    let first_ids: Vec<_> = first["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| &r["id"])
        .collect();
    assert!(next["data"]
        .as_array()
        .unwrap()
        .iter()
        .all(|r| !first_ids.contains(&&r["id"]) && r["id"] != newest["id"]));
    let refreshed = page(&app, "limit=20").await;
    assert_eq!(refreshed["data"][0]["id"], newest["id"]);
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    data(
        post(
            &app,
            &format!(
                "/api/v1/doctor/articles/{}/archive",
                next["data"][0]["id"].as_str().unwrap()
            ),
            Some(&doctor),
            &json!({"version":next["data"][0]["version"]}),
        )
        .await,
    )
    .await;
    assert_eq!(
        page(&app, &cursor_query(&first)).await["data"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_d_discovery_validation_language_fallback_and_quiz_isolation(pool: PgPool) {
    let (app, _) = test_app(pool).await;
    let a = article(&app, false, true).await;
    let document = save(
        &app,
        &a,
        &Value::Null,
        &json!({"include_quiz":true,"questions":[question("Protected answer key")]}),
    )
    .await;
    publish_quiz(&app, &a, document).await;
    let sw = page(&app, "language=sw&search=Platform%20guide").await;
    assert_eq!(sw["data"][0]["id"], a["id"]);
    assert_eq!(sw["data"][0]["quiz"]["included"], true);
    for hidden in [
        "correct_option_id",
        "explanation_en",
        "Protected answer key",
    ] {
        assert!(!sw.to_string().contains(hidden));
    }
    let unpublished = article_step(&app, &a, &admin(), "unpublish").await;
    assert_eq!(unpublished["status"], "DRAFT");
    assert!(
        page(&app, "language=sw&search=Platform%20guide").await["data"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    for suffix in [
        "language=xx".to_owned(),
        format!("before_id={}", Uuid::new_v4()),
        format!(
            "before_time=2026-01-01T00:00:00Z&before_id={}&offset=1",
            Uuid::new_v4()
        ),
        format!("search={}", "x".repeat(161)),
    ] {
        assert_eq!(
            get(
                &app,
                &format!("/api/v1/articles?{suffix}"),
                Some(&patient())
            )
            .await
            .status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    assert_eq!(
        get(&app, "/api/v1/articles?search=guide", None)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_d_notification_detail_is_owner_scoped_non_mutating_and_private(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let id = Uuid::new_v4();
    hellodoctor_backend::repository::notification_repo::insert_if_new(
        &pool,
        id,
        uid(PATIENT_USER_ID).into(),
        "BOOKING_CONFIRMED",
        "Synthetic",
        "Synthetic",
        Some("APPOINTMENT"),
        Some(Uuid::new_v4()),
        None,
        &format!("d:{id}"),
        None,
    )
    .await
    .unwrap();
    let path = format!("/api/v1/notifications/{id}");
    let response = get(&app, &path, Some(&patient())).await;
    assert_eq!(response.headers()["cache-control"], "private, no-store");
    let row = data(response).await;
    assert_eq!(row["id"], id.to_string());
    assert!(row["read_at"].is_null());
    assert_eq!(row["delivery_status"], "CREATED");
    for hidden in ["claim_token", "lease_expires_at", "attempt_count"] {
        assert!(row.get(hidden).is_none());
    }
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    assert_eq!(
        get(&app, &path, Some(&doctor)).await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        get(&app, &path, Some(&fresh_patient_token(&pool).await))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        get(&app, &path, None).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert!(sqlx::query_scalar::<_, Option<DateTime<Utc>>>(
        "SELECT read_at FROM notifications WHERE id=$1"
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap()
    .is_none());
}

#[sqlx::test(migrations = "./migrations")]
async fn test_d_notification_destinations_keep_existing_clinical_authorization(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let (id, _) = book_and_confirm(&app, &patient()).await;
    let stranger = fresh_patient_token(&pool).await;
    let other_doctor = make_token(uid(DOCTOR2_USER_ID), "DOCTOR");
    for token in [&stranger, &other_doctor] {
        for suffix in ["", "/conversation"] {
            let status = get(
                &app,
                &format!("/api/v1/appointments/{id}{suffix}"),
                Some(token),
            )
            .await
            .status();
            assert!(matches!(
                status,
                StatusCode::FORBIDDEN | StatusCode::NOT_FOUND
            ));
        }
    }
    for token in [patient(), make_token(uid(DOCTOR1_USER_ID), "DOCTOR")] {
        assert_eq!(
            get(&app, &format!("/api/v1/appointments/{id}"), Some(&token))
                .await
                .status(),
            StatusCode::OK
        );
    }
}
