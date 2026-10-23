//! In-app notification inbox — own-notifications-only (AuthContext identity,
//! no client-supplied recipient id ever accepted), enforced by both the
//! handler layer here and RLS (`notifications_owner_select`/
//! `notifications_owner_mark_read`) as a second, independent layer.

use crate::abac::{self, Action};
use crate::auth_context::AuthContext;
use crate::error::AppError;
use crate::repository::AppState;
use crate::services::NotificationService;
use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: T,
}

#[derive(Debug, Deserialize)]
pub struct PageQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub after_time: Option<chrono::DateTime<chrono::Utc>>,
    pub after_id: Option<Uuid>,
    pub before_time: Option<chrono::DateTime<chrono::Utc>>,
    pub before_id: Option<Uuid>,
}

/// GET /api/v1/notifications
pub async fn list_own_notifications(
    State(state): State<AppState>,
    ctx: AuthContext,
    Query(q): Query<PageQuery>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::AnyAuthenticated)?;
    let cursor = match (q.after_time, q.after_id, q.before_time, q.before_id) {
        (Some(time), Some(id), None, None) => Some((time, id, false)),
        (None, None, Some(time), Some(id)) => Some((time, id, true)),
        (None, None, None, None) => None,
        _ => {
            return Err(AppError::Validation(
                "Use one complete notification cursor.".into(),
            ))
        }
    };
    let items = if let Some((time, id, before)) = cursor {
        NotificationService::list_cursor(&state, &ctx, q.limit.unwrap_or(25), time, id, before)
            .await?
    } else {
        NotificationService::list_own(&state, &ctx, q.limit.unwrap_or(25), q.offset.unwrap_or(0))
            .await?
    };
    Ok(Json(ApiResponse {
        success: true,
        data: items,
    }))
}

/// GET /api/v1/notifications/unread-count
pub async fn unread_count(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::AnyAuthenticated)?;
    let count = NotificationService::unread_count(&state, &ctx).await?;
    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({ "unread_count": count }),
    }))
}

/// POST /api/v1/notifications/:id/read
pub async fn mark_read(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::AnyAuthenticated)?;
    let marked = NotificationService::mark_read(&state, &ctx, id).await?;
    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({ "id": id, "marked": marked }),
    }))
}

/// POST /api/v1/notifications/read-all
pub async fn mark_all_read(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::AnyAuthenticated)?;
    let count = NotificationService::mark_all_read(&state, &ctx).await?;
    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({ "marked_count": count }),
    }))
}

#[derive(Debug, Deserialize)]
pub struct RegisterDeviceRequest {
    pub device_id: Option<Uuid>,
    pub platform: String,
    pub push_token: String,
}

/// POST /api/v1/notifications/devices — device-token registration contract
/// The token is treated as a provider credential belonging to this account;
/// the database's global uniqueness guard prevents reassignment to another
/// account without an explicit provider-side token rotation.
pub async fn register_device(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(payload): Json<RegisterDeviceRequest>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::AnyAuthenticated)?;
    let platform = payload.platform.to_uppercase();
    if !["ANDROID", "IOS", "WEB"].contains(&platform.as_str()) {
        return Err(AppError::Validation(
            "platform must be ANDROID, IOS, or WEB.".into(),
        ));
    }
    let push_token = crate::sanitize::sanitize_text(&payload.push_token, 4096)?;
    if push_token.trim().is_empty() {
        return Err(AppError::Validation("push_token must not be empty.".into()));
    }
    let device_id = payload.device_id.unwrap_or_else(Uuid::new_v4);
    NotificationService::register_device(&state, &ctx, device_id, &platform, &push_token)
        .await
        .map_err(|error| match error {
            AppError::Database(sqlx::Error::Database(database_error))
                if database_error.code().as_deref() == Some("23505") =>
            {
                AppError::Conflict(
                    "This push token is already registered to another account.".into(),
                )
            }
            other => other,
        })?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(ApiResponse {
            success: true,
            data: serde_json::json!({
                "status": "REGISTERED",
                "push_provider": if state.notification_transport.is_configured() { "FCM_HTTP_V1" } else { "NOOP_DEVELOPMENT" }
            }),
        }),
    ))
}

#[derive(Debug, Deserialize)]
pub struct RevokeDeviceRequest {
    pub device_id: Option<Uuid>,
    pub push_token: Option<String>,
}

/// POST /api/v1/notifications/devices/revoke
pub async fn revoke_device(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(payload): Json<RevokeDeviceRequest>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::AnyAuthenticated)?;
    let push_token = payload
        .push_token
        .as_deref()
        .map(|token| crate::sanitize::sanitize_text(token, 4096))
        .transpose()?;
    if payload.device_id.is_none() && push_token.as_deref().is_none() {
        return Err(AppError::Validation(
            "device_id or push_token is required.".into(),
        ));
    }
    let disabled =
        NotificationService::revoke_device(&state, &ctx, payload.device_id, push_token.as_deref())
            .await?;
    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({ "status": "REVOKED", "disabled_count": disabled }),
    }))
}
