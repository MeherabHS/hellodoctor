use serde_json::Value;
use sqlx::PgExecutor;
use uuid::Uuid;

// Do not serialize media storage keys, hashes or private user identifiers into the Patient projection.
pub const ADMIN_SELECT: &str = "SELECT to_jsonb(a) || jsonb_build_object('author_name','AfyaMkononi Editorial','category_name_en',c.name_en,'category_name_sw',c.name_sw,'requires_independent_review',a.sensitive OR c.sensitive,'can_approve',a.status='PENDING_REVIEW' AND current_setting('app.current_role',true)='ADMIN' AND (NOT(a.sensitive OR c.sensitive) OR a.author_id<>NULLIF(current_setting('app.current_user_id',true),'')::uuid),'approval_block_reason',CASE WHEN a.status='PENDING_REVIEW' AND (a.sensitive OR c.sensitive) AND a.author_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid THEN 'INDEPENDENT_REVIEW_REQUIRED' END) FROM admin_articles a JOIN article_categories c ON c.id=a.category_id";
pub async fn find<'e, E: PgExecutor<'e>>(db: E, id: Uuid) -> Result<Option<Value>, sqlx::Error> {
    sqlx::query_scalar(&format!("{ADMIN_SELECT} WHERE a.id=$1"))
        .bind(id)
        .fetch_optional(db)
        .await
}

// Same timestamp/id ordering and single limit/offset over the combined feed.
// Doctor projection keeps every existing field and adds only type/format metadata.
const FEED: &str = "SELECT to_jsonb(d) || jsonb_build_object('article_type','DOCTOR','content_format','plain') AS article, COALESCE(d.published_at,d.created_at) AS sort_at,d.id FROM (SELECT a.id,a.author_id,p.full_name AS author_name,p.primary_specialty AS specialty,a.title,a.summary,a.body,a.references_list,a.status,a.version,a.created_at,a.updated_at,a.published_at FROM doctor_articles a JOIN doctor_public_directory p ON p.id=a.author_id WHERE a.status='PUBLISHED') d UNION ALL SELECT jsonb_build_object('id',a.id,'author_id',a.author_id,'author_name','AfyaMkononi Editorial','specialty','Editorial','title',a.title_en,'summary',left(a.body_en,300),'body',a.body_en,'title_sw',a.title_sw,'body_sw',a.body_sw,'references_list','[]'::jsonb,'status',a.status,'version',a.version,'created_at',a.created_at,'updated_at',a.updated_at,'published_at',a.published_at,'article_type','ADMIN','content_format','markdown','category_id',c.id,'category_name_en',c.name_en,'category_name_sw',c.name_sw,'featured',a.featured,'tags',a.tags,'cover_image_id',a.cover_image_id,'quiz',COALESCE((SELECT to_jsonb(qp)-'article_id'-'article_content_version' FROM article_quiz_publications qp WHERE qp.article_id=a.id AND qp.article_content_version=a.content_version),jsonb_build_object('schema_version',1,'included',false,'question_count',0,'revision_id',NULL)),'cover_url',CASE WHEN a.cover_image_id IS NOT NULL THEN '/articles/'||a.id::text||'/cover' END),a.published_at,a.id FROM admin_articles a JOIN article_categories c ON c.id=a.category_id WHERE a.status='PUBLISHED' AND a.approved_version=a.content_version";
pub async fn feed<'e, E: PgExecutor<'e>>(
    db: E,
    limit: i64,
    offset: i64,
) -> Result<Vec<Value>, sqlx::Error> {
    sqlx::query_scalar(&format!(
        "SELECT article FROM ({FEED}) f ORDER BY sort_at DESC,id LIMIT $1 OFFSET $2"
    ))
    .bind(limit)
    .bind(offset)
    .fetch_all(db)
    .await
}
pub async fn published<'e, E: PgExecutor<'e>>(
    db: E,
    id: Uuid,
) -> Result<Option<Value>, sqlx::Error> {
    sqlx::query_scalar(&format!("SELECT article FROM ({FEED}) f WHERE id=$1"))
        .bind(id)
        .fetch_optional(db)
        .await
}
