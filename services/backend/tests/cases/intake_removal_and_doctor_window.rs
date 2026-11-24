//! Product-owner instructions of 2026-11-14:
//! - a patient can remove their own wrongly uploaded intake page and upload
//!   the right one, while the consultation still accepts uploads;
//! - a doctor's access to their own consultation's prescriptions and patient
//!   uploads lasts 20 days after the consultation ended (was: forever).
//!
//! Migrations `20261221000001_patient_intake_removal` and
//! `20261221000002_doctor_document_20_day_window`.

use super::*;

fn one_page() -> serde_json::Value {
    serde_json::json!({ "files": [{ "mime_type": "image/jpeg", "base64_content": minimal_jpeg_base64() }] })
}

async fn upload_pages(app: &axum::Router, token: &str, appointment: Uuid, n: usize) {
    let files: Vec<_> = (0..n)
        .map(|_| serde_json::json!({ "mime_type": "image/jpeg", "base64_content": minimal_jpeg_base64() }))
        .collect();
    let response = post(
        app,
        &format!("/api/v1/appointments/{appointment}/prescriptions"),
        Some(token),
        &serde_json::json!({ "files": files }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
}

async fn listed(app: &axum::Router, token: &str, appointment: Uuid) -> serde_json::Value {
    let response = get(
        app,
        &format!("/api/v1/appointments/{appointment}/prescriptions"),
        Some(token),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    json_body(response).await["data"].clone()
}

#[sqlx::test(migrations = "./migrations")]
async fn patient_removes_wrong_intake_page_and_reuploads(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let (appointment, _) = book_and_confirm(&app, &patient).await;
    upload_pages(&app, &patient, appointment, 3).await;

    let before = listed(&app, &patient, appointment).await;
    assert_eq!(before["editable"], true);
    let docs = before["documents"].as_array().unwrap();
    assert_eq!(docs.len(), 3);
    let page_two = docs.iter().find(|d| d["page_number"] == 2).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let page_two_id = Uuid::parse_str(&page_two).unwrap();
    let object_key: String =
        sqlx::query_scalar("SELECT object_key FROM prescription_intake_documents WHERE id=$1")
            .bind(page_two_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    // Another patient and the doctor cannot remove it.
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let remove_uri = format!("/api/v1/appointments/{appointment}/prescriptions/{page_two}");
    assert_eq!(
        delete(&app, &remove_uri, Some(&doctor)).await.status(),
        StatusCode::FORBIDDEN
    );

    let removed = delete(&app, &remove_uri, Some(&patient)).await;
    assert_eq!(removed.status(), StatusCode::OK);
    assert_eq!(
        delete(&app, &remove_uri, Some(&patient)).await.status(),
        StatusCode::NOT_FOUND,
        "a removed page is gone"
    );

    let after = listed(&app, &patient, appointment).await;
    let pages: Vec<i64> = after["documents"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["page_number"].as_i64().unwrap())
        .collect();
    assert_eq!(pages, vec![1, 3]);

    // The stored object is queued for durable deletion and is deleted.
    let queued: String = sqlx::query_scalar(
        "SELECT object_key FROM intake_document_removal_jobs WHERE document_id=$1",
    )
    .bind(page_two_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(queued, object_key);
    hellodoctor_backend::services::notification_scheduler::run_intake_removal_cleanup(&state)
        .await
        .unwrap();
    let job_state: String =
        sqlx::query_scalar("SELECT state FROM intake_document_removal_jobs WHERE document_id=$1")
            .bind(page_two_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(job_state, "COMPLETED");
    assert!(matches!(
        test_object_storage()
            .get(&object_store::path::Path::from(object_key))
            .await,
        Err(object_store::Error::NotFound { .. })
    ));

    // Re-upload fills the freed page number, and the cap still counts pages.
    upload_pages(&app, &patient, appointment, 1).await;
    let refilled = listed(&app, &patient, appointment).await;
    let mut pages: Vec<i64> = refilled["documents"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["page_number"].as_i64().unwrap())
        .collect();
    pages.sort();
    assert_eq!(pages, vec![1, 2, 3]);
    upload_pages(&app, &patient, appointment, 17).await;
    let over = post(
        &app,
        &format!("/api/v1/appointments/{appointment}/prescriptions"),
        Some(&patient),
        &one_page(),
    )
    .await;
    assert_eq!(over.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrations = "./migrations")]
async fn patient_cannot_remove_intake_after_consultation_completed(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let (appointment, _) = book_and_confirm(&app, &patient).await;
    upload_pages(&app, &patient, appointment, 1).await;
    let id: Uuid =
        sqlx::query_scalar("SELECT id FROM prescription_intake_documents WHERE appointment_id=$1")
            .bind(appointment)
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query("UPDATE appointments SET status='COMPLETED' WHERE id=$1")
        .bind(appointment)
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(listed(&app, &patient, appointment).await["editable"], false);
    let response = delete(
        &app,
        &format!("/api/v1/appointments/{appointment}/prescriptions/{id}"),
        Some(&patient),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let still: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM prescription_intake_documents WHERE appointment_id=$1",
    )
    .bind(appointment)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        still, 1,
        "a completed consultation's upload stays in the record"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn doctor_document_access_ends_20_days_after_consultation(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let (appointment, _) = book_and_confirm(&app, &patient).await;
    upload_pages(&app, &patient, appointment, 1).await;
    open_clinical_window(&pool, appointment).await;
    sqlx::query("INSERT INTO consultation_sessions(appointment_id,agora_channel_name) VALUES($1,$1::text) ON CONFLICT(appointment_id) DO NOTHING")
        .bind(appointment)
        .execute(&pool)
        .await
        .unwrap();
    // A prescription needs a started consultation (2026-11-24).
    start_consultation(&pool, appointment).await;
    let rx = post(
        &app,
        &format!("/api/v1/consultations/{appointment}/prescription"),
        Some(&doctor),
        &serde_json::json!({"base64_photo":minimal_jpeg_base64(),"mime_type":"image/jpeg"}),
    )
    .await;
    assert_eq!(rx.status(), StatusCode::CREATED);
    let rx_id = json_body(rx).await["data"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let intake_id: Uuid =
        sqlx::query_scalar("SELECT id FROM prescription_intake_documents WHERE appointment_id=$1")
            .bind(appointment)
            .fetch_one(&pool)
            .await
            .unwrap();
    let rx_uri = format!("/api/v1/appointments/{appointment}/documents/{rx_id}");
    let intake_uri = format!("/api/v1/appointments/{appointment}/documents/{intake_id}");
    let summary_uri = format!("/api/v1/doctor/consultations/{appointment}/summary");

    let end = |days: i32| {
        let pool = pool.clone();
        async move {
            sqlx::query("UPDATE appointments SET status='COMPLETED', completed_at=NOW()-make_interval(days=>$2) WHERE id=$1")
                .bind(appointment).bind(days).execute(&pool).await.unwrap();
            sqlx::query("UPDATE consultation_sessions SET ended_at=NOW()-make_interval(days=>$2) WHERE appointment_id=$1")
                .bind(appointment).bind(days).execute(&pool).await.unwrap();
        }
    };

    // Day 19 after the consultation: still open.
    end(19).await;
    assert_eq!(
        get(&app, &rx_uri, Some(&doctor)).await.status(),
        StatusCode::OK
    );
    assert_eq!(
        get(&app, &intake_uri, Some(&doctor)).await.status(),
        StatusCode::OK
    );
    let summary = json_body(get(&app, &summary_uri, Some(&doctor)).await).await;
    assert_eq!(
        summary["data"]["prescriptions"].as_array().unwrap().len(),
        1
    );
    assert_eq!(summary["data"]["documents"].as_array().unwrap().len(), 1);
    assert_eq!(summary["data"]["files_access_ended"], false);

    // Day 21: closed for the doctor, files and listing alike.
    end(21).await;
    assert_eq!(
        get(&app, &rx_uri, Some(&doctor)).await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        get(&app, &intake_uri, Some(&doctor)).await.status(),
        StatusCode::NOT_FOUND
    );
    let summary = json_body(get(&app, &summary_uri, Some(&doctor)).await).await;
    assert_eq!(
        summary["data"]["prescriptions"].as_array().unwrap().len(),
        0
    );
    assert_eq!(summary["data"]["documents"].as_array().unwrap().len(), 0);
    assert_eq!(
        summary["data"]["files_access_ended"], true,
        "the client must say access ended, not that there were no files"
    );

    // The patient and Admin keep access.
    assert_eq!(
        get(&app, &rx_uri, Some(&patient)).await.status(),
        StatusCode::OK
    );
    assert_eq!(
        get(
            &app,
            &rx_uri,
            Some(&token_for_real_role(&pool, "ADMIN").await)
        )
        .await
        .status(),
        StatusCode::OK
    );
}
