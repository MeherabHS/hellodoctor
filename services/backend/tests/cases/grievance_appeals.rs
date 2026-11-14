//! Grievance appeals recorded by an Admin (docs/admin-ui/ADMIN_REDESIGN_PLAN.md):
//! only WARNED/DISMISSED decisions, one open appeal, UPHOLD keeps the
//! decision, OVERTURN supersedes it and reopens the grievance for a fresh
//! adjudication while keeping the old decision as history.
use super::*;
use serde_json::json;

async fn filed_grievance(app: &axum::Router, pool: &PgPool, patient: &str) -> Uuid {
    let (apt_id, _) = book_and_confirm(app, patient).await;
    let session = consultation_repo::find_by_appointment(pool, apt_id)
        .await
        .unwrap()
        .unwrap();
    let res = post(
        app,
        "/api/v1/grievances",
        Some(patient),
        &json!({"consultation_id": session.id, "target": "DOCTOR", "category": "Communication", "claim_summary": "The doctor ended the call early."}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::CREATED);
    Uuid::parse_str(
        json_body(res).await["data"]["grievance_id"]
            .as_str()
            .unwrap(),
    )
    .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn test_appeal_requires_a_warned_or_dismissed_decision_and_one_open_appeal(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let grv = filed_grievance(&app, &pool, &patient).await;
    let appeal_uri = format!("/api/v1/admin/grievances/{grv}/appeals");
    let reason =
        json!({"reason": "Patient called: the doctor's network failed, not the patient's."});

    // Undecided grievances cannot be appealed.
    let res = post(&app, &appeal_uri, Some(&admin), &reason).await;
    assert_eq!(res.status(), StatusCode::CONFLICT);

    let res = post(
        &app,
        &format!("/api/v1/admin/grievances/{grv}/dismiss"),
        Some(&admin),
        &json!({"audit_notes": "No fault found."}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);

    // Validation: too short, unknown field.
    let res = post(&app, &appeal_uri, Some(&admin), &json!({"reason": "short"})).await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let res = post(
        &app,
        &appeal_uri,
        Some(&admin),
        &json!({"reason": "A long enough reason here", "extra": 1}),
    )
    .await;
    assert!(res.status().is_client_error());

    // Patients and doctors cannot record appeals.
    let res = post(&app, &appeal_uri, Some(&patient), &reason).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    let res = post(&app, &appeal_uri, Some(&admin), &reason).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let res = post(&app, &appeal_uri, Some(&admin), &reason).await;
    assert_eq!(
        res.status(),
        StatusCode::CONFLICT,
        "only one open appeal per grievance"
    );

    let list = json_body(get(&app, "/api/v1/admin/grievances/appeals", Some(&admin)).await).await;
    let items = list["data"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["status"], "OPEN");
    assert_eq!(items[0]["appealed_action"], "DISMISS");
    assert_eq!(items[0]["grievance_status"], "DISMISSED");
    let res = get(&app, "/api/v1/admin/grievances/appeals", Some(&patient)).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_uphold_keeps_the_decision_and_overturn_reopens_for_a_new_one(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");

    // Uphold.
    let grv = filed_grievance(&app, &pool, &patient).await;
    post(
        &app,
        &format!("/api/v1/admin/grievances/{grv}/warn"),
        Some(&admin),
        &json!({"audit_notes": "Warning logged."}),
    )
    .await;
    let appeal = json_body(
        post(
            &app,
            &format!("/api/v1/admin/grievances/{grv}/appeals"),
            Some(&admin),
            &json!({"reason": "Doctor disputes the warning by phone."}),
        )
        .await,
    )
    .await;
    let appeal_id = appeal["data"]["appeal_id"].as_str().unwrap().to_string();
    let decide = format!("/api/v1/admin/grievance-appeals/{appeal_id}/decide");
    let res = post(
        &app,
        &decide,
        Some(&admin),
        &json!({"decision": "MAYBE", "note": "Not a valid decision."}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let res = post(
        &app,
        &decide,
        Some(&admin),
        &json!({"decision": "UPHOLD", "note": "Call records support the warning."}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res).await["data"]["grievance_status"], "WARNED");
    let status: String =
        sqlx::query_scalar("SELECT status::text FROM grievance_reports WHERE id=$1")
            .bind(grv)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "WARNED");
    let res = post(
        &app,
        &decide,
        Some(&admin),
        &json!({"decision": "OVERTURN", "note": "Trying to decide it again."}),
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::CONFLICT,
        "a decided appeal cannot be decided again"
    );

    // Overturn.
    let grv = filed_grievance(&app, &pool, &patient).await;
    post(
        &app,
        &format!("/api/v1/admin/grievances/{grv}/dismiss"),
        Some(&admin),
        &json!({"audit_notes": "First decision."}),
    )
    .await;
    let appeal = json_body(
        post(
            &app,
            &format!("/api/v1/admin/grievances/{grv}/appeals"),
            Some(&admin),
            &json!({"reason": "New evidence from the patient."}),
        )
        .await,
    )
    .await;
    let appeal_id = appeal["data"]["appeal_id"].as_str().unwrap().to_string();
    let res = post(
        &app,
        &format!("/api/v1/admin/grievance-appeals/{appeal_id}/decide"),
        Some(&admin),
        &json!({"decision": "OVERTURN", "note": "Evidence changes the outcome."}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        json_body(res).await["data"]["grievance_status"],
        "PENDING_REVIEW"
    );

    let (current, superseded): (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*) FILTER (WHERE superseded_at IS NULL), COUNT(*) FILTER (WHERE superseded_at IS NOT NULL) FROM grievance_adjudications WHERE grievance_id=$1",
    ).bind(grv).fetch_one(&pool).await.unwrap();
    assert_eq!(
        (current, superseded),
        (0, 1),
        "the overturned decision is kept as history"
    );

    // The grievance can now be decided again through the normal endpoint.
    let res = post(
        &app,
        &format!("/api/v1/admin/grievances/{grv}/warn"),
        Some(&admin),
        &json!({"audit_notes": "Second decision after appeal."}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let (current, superseded): (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*) FILTER (WHERE superseded_at IS NULL), COUNT(*) FILTER (WHERE superseded_at IS NOT NULL) FROM grievance_adjudications WHERE grievance_id=$1",
    ).bind(grv).fetch_one(&pool).await.unwrap();
    assert_eq!((current, superseded), (1, 1));

    // The admin queue shows the current decision, not the superseded one.
    let list = json_body(get(&app, "/api/v1/admin/grievances", Some(&admin)).await).await;
    let row = list["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["id"] == json!(grv.to_string()))
        .cloned()
        .unwrap();
    assert_eq!(row["status"], "WARNED");

    // A second decision still cannot be stacked on the current one.
    let res = post(
        &app,
        &format!("/api/v1/admin/grievances/{grv}/dismiss"),
        Some(&admin),
        &json!({"audit_notes": "Stacking attempt."}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::CONFLICT);

    let audit: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_events WHERE action IN ('GRIEVANCE_APPEAL_RECORDED','GRIEVANCE_APPEAL_UPHELD','GRIEVANCE_APPEAL_OVERTURNED')")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(audit, 4);
}
