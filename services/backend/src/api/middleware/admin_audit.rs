//! Every Admin action is audited (product owner, 2026-11-15).
//!
//! Most Admin handlers write their own, specific audit row
//! (`DOCTOR_PROFESSIONAL_VERIFY`, `PEER_NAVIGATOR_APPROVED`, ...). This layer
//! is the safety net around every `/admin/...` route: when the handler wrote
//! no audit row of its own, it records one for
//!
//! * every change (POST, PUT, PATCH, DELETE), successful or not, and
//! * every read of one record or its details (a GET whose route has an
//!   `:id`-style parameter, e.g. a patient dossier, a doctor's verification
//!   documents, a grievance timeline).
//!
//! Lists, dashboards and polling (notifications, activity, overview) are not
//! recorded: they would bury the log. The row holds the route template, not
//! the concrete URL or any body, so no query text or PHI reaches the log.
//!
//! Whether the handler already audited is tracked with a task-local flag set
//! by `audit_repo::insert`; the handler runs inside this layer's future.

use crate::auth_context::AuthContext;
use crate::domain::models::UserRole;
use crate::repository::AppState;
use crate::services::audit_service::{AuditLogEntry, AuditService};
use axum::{
    extract::{MatchedPath, Request, State},
    http::Method,
    middleware::Next,
    response::Response,
};
use std::cell::Cell;
use uuid::Uuid;

tokio::task_local! {
    static AUDITED: Cell<bool>;
}

/// Called by `audit_repo::insert`: this request wrote its own audit row.
pub fn mark_audited() {
    let _ = AUDITED.try_with(|flag| flag.set(true));
}

/// GET routes that are lists or polling even though they are Admin-only.
/// The patient's peer sessions load with their record, whose opening is
/// already recorded through `POST /admin/audit/views`.
const UNAUDITED_READS: &[&str] = &[
    "/admin/notifications",
    "/admin/activity",
    "/admin/patients/:id/peer-sessions",
];

/// The resource an Admin route acts on, from its first path segment.
pub fn resource_type_for(route: &str) -> &'static str {
    let first = route
        .trim_start_matches("/api/v1")
        .trim_start_matches("/admin/")
        .split('/')
        .next()
        .unwrap_or_default();
    match first {
        "patients" | "patient-data-requests" => "PATIENT",
        "doctors" => "DOCTOR_PROFILE",
        "peers" | "peer-directory" => "PEER_NAVIGATOR",
        "articles" | "article-categories" => "ARTICLE",
        "facilities" | "facility-services" => "FACILITY",
        "campaigns" => "CAMPAIGN",
        "grievances" | "grievance-appeals" => "GRIEVANCE",
        "team" => "ADMIN_TEAM",
        "finance" | "disbursements" | "platform-payment-methods" => "FINANCE",
        "consultations" | "appointments" | "slots" => "APPOINTMENT",
        "system-errors" | "logs" => "SYSTEM_ERROR",
        "quizzes" => "QUIZ",
        _ => "ADMIN_CONSOLE",
    }
}

/// The audit action for an unaudited Admin request. Reads that happen
/// whenever a record is opened are named after that record.
pub fn action_for(method: &Method, route: &str) -> &'static str {
    if *method != Method::GET {
        return "ADMIN_ACTION";
    }
    match route.trim_start_matches("/api/v1") {
        "/admin/patients/:id/dossier" => "PATIENT_RECORD_VIEWED",
        "/admin/doctors/:id/dossier" | "/admin/doctors/:id/verification-submissions" => {
            "DOCTOR_RECORD_VIEWED"
        }
        "/admin/doctors/:id/verification-documents/:document_id" => "DOCTOR_DOCUMENT_VIEWED",
        _ => "ADMIN_RECORD_VIEWED",
    }
}

/// `true` when this Admin request must leave an audit row.
pub fn needs_audit(method: &Method, route: &str) -> bool {
    let route = route.trim_start_matches("/api/v1");
    if !route.starts_with("/admin/") || UNAUDITED_READS.contains(&route) {
        return false;
    }
    match *method {
        Method::GET => route.contains("/:"),
        Method::HEAD | Method::OPTIONS => false,
        _ => true,
    }
}

pub async fn middleware(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let ctx = request.extensions().get::<AuthContext>().cloned();
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map(|p| p.as_str().to_owned());
    let method = request.method().clone();
    let (Some(ctx), Some(route)) = (ctx, route) else {
        return next.run(request).await;
    };
    if ctx.role != UserRole::Admin || !needs_audit(&method, &route) {
        return next.run(request).await;
    }
    // First UUID in the concrete path is the record acted on.
    let resource_id = request
        .uri()
        .path()
        .split('/')
        .find_map(|segment| Uuid::parse_str(segment).ok());

    AUDITED
        .scope(Cell::new(false), async move {
            let response = next.run(request).await;
            if AUDITED.with(|flag| flag.get()) {
                return response;
            }
            let status = response.status();
            let template = route.trim_start_matches("/api/v1").to_owned();
            let action = action_for(&method, &template);
            let entry = AuditLogEntry::new(
                Some(ctx.user_id),
                ctx.role,
                action,
                resource_type_for(&template),
                resource_id,
                if status.is_success() {
                    "SUCCESS"
                } else {
                    "FAILED"
                },
            )
            .with_after(serde_json::json!({
                "method": method.as_str(),
                "route": template,
                "status": status.as_u16(),
            }));
            if let Err(error) = AuditService::log(&state, &ctx, entry).await {
                tracing::error!(%error, "admin action audit could not be written");
            }
            response
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changes_and_record_reads_are_audited_lists_are_not() {
        assert!(needs_audit(&Method::POST, "/api/v1/admin/campaigns"));
        assert!(needs_audit(&Method::DELETE, "/admin/articles/:id"));
        assert!(needs_audit(&Method::GET, "/admin/patients/:id/dossier"));
        assert!(needs_audit(
            &Method::GET,
            "/admin/doctors/:id/verification-documents/:document_id"
        ));
        assert!(!needs_audit(&Method::GET, "/admin/patients"));
        assert!(!needs_audit(&Method::GET, "/admin/notifications"));
        assert!(!needs_audit(&Method::GET, "/admin/activity"));
        assert!(!needs_audit(&Method::POST, "/patients/me"));
    }

    #[test]
    fn opening_a_record_is_named_after_it() {
        assert_eq!(
            action_for(&Method::GET, "/api/v1/admin/patients/:id/dossier"),
            "PATIENT_RECORD_VIEWED"
        );
        assert_eq!(
            action_for(&Method::GET, "/admin/doctors/:id/verification-submissions"),
            "DOCTOR_RECORD_VIEWED"
        );
        assert_eq!(
            action_for(&Method::GET, "/admin/facilities/:id"),
            "ADMIN_RECORD_VIEWED"
        );
        assert_eq!(
            action_for(&Method::POST, "/admin/patients/:id/dossier"),
            "ADMIN_ACTION"
        );
    }

    #[test]
    fn routes_map_to_their_resource() {
        assert_eq!(resource_type_for("/admin/patients/:id/dossier"), "PATIENT");
        assert_eq!(
            resource_type_for("/admin/doctors/:id/verify"),
            "DOCTOR_PROFILE"
        );
        assert_eq!(
            resource_type_for("/admin/peers/:peer_id/history"),
            "PEER_NAVIGATOR"
        );
        assert_eq!(resource_type_for("/admin/something-new"), "ADMIN_CONSOLE");
    }
}
