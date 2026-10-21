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
    pub platform: String,
    pub push_token: String,
}

/// POST /api/v1/notifications/devices — device-token registration contract
/// (see `notification_transport.rs`: no real push provider is configured,
/// this only stores the registration for whenever one is).
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
    NotificationService::register_device(&state, &ctx, Uuid::new_v4(), &platform, &push_token)
        .await?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(ApiResponse {
            success: true,
            data: serde_json::json!({ "status": "REGISTERED", "push_provider": "CONFIGURATION_REQUIRED" }),
        }),
    ))
}
