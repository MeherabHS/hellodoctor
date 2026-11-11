//! Regression coverage for Admin Campaigns: Milestone 6B (draft/review/
//! approval foundations, migration `20261125000001_admin_campaigns`) plus
//! Milestone 6C (schedule/pause/resume/cancel/dispatch/monitoring, migrations
//! `20261126000001_campaign_status_new_values` and
//! `20261126000002_campaign_scheduling_and_delivery`). Real HTTP-level
//! integration tests against the full router (`use super::*;` pulls in
//! `test_app`/`test_app_and_state`, the `get`/`post`/`send`/`json_body`
//! request helpers, `make_token`/`uid`, and the seeded `ADMIN_USER_ID`/
//! `DOCTOR1_USER_ID`/`DOCTOR2_PROFILE_ID`/`PATIENT_USER_ID` identities, the
//! same way `schedule_exclusivity.rs` and `profile_pictures.rs` do), so auth
//! middleware, ABAC, RLS and the campaign guard trigger are genuinely
//! exercised end to end — not just handler logic in isolation. The dispatch
//! tests call `notification_scheduler::run_campaign_dispatch_pass` directly
//! (not through the 60-second background loop), the same way
//! `push_notification_delivery.rs` exercises `run_push_delivery_pass`.
use super::*;
use hellodoctor_backend::repository::notification_repo;
use hellodoctor_backend::services::notification_scheduler;

fn campaign_content(audience: &str) -> Value {
    serde_json::json!({
        "title_en": "New clinic hours",
        "title_sw": "Masaa mapya ya kliniki",
        "body_en": "Our clinic now opens earlier on weekdays.",
        "body_sw": "Kliniki yetu sasa inafunguliwa mapema siku za wiki.",
        "audience": audience
    })
}

async fn draft_campaign(app: &axum::Router, admin: &str, audience: &str) -> Value {
    json_body(
        post(
            app,
            "/api/v1/admin/campaigns",
            Some(admin),
            &serde_json::json!({
                "creation_key": Uuid::new_v4().to_string(),
                "content": campaign_content(audience)
            }),
        )
        .await,
    )
    .await["data"]
        .clone()
}

#[sqlx::test(migrations = "./migrations")]
async fn test_campaign_authorization_is_admin_only(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let body = serde_json::json!({
        "creation_key": Uuid::new_v4().to_string(),
        "content": campaign_content("PATIENTS")
    });

    for token in [doctor.as_str(), patient.as_str()] {
        assert_eq!(
            post(&app, "/api/v1/admin/campaigns", Some(token), &body)
                .await
                .status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            get(&app, "/api/v1/admin/campaigns", Some(token))
                .await
                .status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            get(
                &app,
                "/api/v1/admin/campaigns/audience-preview?audience=BOTH",
                Some(token)
            )
            .await
            .status(),
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(
        post(&app, "/api/v1/admin/campaigns", None, &body)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        get(&app, "/api/v1/admin/campaigns", None).await.status(),
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_campaign_draft_submit_approve_lifecycle_and_version_aware_approval(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");

    let campaign = draft_campaign(&app, &admin, "BOTH").await;
    let id = campaign["id"].as_str().unwrap().to_string();
    assert_eq!(campaign["status"], "DRAFT");
    assert_eq!(campaign["version"], 1);
    assert_eq!(campaign["content_version"], 1);
    assert!(campaign["approved_content_version"].is_null());

    // A fresh draft cannot be approved directly — it must go through review.
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/admin/campaigns/{id}/approve"),
            Some(&admin),
            &serde_json::json!({"version": 1})
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );

    let submitted = json_body(
        post(
            &app,
            &format!("/api/v1/admin/campaigns/{id}/submit"),
            Some(&admin),
            &serde_json::json!({"version": 1}),
        )
        .await,
    )
    .await["data"]
        .clone();
    assert_eq!(submitted["status"], "PENDING_REVIEW");
    assert_eq!(submitted["version"], 2);

    let approved = json_body(
        post(
            &app,
            &format!("/api/v1/admin/campaigns/{id}/approve"),
            Some(&admin),
            &serde_json::json!({"version": 2}),
        )
        .await,
    )
    .await["data"]
        .clone();
    assert_eq!(approved["status"], "APPROVED");
    assert_eq!(approved["version"], 3);
    // Content hasn't changed since creation, so content_version is still 1
    // even though `version` (the row-revision counter) has advanced to 3 —
    // approval is bound to content_version, which is what lets it survive a
    // later schedule/pause/resume without those looking like an edit.
    assert_eq!(approved["content_version"], 1);
    assert_eq!(approved["approved_content_version"], 1);
    assert_eq!(approved["approved_by"], uid(ADMIN_USER_ID).to_string());
    assert!(approved["approved_at"].is_string());

    // Editing an approved campaign always invalidates the approval: status
    // returns to DRAFT and the approval columns are cleared, so the content
    // change can never silently keep riding on the old sign-off.
    let mut content = campaign_content("BOTH");
    content["title_en"] = serde_json::json!("Updated clinic hours");
    let edited = json_body(
        send(
            &app,
            "PATCH",
            &format!("/api/v1/admin/campaigns/{id}"),
            Some(&admin),
            Some(&serde_json::json!({"version": 3, "content": content})),
            None,
        )
        .await,
    )
    .await["data"]
        .clone();
    assert_eq!(edited["status"], "DRAFT");
    assert_eq!(edited["version"], 4);
    assert_eq!(edited["content_version"], 2);
    assert_eq!(edited["title_en"], "Updated clinic hours");
    assert!(edited["approved_content_version"].is_null());
    assert!(edited["approved_by"].is_null());
    assert!(edited["approved_at"].is_null());

    // The now-stale pre-edit version can never be used to approve again.
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/admin/campaigns/{id}/approve"),
            Some(&admin),
            &serde_json::json!({"version": 2})
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );

    let actions: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_events WHERE resource_type='CAMPAIGN' AND resource_id=$1 \
         AND action IN ('CAMPAIGN_CREATE','CAMPAIGN_SUBMIT','CAMPAIGN_APPROVE','CAMPAIGN_EDIT')",
    )
    .bind(Uuid::parse_str(&id).unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(actions, 4);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_campaign_rejects_invalid_content_and_unknown_fields(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");

    let mut empty_title = campaign_content("PATIENTS");
    empty_title["title_en"] = serde_json::json!("");
    assert_eq!(
        post(
            &app,
            "/api/v1/admin/campaigns",
            Some(&admin),
            &serde_json::json!({"creation_key": Uuid::new_v4().to_string(), "content": empty_title})
        )
        .await
        .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );

    let mut too_long = campaign_content("PATIENTS");
    too_long["body_en"] = serde_json::json!("x".repeat(601));
    assert_eq!(
        post(
            &app,
            "/api/v1/admin/campaigns",
            Some(&admin),
            &serde_json::json!({"creation_key": Uuid::new_v4().to_string(), "content": too_long})
        )
        .await
        .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );

    let mut invalid_audience = campaign_content("PATIENTS");
    invalid_audience["audience"] = serde_json::json!("HIV_POSITIVE_PATIENTS");
    assert_eq!(
        post(
            &app,
            "/api/v1/admin/campaigns",
            Some(&admin),
            &serde_json::json!({"creation_key": Uuid::new_v4().to_string(), "content": invalid_audience})
        )
        .await
        .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );

    // No field exists through which a caller could add clinical/diagnosis
    // targeting criteria — any unknown field anywhere in the payload is
    // rejected outright (`#[serde(deny_unknown_fields)]`), the same
    // mass-assignment protection the Doctor article create endpoint uses.
    for forbidden in [
        "diagnosis",
        "hiv_status",
        "medical_record_id",
        "status",
        "approved_by",
    ] {
        let mut payload = serde_json::json!({
            "creation_key": Uuid::new_v4().to_string(),
            "content": campaign_content("PATIENTS")
        });
        payload[forbidden] = serde_json::json!("x");
        assert_eq!(
            post(&app, "/api/v1/admin/campaigns", Some(&admin), &payload)
                .await
                .status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "expected rejection for unknown field {forbidden}"
        );
        let mut nested = serde_json::json!({
            "creation_key": Uuid::new_v4().to_string(),
            "content": campaign_content("PATIENTS")
        });
        nested["content"][forbidden] = serde_json::json!("x");
        assert_eq!(
            post(&app, "/api/v1/admin/campaigns", Some(&admin), &nested)
                .await
                .status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "expected rejection for unknown nested content field {forbidden}"
        );
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn test_campaign_creation_is_idempotent_and_detects_content_mismatch(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let payload = serde_json::json!({
        "creation_key": Uuid::new_v4().to_string(),
        "content": campaign_content("DOCTORS")
    });

    // Two concurrent requests with the identical creation key and content
    // must settle on exactly one row, never two.
    let (a, b) = tokio::join!(
        post(&app, "/api/v1/admin/campaigns", Some(&admin), &payload),
        post(&app, "/api/v1/admin/campaigns", Some(&admin), &payload)
    );
    assert_eq!(a.status(), StatusCode::OK);
    assert_eq!(b.status(), StatusCode::OK);
    let a = json_body(a).await;
    let b = json_body(b).await;
    assert_eq!(a["data"]["id"], b["data"]["id"]);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM campaigns WHERE id=$1")
        .bind(Uuid::parse_str(a["data"]["id"].as_str().unwrap()).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    let create_audit_rows: i64 =
        sqlx::query_scalar("SELECT count(*) FROM audit_events WHERE action='CAMPAIGN_CREATE'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(create_audit_rows, 1);

    // The same creation key reused with DIFFERENT content is a genuine
    // conflict, not a silent overwrite or a second row.
    let mut different_content = payload.clone();
    different_content["content"]["title_en"] = serde_json::json!("A completely different title");
    assert_eq!(
        post(
            &app,
            "/api/v1/admin/campaigns",
            Some(&admin),
            &different_content
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_campaign_stale_version_is_rejected_as_conflict_not_overwritten(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let campaign = draft_campaign(&app, &admin, "PATIENTS").await;
    let id = campaign["id"].as_str().unwrap();

    let mut content = campaign_content("PATIENTS");
    content["title_en"] = serde_json::json!("First edit");
    assert_eq!(
        send(
            &app,
            "PATCH",
            &format!("/api/v1/admin/campaigns/{id}"),
            Some(&admin),
            Some(&serde_json::json!({"version": 1, "content": content})),
            None
        )
        .await
        .status(),
        StatusCode::OK
    );

    // Retrying the edit against the now-stale version 1 must not silently
    // overwrite the row that already moved to version 2.
    let stale = send(
        &app,
        "PATCH",
        &format!("/api/v1/admin/campaigns/{id}"),
        Some(&admin),
        Some(&serde_json::json!({"version": 1, "content": content})),
        None,
    )
    .await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_campaign_cancel_is_terminal(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let campaign = draft_campaign(&app, &admin, "PATIENTS").await;
    let id = campaign["id"].as_str().unwrap();

    let cancelled = json_body(
        post(
            &app,
            &format!("/api/v1/admin/campaigns/{id}/cancel"),
            Some(&admin),
            &serde_json::json!({"version": 1}),
        )
        .await,
    )
    .await["data"]
        .clone();
    assert_eq!(cancelled["status"], "CANCELLED");
    assert!(cancelled["cancelled_at"].is_string());

    // Nothing can move a cancelled campaign again: not submit, not approve,
    // not edit, not a second cancel.
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/admin/campaigns/{id}/submit"),
            Some(&admin),
            &serde_json::json!({"version": 2})
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(
        send(
            &app,
            "PATCH",
            &format!("/api/v1/admin/campaigns/{id}"),
            Some(&admin),
            Some(&serde_json::json!({"version": 2, "content": campaign_content("PATIENTS")})),
            None
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/admin/campaigns/{id}/cancel"),
            Some(&admin),
            &serde_json::json!({"version": 2})
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_campaign_list_pagination_and_status_filter_is_deterministic(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");

    let mut ids = Vec::new();
    for _ in 0..3 {
        let c = draft_campaign(&app, &admin, "PATIENTS").await;
        ids.push(c["id"].as_str().unwrap().to_string());
    }
    let submitted_id = ids[1].clone();
    post(
        &app,
        &format!("/api/v1/admin/campaigns/{submitted_id}/submit"),
        Some(&admin),
        &serde_json::json!({"version": 1}),
    )
    .await;

    let page1 = json_body(
        get(
            &app,
            "/api/v1/admin/campaigns?limit=2&offset=0",
            Some(&admin),
        )
        .await,
    )
    .await;
    let page2 = json_body(
        get(
            &app,
            "/api/v1/admin/campaigns?limit=2&offset=2",
            Some(&admin),
        )
        .await,
    )
    .await;
    let mut seen: Vec<String> = page1["data"]
        .as_array()
        .unwrap()
        .iter()
        .chain(page2["data"].as_array().unwrap())
        .map(|c| c["id"].as_str().unwrap().to_string())
        .collect();
    seen.sort();
    let mut expected = ids.clone();
    expected.sort();
    assert_eq!(seen, expected);
    assert!(page1["data"].as_array().unwrap().len() <= 2);

    let pending_only = json_body(
        get(
            &app,
            "/api/v1/admin/campaigns?status=PENDING_REVIEW",
            Some(&admin),
        )
        .await,
    )
    .await;
    let pending_ids: Vec<String> = pending_only["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(pending_ids, vec![submitted_id]);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_campaign_audience_preview_is_pii_safe_and_excludes_unverified_or_inactive_accounts(
    pool: PgPool,
) {
    let (app, pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");

    // Seed baseline: exactly one patient and two verified doctors.
    let preview = |app: axum::Router, admin: String, audience: &'static str| {
        let uri = format!("/api/v1/admin/campaigns/audience-preview?audience={audience}");
        async move { json_body(get(&app, &uri, Some(&admin)).await).await }
    };
    let patients = preview(app.clone(), admin.clone(), "PATIENTS").await;
    let doctors = preview(app.clone(), admin.clone(), "DOCTORS").await;
    let both = preview(app.clone(), admin.clone(), "BOTH").await;
    // Only a bare count is ever returned — never recipient identities.
    assert!(patients["data"]["eligible_count"].is_number());
    assert!(patients["data"].get("recipients").is_none());
    assert!(patients["data"].get("rows").is_none());
    assert_eq!(patients["data"]["eligible_count"], 1);
    assert_eq!(doctors["data"]["eligible_count"], 2);
    assert_eq!(both["data"]["eligible_count"], 3);

    // An unverified doctor must not count toward the Doctors audience.
    sqlx::query(
        "UPDATE doctor_profiles SET professional_verification_status='PENDING' WHERE id=$1",
    )
    .bind(uid(DOCTOR2_PROFILE_ID))
    .execute(&pool)
    .await
    .unwrap();
    let doctors = preview(app.clone(), admin.clone(), "DOCTORS").await;
    assert_eq!(doctors["data"]["eligible_count"], 1);

    // A deactivated patient account must not count toward the Patients audience.
    sqlx::query("UPDATE users SET is_active=false WHERE id=$1")
        .bind(uid(PATIENT_USER_ID))
        .execute(&pool)
        .await
        .unwrap();
    let patients = preview(app.clone(), admin.clone(), "PATIENTS").await;
    assert_eq!(patients["data"]["eligible_count"], 0);
    let both = preview(app, admin, "BOTH").await;
    assert_eq!(both["data"]["eligible_count"], 1);
}

// ============================================================================
// Milestone 6C — schedule/pause/resume/cancel, dispatch fan-out, monitoring.
// ============================================================================

/// Creates, submits and approves a campaign in one call, returning the
/// approved JSON (`version` == 3: create=1, submit=2, approve=3).
async fn approved_campaign(app: &axum::Router, admin: &str, audience: &str) -> Value {
    let draft = draft_campaign(app, admin, audience).await;
    let id = draft["id"].as_str().unwrap().to_string();
    post(
        app,
        &format!("/api/v1/admin/campaigns/{id}/submit"),
        Some(admin),
        &serde_json::json!({"version": 1}),
    )
    .await;
    json_body(
        post(
            app,
            &format!("/api/v1/admin/campaigns/{id}/approve"),
            Some(admin),
            &serde_json::json!({"version": 2}),
        )
        .await,
    )
    .await["data"]
        .clone()
}

/// Inserts a Patient account with a caller-chosen `id` and
/// `preferred_language`, bypassing the app entirely (direct superuser test
/// pool) — used wherever a test needs to control recipient ordering or
/// language deterministically.
async fn new_patient_with_language_and_id(pool: &PgPool, id: Uuid, lang: &str) {
    sqlx::query("INSERT INTO users(id,phone_number,role,preferred_language,is_active,is_verified,created_at,updated_at) VALUES($1,$2,'PATIENT'::user_role_enum,$3,true,true,NOW(),NOW())")
        .bind(id)
        .bind(format!("+2547{:08}", rand::thread_rng().gen_range(0..100_000_000)))
        .bind(lang)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO patient_profiles(id,user_id,full_name,date_of_birth,gender,created_at,updated_at) VALUES($1,$2,'Campaign Fan-out Test Patient','1990-01-01','OTHER',NOW(),NOW())")
        .bind(Uuid::new_v4())
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn test_campaign_lifecycle_endpoints_are_admin_only_and_validate_schedule(pool: PgPool) {
    let (app, _pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let approved = approved_campaign(&app, &admin, "PATIENTS").await;
    let id = approved["id"].as_str().unwrap().to_string();

    for (path, body) in [
        (
            format!("/api/v1/admin/campaigns/{id}/schedule"),
            serde_json::json!({"version": 3, "scheduled_for": Utc::now().to_rfc3339()}),
        ),
        (
            format!("/api/v1/admin/campaigns/{id}/pause"),
            serde_json::json!({"version": 3}),
        ),
        (
            format!("/api/v1/admin/campaigns/{id}/resume"),
            serde_json::json!({"version": 3}),
        ),
    ] {
        assert_eq!(
            post(&app, &path, Some(&doctor), &body).await.status(),
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(
        get(
            &app,
            &format!("/api/v1/admin/campaigns/{id}/stats"),
            Some(&doctor)
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );

    // A schedule time meaningfully in the past is rejected.
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/admin/campaigns/{id}/schedule"),
            Some(&admin),
            &serde_json::json!({"version": 3, "scheduled_for": (Utc::now() - chrono::Duration::hours(1)).to_rfc3339()})
        )
        .await
        .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );

    // "Immediate" is simply scheduling for right now.
    let scheduled = json_body(
        post(
            &app,
            &format!("/api/v1/admin/campaigns/{id}/schedule"),
            Some(&admin),
            &serde_json::json!({"version": 3, "scheduled_for": Utc::now().to_rfc3339()}),
        )
        .await,
    )
    .await["data"]
        .clone();
    assert_eq!(scheduled["status"], "SCHEDULED");
    assert!(scheduled["scheduled_for"].is_string());

    // Pause requires SCHEDULED/SENDING — a DRAFT campaign cannot be paused.
    let other_draft = draft_campaign(&app, &admin, "PATIENTS").await;
    let other_id = other_draft["id"].as_str().unwrap();
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/admin/campaigns/{other_id}/pause"),
            Some(&admin),
            &serde_json::json!({"version": 1})
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_campaign_dispatch_localizes_dedupes_and_completes_fanout(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");

    let sw_patient = Uuid::new_v4();
    new_patient_with_language_and_id(&pool, sw_patient, "sw").await;

    let approved = approved_campaign(&app, &admin, "PATIENTS").await;
    let id = approved["id"].as_str().unwrap().to_string();
    post(
        &app,
        &format!("/api/v1/admin/campaigns/{id}/schedule"),
        Some(&admin),
        &serde_json::json!({"version": 3, "scheduled_for": Utc::now().to_rfc3339()}),
    )
    .await;

    notification_scheduler::run_campaign_dispatch_pass(&state)
        .await
        .unwrap();

    // Both the seeded (English) patient and the Kiswahili patient get a
    // campaign notification, each in their own preferred language.
    let campaign_id = Uuid::parse_str(&id).unwrap();
    let (en_title, en_body): (String, String) = sqlx::query_as(
        "SELECT title, body FROM notifications WHERE related_entity_id=$1 AND user_id=$2",
    )
    .bind(campaign_id)
    .bind(uid(PATIENT_USER_ID))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(en_title, "New clinic hours");
    assert_eq!(en_body, "Our clinic now opens earlier on weekdays.");

    let (sw_title, sw_body): (String, String) = sqlx::query_as(
        "SELECT title, body FROM notifications WHERE related_entity_id=$1 AND user_id=$2",
    )
    .bind(campaign_id)
    .bind(sw_patient)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(sw_title, "Masaa mapya ya kliniki");
    assert_eq!(
        sw_body,
        "Kliniki yetu sasa inafunguliwa mapema siku za wiki."
    );

    // Dedup key is stable and campaign-scoped.
    let (dedup_key,): (String,) = sqlx::query_as(
        "SELECT dedup_key FROM notifications WHERE related_entity_id=$1 AND user_id=$2",
    )
    .bind(campaign_id)
    .bind(uid(PATIENT_USER_ID))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(dedup_key, format!("campaign:{id}:{}", uid(PATIENT_USER_ID)));

    // Fan-out completion is distinct from FCM delivery: the campaign is
    // COMPLETED purely because every eligible recipient now has a row, even
    // though none of them has been handed to a provider yet (no transport
    // configured in this test, so `run_push_delivery_pass` was never called).
    let (status, completed_at): (String, Option<DateTime<Utc>>) =
        sqlx::query_as("SELECT status::text, completed_at FROM campaigns WHERE id=$1")
            .bind(campaign_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "COMPLETED");
    assert!(completed_at.is_some());
    let (queued_count,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM notifications WHERE related_entity_id=$1 AND delivery_status='CREATED'",
    )
    .bind(campaign_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(queued_count, 2);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_campaign_dispatch_is_safe_under_repeated_and_concurrent_invocation(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");

    let approved = approved_campaign(&app, &admin, "PATIENTS").await;
    let id = approved["id"].as_str().unwrap().to_string();
    post(
        &app,
        &format!("/api/v1/admin/campaigns/{id}/schedule"),
        Some(&admin),
        &serde_json::json!({"version": 3, "scheduled_for": Utc::now().to_rfc3339()}),
    )
    .await;

    // Two "workers" racing on the same tick: `SELECT ... FOR UPDATE SKIP
    // LOCKED` on the campaign row means only one actually processes it, and
    // the per-recipient `dedup_key` unique index means even a hypothetical
    // double-process could never create two rows for the same recipient.
    let (a, b) = tokio::join!(
        notification_scheduler::run_campaign_dispatch_pass(&state),
        notification_scheduler::run_campaign_dispatch_pass(&state)
    );
    a.unwrap();
    b.unwrap();

    // A simulated worker restart: calling the pass again after completion
    // must not create a second notification for the already-processed
    // recipient.
    notification_scheduler::run_campaign_dispatch_pass(&state)
        .await
        .unwrap();

    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM notifications WHERE related_entity_id=$1 AND user_id=$2",
    )
    .bind(Uuid::parse_str(&id).unwrap())
    .bind(uid(PATIENT_USER_ID))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        count, 1,
        "exactly one notification per recipient, never duplicated"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_campaign_fanout_resumes_from_cursor_without_skipping_or_reprocessing(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");

    let p1 = Uuid::new_v4();
    let p2 = Uuid::new_v4();
    new_patient_with_language_and_id(&pool, p1, "en").await;
    new_patient_with_language_and_id(&pool, p2, "en").await;
    let mut all_ids = [uid(PATIENT_USER_ID), p1, p2];
    all_ids.sort();
    let already_processed_cursor = all_ids[0];
    let expected_remaining: std::collections::BTreeSet<Uuid> =
        all_ids[1..].iter().copied().collect();

    let approved = approved_campaign(&app, &admin, "PATIENTS").await;
    let id = approved["id"].as_str().unwrap().to_string();
    let campaign_id = Uuid::parse_str(&id).unwrap();
    post(
        &app,
        &format!("/api/v1/admin/campaigns/{id}/schedule"),
        Some(&admin),
        &serde_json::json!({"version": 3, "scheduled_for": Utc::now().to_rfc3339()}),
    )
    .await;

    // Simulate "a prior tick already processed everything up to and
    // including `already_processed_cursor`" by promoting straight to
    // SENDING with that cursor already set, bypassing the scheduler's own
    // promotion step (which would start the cursor at NULL).
    sqlx::query(
        "UPDATE campaigns SET status='SENDING',sending_started_at=NOW(),fanout_cursor=$2,version=version+1,updated_at=NOW() \
         WHERE id=$1 AND status='SCHEDULED'",
    )
    .bind(campaign_id)
    .bind(already_processed_cursor)
    .execute(&pool)
    .await
    .unwrap();

    notification_scheduler::run_campaign_dispatch_pass(&state)
        .await
        .unwrap();

    let created: std::collections::BTreeSet<Uuid> =
        sqlx::query_scalar("SELECT user_id FROM notifications WHERE related_entity_id=$1")
            .bind(campaign_id)
            .fetch_all(&pool)
            .await
            .unwrap()
            .into_iter()
            .collect();
    assert_eq!(
        created, expected_remaining,
        "only recipients strictly after the cursor are processed — the cursor owner is never reprocessed, and nobody after it is skipped"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_campaign_pause_prevents_dispatch_and_resume_continues(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");

    let approved = approved_campaign(&app, &admin, "PATIENTS").await;
    let id = approved["id"].as_str().unwrap().to_string();
    let campaign_id = Uuid::parse_str(&id).unwrap();
    post(
        &app,
        &format!("/api/v1/admin/campaigns/{id}/schedule"),
        Some(&admin),
        &serde_json::json!({"version": 3, "scheduled_for": Utc::now().to_rfc3339()}),
    )
    .await;
    let paused = json_body(
        post(
            &app,
            &format!("/api/v1/admin/campaigns/{id}/pause"),
            Some(&admin),
            &serde_json::json!({"version": 4}),
        )
        .await,
    )
    .await["data"]
        .clone();
    assert_eq!(paused["status"], "PAUSED");

    // While paused, the dispatch pass must create nothing for this campaign
    // at all — not even promote it.
    notification_scheduler::run_campaign_dispatch_pass(&state)
        .await
        .unwrap();
    let count_while_paused: i64 =
        sqlx::query_scalar("SELECT count(*) FROM notifications WHERE related_entity_id=$1")
            .bind(campaign_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count_while_paused, 0);

    // Resuming a campaign that never started sending goes back to SCHEDULED,
    // so the next dispatch pass promotes and completes it normally.
    let resumed = json_body(
        post(
            &app,
            &format!("/api/v1/admin/campaigns/{id}/resume"),
            Some(&admin),
            &serde_json::json!({"version": 5}),
        )
        .await,
    )
    .await["data"]
        .clone();
    assert_eq!(resumed["status"], "SCHEDULED");

    notification_scheduler::run_campaign_dispatch_pass(&state)
        .await
        .unwrap();
    let (final_status,): (String,) =
        sqlx::query_as("SELECT status::text FROM campaigns WHERE id=$1")
            .bind(campaign_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(final_status, "COMPLETED");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_campaign_cancel_mid_sending_deletes_unsent_but_preserves_provider_accepted(
    pool: PgPool,
) {
    let (app, pool, _state) = test_app_and_state(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");

    let approved = approved_campaign(&app, &admin, "PATIENTS").await;
    let id = approved["id"].as_str().unwrap().to_string();
    let campaign_id = Uuid::parse_str(&id).unwrap();
    post(
        &app,
        &format!("/api/v1/admin/campaigns/{id}/schedule"),
        Some(&admin),
        &serde_json::json!({"version": 3, "scheduled_for": Utc::now().to_rfc3339()}),
    )
    .await;
    sqlx::query(
        "UPDATE campaigns SET status='SENDING',sending_started_at=NOW(),version=version+1,updated_at=NOW() WHERE id=$1",
    )
    .bind(campaign_id)
    .execute(&pool)
    .await
    .unwrap();

    // One recipient already handed to the FCM provider (must survive
    // cancellation untouched — an accepted FCM message cannot be recalled),
    // one still sitting unsent in the outbox (must be deleted on cancel).
    let sent_id = Uuid::new_v4();
    let unsent_id = Uuid::new_v4();
    notification_repo::insert_if_new(
        &pool,
        sent_id,
        UserId::from(uid(PATIENT_USER_ID)),
        "ADMIN_CAMPAIGN",
        "Sent already",
        "Sent already",
        Some("CAMPAIGN"),
        Some(campaign_id),
        None,
        &format!("campaign:{id}:already-sent"),
        None,
    )
    .await
    .unwrap();
    sqlx::query("UPDATE notifications SET delivery_status='SENT_TO_PROVIDER' WHERE id=$1")
        .bind(sent_id)
        .execute(&pool)
        .await
        .unwrap();
    notification_repo::insert_if_new(
        &pool,
        unsent_id,
        UserId::from(uid(DOCTOR1_USER_ID)),
        "ADMIN_CAMPAIGN",
        "Not sent yet",
        "Not sent yet",
        Some("CAMPAIGN"),
        Some(campaign_id),
        None,
        &format!("campaign:{id}:not-sent"),
        None,
    )
    .await
    .unwrap();

    let cancelled = json_body(
        post(
            &app,
            &format!("/api/v1/admin/campaigns/{id}/cancel"),
            Some(&admin),
            // version 5: create=1, submit=2, approve=3, schedule=4, then the
            // manual SCHEDULED->SENDING transition above bumps it to 5.
            &serde_json::json!({"version": 5}),
        )
        .await,
    )
    .await["data"]
        .clone();
    assert_eq!(cancelled["status"], "CANCELLED");

    let remaining: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM notifications WHERE related_entity_id=$1")
            .bind(campaign_id)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        remaining,
        vec![sent_id],
        "only the already-provider-accepted row survives cancellation"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_campaign_stats_endpoint_buckets_correctly_and_never_claims_delivery(pool: PgPool) {
    let (app, pool, _state) = test_app_and_state(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");

    let approved = approved_campaign(&app, &admin, "PATIENTS").await;
    let id = approved["id"].as_str().unwrap().to_string();
    let campaign_id = Uuid::parse_str(&id).unwrap();

    // One of each delivery outcome, inserted directly to isolate the stats
    // query from the dispatch pass itself (already covered above).
    for (n, user, status, error, next_delivery) in [
        (0, PATIENT_USER_ID, "CREATED", None, None),
        (1, DOCTOR1_USER_ID, "SENT_TO_PROVIDER", None, None),
        (
            2,
            DOCTOR2_USER_ID,
            "FAILED",
            Some("Suppressed: engagement notifications disabled by user preference"),
            None,
        ),
        (
            3,
            PATIENT_USER_ID,
            "FAILED",
            Some("Deferred: recipient quiet hours in effect"),
            Some(Utc::now() + chrono::Duration::hours(1)),
        ),
        (
            4,
            DOCTOR1_USER_ID,
            "FAILED",
            Some("FCM provider delivery was temporarily unavailable"),
            Some(Utc::now() + chrono::Duration::minutes(1)),
        ),
        (
            5,
            DOCTOR2_USER_ID,
            "FAILED",
            Some("FCM delivery retry budget exhausted"),
            None,
        ),
    ] {
        let nid = Uuid::new_v4();
        notification_repo::insert_if_new(
            &pool,
            nid,
            UserId::from(uid(user)),
            "ADMIN_CAMPAIGN",
            "Stats test",
            "Stats test",
            Some("CAMPAIGN"),
            Some(campaign_id),
            None,
            &format!("campaign:{id}:stats-{n}"),
            None,
        )
        .await
        .unwrap();
        sqlx::query(
            "UPDATE notifications SET delivery_status=$2,last_delivery_error=$3,next_delivery_at=$4 WHERE id=$1",
        )
        .bind(nid)
        .bind(status)
        .bind(error)
        .bind(next_delivery)
        .execute(&pool)
        .await
        .unwrap();
    }

    let stats = json_body(
        get(
            &app,
            &format!("/api/v1/admin/campaigns/{id}/stats"),
            Some(&admin),
        )
        .await,
    )
    .await;
    let data = &stats["data"];
    assert_eq!(data["queued"], 1);
    assert_eq!(data["sent_to_provider"], 1);
    assert_eq!(data["deferred_by_preference_or_quiet_hours"], 2);
    assert_eq!(data["retry_pending"], 1);
    assert_eq!(data["permanently_failed"], 1);
    assert_eq!(data["audience_count"], 1);

    // Never relabel "handed to FCM" as actually delivered or read.
    let raw = stats.to_string();
    assert!(!raw.to_lowercase().contains("\"delivered\""));
    assert!(!raw.to_lowercase().contains("\"read\""));
}

#[sqlx::test(migrations = "./migrations")]
async fn test_campaign_delivery_respects_engagement_optout_and_quiet_hours(pool: PgPool) {
    use async_trait::async_trait;
    use hellodoctor_backend::services::notification_transport::{
        NotificationTransport, PushOptions, PushOutcome,
    };

    struct AlwaysSent;
    #[async_trait]
    impl NotificationTransport for AlwaysSent {
        fn is_configured(&self) -> bool {
            true
        }
        async fn send_push(
            &self,
            _push_token: &str,
            _title: &str,
            _body: &str,
            _options: &PushOptions,
        ) -> PushOutcome {
            PushOutcome::SentToProvider {
                provider_message_id: Some("test-provider-id".into()),
            }
        }
    }

    let (app, pool, mut state) = test_app_and_state(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    state.notification_transport = std::sync::Arc::new(AlwaysSent);

    // Doctor 1 opts out of engagement notifications entirely.
    sqlx::query(
        "INSERT INTO notification_preferences(user_id,operational_enabled,engagement_enabled,quiet_hours_enabled,quiet_hours_start_local,quiet_hours_end_local,greeting_notifications_enabled,religious_greetings_enabled) \
         VALUES($1,true,false,false,'22:00','07:00',true,false)",
    )
    .bind(uid(DOCTOR1_USER_ID))
    .execute(&pool)
    .await
    .unwrap();
    // Doctor 2 is always in quiet hours (nearly the full day), so delivery
    // must be deferred rather than suppressed or sent.
    sqlx::query(
        "INSERT INTO notification_preferences(user_id,operational_enabled,engagement_enabled,quiet_hours_enabled,quiet_hours_start_local,quiet_hours_end_local,greeting_notifications_enabled,religious_greetings_enabled) \
         VALUES($1,true,true,true,'00:00','23:59',true,false)",
    )
    .bind(uid(DOCTOR2_USER_ID))
    .execute(&pool)
    .await
    .unwrap();
    // A device token is required for the normal (Patient) recipient to reach
    // `SENT_TO_PROVIDER` at all.
    post(
        &app,
        "/api/v1/notifications/devices",
        Some(&make_token(uid(PATIENT_USER_ID), "PATIENT")),
        &serde_json::json!({"platform": "android", "push_token": "campaign-optout-test-token"}),
    )
    .await;

    let approved = approved_campaign(&app, &admin, "BOTH").await;
    let id = approved["id"].as_str().unwrap().to_string();
    post(
        &app,
        &format!("/api/v1/admin/campaigns/{id}/schedule"),
        Some(&admin),
        &serde_json::json!({"version": 3, "scheduled_for": Utc::now().to_rfc3339()}),
    )
    .await;

    notification_scheduler::run_campaign_dispatch_pass(&state)
        .await
        .unwrap();
    notification_scheduler::run_push_delivery_pass(&state)
        .await
        .unwrap();

    let campaign_id = Uuid::parse_str(&id).unwrap();
    let (optout_status, optout_error): (String, Option<String>) = sqlx::query_as(
        "SELECT delivery_status,last_delivery_error FROM notifications WHERE related_entity_id=$1 AND user_id=$2",
    )
    .bind(campaign_id)
    .bind(uid(DOCTOR1_USER_ID))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(optout_status, "SUPPRESSED");
    assert!(optout_error.unwrap().starts_with("Suppressed:"));

    let (quiet_status, quiet_next): (String, Option<DateTime<Utc>>) = sqlx::query_as(
        "SELECT delivery_status,next_delivery_at FROM notifications WHERE related_entity_id=$1 AND user_id=$2",
    )
    .bind(campaign_id)
    .bind(uid(DOCTOR2_USER_ID))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(quiet_status, "FAILED");
    assert!(
        quiet_next.is_some(),
        "a deferred row must keep a future retry time, never a terminal failure"
    );

    let (normal_status,): (String,) = sqlx::query_as(
        "SELECT delivery_status FROM notifications WHERE related_entity_id=$1 AND user_id=$2",
    )
    .bind(campaign_id)
    .bind(uid(PATIENT_USER_ID))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(normal_status, "SENT_TO_PROVIDER");
}
