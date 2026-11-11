//! M8-G-30 / M8-G-31 regression coverage for
//! `consultation_live_reservations` — see
//! docs/MILESTONE_8_SINGLE_LIVE_CONSULTATION_DESIGN.md (Revision 2) for the
//! full design, limitations, and exactly what each of these tests is meant
//! to prove. Local fixtures mirror `schedule_exclusivity.rs`'s own
//! `raw_slot`/`insert_confirmed` style (direct-insert, precise-timing
//! appointment construction) rather than importing them, matching this test
//! suite's existing per-file-local-fixture convention.
use super::*;
use hellodoctor_backend::services::agora_token_service::derive_uid;
use hellodoctor_backend::services::consultation_presence_service::{
    apply_provider_event, ProviderPresenceEvent, ProviderPresenceKind,
};

async fn raw_slot(
    pool: &PgPool,
    doctor_id: Uuid,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO doctor_schedule_slots (id, doctor_id, start_time, end_time, status, created_at) \
         VALUES ($1,$2,$3,$4,'AVAILABLE',clock_timestamp())",
    )
    .bind(id)
    .bind(doctor_id)
    .bind(start)
    .bind(end)
    .execute(pool)
    .await
    .unwrap();
    id
}

async fn insert_confirmed(pool: &PgPool, patient_id: Uuid, doctor_id: Uuid, slot_id: Uuid) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO appointments (id, appointment_number, patient_id, doctor_id, slot_id, modality, status, consultation_fee, created_at, updated_at) \
         VALUES ($1,$2,$3,$4,$5,'VIDEO','CONFIRMED',100.00,clock_timestamp(),clock_timestamp())",
    )
    .bind(id)
    .bind(format!("APT-TEST-{}", id.to_string()[0..8].to_uppercase()))
    .bind(patient_id)
    .bind(doctor_id)
    .bind(slot_id)
    .execute(pool)
    .await
    .unwrap();
    id
}

/// Two back-to-back CONFIRMED appointments for DOCTOR1 with the shared
/// seeded patient — mirrors the exact real-world shape the design doc's §1
/// describes (adjacent, non-overlapping slot times that nonetheless have
/// overlapping Agora mint windows under the test harness's generous
/// `agora_token_window_before/after_mins = 1440` default).
async fn two_back_to_back_appointments_for_doctor1(pool: &PgPool) -> (Uuid, Uuid) {
    let patient_id = uid(PATIENT_PROFILE_ID);
    let doctor_id = uid(DOCTOR1_PROFILE_ID);
    // Must stay within the test harness's configured Agora mint window
    // (`agora_token_window_before/after_mins`, ±1440 minutes = 24h around
    // the slot) — confirmed empirically: a slot 10 days out was rejected by
    // mint_agora_token's own temporal gate before this feature's logic ever
    // ran, not by anything this test is supposed to exercise. Also confirmed
    // empirically: `seed::seed_dev_data` itself pre-creates five 15-minute
    // DOCTOR1 slots at now+1h..now+5h, so this must stay clear of that range
    // too or collide on `no_overlapping_slots`.
    let start_a = Utc::now() + Duration::hours(8);
    let end_a = start_a + Duration::minutes(30);
    let end_b = end_a + Duration::minutes(30);
    let slot_a = raw_slot(pool, doctor_id, start_a, end_a).await;
    let slot_b = raw_slot(pool, doctor_id, end_a, end_b).await;
    let apt_a = insert_confirmed(pool, patient_id, doctor_id, slot_a).await;
    let apt_b = insert_confirmed(pool, patient_id, doctor_id, slot_b).await;
    (apt_a, apt_b)
}

async fn mint(app: &axum::Router, appointment_id: Uuid, token: &str) -> StatusCode {
    post(
        app,
        &format!("/api/v1/consultations/{appointment_id}/rtc-token"),
        Some(token),
        &serde_json::json!({}),
    )
    .await
    .status()
}

async fn channel_of(pool: &PgPool, appointment_id: Uuid) -> String {
    sqlx::query_scalar(
        "SELECT agora_channel_name FROM consultation_sessions WHERE appointment_id=$1",
    )
    .bind(appointment_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn session_id_of(pool: &PgPool, appointment_id: Uuid) -> Uuid {
    sqlx::query_scalar("SELECT id FROM consultation_sessions WHERE appointment_id=$1")
        .bind(appointment_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn presence_event(
    state: &AppState,
    channel: &str,
    uid_value: u32,
    event_id: &str,
    kind: ProviderPresenceKind,
    sequence: i64,
) -> bool {
    apply_provider_event(
        state,
        ProviderPresenceEvent {
            provider_event_id: event_id.into(),
            channel_name: channel.into(),
            participant_uid: uid_value,
            kind,
            provider_sequence: sequence,
            provider_event_at: Utc::now(),
        },
    )
    .await
    .unwrap()
}

async fn reservation_row(
    pool: &PgPool,
    user_id: Uuid,
) -> Option<(Uuid, Uuid, String, DateTime<Utc>)> {
    sqlx::query_as(
        "SELECT appointment_id, session_id, state, expires_at FROM consultation_live_reservations WHERE user_id=$1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .unwrap()
}

// 1. The core failure path: two back-to-back appointments for one doctor
// have overlapping mint windows; minting for the second while the first's
// reservation is still outstanding must be rejected.
#[sqlx::test(migrations = "./migrations")]
async fn test_mint_rejected_for_overlapping_appointment_same_doctor(pool: PgPool) {
    let (app, pool, _state) = test_app_and_state(pool).await;
    let (apt_a, apt_b) = two_back_to_back_appointments_for_doctor1(&pool).await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    assert_eq!(mint(&app, apt_a, &doctor).await, StatusCode::OK);
    assert_eq!(mint(&app, apt_b, &doctor).await, StatusCode::CONFLICT);
}

// 2. A reconnect to the SAME appointment is never gated, regardless of how
// many times it is re-minted.
#[sqlx::test(migrations = "./migrations")]
async fn test_mint_allowed_for_same_appointment_reconnect(pool: PgPool) {
    let (app, pool, _state) = test_app_and_state(pool).await;
    let (apt_a, _apt_b) = two_back_to_back_appointments_for_doctor1(&pool).await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    assert_eq!(mint(&app, apt_a, &doctor).await, StatusCode::OK);
    assert_eq!(mint(&app, apt_a, &doctor).await, StatusCode::OK);
    assert_eq!(mint(&app, apt_a, &doctor).await, StatusCode::OK);
}

// 3. An unconfirmed PENDING claim self-expires after
// RESERVATION_PENDING_GRACE_SECS and becomes reclaimable — proves the short
// lease does not permanently block a legitimate later mint.
#[sqlx::test(migrations = "./migrations")]
async fn test_pending_reservation_self_expires_allows_new_mint(pool: PgPool) {
    let (app, pool, _state) = test_app_and_state(pool).await;
    let (apt_a, apt_b) = two_back_to_back_appointments_for_doctor1(&pool).await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    assert_eq!(mint(&app, apt_a, &doctor).await, StatusCode::OK);
    assert_eq!(mint(&app, apt_b, &doctor).await, StatusCode::CONFLICT);
    sqlx::query(
        "UPDATE consultation_live_reservations SET expires_at = clock_timestamp() - interval '1 second' WHERE user_id=$1",
    )
    .bind(uid(DOCTOR1_USER_ID))
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(mint(&app, apt_b, &doctor).await, StatusCode::OK);
}

// 4. Issue-2 regression: a same-appointment re-mint must never downgrade an
// already-CONFIRMED reservation back to a short-lived PENDING deadline.
#[sqlx::test(migrations = "./migrations")]
async fn test_confirmed_reservation_not_downgraded_by_same_appointment_remint(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let (apt_a, _apt_b) = two_back_to_back_appointments_for_doctor1(&pool).await;
    let doctor_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    assert_eq!(mint(&app, apt_a, &doctor_token).await, StatusCode::OK);
    let channel = channel_of(&pool, apt_a).await;
    let doctor_uid = derive_uid(&channel, UserId::from(uid(DOCTOR1_USER_ID)));
    assert!(
        presence_event(
            &state,
            &channel,
            doctor_uid,
            "e1",
            ProviderPresenceKind::Joined,
            1
        )
        .await
    );
    let (_, _, state_before, expires_before) = reservation_row(&pool, uid(DOCTOR1_USER_ID))
        .await
        .expect("reservation must exist after confirmation");
    assert_eq!(state_before, "CONFIRMED");

    assert_eq!(mint(&app, apt_a, &doctor_token).await, StatusCode::OK);
    let (_, _, state_after, expires_after) = reservation_row(&pool, uid(DOCTOR1_USER_ID))
        .await
        .expect("reservation must still exist");
    assert_eq!(
        state_after, "CONFIRMED",
        "re-mint must not downgrade CONFIRMED to PENDING"
    );
    assert_eq!(
        expires_before, expires_after,
        "re-mint must not shrink a CONFIRMED reservation's backstop expiry"
    );
}

// 5. Issue-3 regression: the true zero-row race. Two concurrent first-ever
// acquisitions for the same user, two different appointments, must resolve
// to exactly one 200/one 409 — never two 200s, and never a raw database
// error surfacing as a 500.
#[sqlx::test(migrations = "./migrations")]
async fn test_concurrent_first_acquisition_exactly_one_succeeds_no_db_error(pool: PgPool) {
    let (app, pool, _state) = test_app_and_state(pool).await;
    let (apt_a, apt_b) = two_back_to_back_appointments_for_doctor1(&pool).await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let (status_a, status_b) = tokio::join!(mint(&app, apt_a, &doctor), mint(&app, apt_b, &doctor));
    let outcomes = [status_a, status_b];
    assert_eq!(
        outcomes.iter().filter(|s| **s == StatusCode::OK).count(),
        1,
        "exactly one concurrent mint must succeed: {outcomes:?}"
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|s| **s == StatusCode::CONFLICT)
            .count(),
        1,
        "the other must be a clean 409, never a 500: {outcomes:?}"
    );
}

// 6. Issue-4 regression: the CONFIRMED backstop must not be derived from the
// appointment's own mint-window ceiling — a reservation confirmed for an
// appointment whose mint window has since closed must still block a
// different appointment's mint, because the first consultation could still
// genuinely be live on an already-issued token.
#[sqlx::test(migrations = "./migrations")]
async fn test_confirmed_reservation_survives_past_appointment_window_during_live_call(
    pool: PgPool,
) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let (apt_a, apt_b) = two_back_to_back_appointments_for_doctor1(&pool).await;
    let doctor_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    assert_eq!(mint(&app, apt_a, &doctor_token).await, StatusCode::OK);
    let channel = channel_of(&pool, apt_a).await;
    let doctor_uid = derive_uid(&channel, UserId::from(uid(DOCTOR1_USER_ID)));
    assert!(
        presence_event(
            &state,
            &channel,
            doctor_uid,
            "e1",
            ProviderPresenceKind::Joined,
            1
        )
        .await
    );

    // Force A's own slot window far into the past, so mint_agora_token's
    // temporal window gate would now reject re-minting A regardless of this
    // reservation logic — proving the reservation's own backstop is
    // independent of that gate.
    sqlx::query(
        "UPDATE doctor_schedule_slots SET start_time = clock_timestamp() - interval '10 days', end_time = clock_timestamp() - interval '9 days 23 hours 30 minutes' \
         WHERE id = (SELECT slot_id FROM appointments WHERE id=$1)",
    )
    .bind(apt_a)
    .execute(&pool)
    .await
    .unwrap();

    assert_eq!(
        mint(&app, apt_b, &doctor_token).await,
        StatusCode::CONFLICT,
        "a CONFIRMED reservation must not be reclaimable just because the originating appointment's mint window closed"
    );
}

// 7. The emergency backstop itself: with no further trusted events at all,
// a CONFIRMED reservation becomes reclaimable once its (long) backstop
// elapses — recovery from a permanently lost Left webhook.
#[sqlx::test(migrations = "./migrations")]
async fn test_confirmed_reservation_releases_after_long_backstop_with_no_further_events(
    pool: PgPool,
) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let (apt_a, apt_b) = two_back_to_back_appointments_for_doctor1(&pool).await;
    let doctor_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    assert_eq!(mint(&app, apt_a, &doctor_token).await, StatusCode::OK);
    let channel = channel_of(&pool, apt_a).await;
    let doctor_uid = derive_uid(&channel, UserId::from(uid(DOCTOR1_USER_ID)));
    assert!(
        presence_event(
            &state,
            &channel,
            doctor_uid,
            "e1",
            ProviderPresenceKind::Joined,
            1
        )
        .await
    );

    sqlx::query(
        "UPDATE consultation_live_reservations SET expires_at = clock_timestamp() - interval '1 second' WHERE user_id=$1",
    )
    .bind(uid(DOCTOR1_USER_ID))
    .execute(&pool)
    .await
    .unwrap();

    assert_eq!(mint(&app, apt_b, &doctor_token).await, StatusCode::OK);
}

// 8. The backstop refreshes on every further accepted trusted event, so a
// normally-reconnecting long call never approaches it.
#[sqlx::test(migrations = "./migrations")]
async fn test_confirmed_backstop_refreshes_on_reconnect_during_long_call(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let (apt_a, _apt_b) = two_back_to_back_appointments_for_doctor1(&pool).await;
    let doctor_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    assert_eq!(mint(&app, apt_a, &doctor_token).await, StatusCode::OK);
    let channel = channel_of(&pool, apt_a).await;
    let doctor_uid = derive_uid(&channel, UserId::from(uid(DOCTOR1_USER_ID)));
    assert!(
        presence_event(
            &state,
            &channel,
            doctor_uid,
            "e1",
            ProviderPresenceKind::Joined,
            1
        )
        .await
    );

    // Simulate most of the backstop window having already elapsed.
    sqlx::query(
        "UPDATE consultation_live_reservations SET expires_at = clock_timestamp() + interval '10 seconds' WHERE user_id=$1",
    )
    .bind(uid(DOCTOR1_USER_ID))
    .execute(&pool)
    .await
    .unwrap();

    // An ordinary reconnect: another accepted Joined event for the same role/session.
    assert!(
        presence_event(
            &state,
            &channel,
            doctor_uid,
            "e2",
            ProviderPresenceKind::Joined,
            2
        )
        .await
    );

    let (_, _, state_now, expires_now) = reservation_row(&pool, uid(DOCTOR1_USER_ID))
        .await
        .expect("reservation must still exist");
    assert_eq!(state_now, "CONFIRMED");
    assert!(
        expires_now - Utc::now() > Duration::hours(1),
        "a reconnect must refresh the backstop back out to the full ceiling, not leave the near-expiry value in place"
    );
}

// 9. A stale, out-of-order Left arriving after a newer Joined must not
// release the reservation — inherits the existing sequence-ordering
// guarantee, asserted here against the new table specifically.
#[sqlx::test(migrations = "./migrations")]
async fn test_stale_left_cannot_release_reservation_after_newer_join(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let (apt_a, _apt_b) = two_back_to_back_appointments_for_doctor1(&pool).await;
    let doctor_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    assert_eq!(mint(&app, apt_a, &doctor_token).await, StatusCode::OK);
    let channel = channel_of(&pool, apt_a).await;
    let doctor_uid = derive_uid(&channel, UserId::from(uid(DOCTOR1_USER_ID)));
    assert!(
        presence_event(
            &state,
            &channel,
            doctor_uid,
            "e1",
            ProviderPresenceKind::Joined,
            2
        )
        .await
    );
    let (_, _, state_before, expires_before) = reservation_row(&pool, uid(DOCTOR1_USER_ID))
        .await
        .expect("reservation must exist");
    assert_eq!(state_before, "CONFIRMED");

    // A stale Left at an OLDER sequence number must be discarded by the
    // existing sequence check before it ever reaches the reservation logic.
    assert!(
        !presence_event(
            &state,
            &channel,
            doctor_uid,
            "stale-left",
            ProviderPresenceKind::Left,
            1
        )
        .await
    );

    let (_, _, state_after, expires_after) = reservation_row(&pool, uid(DOCTOR1_USER_ID))
        .await
        .expect("reservation must be untouched by the rejected stale event");
    assert_eq!(state_after, "CONFIRMED");
    assert_eq!(expires_before, expires_after);
}

// 10. Issue-6 regression (conflict remediation, not prevention): a trusted
// Joined for the same user in a DIFFERENT, already-CONFIRMED-elsewhere
// session must not disrupt either session's own bookkeeping, must not steal
// or alter the existing reservation, and must write exactly one audit event
// flagging the conflict for admin review.
#[sqlx::test(migrations = "./migrations")]
async fn test_concurrent_confirmed_join_elsewhere_flags_audit_without_disrupting_either_session(
    pool: PgPool,
) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let (apt_a, apt_b) = two_back_to_back_appointments_for_doctor1(&pool).await;
    let doctor_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");

    // Confirm A normally.
    assert_eq!(mint(&app, apt_a, &doctor_token).await, StatusCode::OK);
    let channel_a = channel_of(&pool, apt_a).await;
    let doctor_uid_a = derive_uid(&channel_a, UserId::from(uid(DOCTOR1_USER_ID)));
    assert!(
        presence_event(
            &state,
            &channel_a,
            doctor_uid_a,
            "e1",
            ProviderPresenceKind::Joined,
            1
        )
        .await
    );

    // Force B's session into existence directly (bypassing the mint-time
    // guard, which already independently prevents this — this test is
    // specifically about what happens if a provider event for B arrives
    // despite that, e.g. because of the physical-admission-ordering gap
    // documented in design doc §6) and apply a trusted Joined for the same
    // doctor in session B.
    sqlx::query(
        "INSERT INTO consultation_sessions (id, appointment_id, agora_channel_name, started_at, status, created_at) \
         VALUES (gen_random_uuid(), $1, $2, clock_timestamp(), 'ACTIVE', clock_timestamp())",
    )
    .bind(apt_b)
    .bind(format!("rtc_test_conflict_{}", Uuid::new_v4().simple()))
    .execute(&pool)
    .await
    .unwrap();
    let channel_b = channel_of(&pool, apt_b).await;
    let session_b = session_id_of(&pool, apt_b).await;
    let doctor_uid_b = derive_uid(&channel_b, UserId::from(uid(DOCTOR1_USER_ID)));
    let applied = presence_event(
        &state,
        &channel_b,
        doctor_uid_b,
        "e2",
        ProviderPresenceKind::Joined,
        1,
    )
    .await;
    assert!(
        applied,
        "the join itself is physically real and must still be recorded"
    );

    let (b_present,): (bool,) =
        sqlx::query_as("SELECT doctor_present FROM consultation_sessions WHERE id=$1")
            .bind(session_b)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        b_present,
        "session B's own presence bookkeeping must reflect the real join regardless of the reservation conflict"
    );

    let (reserved_appointment, reserved_session, reserved_state, _) =
        reservation_row(&pool, uid(DOCTOR1_USER_ID))
            .await
            .expect("reservation must still exist");
    assert_eq!(
        reserved_appointment, apt_a,
        "the conflict must not steal the reservation for B"
    );
    assert_eq!(reserved_session, session_id_of(&pool, apt_a).await);
    assert_eq!(reserved_state, "CONFIRMED");

    let audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_events WHERE action='CONCURRENT_LIVE_PARTICIPATION_DETECTED' AND resource_id=$1",
    )
    .bind(session_b)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        audit_count, 1,
        "exactly one conflict audit event must be written, referencing session B"
    );
}

// 11. G-31: new RTC token issuance is rejected once a session reaches
// NEEDS_REVIEW, for both participants. This proves only that NEW issuance
// is rejected — it does not and cannot prove an already-issued token stops
// working (see design doc §10).
#[sqlx::test(migrations = "./migrations")]
async fn test_rtc_token_reissue_rejected_after_needs_review(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor_token = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let (apt_a, _apt_b) = two_back_to_back_appointments_for_doctor1(&pool).await;
    // mint_agora_token auto-creates the consultation_sessions row on first
    // mint — it must exist before NEEDS_REVIEW can be set on it.
    assert_eq!(mint(&app, apt_a, &doctor_token).await, StatusCode::OK);
    sqlx::query(
        "UPDATE consultation_sessions SET recovery_state='NEEDS_REVIEW', status='PREMATURE_TERMINATION' WHERE appointment_id=$1",
    )
    .bind(apt_a)
    .execute(&pool)
    .await
    .unwrap();

    assert_eq!(mint(&app, apt_a, &doctor_token).await, StatusCode::CONFLICT);
    assert_eq!(
        mint(&app, apt_a, &patient_token).await,
        StatusCode::CONFLICT
    );
}

// 12. Regression guard: this feature is additive. The existing
// interruption/financial-preservation behavior already proven elsewhere
// must be byte-for-byte unchanged with the new table present.
#[sqlx::test(migrations = "./migrations")]
async fn test_reservation_logic_does_not_affect_financial_or_interruption_state(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let (appointment_id, _) = book_and_confirm(&app, &patient_token).await;
    let channel: String = channel_of(&pool, appointment_id).await;
    let doctor_uid = derive_uid(&channel, UserId::from(uid(DOCTOR1_USER_ID)));
    let patient_uid = derive_uid(&channel, UserId::from(uid(PATIENT_USER_ID)));

    assert!(
        presence_event(
            &state,
            &channel,
            patient_uid,
            "p1",
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
            "d1",
            ProviderPresenceKind::Joined,
            1
        )
        .await
    );
    assert!(
        presence_event(
            &state,
            &channel,
            patient_uid,
            "p2",
            ProviderPresenceKind::Left,
            2
        )
        .await
    );
    assert!(
        presence_event(
            &state,
            &channel,
            doctor_uid,
            "d2",
            ProviderPresenceKind::Left,
            2
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
    assert_eq!(
        hellodoctor_backend::services::consultation_presence_service::expire_empty_rooms(&state)
            .await
            .unwrap(),
        1
    );

    let (session_status, appointment_status, payment_status): (String, String, String) =
        sqlx::query_as(
            "SELECT cs.status::text, a.status::text, t.payment_status::text \
         FROM consultation_sessions cs \
         JOIN appointments a ON a.id = cs.appointment_id \
         JOIN transactions t ON t.appointment_id = a.id \
         WHERE cs.appointment_id = $1",
        )
        .bind(appointment_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(session_status, "PREMATURE_TERMINATION");
    assert_eq!(appointment_status, "IN_CONSULTATION");
    assert_eq!(payment_status, "PAYMENT_HELD");
}
