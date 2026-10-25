use crate::{
    abac::{self, Action},
    auth_context::AuthContext,
    error::AppError,
    rate_limit,
    repository::AppState,
};
use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, HeaderValue},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Check {
    pub revision_id: Uuid,
    pub question_id: Uuid,
    pub selected_option_id: Uuid,
}

fn response(data: Value) -> Result<(HeaderMap, Json<Value>), AppError> {
    match data["state"].as_str() {
        Some("UNAVAILABLE") => {
            return Err(AppError::NotFound(
                "This quiz is no longer available.".into(),
            ))
        }
        Some("SUPERSEDED") => {
            return Err(AppError::Conflict(
                "The article or quiz was updated. Refresh the article to continue.".into(),
            ))
        }
        Some("INVALID_CHOICE") => {
            return Err(AppError::Validation(
                "Question or option does not belong to the current quiz.".into(),
            ))
        }
        _ => {}
    }
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    Ok((headers, Json(json!({"success":true,"data":data}))))
}
pub async fn read(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(article): Path<Uuid>,
) -> Result<(HeaderMap, Json<Value>), AppError> {
    abac::authorize(&ctx, Action::PatientRead)?;
    rate_limit::check_and_increment(
        &state.redis,
        &format!("ratelimit:quiz-read:{}", ctx.user_id),
        60,
        60,
    )
    .await?;
    let mut tx = state.begin_authed(&ctx).await?;
    let value: Value = sqlx::query_scalar("SELECT read_patient_article_quiz($1)")
        .bind(article)
        .fetch_one(&mut *tx)
        .await?;
    let result = response(value)?;
    tx.commit().await?;
    Ok(result)
}
pub async fn check(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(article): Path<Uuid>,
    Json(p): Json<Check>,
) -> Result<(HeaderMap, Json<Value>), AppError> {
    abac::authorize(&ctx, Action::PatientRead)?;
    rate_limit::check_and_increment(
        &state.redis,
        &format!("ratelimit:quiz-check:{}", ctx.user_id),
        120,
        60,
    )
    .await?;
    let mut tx = state.begin_authed(&ctx).await?;
    // Pure evaluation: no audit, telemetry payload, attempt record or notification.
    let value: Value = sqlx::query_scalar("SELECT check_patient_article_quiz($1,$2,$3,$4)")
        .bind(article)
        .bind(p.revision_id)
        .bind(p.question_id)
        .bind(p.selected_option_id)
        .fetch_one(&mut *tx)
        .await?;
    let result = response(value)?;
    tx.commit().await?;
    Ok(result)
}
