use super::ids::DoctorProfileId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[sqlx(
    type_name = "doctor_article_status",
    rename_all = "SCREAMING_SNAKE_CASE"
)]
pub enum ArticleStatus {
    Draft,
    Published,
    Archived,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct DoctorArticle {
    pub id: Uuid,
    pub author_id: DoctorProfileId,
    pub author_name: String,
    pub specialty: String,
    pub title: String,
    pub summary: String,
    pub body: String,
    pub references_list: serde_json::Value,
    pub status: ArticleStatus,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub published_at: Option<DateTime<Utc>>,
    /// Doctor Web redesign (2026-11-14): optional topic, complete Swahili version, cover.
    pub topic_key: Option<String>,
    pub title_sw: Option<String>,
    pub summary_sw: Option<String>,
    pub body_sw: Option<String>,
    pub cover_image_id: Option<Uuid>,
}
