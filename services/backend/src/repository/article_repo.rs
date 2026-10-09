use crate::domain::{articles::DoctorArticle, ids::DoctorProfileId};
use sqlx::PgExecutor;
use uuid::Uuid;

const SELECT: &str = "SELECT a.id,a.author_id,d.full_name AS author_name,d.primary_specialty AS specialty,a.title,a.summary,a.body,a.references_list,a.status,a.version,a.created_at,a.updated_at,a.published_at FROM doctor_articles a JOIN doctor_public_directory d ON d.id=a.author_id";

pub async fn list<'e, E: PgExecutor<'e>>(
    executor: E,
    author: Option<DoctorProfileId>,
    limit: i64,
    offset: i64,
) -> Result<Vec<DoctorArticle>, sqlx::Error> {
    sqlx::query_as(&format!("{SELECT} WHERE ($1::uuid IS NOT NULL AND a.author_id=$1) OR ($1::uuid IS NULL AND a.status='PUBLISHED') ORDER BY COALESCE(a.published_at,a.created_at) DESC,a.id LIMIT $2 OFFSET $3"))
        .bind(author).bind(limit).bind(offset).fetch_all(executor).await
}

pub async fn find<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    author: Option<DoctorProfileId>,
) -> Result<Option<DoctorArticle>, sqlx::Error> {
    sqlx::query_as(&format!("{SELECT} WHERE a.id=$1 AND (($2::uuid IS NOT NULL AND a.author_id=$2) OR ($2::uuid IS NULL AND a.status='PUBLISHED'))"))
        .bind(id).bind(author).fetch_optional(executor).await
}
