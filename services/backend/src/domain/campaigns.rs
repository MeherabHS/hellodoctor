use super::ids::UserId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[sqlx(type_name = "campaign_status", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CampaignStatus {
    Draft,
    PendingReview,
    Approved,
    Scheduled,
    Sending,
    Paused,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[sqlx(type_name = "campaign_audience", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CampaignAudience {
    Patients,
    Doctors,
    Both,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Campaign {
    pub id: Uuid,
    pub created_by: UserId,
    pub title_en: String,
    pub title_sw: String,
    pub body_en: String,
    pub body_sw: String,
    pub audience: CampaignAudience,
    pub status: CampaignStatus,
    /// Pure optimistic-concurrency row-revision counter — bumped by every
    /// mutation (content edit or lifecycle transition alike). Every
    /// PATCH/transition endpoint's `{version}` payload field refers to this.
    pub version: i32,
    /// Advances only when title/body/audience actually change. Approval is
    /// bound to this, not `version`, so it survives later lifecycle
    /// transitions (schedule/pause/resume) but is invalidated by any edit.
    pub content_version: i32,
    pub approved_content_version: Option<i32>,
    pub approved_by: Option<UserId>,
    pub approved_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub scheduled_for: Option<DateTime<Utc>>,
    pub sending_started_at: Option<DateTime<Utc>>,
    pub paused_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub cancelled_at: Option<DateTime<Utc>>,
    /// Keyset-pagination cursor for the resumable fan-out worker — the last
    /// `user_id` already processed. `None` until fan-out has inserted
    /// anything. See `repository::campaign_repo::fanout_candidates`.
    pub fanout_cursor: Option<Uuid>,
}
