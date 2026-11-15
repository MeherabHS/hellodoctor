//! Doctor Web redesign (2026-11-14): Doctor article topic/Swahili/cover and
//! professional verification resubmission.
use super::*;
use serde_json::json;

fn content_with(extras: Option<Value>) -> Value {
    let mut c = json!({"title":"Understanding PrEP","summary":"How PrEP protects you","body":"PrEP is a daily pill.","references_list":[]});
    if let Some(e) = extras {
        c["extras"] = e;
    }
    c
}
async fn create(
    app: &axum::Router,
    doctor: &str,
    extras: Option<Value>,
) -> axum::http::Response<Body> {
    post(
        app,
        "/api/v1/doctor/articles",
        Some(doctor),
        &json!({"creation_key":Uuid::new_v4().to_string(),"content":content_with(extras)}),
    )
    .await
}

#[sqlx::test(migrations = "./migrations")]
async fn test_doctor_article_topic_and_swahili_round_trip_and_feed(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let sw = json!({"title":"Kuelewa PrEP","summary":"Jinsi PrEP inavyokulinda","body":"PrEP ni kidonge cha kila siku."});
    let res = create(
        &app,
        &doctor,
        Some(json!({"topic_key":"PREP","swahili":sw})),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let a = json_body(res).await["data"].clone();
    let id = a["id"].as_str().unwrap().to_owned();
    assert_eq!(a["topic_key"], "PREP");
    assert_eq!(a["title_sw"], "Kuelewa PrEP");
    assert!(a["cover_image_id"].is_null());

    // An older client that omits `extras` keeps the stored topic and Swahili version.
    let res = send(
        &app,
        "PATCH",
        &format!("/api/v1/doctor/articles/{id}"),
        Some(&doctor),
        Some(&json!({"version":a["version"],"content":content_with(None)})),
        None,
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let a = json_body(res).await["data"].clone();
    assert_eq!(a["topic_key"], "PREP");
    assert_eq!(a["summary_sw"], "Jinsi PrEP inavyokulinda");

    // Sending `extras` replaces both; null clears them.
    let res = send(&app, "PATCH", &format!("/api/v1/doctor/articles/{id}"), Some(&doctor), Some(&json!({"version":a["version"],"content":content_with(Some(json!({"topic_key":null,"swahili":null})))})), None).await;
    let cleared = json_body(res).await["data"].clone();
    assert!(cleared["topic_key"].is_null() && cleared["title_sw"].is_null());
    let res = send(&app, "PATCH", &format!("/api/v1/doctor/articles/{id}"), Some(&doctor), Some(&json!({"version":cleared["version"],"content":content_with(Some(json!({"topic_key":"PREP","swahili":sw})))})), None).await;
    let a = json_body(res).await["data"].clone();

    // Published: the patient feed carries the topic and Swahili fields; topic filter finds it.
    let res = post(
        &app,
        &format!("/api/v1/doctor/articles/{id}/publish"),
        Some(&doctor),
        &json!({"version":a["version"]}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let feed =
        json_body(get(&app, "/api/v1/articles?topic=PREP&limit=50", Some(&patient)).await).await;
    let row = feed["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == id.as_str())
        .expect("doctor article under its topic");
    assert_eq!(row["article_type"], "DOCTOR");
    assert_eq!(row["title_sw"], "Kuelewa PrEP");
    assert!(row["cover_url"].is_null());
    // Swahili search matches the Swahili text of a Doctor article.
    let feed = json_body(
        get(
            &app,
            "/api/v1/articles?language=sw&search=kidonge&limit=50",
            Some(&patient),
        )
        .await,
    )
    .await;
    assert!(feed["data"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["id"] == id.as_str()));
}

#[sqlx::test(migrations = "./migrations")]
async fn test_doctor_article_extras_validation(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    for extras in [
        json!({"topic_key":"NOT_A_TOPIC"}),
        json!({"swahili":{"title":"Kichwa","summary":"  ","body":"Makala"}}),
        json!({"topic_key":"PREP","unexpected":true}),
    ] {
        let status = create(&app, &doctor, Some(extras.clone())).await.status();
        assert!(
            status == StatusCode::UNPROCESSABLE_ENTITY || status == StatusCode::BAD_REQUEST,
            "{extras} gave {status}"
        );
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn test_doctor_article_cover_owner_only_and_public_after_publish(pool: PgPool) {
    let (app, _pool, _state) = test_app_and_state(pool).await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let other = make_token(uid(DOCTOR2_USER_ID), "DOCTOR");
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let a = json_body(create(&app, &doctor, None).await).await["data"].clone();
    let id = a["id"].as_str().unwrap().to_owned();
    let upload = |token: String, version: Value| {
        let app = app.clone();
        let id = id.clone();
        async move {
            post(&app, &format!("/api/v1/doctor/articles/{id}/cover"), Some(&token), &json!({"version":version,"mime_type":"image/jpeg","base64_content":minimal_jpeg_base64()})).await
        }
    };
    // Another Doctor cannot set it (looks like a version/ownership conflict, no leak).
    assert_eq!(
        upload(other.clone(), a["version"].clone()).await.status(),
        StatusCode::CONFLICT
    );
    // Spoofed content is rejected.
    let res = post(&app, &format!("/api/v1/doctor/articles/{id}/cover"), Some(&doctor), &json!({"version":a["version"],"mime_type":"image/png","base64_content":minimal_jpeg_base64()})).await;
    assert!(res.status().is_client_error());
    let res = upload(doctor.clone(), a["version"].clone()).await;
    assert_eq!(res.status(), StatusCode::OK);
    let a = json_body(res).await["data"].clone();
    assert!(a["cover_image_id"].is_string());
    let path = format!("/api/v1/articles/{id}/cover");
    assert_eq!(
        get(&app, &path, Some(&doctor)).await.status(),
        StatusCode::OK,
        "author sees own draft cover"
    );
    assert_eq!(
        get(&app, &path, Some(&patient)).await.status(),
        StatusCode::NOT_FOUND,
        "draft cover is private"
    );
    assert_eq!(
        get(&app, &path, Some(&other)).await.status(),
        StatusCode::NOT_FOUND
    );
    let res = post(
        &app,
        &format!("/api/v1/doctor/articles/{id}/publish"),
        Some(&doctor),
        &json!({"version":a["version"]}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let res = get(&app, &path, Some(&patient)).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()["content-type"], "image/jpeg");
    let detail =
        json_body(get(&app, &format!("/api/v1/articles/{id}"), Some(&patient)).await).await;
    assert_eq!(detail["data"]["cover_url"], format!("/articles/{id}/cover"));
    // Stale version is refused.
    assert_eq!(
        upload(doctor.clone(), a["version"].clone()).await.status(),
        StatusCode::CONFLICT
    );
}

async fn set_status(pool: &PgPool, profile: &str, status: &str, note: Option<&str>) {
    sqlx::query("UPDATE doctor_profiles SET professional_verification_status=$2::professional_verification_status_enum, professional_verification_note=$3, professional_verification_reviewed_at=CASE WHEN $2='PENDING' THEN NULL ELSE NOW() END WHERE id=$1")
        .bind(uid(profile)).bind(status).bind(note).execute(pool).await.unwrap();
}
fn submission(count: usize) -> Value {
    json!({"message":"Clearer licence attached.","documents":(0..count).map(|_| json!({"mime_type":"image/jpeg","base64_content":minimal_jpeg_base64()})).collect::<Vec<_>>()})
}

#[sqlx::test(migrations = "./migrations")]
async fn test_rejected_doctor_resubmits_documents_for_admin_review(pool: PgPool) {
    let (app, pool, _state) = test_app_and_state(pool).await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let other = make_token(uid(DOCTOR2_USER_ID), "DOCTOR");
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    // A verified Doctor has nothing to resubmit.
    assert_eq!(
        post(
            &app,
            "/api/v1/doctor/verification/submissions",
            Some(&doctor),
            &submission(1)
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    set_status(
        &pool,
        DOCTOR1_PROFILE_ID,
        "REJECTED",
        Some("Licence unclear"),
    )
    .await;
    assert_eq!(
        post(
            &app,
            "/api/v1/doctor/verification/submissions",
            Some(&doctor),
            &submission(0)
        )
        .await
        .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        post(
            &app,
            "/api/v1/doctor/verification/submissions",
            Some(&doctor),
            &submission(6)
        )
        .await
        .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let res = post(
        &app,
        "/api/v1/doctor/verification/submissions",
        Some(&doctor),
        &submission(2),
    )
    .await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let (status, note, reviewed): (String, Option<String>, Option<chrono::DateTime<chrono::Utc>>) = sqlx::query_as("SELECT professional_verification_status::text, professional_verification_note, professional_verification_reviewed_at FROM doctor_profiles WHERE id=$1")
        .bind(uid(DOCTOR1_PROFILE_ID)).fetch_one(&pool).await.unwrap();
    assert_eq!(status, "PENDING");
    assert!(note.is_none() && reviewed.is_none());
    // Own history keeps the reviewer's earlier note; other Doctors see nothing.
    let own = json_body(
        get(
            &app,
            "/api/v1/doctor/verification/submissions",
            Some(&doctor),
        )
        .await,
    )
    .await;
    assert_eq!(own["data"][0]["previous_status"], "REJECTED");
    assert_eq!(own["data"][0]["previous_note"], "Licence unclear");
    assert_eq!(own["data"][0]["documents"].as_array().unwrap().len(), 2);
    let foreign = json_body(
        get(
            &app,
            "/api/v1/doctor/verification/submissions",
            Some(&other),
        )
        .await,
    )
    .await;
    assert!(foreign["data"].as_array().unwrap().is_empty());
    // Admin lists and opens the documents; a Doctor cannot use the Admin routes.
    let list = json_body(
        get(
            &app,
            &format!("/api/v1/admin/doctors/{DOCTOR1_PROFILE_ID}/verification-submissions"),
            Some(&admin),
        )
        .await,
    )
    .await;
    let doc_id = list["data"][0]["documents"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let res = get(
        &app,
        &format!("/api/v1/admin/doctors/{DOCTOR1_PROFILE_ID}/verification-documents/{doc_id}"),
        Some(&admin),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()["content-type"], "image/jpeg");
    assert_eq!(
        get(
            &app,
            &format!("/api/v1/admin/doctors/{DOCTOR1_PROFILE_ID}/verification-documents/{doc_id}"),
            Some(&doctor)
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    // The pending queue shows the Doctor again and Admin can review as before.
    let res = post(
        &app,
        &format!("/api/v1/admin/doctors/{DOCTOR1_PROFILE_ID}/verify"),
        Some(&admin),
        &json!({}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    // Once verified, no further submissions.
    assert_eq!(
        post(
            &app,
            "/api/v1/doctor/verification/submissions",
            Some(&doctor),
            &submission(1)
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_doctor_cannot_self_verify_through_the_resubmission_guard(pool: PgPool) {
    let (_app, pool, state) = test_app_and_state(pool).await;
    set_status(
        &pool,
        DOCTOR1_PROFILE_ID,
        "REJECTED",
        Some("Licence unclear"),
    )
    .await;
    let ctx = isolation_ctx(
        UserId::from(uid(DOCTOR1_USER_ID)),
        hellodoctor_backend::domain::models::UserRole::Doctor,
    );
    // Through the real API role and RLS context: only REJECTED -> PENDING with cleared review fields.
    for sql in [
        "UPDATE doctor_profiles SET professional_verification_status='VERIFIED', professional_verification_reviewed_at=NOW() WHERE id=$1",
        "UPDATE doctor_profiles SET professional_verification_status='PENDING' WHERE id=$1",
    ] {
        let mut tx = state.begin_authed(&ctx).await.unwrap();
        let result = sqlx::query(sql).bind(uid(DOCTOR1_PROFILE_ID)).execute(&mut *tx).await;
        assert!(result.is_err(), "{sql} must be refused");
    }
    let mut tx = state.begin_authed(&ctx).await.unwrap();
    sqlx::query("UPDATE doctor_profiles SET professional_verification_status='PENDING', professional_verification_reviewed_at=NULL, professional_verification_reviewed_by=NULL, professional_verification_note=NULL WHERE id=$1")
        .bind(uid(DOCTOR1_PROFILE_ID)).execute(&mut *tx).await.expect("own rejected -> pending resubmission");
    tx.rollback().await.unwrap();
    // Another Doctor cannot resubmit on someone else's behalf.
    let foreign = isolation_ctx(
        UserId::from(uid(DOCTOR2_USER_ID)),
        hellodoctor_backend::domain::models::UserRole::Doctor,
    );
    let mut tx = state.begin_authed(&foreign).await.unwrap();
    let changed = sqlx::query("UPDATE doctor_profiles SET professional_verification_status='PENDING', professional_verification_reviewed_at=NULL, professional_verification_reviewed_by=NULL, professional_verification_note=NULL WHERE id=$1")
        .bind(uid(DOCTOR1_PROFILE_ID)).execute(&mut *tx).await;
    assert!(changed.map(|r| r.rows_affected() == 0).unwrap_or(true));
}
