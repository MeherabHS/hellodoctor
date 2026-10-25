use crate::{
    abac::{self, Action},
    auth_context::AuthContext,
    domain::article_quizzes::{self, ImportResult, QuizContent},
    error::AppError,
    repository::{article_quiz_repo as repo, AppState},
};
use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Save {
    pub revision_id: Option<Uuid>,
    pub version: Option<i32>,
    pub content: QuizContent,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Import {
    pub format: String,
    pub content: String,
    pub revision_id: Option<Uuid>,
    pub version: Option<i32>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transition {
    pub version: i32,
}
fn response(data: Value) -> Json<Value> {
    Json(json!({"success":true,"data":data}))
}
fn validate(content: &mut QuizContent, complete: bool) -> Result<(), AppError> {
    let errors = content.validate(complete);
    if let Some(error) = errors.first() {
        return Err(repo::validation(&format!(
            "Question {} ({}): {}",
            error.row, error.field, error.message
        )));
    }
    Ok(())
}
pub async fn get(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    abac::authorize(&ctx, Action::ManageArticles)?;
    let mut tx = state.begin_authed(&ctx).await?;
    repo::parent(&mut tx, id, false).await?;
    let data = repo::document(&mut tx, id).await?;
    tx.commit().await?;
    Ok(response(data))
}
pub async fn save(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
    Json(mut p): Json<Save>,
) -> Result<Json<Value>, AppError> {
    abac::authorize(&ctx, Action::ManageArticles)?;
    validate(&mut p.content, false)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let (article_version, status) = repo::parent(&mut tx, id, true).await?;
    if status == "ARCHIVED" {
        return Err(repo::conflict());
    }
    repo::save(
        &mut tx,
        &ctx,
        id,
        article_version,
        (p.revision_id, p.version),
        &p.content,
        "ARTICLE_QUIZ_SAVE",
    )
    .await?;
    let data = repo::document(&mut tx, id).await?;
    tx.commit().await?;
    Ok(response(data))
}
fn import_check(p: &Import, old: &Value) -> ImportResult {
    let mut result = article_quizzes::parse_import(&p.format, &p.content);
    if result.errors.is_empty() {
        if let Ok(mut combined) = repo::content(old) {
            let previous = combined.questions.len();
            combined.questions.extend(result.questions.clone());
            let mut errors = combined.validate(false);
            for error in &mut errors {
                if error.row > previous {
                    error.row = error.row - previous + usize::from(p.format == "csv");
                }
            }
            result.errors.extend(errors);
        }
    }
    result
}
pub async fn import_preview(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
    Json(p): Json<Import>,
) -> Result<Json<Value>, AppError> {
    abac::authorize(&ctx, Action::ManageArticles)?;
    let mut tx = state.begin_authed(&ctx).await?;
    repo::parent(&mut tx, id, false).await?;
    let old = repo::current(&mut tx, id).await?;
    let result = import_check(&p, &old);
    tx.commit().await?;
    Ok(response(serde_json::to_value(result).map_err(|_| {
        AppError::internal("Import preview failed.".into())
    })?))
}
pub async fn import_confirm(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
    Json(p): Json<Import>,
) -> Result<Json<Value>, AppError> {
    abac::authorize(&ctx, Action::ManageArticles)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let (version, status) = repo::parent(&mut tx, id, true).await?;
    if status == "ARCHIVED" {
        return Err(repo::conflict());
    }
    let old = repo::current(&mut tx, id).await?;
    let result = import_check(&p, &old);
    if let Some(error) = result.errors.first() {
        return Err(repo::validation(&format!(
            "Import row {} ({}): {}. Nothing imported.",
            error.row, error.field, error.message
        )));
    }
    let mut content = repo::content(&old)?;
    content.questions.extend(result.questions);
    repo::save(
        &mut tx,
        &ctx,
        id,
        version,
        (p.revision_id, p.version),
        &content,
        "ARTICLE_QUIZ_IMPORT",
    )
    .await?;
    let data = repo::document(&mut tx, id).await?;
    tx.commit().await?;
    Ok(response(data))
}
pub async fn transition(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path((id, revision, action)): Path<(Uuid, Uuid, String)>,
    Json(p): Json<Transition>,
) -> Result<Json<Value>, AppError> {
    abac::authorize(&ctx, Action::ManageArticles)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let (article_version, article_status) = repo::parent(&mut tx, id, true).await?;
    if article_status == "ARCHIVED" {
        return Err(repo::conflict());
    }
    let old = repo::revision(&mut tx, id, revision).await?;
    if old["version"] != p.version || old["article_content_version"] != article_version {
        return Err(repo::conflict());
    }
    let next = match (old["status"].as_str().unwrap_or(""), action.as_str()) {
        ("DRAFT", "submit") => "PENDING_REVIEW",
        ("PENDING_REVIEW", "approve") => "APPROVED",
        ("PENDING_REVIEW" | "APPROVED", "return") => "DRAFT",
        ("APPROVED", "publish") => "PUBLISHED",
        _ => return Err(repo::conflict()),
    };
    if next == "APPROVED" && old["can_approve"] != true {
        return Err(AppError::Forbidden(
            "Approval required from another Admin.".into(),
        ));
    }
    if next == "PUBLISHED" && article_status != "PUBLISHED" {
        return Err(repo::validation("Publish the approved article first, then explicitly publish its approved quiz revision."));
    }
    let mut content = repo::content(&old)?;
    if next != "DRAFT" {
        let required = content.include_quiz;
        validate(&mut content, required)?;
    }
    sqlx::query("UPDATE article_quiz_revisions SET status=$1,version=version+1,approved_version=CASE WHEN $1='APPROVED' THEN content_version WHEN $1='DRAFT' THEN NULL ELSE approved_version END,approved_by=CASE WHEN $1='APPROVED' THEN $2 WHEN $1='DRAFT' THEN NULL ELSE approved_by END,approved_at=CASE WHEN $1='APPROVED' THEN now() WHEN $1='DRAFT' THEN NULL ELSE approved_at END,published_at=CASE WHEN $1='PUBLISHED' THEN now() ELSE NULL END,updated_at=now() WHERE id=$3 AND article_id=$4 AND version=$5").bind(next).bind(ctx.user_id).bind(revision).bind(id).bind(p.version).execute(&mut *tx).await?;
    if next == "PUBLISHED" {
        sqlx::query("INSERT INTO article_quiz_publications(article_id,revision_id,article_content_version,included,question_count) VALUES($1,$2,$3,$4,$5) ON CONFLICT(article_id) DO UPDATE SET revision_id=EXCLUDED.revision_id,article_content_version=EXCLUDED.article_content_version,included=EXCLUDED.included,question_count=EXCLUDED.question_count").bind(id).bind(revision).bind(article_version).bind(content.include_quiz).bind(if content.include_quiz {content.questions.len() as i32}else{0}).execute(&mut *tx).await?;
    }
    let updated = repo::revision(&mut tx, id, revision).await?;
    repo::audit(
        &mut tx,
        &ctx,
        id,
        &updated,
        &format!("ARTICLE_QUIZ_{}", action.to_uppercase()),
    )
    .await?;
    let data = repo::document(&mut tx, id).await?;
    tx.commit().await?;
    Ok(response(data))
}
