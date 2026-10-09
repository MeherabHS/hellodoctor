use crate::auth_context::AuthContext;
use crate::domain::ids::UserId;
use crate::domain::models::{AuditEvent, UserRole};
use crate::error::AppError;
use crate::repository::{audit_repo, AppState};
use chrono::Utc;
use sqlx::PgExecutor;
use std::collections::BTreeSet;
use uuid::Uuid;

pub struct AuditService;

/// Builder for a single `audit_events` row. `before`/`after` are optional
/// JSON snapshots of the mutated resource — only operational/professional/
/// financial fields ever belong here, never patient clinical content (see
/// `AuditEvent`'s own doc comment for why no redaction pass is needed).
pub struct AuditLogEntry {
    pub actor_id: Option<UserId>,
    pub actor_role: UserRole,
    pub action: String,
    pub resource_type: String,
    pub resource_id: Option<Uuid>,
    pub result: String,
    pub reason: Option<String>,
    pub before: Option<serde_json::Value>,
    pub after: Option<serde_json::Value>,
}

impl AuditLogEntry {
    pub fn new(
        actor_id: Option<UserId>,
        actor_role: UserRole,
        action: impl Into<String>,
        resource_type: impl Into<String>,
        resource_id: Option<Uuid>,
        result: impl Into<String>,
    ) -> Self {
        Self {
            actor_id,
            actor_role,
            action: action.into(),
            resource_type: resource_type.into(),
            resource_id,
            result: result.into(),
            reason: None,
            before: None,
            after: None,
        }
    }

    pub fn with_reason(mut self, reason: Option<String>) -> Self {
        self.reason = reason;
        self
    }

    pub fn with_before(mut self, before: serde_json::Value) -> Self {
        self.before = Some(before);
        self
    }

    pub fn with_after(mut self, after: serde_json::Value) -> Self {
        self.after = Some(after);
        self
    }
}

impl AuditService {
    /// Opens and commits its own transaction — for callers with no already-
    /// open transaction of their own to share (e.g. `simulate_admin_log`,
    /// which has no separate business mutation to be atomic with). Any
    /// caller that already has an open transaction for the business
    /// mutation it's documenting should use `log_in_tx` instead, so the
    /// audit row either commits with that mutation or not at all.
    pub async fn log(
        state: &AppState,
        ctx: &AuthContext,
        entry: AuditLogEntry,
    ) -> Result<AuditEvent, AppError> {
        let event = build_event(entry);
        let mut tx = state.begin_authed(ctx).await?;
        audit_repo::insert(&mut *tx, &event).await?;
        tx.commit().await?;
        Ok(event)
    }

    /// Writes the audit row using the caller's own already-open transaction
    /// (any role whose pool has an `INSERT` grant on `audit_events` —
    /// currently `hellodoctor_api` and `hellodoctor_finance_worker`, see
    /// migrations/20261004000005). The caller commits; if that commit fails,
    /// the business mutation and its audit row roll back together instead of
    /// the mutation silently succeeding with no audit trail (or vice versa)
    /// — the real bug this fixes, found in the security/correctness audit
    /// (docs/CHECKPOINT.md §10): previously every call site committed the
    /// business mutation first, then opened a *separate* transaction for the
    /// audit write, so a failure in that second write left a successful
    /// mutation with no record of who did it.
    pub async fn log_in_tx<'e, E: PgExecutor<'e>>(
        executor: E,
        entry: AuditLogEntry,
    ) -> Result<AuditEvent, AppError> {
        let event = build_event(entry);
        audit_repo::insert(executor, &event).await?;
        Ok(event)
    }
}

fn build_event(entry: AuditLogEntry) -> AuditEvent {
    let changed_fields = diff_changed_fields(&entry.before, &entry.after);
    AuditEvent {
        id: Uuid::new_v4(),
        actor_id: entry.actor_id,
        actor_role: entry.actor_role,
        action: entry.action,
        resource_type: entry.resource_type,
        resource_id: entry.resource_id,
        session_id: None,
        request_id: None,
        ip_hash: None,
        result: entry.result,
        reason: entry.reason,
        before_data: entry.before,
        after_data: entry.after,
        changed_fields,
        created_at: Utc::now(),
    }
}

/// Top-level keys whose value differs between `before`/`after`, sorted for
/// stable output. `None` unless both snapshots are present and are JSON
/// objects — a creation (`before: None`) or a non-object snapshot has no
/// meaningful per-field diff.
fn diff_changed_fields(
    before: &Option<serde_json::Value>,
    after: &Option<serde_json::Value>,
) -> Option<Vec<String>> {
    let (Some(serde_json::Value::Object(b)), Some(serde_json::Value::Object(a))) = (before, after)
    else {
        return None;
    };
    let mut keys = BTreeSet::new();
    for k in b.keys().chain(a.keys()) {
        if b.get(k) != a.get(k) {
            keys.insert(k.clone());
        }
    }
    Some(keys.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_changed_fields_finds_only_differing_keys() {
        let before = Some(serde_json::json!({"status": "OPEN", "note": "x"}));
        let after = Some(serde_json::json!({"status": "RESOLVED", "note": "x"}));
        assert_eq!(
            diff_changed_fields(&before, &after),
            Some(vec!["status".to_string()])
        );
    }

    #[test]
    fn diff_changed_fields_none_on_creation() {
        let after = Some(serde_json::json!({"status": "OPEN"}));
        assert_eq!(diff_changed_fields(&None, &after), None);
    }
}
