use crate::{
    auth_context::AuthContext,
    domain::article_quizzes::QuizContent,
    error::AppError,
    services::audit_service::{AuditLogEntry, AuditService},
};
use serde_json::{json, Value};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

const REVISION: &str = "SELECT to_jsonb(r)||jsonb_build_object('schema_version',1,'requires_independent_review',a.sensitive OR c.sensitive,'can_approve',r.status='PENDING_REVIEW' AND (NOT(a.sensitive OR c.sensitive) OR NULLIF(current_setting('app.current_user_id',true),'')::uuid NOT IN(r.created_by,r.edited_by)),'questions',COALESCE((SELECT jsonb_agg((to_jsonb(q)-'revision_id'-'position')||jsonb_build_object('options',COALESCE((SELECT jsonb_agg(to_jsonb(o)-'revision_id'-'question_id'-'position' ORDER BY o.position) FROM article_quiz_options o WHERE o.revision_id=r.id AND o.question_id=q.id),'[]'::jsonb)) ORDER BY q.position) FROM article_quiz_questions q WHERE q.revision_id=r.id),'[]'::jsonb)) FROM article_quiz_revisions r JOIN admin_articles a ON a.id=r.article_id JOIN article_categories c ON c.id=a.category_id";
pub fn conflict() -> AppError {
    AppError::Conflict(
        "Quiz or article changed. Reload the saved questions before retrying.".into(),
    )
}
pub fn validation(message: &str) -> AppError {
    AppError::Validation(message.into())
}
pub async fn parent(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    lock: bool,
) -> Result<(i32, String), AppError> {
    let query = if lock {
        "SELECT content_version,status FROM admin_articles WHERE id=$1 FOR UPDATE"
    } else {
        "SELECT content_version,status FROM admin_articles WHERE id=$1"
    };
    sqlx::query_as(query)
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| AppError::NotFound("Admin article not found.".into()))
}
pub async fn revision(
    tx: &mut Transaction<'_, Postgres>,
    article: Uuid,
    id: Uuid,
) -> Result<Value, AppError> {
    sqlx::query_scalar(&format!("{REVISION} WHERE r.article_id=$1 AND r.id=$2"))
        .bind(article)
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| AppError::NotFound("Quiz revision not found for this article.".into()))
}
pub async fn document(
    tx: &mut Transaction<'_, Postgres>,
    article: Uuid,
) -> Result<Value, AppError> {
    let draft: Option<Value> = sqlx::query_scalar(&format!(
        "{REVISION} WHERE r.article_id=$1 AND r.status<>'PUBLISHED'"
    ))
    .bind(article)
    .fetch_optional(&mut **tx)
    .await?;
    let published:Option<Value>=sqlx::query_scalar(&format!("{REVISION} WHERE r.article_id=$1 AND r.id=(SELECT revision_id FROM article_quiz_publications WHERE article_id=$1)")).bind(article).fetch_optional(&mut **tx).await?;
    Ok(json!({"schema_version":1,"draft":draft,"published":published}))
}
pub fn content(revision: &Value) -> Result<QuizContent, AppError> {
    if revision.is_null() {
        return Ok(QuizContent::default());
    }
    serde_json::from_value(
        json!({"include_quiz":revision["include_quiz"],"questions":revision["questions"]}),
    )
    .map_err(|_| AppError::internal("Invalid stored quiz contract.".into()))
}
pub async fn current(tx: &mut Transaction<'_, Postgres>, article: Uuid) -> Result<Value, AppError> {
    let doc = document(tx, article).await?;
    Ok(if !doc["draft"].is_null() {
        doc["draft"].clone()
    } else {
        doc["published"].clone()
    })
}
pub async fn audit(
    tx: &mut Transaction<'_, Postgres>,
    ctx: &AuthContext,
    article: Uuid,
    data: &Value,
    action: &str,
) -> Result<(), AppError> {
    AuditService::log_in_tx(&mut **tx,AuditLogEntry::new(Some(ctx.user_id),ctx.role,action,"ARTICLE_QUIZ",data["id"].as_str().and_then(|v|Uuid::parse_str(v).ok()),"SUCCESS").with_after(json!({"article_id":article,"revision_id":data["id"],"status":data["status"],"version":data["version"],"content_version":data["content_version"],"include_quiz":data["include_quiz"],"question_count":data["questions"].as_array().map(Vec::len).unwrap_or(0)}))).await?;
    Ok(())
}
pub async fn save(
    tx: &mut Transaction<'_, Postgres>,
    ctx: &AuthContext,
    article: Uuid,
    article_version: i32,
    expected: (Option<Uuid>, Option<i32>),
    content: &QuizContent,
    action: &str,
) -> Result<Value, AppError> {
    let old = current(tx, article).await?;
    let old_id = old["id"].as_str().and_then(|s| Uuid::parse_str(s).ok());
    let old_version = old["version"].as_i64().and_then(|v| i32::try_from(v).ok());
    if expected != (old_id, old_version) {
        return Err(conflict());
    }
    let id = if old.is_null() || old["status"] == "PUBLISHED" {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO article_quiz_revisions(id,article_id,created_by,edited_by,include_quiz,article_content_version) VALUES($1,$2,$3,$3,$4,$5)").bind(id).bind(article).bind(ctx.user_id).bind(content.include_quiz).bind(article_version).execute(&mut **tx).await?;
        id
    } else {
        let id = old_id.ok_or_else(conflict)?;
        sqlx::query("UPDATE article_quiz_revisions SET status='DRAFT',version=version+1,content_version=content_version+1,article_content_version=$1,edited_by=$2,include_quiz=$3,approved_version=NULL,approved_by=NULL,approved_at=NULL,updated_at=now() WHERE id=$4").bind(article_version).bind(ctx.user_id).bind(content.include_quiz).bind(id).execute(&mut **tx).await?;
        sqlx::query("DELETE FROM article_quiz_options WHERE revision_id=$1")
            .bind(id)
            .execute(&mut **tx)
            .await?;
        sqlx::query("DELETE FROM article_quiz_questions WHERE revision_id=$1")
            .bind(id)
            .execute(&mut **tx)
            .await?;
        id
    };
    for (position, q) in content.questions.iter().enumerate() {
        sqlx::query("INSERT INTO article_quiz_questions(revision_id,id,position,text_en,text_sw,explanation_en,explanation_sw,correct_option_id) VALUES($1,$2,$3,$4,$5,$6,$7,$8)").bind(id).bind(q.id).bind(position as i32).bind(&q.text_en).bind(&q.text_sw).bind(&q.explanation_en).bind(&q.explanation_sw).bind(q.correct_option_id).execute(&mut **tx).await?;
        for (position, o) in q.options.iter().enumerate() {
            sqlx::query("INSERT INTO article_quiz_options(revision_id,question_id,id,position,text_en,text_sw) VALUES($1,$2,$3,$4,$5,$6)").bind(id).bind(q.id).bind(o.id).bind(position as i32).bind(&o.text_en).bind(&o.text_sw).execute(&mut **tx).await?;
        }
    }
    let data = revision(tx, article, id).await?;
    audit(tx, ctx, article, &data, action).await?;
    Ok(data)
}
