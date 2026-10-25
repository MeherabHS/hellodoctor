use crate::{
    abac::{self, Action},
    auth_context::AuthContext,
    domain::admin_articles::{ArticleContent, CategoryContent},
    error::AppError,
    repository::{admin_article_repo, AppState},
    services::audit_service::{AuditLogEntry, AuditService},
};
use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::Row;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Create {
    pub id: Uuid,
    pub content: ArticleContent,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Edit {
    pub version: i32,
    pub content: ArticleContent,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transition {
    pub version: i32,
    pub scheduled_for: Option<DateTime<Utc>>,
}
#[derive(Default, Deserialize)]
pub struct ListQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub status: Option<String>,
    pub category_id: Option<Uuid>,
    pub search: Option<String>,
}
fn response(data: Value) -> Json<Value> {
    Json(json!({"success":true,"data":data}))
}
fn conflict() -> AppError {
    AppError::Conflict("Article changed or transition unavailable. Refresh before retrying.".into())
}
fn db_error(error: sqlx::Error) -> AppError {
    match error.as_database_error().and_then(|e| e.code()).as_deref() {
        Some("23505") => AppError::Conflict("This ID or category name already exists.".into()),
        Some("23503" | "23514") => {
            AppError::Validation("Invalid category, cover, approval or article transition.".into())
        }
        _ => error.into(),
    }
}
pub async fn audit(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ctx: &AuthContext,
    id: Uuid,
    action: &str,
    data: &Value,
) -> Result<(), AppError> {
    AuditService::log_in_tx(&mut **tx,AuditLogEntry::new(Some(ctx.user_id),ctx.role,action,"ADMIN_ARTICLE",Some(id),"SUCCESS").with_after(json!({"status":data["status"],"version":data["version"],"content_version":data["content_version"]}))).await?;
    Ok(())
}
pub async fn list(
    State(state): State<AppState>,
    ctx: AuthContext,
    Query(q): Query<ListQuery>,
) -> Result<Json<Value>, AppError> {
    abac::authorize(&ctx, Action::ManageArticles)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let limit = q.limit.unwrap_or(20).clamp(1, 50);
    let offset = q.offset.unwrap_or(0).max(0);
    if q.search.as_ref().is_some_and(|s| s.chars().count() > 160) {
        return Err(AppError::Validation(
            "Search is limited to 160 characters.".into(),
        ));
    }
    let filter=" WHERE ($1::text IS NULL OR a.status=$1) AND ($2::uuid IS NULL OR a.category_id=$2) AND ($3::text IS NULL OR strpos(lower(a.title_en||' '||a.title_sw),lower($3))>0)";
    let rows: Vec<Value> = sqlx::query_scalar(&format!(
        "{}{filter} ORDER BY a.created_at DESC,a.id LIMIT $4 OFFSET $5",
        admin_article_repo::ADMIN_SELECT
    ))
    .bind(&q.status)
    .bind(q.category_id)
    .bind(&q.search)
    .bind(limit)
    .bind(offset)
    .fetch_all(&mut *tx)
    .await?;
    let total: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM admin_articles a {filter}"))
        .bind(&q.status)
        .bind(q.category_id)
        .bind(&q.search)
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(response(
        json!({"items":rows,"total":total,"limit":limit,"offset":offset}),
    ))
}
pub async fn detail(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    abac::authorize(&ctx, Action::ManageArticles)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let data = admin_article_repo::find(&mut *tx, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Article not found.".into()))?;
    tx.commit().await?;
    Ok(response(data))
}
pub async fn create(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(mut p): Json<Create>,
) -> Result<Json<Value>, AppError> {
    abac::authorize(&ctx, Action::ManageArticles)?;
    p.content.validate()?;
    let mut tx = state.begin_authed(&ctx).await?;
    let c = &p.content;
    // Client-generated UUID makes retry discoverable without duplicating a draft.
    let inserted=sqlx::query("INSERT INTO admin_articles(id,author_id,title_en,title_sw,body_en,body_sw,category_id,tags,featured,sensitive) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) ON CONFLICT(id) DO NOTHING")
        .bind(p.id).bind(ctx.user_id).bind(&c.title_en).bind(&c.title_sw).bind(&c.body_en).bind(&c.body_sw).bind(c.category_id).bind(&c.tags).bind(c.featured).bind(c.sensitive).execute(&mut *tx).await.map_err(db_error)?.rows_affected()==1;
    let data = admin_article_repo::find(&mut *tx, p.id)
        .await?
        .ok_or_else(conflict)?;
    if !inserted {
        return Err(AppError::Conflict(
            "Draft ID already exists. Reopen the existing draft before retrying.".into(),
        ));
    }
    audit(&mut tx, &ctx, p.id, "ADMIN_ARTICLE_CREATE", &data).await?;
    tx.commit().await?;
    Ok(response(data))
}
pub async fn edit(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
    Json(mut p): Json<Edit>,
) -> Result<Json<Value>, AppError> {
    abac::authorize(&ctx, Action::ManageArticles)?;
    p.content.validate()?;
    let c = &p.content;
    let mut tx = state.begin_authed(&ctx).await?;
    let old = sqlx::query(
        "SELECT * FROM admin_articles WHERE id=$1 AND version=$2 AND status<>'ARCHIVED' FOR UPDATE",
    )
    .bind(id)
    .bind(p.version)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(conflict)?;
    let changed = old.get::<String, _>("title_en") != c.title_en
        || old.get::<String, _>("title_sw") != c.title_sw
        || old.get::<String, _>("body_en") != c.body_en
        || old.get::<String, _>("body_sw") != c.body_sw
        || old.get::<Uuid, _>("category_id") != c.category_id
        || old.get::<Vec<String>, _>("tags") != c.tags
        || old.get::<bool, _>("featured") != c.featured
        || old.get::<bool, _>("sensitive") != c.sensitive;
    if changed {
        sqlx::query("UPDATE admin_articles SET title_en=$1,title_sw=$2,body_en=$3,body_sw=$4,category_id=$5,tags=$6,featured=$7,sensitive=$8,status='DRAFT',version=version+1,content_version=content_version+1,approved_version=NULL,approved_by=NULL,approved_at=NULL,scheduled_for=NULL,updated_at=now() WHERE id=$9")
            .bind(&c.title_en).bind(&c.title_sw).bind(&c.body_en).bind(&c.body_sw).bind(c.category_id).bind(&c.tags).bind(c.featured).bind(c.sensitive).bind(id).execute(&mut *tx).await.map_err(db_error)?;
    }
    let data = admin_article_repo::find(&mut *tx, id)
        .await?
        .ok_or_else(conflict)?;
    if changed {
        audit(&mut tx, &ctx, id, "ADMIN_ARTICLE_EDIT", &data).await?;
    }
    tx.commit().await?;
    Ok(response(data))
}

pub async fn transition(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path((id, action)): Path<(Uuid, String)>,
    Json(p): Json<Transition>,
) -> Result<Json<Value>, AppError> {
    abac::authorize(&ctx, Action::ManageArticles)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let old = admin_article_repo::find(&mut *tx, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Article not found.".into()))?;
    if old["version"].as_i64() != Some(i64::from(p.version)) {
        return Err(conflict());
    }
    let status = old["status"].as_str().unwrap_or("");
    let next = match action.as_str() {
        "submit" if status == "DRAFT" => "PENDING_REVIEW",
        "approve" if status == "PENDING_REVIEW" => "APPROVED",
        "return" if status == "PENDING_REVIEW" => "DRAFT",
        "publish" if status == "APPROVED" => "PUBLISHED",
        "schedule" if status == "APPROVED" => "SCHEDULED",
        "unpublish" if matches!(status, "PUBLISHED" | "SCHEDULED" | "APPROVED") => "DRAFT",
        "archive" if status != "ARCHIVED" => "ARCHIVED",
        _ => return Err(conflict()),
    };
    if next == "APPROVED"
        && old["requires_independent_review"] == true
        && old["author_id"] == ctx.user_id.to_string()
    {
        return Err(AppError::Forbidden("Sensitive health articles require approval by another Admin. This article remains pending review.".into()));
    }
    if next == "SCHEDULED" && p.scheduled_for.is_none_or(|date| date <= Utc::now()) {
        return Err(AppError::Validation(
            "Schedule must be a future UTC timestamp.".into(),
        ));
    }
    if next != "SCHEDULED" && p.scheduled_for.is_some() {
        return Err(AppError::Validation(
            "A schedule is only accepted by the schedule action.".into(),
        ));
    }
    let changed=sqlx::query("UPDATE admin_articles SET status=$1,version=version+1,approved_version=CASE WHEN $1='APPROVED' THEN content_version WHEN $1='DRAFT' THEN NULL ELSE approved_version END,approved_by=CASE WHEN $1='APPROVED' THEN $2 WHEN $1='DRAFT' THEN NULL ELSE approved_by END,approved_at=CASE WHEN $1='APPROVED' THEN now() WHEN $1='DRAFT' THEN NULL ELSE approved_at END,scheduled_for=CASE WHEN $1='SCHEDULED' THEN $3 ELSE NULL END,published_at=CASE WHEN $1='PUBLISHED' THEN now() ELSE published_at END,archived_at=CASE WHEN $1='ARCHIVED' THEN now() ELSE NULL END,updated_at=now() WHERE id=$4 AND version=$5")
        .bind(next).bind(ctx.user_id).bind(p.scheduled_for).bind(id).bind(p.version).execute(&mut *tx).await.map_err(db_error)?.rows_affected()==1;
    if !changed {
        return Err(conflict());
    }
    let data = admin_article_repo::find(&mut *tx, id)
        .await?
        .ok_or_else(conflict)?;
    audit(
        &mut tx,
        &ctx,
        id,
        &format!("ADMIN_ARTICLE_{}", action.to_uppercase()),
        &data,
    )
    .await?;
    tx.commit().await?;
    Ok(response(data))
}

pub async fn categories(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<Json<Value>, AppError> {
    abac::authorize(&ctx, Action::ManageArticles)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let rows: Vec<Value> = sqlx::query_scalar(
        "SELECT to_jsonb(c) FROM article_categories c ORDER BY lower(name_en),id",
    )
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(response(json!(rows)))
}
pub async fn create_category(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(mut c): Json<CategoryContent>,
) -> Result<Json<Value>, AppError> {
    abac::authorize(&ctx, Action::ManageArticles)?;
    c.validate()?;
    let mut tx = state.begin_authed(&ctx).await?;
    let id = Uuid::new_v4();
    let data:Value=sqlx::query_scalar("INSERT INTO article_categories(id,name_en,name_sw,sensitive) VALUES($1,$2,$3,$4) RETURNING to_jsonb(article_categories)").bind(id).bind(&c.name_en).bind(&c.name_sw).bind(c.sensitive).fetch_one(&mut *tx).await.map_err(db_error)?;
    audit(&mut tx, &ctx, id, "ARTICLE_CATEGORY_CREATE", &Value::Null).await?;
    tx.commit().await?;
    Ok(response(data))
}
pub async fn edit_category(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
    Json(mut c): Json<CategoryContent>,
) -> Result<Json<Value>, AppError> {
    abac::authorize(&ctx, Action::ManageArticles)?;
    c.validate()?;
    let mut tx = state.begin_authed(&ctx).await?;
    let data:Value=sqlx::query_scalar("UPDATE article_categories SET name_en=$1,name_sw=$2 WHERE id=$3 AND sensitive=$4 RETURNING to_jsonb(article_categories)").bind(c.name_en).bind(c.name_sw).bind(id).bind(c.sensitive).fetch_optional(&mut *tx).await.map_err(db_error)?.ok_or_else(||AppError::Validation("Category not found or review requirement changed. Create a separate category to change its review requirement.".into()))?;
    audit(&mut tx, &ctx, id, "ARTICLE_CATEGORY_EDIT", &Value::Null).await?;
    tx.commit().await?;
    Ok(response(data))
}
pub async fn delete_category(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    abac::authorize(&ctx, Action::ManageArticles)?;
    let mut tx = state.begin_authed(&ctx).await?;
    if sqlx::query("DELETE FROM article_categories WHERE id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?
        .rows_affected()
        != 1
    {
        return Err(AppError::NotFound("Category not found.".into()));
    }
    audit(&mut tx, &ctx, id, "ARTICLE_CATEGORY_DELETE", &Value::Null).await?;
    tx.commit().await?;
    Ok(response(json!({"id":id})))
}
