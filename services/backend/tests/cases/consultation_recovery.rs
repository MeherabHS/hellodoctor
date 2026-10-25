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

// ============================================================================
// Notification Milestone 2 — Consultation Lifecycle Notifications.
// ============================================================================

/// `CONSULTATION_REJOIN` fires exactly once per Doctor/Patient the moment the
/// room becomes fully empty (both provider-confirmed presences absent), and
/// a second, rapid re-entry into the empty-room state on the same session is
/// suppressed by the 120s cooldown rather than spamming — the underlying
/// `GRACE_PERIOD`/`empty_room_deadline` state itself still updates correctly,
/// only the notification is rate-limited.
#[sqlx::test(migrations = "./migrations")]
async fn test_consultation_rejoin_notifies_once_and_is_cooldown_limited(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let (appointment_id, _) = book_and_confirm(&app, &patient_token).await;
    let session_id: Uuid =
        sqlx::query_scalar("SELECT id FROM consultation_sessions WHERE appointment_id=$1")
            .bind(appointment_id)
            .fetch_one(&pool)
            .await
            .unwrap();
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
            "r1",
            ProviderPresenceKind::Joined,
            1
        )
        .await
    );
    assert!(
        presence_event(
            &state,
            &channel,
            doctor_uid,
            "r2",
            ProviderPresenceKind::Joined,
            1
        )
        .await
    );
    assert!(
        presence_event(
            &state,
            &channel,
            doctor_uid,
            "r3",
            ProviderPresenceKind::Left,
            2
        )
        .await
    );
    // Both now absent -> the room just became empty, the one real transition
    // into GRACE_PERIOD this test exercises.
    assert!(
        presence_event(
            &state,
            &channel,
            patient_uid,
            "r4",
            ProviderPresenceKind::Left,
            2
        )
        .await
    );

    let rows: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT user_id, dedup_key FROM notifications \
         WHERE related_entity_id = $1 AND type = 'CONSULTATION_REJOIN' ORDER BY dedup_key",
    )
    .bind(appointment_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        rows.len(),
        2,
        "exactly one Rejoin notification each for Doctor and Patient"
    );
    assert_eq!(rows[0].0, uid(DOCTOR1_USER_ID));
    assert!(rows[0].1.contains(&session_id.to_string()) && rows[0].1.ends_with(":doctor"));
    assert_eq!(rows[1].0, uid(PATIENT_USER_ID));

    // Patient rejoins (clears the grace period), then both leave again
    // immediately — a second, genuinely distinct GRACE_PERIOD entry, keyed by
    // a fresh deadline, but within the 120s cooldown window of the first.
    assert!(
        presence_event(
            &state,
            &channel,
            patient_uid,
            "r5",
            ProviderPresenceKind::Joined,
            3
        )
        .await
    );
    let (recovery,): (String,) =
        sqlx::query_as("SELECT recovery_state FROM consultation_sessions WHERE id=$1")
            .bind(session_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        recovery, "NONE",
        "rejoin must still clear recovery_state normally"
    );
    assert!(
        presence_event(
            &state,
            &channel,
            patient_uid,
            "r6",
            ProviderPresenceKind::Left,
            4
        )
        .await
    );

    let (recovery, deadline): (String, Option<chrono::DateTime<Utc>>) = sqlx::query_as(
        "SELECT recovery_state, empty_room_deadline FROM consultation_sessions WHERE id=$1",
    )
    .bind(session_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        recovery, "GRACE_PERIOD",
        "the business state machine itself is never rate-limited, only the notification"
    );
    assert!(deadline.is_some());

    let count_after: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM notifications WHERE related_entity_id = $1 AND type = 'CONSULTATION_REJOIN'",
    )
    .bind(appointment_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        count_after, 2,
        "a second empty-room within the 120s cooldown must not add more Rejoin notifications"
    );
}

/// `CONSULTATION_INTERRUPTED` (both participants) and `CONSULTATION_NEEDS_REVIEW`
/// (every active Admin) fire exactly once, atomically with the real 180s
/// grace-period expiry sweep — never implying medical completion.
#[sqlx::test(migrations = "./migrations")]
async fn test_consultation_interrupted_and_needs_review_on_grace_expiry(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let (appointment_id, _) = book_and_confirm(&app, &patient_token).await;
    let session_id: Uuid =
        sqlx::query_scalar("SELECT id FROM consultation_sessions WHERE appointment_id=$1")
            .bind(appointment_id)
            .fetch_one(&pool)
            .await
            .unwrap();
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
            "e1",
            ProviderPresenceKind::Joined,
            1
        )
        .await
    );
    assert!(
        presence_event(
            &state,
            &channel,
            doctor_uid,
            "e2",
            ProviderPresenceKind::Joined,
            1
        )
        .await
    );
    assert!(
        presence_event(
            &state,
            &channel,
            doctor_uid,
            "e3",
            ProviderPresenceKind::Left,
            2
        )
        .await
    );
    assert!(
        presence_event(
            &state,
            &channel,
            patient_uid,
            "e4",
            ProviderPresenceKind::Left,
            2
        )
        .await
    );

    sqlx::query(
        "UPDATE consultation_sessions SET empty_room_deadline=clock_timestamp()-interval '1 second' WHERE id=$1",
    )
    .bind(session_id)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(expire_empty_rooms(&state).await.unwrap(), 1);

    let interrupted: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT user_id, body FROM notifications \
         WHERE related_entity_id = $1 AND type = 'CONSULTATION_INTERRUPTED'",
    )
    .bind(appointment_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(interrupted.len(), 2, "both Doctor and Patient notified");
    for (_, body) in &interrupted {
        let lower = body.to_lowercase();
        assert!(lower.contains("interrupted"));
        assert!(
            !lower.contains("completed consultation")
                && lower.contains("not been marked completed"),
            "must never imply medical completion: {body}"
        );
    }

    let needs_review: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM notifications \
         WHERE related_entity_id = $1 AND type = 'CONSULTATION_NEEDS_REVIEW' AND user_id = $2",
    )
    .bind(session_id)
    .bind(uid(ADMIN_USER_ID))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        needs_review, 1,
        "the seeded active Admin gets exactly one persistent review alert"
    );

    // A second sweep tick must never re-notify for the same, already-resolved
    // (well, already-NEEDS_REVIEW) session.
    assert_eq!(expire_empty_rooms(&state).await.unwrap(), 0);
    let total_after_second_sweep: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM notifications WHERE related_entity_id = $1 AND type = 'CONSULTATION_INTERRUPTED'",
    )
    .bind(appointment_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(total_after_second_sweep, 2);
}

/// The *other* path into `NEEDS_REVIEW` — a late provider join arriving after
/// the deadline has technically passed but before the background sweep has
/// processed it — must notify exactly the same way as the sweep path, not a
/// `CONSULTATION_REJOIN` (rejoining is, by definition, no longer possible).
#[sqlx::test(migrations = "./migrations")]
async fn test_late_rejoin_after_deadline_notifies_interrupted_not_rejoin(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let (appointment_id, _) = book_and_confirm(&app, &patient_token).await;
    let session_id: Uuid =
        sqlx::query_scalar("SELECT id FROM consultation_sessions WHERE appointment_id=$1")
            .bind(appointment_id)
            .fetch_one(&pool)
            .await
            .unwrap();
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
            "l1",
            ProviderPresenceKind::Joined,
            1
        )
        .await
    );
    assert!(
        presence_event(
            &state,
            &channel,
            doctor_uid,
            "l2",
            ProviderPresenceKind::Joined,
            1
        )
        .await
    );
    assert!(
        presence_event(
            &state,
            &channel,
            doctor_uid,
            "l3",
            ProviderPresenceKind::Left,
            2
        )
        .await
    );
    assert!(
        presence_event(
            &state,
            &channel,
            patient_uid,
            "l4",
            ProviderPresenceKind::Left,
            2
        )
        .await
    );
    let rejoin_count_before: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM notifications WHERE related_entity_id = $1 AND type = 'CONSULTATION_REJOIN'",
    )
    .bind(appointment_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(rejoin_count_before, 2);

    // Simulate the deadline having already passed without the sweep having
    // run yet (the exact race `apply_provider_event`'s own branch guards).
    sqlx::query(
        "UPDATE consultation_sessions SET empty_room_deadline=clock_timestamp()-interval '1 second' WHERE id=$1",
    )
    .bind(session_id)
    .execute(&pool)
    .await
    .unwrap();
    assert!(
        !presence_event(
            &state,
            &channel,
            patient_uid,
            "late-join",
            ProviderPresenceKind::Joined,
            3
        )
        .await
    );

    let rejoin_count_after: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM notifications WHERE related_entity_id = $1 AND type = 'CONSULTATION_REJOIN'",
    )
    .bind(appointment_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        rejoin_count_after, 2,
        "a rejected late join must never create a new Rejoin invitation"
    );
    let interrupted: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM notifications WHERE related_entity_id = $1 AND type = 'CONSULTATION_INTERRUPTED'",
    )
    .bind(appointment_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(interrupted, 2, "this path must also notify Interrupted");
    let needs_review: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM notifications WHERE related_entity_id = $1 AND type = 'CONSULTATION_NEEDS_REVIEW'",
    )
    .bind(session_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(needs_review, 1);
}

/// `CONSULTATION_RECOVERY_RESOLVED` fires once per participant on a genuine
/// `RESOLVED` outcome, is worded per-outcome without ever promising money
/// moved, and a replayed identical resolution (`ALREADY_RESOLVED`) must not
/// create a second pair.
#[sqlx::test(migrations = "./migrations")]
async fn test_consultation_recovery_resolution_notifies_without_promising_payment(pool: PgPool) {
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

    let reason = "Review interruption evidence before finance action.";
    let first = post(
        &app,
        &format!("/api/v1/admin/consultations/{session_id}/recovery-resolution"),
        Some(&admin),
        &serde_json::json!({"outcome": "PENDING_FINANCE_ACTION", "reason": reason}),
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK);
    let second = post(
        &app,
        &format!("/api/v1/admin/consultations/{session_id}/recovery-resolution"),
        Some(&admin),
        &serde_json::json!({"outcome": "PENDING_FINANCE_ACTION", "reason": reason}),
    )
    .await;
    assert_eq!(second.status(), StatusCode::OK);

    let rows: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT user_id, body FROM notifications \
         WHERE related_entity_id = $1 AND type = 'CONSULTATION_RECOVERY_RESOLVED'",
    )
    .bind(session_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        rows.len(),
        2,
        "exactly one notification each for Doctor and Patient, not duplicated on replay"
    );
    for (_, body) in &rows {
        let lower = body.to_lowercase();
        assert!(lower.contains("pending"));
        assert!(
            !lower.contains("refund has been") && !lower.contains("payment has been issued"),
            "PENDING_FINANCE_ACTION must never claim money already moved: {body}"
        );
    }
}

/// `CONSULTATION_COMPLETED` fires exactly once per participant on the real
/// authorized completion transition, and a retried completion call (already
/// `ALREADY_COMPLETED` at the SQL layer) never duplicates it.
#[sqlx::test(migrations = "./migrations")]
async fn test_consultation_completed_notifies_once_and_is_idempotent(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let (appointment_id, _) = book_and_confirm(&app, &patient).await;

    let complete = |app: axum::Router| {
        let doctor = doctor.clone();
        async move {
            post(
                &app,
                &format!("/api/v1/consultations/{appointment_id}/complete"),
                Some(&doctor),
                &serde_json::json!({"outcome":"COMPLETED_NO_RX"}),
            )
            .await
        }
    };
    assert_eq!(complete(app.clone()).await.status(), StatusCode::OK);
    assert_eq!(complete(app.clone()).await.status(), StatusCode::OK);

    let rows: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT user_id, dedup_key FROM notifications \
         WHERE related_entity_id = $1 AND type = 'CONSULTATION_COMPLETED' ORDER BY dedup_key",
    )
    .bind(appointment_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        rows.len(),
        2,
        "exactly one notification each for Doctor and Patient, not duplicated on retry"
    );
    assert_eq!(rows[0].0, uid(DOCTOR1_USER_ID));
    assert_eq!(rows[1].0, uid(PATIENT_USER_ID));
}

/// `CONSULTATION_READY` fires for a video appointment whose authorized join
/// window (the SAME window `mint_agora_token` enforces) is currently open,
/// routed through the guardian for a dependent's appointment — never to the
/// dependent, who has no account of their own.
#[sqlx::test(migrations = "./migrations")]
async fn test_consultation_ready_pass_routes_dependent_appointment_to_guardian(pool: PgPool) {
    let (_app, pool, state) = test_app_and_state(pool).await;
    use hellodoctor_backend::services::notification_scheduler::run_consultation_ready_pass;

    let dependent_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO patient_profiles (id, user_id, guardian_patient_id, full_name, date_of_birth, gender, created_at, updated_at) \
         VALUES ($1, NULL, $2, 'Consultation Ready Dependent', '2015-01-01', 'OTHER', NOW(), NOW())",
    )
    .bind(dependent_id)
    .bind(uid(PATIENT_PROFILE_ID))
    .execute(&pool)
    .await
    .unwrap();

    let now = Utc::now();
    // `test_config()`'s default 1440-minute (24h) window is deliberately
    // generous (see `test_agora_token_time_gate_rejects_outside_window`'s own
    // comment); a slot starting soon sits comfortably inside it.
    let (_slot, appointment_id) = insert_confirmed_appointment_for(
        &pool,
        uid(DOCTOR1_PROFILE_ID),
        dependent_id,
        now + chrono::Duration::minutes(10),
    )
    .await;

    run_consultation_ready_pass(&state, now).await.unwrap();

    let rows: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT user_id, dedup_key FROM notifications \
         WHERE related_entity_id = $1 AND type = 'CONSULTATION_READY' ORDER BY dedup_key",
    )
    .bind(appointment_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        rows.len(),
        2,
        "Doctor and the guardian, never the dependent"
    );
    assert_eq!(rows[0].0, uid(DOCTOR1_USER_ID));
    assert_eq!(
        rows[1].0,
        uid(PATIENT_USER_ID),
        "routed to the guardian's own account"
    );

    // A second tick must not duplicate it.
    run_consultation_ready_pass(&state, Utc::now())
        .await
        .unwrap();
    let count_after: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM notifications WHERE related_entity_id = $1 AND type = 'CONSULTATION_READY'",
    )
    .bind(appointment_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count_after, 2);
}

/// A video appointment whose join window has not yet opened (and a CHAT
/// appointment, for which no join window concept applies at all) must never
/// get a `CONSULTATION_READY` notification.
#[sqlx::test(migrations = "./migrations")]
async fn test_consultation_ready_pass_excludes_out_of_window_and_chat_appointments(pool: PgPool) {
    let (_app, pool, state) = test_app_and_state(pool).await;
    use hellodoctor_backend::services::notification_scheduler::run_consultation_ready_pass;

    let now = Utc::now();
    // Outside even the test harness's generous 24h default window.
    let (_slot, far_future_appointment) =
        insert_confirmed_appointment(&pool, now + chrono::Duration::days(10)).await;

    run_consultation_ready_pass(&state, now).await.unwrap();

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM notifications WHERE related_entity_id = $1 AND type = 'CONSULTATION_READY'",
    )
    .bind(far_future_appointment)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        count, 0,
        "an appointment far outside the join window must not be announced ready"
    );
}
