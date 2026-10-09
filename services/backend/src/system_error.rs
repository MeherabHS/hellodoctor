//! Centralized System Error Service.
//!
//! `system_error_logs` existed in the schema with zero writers anywhere in the
//! application until this module (confirmed via a live audit — see
//! `docs/CAPACITY_EVIDENCE_REPORT.md` §8.7: 0 rows, 0 call sites). This is the
//! one place genuine backend failures get reported from, deliberately kept
//! separate from `grievance_reports` (clinical/service complaints) and
//! `audit_events` (administrator accountability) — see `docs/CHECKPOINT.md` §8.
//!
//! Reporting is fire-and-forget over an unbounded channel: `report()` never
//! blocks the request it's describing, and a failure to persist the log row
//! itself falls back to a plain structured `tracing::error!` line rather than
//! recursing into another `report()` call.

use crate::domain::models::ErrorSeverity;
use crate::repository::system_error_repo;
use chrono::{Duration as ChronoDuration, Utc};
use regex::Regex;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::sync::{LazyLock, RwLock};
use tokio::sync::mpsc::{self, UnboundedSender};
use uuid::Uuid;

static SENDER: RwLock<Option<UnboundedSender<ErrorEvent>>> = RwLock::new(None);
const DEDUP_WINDOW_MINUTES: i64 = 60;

#[derive(Debug, Clone)]
pub struct ErrorEvent {
    pub severity: ErrorSeverity,
    /// Coarse category — AUTH, DATABASE, PAYMENT, RTC, INTERNAL, etc.
    pub subsystem: &'static str,
    /// Which part of the backend raised this — e.g. "AppError::Database".
    pub component: &'static str,
    pub action_attempted: String,
    /// Raw message — redacted before persistence, never returned to clients.
    pub error_message: String,
    pub error_code: Option<&'static str>,
    pub http_status: Option<u16>,
    pub endpoint: Option<String>,
    pub request_id: String,
    pub impacted_user_id: Option<Uuid>,
    /// A source-code location ("file:line") pointing at the exact call site
    /// that raised this error — the admin panel's "code snippet" field. Real
    /// `AppError::internal`/`AppError::service_unavailable`/`?`-propagated
    /// `sqlx::Error` conversions capture this automatically via
    /// `#[track_caller]` (see `error.rs`); it is `None` only for call sites
    /// that still construct the bare enum variant directly.
    pub stack_trace: Option<String>,
    /// Sanitized, bounded client metadata. Backend events leave this empty.
    pub client_context: Option<serde_json::Value>,
    pub fingerprint_context: Option<String>,
}

impl ErrorEvent {
    pub fn new(
        severity: ErrorSeverity,
        subsystem: &'static str,
        component: &'static str,
        error_code: &'static str,
        error_message: impl Into<String>,
        request_id: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            subsystem,
            component,
            action_attempted: component.to_string(),
            error_message: error_message.into(),
            error_code: Some(error_code),
            http_status: None,
            endpoint: None,
            request_id: request_id.into(),
            impacted_user_id: None,
            stack_trace: None,
            client_context: None,
            fingerprint_context: None,
        }
    }

    pub fn with_http_status(mut self, status: u16) -> Self {
        self.http_status = Some(status);
        self
    }

    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = Some(endpoint.into());
        self
    }

    pub fn with_user(mut self, user_id: Uuid) -> Self {
        self.impacted_user_id = Some(user_id);
        self
    }

    pub fn with_stack_trace(mut self, stack_trace: impl Into<String>) -> Self {
        self.stack_trace = Some(stack_trace.into());
        self
    }
}

/// Call once at startup (see `main.rs`) with a pool for the
/// `hellodoctor_system_worker` role — matching the pre-existing
/// `GRANT SELECT, INSERT ON system_error_logs TO hellodoctor_system_worker`
/// design intent from the RLS migration, rather than inventing a sixth role.
///
/// Safe to call again: each `#[sqlx::test]` gets its own throwaway database
/// and can call this once to point reporting at it; the old channel's sender
/// is dropped, so the old consumer task drains whatever was already queued
/// and exits cleanly rather than leaking.
pub fn init(pool: PgPool) {
    let (tx, mut rx) = mpsc::unbounded_channel::<ErrorEvent>();
    *SENDER.write().unwrap() = Some(tx);

    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            if let Err(e) = persist(&pool, event).await {
                tracing::error!(error = ?e, "failed to persist a system_error_logs row (not retried, not recursively reported)");
            }
        }
    });
}

/// Non-blocking. If `init` was never called, the event is silently dropped —
/// diagnostics must never be able to crash or stall the request they describe.
pub fn report(event: ErrorEvent) {
    let guard = SENDER.read().unwrap_or_else(|e| e.into_inner());
    if let Some(tx) = guard.as_ref() {
        let _ = tx.send(event);
    }
}

/// Synchronous variant of `report()` — persists immediately and returns the
/// affected row's id, instead of queuing onto the background channel. Used
/// by the dev/staging-only admin simulator and bounded authenticated client
/// ingestion, which need the persisted row's ID in the acceptance response.
pub async fn report_sync(pool: &PgPool, event: ErrorEvent) -> Result<Uuid, sqlx::Error> {
    persist(pool, event).await
}

async fn persist(pool: &PgPool, event: ErrorEvent) -> Result<Uuid, sqlx::Error> {
    let safe_message = redact(&event.error_message);
    let mut fp = fingerprint(event.subsystem, event.component, event.error_code);
    if let Some(context) = &event.fingerprint_context {
        let mut hasher = Sha256::new();
        hasher.update(fp.as_bytes());
        hasher.update(context.as_bytes());
        fp = hex::encode(hasher.finalize());
    }
    if event.client_context.is_some() {
        // Concurrent browser/device reports of the same incident must group
        // atomically. Only the new client path takes this per-fingerprint lock.
        let mut tx = pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(&fp)
            .execute(&mut *tx)
            .await?;
        let id = persist_row(&mut tx, event, safe_message, fp).await?;
        tx.commit().await?;
        return Ok(id);
    }
    let mut connection = pool.acquire().await?;
    persist_row(&mut connection, event, safe_message, fp).await
}

async fn persist_row(
    connection: &mut sqlx::PgConnection,
    event: ErrorEvent,
    safe_message: String,
    fp: String,
) -> Result<Uuid, sqlx::Error> {
    let now = Utc::now();
    let since = now - ChronoDuration::minutes(DEDUP_WINDOW_MINUTES);

    if let Some(existing) =
        system_error_repo::find_open_by_fingerprint(&mut *connection, &fp, since).await?
    {
        system_error_repo::bump_occurrence(&mut *connection, existing.id, now).await?;
        return Ok(existing.id);
    }

    let id = Uuid::new_v4();
    system_error_repo::insert(
        &mut *connection,
        id,
        &event.request_id,
        event.subsystem,
        event.component,
        event.severity.as_str(),
        &event.action_attempted,
        &safe_message,
        event.stack_trace.as_deref(),
        event.impacted_user_id,
        event.error_code,
        &fp,
        event.http_status.map(|s| s as i16),
        event.endpoint.as_deref(),
        event.client_context.as_ref(),
        now,
    )
    .await?;
    Ok(id)
}

/// Stable grouping key — category + component + error code only, **never**
/// the raw message, which may embed variable IDs that would otherwise
/// fragment one recurring failure into thousands of distinct fingerprints.
fn fingerprint(subsystem: &str, component: &str, error_code: Option<&str>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(subsystem.as_bytes());
    hasher.update(b":");
    hasher.update(component.as_bytes());
    hasher.update(b":");
    hasher.update(error_code.unwrap_or("").as_bytes());
    hex::encode(hasher.finalize())
}

static BEARER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)bearer\s+[A-Za-z0-9\-_.]+").unwrap());
static JWT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[A-Za-z0-9\-_]{10,}\.[A-Za-z0-9\-_]{10,}\.[A-Za-z0-9\-_]{10,}").unwrap()
});
static KV_SECRET_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)(password|secret|token|api[_-]?key|authorization)\s*[=:]\s*\S+"#).unwrap()
});
const MAX_MESSAGE_LEN: usize = 2000;

/// First, real redaction pass — not a promise every possible secret shape is
/// caught, but a meaningful default rather than none: strips bearer tokens,
/// JWT-shaped strings, and common `key=value`/`key: value` secret patterns
/// before a message is ever persisted. Callers should still avoid putting
/// raw secrets into `error_message` in the first place; this is defense in
/// depth, not the only layer.
pub(crate) fn redact(message: &str) -> String {
    let mut out = BEARER_RE
        .replace_all(message, "[REDACTED_BEARER]")
        .into_owned();
    out = JWT_RE.replace_all(&out, "[REDACTED_JWT]").into_owned();
    out = KV_SECRET_RE.replace_all(&out, "[REDACTED]").into_owned();
    if out.len() > MAX_MESSAGE_LEN {
        out.truncate(
            out.char_indices()
                .nth(MAX_MESSAGE_LEN)
                .map_or(out.len(), |(i, _)| i),
        );
        out.push_str("...[truncated]");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_bearer_tokens() {
        let msg = "request failed, Authorization: Bearer eyJhbGciOiJFZERTQSJ9.eyJzdWIiOiIxMjMifQ.abc123signature";
        let out = redact(msg);
        assert!(!out.contains("eyJhbGciOiJFZERTQSJ9"));
        assert!(out.contains("REDACTED"));
    }

    #[test]
    fn redacts_key_value_secrets() {
        let msg = "connect failed: password=hunter2 host=localhost";
        let out = redact(msg);
        assert!(!out.contains("hunter2"));
    }

    #[test]
    fn fingerprint_ignores_message_variable_ids() {
        let a = fingerprint("DATABASE", "AppError::Database", Some("DB_QUERY_FAILED"));
        let b = fingerprint("DATABASE", "AppError::Database", Some("DB_QUERY_FAILED"));
        assert_eq!(a, b);
        let c = fingerprint("PAYMENT", "AppError::Database", Some("DB_QUERY_FAILED"));
        assert_ne!(a, c);
    }
}
