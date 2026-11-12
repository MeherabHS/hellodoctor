//! Join-window enforcement through the real HTTP stack: the RTC-token mint,
//! the read-only `GET /consultations/:id/join-eligibility`, and the `join`
//! object embedded in the Patient and Doctor appointment lists must all come
//! from the same authoritative policy (`domain::join_window`).
//!
//! Boundary arithmetic (exactly 15 min before, exactly end+60) is covered by
//! the pure unit tests in `src/domain/join_window.rs`; the real clock cannot
//! be pinned here, so these tests use offsets comfortably either side of a
//! boundary (16 vs 14 minutes before start; 30 vs 90 minutes after end).
use super::*;

fn deferred_config() -> AppConfig {
    AppConfig {
        financial_module_mode: hellodoctor_backend::config::FinancialModuleMode::Deferred,
        // The shared test_config() widens the window so far-future seeded slots
        // can mint; these tests assert the real default policy instead.
        agora_token_window_before_mins: 15,
        agora_token_window_after_mins: 60,
        ..test_config()
    }
}

struct Booked {
    apt: Uuid,
    slot: Uuid,
    patient: String,
    doctor: String,
}

async fn book(app: &axum::Router, pool: &PgPool) -> Booked {
    let patient = fresh_patient_token(pool).await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let res = get(
        app,
        &format!("/api/v1/doctors/{DOCTOR1_PROFILE_ID}/slots"),
        Some(&patient),
    )
    .await;
    let slot = Uuid::parse_str(json_body(res).await["data"][0]["id"].as_str().unwrap()).unwrap();
    let res = post(
        app,
        "/api/v1/appointments/book",
        Some(&patient),
        &serde_json::json!({"doctor_id": DOCTOR1_PROFILE_ID, "slot_id": slot, "modality": "VIDEO"}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let apt = Uuid::parse_str(
        json_body(res).await["data"]["appointment_id"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    Booked {
        apt,
        slot,
        patient,
        doctor,
    }
}

/// Moves the booked slot so that "now" sits `start_offset_min` minutes from its
/// start (negative = start in the past), with a 10-minute duration.
async fn move_slot(pool: &PgPool, slot: Uuid, start_offset_min: i64) {
    sqlx::query(
        "UPDATE doctor_schedule_slots SET start_time = now() + make_interval(mins => $2::int), \
         end_time = now() + make_interval(mins => $2::int + 10) WHERE id = $1",
    )
    .bind(slot)
    .bind(start_offset_min as i32)
    .execute(pool)
    .await
    .unwrap();
}

async fn mint(app: &axum::Router, b: &Booked, token: &str) -> (StatusCode, serde_json::Value) {
    let res = post(
        app,
        &format!("/api/v1/consultations/{}/rtc-token", b.apt),
        Some(token),
        &serde_json::json!({}),
    )
    .await;
    (res.status(), json_body(res).await)
}

async fn eligibility(app: &axum::Router, b: &Booked, token: &str) -> serde_json::Value {
    let res = get(
        app,
        &format!("/api/v1/consultations/{}/join-eligibility", b.apt),
        Some(token),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    json_body(res).await["data"].clone()
}

#[sqlx::test(migrations = "./migrations")]
async fn test_too_early_is_denied_with_stable_code_and_issues_no_session(pool: PgPool) {
    let (app, pool) = test_app_with_config(pool, deferred_config()).await;
    let b = book(&app, &pool).await;
    move_slot(&pool, b.slot, 16).await; // 16 minutes before start: closed

    for token in [&b.patient, &b.doctor] {
        let (status, body) = mint(&app, &b, token).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["error"]["code"], "NOT_YET_OPEN");
        let e = eligibility(&app, &b, token).await;
        assert_eq!(e["eligible"], false);
        assert_eq!(e["reason"], "NOT_YET_OPEN");
        assert!(e["join_open_at"].is_string() && e["join_close_at"].is_string());
        assert!(e["server_time"].is_string());
    }
    let sessions: i64 =
        sqlx::query_scalar("SELECT count(*) FROM consultation_sessions WHERE appointment_id = $1")
            .bind(b.apt)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        sessions, 0,
        "an ineligible attempt must not create a consultation session"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_inside_window_both_roles_are_eligible_and_mint_matches(pool: PgPool) {
    let (app, pool) = test_app_with_config(pool, deferred_config()).await;
    let b = book(&app, &pool).await;
    move_slot(&pool, b.slot, 14).await; // 14 minutes before start: open
    let e = eligibility(&app, &b, &b.doctor).await;
    assert_eq!(e["eligible"], true);
    assert_eq!(e["reason"], "OK");
    let (status, _) = mint(&app, &b, &b.doctor).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = mint(&app, &b, &b.patient).await;
    assert_eq!(status, StatusCode::OK);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_post_end_grace_is_permitted_then_window_closes(pool: PgPool) {
    let (app, pool) = test_app_with_config(pool, deferred_config()).await;
    let b = book(&app, &pool).await;
    move_slot(&pool, b.slot, -40).await; // ended 30 minutes ago: inside the 60-minute grace
    assert_eq!(eligibility(&app, &b, &b.doctor).await["eligible"], true);
    let (status, _) = mint(&app, &b, &b.doctor).await;
    assert_eq!(status, StatusCode::OK);

    move_slot(&pool, b.slot, -100).await; // ended 90 minutes ago: closed
    let e = eligibility(&app, &b, &b.patient).await;
    assert_eq!(e["reason"], "WINDOW_CLOSED");
    let (status, body) = mint(&app, &b, &b.patient).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], "WINDOW_CLOSED");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_completed_and_cancelled_appointments_are_invalid_state(pool: PgPool) {
    let (app, pool) = test_app_with_config(pool, deferred_config()).await;
    for (i, (status_value, label)) in [("COMPLETED", "completed"), ("CANCELLED", "cancelled")]
        .into_iter()
        .enumerate()
    {
        let b = book(&app, &pool).await;
        move_slot(&pool, b.slot, 5 + 30 * i as i64).await; // inside the window, apart from each other
        sqlx::query("UPDATE appointments SET status = $2::appointment_status_enum WHERE id = $1")
            .bind(b.apt)
            .bind(status_value)
            .execute(&pool)
            .await
            .unwrap();
        let e = eligibility(&app, &b, &b.doctor).await;
        assert_eq!(e["eligible"], false, "{label}");
        assert_eq!(e["reason"], "INVALID_STATE", "{label}");
        let (status, body) = mint(&app, &b, &b.doctor).await;
        assert_eq!(status, StatusCode::CONFLICT, "{label}");
        assert_eq!(body["error"]["code"], "INVALID_STATE", "{label}");
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn test_interrupted_session_needing_review_is_not_rejoinable(pool: PgPool) {
    let (app, pool) = test_app_with_config(pool, deferred_config()).await;
    let b = book(&app, &pool).await;
    move_slot(&pool, b.slot, 5).await;
    let (status, _) = mint(&app, &b, &b.doctor).await; // creates the session
    assert_eq!(status, StatusCode::OK);
    sqlx::query("UPDATE consultation_sessions SET recovery_state = 'NEEDS_REVIEW' WHERE appointment_id = $1")
        .bind(b.apt)
        .execute(&pool)
        .await
        .unwrap();
    let e = eligibility(&app, &b, &b.doctor).await;
    assert_eq!(e["eligible"], false);
    assert_eq!(e["reason"], "NEEDS_REVIEW");
    let (status, body) = mint(&app, &b, &b.doctor).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "NEEDS_REVIEW");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_non_participants_cannot_read_eligibility_or_mint(pool: PgPool) {
    let (app, pool) = test_app_with_config(pool, deferred_config()).await;
    let b = book(&app, &pool).await;
    move_slot(&pool, b.slot, 5).await;
    let stranger = fresh_patient_token(&pool).await;
    let other_doctor = make_token(uid(DOCTOR2_USER_ID), "DOCTOR");
    for token in [&stranger, &other_doctor] {
        let res = get(
            &app,
            &format!("/api/v1/consultations/{}/join-eligibility", b.apt),
            Some(token),
        )
        .await;
        assert!(
            matches!(res.status(), StatusCode::FORBIDDEN | StatusCode::NOT_FOUND),
            "eligibility must not leak to a non-participant: {}",
            res.status()
        );
        let (status, _) = mint(&app, &b, token).await;
        assert!(matches!(
            status,
            StatusCode::FORBIDDEN | StatusCode::NOT_FOUND
        ));
    }
    let res = get(
        &app,
        &format!("/api/v1/consultations/{}/join-eligibility", b.apt),
        None,
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_appointment_lists_carry_the_same_server_derived_join_state(pool: PgPool) {
    let (app, pool) = test_app_with_config(pool, deferred_config()).await;
    let b = book(&app, &pool).await;
    move_slot(&pool, b.slot, 16).await;

    let patient_list = json_body(
        get(
            &app,
            "/api/v1/patients/me/appointments?status=upcoming",
            Some(&b.patient),
        )
        .await,
    )
    .await;
    let row = patient_list["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == b.apt.to_string())
        .expect("patient list row");
    assert_eq!(row["join"]["eligible"], false);
    assert_eq!(row["join"]["reason"], "NOT_YET_OPEN");

    let doctor_list = json_body(
        get(
            &app,
            "/api/v1/doctor/appointments?view=upcoming",
            Some(&b.doctor),
        )
        .await,
    )
    .await;
    let row = doctor_list["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == b.apt.to_string())
        .expect("doctor list row");
    assert_eq!(row["join"]["reason"], "NOT_YET_OPEN");

    move_slot(&pool, b.slot, 5).await;
    let doctor_list = json_body(
        get(
            &app,
            "/api/v1/doctor/appointments?view=upcoming",
            Some(&b.doctor),
        )
        .await,
    )
    .await;
    let row = doctor_list["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == b.apt.to_string())
        .unwrap();
    assert_eq!(row["join"]["eligible"], true);
}
