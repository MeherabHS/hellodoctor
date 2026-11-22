//! The signed-in Admin's own profile: who they are, and their profile photo.
//! Password changes use the shared `POST /auth/change-password`.
//!
//! The photo is returned as JSON (base64) rather than raw bytes: Admin Web
//! reaches the backend only through its text-forwarding same-origin proxy,
//! and a profile photo is at most `MAX_PROFILE_PICTURE_BYTES` (40 KB).

use crate::abac::{self, Action};
use crate::auth_context::AuthContext;
use crate::error::AppError;
use crate::rate_limit;
use crate::repository::AppState;
use crate::services::profile_picture_service::ProfilePictureService;
use axum::{extract::State, Json};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UploadAdminPhotoRequest {
    pub mime_type: String,
    pub base64_content: String,
}

impl std::fmt::Debug for UploadAdminPhotoRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UploadAdminPhotoRequest")
            .field("mime_type", &self.mime_type)
            .field(
                "base64_content",
                &format!("[{} chars]", self.base64_content.len()),
            )
            .finish()
    }
}

fn ok(data: Value) -> Json<Value> {
    Json(json!({"success": true, "data": data}))
}

/// Last three digits only, like the Admin Team list.
fn mask_phone(phone: &str) -> String {
    let digits: Vec<char> = phone.chars().collect();
    let tail: String = digits[digits.len().saturating_sub(3)..].iter().collect();
    format!("•••• {tail}")
}

/// GET /api/v1/admin/me
pub async fn me(State(state): State<AppState>, ctx: AuthContext) -> Result<Json<Value>, AppError> {
    abac::authorize(&ctx, Action::AdminSelf)?;
    let user_id = ctx.user_id.into_inner();
    let (email, phone, created_at): (Option<String>, String, DateTime<Utc>) = sqlx::query_as(
        "SELECT email, phone_number, created_at FROM users WHERE id=$1 AND role='ADMIN' AND deleted_at IS NULL",
    )
    .bind(user_id)
    .fetch_optional(&state.auth_db)
    .await?
    .ok_or_else(|| AppError::NotFound("Admin account not found.".into()))?;
    let mut tx = state.begin_authed(&ctx).await?;
    let photo_updated_at: Option<DateTime<Utc>> =
        sqlx::query_scalar("SELECT updated_at FROM admin_profile_pictures WHERE user_id=$1")
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?;
    tx.commit().await?;
    Ok(ok(json!({
        "email": email,
        "phone": mask_phone(&phone),
        "created_at": created_at,
        "has_photo": photo_updated_at.is_some(),
        "photo_updated_at": photo_updated_at,
    })))
}

/// GET /api/v1/admin/me/photo
pub async fn get_photo(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<Json<Value>, AppError> {
    abac::authorize(&ctx, Action::AdminSelf)?;
    let photo = ProfilePictureService::get_admin_photo(&state, &ctx).await?;
    Ok(ok(json!({
        "mime_type": photo.mime_type,
        "base64_content": STANDARD.encode(photo.bytes),
    })))
}

/// POST /api/v1/admin/me/photo
pub async fn upload_photo(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(payload): Json<UploadAdminPhotoRequest>,
) -> Result<Json<Value>, AppError> {
    abac::authorize(&ctx, Action::AdminSelf)?;
    rate_limit::check_upload_limit(&state.redis, &ctx.user_id.to_string()).await?;
    let _upload_concurrency_guard =
        rate_limit::acquire_upload_concurrency(&state.redis, &ctx.user_id.to_string(), 2).await?;
    let raw = payload
        .base64_content
        .split(',')
        .next_back()
        .unwrap_or_default();
    let bytes = STANDARD
        .decode(raw)
        .map_err(|_| AppError::UnprocessableEntity("File content is not valid base64.".into()))?;
    ProfilePictureService::upload_admin_photo(&state, &ctx, payload.mime_type, bytes).await?;
    Ok(ok(json!({"updated": true})))
}

/// DELETE /api/v1/admin/me/photo
pub async fn delete_photo(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<Json<Value>, AppError> {
    abac::authorize(&ctx, Action::AdminSelf)?;
    ProfilePictureService::remove_admin_photo(&state, &ctx).await?;
    Ok(ok(json!({"removed": true})))
}

#[cfg(test)]
mod tests {
    use super::mask_phone;

    #[test]
    fn masks_all_but_the_last_three_digits() {
        assert_eq!(mask_phone("+254712345678"), "•••• 678");
        assert_eq!(mask_phone("12"), "•••• 12");
    }
}
