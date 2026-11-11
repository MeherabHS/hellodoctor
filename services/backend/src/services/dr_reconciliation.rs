//! Remediation E1: disaster-recovery erasure-reconciliation gate.
//!
//! Three independent concerns, kept separate deliberately:
//! 1. `sync_outbox_to_journal` — periodic background pass (scheduler-driven,
//!    like every other lifecycle sweep in this codebase) that durably copies
//!    completed-erasure intent, already committed to Postgres by
//!    `data_rights_service::finish_step`, into the externally-custodied
//!    HMAC-chained journal (`dr_erasure_journal`). This is an outbox, not a
//!    distributed transaction: Postgres and the external journal never share
//!    an atomic commit. A journal-append failure leaves the outbox row
//!    unsynced for retry; it never marks a row synced on failure.
//! 2. `mark_recovery_boot` / `enforce_recovery_gate` — the two halves of the
//!    startup-time gate. The first runs whenever this binary boots with
//!    RECOVERY_MODE=true and durably records (in Postgres, which is by
//!    definition the actual database this boot is serving) that this
//!    recovery instance is AWAITING_RECONCILIATION. The second runs on every
//!    *normal*-mode boot and refuses to let the process serve ordinary
//!    traffic while any instance remains unresolved.
//! 3. `reconcile_after_restore` — the corrective pass, run only by the
//!    operator-invoked `dr_reconcile` CLI tool (never by an HTTP handler, so
//!    there is no network-accessible way to pass this gate). It compares the
//!    external journal's proof of completed erasures against the restored
//!    database's live `users` state, re-applies deactivation/session
//!    revocation/device-token disabling for any drifted (resurrected)
//!    account through the *existing* authorized mutation functions in
//!    `data_rights_service`, and only then marks the recovery instance
//!    PASSED.
//!
//! LIMITATION, stated plainly for the report: an erasure whose outbox row
//! has not yet been synced to the external journal at the moment a backup is
//! taken is **not** protected by this mechanism — the outbox row itself is
//! just as subject to rollback as everything else in Postgres. This closes
//! the specific gap the Remediation E audit found (the journal was never
//! populated by real application code at all); it narrows, but does not
//! eliminate, the residual window, which is bounded by how often
//! `sync_outbox_to_journal` runs (every 60 seconds, matching this
//! codebase's existing scheduler cadence) and should be monitored in
//! production (alert on a growing unsynced-outbox backlog).

use crate::auth_context::AuthContext;
use crate::domain::ids::UserId;
use crate::error::AppError;
use crate::repository::{AppState, DbRole};
use crate::services::dr_erasure_journal::{self, JournalError};
use std::collections::HashSet;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, sqlx::FromRow)]
struct OutboxRow {
    id: Uuid,
    event_id: String,
    subject_ref: String,
    state: String,
    retention_class: String,
}

struct JournalConfig {
    path: PathBuf,
    hmac_key: Vec<u8>,
    anchor_key: Vec<u8>,
    key_version: String,
}

fn journal_config(state: &AppState) -> Option<JournalConfig> {
    let path = state.config.external_erasure_journal_path.as_deref()?;
    let hmac_key = state.config.erasure_journal_hmac_key()?;
    let anchor_key = state.config.erasure_journal_anchor_key()?;
    Some(JournalConfig {
        path: PathBuf::from(path),
        hmac_key,
        anchor_key,
        key_version: state.config.external_erasure_journal_key_version.clone(),
    })
}

/// Periodic pass: copy every not-yet-synced outbox row into the external
/// journal, idempotently. Returns the number of rows newly marked synced.
/// Never panics and never treats "journal not configured" (development/test
/// only — `validate_public_exposure` already forbids this in staging/
/// production or RECOVERY_MODE) as success; it is logged and skipped so the
/// obligation remains visibly pending rather than silently discarded.
pub async fn sync_outbox_to_journal(state: &AppState) -> Result<u64, AppError> {
    let Some(journal) = journal_config(state) else {
        tracing::warn!(
            "Erasure journal is not configured; durable-erasure outbox rows remain pending sync"
        );
        return Ok(0);
    };
    let ctx = AuthContext::system();
    let mut tx = state.begin_authed_as(&ctx, DbRole::SystemWorker).await?;
    let pending: Vec<OutboxRow> = sqlx::query_as(
        "SELECT id,event_id,subject_ref,state,retention_class FROM durable_erasure_journal_outbox \
         WHERE journal_synced_at IS NULL ORDER BY created_at,id LIMIT 25",
    )
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;

    let mut synced = 0u64;
    for row in pending {
        let append_result = dr_erasure_journal::append_idempotent(
            &journal.path,
            &journal.hmac_key,
            &journal.anchor_key,
            &journal.key_version,
            &row.event_id,
            &row.subject_ref,
            &row.state,
            &row.retention_class,
        );
        let mut tx = state.begin_authed_as(&ctx, DbRole::SystemWorker).await?;
        match append_result {
            Ok(_) => {
                sqlx::query(
                    "UPDATE durable_erasure_journal_outbox SET journal_synced_at=clock_timestamp() WHERE id=$1",
                )
                .bind(row.id)
                .execute(&mut *tx)
                .await?;
                synced += 1;
            }
            Err(error) => {
                // Fixed diagnostic code only, never the external journal's
                // raw filesystem/integrity error text (which could, in a
                // tampering scenario, be adversarially crafted).
                let code = match error {
                    JournalError::Io(_) => "JOURNAL_IO_ERROR",
                    JournalError::Integrity(_) => "JOURNAL_INTEGRITY_ERROR",
                    JournalError::Malformed(_) => "JOURNAL_MALFORMED",
                };
                sqlx::query(
                    "UPDATE durable_erasure_journal_outbox SET journal_sync_attempts=journal_sync_attempts+1,last_sync_error=$2 WHERE id=$1",
                )
                .bind(row.id)
                .bind(code)
                .execute(&mut *tx)
                .await?;
                tracing::error!(error = code, event_id = %row.event_id, "erasure journal sync failed; will retry");
            }
        }
        tx.commit().await?;
    }
    Ok(synced)
}

/// Called once at startup whenever `RECOVERY_MODE=true`. Idempotent per
/// `recovery_instance_id`: a repeated boot for the same incident never
/// downgrades an already-advanced row (enforced both here via
/// `ON CONFLICT DO NOTHING` and, independently, by the database trigger
/// `protect_dr_recovery_state`).
pub async fn mark_recovery_boot(state: &AppState) -> Result<(), AppError> {
    let Some(instance_id) = state.config.recovery_instance_id.as_deref() else {
        // `validate_public_exposure` already refuses to boot with
        // RECOVERY_MODE=true and no RECOVERY_INSTANCE_ID; this is defensive.
        return Ok(());
    };
    let recovery_point = std::env::var("RECOVERY_POINT").ok();
    let ctx = AuthContext::system();
    let mut tx = state.begin_authed_as(&ctx, DbRole::SystemWorker).await?;
    sqlx::query(
        "INSERT INTO dr_recovery_state(recovery_instance_id,recovery_point,status) \
         VALUES ($1,$2,'AWAITING_RECONCILIATION') ON CONFLICT (recovery_instance_id) DO NOTHING",
    )
    .bind(instance_id)
    .bind(recovery_point)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    tracing::warn!(
        recovery_instance_id = instance_id,
        "RECOVERY_MODE=true: this instance will remain blocked from normal-mode startup until \
         `dr_reconcile` records a PASSED reconciliation for this recovery_instance_id"
    );
    Ok(())
}

/// Called once at startup whenever `RECOVERY_MODE=false`. Returns an error
/// string (never panics itself — `main.rs` decides how to fail) naming every
/// unresolved recovery instance if normal-mode startup must be refused.
pub async fn enforce_recovery_gate(state: &AppState) -> Result<(), String> {
    let ctx = AuthContext::system();
    let mut tx = state
        .begin_authed_as(&ctx, DbRole::SystemWorker)
        .await
        .map_err(|e| format!("could not check recovery-reconciliation state: {e}"))?;
    let blocking: Vec<(String, String)> = sqlx::query_as(
        "SELECT recovery_instance_id,status FROM dr_recovery_state WHERE status<>'PASSED' ORDER BY created_at",
    )
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| format!("could not check recovery-reconciliation state: {e}"))?;
    let _ = tx.commit().await;
    if blocking.is_empty() {
        return Ok(());
    }
    let summary = blocking
        .iter()
        .map(|(id, status)| format!("{id}={status}"))
        .collect::<Vec<_>>()
        .join(", ");
    Err(format!(
        "unresolved disaster-recovery reconciliation for: {summary}. Run the `dr_reconcile` tool \
         to completion for each recovery_instance_id before normal-mode startup is permitted."
    ))
}

#[derive(Debug)]
pub struct ReconciliationOutcome {
    pub status: &'static str,
    pub reconciled_accounts: Vec<Uuid>,
    pub review_required_subjects: Vec<String>,
    pub journal_tail_sequence: i64,
    pub journal_tail_mac: String,
}

#[derive(Debug)]
pub enum ReconciliationError {
    JournalNotConfigured,
    Journal(JournalError),
    App(AppError),
}
impl std::fmt::Display for ReconciliationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReconciliationError::JournalNotConfigured => {
                write!(f, "external erasure journal is not configured")
            }
            ReconciliationError::Journal(e) => write!(f, "{e}"),
            ReconciliationError::App(e) => write!(f, "{e}"),
        }
    }
}
impl From<AppError> for ReconciliationError {
    fn from(e: AppError) -> Self {
        ReconciliationError::App(e)
    }
}
impl From<sqlx::Error> for ReconciliationError {
    fn from(e: sqlx::Error) -> Self {
        ReconciliationError::App(AppError::from(e))
    }
}

/// The corrective pass. Only ever invoked by the operator-run `dr_reconcile`
/// binary (see `examples/dr_reconcile.rs`), which independently re-checks
/// the recovery-operator token before calling this — there is no HTTP route
/// that reaches this function.
///
/// Fails closed: a missing, truncated, or tampered journal returns an error
/// and writes no PASSED state. A journal entry whose state is not
/// `"COMPLETED"` is never auto-corrected — it is classified REVIEW_REQUIRED
/// and blocks PASSED until a human resolves it.
pub async fn reconcile_after_restore(
    state: &AppState,
    recovery_instance_id: &str,
    recovery_point: &str,
) -> Result<ReconciliationOutcome, ReconciliationError> {
    let journal = journal_config(state).ok_or(ReconciliationError::JournalNotConfigured)?;
    let entries = dr_erasure_journal::read_verified(
        &journal.path,
        &journal.hmac_key,
        &journal.anchor_key,
        &journal.key_version,
    )
    .map_err(ReconciliationError::Journal)?;

    let mut completed_erased: HashSet<String> = HashSet::new();
    let mut review_required: Vec<String> = Vec::new();
    for entry in &entries {
        if entry.state == "COMPLETED" {
            completed_erased.insert(entry.subject_ref.clone());
        } else {
            review_required.push(entry.subject_ref.clone());
        }
    }
    let journal_tail_sequence = entries.last().map(|e| e.sequence).unwrap_or(0);
    let journal_tail_mac = entries
        .last()
        .map(|e| e.mac.clone())
        .unwrap_or_else(|| "0".repeat(64));

    let ctx = AuthContext::system();
    let active_users: Vec<(Uuid,)> = {
        let mut tx = state.begin_authed_as(&ctx, DbRole::SystemWorker).await?;
        let rows = sqlx::query_as("SELECT id FROM users WHERE is_active=true")
            .fetch_all(&mut *tx)
            .await?;
        tx.commit().await?;
        rows
    };

    let mut reconciled_accounts = Vec::new();
    for (user_id,) in active_users {
        let subject_ref =
            dr_erasure_journal::pseudonymous_subject_ref(&user_id.to_string(), "hellodoctor-user");
        if completed_erased.contains(&subject_ref) {
            let user = UserId::from(user_id);
            data_rights_reapply(state, user).await?;
            reconciled_accounts.push(user_id);
        }
    }

    let status: &'static str = if !review_required.is_empty() {
        "REVIEW_REQUIRED"
    } else {
        "PASSED"
    };

    let details = serde_json::json!({
        "reconciled_account_count": reconciled_accounts.len(),
        "review_required_count": review_required.len(),
        "journal_entry_count": entries.len(),
    });
    {
        let mut tx = state.begin_authed_as(&ctx, DbRole::SystemWorker).await?;
        sqlx::query(
            "INSERT INTO dr_recovery_state(recovery_instance_id,recovery_point,status,journal_tail_sequence,journal_tail_mac,details) \
             VALUES ($1,$2,$3,$4,$5,$6) \
             ON CONFLICT (recovery_instance_id) DO UPDATE SET \
               recovery_point=EXCLUDED.recovery_point,status=EXCLUDED.status, \
               journal_tail_sequence=EXCLUDED.journal_tail_sequence,journal_tail_mac=EXCLUDED.journal_tail_mac, \
               details=EXCLUDED.details,updated_at=clock_timestamp()",
        )
        .bind(recovery_instance_id)
        .bind(recovery_point)
        .bind(status)
        .bind(journal_tail_sequence)
        .bind(&journal_tail_mac)
        .bind(details)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
    }

    Ok(ReconciliationOutcome {
        status,
        reconciled_accounts,
        review_required_subjects: review_required,
        journal_tail_sequence,
        journal_tail_mac,
    })
}

async fn data_rights_reapply(state: &AppState, user: UserId) -> Result<(), AppError> {
    super::data_rights_service::reapply_erasure_deactivation_for_resurrected_account(state, user)
        .await?;
    super::data_rights_service::disable_device_tokens_for_resurrected_account(state, user).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconciliation_error_display_is_informative_and_source_free() {
        let err = ReconciliationError::JournalNotConfigured;
        assert_eq!(
            err.to_string(),
            "external erasure journal is not configured"
        );
    }
}
