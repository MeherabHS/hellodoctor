use crate::{
    abac::{self, Action},
    auth_context::AuthContext,
    domain::{
        articles::{ArticleStatus, DoctorArticle},
        ids::DoctorProfileId,
    },
    error::AppError,
    repository::{article_repo, AppState},
    services::audit_service::{AuditLogEntry, AuditService},
};
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::Row;
use uuid::Uuid;

#[derive(Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArticleContent {
    pub title: String,
    pub summary: String,
    pub body: String,
    #[serde(default)]
    pub references_list: Vec<String>,
}
impl ArticleContent {
    fn validate(&mut self) -> Result<(), AppError> {
        for (name, text, maximum) in [
            ("Title", &mut self.title, 160),
            ("Summary", &mut self.summary, 600),
            ("Article", &mut self.body, 30000),
        ] {
            *text = text.trim().to_owned();
            if text.is_empty() || text.chars().count() > maximum || text.contains('\0') {
                return Err(AppError::Validation(format!(
                    "{name} must contain 1-{maximum} characters."
                )));
            }
        }
        if self.references_list.len() > 10
            || self
                .references_list
                .iter()
                .any(|r| r.trim().is_empty() || r.chars().count() > 500 || r.contains('\0'))
        {
            return Err(AppError::Validation(
                "Include at most 10 references of 1-500 characters.".into(),
            ));
        }
        Ok(())
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateArticle {
    pub creation_key: String,
    pub content: ArticleContent,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateArticle {
    pub version: i32,
    pub content: ArticleContent,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArticleVersion {
    pub version: i32,
}
#[derive(Deserialize, Default)]
pub struct ArticleQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub async fn list_own(
    State(state): State<AppState>,
    ctx: AuthContext,
    Query(query): Query<ArticleQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    abac::authorize(&ctx, Action::DoctorArticles)?;
    let author = ctx.verified_doctor_profile_id(&state).await?;
    list(&state, &ctx, Some(author), query).await
}
pub async fn feed(
    State(state): State<AppState>,
    ctx: AuthContext,
    Query(query): Query<ArticleQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    abac::authorize(&ctx, Action::AnyAuthenticated)?;
    list(&state, &ctx, None, query).await
}
async fn list(
    state: &AppState,
    ctx: &AuthContext,
    author: Option<DoctorProfileId>,
    query: ArticleQuery,
) -> Result<Json<serde_json::Value>, AppError> {
    let mut tx = state.begin_authed(ctx).await?;
    let rows = article_repo::list(
        &mut *tx,
        author,
        query.limit.unwrap_or(20).clamp(1, 50),
        query.offset.unwrap_or(0).max(0),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(json!({"success":true,"data":rows})))
}
pub async fn detail(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    abac::authorize(&ctx, Action::AnyAuthenticated)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let article = article_repo::find(&mut *tx, id, None)
        .await?
        .ok_or_else(|| AppError::NotFound("Published article not found.".into()))?;
    tx.commit().await?;
    Ok(Json(json!({"success":true,"data":article})))
}
pub async fn own_detail(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    abac::authorize(&ctx, Action::DoctorArticles)?;
    let author = ctx.verified_doctor_profile_id(&state).await?;
    let mut tx = state.begin_authed(&ctx).await?;
    let article = article_repo::find(&mut *tx, id, Some(author))
        .await?
        .ok_or_else(|| AppError::NotFound("Article not found.".into()))?;
    tx.commit().await?;
    Ok(Json(json!({"success":true,"data":article})))
}
pub async fn create(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(mut payload): Json<CreateArticle>,
) -> Result<Json<serde_json::Value>, AppError> {
    abac::authorize(&ctx, Action::DoctorArticles)?;
    let author = ctx.verified_doctor_profile_id(&state).await?;
    payload.content.validate()?;
    if !(16..=64).contains(&payload.creation_key.len())
        || !payload
            .creation_key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err(AppError::Validation(
            "A valid draft creation key is required.".into(),
        ));
    }
    let hash = format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&payload.content).map_err(|e| AppError::internal(e.to_string()))?
        )
    );
    let mut tx = state.begin_authed(&ctx).await?;
    let id = Uuid::new_v4();
    let inserted=sqlx::query("INSERT INTO doctor_articles(id,author_id,title,summary,body,references_list,creation_key,creation_hash) VALUES($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT(author_id,creation_key) DO NOTHING")
        .bind(id).bind(author).bind(&payload.content.title).bind(&payload.content.summary).bind(&payload.content.body).bind(json!(payload.content.references_list)).bind(&payload.creation_key).bind(&hash).execute(&mut *tx).await?.rows_affected()==1;
    let id = if inserted {
        id
    } else {
        let row = sqlx::query(
            "SELECT id,creation_hash FROM doctor_articles WHERE author_id=$1 AND creation_key=$2",
        )
        .bind(author)
        .bind(&payload.creation_key)
        .fetch_one(&mut *tx)
        .await?;
        if row.get::<String, _>("creation_hash") != hash {
            return Err(AppError::Conflict(
                "Draft creation key was already used for different content.".into(),
            ));
        }
        row.get("id")
    };
    let article = article_repo::find(&mut *tx, id, Some(author))
        .await?
        .ok_or_else(|| AppError::NotFound("Article not found.".into()))?;
    if inserted {
        log(&mut tx, &ctx, "ARTICLE_CREATE", &article).await?;
    }
    tx.commit().await?;
    Ok(Json(json!({"success":true,"data":article})))
}
pub async fn update(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
    Json(mut payload): Json<UpdateArticle>,
) -> Result<Json<serde_json::Value>, AppError> {
    abac::authorize(&ctx, Action::DoctorArticles)?;
    let author = ctx.verified_doctor_profile_id(&state).await?;
    payload.content.validate()?;
    let mut tx = state.begin_authed(&ctx).await?;
    let changed=sqlx::query("UPDATE doctor_articles SET title=$1,summary=$2,body=$3,references_list=$4,version=version+1,updated_at=NOW() WHERE id=$5 AND author_id=$6 AND version=$7 AND status<>'ARCHIVED'")
        .bind(payload.content.title).bind(payload.content.summary).bind(payload.content.body).bind(json!(payload.content.references_list)).bind(id).bind(author).bind(payload.version).execute(&mut *tx).await?.rows_affected()==1;
    finish_mutation(tx, &ctx, id, author, changed, "ARTICLE_EDIT").await
}
pub async fn publish(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<ArticleVersion>,
) -> Result<Json<serde_json::Value>, AppError> {
    transition(&state, &ctx, id, payload.version, ArticleStatus::Published).await
}
pub async fn archive(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<ArticleVersion>,
) -> Result<Json<serde_json::Value>, AppError> {
    transition(&state, &ctx, id, payload.version, ArticleStatus::Archived).await
}
async fn transition(
    state: &AppState,
    ctx: &AuthContext,
    id: Uuid,
    version: i32,
    status: ArticleStatus,
) -> Result<Json<serde_json::Value>, AppError> {
    abac::authorize(ctx, Action::DoctorArticles)?;
    let author = ctx.verified_doctor_profile_id(state).await?;
    let mut tx = state.begin_authed(ctx).await?;
    let changed=sqlx::query("UPDATE doctor_articles SET status=$1,version=version+1,updated_at=NOW(),published_at=CASE WHEN $1='PUBLISHED' THEN COALESCE(published_at,NOW()) ELSE published_at END WHERE id=$2 AND author_id=$3 AND version=$4 AND status<>'ARCHIVED' AND status<>$1")
        .bind(status).bind(id).bind(author).bind(version).execute(&mut *tx).await?.rows_affected()==1;
    finish_mutation(
        tx,
        ctx,
        id,
        author,
        changed,
        if status == ArticleStatus::Published {
            "ARTICLE_PUBLISH"
        } else {
            "ARTICLE_ARCHIVE"
        },
    )
    .await
}
async fn finish_mutation(
    mut tx: sqlx::Transaction<'_, sqlx::Postgres>,
    ctx: &AuthContext,
    id: Uuid,
    author: DoctorProfileId,
    changed: bool,
    action: &str,
) -> Result<Json<serde_json::Value>, AppError> {
    let article = article_repo::find(&mut *tx, id, Some(author))
        .await?
        .ok_or_else(|| AppError::NotFound("Article not found.".into()))?;
    if !changed {
        return Err(AppError::Conflict(
            "Article changed or this transition is unavailable. Refresh before retrying.".into(),
        ));
    }
    log(&mut tx, ctx, action, &article).await?;
    if action == "ARTICLE_PUBLISH" || action == "ARTICLE_ARCHIVE" {
        crate::services::notification_service::NotificationService::notify_article_status(
            &mut *tx,
            ctx.user_id,
            id,
            article.version,
            article.status == ArticleStatus::Published,
        )
        .await?;
    }
    tx.commit().await?;
    Ok(Json(json!({"success":true,"data":article})))
}
async fn log(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ctx: &AuthContext,
    action: &str,
    article: &DoctorArticle,
) -> Result<(), AppError> {
    AuditService::log_in_tx(
        &mut **tx,
        AuditLogEntry::new(
            Some(ctx.user_id),
            ctx.role,
            action,
            "DOCTOR_ARTICLE",
            Some(article.id),
            "SUCCESS",
        )
        .with_after(json!({"status":article.status,"version":article.version})),
    )
    .await?;
    Ok(())
}
