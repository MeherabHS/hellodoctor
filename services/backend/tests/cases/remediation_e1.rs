//! Remediation E1: durable erasure -> external-journal outbox, and the
//! disaster-recovery erasure-reconciliation gate.
//!
//! Coverage note against the full 15-scenario negative-test list in the
//! Remediation E1 plan: HMAC-key-mismatch, journal truncation/rollback,
//! missing-checkpoint, duplicate-event idempotency, and out-of-order
//! rejection are exercised at the journal-format level by the eight unit
//! tests in `src/services/dr_erasure_journal.rs` (run via `cargo test --lib
//! dr_erasure_journal`) — this file does not repeat them. Two scenarios
//! ("database failure after journaling an intent" and "journal-write
//! timeout/ambiguous acknowledgment") are not exercised by fault injection
//! here; they are structurally guaranteed by using a single Postgres
//! transaction for the COMPLETED state change and the outbox insert
//! together (see `data_rights_service::finish_step`), and by the
//! outbox-sync loop treating every non-success outcome identically as
//! "retry later" regardless of the specific failure shape — this is
//! disclosed, not silently assumed, in `docs/REMEDIATION_E1_DR_ERASURE_REPORT.md`.

use super::*;
use hellodoctor_backend::services::dr_erasure_journal::{pseudonymous_subject_ref, read_verified};
use hellodoctor_backend::services::dr_reconciliation::{
    enforce_recovery_gate, mark_recovery_boot, reconcile_after_restore, sync_outbox_to_journal,
};
use std::path::PathBuf;

fn journal_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hellodoctor-e1-test-{label}-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Points `state` at a fresh, disposable external journal file. Never the
/// shared dev database or any persistent path — a unique temp directory per
/// test, removed by the OS's own temp-cleanup, not by this test (so a failed
/// assertion mid-test still leaves evidence on disk for debugging).
fn configure_journal(state: &mut AppState, dir: &std::path::Path) {
    let mut config = (*state.config).clone();
    config.external_erasure_journal_path =
        Some(dir.join("erasure.journal").to_string_lossy().into_owned());
    config.external_erasure_journal_hmac_key_b64 = Some(STANDARD.encode([b'k'; 32]));
    config.external_erasure_journal_anchor_key_b64 = Some(STANDARD.encode([b'a'; 32]));
    state.config = Arc::new(config);
}

fn journal_path(state: &AppState) -> PathBuf {
    PathBuf::from(state.config.external_erasure_journal_path.clone().unwrap())
}

async fn erase_seeded_patient(app: &axum::Router, admin: &str) {
    let response = post(
        app,
        &format!("/api/v1/admin/patients/{PATIENT_PROFILE_ID}/data-rights/erase"),
        Some(admin),
        &serde_json::json!({"reason": "Remediation E1 regression fixture"}),
    )
    .await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "seeded-patient erasure must succeed for this fixture to be meaningful"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn e1_completed_erasure_writes_and_syncs_outbox_to_external_journal(pool: PgPool) {
    let (app, pool, mut state) = test_app_and_state(pool).await;
    let dir = journal_dir("sync");
    configure_journal(&mut state, &dir);
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");

    erase_seeded_patient(&app, &admin).await;

    let (synced_at, event_id): (Option<chrono::DateTime<Utc>>, String) = sqlx::query_as(
        "SELECT journal_synced_at,event_id FROM durable_erasure_journal_outbox WHERE erasure_job_id IN (SELECT id FROM account_erasure_jobs WHERE profile_id=$1)",
    )
    .bind(uid(PATIENT_PROFILE_ID))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        synced_at.is_none(),
        "the outbox row must be written durably before any journal sync attempt"
    );

    let synced = sync_outbox_to_journal(&state).await.unwrap();
    assert_eq!(synced, 1);

    let (synced_at,): (Option<chrono::DateTime<Utc>>,) = sqlx::query_as(
        "SELECT journal_synced_at FROM durable_erasure_journal_outbox WHERE event_id=$1",
    )
    .bind(&event_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(synced_at.is_some());

    let subject_ref =
        pseudonymous_subject_ref(&uid(PATIENT_USER_ID).to_string(), "hellodoctor-user");
    let entries = read_verified(
        &journal_path(&state),
        &state.config.erasure_journal_hmac_key().unwrap(),
        &state.config.erasure_journal_anchor_key().unwrap(),
        &state.config.external_erasure_journal_key_version,
    )
    .unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].subject_ref, subject_ref);
    assert_eq!(entries[0].state, "COMPLETED");

    // Idempotency at the outbox layer: a second sync pass finds nothing
    // pending and must not append a duplicate journal entry.
    let synced_again = sync_outbox_to_journal(&state).await.unwrap();
    assert_eq!(synced_again, 0);
    let entries = read_verified(
        &journal_path(&state),
        &state.config.erasure_journal_hmac_key().unwrap(),
        &state.config.erasure_journal_anchor_key().unwrap(),
        &state.config.external_erasure_journal_key_version,
    )
    .unwrap();
    assert_eq!(entries.len(), 1, "re-syncing must never duplicate an entry");

    let _ = pool;
}

#[sqlx::test(migrations = "./migrations")]
async fn e1_sync_fails_closed_when_journal_not_configured(pool: PgPool) {
    // `state` deliberately keeps `test_config()`'s default: no journal path.
    let (app, pool, state) = test_app_and_state(pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    erase_seeded_patient(&app, &admin).await;

    let synced = sync_outbox_to_journal(&state).await.unwrap();
    assert_eq!(
        synced, 0,
        "an unconfigured journal must never be treated as a successful sync"
    );
    let (synced_at, attempts): (Option<chrono::DateTime<Utc>>, i32) = sqlx::query_as(
        "SELECT journal_synced_at,journal_sync_attempts FROM durable_erasure_journal_outbox",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        synced_at.is_none(),
        "must remain durably pending, not silently dropped"
    );
    assert_eq!(
        attempts, 0,
        "a missing configuration is distinct from an attempted-and-failed sync"
    );
}

/// The core Remediation E1 acceptance scenario: a database restored to a
/// point before an erasure event must not let the resurrected account
/// resume ordinary access, and normal-mode startup must stay blocked until
/// reconciliation explicitly passes.
#[sqlx::test(migrations = "./migrations")]
async fn e1_reconciliation_detects_and_corrects_backdated_restore_then_passes_gate(pool: PgPool) {
    let (app, pool, mut state) = test_app_and_state(pool).await;
    let dir = journal_dir("reconcile");
    configure_journal(&mut state, &dir);
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");

    // 1) Real erasure happens and is synced to the external journal.
    erase_seeded_patient(&app, &admin).await;
    assert_eq!(sync_outbox_to_journal(&state).await.unwrap(), 1);

    // 2) Simulate "the database was restored to a point before the erasure":
    // the user row, an active session and an enabled device token all
    // reappear exactly as a PITR restore of an older backup would produce.
    // The external journal (a separate file, never touched by this restore)
    // is untouched and still proves the erasure happened.
    sqlx::query("UPDATE users SET is_active=true, deleted_at=NULL, phone_number=$2 WHERE id=$1")
        .bind(uid(PATIENT_USER_ID))
        .bind("+254700000099")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO auth_sessions(id,user_id,token_family_id,refresh_token_hash,created_at,last_used_at,expires_at) \
         VALUES (gen_random_uuid(),$1,gen_random_uuid(),'resurrected-session',NOW(),NOW(),NOW()+INTERVAL '7 days')",
    )
    .bind(uid(PATIENT_USER_ID))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO notification_device_tokens(user_id,platform,push_token,enabled) VALUES ($1,'ANDROID','resurrected-device-token',true)",
    )
    .bind(uid(PATIENT_USER_ID))
    .execute(&pool)
    .await
    .unwrap();
    let active_sessions_before: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM auth_sessions WHERE user_id=$1 AND revoked_at IS NULL",
    )
    .bind(uid(PATIENT_USER_ID))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        active_sessions_before > 0,
        "fixture must actually resurrect a usable session"
    );

    // 3) Before reconciliation, the gate must block normal-mode startup.
    mark_recovery_boot(&state).await.unwrap();
    let gate_before = enforce_recovery_gate(&state).await;
    assert!(
        gate_before.is_err(),
        "startup must refuse normal mode before reconciliation passes"
    );

    // 4) Run the corrective reconciliation pass (the operator CLI's core logic).
    let instance_id = state.config.recovery_instance_id.clone().unwrap();
    let outcome = reconcile_after_restore(&state, &instance_id, "2026-10-29T00:00:00Z")
        .await
        .unwrap();
    assert_eq!(outcome.status, "PASSED");
    assert_eq!(outcome.reconciled_accounts, vec![uid(PATIENT_USER_ID)]);

    // 5) The resurrected session and device token must be gone; the user
    // must be deactivated again.
    let active_sessions_after: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM auth_sessions WHERE user_id=$1 AND revoked_at IS NULL",
    )
    .bind(uid(PATIENT_USER_ID))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        active_sessions_after, 0,
        "every resurrected session must be revoked"
    );
    let enabled_devices: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM notification_device_tokens WHERE user_id=$1 AND enabled",
    )
    .bind(uid(PATIENT_USER_ID))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        enabled_devices, 0,
        "every resurrected device token must be disabled"
    );
    let (is_active,): (bool,) = sqlx::query_as("SELECT is_active FROM users WHERE id=$1")
        .bind(uid(PATIENT_USER_ID))
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(
        !is_active,
        "the resurrected account must be deactivated again"
    );

    // 6) The gate must now allow normal-mode startup.
    assert!(enforce_recovery_gate(&state).await.is_ok());

    // 7) Idempotency: re-running reconciliation finds nothing left to
    // correct and still reports PASSED.
    let second = reconcile_after_restore(&state, &instance_id, "2026-10-29T00:00:00Z")
        .await
        .unwrap();
    assert_eq!(second.status, "PASSED");
    assert!(second.reconciled_accounts.is_empty());

    let _ = app;
}

#[sqlx::test(migrations = "./migrations")]
async fn e1_reconciliation_blocks_on_review_required_journal_entry(pool: PgPool) {
    use hellodoctor_backend::services::dr_erasure_journal::append_idempotent;

    let (_app, pool, mut state) = test_app_and_state(pool).await;
    let dir = journal_dir("review-required");
    configure_journal(&mut state, &dir);

    // A journal entry that is not COMPLETED (e.g. an in-flight erasure
    // interrupted elsewhere) must never be auto-resolved by reconciliation.
    let subject_ref = pseudonymous_subject_ref("ambiguous-subject", "hellodoctor-user");
    append_idempotent(
        &journal_path(&state),
        &state.config.erasure_journal_hmac_key().unwrap(),
        &state.config.erasure_journal_anchor_key().unwrap(),
        &state.config.external_erasure_journal_key_version,
        "event-ambiguous",
        &subject_ref,
        "PENDING",
        "clinical-retention",
    )
    .unwrap();

    mark_recovery_boot(&state).await.unwrap();
    let instance_id = state.config.recovery_instance_id.clone().unwrap();
    let outcome = reconcile_after_restore(&state, &instance_id, "2026-10-29T00:00:00Z")
        .await
        .unwrap();
    assert_eq!(outcome.status, "REVIEW_REQUIRED");
    assert_eq!(outcome.review_required_subjects, vec![subject_ref]);

    assert!(
        enforce_recovery_gate(&state).await.is_err(),
        "a REVIEW_REQUIRED outcome must continue to block normal-mode startup"
    );
    let _ = pool;
}

#[sqlx::test(migrations = "./migrations")]
async fn e1_reconciliation_fails_closed_on_tampered_journal(pool: PgPool) {
    let (app, pool, mut state) = test_app_and_state(pool).await;
    let dir = journal_dir("tampered");
    configure_journal(&mut state, &dir);
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");

    erase_seeded_patient(&app, &admin).await;
    assert_eq!(sync_outbox_to_journal(&state).await.unwrap(), 1);

    // Tamper with the journal data file without updating its anchor —
    // exactly the rollback/truncation shape a backdated restore of the
    // journal's own storage would produce if it were not independently
    // custodied.
    let path = journal_path(&state);
    let original = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, original.replace("COMPLETED", "PENDING")).unwrap();

    mark_recovery_boot(&state).await.unwrap();
    let instance_id = state.config.recovery_instance_id.clone().unwrap();
    let result = reconcile_after_restore(&state, &instance_id, "2026-10-29T00:00:00Z").await;
    assert!(
        result.is_err(),
        "a tampered journal must never be trusted as reconciliation evidence"
    );

    assert!(
        enforce_recovery_gate(&state).await.is_err(),
        "a failed reconciliation attempt must never leave the gate open"
    );
    let _ = pool;
}
