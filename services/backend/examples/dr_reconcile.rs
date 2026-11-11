//! Remediation E1 — operator-run disaster-recovery erasure-reconciliation tool.
//!
//! This is intentionally a CLI, not an HTTP endpoint: there is no network-
//! accessible way to pass the recovery-exit gate. It must be run against the
//! restored, RECOVERY_MODE=true database before an operator restarts the
//! server with RECOVERY_MODE=false.
//!
//! Usage:
//!   HELLODOCTOR_ENV_FILE=/etc/hellodoctor/recovery.env \
//!   RECOVERY_OPERATOR_TOKEN=<raw operator token, never the stored digest> \
//!     cargo run --example dr_reconcile --release -- \
//!       --recovery-instance-id INC-2026-11-30-01 \
//!       --recovery-point "2026-11-29T23:00:00Z"
//!
//! Exit code 0 only on a verified PASSED outcome. Any other outcome
//! (REVIEW_REQUIRED, a journal integrity failure, missing configuration, a
//! wrong operator token) exits non-zero and prints the reason — never a
//! silent partial success.

use hellodoctor_backend::api::middleware::global_guard::operator_digest_matches;
use hellodoctor_backend::config::AppConfig;
use hellodoctor_backend::repository::AppState;
use hellodoctor_backend::services::dr_reconciliation::{
    reconcile_after_restore, ReconciliationError,
};
use sqlx::postgres::PgPoolOptions;
use std::process::ExitCode;
use std::sync::Arc;

fn usage() -> ! {
    eprintln!(
        "Usage: dr_reconcile --recovery-instance-id <id> [--recovery-point <description>]\n\
         Requires RECOVERY_MODE=true, a valid RECOVERY_OPERATOR_TOKEN matching the configured \
         RECOVERY_OPERATOR_TOKEN_SHA256, and the EXTERNAL_ERASURE_JOURNAL_* variables."
    );
    std::process::exit(2);
}

fn parse_args() -> (String, String) {
    let mut instance_id: Option<String> = None;
    let mut recovery_point = String::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--recovery-instance-id" => instance_id = args.next(),
            "--recovery-point" => recovery_point = args.next().unwrap_or_default(),
            _ => usage(),
        }
    }
    match instance_id {
        Some(id) if !id.trim().is_empty() => (id, recovery_point),
        _ => usage(),
    }
}

async fn connect_role(max_connections: u32, dsn: String) -> sqlx::Result<sqlx::PgPool> {
    PgPoolOptions::new()
        .max_connections(max_connections)
        .acquire_timeout(std::time::Duration::from_secs(5))
        .connect(&dsn)
        .await
}

#[tokio::main]
async fn main() -> ExitCode {
    let (recovery_instance_id, recovery_point) = parse_args();
    let config = AppConfig::from_env();

    if !config.recovery_mode {
        eprintln!("ERROR: RECOVERY_MODE must be true to run reconciliation.");
        return ExitCode::from(2);
    }
    let Some(configured_instance_id) = config.recovery_instance_id.as_deref() else {
        eprintln!("ERROR: RECOVERY_INSTANCE_ID is not configured.");
        return ExitCode::from(2);
    };
    if configured_instance_id != recovery_instance_id {
        eprintln!(
            "ERROR: --recovery-instance-id ({recovery_instance_id}) does not match the \
             RECOVERY_INSTANCE_ID this environment booted with ({configured_instance_id})."
        );
        return ExitCode::from(2);
    }
    let Some(expected_digest) = config.recovery_operator_token_sha256.as_deref() else {
        eprintln!("ERROR: RECOVERY_OPERATOR_TOKEN_SHA256 is not configured.");
        return ExitCode::from(2);
    };
    let presented = std::env::var("RECOVERY_OPERATOR_TOKEN").unwrap_or_default();
    if presented.is_empty() || !operator_digest_matches(expected_digest, &presented) {
        eprintln!("ERROR: RECOVERY_OPERATOR_TOKEN is missing or does not match.");
        return ExitCode::from(2);
    }

    let max_conn = config.db_pool_max_connections_per_role;
    let pools = tokio::try_join!(
        connect_role(max_conn, config.api_role_dsn()),
        connect_role(max_conn, config.auth_worker_dsn()),
        connect_role(max_conn, config.payment_worker_dsn()),
        connect_role(max_conn, config.system_worker_dsn()),
        connect_role(max_conn, config.finance_worker_dsn()),
    );
    let (db, auth_db, payment_db, system_db, finance_db) = match pools {
        Ok(pools) => pools,
        Err(error) => {
            eprintln!("ERROR: failed to connect to one of the five capability-role pools: {error}");
            return ExitCode::from(2);
        }
    };
    let redis_cfg = deadpool_redis::Config::from_url(&config.redis_url);
    let redis = match redis_cfg.create_pool(Some(deadpool_redis::Runtime::Tokio1)) {
        Ok(pool) => pool,
        Err(error) => {
            eprintln!("ERROR: failed to build Redis connection pool: {error}");
            return ExitCode::from(2);
        }
    };
    let state = AppState::new(
        db,
        auth_db,
        payment_db,
        system_db,
        finance_db,
        redis,
        Arc::new(config),
    );

    match reconcile_after_restore(&state, &recovery_instance_id, &recovery_point).await {
        Ok(outcome) => {
            println!(
                "{}",
                serde_json::json!({
                    "recovery_instance_id": recovery_instance_id,
                    "status": outcome.status,
                    "reconciled_account_count": outcome.reconciled_accounts.len(),
                    "review_required_subject_count": outcome.review_required_subjects.len(),
                    "journal_tail_sequence": outcome.journal_tail_sequence,
                    "journal_tail_mac": outcome.journal_tail_mac,
                })
            );
            if outcome.status == "PASSED" {
                ExitCode::SUCCESS
            } else {
                eprintln!(
                    "REVIEW_REQUIRED: {} journal entr{} require human review before this \
                     recovery instance can pass. Normal-mode startup remains blocked.",
                    outcome.review_required_subjects.len(),
                    if outcome.review_required_subjects.len() == 1 {
                        "y"
                    } else {
                        "ies"
                    }
                );
                ExitCode::from(1)
            }
        }
        Err(ReconciliationError::JournalNotConfigured) => {
            eprintln!("ERROR: external erasure journal is not configured; cannot reconcile.");
            ExitCode::from(2)
        }
        Err(error) => {
            eprintln!("ERROR: reconciliation failed closed: {error}");
            ExitCode::from(2)
        }
    }
}
