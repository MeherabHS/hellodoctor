//! Every error response reaches the Admin incident log (product owner,
//! 2026-11-20: "every error incident has to be reported from server side").
//!
//! `system_error.rs` has always recorded genuine server failures (5xx,
//! dependency outages, panics). Refused requests — 4xx such as a document
//! that cannot be opened, a forbidden action or a validation failure — were
//! invisible on the incident log, so a patient's "unable to open" left no
//! trace. This outermost layer records every response with status >= 400
//! that was not already reported, wherever it came from: a handler, auth,
//! rate limiting, admission control, an unknown route or a body-limit
//! rejection.
//!
//! Refused requests are stored under the [`SUBSYSTEM`] category at
//! `WARNING`, separate from the server-failure categories, and group by
//! method + normalized route + error code (ids are replaced), so a recurring
//! refusal is one incident with an occurrence count, not thousands of rows.
//! Only the server's own fixed message text is stored, redacted, never request
//! bodies.
//!
//! Two kinds of response are expected noise, not incidents, and are skipped
//! ([`is_expected_noise`], product owner, 2026-11-23):
//! - a 401 for a genuine access token that has simply expired: every app
//!   refreshes and retries on it, so after 15 minutes away each one used to
//!   add rows. Missing, forged, tampered or malformed tokens are still
//!   recorded.
//! - a 404 outside `/api/v1/` (`/`, `/favicon.ico`, `/wp-login.php`): a
//!   browser or a scanner opening the API address. The edge proxy answers
//!   these itself; this is the fallback when it does not.

use crate::domain::models::ErrorSeverity;
use crate::error::ErrorResponseInfo;
use crate::system_error::{self, ErrorEvent};
use axum::{extract::Request, middleware::Next, response::Response};
use regex::Regex;
use std::sync::LazyLock;

/// Category of refused-request incidents on the Admin incident log.
pub const SUBSYSTEM: &str = "CLIENT_REQUEST";

pub async fn middleware(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let route = normalize_path(request.uri().path());
    let response = next.run(request).await;
    let status = response.status();
    if status.as_u16() < 400 {
        return response;
    }
    let info = response.extensions().get::<ErrorResponseInfo>().cloned();
    if info.as_ref().is_some_and(|info| info.already_reported) {
        return response;
    }
    let (code, message) = match info {
        Some(info) => (info.code, info.message),
        None => (
            status_code_name(status.as_u16()),
            status.canonical_reason().unwrap_or("Error").to_string(),
        ),
    };
    if is_expected_noise(status.as_u16(), code, &message, &route) {
        return response;
    }
    let endpoint = format!("{method} {route}");
    let (subsystem, severity) = if status.is_server_error() {
        ("HTTP", ErrorSeverity::Error)
    } else {
        (SUBSYSTEM, ErrorSeverity::Warning)
    };
    let mut event = ErrorEvent::new(
        severity,
        subsystem,
        "incident_reporting",
        code,
        format!(
            "{endpoint} answered {}: {}",
            status.as_u16(),
            system_error::redact(&message)
        ),
        crate::api::middleware::request_context::request_id(),
    )
    .with_http_status(status.as_u16())
    .with_endpoint(endpoint.clone());
    event.fingerprint_context = Some(endpoint);
    // The Admin incident log only: a refused request adds no
    // CONSULTATION_REQUEST_FAILED reference to the consultation trace (stored
    // in the background, such a reference would also briefly lock the
    // appointment row that payment and expiry work is about to update).
    event.consultation_evidence = None;
    system_error::report(event);
    response
}

/// Responses that are part of normal operation and would only bury real
/// incidents. See the module comment.
pub fn is_expected_noise(status: u16, code: &str, message: &str, path: &str) -> bool {
    let expired_token =
        status == 401 && code == "UNAUTHORIZED" && message == crate::crypto::EXPIRED_ACCESS_TOKEN;
    let not_an_api_path = status == 404 && !path.starts_with("/api/v1/");
    expired_token || not_an_api_path
}

static UUID: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$")
        .unwrap()
});
static NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d+$").unwrap());
static TOKEN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Za-z0-9_-]{24,}$").unwrap());

/// `/api/v1/appointments/0d2c…/documents/9f1e…` → `/api/v1/appointments/:id/documents/:id`.
/// Ids, numbers and token-like segments are replaced so incidents group by
/// route and no identifier ends up in the grouping key.
pub fn normalize_path(path: &str) -> String {
    let normalized: Vec<&str> = path
        .split('/')
        .map(|segment| {
            if UUID.is_match(segment) {
                ":id"
            } else if NUMBER.is_match(segment) {
                ":n"
            } else if TOKEN.is_match(segment) {
                ":token"
            } else if segment.len() > 64 {
                ":long"
            } else {
                segment
            }
        })
        .collect();
    normalized.join("/")
}

fn status_code_name(status: u16) -> &'static str {
    match status {
        400 => "BAD_REQUEST",
        401 => "UNAUTHORIZED",
        403 => "FORBIDDEN",
        404 => "NOT_FOUND",
        405 => "METHOD_NOT_ALLOWED",
        408 => "REQUEST_TIMEOUT",
        409 => "CONFLICT",
        413 => "PAYLOAD_TOO_LARGE",
        415 => "UNSUPPORTED_MEDIA_TYPE",
        422 => "UNPROCESSABLE_ENTITY",
        429 => "TOO_MANY_REQUESTS",
        500 => "INTERNAL_SERVER_ERROR",
        502 => "BAD_GATEWAY",
        503 => "SERVICE_UNAVAILABLE",
        504 => "GATEWAY_TIMEOUT",
        other if other < 500 => "CLIENT_ERROR",
        _ => "SERVER_ERROR",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_and_tokens_never_reach_the_grouping_key() {
        assert_eq!(
            normalize_path("/api/v1/appointments/0d2c6b1e-0000-4000-8000-000000000001/documents/9f1e6b1e-0000-4000-8000-000000000002"),
            "/api/v1/appointments/:id/documents/:id"
        );
        assert_eq!(normalize_path("/api/v1/doctors/42"), "/api/v1/doctors/:n");
        assert_eq!(
            normalize_path("/api/v1/invite/abcdefghijklmnopqrstuvwxyz0123"),
            "/api/v1/invite/:token"
        );
        assert_eq!(
            normalize_path("/api/v1/doctor/chats"),
            "/api/v1/doctor/chats"
        );
    }

    #[test]
    fn expected_noise_is_skipped_and_real_refusals_are_kept() {
        let expired = crate::crypto::EXPIRED_ACCESS_TOKEN;
        // Skipped: an expired token, and 404s outside the API.
        assert!(is_expected_noise(
            401,
            "UNAUTHORIZED",
            expired,
            "/api/v1/patients/me"
        ));
        assert!(is_expected_noise(
            404,
            "NOT_FOUND",
            "Not Found",
            "/favicon.ico"
        ));
        assert!(is_expected_noise(404, "NOT_FOUND", "Not Found", "/"));
        assert!(is_expected_noise(
            404,
            "NOT_FOUND",
            "Not Found",
            "/wp-login.php"
        ));
        // Kept: forged or malformed tokens, missing tokens, other 401s.
        let forged = "Invalid or expired access token: InvalidSignature";
        assert!(!is_expected_noise(
            401,
            "UNAUTHORIZED",
            forged,
            "/api/v1/patients/me"
        ));
        assert!(!is_expected_noise(
            401,
            "UNAUTHORIZED",
            "Missing bearer token.",
            "/api/v1/doctor/chats"
        ));
        assert!(!is_expected_noise(
            401,
            "UNAUTHORIZED",
            "Invalid or expired verification code.",
            "/api/v1/auth/password-reset/verify-otp"
        ));
        // Kept: every 404 inside the API, and other statuses outside it.
        assert!(!is_expected_noise(
            404,
            "RESOURCE_NOT_FOUND",
            "Document not found.",
            "/api/v1/appointments/:id/documents/:id"
        ));
        assert!(!is_expected_noise(403, "FORBIDDEN", "Forbidden", "/"));
        assert!(!is_expected_noise(
            500,
            "INTERNAL_SERVER_ERROR",
            "boom",
            "/favicon.ico"
        ));
    }

    #[test]
    fn unknown_statuses_fall_back_to_their_class() {
        assert_eq!(status_code_name(418), "CLIENT_ERROR");
        assert_eq!(status_code_name(599), "SERVER_ERROR");
        assert_eq!(status_code_name(404), "NOT_FOUND");
    }
}
