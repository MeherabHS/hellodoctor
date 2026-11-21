//! Security-data retention purge (product owner, 2026-11-21: "security data
//! only"; `docs/DATA_RETENTION_POLICY_TEMPLATE.md`).
//!
//! Approved periods:
//! - expired sign-in / OTP / MFA / password-reset codes: deleted
//!   [`CODE_RETENTION_DAYS`] days after they expired;
//! - ended sessions (revoked, rotated or expired) and their device keys:
//!   deleted [`SESSION_RETENTION_DAYS`] days after they ended, unless a
//!   permanent record (audit log, consultation trace, reschedule) still points
//!   at the session — those are kept as security evidence.
//!
//! Nothing clinical, financial, chat, grievance or audit is ever deleted here.
//! Runs from the notification scheduler at most once per [`RUN_EVERY`], in
//! bounded batches, as the auth worker. Logs counts only.

use crate::error::AppError;
use crate::repository::AppState;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

pub const CODE_RETENTION_DAYS: i32 = 7;
pub const SESSION_RETENTION_DAYS: i32 = 90;
pub const RUN_EVERY: Duration = Duration::from_secs(60 * 60);
const BATCH: i64 = 500;
const MAX_BATCHES: usize = 10;

/// Rows removed by one purge run, per data class.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PurgeCounts {
    pub phone_otps: u64,
    pub patient_auth_challenges: u64,
    pub pending_mfa_challenges: u64,
    pub password_reset_requests: u64,
    pub auth_sessions: u64,
}

impl PurgeCounts {
    pub fn total(&self) -> u64 {
        self.phone_otps
            + self.patient_auth_challenges
            + self.pending_mfa_challenges
            + self.password_reset_requests
            + self.auth_sessions
    }
}

/// Code tables and the column that says when each row stopped being usable.
const CODE_TABLES: [(&str, &str); 4] = [
    ("phone_otps", "expires_at"),
    ("patient_auth_challenges", "expires_at"),
    ("pending_mfa_challenges", "expires_at"),
    (
        "password_reset_requests",
        "GREATEST(otp_expires_at, COALESCE(authorization_expires_at, otp_expires_at))",
    ),
];

async fn purge_codes(state: &AppState, table: &str, expired_at: &str) -> Result<u64, AppError> {
    // Table and column names are compile-time constants above, never input.
    let sql = format!(
        "DELETE FROM {table} WHERE ctid IN (SELECT ctid FROM {table} \
         WHERE {expired_at} < now() - make_interval(days => $1) LIMIT $2)"
    );
    let mut removed = 0u64;
    for _ in 0..MAX_BATCHES {
        let n = sqlx::query(&sql)
            .bind(CODE_RETENTION_DAYS)
            .bind(BATCH)
            .execute(&state.auth_db)
            .await?
            .rows_affected();
        removed += n;
        if n < BATCH as u64 {
            break;
        }
    }
    Ok(removed)
}

async fn purge_sessions(state: &AppState) -> Result<u64, AppError> {
    let mut removed = 0u64;
    for _ in 0..MAX_BATCHES {
        let n: i32 = sqlx::query_scalar("SELECT purge_ended_auth_sessions($1, $2)")
            .bind(SESSION_RETENTION_DAYS)
            .bind(BATCH as i32)
            .fetch_one(&state.auth_db)
            .await?;
        removed += n.max(0) as u64;
        if (n as i64) < BATCH {
            break;
        }
    }
    Ok(removed)
}

/// One full purge pass, regardless of when the last one ran.
pub async fn run_security_retention_purge(state: &AppState) -> Result<PurgeCounts, AppError> {
    let mut counts = PurgeCounts::default();
    for (table, expired_at) in CODE_TABLES {
        let n = purge_codes(state, table, expired_at).await?;
        match table {
            "phone_otps" => counts.phone_otps = n,
            "patient_auth_challenges" => counts.patient_auth_challenges = n,
            "pending_mfa_challenges" => counts.pending_mfa_challenges = n,
            _ => counts.password_reset_requests = n,
        }
    }
    counts.auth_sessions = purge_sessions(state).await?;
    if counts.total() > 0 {
        tracing::info!(
            phone_otps = counts.phone_otps,
            patient_auth_challenges = counts.patient_auth_challenges,
            pending_mfa_challenges = counts.pending_mfa_challenges,
            password_reset_requests = counts.password_reset_requests,
            auth_sessions = counts.auth_sessions,
            "security retention purge removed expired records"
        );
    }
    Ok(counts)
}

/// Scheduler hook: runs the purge when [`RUN_EVERY`] has passed since the
/// last run in this process (the first tick after start runs it).
pub async fn run_if_due(state: &AppState) -> Result<(), AppError> {
    static LAST_RUN: AtomicI64 = AtomicI64::new(0);
    let now = chrono::Utc::now().timestamp();
    let last = LAST_RUN.load(Ordering::Relaxed);
    if last != 0 && now - last < RUN_EVERY.as_secs() as i64 {
        return Ok(());
    }
    LAST_RUN.store(now, Ordering::Relaxed);
    run_security_retention_purge(state).await.map(|_| ())
}
