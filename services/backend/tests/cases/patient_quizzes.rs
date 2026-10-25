use super::admin_quizzes::{
    admin, article, article_step, chosen, data, patient, publish_quiz, question, save, step,
};
use super::*;
use hellodoctor_backend::auth_context::AuthContext;
use serde_json::json;
fn path(a: &Value) -> String {
    format!("/api/v1/articles/{}/quiz", a["id"].as_str().unwrap())
}
fn selection(r: &Value, index: usize, choice: usize) -> Value {
    json!({"revision_id":r["id"],"question_id":r["questions"][index]["id"],"selected_option_id":r["questions"][index]["options"][choice]["id"]})
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
async fn ready(app: &axum::Router) -> (Value, Value) {
    let a = article(app, false, true).await;
    let content = json!({"include_quiz":true,"questions":[question("First public question"),question("Second public question")]});
    let doc = save(app, &a, &Value::Null, &content).await;
    (
        a.clone(),
        publish_quiz(app, &a, doc).await["published"].clone(),
    )
}

#[sqlx::test(migrations = "./migrations")]
async fn test_patient_quiz_read_feedback_order_and_no_answer_persistence(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let (a, r) = ready(&app).await;
    let token = patient();
    let audits: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    let response = get(&app, &path(&a), Some(&token)).await;
    assert_eq!(response.headers()["cache-control"], "private, no-store");
    let quiz = data(response).await;
    assert_eq!(quiz["schema_version"], 1);
    assert_eq!(quiz["question_count"], 2);
    assert_eq!(quiz["questions"][0]["id"], r["questions"][0]["id"]);
    assert_eq!(quiz["questions"][1]["id"], r["questions"][1]["id"]);
    let text = quiz.to_string();
    for hidden in ["correct_option", "explanation", "approved_by", "created_by"] {
        assert!(!text.contains(hidden));
    }
    for (choice, correct) in [(0, true), (1, false)] {
        let response = post(
            &app,
            &format!("{}/check", path(&a)),
            Some(&token),
            &selection(&r, 0, choice),
        )
        .await;
        assert_eq!(response.headers()["cache-control"], "private, no-store");
        let feedback = data(response).await;
        assert_eq!(feedback["correct"], correct);
        assert_eq!(
            feedback["correct_option_id"],
            r["questions"][0]["options"][0]["id"]
        );
        assert_eq!(feedback["explanation_en"], "The first option is correct.");
        assert!(feedback.get("questions").is_none());
        assert!(feedback.get("selected_option_id").is_none());
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM audit_events")
            .fetch_one(&pool)
            .await
            .unwrap(),
        audits
    );
    let feed = data(get(&app, "/api/v1/articles", Some(&token)).await).await;
    assert!(!feed.to_string().contains("correct_option"));
    assert!(!feed.to_string().contains("explanation_en"));
}

#[sqlx::test(migrations = "./migrations")]
async fn test_patient_quiz_missing_off_draft_approved_and_unpublished_visibility(pool: PgPool) {
    let (app, _) = test_app(pool).await;
    let a = article(&app, false, true).await;
    let token = patient();
    assert_eq!(
        get(&app, &path(&a), Some(&token)).await.status(),
        StatusCode::NOT_FOUND
    );
    let doc = save(
        &app,
        &a,
        &Value::Null,
        &json!({"include_quiz":true,"questions":[question("Private until published")]}),
    )
    .await;
    assert_eq!(
        get(&app, &path(&a), Some(&token)).await.status(),
        StatusCode::NOT_FOUND
    );
    let doc = step(&app, &a, &chosen(&doc), &admin(), "submit").await;
    let doc = step(&app, &a, &chosen(&doc), &admin(), "approve").await;
    assert_eq!(
        get(&app, &path(&a), Some(&token)).await.status(),
        StatusCode::NOT_FOUND
    );
    let doc = step(&app, &a, &chosen(&doc), &admin(), "publish").await;
    let r = doc["published"].clone();
    assert_eq!(
        get(&app, &path(&a), Some(&token)).await.status(),
        StatusCode::OK
    );
    let doc = save(
        &app,
        &a,
        &r,
        &json!({"include_quiz":false,"questions":r["questions"]}),
    )
    .await;
    assert_eq!(
        get(&app, &path(&a), Some(&token)).await.status(),
        StatusCode::OK
    );
    let doc = publish_quiz(&app, &a, doc).await;
    assert_eq!(
        get(&app, &path(&a), Some(&token)).await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        post(
            &app,
            &format!("{}/check", path(&a)),
            Some(&token),
            &selection(&doc["published"], 0, 0)
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    let (a, r) = ready(&app).await;
    let a = article_step(&app, &a, &admin(), "unpublish").await;
    assert_eq!(
        get(&app, &path(&a), Some(&token)).await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        post(
            &app,
            &format!("{}/check", path(&a)),
            Some(&token),
            &selection(&r, 0, 0)
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_patient_quiz_superseded_and_article_version_compatibility(pool: PgPool) {
    let (app, _) = test_app(pool).await;
    let (a, r) = ready(&app).await;
    let token = patient();
    let doc = save(
        &app,
        &a,
        &r,
        &json!({"include_quiz":true,"questions":[question("Replacement question")]}),
    )
    .await;
    let current = publish_quiz(&app, &a, doc).await["published"].clone();
    assert_eq!(
        post(
            &app,
            &format!("{}/check", path(&a)),
            Some(&token),
            &selection(&r, 0, 0)
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    let a=data(send(&app,"PATCH",&format!("/api/v1/admin/articles/{}",a["id"].as_str().unwrap()),Some(&admin()),Some(&json!({"version":a["version"],"content":{"title_en":a["title_en"],"body_en":"A revised article body","category_id":a["category_id"],"sensitive":false}})),None).await).await;
    let mut a = article_step(&app, &a, &admin(), "submit").await;
    for action in ["approve", "publish"] {
        a = article_step(&app, &a, &admin(), action).await;
    }
    assert_eq!(
        get(&app, &path(&a), Some(&token)).await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        post(
            &app,
            &format!("{}/check", path(&a)),
            Some(&token),
            &selection(&current, 0, 0)
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_patient_quiz_authorization_and_cross_article_choice_validation(pool: PgPool) {
    let (app, _) = test_app(pool).await;
    let (a, r) = ready(&app).await;
    let (b, other) = ready(&app).await;
    for token in [
        None,
        Some(admin()),
        Some(make_token(uid(DOCTOR1_USER_ID), "DOCTOR")),
    ] {
        let status = if token.is_none() {
            StatusCode::UNAUTHORIZED
        } else {
            StatusCode::FORBIDDEN
        };
        assert_eq!(
            get(&app, &path(&a), token.as_deref()).await.status(),
            status
        );
        assert_eq!(
            post(
                &app,
                &format!("{}/check", path(&a)),
                token.as_deref(),
                &selection(&r, 0, 0)
            )
            .await
            .status(),
            status
        );
    }
    let token = patient();
    let mut invalid = selection(&r, 0, 0);
    invalid["question_id"] = other["questions"][0]["id"].clone();
    assert_eq!(
        post(&app, &format!("{}/check", path(&a)), Some(&token), &invalid)
            .await
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    invalid = selection(&r, 0, 0);
    invalid["selected_option_id"] = r["questions"][1]["options"][0]["id"].clone();
    assert_eq!(
        post(&app, &format!("{}/check", path(&a)), Some(&token), &invalid)
            .await
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        post(
            &app,
            &format!("{}/check", path(&b)),
            Some(&token),
            &selection(&r, 0, 0)
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    invalid = selection(&r, 0, 0);
    invalid["unknown"] = json!(true);
    assert_eq!(
        post(&app, &format!("{}/check", path(&a)), Some(&token), &invalid)
            .await
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    invalid = selection(&r, 0, 0);
    invalid["padding"] = json!("x".repeat(1100));
    assert_eq!(
        post(&app, &format!("{}/check", path(&a)), Some(&token), &invalid)
            .await
            .status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_patient_quiz_bilingual_fields_and_complete_english_fallback(pool: PgPool) {
    let (app, _) = test_app(pool).await;
    let a = article(&app, false, true).await;
    let mut q = question("Translated question");
    q["text_sw"] = json!("Swali");
    q["explanation_sw"] = json!("Maelezo");
    q["options"][0]["text_sw"] = json!("Kwanza");
    q["options"][1]["text_sw"] = json!("Pili");
    let doc = save(
        &app,
        &a,
        &Value::Null,
        &json!({"include_quiz":true,"questions":[q,question("English fallback question")]}),
    )
    .await;
    let r = publish_quiz(&app, &a, doc).await["published"].clone();
    let token = patient();
    let quiz = data(get(&app, &path(&a), Some(&token)).await).await;
    assert_eq!(quiz["questions"][0]["text_sw"], "Swali");
    assert_eq!(quiz["questions"][0]["options"][0]["text_sw"], "Kwanza");
    assert_eq!(quiz["questions"][1]["text_sw"], "");
    assert_eq!(quiz["questions"][1]["options"][0]["text_sw"], "");
    let feedback = data(
        post(
            &app,
            &format!("{}/check", path(&a)),
            Some(&token),
            &selection(&r, 0, 0),
        )
        .await,
    )
    .await;
    assert_eq!(feedback["explanation_sw"], "Maelezo");
    let feedback = data(
        post(
            &app,
            &format!("{}/check", path(&a)),
            Some(&token),
            &selection(&r, 1, 0),
        )
        .await,
    )
    .await;
    assert_eq!(feedback["explanation_sw"], "");
    assert_eq!(feedback["explanation_en"], "The first option is correct.");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_patient_quiz_rls_and_function_execution_boundaries(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let (a, _) = ready(&app).await;
    let ctx = actor(UserRole::Patient, PATIENT_USER_ID);
    let mut tx = state.begin_authed(&ctx).await.unwrap();
    let rows: Vec<Option<Uuid>> =
        sqlx::query_scalar("SELECT correct_option_id FROM article_quiz_questions")
            .fetch_all(&mut *tx)
            .await
            .unwrap();
    assert!(rows.is_empty());
    let quiz: Value = sqlx::query_scalar("SELECT read_patient_article_quiz($1)")
        .bind(uid(a["id"].as_str().unwrap()))
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(quiz["included"], true);
    tx.commit().await.unwrap();
    for role in [UserRole::Admin, UserRole::Doctor] {
        let ctx = actor(
            role,
            if role == UserRole::Admin {
                ADMIN_USER_ID
            } else {
                DOCTOR1_USER_ID
            },
        );
        let mut tx = state.begin_authed(&ctx).await.unwrap();
        let denied = sqlx::query("SELECT read_patient_article_quiz($1)")
            .bind(uid(a["id"].as_str().unwrap()))
            .execute(&mut *tx)
            .await
            .unwrap_err();
        assert_eq!(
            denied.as_database_error().and_then(|e| e.code()).as_deref(),
            Some("42501")
        );
        tx.rollback().await.unwrap();
    }
    let allowed:bool=sqlx::query_scalar("SELECT has_function_privilege('hellodoctor_api','read_patient_article_quiz(uuid)','EXECUTE') AND NOT has_function_privilege('hellodoctor_api','lock_patient_article_quiz(uuid)','EXECUTE') AND NOT has_function_privilege('hellodoctor_auth_worker','read_patient_article_quiz(uuid)','EXECUTE')").fetch_one(&pool).await.unwrap();
    assert!(allowed);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_patient_quiz_publication_changes_serialize_with_answer_checks(pool: PgPool) {
    let (app, _, state) = test_app_and_state(pool).await;
    let (a, r) = ready(&app).await;
    let doc = save(
        &app,
        &a,
        &r,
        &json!({"include_quiz":true,"questions":[question("New revision")]}),
    )
    .await;
    let doc = step(&app, &a, &chosen(&doc), &admin(), "submit").await;
    let pending = chosen(&step(&app, &a, &chosen(&doc), &admin(), "approve").await);
    let mut tx = state
        .begin_authed(&actor(UserRole::Patient, PATIENT_USER_ID))
        .await
        .unwrap();
    let checked: Value = sqlx::query_scalar("SELECT check_patient_article_quiz($1,$2,$3,$4)")
        .bind(uid(a["id"].as_str().unwrap()))
        .bind(uid(r["id"].as_str().unwrap()))
        .bind(uid(r["questions"][0]["id"].as_str().unwrap()))
        .bind(uid(r["questions"][0]["options"][0]["id"].as_str().unwrap()))
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(checked["correct"], true);
    let publish_app = app.clone();
    let publish_a = a.clone();
    let publish_r = pending.clone();
    let task = tokio::spawn(async move {
        step(&publish_app, &publish_a, &publish_r, &admin(), "publish").await
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert!(!task.is_finished());
    tx.commit().await.unwrap();
    task.await.unwrap();
    assert_eq!(
        post(
            &app,
            &format!("{}/check", path(&a)),
            Some(&patient()),
            &selection(&r, 0, 0)
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_patient_quiz_rate_limits_are_separate_and_do_not_store_answers(pool: PgPool) {
    let (app, _, state) = test_app_and_state(pool).await;
    let (a, r) = ready(&app).await;
    let mut redis = state.redis.get().await.unwrap();
    let _: () = deadpool_redis::redis::cmd("SET")
        .arg(format!("ratelimit:quiz-check:{}", PATIENT_USER_ID))
        .arg(120)
        .arg("EX")
        .arg(60)
        .query_async(&mut redis)
        .await
        .unwrap();
    assert_eq!(
        post(
            &app,
            &format!("{}/check", path(&a)),
            Some(&patient()),
            &selection(&r, 0, 0)
        )
        .await
        .status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(
        get(&app, &path(&a), Some(&patient())).await.status(),
        StatusCode::OK
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_patient_quiz_migration_rollback_preserves_content_and_rls(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let (a, r) = ready(&app).await;
    let policies:Value=sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(p) ORDER BY policyname) FROM pg_policies p WHERE tablename='doctor_articles'").fetch_one(&pool).await.unwrap();
    hellodoctor_backend::migration_status::MIGRATOR
        .undo(&pool, 20261128000001)
        .await
        .unwrap();
    hellodoctor_backend::migration_status::MIGRATOR
        .run(&pool)
        .await
        .unwrap();
    assert_eq!(
        data(get(&app, &path(&a), Some(&patient())).await).await["revision_id"],
        r["id"]
    );
    let after:Value=sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(p) ORDER BY policyname) FROM pg_policies p WHERE tablename='doctor_articles'").fetch_one(&pool).await.unwrap();
    assert_eq!(policies, after);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_patient_quiz_sensitive_article_and_quiz_independent_acceptance(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let mut a = article(&app, true, false).await;
    let reviewer_id = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,phone_number,role,is_active,is_verified) VALUES($1,$2,'ADMIN',true,true)").bind(reviewer_id).bind(format!("+254{}",rand::thread_rng().gen_range(100000000..999999999))).execute(&pool).await.unwrap();
    fixture_session(&pool, reviewer_id).await;
    let reviewer = make_token(reviewer_id, "ADMIN");
    a = article_step(&app, &a, &admin(), "submit").await;
    a = article_step(&app, &a, &reviewer, "approve").await;
    a = article_step(&app, &a, &admin(), "publish").await;
    let doc = save(
        &app,
        &a,
        &Value::Null,
        &json!({"include_quiz":true,"questions":[question("Approved educational question")]}),
    )
    .await;
    let doc = step(&app, &a, &chosen(&doc), &admin(), "submit").await;
    let doc = step(&app, &a, &chosen(&doc), &reviewer, "approve").await;
    let r = step(&app, &a, &chosen(&doc), &admin(), "publish").await["published"].clone();
    let token = patient();
    let detail = data(
        get(
            &app,
            &format!("/api/v1/articles/{}", a["id"].as_str().unwrap()),
            Some(&token),
        )
        .await,
    )
    .await;
    assert_eq!(detail["quiz"]["included"], true);
    assert_eq!(
        data(get(&app, &path(&a), Some(&token)).await).await["question_count"],
        1
    );
    assert_eq!(
        data(
            post(
                &app,
                &format!("{}/check", path(&a)),
                Some(&token),
                &selection(&r, 0, 0)
            )
            .await
        )
        .await["correct"],
        true
    );
}
