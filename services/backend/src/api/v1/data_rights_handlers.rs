use crate::abac::{self, Action};
use crate::auth_context::AuthContext;
use crate::error::AppError;
use crate::rate_limit;
use crate::repository::AppState;
use axum::{
    extract::{Path, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, sqlx::FromRow)]
pub struct DataRequest {
    id: Uuid,
    user_id: Uuid,
    request_type: String,
    details: String,
    status: String,
    response: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateRequest {
    request_type: String,
    details: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewRequest {
    status: String,
    response: String,
}

pub async fn create(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(body): Json<CreateRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    abac::authorize(&ctx, Action::PatientRead)?;
    if ![
        "ACCESS",
        "CORRECTION",
        "DELETION",
        "WITHDRAWAL",
        "RESTRICTION",
        "PORTABILITY",
        "COMPLAINT",
    ]
    .contains(&body.request_type.as_str())
        || body.details.trim().is_empty()
        || body.details.chars().count() > 2000
    {
        return Err(AppError::Validation(
            "Choose a valid request type and enter 1 to 2000 characters.".into(),
        ));
    }
    rate_limit::check_and_increment(
        &state.redis,
        &format!("ratelimit:data-rights:{}", ctx.user_id),
        10,
        3600,
    )
    .await?;
    let mut tx = state.begin_authed(&ctx).await?;
    let row = sqlx::query_as::<_, DataRequest>("INSERT INTO patient_data_requests(user_id,request_type,details) VALUES($1,$2,$3) RETURNING *")
        .bind(ctx.user_id).bind(body.request_type).bind(body.details.trim()).fetch_one(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(serde_json::json!({"success":true,"data":row})))
}
pub async fn list_own(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<Json<serde_json::Value>, AppError> {
    abac::authorize(&ctx, Action::PatientRead)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let rows = sqlx::query_as::<_, DataRequest>(
        "SELECT * FROM patient_data_requests WHERE user_id=$1 ORDER BY created_at DESC LIMIT 100",
    )
    .bind(ctx.user_id)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(serde_json::json!({"success":true,"data":rows})))
}
fn authorize_review(ctx: &AuthContext) -> Result<(), AppError> {
    abac::authorize(ctx, Action::ViewLegalAcceptances)?;
    if !["ADMIN"].contains(&ctx.role.as_str()) {
        return Err(AppError::Forbidden(
            "Privacy review requires Compliance or Platform Admin.".into(),
        ));
    }
    Ok(())
}
pub async fn list_for_review(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<Json<serde_json::Value>, AppError> {
    authorize_review(&ctx)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let rows=sqlx::query_as::<_,DataRequest>("SELECT * FROM patient_data_requests WHERE status IN ('SUBMITTED','IN_REVIEW') ORDER BY created_at LIMIT 100").fetch_all(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(serde_json::json!({"success":true,"data":rows})))
}
pub async fn review(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
    Json(body): Json<ReviewRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    authorize_review(&ctx)?;
    if !["IN_REVIEW", "COMPLETED", "DECLINED"].contains(&body.status.as_str())
        || body.response.trim().is_empty()
        || body.response.chars().count() > 4000
    {
        return Err(AppError::Validation(
            "A valid status and explanatory response (1 to 4000 characters) are required.".into(),
        ));
    }
    let mut tx = state.begin_authed(&ctx).await?;
    let row=sqlx::query_as::<_,DataRequest>("UPDATE patient_data_requests SET status=$2,response=$3,updated_at=NOW() WHERE id=$1 AND status IN ('SUBMITTED','IN_REVIEW') RETURNING *")
        .bind(id).bind(body.status).bind(body.response.trim()).fetch_optional(&mut *tx).await?.ok_or_else(|| AppError::Conflict("Request is unavailable or already closed.".into()))?;
    tx.commit().await?;
    Ok(Json(serde_json::json!({"success":true,"data":row})))
}
