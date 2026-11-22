//! Activity Log (product owner, 2026-11-14): every Admin can see who did what
//! and when, newest first, read live from `audit_events`.
//!
//! Privacy: Admins are named by email; patient and doctor actors appear only
//! by role. `before_data`, `after_data`, `reason`, IP hashes and request ids
//! are never returned, since they can hold clinical or personal detail. Every
//! filter is bound as a query parameter (no SQL is built from input).
use crate::abac::{self, Action};
use crate::auth_context::AuthContext;
use crate::error::AppError;
use crate::repository::AppState;
use axum::{
    extract::{Query, State},
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Serialize)]
struct ApiResponse<T> {
    success: bool,
    data: T,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivityQuery {
    pub limit: Option<i64>,
    /// Cursor: only events strictly older than this instant.
    pub before: Option<DateTime<Utc>>,
    /// content, people, operations, campaigns, finance, security, team; absent = all.
    pub area: Option<String>,
    /// Case-insensitive match on the action name, e.g. "publish".
    pub search: Option<String>,
    /// Only actions by this user (an Admin's id from the team list).
    pub actor_id: Option<Uuid>,
    /// Only the caller's own actions (takes precedence over `actor_id`).
    pub mine: Option<bool>,
}

/// Resource types per area. Unknown resource types still appear under "all".
fn area_resources(area: &str) -> Result<&'static [&'static str], AppError> {
    Ok(match area {
        "content" => &[
            "ADMIN_ARTICLE",
            "ARTICLE_QUIZ",
            "ARTICLE_CATEGORY",
            "HEALTH_FACILITY",
            "DOCTOR_ARTICLE",
            "HEALTH_ANSWER",
        ],
        "people" => &[
            "USER",
            "DOCTOR_PROFILE",
            "PATIENT_PROFILE",
            "PATIENT_HISTORY",
            "DATA_RIGHTS_REQUEST",
            "PATIENT_DATA_REQUEST",
            "PATIENT",
            "ERASURE_JOB",
            "IDENTITY_TRACE",
        ],
        "operations" => &[
            "SCHEDULE_SLOT",
            "APPOINTMENT",
            "CONSULTATION_SESSION",
            "GRIEVANCE_REPORT",
            "SYSTEM_ERROR",
            "SYSTEM_ERROR_LOG",
            "PRESCRIPTION",
        ],
        "campaigns" => &["CAMPAIGN", "NOTIFICATION_CAMPAIGN"],
        "finance" => &[
            "TRANSACTION",
            "DISBURSEMENT",
            "DISBURSEMENT_ITEM",
            "DISBURSEMENT_BATCH",
            "FINANCIAL_PERIOD",
            "FINANCIAL_REFUND_REQUEST",
            "REFUND_REQUEST",
            "PLATFORM_PAYMENT_METHOD",
            "MONTHLY_STATEMENT",
        ],
        "security" => &["AUTH_SESSION", "ADMIN_SIGN_IN"],
        "team" => &["ADMIN_INVITATION"],
        _ => {
            return Err(AppError::Validation(
                "area must be content, people, operations, campaigns, finance, security or team."
                    .into(),
            ))
        }
    })
}

type EventRow = (
    Uuid,
    DateTime<Utc>,
    Option<Uuid>,
    String,
    String,
    String,
    Option<Uuid>,
    String,
);

/// GET /api/v1/admin/activity
pub async fn list_activity(
    State(state): State<AppState>,
    ctx: AuthContext,
    Query(q): Query<ActivityQuery>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ViewLogs)?;
    let limit = q.limit.unwrap_or(50).clamp(1, 100);
    let resources: Vec<String> = match q.area.as_deref().filter(|a| !a.is_empty() && *a != "all") {
        Some(area) => area_resources(area)?
            .iter()
            .map(|s| s.to_string())
            .collect(),
        None => Vec::new(),
    };
    let search = q.search.as_deref().map(str::trim).unwrap_or("");
    if search.chars().count() > 80
        || !search
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == ' ')
    {
        return Err(AppError::Validation(
            "Search by action name: letters, digits, spaces and _ only.".into(),
        ));
    }
    let pattern = format!("%{}%", search.replace(' ', "_"));

    let mut tx = state.begin_authed(&ctx).await?;
    let rows: Vec<EventRow> = sqlx::query_as(
        "SELECT id, created_at, actor_id, actor_role::text, action, resource_type, resource_id, result \
         FROM audit_events \
         WHERE ($1::timestamptz IS NULL OR created_at < $1) \
           AND (cardinality($2::text[]) = 0 OR resource_type = ANY($2)) \
           AND ($3 = '%%' OR action ILIKE $3 \
                OR ($6 AND resource_type IN ('SYSTEM_ERROR', 'SYSTEM_ERROR_LOG'))) \
           AND ($4::uuid IS NULL OR actor_id = $4) \
         ORDER BY created_at DESC, id DESC LIMIT $5",
    )
    .bind(q.before)
    .bind(&resources)
    .bind(&pattern)
    .bind(if q.mine == Some(true) {
        Some(ctx.user_id.into_inner())
    } else {
        q.actor_id
    })
    .bind(limit)
    // The Admin screens call system errors "incidents"; searching for that
    // word finds their resolve/acknowledge entries too.
    .bind(search.to_ascii_lowercase().contains("incident"))
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;

    // Admin actors are named by email; everyone else only by role.
    let admin_ids: Vec<Uuid> = rows
        .iter()
        .filter(|r| r.3 == "ADMIN")
        .filter_map(|r| r.2)
        .collect();
    let emails: HashMap<Uuid, String> = if admin_ids.is_empty() {
        HashMap::new()
    } else {
        sqlx::query_as::<_, (Uuid, Option<String>)>(
            "SELECT id, email FROM users WHERE id = ANY($1) AND role='ADMIN'",
        )
        .bind(&admin_ids)
        .fetch_all(&state.auth_db)
        .await?
        .into_iter()
        .filter_map(|(id, email)| email.map(|e| (id, e)))
        .collect()
    };
    let next_before = if rows.len() as i64 == limit {
        rows.last().map(|r| r.1)
    } else {
        None
    };
    let items: Vec<_> = rows
        .into_iter()
        .map(
            |(id, at, actor, role, action, resource_type, resource_id, result)| {
                serde_json::json!({
                    "id": id,
                    "at": at,
                    "actor_role": role,
                    "actor_email": actor.and_then(|a| emails.get(&a).cloned()),
                    "is_you": actor == Some(ctx.user_id.into_inner()),
                    "action": action,
                    "resource_type": resource_type,
                    "resource_id": resource_id,
                    "result": result,
                })
            },
        )
        .collect();
    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({ "items": items, "next_before": next_before }),
    }))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationQuery {
    /// Server time from the previous response; absent = the last 24 hours.
    pub since: Option<DateTime<Utc>>,
}

/// Non-Admin events every Admin should hear about. Admin actions by other
/// Admins are always included (except sign-ins and record views).
const ATTENTION_ACTIONS: &[&str] = &[
    "DOCTOR_PROFILE_CREATED",
    "PEER_NAVIGATOR_CREATED",
    "DATA_RIGHTS_ERASURE_REQUESTED",
    "AUTH_ROLE_MISMATCH",
    "CONCURRENT_LIVE_PARTICIPATION_DETECTED",
    "LATE_PAYMENT_RECONCILIATION_REQUIRED",
];

type NoticeRow = (
    Uuid,
    DateTime<Utc>,
    String,
    String,
    String,
    String,
    Option<Uuid>,
);

/// GET /api/v1/admin/notifications — pop-up notifications for every Admin
/// (product owner, 2026-11-14): other Admins' actions, new doctor
/// applications, new grievances, new app incidents and security events since
/// `since`. Carries no patient names, complaint text or error details; the
/// grievance is identified by its number and the incident by severity and
/// component only.
pub async fn notifications(
    State(state): State<AppState>,
    ctx: AuthContext,
    Query(q): Query<NotificationQuery>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ViewLogs)?;
    let now = Utc::now();
    let since = q
        .since
        .filter(|s| *s <= now)
        .map(|s| s.max(now - chrono::Duration::days(7)))
        .unwrap_or(now - chrono::Duration::hours(24));
    let me = ctx.user_id.into_inner();
    let attention: Vec<String> = ATTENTION_ACTIONS.iter().map(|s| s.to_string()).collect();

    let mut tx = state.begin_authed(&ctx).await?;
    let events: Vec<NoticeRow> = sqlx::query_as(
        "SELECT id, created_at, actor_role::text, action, resource_type, result, actor_id \
         FROM audit_events \
         WHERE created_at > $1 AND actor_id IS DISTINCT FROM $2 \
           AND ((actor_role = 'ADMIN' AND actor_id IS NOT NULL \
                 AND action <> 'ADMIN_SIGNED_IN' AND right(action, 7) <> '_VIEWED' \
                 AND result = 'SUCCESS') \
                OR action = ANY($3)) \
         ORDER BY created_at DESC, id DESC LIMIT 50",
    )
    .bind(since)
    .bind(me)
    .bind(&attention)
    .fetch_all(&mut *tx)
    .await?;
    let grievances: Vec<(Uuid, DateTime<Utc>, String)> = sqlx::query_as(
        "SELECT id, created_at, grievance_number FROM grievance_reports \
         WHERE created_at > $1 ORDER BY created_at DESC LIMIT 20",
    )
    .bind(since)
    .fetch_all(&mut *tx)
    .await?;
    let incidents: Vec<(Uuid, DateTime<Utc>, String, String)> = sqlx::query_as(
        "SELECT id, logged_at, severity::text, COALESCE(component, subsystem::text, 'app') FROM system_error_logs \
         WHERE logged_at > $1 ORDER BY logged_at DESC LIMIT 20",
    )
    .bind(since)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;

    let admin_ids: Vec<Uuid> = events.iter().filter_map(|e| e.6).collect();
    let emails: HashMap<Uuid, String> = if admin_ids.is_empty() {
        HashMap::new()
    } else {
        sqlx::query_as::<_, (Uuid, Option<String>)>(
            "SELECT id, email FROM users WHERE id = ANY($1) AND role='ADMIN'",
        )
        .bind(&admin_ids)
        .fetch_all(&state.auth_db)
        .await?
        .into_iter()
        .filter_map(|(id, email)| email.map(|e| (id, e)))
        .collect()
    };

    let mut items: Vec<(DateTime<Utc>, serde_json::Value)> = Vec::new();
    for (id, at, role, action, resource_type, result, actor) in events {
        items.push((
            at,
            serde_json::json!({
                "id": id, "kind": "activity", "at": at, "action": action,
                "resource_type": resource_type, "result": result, "actor_role": role,
                "actor_email": actor.and_then(|a| emails.get(&a).cloned()),
            }),
        ));
    }
    for (id, at, number) in grievances {
        items.push((
            at,
            serde_json::json!({
                "id": id, "kind": "grievance", "at": at, "grievance_number": number,
            }),
        ));
    }
    for (id, at, severity, component) in incidents {
        items.push((at, serde_json::json!({
            "id": id, "kind": "incident", "at": at, "severity": severity, "component": component,
        })));
    }
    items.sort_by_key(|item| std::cmp::Reverse(item.0));
    items.truncate(60);
    let items: Vec<serde_json::Value> = items.into_iter().map(|(_, v)| v).collect();
    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({ "items": items, "server_time": now }),
    }))
}

/// Records that an Admin opened a record the console already had loaded
/// (a patient, a doctor, a Peer Navigator), so viewing leaves an audit row
/// even when no detail request reaches the server.
#[derive(Deserialize)]
pub struct RecordViewRequest {
    pub resource_type: String,
    pub resource_id: Uuid,
}

/// `POST /admin/audit/views`
pub async fn record_view(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(body): Json<RecordViewRequest>,
) -> Result<impl IntoResponse, AppError> {
    if ctx.role != crate::domain::models::UserRole::Admin {
        return Err(AppError::Forbidden("Admins only.".into()));
    }
    let action = match body.resource_type.as_str() {
        "PATIENT" => "PATIENT_RECORD_VIEWED",
        "DOCTOR_PROFILE" => "DOCTOR_RECORD_VIEWED",
        "PEER_NAVIGATOR" => "PEER_NAVIGATOR_RECORD_VIEWED",
        _ => return Err(AppError::Validation("Unknown record type.".into())),
    };
    crate::services::audit_service::AuditService::log(
        &state,
        &ctx,
        crate::services::audit_service::AuditLogEntry::new(
            Some(ctx.user_id),
            ctx.role,
            action,
            body.resource_type.as_str(),
            Some(body.resource_id),
            "SUCCESS",
        ),
    )
    .await?;
    Ok(Json(
        serde_json::json!({ "success": true, "data": { "recorded": true } }),
    ))
}
