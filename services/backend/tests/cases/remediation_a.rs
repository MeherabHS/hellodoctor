//! Cross-workflow regressions for financial disposition and durable erasure.
use super::*;
use hellodoctor_backend::services::notification_scheduler::run_hold_expiry_sweep;

#[sqlx::test(migrations = "./migrations")]
async fn a1_unpaid_expiry_allows_accounting_close(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let (appointment, slot) = book_pending_payment(&app, &patient, None).await;
    assert_finance_volume(&app, &admin, "0").await;
    sqlx::query(
        "UPDATE transactions SET created_at='2026-09-12T10:00:00Z' WHERE appointment_id=$1",
    )
    .bind(appointment)
    .execute(&pool)
    .await
    .unwrap();
    force_hold_due(&pool, appointment).await;
    assert_eq!(run_hold_expiry_sweep(&state).await.unwrap(), 1);
    let payment: String =
        sqlx::query_scalar("SELECT payment_status::text FROM transactions WHERE appointment_id=$1")
            .bind(appointment)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        payment, "FAILED",
        "a proven unsubmitted checkout must have a terminal unpaid disposition"
    );
    let available: String =
        sqlx::query_scalar("SELECT status::text FROM doctor_schedule_slots WHERE id=$1")
            .bind(slot)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(available, "AVAILABLE");
    assert_eq!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM audit_events WHERE action='UNPAID_CHECKOUT_EXPIRED' AND resource_id IN(SELECT id FROM transactions WHERE appointment_id=$1)").bind(appointment).fetch_one(&pool).await.unwrap(), 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM financial_economic_effects WHERE business_id=$1"
        )
        .bind(appointment)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0,
        "an abandoned checkout must not fabricate a payment, settlement or refund effect"
    );
    assert_finance_volume(&app, &admin, "0").await;
    assert_eq!(
        post(
            &app,
            "/api/v1/admin/finance/periods/finalize",
            Some(&admin),
            &serde_json::json!({"year":2026,"month":9})
        )
        .await
        .status(),
        StatusCode::OK
    );
}

async fn assert_finance_volume(app: &axum::Router, admin: &str, expected: &str) {
    let listing =
        json_body(get(app, "/api/v1/admin/finance/transactions", Some(admin)).await).await;
    assert_eq!(
        listing["data"].as_array().unwrap().len(),
        1,
        "unfunded and refunded history must remain inspectable"
    );
    let overview = json_body(get(app, "/api/v1/admin/overview", Some(admin)).await).await;
    let summary = json_body(get(app, "/api/v1/admin/finance/summary", Some(admin)).await).await;
    for actual in [
        &overview["data"]["gross_volume"],
        &overview["data"]["modality_split"]["videoRevenue"],
        &summary["data"]["total_gross"],
    ] {
        let actual: bigdecimal::BigDecimal = actual.as_str().unwrap().parse().unwrap();
        assert_eq!(actual, expected.parse::<bigdecimal::BigDecimal>().unwrap());
    }
    if expected == "0" {
        for actual in [
            &overview["data"]["platform_fees"],
            &summary["data"]["total_platform_fee"],
            &summary["data"]["total_net"],
            &summary["data"]["gateway_volume"]["card"],
            &summary["data"]["gateway_volume"]["mpesa"],
        ] {
            assert_eq!(
                actual
                    .as_str()
                    .unwrap()
                    .parse::<bigdecimal::BigDecimal>()
                    .unwrap(),
                bigdecimal::BigDecimal::from(0)
            );
        }
    }
}

async fn approved_refund(app: &axum::Router, pool: &PgPool) -> (Uuid, Uuid, Uuid) {
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let (appointment, _) = book_and_confirm(app, &patient).await;
    let session = consultation_repo::find_by_appointment(pool, appointment)
        .await
        .unwrap()
        .unwrap();
    let response = post(app, "/api/v1/grievances", Some(&patient), &serde_json::json!({
        "consultation_id":session.id,"target":"DOCTOR","category":"Rushed Consultation / Ended Abruptly",
        "claim_summary":"Synthetic fixture: interrupted consultation"})).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let grievance = uid(json_body(response).await["data"]["grievance_id"]
        .as_str()
        .unwrap());
    assert_eq!(
        post(
            app,
            &format!("/api/v1/admin/grievances/{grievance}/refund"),
            Some(&admin),
            &serde_json::json!({"audit_notes":"Synthetic reviewed refund evidence"})
        )
        .await
        .status(),
        StatusCode::OK
    );
    let refund: Uuid =
        sqlx::query_scalar("SELECT id FROM financial_refund_requests WHERE grievance_id=$1")
            .bind(grievance)
            .fetch_one(pool)
            .await
            .unwrap();
    (appointment, grievance, refund)
}

async fn force_hold_due(pool: &PgPool, appointment: Uuid) {
    // Clock advancement only in this SQLx-owned disposable database. Restore
    // the immutable-deadline trigger before exercising the real expiry code.
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("ALTER TABLE appointments DISABLE TRIGGER appointments_hold_deadline_immutable")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE appointments SET hold_expires_at=clock_timestamp()-interval '1 minute' WHERE id=$1",
    )
    .bind(appointment)
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query("ALTER TABLE appointments ENABLE TRIGGER appointments_hold_deadline_immutable")
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn a2_refund_completion_updates_all_authoritative_records(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let (appointment, grievance, refund) = approved_refund(&app, &pool).await;
    // Approval alone does not remove funds from reporting.
    assert_finance_volume(&app, &admin, "800").await;
    let wallet_before: serde_json::Value =
        sqlx::query_scalar("SELECT to_jsonb(w) FROM doctor_wallets w WHERE doctor_id=$1")
            .bind(uid(DOCTOR1_PROFILE_ID))
            .fetch_one(&pool)
            .await
            .unwrap();
    let response = post(
        &app,
        &format!("/api/v1/admin/finance/refund-requests/{refund}/transition"),
        Some(&admin),
        &serde_json::json!({"status":"REFUNDED","provider_reference":"SYNTHETIC-CONFIRMED-REFUND"}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let payment: String =
        sqlx::query_scalar("SELECT payment_status::text FROM transactions WHERE appointment_id=$1")
            .bind(appointment)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(payment, "REFUNDED_TO_PATIENT");
    let status: String =
        sqlx::query_scalar("SELECT status::text FROM grievance_reports WHERE id=$1")
            .bind(grievance)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "REFUNDED");
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM financial_refund_requests WHERE id=$1")
            .bind(refund)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "REFUNDED"
    );
    assert_eq!(
        sqlx::query_scalar::<_, serde_json::Value>(
            "SELECT to_jsonb(w) FROM doctor_wallets w WHERE doctor_id=$1"
        )
        .bind(uid(DOCTOR1_PROFILE_ID))
        .fetch_one(&pool)
        .await
        .unwrap(),
        wallet_before,
        "completion must not repeat the approval's wallet reversal"
    );
    assert_eq!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM financial_economic_effects WHERE business_id=$1 AND effect_type='REFUND_APPROVED'").bind(appointment).fetch_one(&pool).await.unwrap(), 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM notifications WHERE dedup_key=$1 AND type='REFUND_COMPLETED'"
        )
        .bind(format!("refund-completed:{refund}"))
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    assert_finance_volume(&app, &admin, "0").await;
}

#[sqlx::test(migrations = "./migrations")]
async fn a3_failed_login_deactivation_is_not_completed_erasure(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    sqlx::query("CREATE FUNCTION fixture_reject_erasure() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.is_active=false AND OLD.is_active THEN RAISE EXCEPTION 'synthetic deactivation failure'; END IF; RETURN NEW; END $$")
        .execute(&pool).await.unwrap();
    sqlx::query("CREATE TRIGGER fixture_reject_erasure BEFORE UPDATE ON users FOR EACH ROW EXECUTE FUNCTION fixture_reject_erasure()")
        .execute(&pool).await.unwrap();
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let response = post(
        &app,
        &format!("/api/v1/admin/patients/{PATIENT_PROFILE_ID}/data-rights/erase"),
        Some(&admin),
        &serde_json::json!({"reason":"Synthetic authorized erasure request"}),
    )
    .await;
    assert_eq!(
        response.status(),
        StatusCode::ACCEPTED,
        "failed deactivation must report pending, never completed erasure"
    );
    let first = json_body(response).await;
    assert_eq!(first["data"]["outcome"], "PENDING");
    let job = uid(first["data"]["job_id"].as_str().unwrap());
    assert!(
        sqlx::query_scalar::<_, bool>("SELECT is_active FROM users WHERE id=$1")
            .bind(uid(PATIENT_USER_ID))
            .fetch_one(&pool)
            .await
            .unwrap()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM patient_profiles WHERE id=$1")
            .bind(uid(PATIENT_PROFILE_ID))
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT state FROM account_erasure_jobs WHERE id=$1")
            .bind(job)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "PENDING_LOGIN"
    );
    let owner_ctx = hellodoctor_backend::auth_context::AuthContext {
        user_id: uid(PATIENT_USER_ID).into(),
        role: UserRole::Patient,
        permissions: vec![],
        session_id: None,
        request_id: None,
    };
    let mut owner_tx = state.begin_authed(&owner_ctx).await.unwrap();
    assert_eq!(
        sqlx::query(
            "UPDATE patient_profiles SET full_name='synthetic stale in-flight update' WHERE id=$1"
        )
        .bind(uid(PATIENT_PROFILE_ID))
        .execute(&mut *owner_tx)
        .await
        .unwrap()
        .rows_affected(),
        0
    );
    owner_tx.rollback().await.unwrap();
    sqlx::query("DROP TRIGGER fixture_reject_erasure ON users")
        .execute(&pool)
        .await
        .unwrap();
    force_erasure_due(&pool, job).await;
    hellodoctor_backend::services::data_rights_service::retry_pending_erasures(&state)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT state FROM account_erasure_jobs WHERE id=$1")
            .bind(job)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "COMPLETED"
    );
    assert!(
        !sqlx::query_scalar::<_, bool>("SELECT is_active FROM users WHERE id=$1")
            .bind(uid(PATIENT_USER_ID))
            .fetch_one(&pool)
            .await
            .unwrap()
    );
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    assert_eq!(
        get(&app, "/api/v1/patients/me", Some(&patient))
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let replay = post(
        &app,
        &format!("/api/v1/admin/patients/{PATIENT_PROFILE_ID}/data-rights/erase"),
        Some(&admin),
        &serde_json::json!({"reason":"Repeat synthetic erasure request"}),
    )
    .await;
    assert_eq!(replay.status(), StatusCode::OK);
    assert_eq!(json_body(replay).await["data"]["job_id"], job.to_string());
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM audit_events WHERE action='DATA_RIGHTS_PATIENT_ERASED'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
}

async fn force_erasure_due(pool: &PgPool, id: Uuid) {
    sqlx::query("UPDATE account_erasure_jobs SET next_attempt_at=clock_timestamp()-interval '1 second',lease_until=NULL WHERE id=$1")
        .bind(id).execute(pool).await.unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn a1_unknown_and_provider_started_attempts_keep_close_blocked(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    for provenance in ["UNKNOWN", "STARTED"] {
        let (appointment, _) = book_pending_payment(&app, &patient, None).await;
        sqlx::query("UPDATE transactions SET provider_request_state=$2,created_at='2026-09-12T10:00:00Z' WHERE appointment_id=$1")
            .bind(appointment).bind(provenance).execute(&pool).await.unwrap();
        force_hold_due(&pool, appointment).await;
        run_hold_expiry_sweep(&state).await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, String>(
                "SELECT payment_status::text FROM transactions WHERE appointment_id=$1"
            )
            .bind(appointment)
            .fetch_one(&pool)
            .await
            .unwrap(),
            "INITIATED"
        );
    }
    assert_eq!(
        post(
            &app,
            "/api/v1/admin/finance/periods/finalize",
            Some(&admin),
            &serde_json::json!({"year":2026,"month":9})
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn a1_late_and_duplicate_callbacks_preserve_captured_funds_after_close(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let (appointment, slot) = book_pending_payment(&app, &patient, None).await;
    sqlx::query(
        "UPDATE transactions SET created_at='2026-09-12T10:00:00Z' WHERE appointment_id=$1",
    )
    .bind(appointment)
    .execute(&pool)
    .await
    .unwrap();
    let session: Uuid =
        sqlx::query_scalar("SELECT payment_session_id FROM transactions WHERE appointment_id=$1")
            .bind(appointment)
            .fetch_one(&pool)
            .await
            .unwrap();
    force_hold_due(&pool, appointment).await;
    run_hold_expiry_sweep(&state).await.unwrap();
    assert_eq!(
        post(
            &app,
            "/api/v1/admin/finance/periods/finalize",
            Some(&admin),
            &serde_json::json!({"year":2026,"month":9})
        )
        .await
        .status(),
        StatusCode::OK
    );
    for _ in 0..2 {
        let result = hellodoctor_backend::services::AppointmentService::process_payment_webhook(
            &state,
            session,
            Gateway::Mpesa,
            "SYNTHETIC-LATE-CAPTURE",
            "synthetic-late-event",
        )
        .await
        .unwrap();
        assert_eq!(result.status, AppointmentStatus::Expired);
    }
    let (status,reconcile):(String,bool)=sqlx::query_as("SELECT payment_status::text,reconciliation_required FROM transactions WHERE appointment_id=$1").bind(appointment).fetch_one(&pool).await.unwrap();
    assert_eq!(status, "PAYMENT_HELD");
    assert!(reconcile);
    let visible =
        json_body(get(&app, "/api/v1/admin/finance/transactions", Some(&admin)).await).await;
    assert_eq!(visible["data"][0]["reconciliation_required"], true);
    assert_eq!(visible["data"][0]["payment_status"], "PAYMENT_HELD");
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT status::text FROM doctor_schedule_slots WHERE id=$1"
        )
        .bind(slot)
        .fetch_one(&pool)
        .await
        .unwrap(),
        "AVAILABLE"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM payment_events WHERE provider_event_id='synthetic-late-event'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM consultation_sessions WHERE appointment_id=$1"
        )
        .bind(appointment)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM audit_events WHERE action='LATE_PAYMENT_RECONCILIATION_REQUIRED'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn a1_payment_and_expiry_race_keeps_one_authoritative_obligation(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let (appointment, _) = book_pending_payment(&app, &patient, None).await;
    let session: Uuid =
        sqlx::query_scalar("SELECT payment_session_id FROM transactions WHERE appointment_id=$1")
            .bind(appointment)
            .fetch_one(&pool)
            .await
            .unwrap();
    force_hold_due(&pool, appointment).await;
    let (expiry, payment) = tokio::join!(
        run_hold_expiry_sweep(&state),
        hellodoctor_backend::services::AppointmentService::process_payment_webhook(
            &state,
            session,
            Gateway::Mpesa,
            "SYNTHETIC-RACE-CAPTURE",
            "synthetic-race-event"
        )
    );
    expiry.unwrap();
    payment.unwrap();
    let (status,reconcile):(String,bool)=sqlx::query_as("SELECT payment_status::text,reconciliation_required FROM transactions WHERE appointment_id=$1").bind(appointment).fetch_one(&pool).await.unwrap();
    assert_eq!(status, "PAYMENT_HELD");
    assert!(reconcile);
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status::text FROM appointments WHERE id=$1")
            .bind(appointment)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "EXPIRED"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn a2_completion_failure_rolls_back_every_record_and_outbox(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let (appointment, grievance, refund) = approved_refund(&app, &pool).await;
    sqlx::query("CREATE FUNCTION fixture_reject_refunded() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.status::text='REFUNDED' THEN RAISE EXCEPTION 'synthetic terminal failure'; END IF; RETURN NEW; END $$").execute(&pool).await.unwrap();
    sqlx::query("CREATE TRIGGER fixture_reject_refunded BEFORE UPDATE ON grievance_reports FOR EACH ROW EXECUTE FUNCTION fixture_reject_refunded()").execute(&pool).await.unwrap();
    let response = post(
        &app,
        &format!("/api/v1/admin/finance/refund-requests/{refund}/transition"),
        Some(&admin),
        &serde_json::json!({"status":"REFUNDED","provider_reference":"SYNTHETIC-REFUND-ROLLBACK"}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM financial_refund_requests WHERE id=$1")
            .bind(refund)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "APPROVED"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT payment_status::text FROM transactions WHERE appointment_id=$1"
        )
        .bind(appointment)
        .fetch_one(&pool)
        .await
        .unwrap(),
        "REFUND_PENDING"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status::text FROM grievance_reports WHERE id=$1")
            .bind(grievance)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "REFUND_APPROVED"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM notifications WHERE type='REFUND_COMPLETED'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn a2_concurrent_completion_has_one_terminal_effect(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let (_, _, refund) = approved_refund(&app, &pool).await;
    let path = format!("/api/v1/admin/finance/refund-requests/{refund}/transition");
    let body =
        serde_json::json!({"status":"REFUNDED","provider_reference":"SYNTHETIC-REFUND-RACE"});
    let (a, b) = tokio::join!(
        post(&app, &path, Some(&admin), &body),
        post(&app, &path, Some(&admin), &body)
    );
    let mut codes = [a.status().as_u16(), b.status().as_u16()];
    codes.sort();
    assert_eq!(codes, [200, 409]);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM audit_events WHERE action='REFUND_REQUEST_TRANSITIONED'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn a2_provider_failure_does_not_mark_refund_complete(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let (appointment, grievance, refund) = approved_refund(&app, &pool).await;
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/admin/finance/refund-requests/{refund}/transition"),
            Some(&admin),
            &serde_json::json!({"status":"FAILED"})
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT payment_status::text FROM transactions WHERE appointment_id=$1"
        )
        .bind(appointment)
        .fetch_one(&pool)
        .await
        .unwrap(),
        "REFUND_PENDING"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status::text FROM grievance_reports WHERE id=$1")
            .bind(grievance)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "REFUND_APPROVED"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn a3_media_cleanup_failure_is_pending_and_recoverable(pool: PgPool) {
    let (app, pool, mut state) = test_app_and_state(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let key = format!(
        "avatars/patient/{PATIENT_PROFILE_ID}/{}.bin",
        Uuid::new_v4()
    );
    hellodoctor_backend::services::document_storage_service::DocumentStorageService::put(
        &state,
        &key,
        b"synthetic personal avatar",
    )
    .await
    .unwrap();
    sqlx::query("UPDATE patient_profiles SET profile_picture_object_key=$2,profile_picture_bucket=$3 WHERE id=$1")
        .bind(uid(PATIENT_PROFILE_ID)).bind(&key).bind(&state.config.storage_bucket).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO notification_device_tokens(device_id,user_id,platform,push_token) VALUES($1,$2,'ANDROID','synthetic-erasure-device')")
        .bind(Uuid::new_v4()).bind(uid(PATIENT_USER_ID)).execute(&pool).await.unwrap();
    let store = state.object_storage.clone();
    state.object_storage = None;
    let unavailable = create_router(state.clone());
    let response = post(
        &unavailable,
        &format!("/api/v1/admin/patients/{PATIENT_PROFILE_ID}/data-rights/erase"),
        Some(&admin),
        &serde_json::json!({"reason":"Synthetic avatar erasure request"}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let job = uid(json_body(response).await["data"]["job_id"]
        .as_str()
        .unwrap());
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT state FROM account_erasure_jobs WHERE id=$1")
            .bind(job)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "PENDING_MEDIA"
    );
    assert!(
        !sqlx::query_scalar::<_, bool>("SELECT is_active FROM users WHERE id=$1")
            .bind(uid(PATIENT_USER_ID))
            .fetch_one(&pool)
            .await
            .unwrap()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM notification_device_tokens WHERE user_id=$1 AND enabled"
        )
        .bind(uid(PATIENT_USER_ID))
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    assert!(
        sqlx::query_scalar::<_, bool>("SELECT is_active FROM users WHERE id=$1")
            .bind(uid(DOCTOR1_USER_ID))
            .fetch_one(&pool)
            .await
            .unwrap()
    );
    state.object_storage = store;
    force_erasure_due(&pool, job).await;
    hellodoctor_backend::services::data_rights_service::retry_pending_erasures(&state)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT state FROM account_erasure_jobs WHERE id=$1")
            .bind(job)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "COMPLETED"
    );
    assert!(state
        .object_storage
        .as_ref()
        .unwrap()
        .get(&object_store::path::Path::from(key))
        .await
        .is_err());
    let _ = app;
}

#[sqlx::test(migrations = "./migrations")]
async fn a1_captured_live_payment_is_never_retired_by_expiry(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let (appointment, _) = book_and_confirm(&app, &patient).await;
    sqlx::query(
        "UPDATE transactions SET created_at='2026-09-12T10:00:00Z' WHERE appointment_id=$1",
    )
    .bind(appointment)
    .execute(&pool)
    .await
    .unwrap();
    force_hold_due(&pool, appointment).await;
    assert_eq!(run_hold_expiry_sweep(&state).await.unwrap(), 0);
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT payment_status::text FROM transactions WHERE appointment_id=$1"
        )
        .bind(appointment)
        .fetch_one(&pool)
        .await
        .unwrap(),
        "PAYMENT_HELD"
    );
    assert_eq!(
        post(
            &app,
            "/api/v1/admin/finance/periods/finalize",
            Some(&admin),
            &serde_json::json!({"year":2026,"month":9})
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn a3_profile_failure_keeps_durable_progress_and_account_disabled(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    sqlx::query("CREATE FUNCTION fixture_reject_profile_erase() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'synthetic profile failure'; END $$").execute(&pool).await.unwrap();
    sqlx::query("CREATE TRIGGER fixture_reject_profile_erase BEFORE DELETE ON patient_profiles FOR EACH ROW EXECUTE FUNCTION fixture_reject_profile_erase()").execute(&pool).await.unwrap();
    let response = post(
        &app,
        &format!("/api/v1/admin/patients/{PATIENT_PROFILE_ID}/data-rights/erase"),
        Some(&admin),
        &serde_json::json!({"reason":"Synthetic partial progress erasure"}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let job = uid(json_body(response).await["data"]["job_id"]
        .as_str()
        .unwrap());
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT state FROM account_erasure_jobs WHERE id=$1")
            .bind(job)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "PENDING_PROFILE"
    );
    assert!(
        !sqlx::query_scalar::<_, bool>("SELECT is_active FROM users WHERE id=$1")
            .bind(uid(PATIENT_USER_ID))
            .fetch_one(&pool)
            .await
            .unwrap()
    );
    sqlx::query("DROP TRIGGER fixture_reject_profile_erase ON patient_profiles")
        .execute(&pool)
        .await
        .unwrap();
    force_erasure_due(&pool, job).await;
    hellodoctor_backend::services::data_rights_service::retry_pending_erasures(&state)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT state FROM account_erasure_jobs WHERE id=$1")
            .bind(job)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "COMPLETED"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn a3_erasure_retains_clinical_history_and_disposes_personal_avatar(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let (appointment, _) = book_and_confirm(&app, &patient).await;
    let photo = format!(
        "avatars/patient/{PATIENT_PROFILE_ID}/{}.bin",
        Uuid::new_v4()
    );
    let clinical = format!("clinical/{appointment}/{}.bin", Uuid::new_v4());
    for key in [&photo, &clinical] {
        hellodoctor_backend::services::document_storage_service::DocumentStorageService::put(
            &state,
            key,
            b"synthetic fixture content",
        )
        .await
        .unwrap();
    }
    sqlx::query("UPDATE patient_profiles SET profile_picture_object_key=$2,profile_picture_bucket=$3 WHERE id=$1")
        .bind(uid(PATIENT_PROFILE_ID)).bind(&photo).bind(&state.config.storage_bucket).execute(&pool).await.unwrap();
    let response = post(
        &app,
        &format!("/api/v1/admin/patients/{PATIENT_PROFILE_ID}/data-rights/erase"),
        Some(&admin),
        &serde_json::json!({"reason":"Synthetic retained-history erasure"}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json_body(response).await["data"]["outcome"], "ANONYMIZED");
    assert!(sqlx::query_scalar::<_, bool>(
        "SELECT profile_picture_object_key IS NULL FROM patient_profiles WHERE id=$1"
    )
    .bind(uid(PATIENT_PROFILE_ID))
    .fetch_one(&pool)
    .await
    .unwrap());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM transactions WHERE appointment_id=$1")
            .bind(appointment)
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert!(state
        .object_storage
        .as_ref()
        .unwrap()
        .get(&object_store::path::Path::from(photo))
        .await
        .is_err());
    assert!(state
        .object_storage
        .as_ref()
        .unwrap()
        .get(&object_store::path::Path::from(clinical))
        .await
        .is_ok());
}

#[sqlx::test(migrations = "./migrations")]
async fn a3_invalid_avatar_reference_never_deletes_clinical_objects(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let key = format!("clinical/{}.bin", Uuid::new_v4());
    hellodoctor_backend::services::document_storage_service::DocumentStorageService::put(
        &state,
        &key,
        b"synthetic retained record",
    )
    .await
    .unwrap();
    sqlx::query("UPDATE patient_profiles SET profile_picture_object_key=$2,profile_picture_bucket=$3 WHERE id=$1")
        .bind(uid(PATIENT_PROFILE_ID)).bind(&key).bind(&state.config.storage_bucket).execute(&pool).await.unwrap();
    let response = post(
        &app,
        &format!("/api/v1/admin/patients/{PATIENT_PROFILE_ID}/data-rights/erase"),
        Some(&admin),
        &serde_json::json!({"reason":"Synthetic invalid media reference"}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(json_body(response).await["data"]["outcome"], "PENDING");
    assert!(state
        .object_storage
        .as_ref()
        .unwrap()
        .get(&object_store::path::Path::from(key))
        .await
        .is_ok());
}

#[sqlx::test(migrations = "./migrations")]
async fn a3_concurrent_requests_share_one_journal_and_completion(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let path = format!("/api/v1/admin/patients/{PATIENT_PROFILE_ID}/data-rights/erase");
    let body = serde_json::json!({"reason":"Synthetic duplicate erasure request"});
    let (a, b) = tokio::join!(
        post(&app, &path, Some(&admin), &body),
        post(&app, &path, Some(&admin), &body)
    );
    assert!([StatusCode::OK, StatusCode::ACCEPTED].contains(&a.status()));
    assert!([StatusCode::OK, StatusCode::ACCEPTED].contains(&b.status()));
    assert_eq!(
        json_body(a).await["data"]["job_id"],
        json_body(b).await["data"]["job_id"]
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM account_erasure_jobs")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM audit_events WHERE action='DATA_RIGHTS_PATIENT_ERASED'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn a3_job_rls_and_subject_immutability(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/admin/patients/{PATIENT_PROFILE_ID}/data-rights/erase"),
            Some(&admin),
            &serde_json::json!({"reason":"Synthetic RLS erasure regression"})
        )
        .await
        .status(),
        StatusCode::OK
    );
    let ctx = hellodoctor_backend::auth_context::AuthContext {
        user_id: uid(DOCTOR1_USER_ID).into(),
        role: UserRole::Doctor,
        permissions: vec![],
        session_id: None,
        request_id: None,
    };
    let mut tx = state.begin_authed(&ctx).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM account_erasure_jobs")
            .fetch_one(&mut *tx)
            .await
            .unwrap(),
        0
    );
    assert!(sqlx::query("INSERT INTO account_erasure_jobs(id,profile_kind,profile_id,requested_by) VALUES($1,'DOCTOR',$2,$3)").bind(Uuid::new_v4()).bind(uid(DOCTOR1_PROFILE_ID)).bind(uid(DOCTOR1_USER_ID)).execute(&mut *tx).await.is_err());
    tx.rollback().await.unwrap();
    assert!(sqlx::query("UPDATE account_erasure_jobs SET user_id=$1")
        .bind(uid(DOCTOR1_USER_ID))
        .execute(&pool)
        .await
        .is_err());
    assert!(
        sqlx::query_scalar::<_, bool>("SELECT is_active FROM users WHERE id=$1")
            .bind(uid(DOCTOR1_USER_ID))
            .fetch_one(&pool)
            .await
            .unwrap()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn a3_dependent_erasure_never_deactivates_guardian(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let dependent = Uuid::new_v4();
    sqlx::query("INSERT INTO patient_profiles(id,user_id,guardian_patient_id,full_name,date_of_birth,gender,relationship) VALUES($1,NULL,$2,'Synthetic dependent','2016-01-01','OTHER','Child')")
        .bind(dependent).bind(uid(PATIENT_PROFILE_ID)).execute(&pool).await.unwrap();
    let response = post(
        &app,
        &format!("/api/v1/admin/patients/{dependent}/data-rights/erase"),
        Some(&admin),
        &serde_json::json!({"reason":"Synthetic dependent erasure request"}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        sqlx::query_scalar::<_, bool>("SELECT is_active FROM users WHERE id=$1")
            .bind(uid(PATIENT_USER_ID))
            .fetch_one(&pool)
            .await
            .unwrap()
    );
    assert_eq!(
        get(&app, "/api/v1/patients/me", Some(&patient))
            .await
            .status(),
        StatusCode::OK
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn a3_registration_racing_erasure_leaves_no_enabled_tokens(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let ctx = isolation_ctx(UserId::from(uid(PATIENT_USER_ID)), UserRole::Patient);
    let url = format!("/api/v1/admin/patients/{PATIENT_PROFILE_ID}/data-rights/erase");
    let body = serde_json::json!({"reason":"Synthetic registration/erasure race"});
    let (registration, erasure) = tokio::join!(
        hellodoctor_backend::services::NotificationService::register_device(
            &state,
            &ctx,
            Uuid::new_v4(),
            "ANDROID",
            "synthetic-registration-race"
        ),
        post(&app, &url, Some(&admin), &body)
    );
    assert!(
        registration.is_ok()
            || matches!(
                registration,
                Err(hellodoctor_backend::error::AppError::Forbidden(_))
            )
    );
    assert_eq!(erasure.status(), StatusCode::OK);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM notification_device_tokens WHERE user_id=$1 AND enabled"
        )
        .bind(uid(PATIENT_USER_ID))
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    assert!(
        !sqlx::query_scalar::<_, bool>("SELECT is_active FROM users WHERE id=$1")
            .bind(uid(PATIENT_USER_ID))
            .fetch_one(&pool)
            .await
            .unwrap()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn a3_inflight_device_registration_cannot_reenable_erased_account(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/admin/patients/{PATIENT_PROFILE_ID}/data-rights/erase"),
            Some(&admin),
            &serde_json::json!({"reason":"Synthetic in-flight registration erasure"})
        )
        .await
        .status(),
        StatusCode::OK
    );
    // Represents a request authenticated before erasure, then delayed before
    // entering its write transaction. Outer middleware alone cannot protect it.
    let ctx = isolation_ctx(UserId::from(uid(PATIENT_USER_ID)), UserRole::Patient);
    let result = hellodoctor_backend::services::NotificationService::register_device(
        &state,
        &ctx,
        Uuid::new_v4(),
        "ANDROID",
        "synthetic-after-erasure",
    )
    .await;
    assert!(
        result.is_err(),
        "an already-authenticated registration must not re-enable an erased account"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM notification_device_tokens WHERE user_id=$1 AND enabled"
        )
        .bind(uid(PATIENT_USER_ID))
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    let other = isolation_ctx(UserId::from(uid(DOCTOR1_USER_ID)), UserRole::Doctor);
    hellodoctor_backend::services::NotificationService::register_device(
        &state,
        &other,
        Uuid::new_v4(),
        "ANDROID",
        "synthetic-unrelated-live-account",
    )
    .await
    .unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn a3_restarted_worker_recovers_expired_claim_without_duplicate_erasure(pool: PgPool) {
    let (_app, pool, state) = test_app_and_state(pool).await;
    let job = Uuid::new_v4();
    // Simulate termination after durable intent/claim commit, before the first
    // phase. A fresh worker must respect a live lease, then recover an expired one.
    sqlx::query("INSERT INTO account_erasure_jobs(id,profile_kind,profile_id,user_id,requested_by,claim_id,lease_until) VALUES($1,'PATIENT',$2,$3,$4,$5,clock_timestamp()+interval '1 minute')")
        .bind(job).bind(uid(PATIENT_PROFILE_ID)).bind(uid(PATIENT_USER_ID)).bind(uid(ADMIN_USER_ID)).bind(Uuid::new_v4()).execute(&pool).await.unwrap();
    let restarted = state.clone();
    assert_eq!(
        hellodoctor_backend::services::data_rights_service::retry_pending_erasures(&restarted)
            .await
            .unwrap(),
        0
    );
    assert!(
        sqlx::query_scalar::<_, bool>("SELECT is_active FROM users WHERE id=$1")
            .bind(uid(PATIENT_USER_ID))
            .fetch_one(&pool)
            .await
            .unwrap()
    );
    sqlx::query("UPDATE account_erasure_jobs SET lease_until=clock_timestamp()-interval '1 second' WHERE id=$1").bind(job).execute(&pool).await.unwrap();
    assert_eq!(
        hellodoctor_backend::services::data_rights_service::retry_pending_erasures(&restarted)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        hellodoctor_backend::services::data_rights_service::retry_pending_erasures(&restarted)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT state FROM account_erasure_jobs WHERE id=$1")
            .bind(job)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "COMPLETED"
    );
    assert!(
        !sqlx::query_scalar::<_, bool>("SELECT is_active FROM users WHERE id=$1")
            .bind(uid(PATIENT_USER_ID))
            .fetch_one(&pool)
            .await
            .unwrap()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM audit_events WHERE action='DATA_RIGHTS_PATIENT_ERASED'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn a3_rollback_refuses_to_discard_pending_erasure_jobs(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    sqlx::query("CREATE FUNCTION fixture_reject_deactivation() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NOT NEW.is_active THEN RAISE EXCEPTION 'synthetic failure'; END IF; RETURN NEW; END $$").execute(&pool).await.unwrap();
    sqlx::query("CREATE TRIGGER fixture_reject_deactivation BEFORE UPDATE ON users FOR EACH ROW EXECUTE FUNCTION fixture_reject_deactivation()").execute(&pool).await.unwrap();
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/admin/patients/{PATIENT_PROFILE_ID}/data-rights/erase"),
            Some(&admin),
            &serde_json::json!({"reason":"Synthetic rollback safety request"})
        )
        .await
        .status(),
        StatusCode::ACCEPTED
    );
    let down = include_str!("../../migrations/20261130000002_durable_account_erasure.down.sql");
    assert!(sqlx::raw_sql(down).execute(&pool).await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM account_erasure_jobs WHERE state='PENDING_LOGIN'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
}
