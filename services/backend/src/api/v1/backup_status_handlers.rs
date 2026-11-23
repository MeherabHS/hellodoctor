//! `GET /api/v1/admin/system/backup-status`: the nightly database dump's
//! status for the Admin Web App Errors & Incidents page.
//!
//! The dump runs on the VM host (`deploy/single-vm/nightly-dump`), outside
//! every container. After each run it writes small JSON status files (no
//! data, no secrets) to a host directory that is mounted read-only into the
//! backend at `BACKUP_STATUS_DIR`. This endpoint only reads those files.
//! Admin-only, because backup timing is useful to an attacker.

use crate::abac::{self, Action};
use crate::auth_context::AuthContext;
use crate::error::AppError;
use axum::Json;
use chrono::{DateTime, Duration, Utc};
use serde_json::{json, Value};
use std::path::Path;

/// A backup older than this is reported as overdue: one nightly run plus a
/// two-hour margin. It also catches a timer that stopped without failing.
pub const OVERDUE_AFTER_HOURS: i64 = 26;
const MAX_STATUS_FILE_BYTES: u64 = 16 * 1024;

/// HEALTHY, FAILED, OVERDUE, NEVER_RUN or NOT_CONFIGURED.
pub fn evaluate(
    configured: bool,
    last_run: Option<&Value>,
    last_success: Option<&Value>,
    now: DateTime<Utc>,
) -> &'static str {
    if !configured {
        return "NOT_CONFIGURED";
    }
    if last_run.is_none() && last_success.is_none() {
        return "NEVER_RUN";
    }
    if last_run
        .and_then(|r| r.get("result"))
        .and_then(Value::as_str)
        == Some("failed")
    {
        return "FAILED";
    }
    let success_at = last_success
        .and_then(|s| s.get("at"))
        .and_then(Value::as_str)
        .and_then(|at| DateTime::parse_from_rfc3339(at).ok())
        .map(|at| at.with_timezone(&Utc));
    match success_at {
        Some(at) if now - at <= Duration::hours(OVERDUE_AFTER_HOURS) => "HEALTHY",
        _ => "OVERDUE",
    }
}

/// One status file as JSON, or None when it is missing, too large or invalid.
async fn read_status(dir: &Path, name: &str) -> Option<Value> {
    let path = dir.join(format!("{name}.json"));
    let meta = tokio::fs::metadata(&path).await.ok()?;
    if !meta.is_file() || meta.len() > MAX_STATUS_FILE_BYTES {
        return None;
    }
    let text = tokio::fs::read_to_string(&path).await.ok()?;
    serde_json::from_str::<Value>(&text)
        .ok()
        .filter(Value::is_object)
}

pub async fn backup_status(ctx: AuthContext) -> Result<Json<Value>, AppError> {
    abac::authorize(&ctx, Action::ViewSystemErrors)?;
    let dir = std::env::var("BACKUP_STATUS_DIR")
        .ok()
        .filter(|d| !d.trim().is_empty());
    let (last_run, last_success, last_restore_test) = match &dir {
        Some(dir) => {
            let dir = Path::new(dir);
            (
                read_status(dir, "last_run").await,
                read_status(dir, "last_success").await,
                read_status(dir, "last_restore_test").await,
            )
        }
        None => (None, None, None),
    };
    let now = Utc::now();
    Ok(Json(json!({
        "success": true,
        "data": {
            "state": evaluate(dir.is_some(), last_run.as_ref(), last_success.as_ref(), now),
            "checked_at": now,
            "overdue_after_hours": OVERDUE_AFTER_HOURS,
            "last_run": last_run,
            "last_success": last_success,
            "last_restore_test": last_restore_test,
        }
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(hours_ago: i64, now: DateTime<Utc>) -> Value {
        json!({"at": (now - Duration::hours(hours_ago)).to_rfc3339()})
    }

    #[test]
    fn reports_each_state() {
        let now = Utc::now();
        let ok = json!({"result": "ok"});
        let failed = json!({"result": "failed", "message": "disk full"});
        assert_eq!(evaluate(false, None, None, now), "NOT_CONFIGURED");
        assert_eq!(evaluate(true, None, None, now), "NEVER_RUN");
        assert_eq!(evaluate(true, Some(&ok), Some(&at(2, now)), now), "HEALTHY");
        assert_eq!(
            evaluate(true, Some(&failed), Some(&at(2, now)), now),
            "FAILED"
        );
        // The timer stopped: the last run was fine but long ago.
        assert_eq!(
            evaluate(true, Some(&ok), Some(&at(30, now)), now),
            "OVERDUE"
        );
        // Ran but never succeeded.
        assert_eq!(evaluate(true, Some(&ok), None, now), "OVERDUE");
        // An unreadable timestamp is treated as overdue, never healthy.
        assert_eq!(
            evaluate(true, Some(&ok), Some(&json!({"at": "yesterday"})), now),
            "OVERDUE"
        );
    }
}
