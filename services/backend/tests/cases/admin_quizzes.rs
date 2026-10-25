use super::*;
use hellodoctor_backend::auth_context::AuthContext;
use serde_json::json;

pub(super) fn admin() -> String {
    make_token(uid(ADMIN_USER_ID), "ADMIN")
}
pub(super) fn patient() -> String {
    make_token(uid(PATIENT_USER_ID), "PATIENT")
}
pub(super) fn path(a: &Value) -> String {
    format!("/api/v1/admin/articles/{}/quiz", a["id"].as_str().unwrap())
}
pub(super) fn chosen(doc: &Value) -> Value {
    if doc["draft"].is_null() {
        doc["published"].clone()
    } else {
        doc["draft"].clone()
    }
}
pub(super) async fn data(response: axum::http::Response<Body>) -> Value {
    let status = response.status();
    let body = json_body(response).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["data"].clone()
}
pub(super) async fn article_step(
    app: &axum::Router,
    a: &Value,
    token: &str,
    action: &str,
) -> Value {
    data(
        post(
            app,
            &format!(
                "/api/v1/admin/articles/{}/{action}",
                a["id"].as_str().unwrap()
            ),
            Some(token),
            &json!({"version":a["version"]}),
        )
        .await,
    )
    .await
}
pub(super) async fn article(app: &axum::Router, sensitive: bool, published: bool) -> Value {
    let token = admin();
    let name = Uuid::new_v4().to_string();
    let category = data(
        post(
            app,
            "/api/v1/admin/article-categories",
            Some(&token),
            &json!({"name_en":name,"name_sw":name,"sensitive":sensitive}),
        )
        .await,
    )
    .await;
    let mut a=data(post(app,"/api/v1/admin/articles",Some(&token),&json!({"id":Uuid::new_v4(),"content":{"title_en":"Platform guide","body_en":"Article remains unchanged.","category_id":category["id"],"sensitive":false}})).await).await;
    if published {
        for action in ["submit", "approve", "publish"] {
            a = article_step(app, &a, &token, action).await;
        }
    }
    a
}
pub(super) fn question(text: &str) -> Value {
    let a = Uuid::new_v4();
    json!({"id":Uuid::new_v4(),"text_en":text,"text_sw":"","explanation_en":"The first option is correct.","explanation_sw":"","options":[{"id":a,"text_en":"First","text_sw":""},{"id":Uuid::new_v4(),"text_en":"Second","text_sw":""}],"correct_option_id":a})
}
pub(super) async fn save(
    app: &axum::Router,
    a: &Value,
    revision: &Value,
    content: &Value,
) -> Value {
    data(send(app,"PATCH",&path(a),Some(&admin()),Some(&json!({"revision_id":revision["id"],"version":revision["version"],"content":content})),None).await).await
}
pub(super) async fn step(
    app: &axum::Router,
    a: &Value,
    r: &Value,
    token: &str,
    action: &str,
) -> Value {
    data(
        post(
            app,
            &format!("{}/{}/{action}", path(a), r["id"].as_str().unwrap()),
            Some(token),
            &json!({"version":r["version"]}),
        )
        .await,
    )
    .await
}
pub(super) async fn publish_quiz(app: &axum::Router, a: &Value, mut doc: Value) -> Value {
    for action in ["submit", "approve", "publish"] {
        doc = step(app, a, &chosen(&doc), &admin(), action).await;
    }
    doc
}

#[sqlx::test(migrations = "./migrations")]
async fn test_mcq_optional_drafts_do_not_change_published_articles_or_leak_keys(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let a = article(&app, false, true).await;
    let public_path = format!("/api/v1/articles/{}", a["id"].as_str().unwrap());
    let before = data(get(&app, &public_path, Some(&patient())).await).await;
    assert_eq!(before["quiz"]["included"], false);
    let q = json!({"id":Uuid::new_v4(),"text_en":"Unfinished private question","options":[],"correct_option_id":null});
    let doc = save(
        &app,
        &a,
        &Value::Null,
        &json!({"include_quiz":false,"questions":[q]}),
    )
    .await;
    let after = data(get(&app, &public_path, Some(&patient())).await).await;
    assert_eq!(before, after);
    assert!(!after.to_string().contains("Unfinished"));
    assert_eq!(
        get(&app, &path(&a), Some(&patient())).await.status(),
        StatusCode::FORBIDDEN
    );
    let ctx = AuthContext {
        user_id: uid(PATIENT_USER_ID).into(),
        role: UserRole::Patient,
        permissions: vec![],
        session_id: None,
        request_id: None,
    };
    let mut tx = state.begin_authed(&ctx).await.unwrap();
    for table in [
        "article_quiz_revisions",
        "article_quiz_questions",
        "article_quiz_options",
    ] {
        let count: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {table}"))
            .fetch_one(&mut *tx)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }
    tx.commit().await.unwrap();
    let mut tx = state.begin_authed(&ctx).await.unwrap();
    let denied=sqlx::query("INSERT INTO article_quiz_questions(revision_id,id,position,text_en) VALUES($1,$2,0,'forbidden')")
        .bind(uid(doc["draft"]["id"].as_str().unwrap())).bind(Uuid::new_v4()).execute(&mut *tx).await.unwrap_err();
    assert_eq!(
        denied.as_database_error().and_then(|e| e.code()).as_deref(),
        Some("42501")
    );
    tx.rollback().await.unwrap();
    let doc = publish_quiz(&app, &a, doc).await;
    assert_eq!(doc["published"]["questions"].as_array().unwrap().len(), 1);
    let feed = data(get(&app, "/api/v1/articles", Some(&patient())).await).await;
    assert!(!feed.to_string().contains("correct_option"));
    assert!(!feed.to_string().contains("explanation_en"));
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM article_quiz_publications WHERE NOT included AND question_count=0").fetch_one(&pool).await.unwrap(),1);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_mcq_manual_edit_delete_reorder_translation_and_invalid_references(pool: PgPool) {
    let (app, _) = test_app(pool).await;
    let a = article(&app, false, false).await;
    let mut first = question("First question");
    first["text_sw"] = json!("Swali la kwanza");
    first["explanation_sw"] = json!("Maelezo");
    first["options"][0]["text_sw"] = json!("Kwanza");
    first["options"][1]["text_sw"] = json!("Pili");
    let second = question("Second question");
    let content = json!({"include_quiz":true,"questions":[first,second]});
    let doc = save(&app, &a, &Value::Null, &content).await;
    let r = chosen(&doc);
    let mut changed = content.clone();
    changed["questions"].as_array_mut().unwrap().swap(0, 1);
    let doc = save(&app, &a, &r, &changed).await;
    let r = chosen(&doc);
    assert_eq!(r["questions"][0]["text_en"], "Second question");
    assert_eq!(r["questions"][1]["text_sw"], "Swali la kwanza");
    let mut invalid = changed.clone();
    invalid["questions"][0]["correct_option_id"] =
        changed["questions"][1]["options"][0]["id"].clone();
    assert_eq!(
        send(
            &app,
            "PATCH",
            &path(&a),
            Some(&admin()),
            Some(&json!({"revision_id":r["id"],"version":r["version"],"content":invalid})),
            None
        )
        .await
        .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    invalid = changed.clone();
    invalid["questions"][1]["options"][0]["id"] =
        invalid["questions"][0]["options"][0]["id"].clone();
    assert_eq!(
        send(
            &app,
            "PATCH",
            &path(&a),
            Some(&admin()),
            Some(&json!({"revision_id":r["id"],"version":r["version"],"content":invalid})),
            None
        )
        .await
        .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    changed["questions"].as_array_mut().unwrap().remove(0);
    let doc = save(&app, &a, &r, &changed).await;
    assert_eq!(chosen(&doc)["questions"].as_array().unwrap().len(), 1);
    let pending = step(&app, &a, &chosen(&doc), &admin(), "submit").await;
    let doc = save(&app, &a, &chosen(&pending), &changed).await;
    assert_eq!(chosen(&doc)["status"], "DRAFT");
    assert!(chosen(&doc)["approved_version"].is_null());
}

#[sqlx::test(migrations = "./migrations")]
async fn test_mcq_publication_snapshot_survives_drafts_and_reviewed_exclusion(pool: PgPool) {
    let (app, _) = test_app(pool).await;
    let a = article(&app, false, true).await;
    let content = json!({"include_quiz":true,"questions":[question("Approved question")]});
    let doc = save(&app, &a, &Value::Null, &content).await;
    let doc = publish_quiz(&app, &a, doc).await;
    let published = doc["published"].clone();
    let doc = save(
        &app,
        &a,
        &published,
        &json!({"include_quiz":false,"questions":content["questions"]}),
    )
    .await;
    assert_ne!(doc["draft"]["id"], published["id"]);
    assert_eq!(doc["published"], published);
    let public_path = format!("/api/v1/articles/{}", a["id"].as_str().unwrap());
    let live = data(get(&app, &public_path, Some(&patient())).await).await;
    assert_eq!(live["quiz"]["included"], true);
    assert_eq!(live["quiz"]["revision_id"], published["id"]);
    assert_eq!(live["version"], a["version"]);
    let doc = publish_quiz(&app, &a, doc).await;
    let live = data(get(&app, &public_path, Some(&patient())).await).await;
    assert_eq!(live["quiz"]["included"], false);
    assert_eq!(live["quiz"]["question_count"], 0);
    assert_eq!(doc["published"]["questions"], content["questions"]);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_mcq_sensitive_review_and_article_version_binding(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let a = article(&app, true, false).await;
    let reviewer_id = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,phone_number,role,is_active,is_verified) VALUES($1,$2,'ADMIN',true,true)").bind(reviewer_id).bind(format!("+254{}",rand::thread_rng().gen_range(100000000..999999999))).execute(&pool).await.unwrap();
    fixture_session(&pool, reviewer_id).await;
    let reviewer = make_token(reviewer_id, "ADMIN");
    let doc = save(
        &app,
        &a,
        &Value::Null,
        &json!({"include_quiz":true,"questions":[question("Sensitive learning question")]}),
    )
    .await;
    let doc = step(&app, &a, &chosen(&doc), &admin(), "submit").await;
    let r = chosen(&doc);
    assert_eq!(r["can_approve"], false);
    assert_eq!(
        post(
            &app,
            &format!("{}/{}/approve", path(&a), r["id"].as_str().unwrap()),
            Some(&admin()),
            &json!({"version":r["version"]})
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    let doc = step(&app, &a, &r, &reviewer, "approve").await;
    let r = chosen(&doc);
    assert_eq!(r["approved_version"], r["content_version"]);
    let mut article_content = json!({"title_en":a["title_en"],"body_en":"Updated body","category_id":a["category_id"],"sensitive":false});
    let edited = data(
        send(
            &app,
            "PATCH",
            &format!("/api/v1/admin/articles/{}", a["id"].as_str().unwrap()),
            Some(&admin()),
            Some(&json!({"version":a["version"],"content":article_content})),
            None,
        )
        .await,
    )
    .await;
    assert_eq!(
        post(
            &app,
            &format!("{}/{}/publish", path(&a), r["id"].as_str().unwrap()),
            Some(&admin()),
            &json!({"version":r["version"]})
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    article_content["body_en"] = json!("Updated body");
    assert!(edited["content_version"].as_i64().unwrap() > a["content_version"].as_i64().unwrap());
}

#[sqlx::test(migrations = "./migrations")]
async fn test_mcq_authorization_cross_article_and_concurrent_updates(pool: PgPool) {
    let (app, _) = test_app(pool).await;
    let a = article(&app, false, true).await;
    let b = article(&app, false, false).await;
    let content = json!({"include_quiz":true,"questions":[question("Concurrent question")]});
    let doc = save(&app, &a, &Value::Null, &content).await;
    let r = chosen(&doc);
    for token in [
        None,
        Some(patient()),
        Some(make_token(uid(DOCTOR1_USER_ID), "DOCTOR")),
    ] {
        let expected = if token.is_none() {
            StatusCode::UNAUTHORIZED
        } else {
            StatusCode::FORBIDDEN
        };
        assert_eq!(
            get(&app, &path(&a), token.as_deref()).await.status(),
            expected
        );
        assert_eq!(
            send(
                &app,
                "PATCH",
                &path(&a),
                token.as_deref(),
                Some(&json!({"content":content})),
                None
            )
            .await
            .status(),
            expected
        );
    }
    assert_eq!(
        post(
            &app,
            &format!("{}/{}/submit", path(&b), r["id"].as_str().unwrap()),
            Some(&admin()),
            &json!({"version":r["version"]})
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    let body = json!({"revision_id":r["id"],"version":r["version"],"content":content});
    let url = path(&a);
    let token = admin();
    let (first, second) = tokio::join!(
        send(&app, "PATCH", &url, Some(&token), Some(&body), None),
        send(&app, "PATCH", &url, Some(&token), Some(&body), None)
    );
    let mut statuses = vec![first.status().as_u16(), second.status().as_u16()];
    statuses.sort();
    assert_eq!(statuses, vec![200, 409]);
    let doc = data(get(&app, &url, Some(&token)).await).await;
    let doc = step(&app, &a, &chosen(&doc), &token, "submit").await;
    let doc = step(&app, &a, &chosen(&doc), &token, "approve").await;
    let r = chosen(&doc);
    let url = format!("{}/{}/publish", url, r["id"].as_str().unwrap());
    let body = json!({"version":r["version"]});
    let (first, second) = tokio::join!(
        post(&app, &url, Some(&token), &body),
        post(&app, &url, Some(&token), &body)
    );
    let mut statuses = vec![first.status().as_u16(), second.status().as_u16()];
    statuses.sort();
    assert_eq!(statuses, vec![200, 409]);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_mcq_bulk_import_preview_validation_duplicates_and_atomic_confirmation(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let a = article(&app, false, true).await;
    let token = admin();
    let csv="text_en,option_a_en,option_b_en,correct_option,explanation_en\r\n\"A quoted, question?\",Yes,No,a,Explanation\r\nSecond question?,Yes,No,b,Explanation";
    let body = json!({"format":"csv","content":csv,"revision_id":null,"version":null});
    let preview = data(
        post(
            &app,
            &format!("{}/import-preview", path(&a)),
            Some(&token),
            &body,
        )
        .await,
    )
    .await;
    assert!(preview["errors"].as_array().unwrap().is_empty());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM article_quiz_revisions")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    let doc = data(post(&app, &format!("{}/import", path(&a)), Some(&token), &body).await).await;
    let r = chosen(&doc);
    assert_eq!(r["include_quiz"], false);
    assert_eq!(r["questions"].as_array().unwrap().len(), 2);
    let body = json!({"format":"csv","content":csv,"revision_id":r["id"],"version":r["version"]});
    let preview = data(
        post(
            &app,
            &format!("{}/import-preview", path(&a)),
            Some(&token),
            &body,
        )
        .await,
    )
    .await;
    assert!(preview["errors"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["row"] == 2));
    assert_eq!(
        post(&app, &format!("{}/import", path(&a)), Some(&token), &body)
            .await
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        chosen(&data(get(&app, &path(&a), Some(&token)).await).await),
        r
    );
    for input in [
        json!({"schema_version":1,"questions":[{"text_en":"Bad","options":[],"correct_option":9,"explanation_en":"x"}]}),
        json!({"schema_version":1,"questions":[],"unknown":true}),
    ] {
        let preview = data(
            post(
                &app,
                &format!("{}/import-preview", path(&a)),
                Some(&token),
                &json!({"format":"json","content":input.to_string()}),
            )
            .await,
        )
        .await;
        assert!(!preview["errors"].as_array().unwrap().is_empty());
    }
    let oversized = "x".repeat(128 * 1024 + 1);
    let preview = data(
        post(
            &app,
            &format!("{}/import-preview", path(&a)),
            Some(&token),
            &json!({"format":"csv","content":oversized}),
        )
        .await,
    )
    .await;
    assert!(!preview["errors"].as_array().unwrap().is_empty());
}

#[sqlx::test(migrations = "./migrations")]
async fn test_mcq_database_guards_immutable_answers_order_and_audit_rollback(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let a = article(&app, false, true).await;
    let content = json!({"include_quiz":true,"questions":[question("Immutable answer") ]});
    let doc = save(&app, &a, &Value::Null, &content).await;
    let doc = publish_quiz(&app, &a, doc).await;
    let r = doc["published"].clone();
    let ctx = AuthContext {
        user_id: uid(ADMIN_USER_ID).into(),
        role: UserRole::Admin,
        permissions: vec![],
        session_id: None,
        request_id: None,
    };
    let mut protected = state.begin_authed(&ctx).await.unwrap();
    let immutable =
        sqlx::query("UPDATE article_quiz_questions SET text_en='Unreviewed' WHERE revision_id=$1")
            .bind(uid(r["id"].as_str().unwrap()))
            .execute(&mut *protected)
            .await
            .unwrap_err();
    assert_eq!(
        immutable
            .as_database_error()
            .and_then(|e| e.code())
            .as_deref(),
        Some("23514")
    );
    protected.rollback().await.unwrap();
    let doc = save(&app, &a, &r, &content).await;
    let r = chosen(&doc);
    let mut tx = state.begin_authed(&ctx).await.unwrap();
    sqlx::query("UPDATE article_quiz_questions SET position=4 WHERE revision_id=$1")
        .bind(uid(r["id"].as_str().unwrap()))
        .execute(&mut *tx)
        .await
        .unwrap();
    assert!(tx.commit().await.is_err());
    let force:i64=sqlx::query_scalar("SELECT count(*) FROM pg_class WHERE relname IN ('article_quiz_revisions','article_quiz_questions','article_quiz_options','article_quiz_publications') AND relforcerowsecurity").fetch_one(&pool).await.unwrap();
    assert_eq!(force, 4);
    sqlx::raw_sql("CREATE FUNCTION deny_quiz_audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.resource_type='ARTICLE_QUIZ' THEN RAISE EXCEPTION 'isolated quiz audit failure'; END IF; RETURN NEW; END $$; CREATE TRIGGER deny_quiz_audit BEFORE INSERT ON audit_events FOR EACH ROW EXECUTE FUNCTION deny_quiz_audit();").execute(&pool).await.unwrap();
    let before = chosen(&data(get(&app, &path(&a), Some(&admin())).await).await);
    assert_eq!(
        send(
            &app,
            "PATCH",
            &path(&a),
            Some(&admin()),
            Some(&json!({"revision_id":r["id"],"version":r["version"],"content":content})),
            None
        )
        .await
        .status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(
        chosen(&data(get(&app, &path(&a), Some(&admin())).await).await),
        before
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_mcq_migration_down_up_preserves_articles_and_doctor_rls(pool: PgPool) {
    let (_, pool) = test_app(pool).await;
    let policies:Value=sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(p) ORDER BY policyname) FROM pg_policies p WHERE tablename='doctor_articles'").fetch_one(&pool).await.unwrap();
    hellodoctor_backend::migration_status::MIGRATOR
        .undo(&pool, 20261127000001)
        .await
        .unwrap();
    assert!(sqlx::query_scalar::<_, Option<String>>(
        "SELECT to_regclass('article_quiz_revisions')::text"
    )
    .fetch_one(&pool)
    .await
    .unwrap()
    .is_none());
    hellodoctor_backend::migration_status::MIGRATOR
        .run(&pool)
        .await
        .unwrap();
    let after:Value=sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(p) ORDER BY policyname) FROM pg_policies p WHERE tablename='doctor_articles'").fetch_one(&pool).await.unwrap();
    assert_eq!(policies, after);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_mcq_json_import_caps_and_review_validation(pool: PgPool) {
    let (app, _) = test_app(pool).await;
    let a = article(&app, false, true).await;
    let token = admin();
    let json = json!({"schema_version":1,"questions":[{"text_en":"JSON question","text_sw":"Swali","explanation_en":"Explanation","explanation_sw":"Maelezo","options":[{"text_en":"Yes","text_sw":"Ndiyo"},{"text_en":"No","text_sw":"Hapana"}],"correct_option":1}]});
    let doc = data(
        post(
            &app,
            &format!("{}/import", path(&a)),
            Some(&token),
            &json!({"format":"json","content":json.to_string(),"revision_id":null,"version":null}),
        )
        .await,
    )
    .await;
    let r = chosen(&doc);
    assert_eq!(
        r["questions"][0]["correct_option_id"],
        r["questions"][0]["options"][1]["id"]
    );
    assert_eq!(r["questions"][0]["text_sw"], "Swali");
    let excessive: Vec<_> = (0..51)
        .map(|i| question(&format!("Question {i}")))
        .collect();
    assert_eq!(send(&app,"PATCH",&path(&a),Some(&token),Some(&json!({"revision_id":r["id"],"version":r["version"],"content":{"include_quiz":false,"questions":excessive}})),None).await.status(),StatusCode::UNPROCESSABLE_ENTITY);
    let incomplete = json!({"include_quiz":true,"questions":[{"id":Uuid::new_v4(),"text_en":"","options":[],"correct_option_id":null}]});
    let doc = save(&app, &a, &r, &incomplete).await;
    let r = chosen(&doc);
    assert_eq!(
        post(
            &app,
            &format!("{}/{}/submit", path(&a), r["id"].as_str().unwrap()),
            Some(&token),
            &json!({"version":r["version"]})
        )
        .await
        .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        post(
            &app,
            &format!("{}/import-preview", path(&a)),
            Some(&token),
            &json!({"format":"json","content":"x".repeat(1024*1024)})
        )
        .await
        .status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
    let live = data(
        get(
            &app,
            &format!("/api/v1/articles/{}", a["id"].as_str().unwrap()),
            Some(&patient()),
        )
        .await,
    )
    .await;
    assert_eq!(live["quiz"]["included"], false);
}
