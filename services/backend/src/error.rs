use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::panic::Location;

thread_local! {
    /// The source location ("file:line") of the most recent `AppError`
    /// construction on this thread, captured via `#[track_caller]` at the
    /// `AppError::internal`/`AppError::service_unavailable` constructors and
    /// the manual `From<sqlx::Error>` impl below. Read-and-cleared by
    /// `into_response` immediately afterward.
    ///
    /// Safe despite being thread-local rather than task-local: Tokio only
    /// switches which task a worker thread is polling at an `.await` point,
    /// and there is no `.await` between an `AppError` being constructed
    /// (inside a handler/service function, synchronously) and the same
    /// task's `Result::Err` being converted via `into_response` — the
    /// conversion happens in the same synchronous continuation of polling
    /// that already-resolved future, so no other task can interleave and
    /// read a stale value.
    static LAST_ERROR_LOCATION: RefCell<Option<String>> = const { RefCell::new(None) };
}

fn take_last_error_location() -> Option<String> {
    LAST_ERROR_LOCATION.with(|cell| cell.borrow_mut().take())
}

fn record_error_location(loc: &Location<'_>) {
    LAST_ERROR_LOCATION.with(|cell| {
        *cell.borrow_mut() = Some(format!("{}:{}", loc.file(), loc.line()));
    });
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Validation failed: {0}")]
    Validation(String),

    /// Same top-level shape/status as `Validation` (`422` /
    /// `"VALIDATION_FAILED"`), but additionally carries a stable, machine-
    /// readable `(field, code)` pair in `details` so a client can map a
    /// specific error without string-matching the human-readable `message`.
    /// Purely additive — existing `Validation(String)` call sites are
    /// untouched and keep returning `details: None`.
    #[error("Validation failed on field {field}: {message}")]
    FieldValidation {
        field: &'static str,
        code: &'static str,
        message: String,
    },

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Rate limit exceeded: {0}")]
    RateLimitExceeded(String),

    #[error("Rate-limit service unavailable: {0}")]
    RateLimitServiceUnavailable(String),

    #[error("Payload too large: {0}")]
    PayloadTooLarge(String),

    #[error("Unprocessable entity: {0}")]
    UnprocessableEntity(String),

    #[error("Payment required: {0}")]
    PaymentRequired(String),

    /// Consultation room join denied by the authoritative join-window policy
    /// (`domain::join_window`). `reason` is a stable machine-readable code
    /// returned as the error `code`, so Doctor Web and the Patient app can
    /// tell "too early" from "expired" from "invalid state" without
    /// string-matching the message.
    #[error("Consultation join denied ({reason}): {message}")]
    JoinDenied {
        reason: &'static str,
        message: String,
    },

    #[error("Capacity limit reached: {0}")]
    CapacityLimitReached(String),

    #[error("Internal error: {0}")]
    Internal(String),

    /// A dependency the request *needs* to proceed safely (e.g. the abuse-
    /// control store backing login rate limiting) is unreachable, and the
    /// caller deliberately fails closed rather than silently disabling the
    /// control. Distinguished from `Internal` so an operator (and monitoring)
    /// can tell "a dependency is down, by design we refuse to proceed" apart
    /// from "the server has a genuine unhandled bug" — see B-05 in
    /// docs/audit/DOCTOR_SECURITY_BUG_AUDIT.md.
    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),

    /// Deliberately not `#[from] sqlx::Error` — a manual `From` impl below is
    /// used instead so `#[track_caller]` can record exactly which call site's
    /// `?` triggered the conversion (see `LAST_ERROR_LOCATION` above), giving
    /// the admin panel's system-errors view a genuine source-code reference
    /// instead of a permanently-null `stack_trace` column.
    #[error("Database error: {0}")]
    Database(#[source] sqlx::Error),
}

/// Maps the PostgreSQL exclusion violations that enforce appointment interval
/// exclusivity (migration 20261111000001) to an intentional 409. The messages
/// are fixed strings: they never echo the conflicting appointment, the other
/// party or the offending interval.
fn schedule_conflict(e: &sqlx::Error) -> Option<AppError> {
    let db = e.as_database_error()?;
    if db.code().as_deref() != Some("23P01") {
        return None;
    }
    match db.constraint() {
        Some("appt_patient_no_overlap") => Some(AppError::Conflict(
            "This time overlaps another appointment for the same patient.".into(),
        )),
        Some("appt_doctor_no_overlap") => Some(AppError::Conflict(
            "The selected time is no longer available.".into(),
        )),
        _ => None,
    }
}

impl From<sqlx::Error> for AppError {
    #[track_caller]
    fn from(e: sqlx::Error) -> Self {
        if let Some(conflict) = schedule_conflict(&e) {
            return conflict;
        }
        record_error_location(Location::caller());
        AppError::Database(e)
    }
}

impl AppError {
    /// Preferred way to construct `AppError::Internal` — captures the exact
    /// call site via `#[track_caller]` so the resulting `system_error_logs`
    /// row carries a real "file:line" `stack_trace` instead of `None`. Call
    /// sites should use this (or `.map_err(AppError::internal)`) instead of
    /// constructing the bare `AppError::Internal(...)` variant directly.
    ///
    /// Takes a concrete `String` rather than `impl Into<String>` deliberately:
    /// an `impl Trait` parameter here makes every existing `"literal".into()`
    /// call site across the codebase ambiguous (`.into()`'s target type can no
    /// longer be inferred once the parameter is generic over the `Into` bound
    /// rather than concretely `String` — confirmed by a real `cargo check`
    /// failure during this change, not a hypothetical), so callers keep their
    /// existing `.into()`/`format!()` call shape unchanged.
    #[track_caller]
    pub fn internal(msg: String) -> Self {
        record_error_location(Location::caller());
        AppError::Internal(msg)
    }

    /// Same as `internal`, for `AppError::ServiceUnavailable`.
    #[track_caller]
    pub fn service_unavailable(msg: String) -> Self {
        record_error_location(Location::caller());
        AppError::ServiceUnavailable(msg)
    }

    #[track_caller]
    pub fn rate_limit_service_unavailable(msg: String) -> Self {
        record_error_location(Location::caller());
        AppError::RateLimitServiceUnavailable(msg)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorDetail {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    pub issue: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Vec<ErrorDetail>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub error: ErrorBody,
    pub meta: ResponseMeta,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResponseMeta {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub request_id: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let request_id = crate::api::middleware::request_context::request_id();
        let mut details: Option<Vec<ErrorDetail>> = None;
        let (status, code, message) = match &self {
            AppError::Validation(msg) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "VALIDATION_FAILED",
                msg.clone(),
            ),
            AppError::FieldValidation {
                field,
                code,
                message,
            } => {
                details = Some(vec![ErrorDetail {
                    field: Some(field.to_string()),
                    issue: code.to_string(),
                }]);
                (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "VALIDATION_FAILED",
                    message.clone(),
                )
            }
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, "RESOURCE_NOT_FOUND", msg.clone()),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, "RESOURCE_CONFLICT", msg.clone()),
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, "UNAUTHORIZED", msg.clone()),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, "FORBIDDEN", msg.clone()),
            AppError::RateLimitExceeded(msg) => (
                StatusCode::TOO_MANY_REQUESTS,
                "RATE_LIMIT_EXCEEDED",
                msg.clone(),
            ),
            AppError::PayloadTooLarge(msg) => (
                StatusCode::PAYLOAD_TOO_LARGE,
                "PAYLOAD_TOO_LARGE",
                msg.clone(),
            ),
            AppError::UnprocessableEntity(msg) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "UNPROCESSABLE_ENTITY",
                msg.clone(),
            ),
            AppError::JoinDenied { reason, message } => {
                // Too early / window closed are authorization-in-time (403);
                // wrong state / needs-review are conflicts (409).
                let status = match *reason {
                    "NOT_YET_OPEN" | "WINDOW_CLOSED" | "NOT_VIDEO" => StatusCode::FORBIDDEN,
                    _ => StatusCode::CONFLICT,
                };
                (status, *reason, message.clone())
            }
            AppError::PaymentRequired(msg) => (
                StatusCode::PAYMENT_REQUIRED,
                "PAYMENT_REQUIRED",
                msg.clone(),
            ),
            AppError::CapacityLimitReached(msg) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "CAPACITY_LIMIT_REACHED",
                msg.clone(),
            ),
            AppError::ServiceUnavailable(msg) | AppError::RateLimitServiceUnavailable(msg) => {
                let code = if matches!(&self, AppError::RateLimitServiceUnavailable(_)) {
                    "RATE_LIMIT_SERVICE_UNAVAILABLE"
                } else {
                    "SERVICE_UNAVAILABLE"
                };
                tracing::warn!("Service unavailable: {}", crate::system_error::redact(msg));
                let mut event = crate::system_error::ErrorEvent::new(
                    crate::domain::models::ErrorSeverity::Warning,
                    "DEPENDENCY",
                    "AppError::ServiceUnavailable",
                    code,
                    msg.clone(),
                    request_id.clone(),
                )
                .with_http_status(503);
                if let Some(loc) = take_last_error_location() {
                    event = event.with_stack_trace(loc);
                }
                crate::system_error::report(event);
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    code,
                    "This action depends on a service that is temporarily unavailable. Please retry shortly.".into(),
                )
            }
            AppError::Internal(msg) => {
                // Redacted before it ever reaches the plain log stream, not
                // just before persistence — the Pass 2 fix for a real gap
                // (docs/CHECKPOINT.md §10): `system_error::report` below
                // already redacted before writing to `system_error_logs`,
                // but this `tracing::error!` line logged the raw message
                // regardless, so a secret embedded in `msg` (e.g. a Postgres
                // constraint-violation DETAIL line quoting the offending
                // value) still reached plain stdout/log aggregation.
                tracing::error!("Internal error: {}", crate::system_error::redact(msg));
                let mut event = crate::system_error::ErrorEvent::new(
                    crate::domain::models::ErrorSeverity::Error,
                    "INTERNAL",
                    "AppError::Internal",
                    "INTERNAL_ERROR",
                    msg.clone(),
                    request_id.clone(),
                )
                .with_http_status(500);
                if let Some(loc) = take_last_error_location() {
                    event = event.with_stack_trace(loc);
                }
                crate::system_error::report(event);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_SERVER_ERROR",
                    "An unexpected error occurred.".into(),
                )
            }
            AppError::Database(e) => {
                tracing::error!(
                    "Database error: {}",
                    crate::system_error::redact(&e.to_string())
                );
                let mut event = crate::system_error::ErrorEvent::new(
                    crate::domain::models::ErrorSeverity::Error,
                    "DATABASE",
                    "AppError::Database",
                    "DB_QUERY_FAILED",
                    e.to_string(),
                    request_id.clone(),
                )
                .with_http_status(500);
                if let Some(loc) = take_last_error_location() {
                    event = event.with_stack_trace(loc);
                }
                crate::system_error::report(event);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_SERVER_ERROR",
                    "An unexpected error occurred.".into(),
                )
            }
        };

        let body = ErrorResponse {
            success: false,
            error: ErrorBody {
                code: code.to_string(),
                message,
                details,
            },
            meta: ResponseMeta {
                timestamp: chrono::Utc::now(),
                request_id,
            },
        };

        let mut response = (status, Json(body)).into_response();
        if status == StatusCode::TOO_MANY_REQUESTS || status == StatusCode::SERVICE_UNAVAILABLE {
            response.headers_mut().insert(
                axum::http::header::RETRY_AFTER,
                axum::http::HeaderValue::from_static(if status == StatusCode::TOO_MANY_REQUESTS {
                    "60"
                } else {
                    "1"
                }),
            );
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Proves the `#[track_caller]` location-capture mechanism actually
    /// records the real call site of `AppError::internal`, not some fixed
    /// location inside `error.rs` itself — this is the "code snippet" field
    /// the admin panel's dedicated system-errors section displays.
    #[test]
    fn internal_constructor_captures_its_own_call_site() {
        let before_line = line!() + 1;
        let _ = AppError::internal("test error for location capture".to_string());
        let loc = take_last_error_location().expect("location should have been captured");
        assert!(
            loc.ends_with(&format!("error.rs:{before_line}")),
            "expected location to point at this test's own call site, got {loc:?}"
        );
    }

    #[test]
    fn service_unavailable_constructor_captures_its_own_call_site() {
        let before_line = line!() + 1;
        let _ = AppError::service_unavailable("dependency down".to_string());
        let loc = take_last_error_location().expect("location should have been captured");
        assert!(
            loc.ends_with(&format!("error.rs:{before_line}")),
            "expected location to point at this test's own call site, got {loc:?}"
        );
    }

    /// `?`-propagated `sqlx::Error` should capture the location of the `?`
    /// use site, via the manual `From<sqlx::Error>` impl above — not via a
    /// `#[from]` attribute, which cannot be made `#[track_caller]`.
    #[test]
    fn database_from_impl_captures_the_question_mark_call_site() {
        fn fails(expected_line: &mut u32) -> Result<(), AppError> {
            *expected_line = line!() + 1;
            Err(sqlx::Error::RowNotFound)?;
            unreachable!()
        }
        let mut expected_line = 0;
        let _ = fails(&mut expected_line);
        let loc = take_last_error_location().expect("location should have been captured");
        assert!(
            loc.ends_with(&format!("error.rs:{expected_line}")),
            "expected location to point at the `?` call site, got {loc:?}"
        );
    }

    /// A bare `AppError::Internal(...)` construction (bypassing the tracked
    /// constructor) must not fabricate a stale location from a previous,
    /// unrelated call on the same thread.
    #[test]
    fn bare_variant_construction_does_not_leave_a_stale_location() {
        let _ = AppError::internal("prime the thread-local".to_string());
        let _ = take_last_error_location();
        let _ = AppError::Internal("constructed without the tracked helper".to_string());
        assert_eq!(take_last_error_location(), None);
    }
}
