//! Phase 1B: the durable, asynchronous transactional-email outbox/worker —
//! atomic intent creation, idempotent enqueueing, safe concurrent claim,
//! bounded retry/backoff, permanent-vs-retryable classification, stale-lease
//! recovery, and cancellation/supersession. `transactional_email_triggers.rs`
//! already covers "does the right event trigger the right template"; this
//! file covers the delivery machinery itself, plus the two events that file
//! doesn't (consultation confirmed/rescheduled, payment confirmed, refund
//! processed, patient welcome).
use super::*;
use async_trait::async_trait;
use hellodoctor_backend::repository::email_outbox_repo;
use hellodoctor_backend::services::email_dispatch_worker;
use hellodoctor_backend::services::email_transport::{EmailMessage, EmailOutcome, EmailTransport};
use std::sync::Mutex;

/// Mirrors `email_outbox_delivery.rs`'s own `ScriptedEmailTransport` — each
/// call pops the next scripted outcome, recording every recipient/subject it
/// was asked to send so a test can assert both delivery classification and
/// the actual rendered content.
struct ScriptedEmailTransport {
    outcomes: Mutex<std::collections::VecDeque<EmailOutcome>>,
    sent: Mutex<Vec<(String, String, String)>>, // (recipient, subject, plain_text_body)
}

#[async_trait]
impl EmailTransport for ScriptedEmailTransport {
    fn is_configured(&self) -> bool {
        true
    }
    async fn send_email(&self, message: &EmailMessage<'_>) -> EmailOutcome {
        self.sent.lock().unwrap().push((
            message.recipient.to_string(),
            message.subject.to_string(),
            message.plain_text_body.to_string(),
        ));
        self.outcomes
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(EmailOutcome::SentToProvider {
                provider_message_id: None,
            })
    }
}

fn scripted(outcomes: Vec<EmailOutcome>) -> std::sync::Arc<ScriptedEmailTransport> {
    std::sync::Arc::new(ScriptedEmailTransport {
        outcomes: Mutex::new(outcomes.into()),
        sent: Mutex::new(Vec::new()),
    })
}

/// Unconfigured transport (`is_configured() == false`) — both `dispatch_now`
/// and `run_transactional_email_dispatch_pass` early-return without
/// attempting any send when this is installed, so a row enqueued under it
/// stays `CREATED` indefinitely. Used to isolate the *enqueue/supersede*
/// mechanics from delivery timing.
struct UnconfiguredTransport;
#[async_trait]
impl EmailTransport for UnconfiguredTransport {
    fn is_configured(&self) -> bool {
        false
    }
    async fn send_email(&self, _message: &EmailMessage<'_>) -> EmailOutcome {
        EmailOutcome::NoProviderConfigured
    }
}

async fn set_patient_email(pool: &PgPool, email: &str) {
    sqlx::query("UPDATE users SET email=$1 WHERE id=$2")
        .bind(email)
        .bind(uid(PATIENT_USER_ID))
        .execute(pool)
        .await
        .unwrap();
}

async fn outbox_row(
    pool: &PgPool,
    email_type: &str,
    related_entity_id: Uuid,
) -> Option<EmailOutboxRowForTest> {
    sqlx::query_as(
        "SELECT id, delivery_status, delivery_attempts, recipient_email, claimed_until, next_attempt_at \
         FROM email_outbox WHERE email_type=$1 AND related_entity_id=$2 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(email_type)
    .bind(related_entity_id)
    .fetch_optional(pool)
    .await
    .unwrap()
}

#[derive(Debug, sqlx::FromRow)]
struct EmailOutboxRowForTest {
    id: Uuid,
    delivery_status: String,
    delivery_attempts: i32,
    recipient_email: String,
    claimed_until: Option<chrono::DateTime<Utc>>,
    next_attempt_at: chrono::DateTime<Utc>,
}

async fn wait_for_terminal(
    pool: &PgPool,
    email_type: &str,
    related_entity_id: Uuid,
) -> EmailOutboxRowForTest {
    for _ in 0..100 {
        if let Some(row) = outbox_row(pool, email_type, related_entity_id).await {
            if row.delivery_status != "CREATED" {
                return row;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!(
        "{email_type} outbox row for entity {related_entity_id} never reached a terminal status"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn booking_confirmation_enqueues_and_sends_confirmed_and_payment_emails(pool: PgPool) {
    let (_, pool, mut state) = test_app_and_state(pool).await;
    set_patient_email(&pool, "amani.booking@example.invalid").await;
    state.email_transport = scripted(vec![
        EmailOutcome::SentToProvider {
            provider_message_id: None,
        },
        EmailOutcome::SentToProvider {
            provider_message_id: None,
        },
    ]);
    let app = hellodoctor_backend::api::create_router(state);
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");

    let (apt_id, _) = book_and_confirm(&app, &patient_token).await;

    let confirmed = wait_for_terminal(&pool, "CONSULTATION_CONFIRMED", apt_id).await;
    assert_eq!(confirmed.delivery_status, "SENT_TO_PROVIDER");
    assert_eq!(confirmed.recipient_email, "amani.booking@example.invalid");

    let txn_id: Uuid = sqlx::query_scalar("SELECT id FROM transactions WHERE appointment_id=$1")
        .bind(apt_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let payment = wait_for_terminal(&pool, "PAYMENT_CONFIRMED", txn_id).await;
    assert_eq!(payment.delivery_status, "SENT_TO_PROVIDER");
    assert_eq!(payment.recipient_email, "amani.booking@example.invalid");
}

/// Consultation ID correctness + no clinical content: the rendered
/// CONSULTATION_CONFIRMED email must carry the real appointment number and
/// must never carry the chief complaint submitted at booking.
#[sqlx::test(migrations = "./migrations")]
async fn consultation_confirmed_email_carries_the_real_consultation_id_never_the_chief_complaint(
    pool: PgPool,
) {
    let (_, pool, mut state) = test_app_and_state(pool).await;
    set_patient_email(&pool, "amani.content@example.invalid").await;
    let transport = scripted(vec![
        EmailOutcome::SentToProvider {
            provider_message_id: None,
        },
        EmailOutcome::SentToProvider {
            provider_message_id: None,
        },
    ]);
    state.email_transport = transport.clone();
    let app = hellodoctor_backend::api::create_router(state);
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");

    let (apt_id, _) = book_and_confirm(&app, &patient_token).await;
    wait_for_terminal(&pool, "CONSULTATION_CONFIRMED", apt_id).await;

    let appointment_number: String =
        sqlx::query_scalar("SELECT appointment_number FROM appointments WHERE id=$1")
            .bind(apt_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let sent = transport.sent.lock().unwrap().clone();
    let confirmed_body = sent
        .iter()
        .find(|(_, subject, _)| !subject.to_lowercase().contains("payment"))
        .map(|(_, _, body)| body.clone())
        .expect("a non-payment (consultation confirmed) email must have been sent");
    assert!(
        confirmed_body.contains(&appointment_number),
        "confirmed email must carry the real consultation/appointment id"
    );
    // `book_and_confirm` always submits a real chief complaint
    // ("Persistent fever and joint pain") — it must never leak into any
    // patient-facing transactional email.
    for (_, _, body) in &sent {
        assert!(
            !body.to_lowercase().contains("fever"),
            "transactional email must never disclose the clinical chief complaint"
        );
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn booking_without_patient_email_enqueues_no_confirmation_emails(pool: PgPool) {
    let (_, pool, mut state) = test_app_and_state(pool).await;
    // Seeded patient has no email by default — left untouched.
    state.email_transport = scripted(vec![]);
    let app = hellodoctor_backend::api::create_router(state);
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");

    let (apt_id, _) = book_and_confirm(&app, &patient_token).await;

    assert!(outbox_row(&pool, "CONSULTATION_CONFIRMED", apt_id)
        .await
        .is_none());
    let txn_id: Uuid = sqlx::query_scalar("SELECT id FROM transactions WHERE appointment_id=$1")
        .bind(apt_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(outbox_row(&pool, "PAYMENT_CONFIRMED", txn_id)
        .await
        .is_none());
}

#[sqlx::test(migrations = "./migrations")]
async fn patient_registration_enqueues_and_sends_welcome_email(pool: PgPool) {
    let (_, pool, mut state) = test_app_and_state(pool).await;
    state.email_transport = scripted(vec![EmailOutcome::SentToProvider {
        provider_message_id: None,
    }]);
    let app = hellodoctor_backend::api::create_router(state);

    let register = post(
        &app,
        "/api/v1/auth/patient/register",
        None,
        &serde_json::json!({
            "email": "welcome.fixture.patient@example.invalid",
            "phone_number": "+254711000222",
            "password": "Str0ng!OldPassw0rd",
            "full_name": "Welcome Fixture Patient",
            "date_of_birth": "1992-04-01",
            "gender": "FEMALE",
        }),
    )
    .await;
    assert_eq!(register.status(), StatusCode::OK);
    let register_body = json_body(register).await;
    let verify = post(
        &app,
        "/api/v1/auth/patient/verify-otp",
        None,
        &serde_json::json!({
            "challenge_token": register_body["data"]["challenge_token"].as_str().unwrap(),
            "otp_code": register_body["data"]["debug_otp_code"].as_str().unwrap(),
            "accepted_documents": [
                {"type": "PATIENT_DATA_SHARING_CONSENT", "version": "1.0-afyamkononi"},
                {"type": "PATIENT_TERMS", "version": "1.0-afyamkononi"},
                {"type": "PATIENT_PRIVACY_NOTICE", "version": "1.0-afyamkononi"},
                {"type": "PATIENT_DATA_RIGHTS", "version": "1.0"},
            ],
        }),
    )
    .await;
    assert_eq!(verify.status(), StatusCode::OK);
    let user_id: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE email=$1")
        .bind("welcome.fixture.patient@example.invalid")
        .fetch_one(&pool)
        .await
        .unwrap();

    let row = wait_for_terminal(&pool, "PATIENT_WELCOME", user_id).await;
    assert_eq!(row.delivery_status, "SENT_TO_PROVIDER");
    assert_eq!(
        row.recipient_email,
        "welcome.fixture.patient@example.invalid"
    );
}

/// Cancellation/supersession ordering: a reschedule must mark an earlier,
/// still-pending CONSULTATION_CONFIRMED row for the same appointment
/// `SUPERSEDED` so it is never claimed/sent after the fact. Uses an
/// unconfigured transport so neither row is ever actually dispatched —
/// isolating the supersession mechanic itself from delivery timing.
#[sqlx::test(migrations = "./migrations")]
async fn reschedule_supersedes_a_still_pending_confirmed_email(pool: PgPool) {
    let (_, pool, mut state) = test_app_and_state(pool).await;
    set_patient_email(&pool, "amani.reschedule@example.invalid").await;
    state.email_transport = std::sync::Arc::new(UnconfiguredTransport);
    let app = hellodoctor_backend::api::create_router(state);
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let admin_token = make_token(uid(ADMIN_USER_ID), "ADMIN");

    let (apt_id, _) = book_and_confirm(&app, &patient_token).await;
    let confirmed_before = outbox_row(&pool, "CONSULTATION_CONFIRMED", apt_id)
        .await
        .expect("CONSULTATION_CONFIRMED must have been enqueued");
    assert_eq!(
        confirmed_before.delivery_status, "CREATED",
        "the unconfigured transport must leave this row pending, not sent"
    );

    let new_slot: Uuid = sqlx::query_scalar(
        "SELECT id FROM doctor_schedule_slots WHERE doctor_id=$1 AND status='AVAILABLE' ORDER BY start_time LIMIT 1",
    )
    .bind(uid(DOCTOR1_PROFILE_ID))
    .fetch_one(&pool)
    .await
    .unwrap();
    let reschedule = post(
        &app,
        &format!("/api/v1/admin/appointments/{apt_id}/reschedule"),
        Some(&admin_token),
        &serde_json::json!({ "slot_id": new_slot, "reason": "Doctor requested a later slot" }),
    )
    .await;
    assert_eq!(reschedule.status(), StatusCode::OK);

    let confirmed_after: String =
        sqlx::query_scalar("SELECT delivery_status FROM email_outbox WHERE id=$1")
            .bind(confirmed_before.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        confirmed_after, "SUPERSEDED",
        "a pending CONFIRMED row must be superseded by the newer reschedule event"
    );
    let rescheduled = outbox_row(&pool, "CONSULTATION_RESCHEDULED", apt_id).await;
    assert!(
        rescheduled.is_none(),
        "CONSULTATION_RESCHEDULED is keyed by the reschedule event id, not the appointment id"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn refund_processed_email_is_sent_only_once_the_refund_actually_completes(pool: PgPool) {
    let (_, pool, mut state) = test_app_and_state(pool).await;
    set_patient_email(&pool, "amani.refund@example.invalid").await;
    state.email_transport = scripted(vec![
        EmailOutcome::SentToProvider {
            provider_message_id: None,
        },
        EmailOutcome::SentToProvider {
            provider_message_id: None,
        },
        EmailOutcome::SentToProvider {
            provider_message_id: None,
        },
    ]);
    let app = hellodoctor_backend::api::create_router(state);
    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let admin_token = make_token(uid(ADMIN_USER_ID), "ADMIN");

    let (apt_id, _) = book_and_confirm(&app, &patient_token).await;
    let session =
        hellodoctor_backend::repository::consultation_repo::find_by_appointment(&pool, apt_id)
            .await
            .unwrap()
            .unwrap();
    let grievance = json_body(
        post(
            &app,
            "/api/v1/grievances",
            Some(&patient_token),
            &serde_json::json!({
                "consultation_id": session.id,
                "target": "DOCTOR",
                "category": "Rushed Consultation / Ended Abruptly",
                "claim_summary": "Doctor disconnected after 45 seconds",
            }),
        )
        .await,
    )
    .await;
    let grv_id = Uuid::parse_str(grievance["data"]["grievance_id"].as_str().unwrap()).unwrap();
    post(
        &app,
        &format!("/api/v1/admin/grievances/{grv_id}/refund"),
        Some(&admin_token),
        &serde_json::json!({ "audit_notes": "Substantiated." }),
    )
    .await;
    let list = json_body(
        get(
            &app,
            "/api/v1/admin/finance/refund-requests",
            Some(&admin_token),
        )
        .await,
    )
    .await;
    let refund_id = Uuid::parse_str(list["data"][0]["id"].as_str().unwrap()).unwrap();

    // APPROVED -> PROCESSING must not send anything yet.
    let processing = post(
        &app,
        &format!("/api/v1/admin/finance/refund-requests/{refund_id}/transition"),
        Some(&admin_token),
        &serde_json::json!({ "status": "PROCESSING" }),
    )
    .await;
    assert_eq!(processing.status(), StatusCode::OK);
    assert!(
        outbox_row(&pool, "REFUND_PROCESSED", refund_id)
            .await
            .is_none(),
        "no refund email before the refund actually completes"
    );

    // PROCESSING -> REFUNDED is the real completion and must send.
    let refunded = post(
        &app,
        &format!("/api/v1/admin/finance/refund-requests/{refund_id}/transition"),
        Some(&admin_token),
        &serde_json::json!({ "status": "REFUNDED", "provider_reference": "MPESA-CONF-998877" }),
    )
    .await;
    assert_eq!(refunded.status(), StatusCode::OK);
    let row = wait_for_terminal(&pool, "REFUND_PROCESSED", refund_id).await;
    assert_eq!(row.delivery_status, "SENT_TO_PROVIDER");
    assert_eq!(row.recipient_email, "amani.refund@example.invalid");
}

/// Atomic rollback: if the business transaction that enqueued the row never
/// commits, the row must never exist — not even transiently for another
/// connection to observe.
#[sqlx::test(migrations = "./migrations")]
async fn a_rolled_back_transaction_leaves_no_outbox_row_behind(pool: PgPool) {
    let (_, pool, state) = test_app_and_state(pool).await;
    let outbox_id = Uuid::new_v4();
    let entity_id = Uuid::new_v4();
    let dedup_key = format!("rollback-test:{entity_id}");

    let mut tx = state.auth_db.begin().await.unwrap();
    email_outbox_repo::enqueue_transactional(
        &mut *tx,
        outbox_id,
        "PATIENT_WELCOME",
        "rollback@example.invalid",
        &dedup_key,
        "USER",
        entity_id,
        email_outbox_repo::transactional_email_expiry(),
    )
    .await
    .unwrap();
    // The business transaction fails for some unrelated reason and is never
    // committed.
    tx.rollback().await.unwrap();

    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM email_outbox WHERE id=$1)")
        .bind(outbox_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(
        !exists,
        "a rolled-back business transaction must never leave its email intent behind"
    );
}

/// Business success despite a permanently failing provider: the doctor
/// approval itself must still succeed even though its notification email
/// can never be delivered.
#[sqlx::test(migrations = "./migrations")]
async fn doctor_approval_succeeds_even_when_email_delivery_permanently_fails(pool: PgPool) {
    let (_, pool, mut state) = test_app_and_state(pool).await;
    state.email_transport = scripted(vec![EmailOutcome::Permanent {
        reason: "simulated hard provider rejection".into(),
    }]);
    let app = hellodoctor_backend::api::create_router(state);

    sqlx::query(
        "UPDATE doctor_profiles SET professional_verification_status='PENDING' WHERE id=$1",
    )
    .bind(uid(DOCTOR1_PROFILE_ID))
    .execute(&pool)
    .await
    .unwrap();
    let admin_token = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let response = post(
        &app,
        &format!("/api/v1/admin/doctors/{DOCTOR1_PROFILE_ID}/verify"),
        Some(&admin_token),
        &serde_json::json!({ "note": "license cross-checked" }),
    )
    .await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "the business event (doctor verification) must succeed independently of email delivery"
    );
    let verified: String = sqlx::query_scalar(
        "SELECT professional_verification_status::text FROM doctor_profiles WHERE id=$1",
    )
    .bind(uid(DOCTOR1_PROFILE_ID))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(verified, "VERIFIED");

    let row = wait_for_terminal(&pool, "DOCTOR_APPROVED", uid(DOCTOR1_PROFILE_ID)).await;
    assert_eq!(row.delivery_status, "FAILED");
    assert_eq!(
        row.delivery_attempts, 1,
        "a permanent outcome must not retry"
    );
}

/// Retry/backoff: a retryable outcome must not be immediately reclaimed —
/// `next_attempt_at` must genuinely gate the next claim, not just exist as a
/// column nobody checks.
#[sqlx::test(migrations = "./migrations")]
async fn retryable_outcome_schedules_backoff_and_is_not_immediately_reclaimed(pool: PgPool) {
    let (_, pool, state) = test_app_and_state(pool).await;
    let transport = scripted(vec![EmailOutcome::Retryable {
        reason: "transient".into(),
    }]);
    let mut state = state;
    state.email_transport = transport.clone();

    let outbox_id = Uuid::new_v4();
    // A real patient, not a random id: PATIENT_WELCOME's resolver looks up
    // this entity via `patient_repo::find_by_user_id` at dispatch time, so
    // a non-existent id would resolve to `None` and be marked permanently
    // failed before `send_email` is ever called — testing the wrong thing.
    let entity_id = uid(PATIENT_USER_ID);
    email_outbox_repo::enqueue_transactional(
        &state.system_db,
        outbox_id,
        "PATIENT_WELCOME",
        "retry@example.invalid",
        &format!("retry-test:{entity_id}"),
        "USER",
        entity_id,
        email_outbox_repo::transactional_email_expiry(),
    )
    .await
    .unwrap();

    let claimed = email_dispatch_worker::run_transactional_email_dispatch_pass(&state)
        .await
        .unwrap();
    assert_eq!(claimed, 1);
    assert_eq!(transport.sent.lock().unwrap().len(), 1);

    let row = outbox_row(&pool, "PATIENT_WELCOME", entity_id)
        .await
        .unwrap();
    assert_eq!(
        row.delivery_status, "CREATED",
        "still retryable, not terminal"
    );
    assert_eq!(row.delivery_attempts, 1);
    assert!(
        row.next_attempt_at > Utc::now(),
        "backoff must push the next attempt into the future"
    );
    assert!(
        row.claimed_until.is_none(),
        "the lease must be released after the attempt"
    );

    // A second pass immediately afterward must not re-send: the row is not
    // yet due.
    let claimed_again = email_dispatch_worker::run_transactional_email_dispatch_pass(&state)
        .await
        .unwrap();
    assert_eq!(claimed_again, 0);
    assert_eq!(
        transport.sent.lock().unwrap().len(),
        1,
        "a not-yet-due row must not be reclaimed and resent"
    );
}

/// Stale-lease recovery: a claim abandoned by a crashed worker (lease
/// expired, row never reached a terminal status) must be reclaimable by a
/// later pass.
#[sqlx::test(migrations = "./migrations")]
async fn a_stale_lease_is_reclaimed_by_a_later_pass(pool: PgPool) {
    let (_, pool, state) = test_app_and_state(pool).await;
    let transport = scripted(vec![EmailOutcome::SentToProvider {
        provider_message_id: None,
    }]);
    let mut state = state;
    state.email_transport = transport.clone();

    let outbox_id = Uuid::new_v4();
    let entity_id = uid(PATIENT_USER_ID);
    email_outbox_repo::enqueue_transactional(
        &state.system_db,
        outbox_id,
        "PATIENT_WELCOME",
        "stale-lease@example.invalid",
        &format!("stale-lease-test:{entity_id}"),
        "USER",
        entity_id,
        email_outbox_repo::transactional_email_expiry(),
    )
    .await
    .unwrap();
    // Simulate a worker that claimed this row and then crashed before
    // completing it: an expired lease, row still CREATED.
    sqlx::query("UPDATE email_outbox SET claimed_until=NOW() - INTERVAL '1 minute' WHERE id=$1")
        .bind(outbox_id)
        .execute(&pool)
        .await
        .unwrap();

    let claimed = email_dispatch_worker::run_transactional_email_dispatch_pass(&state)
        .await
        .unwrap();
    assert_eq!(
        claimed, 1,
        "a row with an expired lease must be reclaimable"
    );
    let row = outbox_row(&pool, "PATIENT_WELCOME", entity_id)
        .await
        .unwrap();
    assert_eq!(row.delivery_status, "SENT_TO_PROVIDER");
}

/// Safe concurrent claim: two dispatch passes racing against the exact same
/// due row must never both send it — `FOR UPDATE SKIP LOCKED` must actually
/// serialize them.
#[sqlx::test(migrations = "./migrations")]
async fn concurrent_dispatch_passes_never_double_send_the_same_row(pool: PgPool) {
    let (_, _pool, state) = test_app_and_state(pool).await;
    let transport = scripted(vec![
        EmailOutcome::SentToProvider {
            provider_message_id: None,
        },
        EmailOutcome::SentToProvider {
            provider_message_id: None,
        },
    ]);
    let mut state = state;
    state.email_transport = transport.clone();

    let outbox_id = Uuid::new_v4();
    let entity_id = uid(PATIENT_USER_ID);
    email_outbox_repo::enqueue_transactional(
        &state.system_db,
        outbox_id,
        "PATIENT_WELCOME",
        "concurrent@example.invalid",
        &format!("concurrent-test:{entity_id}"),
        "USER",
        entity_id,
        email_outbox_repo::transactional_email_expiry(),
    )
    .await
    .unwrap();

    let state_a = state.clone();
    let state_b = state.clone();
    let (claimed_a, claimed_b) = tokio::join!(
        email_dispatch_worker::run_transactional_email_dispatch_pass(&state_a),
        email_dispatch_worker::run_transactional_email_dispatch_pass(&state_b),
    );
    let total_claimed = claimed_a.unwrap() + claimed_b.unwrap();
    assert_eq!(
        total_claimed, 1,
        "only one of the two concurrent passes may claim this row"
    );
    assert_eq!(
        transport.sent.lock().unwrap().len(),
        1,
        "the row must be sent exactly once despite the race"
    );
}
