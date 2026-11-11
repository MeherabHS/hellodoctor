use super::*;
use async_trait::async_trait;
use hellodoctor_backend::repository::notification_repo;
use hellodoctor_backend::services::notification_scheduler::run_push_delivery_pass;
use hellodoctor_backend::services::notification_transport::{
    NotificationTransport, PushOptions, PushOutcome,
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tokio::sync::Semaphore;

struct ActiveSend<'a>(&'a AtomicUsize);
impl Drop for ActiveSend<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

struct ControlledTransport {
    calls: AtomicUsize,
    active: AtomicUsize,
    maximum_active: AtomicUsize,
    entered: Semaphore,
    release: Semaphore,
}

impl ControlledTransport {
    fn new(permits: usize) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            active: AtomicUsize::new(0),
            maximum_active: AtomicUsize::new(0),
            entered: Semaphore::new(0),
            release: Semaphore::new(permits),
        }
    }
    async fn wait_for_send(&self) {
        tokio::time::timeout(std::time::Duration::from_secs(3), self.entered.acquire())
            .await
            .unwrap()
            .unwrap()
            .forget();
    }
}

#[async_trait]
impl NotificationTransport for ControlledTransport {
    fn is_configured(&self) -> bool {
        true
    }
    async fn send_push(&self, _: &str, _: &str, _: &str, _: &PushOptions) -> PushOutcome {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        let _active = ActiveSend(&self.active);
        self.maximum_active.fetch_max(active, Ordering::SeqCst);
        self.entered.add_permits(1);
        self.release.acquire().await.unwrap().forget();
        PushOutcome::SentToProvider {
            provider_message_id: Some("synthetic-remediation-c".into()),
        }
    }
}

async fn fixture(
    pool: PgPool,
    transport: Arc<dyn NotificationTransport>,
    kind: &str,
) -> (axum::Router, PgPool, AppState, Uuid) {
    let (app, pool, mut state) = test_app_and_state(pool).await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    assert_eq!(
        post(
            &app,
            "/api/v1/notifications/devices",
            Some(&doctor),
            &serde_json::json!({"platform":"android","push_token":"synthetic-c-device"})
        )
        .await
        .status(),
        StatusCode::CREATED
    );
    let id = Uuid::new_v4();
    notification_repo::insert_if_new(
        &pool,
        id,
        UserId::from(uid(DOCTOR1_USER_ID)),
        kind,
        "Synthetic test",
        "No clinical content",
        None,
        None,
        None,
        &format!("remediation-c:{id}"),
        None,
    )
    .await
    .unwrap();
    state.notification_transport = transport;
    (app, pool, state, id)
}

#[sqlx::test(migrations = "./migrations")]
async fn test_c1_permanent_suppression_never_reclaimed(pool: PgPool) {
    let transport = Arc::new(ControlledTransport::new(10));
    let (_app, pool, state, id) = fixture(pool, transport.clone(), "SERVICE_ANNOUNCEMENT").await;
    sqlx::query(
        "INSERT INTO notification_preferences(user_id,operational_enabled) VALUES($1,false)",
    )
    .bind(uid(DOCTOR1_USER_ID))
    .execute(&pool)
    .await
    .unwrap();
    run_push_delivery_pass(&state).await.unwrap();
    assert_eq!(delivery(&pool, id).await.0, "SUPPRESSED");
    assert_eq!(transport.calls.load(Ordering::SeqCst), 0);
    let first: i32 = sqlx::query_scalar("SELECT delivery_attempts FROM notifications WHERE id=$1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE notification_preferences SET operational_enabled=true WHERE user_id=$1")
        .bind(uid(DOCTOR1_USER_ID))
        .execute(&pool)
        .await
        .unwrap();
    run_push_delivery_pass(&state).await.unwrap();
    assert_eq!(
        transport.calls.load(Ordering::SeqCst),
        0,
        "a terminally suppressed event must not be resurrected when preferences later change"
    );
    let after: i32 = sqlx::query_scalar("SELECT delivery_attempts FROM notifications WHERE id=$1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(first, after);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_c2_expired_lease_does_not_overlap_live_workers(pool: PgPool) {
    let transport = Arc::new(ControlledTransport::new(0));
    let (_app, pool, state, id) = fixture(pool, transport.clone(), "EARNING_CREDITED").await;
    let first_state = state.clone();
    let first = tokio::spawn(async move { run_push_delivery_pass(&first_state).await });
    transport.wait_for_send().await;
    sqlx::query("UPDATE notifications SET next_delivery_at=clock_timestamp()-interval '1 second', delivery_lease_until=clock_timestamp()-interval '1 second' WHERE id=$1").bind(id).execute(&pool).await.unwrap();
    let second_state = state.clone();
    let second = tokio::spawn(async move { run_push_delivery_pass(&second_state).await });
    let overlapped = tokio::time::timeout(
        std::time::Duration::from_millis(250),
        transport.entered.acquire(),
    )
    .await
    .is_ok();
    transport.release.add_permits(4);
    first.await.unwrap().unwrap();
    second.await.unwrap().unwrap();
    assert!(
        !overlapped,
        "expired lease allowed a second worker to enter FCM while the first remained active"
    );
    assert_eq!(transport.maximum_active.load(Ordering::SeqCst), 1);
    assert_eq!(transport.calls.load(Ordering::SeqCst), 1);
}

async fn expire(pool: &PgPool, id: Uuid) {
    sqlx::query("UPDATE notifications SET delivery_lease_until=CASE WHEN delivery_claim_token IS NULL THEN NULL ELSE clock_timestamp()-interval '1 second' END,next_delivery_at=clock_timestamp()-interval '1 second' WHERE id=$1")
        .bind(id).execute(pool).await.unwrap();
}

async fn delivery(pool: &PgPool, id: Uuid) -> (String, i32, bool) {
    sqlx::query_as("SELECT delivery_status,delivery_attempts,delivery_uncertain FROM notifications WHERE id=$1")
        .bind(id).fetch_one(pool).await.unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn test_claim_fencing_all_completions_and_duplicate_ack(pool: PgPool) {
    let (_app, pool, state, id) = fixture(
        pool,
        Arc::new(ControlledTransport::new(0)),
        "EARNING_CREDITED",
    )
    .await;
    let first = notification_repo::claim_next_notification(&state.system_db)
        .await
        .unwrap()
        .unwrap();
    let old = first.row.delivery_claim_token;
    assert!(notification_repo::claim_next_notification(&state.system_db)
        .await
        .unwrap()
        .is_none());
    first.release().await.unwrap();
    expire(&pool, id).await;
    assert!(
        !notification_repo::renew_delivery_claim(&pool, id, old)
            .await
            .unwrap(),
        "a connection without the session lock cannot renew an expired claim"
    );
    let mut second = notification_repo::claim_next_notification(&state.system_db)
        .await
        .unwrap()
        .unwrap();
    let new = second.row.delivery_claim_token;
    assert_ne!(old, new);
    assert!(
        !notification_repo::mark_delivery_sent(&pool, id, old, Some("stale"))
            .await
            .unwrap()
    );
    assert!(
        !notification_repo::mark_delivery_retry(&pool, id, old, "stale", Utc::now())
            .await
            .unwrap()
    );
    assert!(
        !notification_repo::mark_delivery_permanent(&pool, id, old, "stale")
            .await
            .unwrap()
    );
    assert!(
        !notification_repo::mark_delivery_suppressed(&pool, id, old, "stale")
            .await
            .unwrap()
    );
    assert!(!notification_repo::mark_delivery_deferred(
        &pool,
        id,
        old,
        Utc::now(),
        "Deferred: stale",
        false
    )
    .await
    .unwrap());
    assert!(!notification_repo::begin_provider_send(&pool, id, old)
        .await
        .unwrap());
    assert!(notification_repo::mark_delivery_sent(
        second.connection(),
        id,
        new,
        Some("authoritative")
    )
    .await
    .unwrap());
    assert!(!notification_repo::mark_delivery_sent(
        second.connection(),
        id,
        new,
        Some("duplicate")
    )
    .await
    .unwrap());
    second.release().await.unwrap();
    let (status, provider): (String, String) =
        sqlx::query_as("SELECT delivery_status,provider_message_id FROM notifications WHERE id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        (status.as_str(), provider.as_str()),
        ("SENT_TO_PROVIDER", "authoritative")
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_worker_interruption_durable_uncertainty_and_restart(pool: PgPool) {
    let transport = Arc::new(ControlledTransport::new(0));
    let (_app, pool, state, id) = fixture(pool, transport.clone(), "EARNING_CREDITED").await;
    let worker_state = state.clone();
    let worker = tokio::spawn(async move { run_push_delivery_pass(&worker_state).await });
    transport.wait_for_send().await;
    assert_eq!(delivery(&pool, id).await, ("DUE".into(), 1, true));
    let open_transactions:i64=sqlx::query_scalar("SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() AND usename='hellodoctor_system_worker' AND state='idle in transaction'").fetch_one(&pool).await.unwrap();
    assert_eq!(
        open_transactions, 0,
        "provider wait must not hold a database transaction"
    );
    worker.abort();
    assert!(worker.await.unwrap_err().is_cancelled());
    expire(&pool, id).await;
    transport.release.add_permits(1);
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            if run_push_delivery_pass(&state).await.unwrap() > 0 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        delivery(&pool, id).await,
        ("SENT_TO_PROVIDER".into(), 2, false)
    );
    assert_eq!(transport.maximum_active.load(Ordering::SeqCst), 1);
    assert_eq!(
        transport.calls.load(Ordering::SeqCst),
        2,
        "crash recovery is at-least-once, not provider exactly-once"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_slow_send_renews_lease_and_excludes_second_worker(pool: PgPool) {
    let transport = Arc::new(ControlledTransport::new(0));
    let (_app, pool, state, id) = fixture(pool, transport.clone(), "EARNING_CREDITED").await;
    let worker_state = state.clone();
    let worker = tokio::spawn(async move { run_push_delivery_pass(&worker_state).await });
    transport.wait_for_send().await;
    sqlx::query("UPDATE notifications SET delivery_lease_until=clock_timestamp()+interval '20 seconds' WHERE id=$1").bind(id).execute(&pool).await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(14),async{
        loop{
            let renewed:bool=sqlx::query_scalar("SELECT delivery_lease_until>clock_timestamp()+interval '100 seconds' FROM notifications WHERE id=$1").bind(id).fetch_one(&pool).await.unwrap();
            if renewed{break;}
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    }).await.unwrap();
    assert_eq!(run_push_delivery_pass(&state).await.unwrap(), 0);
    transport.release.add_permits(1);
    worker.await.unwrap().unwrap();
    assert_eq!(delivery(&pool, id).await.0, "SENT_TO_PROVIDER");
    assert_eq!(transport.calls.load(Ordering::SeqCst), 1);
}

struct FixedTransport {
    outcome: PushOutcome,
    calls: AtomicUsize,
}
#[async_trait]
impl NotificationTransport for FixedTransport {
    fn is_configured(&self) -> bool {
        true
    }
    async fn send_push(&self, _: &str, _: &str, _: &str, _: &PushOptions) -> PushOutcome {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.outcome.clone()
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn test_retry_budget_and_terminal_failure_never_reclaimed(pool: PgPool) {
    let transport = Arc::new(FixedTransport {
        outcome: PushOutcome::Retryable {
            reason: "synthetic unavailable".into(),
        },
        calls: AtomicUsize::new(0),
    });
    let (_app, pool, state, id) = fixture(pool, transport.clone(), "EARNING_CREDITED").await;
    for attempt in 1..=5 {
        assert_eq!(run_push_delivery_pass(&state).await.unwrap(), 1);
        assert_eq!(delivery(&pool, id).await, ("FAILED".into(), attempt, true));
        let next: Option<DateTime<Utc>> =
            sqlx::query_scalar("SELECT next_delivery_at FROM notifications WHERE id=$1")
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap();
        if attempt < 5 {
            assert!(next.unwrap() > Utc::now());
            expire(&pool, id).await;
        } else {
            assert!(next.is_none());
        }
    }
    assert_eq!(run_push_delivery_pass(&state).await.unwrap(), 0);
    assert_eq!(transport.calls.load(Ordering::SeqCst), 5);
    let stats = notification_repo::delivery_queue_stats(&pool)
        .await
        .unwrap();
    assert_eq!(
        (stats.retrying, stats.permanently_failed, stats.uncertain),
        (0, 1, 1)
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_legacy_terminal_rows_are_not_replayed_or_rewritten(pool: PgPool) {
    let transport = Arc::new(ControlledTransport::new(10));
    let (_app, pool, state, id) = fixture(pool, transport.clone(), "SERVICE_ANNOUNCEMENT").await;
    sqlx::query("UPDATE notifications SET delivery_status='FAILED',last_delivery_error='Suppressed: legacy opt-out',next_delivery_at=NULL WHERE id=$1").bind(id).execute(&pool).await.unwrap();
    assert_eq!(run_push_delivery_pass(&state).await.unwrap(), 0);
    assert_eq!(delivery(&pool, id).await, ("FAILED".into(), 0, false));
    let stats = notification_repo::delivery_queue_stats(&pool)
        .await
        .unwrap();
    assert_eq!((stats.suppressed, stats.permanently_failed), (1, 0));
    assert_eq!(transport.calls.load(Ordering::SeqCst), 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_quiet_hours_deferral_does_not_exhaust_retry_budget(pool: PgPool) {
    let transport = Arc::new(ControlledTransport::new(10));
    let (_app, pool, state, id) = fixture(pool, transport.clone(), "SERVICE_ANNOUNCEMENT").await;
    sqlx::query("INSERT INTO notification_preferences(user_id,quiet_hours_enabled,quiet_hours_start_local,quiet_hours_end_local) VALUES($1,true,'00:00','23:59:59')").bind(uid(DOCTOR1_USER_ID)).execute(&pool).await.unwrap();
    for _ in 0..6 {
        run_push_delivery_pass(&state).await.unwrap();
        assert_eq!(delivery(&pool, id).await, ("FAILED".into(), 0, false));
        assert_eq!(
            notification_repo::delivery_queue_stats(&pool)
                .await
                .unwrap()
                .deferred,
            1
        );
        expire(&pool, id).await;
    }
    assert_eq!(transport.calls.load(Ordering::SeqCst), 0);
    let reserved: i64 = sqlx::query_scalar(
        "SELECT COALESCE(sum(sent_count),0)::bigint FROM notification_frequency_counters",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(reserved, 0);
    sqlx::query("UPDATE notification_preferences SET quiet_hours_enabled=false WHERE user_id=$1")
        .bind(uid(DOCTOR1_USER_ID))
        .execute(&pool)
        .await
        .unwrap();
    run_push_delivery_pass(&state).await.unwrap();
    assert_eq!(
        delivery(&pool, id).await,
        ("SENT_TO_PROVIDER".into(), 1, false)
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_expired_final_attempt_becomes_terminal_without_another_send(pool: PgPool) {
    let transport = Arc::new(ControlledTransport::new(10));
    let (_app, pool, state, id) = fixture(pool, transport.clone(), "EARNING_CREDITED").await;
    let mut claim = notification_repo::claim_next_notification(&state.system_db)
        .await
        .unwrap()
        .unwrap();
    let token = claim.row.delivery_claim_token;
    assert!(
        notification_repo::begin_provider_send(claim.connection(), id, token)
            .await
            .unwrap()
    );
    claim.release().await.unwrap();
    sqlx::query("UPDATE notifications SET delivery_attempts=5 WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    expire(&pool, id).await;
    assert_eq!(run_push_delivery_pass(&state).await.unwrap(), 1);
    assert_eq!(delivery(&pool, id).await, ("FAILED".into(), 6, true));
    assert_eq!(run_push_delivery_pass(&state).await.unwrap(), 0);
    assert_eq!(transport.calls.load(Ordering::SeqCst), 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_preference_change_during_send_stops_pending_work_truthfully(pool: PgPool) {
    let transport = Arc::new(ControlledTransport::new(0));
    let (_app, pool, state, id) = fixture(pool, transport.clone(), "SERVICE_ANNOUNCEMENT").await;
    let worker_state = state.clone();
    let worker = tokio::spawn(async move { run_push_delivery_pass(&worker_state).await });
    transport.wait_for_send().await;
    sqlx::query(
        "INSERT INTO notification_preferences(user_id,operational_enabled) VALUES($1,false)",
    )
    .bind(uid(DOCTOR1_USER_ID))
    .execute(&pool)
    .await
    .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(14), worker)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(
        delivery(&pool, id).await,
        ("SUPPRESSED".into(), 1, true),
        "an in-flight cancellation cannot assert a definite provider rejection"
    );
    assert_eq!(transport.active.load(Ordering::SeqCst), 0);
    assert_eq!(run_push_delivery_pass(&state).await.unwrap(), 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_token_revocation_during_send_is_not_reported_as_accepted(pool: PgPool) {
    let transport = Arc::new(ControlledTransport::new(0));
    let (app, pool, state, id) = fixture(pool, transport.clone(), "EARNING_CREDITED").await;
    let worker_state = state.clone();
    let worker = tokio::spawn(async move { run_push_delivery_pass(&worker_state).await });
    transport.wait_for_send().await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    assert_eq!(
        post(
            &app,
            "/api/v1/notifications/devices/revoke",
            Some(&doctor),
            &serde_json::json!({"push_token":"synthetic-c-device"})
        )
        .await
        .status(),
        StatusCode::OK
    );
    tokio::time::timeout(std::time::Duration::from_secs(14), worker)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(delivery(&pool, id).await, ("FAILED".into(), 1, true));
    assert_eq!(transport.active.load(Ordering::SeqCst), 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_completion_database_failure_preserves_recoverable_claim(pool: PgPool) {
    let transport = Arc::new(ControlledTransport::new(10));
    let (_app, pool, state, id) = fixture(pool, transport.clone(), "EARNING_CREDITED").await;
    sqlx::raw_sql("CREATE FUNCTION c_fail_ack() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.delivery_status='SENT_TO_PROVIDER' THEN RAISE EXCEPTION 'synthetic completion failure'; END IF; RETURN NEW; END $$; CREATE TRIGGER c_fail_ack BEFORE UPDATE ON notifications FOR EACH ROW EXECUTE FUNCTION c_fail_ack();").execute(&pool).await.unwrap();
    assert!(run_push_delivery_pass(&state).await.is_err());
    assert_eq!(delivery(&pool, id).await, ("DUE".into(), 1, true));
    sqlx::raw_sql("DROP TRIGGER c_fail_ack ON notifications; DROP FUNCTION c_fail_ack();")
        .execute(&pool)
        .await
        .unwrap();
    expire(&pool, id).await;
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            if run_push_delivery_pass(&state).await.unwrap() > 0 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        delivery(&pool, id).await,
        ("SENT_TO_PROVIDER".into(), 2, false)
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_inbox_read_does_not_relabel_provider_outcome_or_expose_claim(pool: PgPool) {
    let transport = Arc::new(ControlledTransport::new(1));
    let (app, pool, state, id) = fixture(pool, transport.clone(), "EARNING_CREDITED").await;
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let before = json_body(get(&app, "/api/v1/notifications", Some(&doctor)).await)
        .await
        .to_string();
    assert!(
        !before.contains("delivery_claim_token")
            && !before.contains("delivery_lease_until")
            && !before.contains("delivery_uncertain")
    );
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/notifications/{id}/read"),
            Some(&doctor),
            &serde_json::json!({})
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(delivery(&pool, id).await.0, "SUPPRESSED");
    run_push_delivery_pass(&state).await.unwrap();
    assert_eq!(
        transport.calls.load(Ordering::SeqCst),
        0,
        "reading an unsent inbox item must prevent a later redundant push"
    );
    let sent_id = Uuid::new_v4();
    notification_repo::insert_if_new(
        &pool,
        sent_id,
        UserId::from(uid(DOCTOR1_USER_ID)),
        "EARNING_CREDITED",
        "Synthetic",
        "Synthetic",
        None,
        None,
        None,
        &format!("read-after-send:{sent_id}"),
        None,
    )
    .await
    .unwrap();
    run_push_delivery_pass(&state).await.unwrap();
    assert_eq!(delivery(&pool, sent_id).await.0, "SENT_TO_PROVIDER");
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/notifications/{sent_id}/read"),
            Some(&doctor),
            &serde_json::json!({})
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        delivery(&pool, sent_id).await.0,
        "SENT_TO_PROVIDER",
        "inbox reading must not erase actual provider acceptance"
    );
    let read: bool =
        sqlx::query_scalar("SELECT read_at IS NOT NULL FROM notifications WHERE id=$1")
            .bind(sent_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(read);
    let foreign = make_token(uid(DOCTOR2_USER_ID), "DOCTOR");
    assert!(
        !json_body(get(&app, "/api/v1/notifications", Some(&foreign)).await)
            .await
            .to_string()
            .contains(&id.to_string())
    );
}

async fn attach_sending_campaign(
    app: &axum::Router,
    pool: &PgPool,
    notification: Uuid,
) -> (Uuid, String) {
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let created=post(app,"/api/v1/admin/campaigns",Some(&admin),&serde_json::json!({"creation_key":Uuid::new_v4().to_string(),"content":{"title_en":"Synthetic hours update","title_sw":"Taarifa ya muda","body_en":"The service hours have changed.","body_sw":"Muda wa huduma umebadilika.","audience":"DOCTORS"}})).await;
    assert_eq!(created.status(), StatusCode::OK);
    let data = json_body(created).await;
    let id = Uuid::parse_str(data["data"]["id"].as_str().unwrap()).unwrap();
    for (action, version) in [("submit", 1), ("approve", 2)] {
        assert_eq!(
            post(
                app,
                &format!("/api/v1/admin/campaigns/{id}/{action}"),
                Some(&admin),
                &serde_json::json!({"version":version})
            )
            .await
            .status(),
            StatusCode::OK
        );
    }
    assert_eq!(
        post(
            app,
            &format!("/api/v1/admin/campaigns/{id}/schedule"),
            Some(&admin),
            &serde_json::json!({"version":3,"scheduled_for":Utc::now().to_rfc3339()})
        )
        .await
        .status(),
        StatusCode::OK
    );
    sqlx::query("UPDATE campaigns SET status='SENDING',sending_started_at=NOW(),version=version+1,updated_at=NOW() WHERE id=$1").bind(id).execute(pool).await.unwrap();
    sqlx::query(
        "UPDATE notifications SET related_entity_type='CAMPAIGN',related_entity_id=$2 WHERE id=$1",
    )
    .bind(notification)
    .bind(id)
    .execute(pool)
    .await
    .unwrap();
    (id, admin)
}

#[sqlx::test(migrations = "./migrations")]
async fn test_campaign_pause_defers_queued_delivery_until_authorized_resume(pool: PgPool) {
    let transport = Arc::new(ControlledTransport::new(10));
    let (app, pool, state, id) = fixture(pool, transport.clone(), "ADMIN_CAMPAIGN").await;
    let (campaign, admin) = attach_sending_campaign(&app, &pool, id).await;
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/admin/campaigns/{campaign}/pause"),
            Some(&admin),
            &serde_json::json!({"version":5})
        )
        .await
        .status(),
        StatusCode::OK
    );
    run_push_delivery_pass(&state).await.unwrap();
    assert_eq!(delivery(&pool, id).await, ("FAILED".into(), 0, false));
    assert_eq!(transport.calls.load(Ordering::SeqCst), 0);
    let stats = json_body(
        get(
            &app,
            &format!("/api/v1/admin/campaigns/{campaign}/stats"),
            Some(&admin),
        )
        .await,
    )
    .await;
    assert_eq!(stats["data"]["deferred"], 1);
    assert_eq!(stats["data"]["sent_to_provider"], 0);
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/admin/campaigns/{campaign}/resume"),
            Some(&admin),
            &serde_json::json!({"version":6})
        )
        .await
        .status(),
        StatusCode::OK
    );
    expire(&pool, id).await;
    run_push_delivery_pass(&state).await.unwrap();
    assert_eq!(
        delivery(&pool, id).await,
        ("SENT_TO_PROVIDER".into(), 1, false)
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_campaign_cancellation_fences_inflight_ack_and_preserves_uncertainty(pool: PgPool) {
    let transport = Arc::new(ControlledTransport::new(0));
    let (app, pool, state, id) = fixture(pool, transport.clone(), "ADMIN_CAMPAIGN").await;
    let (campaign, admin) = attach_sending_campaign(&app, &pool, id).await;
    let worker_state = state.clone();
    let worker = tokio::spawn(async move { run_push_delivery_pass(&worker_state).await });
    transport.wait_for_send().await;
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/admin/campaigns/{campaign}/cancel"),
            Some(&admin),
            &serde_json::json!({"version":5})
        )
        .await
        .status(),
        StatusCode::OK
    );
    transport.release.add_permits(1);
    worker.await.unwrap().unwrap();
    assert_eq!(delivery(&pool, id).await, ("SUPPRESSED".into(), 1, true));
    assert_eq!(run_push_delivery_pass(&state).await.unwrap(), 0);
    let stats = json_body(
        get(
            &app,
            &format!("/api/v1/admin/campaigns/{campaign}/stats"),
            Some(&admin),
        )
        .await,
    )
    .await;
    for (field, value) in [
        ("queued", 0),
        ("processing", 0),
        ("retry_pending", 0),
        ("sent_to_provider", 0),
        ("suppressed", 1),
        ("uncertain", 1),
    ] {
        assert_eq!(stats["data"][field], value, "{field}");
    }
    let claims: (Option<Uuid>, Option<DateTime<Utc>>) = sqlx::query_as(
        "SELECT delivery_claim_token,delivery_lease_until FROM notifications WHERE id=$1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(claims, (None, None));
    let audits:i64=sqlx::query_scalar("SELECT count(*) FROM audit_events WHERE resource_id=$1 AND action='CAMPAIGN_CANCEL' AND result='SUCCESS'").bind(campaign).fetch_one(&pool).await.unwrap();
    assert_eq!(audits, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_provider_timeout_is_retryable_and_uncertain_not_accepted(pool: PgPool) {
    let transport = Arc::new(ControlledTransport::new(0));
    let (_app, pool, state, id) = fixture(pool, transport.clone(), "EARNING_CREDITED").await;
    tokio::time::timeout(
        std::time::Duration::from_secs(55),
        run_push_delivery_pass(&state),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(delivery(&pool, id).await, ("FAILED".into(), 1, true));
    assert_eq!(transport.active.load(Ordering::SeqCst), 0);
    let stats = notification_repo::delivery_queue_stats(&pool)
        .await
        .unwrap();
    assert_eq!(
        (stats.retrying, stats.uncertain, stats.sent_to_provider),
        (1, 1, 0)
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_campaign_cancel_audit_failure_rolls_back_all_dependent_state(pool: PgPool) {
    let transport = Arc::new(ControlledTransport::new(0));
    let (app, pool, state, id) = fixture(pool, transport.clone(), "ADMIN_CAMPAIGN").await;
    let (campaign, admin) = attach_sending_campaign(&app, &pool, id).await;
    let worker_state = state.clone();
    let worker = tokio::spawn(async move { run_push_delivery_pass(&worker_state).await });
    transport.wait_for_send().await;
    sqlx::raw_sql("CREATE FUNCTION c_fail_cancel_audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.action='CAMPAIGN_CANCEL' THEN RAISE EXCEPTION 'synthetic audit failure'; END IF; RETURN NEW; END $$; CREATE TRIGGER c_fail_cancel_audit BEFORE INSERT ON audit_events FOR EACH ROW EXECUTE FUNCTION c_fail_cancel_audit();").execute(&pool).await.unwrap();
    let response = post(
        &app,
        &format!("/api/v1/admin/campaigns/{campaign}/cancel"),
        Some(&admin),
        &serde_json::json!({"version":5}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let status: String = sqlx::query_scalar("SELECT status::text FROM campaigns WHERE id=$1")
        .bind(campaign)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "SENDING");
    assert_eq!(delivery(&pool, id).await.0, "DUE");
    let audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_events WHERE resource_id=$1 AND action='CAMPAIGN_CANCEL'",
    )
    .bind(campaign)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audits, 0);
    transport.release.add_permits(1);
    worker.await.unwrap().unwrap();
    assert_eq!(
        delivery(&pool, id).await,
        ("SENT_TO_PROVIDER".into(), 1, false)
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_inflight_read_all_fences_ack_without_cross_account_changes(pool: PgPool) {
    let transport = Arc::new(ControlledTransport::new(0));
    let (app, pool, state, id) = fixture(pool, transport.clone(), "EARNING_CREDITED").await;
    let worker_state = state.clone();
    let worker = tokio::spawn(async move { run_push_delivery_pass(&worker_state).await });
    transport.wait_for_send().await;
    let foreign = make_token(uid(DOCTOR2_USER_ID), "DOCTOR");
    let forbidden = json_body(
        post(
            &app,
            &format!("/api/v1/notifications/{id}/read"),
            Some(&foreign),
            &serde_json::json!({}),
        )
        .await,
    )
    .await;
    assert_eq!(forbidden["data"]["marked"], false);
    assert_eq!(delivery(&pool, id).await.0, "DUE");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let result = json_body(
        post(
            &app,
            "/api/v1/notifications/read-all",
            Some(&doctor),
            &serde_json::json!({}),
        )
        .await,
    )
    .await;
    assert_eq!(result["data"]["marked_count"], 1);
    transport.release.add_permits(1);
    worker.await.unwrap().unwrap();
    assert_eq!(delivery(&pool, id).await, ("SUPPRESSED".into(), 1, true));
    assert_eq!(run_push_delivery_pass(&state).await.unwrap(), 0);
    let unread =
        json_body(get(&app, "/api/v1/notifications/unread-count", Some(&doctor)).await).await;
    assert_eq!(unread["data"]["unread_count"], 0);
}

async fn quiet_change_after_provider_start(pool: PgPool, already_attempted: i32) {
    let transport = Arc::new(ControlledTransport::new(0));
    let (_app, pool, state, id) = fixture(pool, transport.clone(), "SERVICE_ANNOUNCEMENT").await;
    sqlx::query("UPDATE notifications SET delivery_attempts=$2 WHERE id=$1")
        .bind(id)
        .bind(already_attempted)
        .execute(&pool)
        .await
        .unwrap();
    let worker_state = state.clone();
    let worker = tokio::spawn(async move { run_push_delivery_pass(&worker_state).await });
    transport.wait_for_send().await;
    sqlx::query("INSERT INTO notification_preferences(user_id,quiet_hours_enabled,quiet_hours_start_local,quiet_hours_end_local) VALUES($1,true,'00:00','23:59:59')").bind(uid(DOCTOR1_USER_ID)).execute(&pool).await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(14), worker)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(
        delivery(&pool, id).await,
        ("FAILED".into(), already_attempted + 1, true),
        "deferral cannot refund an attempt already started at the provider"
    );
    let retry: Option<DateTime<Utc>> =
        sqlx::query_scalar("SELECT next_delivery_at FROM notifications WHERE id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    if already_attempted == 4 {
        assert!(retry.is_none());
        assert_eq!(
            notification_repo::delivery_queue_stats(&pool)
                .await
                .unwrap()
                .permanently_failed,
            1
        );
    } else {
        assert!(retry.unwrap() > Utc::now());
        assert_eq!(
            notification_repo::delivery_queue_stats(&pool)
                .await
                .unwrap()
                .deferred,
            1
        );
    }
    assert_eq!(run_push_delivery_pass(&state).await.unwrap(), 0);
    assert_eq!(transport.active.load(Ordering::SeqCst), 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_inflight_quiet_hours_deferral_retains_consumed_attempt(pool: PgPool) {
    quiet_change_after_provider_start(pool, 0).await;
}

#[sqlx::test(migrations = "./migrations")]
async fn test_inflight_deferral_cannot_bypass_final_retry_limit(pool: PgPool) {
    quiet_change_after_provider_start(pool, 4).await;
}
