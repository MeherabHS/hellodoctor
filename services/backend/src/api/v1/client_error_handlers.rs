//! Authenticated doctor-only, best-effort ingestion into the existing error service.
//! Never retain arbitrary exception prose: it can embed clinical data even after
//! token/phone regex redaction. Source frames and bounded vocabulary carry diagnostics.
use crate::{
    auth_context::AuthContext,
    domain::models::{ErrorSeverity, UserRole},
    repository::AppState,
    system_error::{self, ErrorEvent},
};
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::Deserialize;
use std::sync::LazyLock;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClientErrorRequest {
    pub client: String,
    pub error_type: String,
    pub message: String,
    pub stack_trace: String,
    pub screen: String,
    pub operation: String,
    pub app_version: String,
    pub build_number: String,
    pub platform: String,
    pub occurred_at: DateTime<Utc>,
    pub backend_reference: Option<Uuid>,
    pub release_sha: Option<String>,
    pub startup_reference: Option<String>,
}

const TYPES: &[&str] = &[
    "Exception",
    "Error",
    "TypeError",
    "StateError",
    "FormatException",
    "NoSuchMethodError",
    "RangeError",
    "AssertionError",
    "PlatformException",
    "SyntaxError",
    "ReferenceError",
    "API_FAILURE",
    "RESPONSE_INVALID",
];
static WEB_BUILD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[a-zA-Z0-9_-]{1,64}$").unwrap());
const SCREENS: &[&str] = &[
    "Application",
    "Auth",
    "Queue",
    "Chat",
    "SmartRx",
    "Schedule",
    "Wallet",
    "Qna",
    "Telehealth",
    "Settings",
    "Notifications",
    "Patients",
];
const OPERATIONS: &[&str] = &[
    "render",
    "unhandled_async",
    "browser_error",
    "load",
    "send_message",
    "create_slot",
    "block_slot",
    "cancel_slot",
    "book_appointment",
    "login",
    "verify_login",
    "load_consultation",
    "upload_prescription",
    "complete_consultation",
    "call_telemetry",
    "mint_rtc_token",
    "load_document",
    "update",
    "response_parse",
    "startup",
];
static VERSION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\d{1,4}\.\d{1,4}\.\d{1,4}(?:-[a-zA-Z0-9.]{1,20})?$").unwrap());
static FRAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
    r"(?:package:(?:hellodoctor_mobile|flutter)/[a-zA-Z0-9_./-]+\.dart:\d{1,7}:\d{1,7}|dart:[a-z_]+/[a-zA-Z0-9_./-]+\.dart:\d{1,7}:\d{1,7}|/_next/static/(?:chunks|webpack)/[a-zA-Z0-9_./%\[\]-]+\.(?:js|tsx?):\d{1,7}:\d{1,7})"
).unwrap()
});

pub fn safe_stack(raw: &str) -> String {
    // Only source locations survive; no exception headers, URLs, arguments or
    // arbitrary function names (which can contain user-supplied strings).
    FRAME
        .find_iter(raw)
        .take(24)
        .map(|m| m.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn normalize(p: &ClientErrorRequest, user: Uuid) -> Result<ErrorEvent, &'static str> {
    if p.message.len() > 2000
        || p.stack_trace.len() > 6000
        || p.error_type.len() > 64
        || p.screen.len() > 32
        || p.operation.len() > 40
        || p.app_version.len() > 40
        || p.build_number.len() > 64
        || p.platform.len() > 16
    {
        return Err("Client incident exceeds field limits.");
    }
    let (subsystem, component) = match p.client.as_str() {
        "DOCTOR_FLUTTER" => ("DOCTOR_FLUTTER", "FlutterExceptionBoundary"),
        "PATIENT_FLUTTER" => ("PATIENT_FLUTTER", "FlutterExceptionBoundary"),
        "DOCTOR_WEB" => ("DOCTOR_WEB", "DoctorWebRuntime"),
        "ADMIN_WEB" => ("ADMIN_WEB", "AdminWebRuntime"),
        _ => return Err("Unsupported client."),
    };
    if !VERSION.is_match(&p.app_version)
        || p.build_number.is_empty()
        || !(p.build_number.bytes().all(|c| c.is_ascii_digit())
            || (matches!(p.client.as_str(), "DOCTOR_WEB" | "ADMIN_WEB")
                && WEB_BUILD.is_match(&p.build_number)))
        || !["android", "ios", "web", "windows", "macos", "linux"].contains(&p.platform.as_str())
    {
        return Err("Invalid release metadata.");
    }
    if p.release_sha
        .as_ref()
        .is_some_and(|s| s.len() != 40 || !s.bytes().all(|c| c.is_ascii_hexdigit()))
    {
        return Err("Invalid release SHA.");
    }
    if let Some(reference) = &p.startup_reference {
        if p.client != "DOCTOR_FLUTTER"
            || p.operation != "startup"
            || reference.len() != 8
            || !reference
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'A'..=b'F').contains(&c))
        {
            return Err("Invalid startup reference.");
        }
    }
    let kind = if TYPES.contains(&p.error_type.as_str()) {
        p.error_type.as_str()
    } else {
        "Exception"
    };
    let screen = if SCREENS.contains(&p.screen.as_str()) {
        p.screen.as_str()
    } else {
        "Application"
    };
    let operation = if OPERATIONS.contains(&p.operation.as_str()) {
        p.operation.as_str()
    } else {
        "unhandled_async"
    };
    let mut event = ErrorEvent::new(
        ErrorSeverity::Error,
        subsystem,
        component,
        if p.operation == "startup" {
            "CLIENT_STARTUP_EXCEPTION"
        } else {
            "CLIENT_EXCEPTION"
        },
        format!("Unexpected client exception ({kind}); arbitrary message omitted for privacy."),
        crate::api::middleware::request_context::request_id(),
    )
    .with_user(user)
    .with_endpoint("/api/v1/client-errors");
    event.action_attempted = operation.into();
    event.stack_trace = Some(safe_stack(&p.stack_trace));
    event.client_context = Some(serde_json::json!({
        "client": subsystem, "error_type": kind, "screen": screen, "operation": operation,
        "app_version": p.app_version, "build_number": p.build_number, "platform": p.platform,
        "occurred_at": p.occurred_at, "backend_reference": p.backend_reference, "release_sha": p.release_sha,
        "source": if p.operation == "startup" { "deferred_startup_report" } else { "authenticated_client_report" },
        "startup_reference": p.startup_reference, "measurements_verified": false
    }));
    // Keep distinct doctors/builds/errors separate. The existing backend grouping
    // remains unchanged. No raw message, timestamp, JWT or client-owned fingerprint.
    event.fingerprint_context = Some(format!(
        "{user}:{}:{}:{}:{kind}:{screen}:{operation}",
        p.app_version, p.build_number, p.platform
    ));
    Ok(event)
}

pub async fn submit_client_error(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(payload): Json<ClientErrorRequest>,
) -> impl IntoResponse {
    let fail = |status, message| {
        (
            status,
            Json(serde_json::json!({"success": false,
        "error": {"code": "CLIENT_INCIDENT_REJECTED", "message": message}})),
        )
    };
    if !matches!(
        (ctx.role, payload.client.as_str()),
        (UserRole::Doctor, "DOCTOR_WEB" | "DOCTOR_FLUTTER")
            | (UserRole::Patient, "PATIENT_FLUTTER")
            | (UserRole::Admin, "ADMIN_WEB")
    ) {
        return fail(
            StatusCode::FORBIDDEN,
            "Patient, Doctor, or Admin session required.",
        );
    }
    let event = match normalize(&payload, ctx.user_id.into()) {
        Ok(event) => event,
        Err(message) => return fail(StatusCode::UNPROCESSABLE_ENTITY, message),
    };
    // Dedicated per-actor ceiling, fail closed. A reporting-store failure is not
    // recursively converted into another operational incident.
    match crate::rate_limit::check_and_increment(
        &state.redis,
        &format!("ratelimit:client-errors:{}", ctx.user_id),
        12,
        60,
    )
    .await
    {
        Ok(()) => {}
        Err(crate::error::AppError::RateLimitExceeded(_)) => {
            return fail(
                StatusCode::TOO_MANY_REQUESTS,
                "Incident rate limit reached.",
            )
        }
        Err(_) => {
            return fail(
                StatusCode::SERVICE_UNAVAILABLE,
                "Incident reporting unavailable.",
            )
        }
    }
    match tokio::time::timeout(
        std::time::Duration::from_secs(2),
        system_error::report_sync(&state.system_db, event),
    )
    .await
    {
        Ok(Ok(id)) => (
            StatusCode::ACCEPTED,
            Json(serde_json::json!({"success": true, "data": {"incident_id": id}})),
        ),
        _ => fail(
            StatusCode::SERVICE_UNAVAILABLE,
            "Incident reporting unavailable.",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stack_keeps_source_locations_only() {
        let result = safe_stack("patient=Jane diagnosis=asthma token=secret\n#0 load (package:hellodoctor_mobile/features/doctor/doctor_repository.dart:33:7)\n at send (https://example.org/_next/static/chunks/app/page.js:22:8)\nhttps://documents.test/private?token=secret");
        assert_eq!(result, "package:hellodoctor_mobile/features/doctor/doctor_repository.dart:33:7\n/_next/static/chunks/app/page.js:22:8");
    }
}
