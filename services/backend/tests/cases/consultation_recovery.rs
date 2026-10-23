//! Stage B4 recovery regression coverage. These tests use provider-shaped
//! events through the server service boundary; client telemetry is never used
//! to mutate presence state.
use super::*;
use chrono::Utc;
use hellodoctor_backend::services::agora_token_service::derive_uid;
use hellodoctor_backend::services::consultation_presence_service::{
    apply_provider_event, expire_empty_rooms, ProviderPresenceEvent, ProviderPresenceKind,
};

async fn presence_event(
    state: &AppState,
    channel: &str,
    uid: u32,
    id: &str,
    kind: ProviderPresenceKind,
    sequence: i64,
) -> bool {
    apply_provider_event(
        state,
        ProviderPresenceEvent {
            provider_event_id: id.into(),
            channel_name: channel.into(),
            participant_uid: uid,
            kind,
            provider_sequence: sequence,
            provider_event_at: Utc::now(),
        },
    )
    .await
    .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn test_provider_presence_recovery_grace_rejoin_and_expiry(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let (appointment_id, _) = book_and_confirm(&app, &patient_token).await;
    let channel: String = sqlx::query_scalar(
        "SELECT agora_channel_name FROM consultation_sessions WHERE appointment_id=$1",
    )
    .bind(appointment_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let doctor_uid = derive_uid(&channel, UserId::from(uid(DOCTOR1_USER_ID)));
    let patient_uid = derive_uid(&channel, UserId::from(uid(PATIENT_USER_ID)));

    assert!(
        presence_event(
            &state,
            &channel,
            patient_uid,
            "presence-1",
            ProviderPresenceKind::Joined,
            1,
        )
        .await
    );
    assert!(
        presence_event(
            &state,
            &channel,
            doctor_uid,
            "presence-2",
            ProviderPresenceKind::Joined,
            1,
        )
        .await
    );
    let status: String = sqlx::query_scalar("SELECT status::text FROM appointments WHERE id=$1")
        .bind(appointment_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "IN_CONSULTATION");

    // A stale leave cannot override a newer join for the same participant.
    assert!(
        presence_event(
            &state,
            &channel,
            doctor_uid,
            "presence-3",
            ProviderPresenceKind::Left,
            2,
        )
        .await
    );
    assert!(
        !presence_event(
            &state,
            &channel,
            doctor_uid,
            "presence-stale",
            ProviderPresenceKind::Joined,
            1,
        )
        .await
    );
    let (doctor_present, recovery): (bool, String) = sqlx::query_as(
        "SELECT doctor_present, recovery_state FROM consultation_sessions WHERE appointment_id=$1",
    )
    .bind(appointment_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!doctor_present);
    assert_eq!(recovery, "NONE");

    assert!(
        presence_event(
            &state,
            &channel,
            patient_uid,
            "presence-4",
            ProviderPresenceKind::Left,
            2,
        )
        .await
    );
    let (recovery, deadline): (String, chrono::DateTime<Utc>) = sqlx::query_as(
        "SELECT recovery_state, empty_room_deadline FROM consultation_sessions WHERE appointment_id=$1",
    )
    .bind(appointment_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(recovery, "GRACE_PERIOD");
    assert!((deadline - Utc::now()).num_seconds() <= 180);
    assert!((deadline - Utc::now()).num_seconds() >= 175);

    // Either participant can rejoin alone and clears the persisted deadline.
    assert!(
        presence_event(
            &state,
            &channel,
            patient_uid,
            "presence-5",
            ProviderPresenceKind::Joined,
            3,
        )
        .await
    );
    let (patient_present, recovery, deadline): (bool, String, Option<chrono::DateTime<Utc>>) =
        sqlx::query_as(
            "SELECT patient_present, recovery_state, empty_room_deadline FROM consultation_sessions WHERE appointment_id=$1",
        )
        .bind(appointment_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(patient_present);
    assert_eq!(recovery, "NONE");
    assert!(deadline.is_none());

    // Both leave again; the worker transitions only the session to review.
    assert!(
        presence_event(
            &state,
            &channel,
            patient_uid,
            "presence-6",
            ProviderPresenceKind::Left,
            4,
        )
        .await
    );
    assert!(
        presence_event(
            &state,
            &channel,
            doctor_uid,
            "presence-7",
            ProviderPresenceKind::Left,
            3,
        )
        .await
    );
    sqlx::query(
        "UPDATE consultation_sessions SET empty_room_deadline=clock_timestamp()-interval '1 second' WHERE appointment_id=$1",
    )
    .bind(appointment_id)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(expire_empty_rooms(&state).await.unwrap(), 1);
    let (session_status, recovery): (String, String) = sqlx::query_as(
        "SELECT status::text, recovery_state FROM consultation_sessions WHERE appointment_id=$1",
    )
    .bind(appointment_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(session_status, "PREMATURE_TERMINATION");
    assert_eq!(recovery, "NEEDS_REVIEW");
    let appointment_status: String =
        sqlx::query_scalar("SELECT status::text FROM appointments WHERE id=$1")
            .bind(appointment_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(appointment_status, "IN_CONSULTATION");
    let payment_status: String =
        sqlx::query_scalar("SELECT payment_status::text FROM transactions WHERE appointment_id=$1")
            .bind(appointment_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(payment_status, "PAYMENT_HELD");
    assert!(
        !presence_event(
            &state,
            &channel,
            patient_uid,
            "presence-late-join",
            ProviderPresenceKind::Joined,
            5,
        )
        .await
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_provider_presence_rejects_unknown_participant_and_is_idempotent(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let (appointment_id, _) = book_and_confirm(&app, &patient_token).await;
    let channel: String = sqlx::query_scalar(
        "SELECT agora_channel_name FROM consultation_sessions WHERE appointment_id=$1",
    )
    .bind(appointment_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let err = apply_provider_event(
        &state,
        ProviderPresenceEvent {
            provider_event_id: "unauthorized".into(),
            channel_name: channel.clone(),
            participant_uid: 999_999,
            kind: ProviderPresenceKind::Joined,
            provider_sequence: 1,
            provider_event_at: Utc::now(),
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(
        err,
        hellodoctor_backend::error::AppError::Forbidden(_)
    ));

    let uid = derive_uid(&channel, UserId::from(uid(PATIENT_USER_ID)));
    assert!(
        presence_event(
            &state,
            &channel,
            uid,
            "same-event",
            ProviderPresenceKind::Joined,
            1,
        )
        .await
    );
    assert!(
        !presence_event(
            &state,
            &channel,
            uid,
            "same-event",
            ProviderPresenceKind::Joined,
            1,
        )
        .await
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_agora_notification_signature_and_event_mapping(pool: PgPool) {
    let (app, pool, _state) = test_app_and_state(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let (appointment_id, _) = book_and_confirm(&app, &patient_token).await;
    let channel: String = sqlx::query_scalar(
        "SELECT agora_channel_name FROM consultation_sessions WHERE appointment_id=$1",
    )
    .bind(appointment_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let patient_uid = derive_uid(&channel, UserId::from(uid(PATIENT_USER_ID)));
    let now = Utc::now();
    let body = serde_json::json!({
        "noticeId": "agora-notice-1",
        "productId": 1,
        "eventType": 107,
        "notifyMs": now.timestamp_millis(),
        "payload": {
            "channelName": channel,
            "uid": patient_uid,
            "clientSeq": 1,
            "ts": now.timestamp()
        }
    })
    .to_string();
    let mut request = Request::builder()
        .method("POST")
        .uri("/api/v1/webhooks/agora/presence")
        .header("Content-Type", "application/json")
        .header(
            "Agora-Signature-V2",
            sign_webhook_body(&body, TEST_WEBHOOK_SECRET),
        )
        .body(Body::from(body))
        .unwrap();
    request.extensions_mut().insert(random_peer());
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_interrupted_completion_preserves_payment_and_normal_completion_settles(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let (appointment_id, _) = book_and_confirm(&app, &patient).await;

    sqlx::query(
        "UPDATE consultation_sessions
            SET status='PREMATURE_TERMINATION', recovery_state='NEEDS_REVIEW'
          WHERE appointment_id=$1",
    )
    .bind(appointment_id)
    .execute(&pool)
    .await
    .unwrap();
    let rejected = post(
        &app,
        &format!("/api/v1/consultations/{appointment_id}/complete"),
        Some(&doctor),
        &serde_json::json!({"outcome":"COMPLETED_NO_RX"}),
    )
    .await;
    assert_eq!(rejected.status(), StatusCode::CONFLICT);
    let state: (String, String, String) = sqlx::query_as(
        "SELECT a.status::text, cs.status::text, t.payment_status::text
           FROM appointments a
           JOIN consultation_sessions cs ON cs.appointment_id=a.id
           JOIN transactions t ON t.appointment_id=a.id
          WHERE a.id=$1",
    )
    .bind(appointment_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        state,
        (
            "CONFIRMED".into(),
            "PREMATURE_TERMINATION".into(),
            "PAYMENT_HELD".into()
        )
    );

    let normal_patient = fresh_patient_token(&pool).await;
    let (normal_appointment, _) = book_and_confirm(&app, &normal_patient).await;
    let accepted = post(
        &app,
        &format!("/api/v1/consultations/{normal_appointment}/complete"),
        Some(&doctor),
        &serde_json::json!({"outcome":"COMPLETED_NO_RX"}),
    )
    .await;
    assert_eq!(accepted.status(), StatusCode::OK);
    let normal_state: (String, String) = sqlx::query_as(
        "SELECT a.status::text, t.payment_status::text
           FROM appointments a JOIN transactions t ON t.appointment_id=a.id
          WHERE a.id=$1",
    )
    .bind(normal_appointment)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        normal_state,
        ("COMPLETED".into(), "SETTLED_TO_DOCTOR".into())
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_interruption_and_completion_race_never_settles_interrupted_session(pool: PgPool) {
    let (app, pool, _state) = test_app_and_state(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let (appointment_id, _) = book_and_confirm(&app, &patient).await;
    sqlx::query("UPDATE appointments SET status='IN_CONSULTATION' WHERE id=$1")
        .bind(appointment_id)
        .execute(&pool)
        .await
        .unwrap();

    let transition_pool = pool.clone();
    let interrupt = async move {
        let mut tx = transition_pool.begin().await.unwrap();
        sqlx::query("SELECT id FROM consultation_sessions WHERE appointment_id=$1 FOR UPDATE")
            .bind(appointment_id)
            .fetch_one(&mut *tx)
            .await
            .unwrap();
        sqlx::query(
            "UPDATE consultation_sessions
                SET status='PREMATURE_TERMINATION', recovery_state='NEEDS_REVIEW'
              WHERE appointment_id=$1 AND recovery_state='NONE'
                AND status IN ('INITIALIZED','ACTIVE')",
        )
        .bind(appointment_id)
        .execute(&mut *tx)
        .await
        .unwrap();
        tx.commit().await.unwrap();
    };
    let complete_uri = format!("/api/v1/consultations/{appointment_id}/complete");
    let complete_body = serde_json::json!({"outcome":"COMPLETED_NO_RX"});
    let complete = post(&app, &complete_uri, Some(&doctor), &complete_body);
    let (_, completion_response) = tokio::join!(interrupt, complete);
    let final_state: (String, String, String) = sqlx::query_as(
        "SELECT a.status::text, cs.status::text, t.payment_status::text
           FROM appointments a
           JOIN consultation_sessions cs ON cs.appointment_id=a.id
           JOIN transactions t ON t.appointment_id=a.id
          WHERE a.id=$1",
    )
    .bind(appointment_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        (final_state.1 == "PREMATURE_TERMINATION"
            && final_state.2 == "PAYMENT_HELD"
            && completion_response.status() == StatusCode::CONFLICT)
            || (final_state.1 == "COMPLETED"
                && final_state.2 == "SETTLED_TO_DOCTOR"
                && completion_response.status() == StatusCode::OK),
        "race produced an invalid mixed financial/session state: {final_state:?}"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_admin_monitor_and_resolution_preserve_interrupted_financial_state(pool: PgPool) {
    let (app, pool, _state) = test_app_and_state(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let (appointment_id, _) = book_and_confirm(&app, &patient).await;
    let session_id: Uuid =
        sqlx::query_scalar("SELECT id FROM consultation_sessions WHERE appointment_id=$1")
            .bind(appointment_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query(
        "UPDATE consultation_sessions
         SET status='PREMATURE_TERMINATION', recovery_state='NEEDS_REVIEW', ended_at=clock_timestamp() 
         WHERE id=$1",
    )
    .bind(session_id)
    .execute(&pool)
    .await
    .unwrap();

    let monitor = get(&app, "/api/v1/admin/command/telemetry", Some(&admin)).await;
    assert_eq!(monitor.status(), StatusCode::OK);
    let monitor_body: Value = json_body(monitor).await;
    assert!(monitor_body["data"]["active_streams"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["session_id"] == session_id.to_string() && row["needs_review"] == true));

    let resolution = post(
        &app,
        &format!("/api/v1/admin/consultations/{session_id}/recovery-resolution"),
        Some(&admin),
        &serde_json::json!({
            "outcome": "PENDING_FINANCE_ACTION",
            "reason": "Review interruption evidence before finance action."
        }),
    )
    .await;
    assert_eq!(resolution.status(), StatusCode::OK);
    let duplicate = post(
        &app,
        &format!("/api/v1/admin/consultations/{session_id}/recovery-resolution"),
        Some(&admin),
        &serde_json::json!({
            "outcome": "PENDING_FINANCE_ACTION",
            "reason": "Review interruption evidence before finance action."
        }),
    )
    .await;
    assert_eq!(duplicate.status(), StatusCode::OK);

    let state: (String, String) = sqlx::query_as(
        "SELECT cs.recovery_state, t.payment_status::text
         FROM consultation_sessions cs JOIN transactions t ON t.appointment_id=cs.appointment_id
         WHERE cs.id=$1",
    )
    .bind(session_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(state, ("NEEDS_REVIEW".into(), "PAYMENT_HELD".into()));
}
