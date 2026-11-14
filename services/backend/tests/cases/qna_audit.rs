//! Q&A answer audit (docs/admin-ui/ADMIN_REDESIGN_PLAN.md): Admins approve or
//! flag doctors' answers with a note; a flagged answer disappears from the
//! patient feed until approved again; the patient author is never exposed.
use super::*;
use serde_json::json;

async fn answered_question(app: &axum::Router, pool: &PgPool, text: &str) -> (Uuid, Uuid) {
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let question = Uuid::new_v4();
    sqlx::query("INSERT INTO health_questions (id, author_patient_id, question_text, category, status, created_at, updated_at) VALUES ($1,$2,$3,'General','OPEN',$4,$4)")
        .bind(question)
        .bind(uid(PATIENT_PROFILE_ID))
        .bind(text)
        .bind(Utc::now())
        .execute(pool)
        .await
        .unwrap();
    let res = post(
        app,
        &format!("/api/v1/doctor/qna/questions/{question}/answers"),
        Some(&doctor),
        &json!({"answer_text": "Drink safe fluids and seek care for danger signs."}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let answer: Uuid = sqlx::query_scalar("SELECT id FROM health_answers WHERE question_id=$1")
        .bind(question)
        .fetch_one(pool)
        .await
        .unwrap();
    (question, answer)
}

async fn feed_has(app: &axum::Router, patient: &str, question: Uuid) -> bool {
    let feed = json_body(get(app, "/api/v1/patients/me/qna/questions", Some(patient)).await).await;
    feed["data"]
        .as_array()
        .unwrap()
        .iter()
        .any(|q| q["id"] == json!(question.to_string()))
}

#[sqlx::test(migrations = "./migrations")]
async fn test_flagged_answer_is_hidden_from_patients_until_approved(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let (question, answer) = answered_question(&app, &pool, "What helps during a fever?").await;
    assert!(feed_has(&app, &patient, question).await);

    let list = json_body(
        get(
            &app,
            "/api/v1/admin/qna/answers?status=UNREVIEWED",
            Some(&admin),
        )
        .await,
    )
    .await;
    let item = list["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["answer_id"] == json!(answer.to_string()))
        .cloned()
        .unwrap();
    assert_eq!(item["audit_status"], "UNREVIEWED");
    assert_eq!(item["question_text"], "What helps during a fever?");
    assert!(
        item.get("author_patient_id").is_none(),
        "the patient author is never exposed"
    );
    assert!(!list.to_string().contains(&PATIENT_PROFILE_ID.to_string()));
    assert_eq!(list["data"]["counts"]["UNREVIEWED"], 1);

    let reviews = format!("/api/v1/admin/qna/answers/{answer}/reviews");
    let res = post(&app, &reviews, Some(&admin), &json!({"decision": "FLAGGED", "note": "Advice is too general; ask the doctor to add danger signs."})).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res).await["data"]["hidden_from_patients"], true);
    assert!(
        !feed_has(&app, &patient, question).await,
        "a flagged answer is hidden from patients"
    );

    let res = post(
        &app,
        &reviews,
        Some(&admin),
        &json!({"decision": "APPROVED", "note": "Reviewed again; acceptable."}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert!(
        feed_has(&app, &patient, question).await,
        "approved again: visible"
    );

    let history = json_body(get(&app, &reviews, Some(&admin)).await).await;
    let history = history["data"].as_array().unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0]["decision"], "APPROVED");
    assert_eq!(history[1]["decision"], "FLAGGED");

    let flagged = json_body(
        get(
            &app,
            "/api/v1/admin/qna/answers?status=FLAGGED",
            Some(&admin),
        )
        .await,
    )
    .await;
    assert_eq!(flagged["data"]["total"], 0);
    let approved = json_body(
        get(
            &app,
            "/api/v1/admin/qna/answers?status=APPROVED",
            Some(&admin),
        )
        .await,
    )
    .await;
    assert_eq!(approved["data"]["total"], 1);
    assert_eq!(
        approved["data"]["items"][0]["last_review"]["decision"],
        "APPROVED"
    );

    let audit: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_events WHERE action IN ('QNA_ANSWER_FLAGGED','QNA_ANSWER_APPROVED')")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(audit, 2);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_qna_review_is_admin_only_and_validated(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let (_, answer) = answered_question(&app, &pool, "Is it safe to exercise with a cold?").await;
    let reviews = format!("/api/v1/admin/qna/answers/{answer}/reviews");
    let body = json!({"decision": "FLAGGED", "note": "Needs more detail."});

    for token in [&doctor, &patient] {
        assert_eq!(
            get(&app, "/api/v1/admin/qna/answers", Some(token))
                .await
                .status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            post(&app, &reviews, Some(token), &body).await.status(),
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(
        post(
            &app,
            &reviews,
            Some(&admin),
            &json!({"decision": "MAYBE", "note": "x"})
        )
        .await
        .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        post(
            &app,
            &reviews,
            Some(&admin),
            &json!({"decision": "APPROVED", "note": "   "})
        )
        .await
        .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        get(&app, "/api/v1/admin/qna/answers?status=WRONG", Some(&admin))
            .await
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let missing = format!("/api/v1/admin/qna/answers/{}/reviews", Uuid::new_v4());
    assert_eq!(
        post(&app, &missing, Some(&admin), &body).await.status(),
        StatusCode::NOT_FOUND
    );

    // The Admin review cannot rewrite the doctor's answer text.
    let mut conn = pool.acquire().await.unwrap();
    sqlx::query("SELECT set_config('app.current_role', 'ADMIN', false)")
        .execute(&mut *conn)
        .await
        .unwrap();
    let rewrite =
        sqlx::query("UPDATE health_answers SET answer_text='changed by admin' WHERE id=$1")
            .bind(answer)
            .execute(&mut *conn)
            .await;
    assert!(
        rewrite.is_err(),
        "the guard trigger must reject answer edits under the ADMIN role"
    );
}
