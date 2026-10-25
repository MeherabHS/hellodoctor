//! Admin notification campaign backend (Milestones 6B + 6C).
//!
//! 6B: draft authoring and the draft -> review -> approval lifecycle.
//! 6C adds: schedule (immediate or future), pause/resume, cancel of an
//! in-flight campaign, and PII-safe aggregate delivery monitoring. Dispatch
//! itself — turning an approved, scheduled campaign into real per-recipient
//! `notifications` rows — lives in `notification_scheduler::
//! run_campaign_dispatch_pass`, never here: these handlers only ever change
//! the campaign row's own lifecycle state, so campaigns always inherit the
//! existing outbox's quiet-hours/frequency-cap/consent enforcement instead of
//! needing to reimplement it.
use crate::{
    abac::{self, Action},
    auth_context::AuthContext,
    domain::campaigns::{Campaign, CampaignAudience, CampaignStatus},
    error::AppError,
    repository::{campaign_repo, AppState},
    services::audit_service::{AuditLogEntry, AuditService},
};
use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::Row;
use uuid::Uuid;

/// Everything about a campaign's content an Admin can author — deliberately
/// closed (`deny_unknown_fields`) so no additional targeting criteria (a
/// diagnosis, an HIV status, any clinical attribute) can ever be smuggled in
/// through this payload. Audience selection is limited to the fixed
/// `CampaignAudience` enum (Patients / Doctors / Both) — there is no field
/// here through which a caller could narrow by anything else.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CampaignContent {
    pub title_en: String,
    pub title_sw: String,
    pub body_en: String,
    pub body_sw: String,
    pub audience: CampaignAudience,
}
impl CampaignContent {
    fn validate(&mut self) -> Result<(), AppError> {
        for (name, text, maximum) in [
            ("English title", &mut self.title_en, 160),
            ("Kiswahili title", &mut self.title_sw, 160),
            ("English message", &mut self.body_en, 600),
            ("Kiswahili message", &mut self.body_sw, 600),
        ] {
            *text = text.trim().to_owned();
            if text.is_empty() || text.chars().count() > maximum || text.contains('\0') {
                return Err(AppError::Validation(format!(
                    "{name} must contain 1-{maximum} characters."
                )));
            }
        }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateCampaign {
    pub creation_key: String,
    pub content: CampaignContent,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateCampaign {
    pub version: i32,
    pub content: CampaignContent,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CampaignTransition {
    pub version: i32,
}
#[derive(Deserialize, Default)]
pub struct CampaignQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub status: Option<CampaignStatus>,
}
#[derive(Deserialize)]
pub struct AudiencePreviewQuery {
    pub audience: CampaignAudience,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScheduleCampaign {
    pub version: i32,
    pub scheduled_for: DateTime<Utc>,
}

fn validate_creation_key(key: &str) -> Result<(), AppError> {
    if !(16..=64).contains(&key.len())
        || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err(AppError::Validation(
            "A valid draft creation key is required.".into(),
        ));
    }
    Ok(())
}

pub async fn list(
    State(state): State<AppState>,
    ctx: AuthContext,
    Query(query): Query<CampaignQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    abac::authorize(&ctx, Action::ManageCampaigns)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let rows = campaign_repo::list(
        &mut *tx,
        query.status,
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
    abac::authorize(&ctx, Action::ManageCampaigns)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let campaign = campaign_repo::find(&mut *tx, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Campaign not found.".into()))?;
    tx.commit().await?;
    Ok(Json(json!({"success":true,"data":campaign})))
}

/// A PII-safe count of how many Patients/Doctors/both are currently eligible
/// for the given audience selector — never a list of identifiable recipients.
pub async fn audience_preview(
    State(state): State<AppState>,
    ctx: AuthContext,
    Query(query): Query<AudiencePreviewQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    abac::authorize(&ctx, Action::ManageCampaigns)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let count = campaign_repo::audience_size(&mut *tx, query.audience).await?;
    tx.commit().await?;
    Ok(Json(
        json!({"success":true,"data":{"audience":query.audience,"eligible_count":count}}),
    ))
}

pub async fn create(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(mut payload): Json<CreateCampaign>,
) -> Result<Json<serde_json::Value>, AppError> {
    abac::authorize(&ctx, Action::ManageCampaigns)?;
    payload.content.validate()?;
    validate_creation_key(&payload.creation_key)?;
    let hash = format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&payload.content).map_err(|e| AppError::internal(e.to_string()))?
        )
    );
    let mut tx = state.begin_authed(&ctx).await?;
    let id = Uuid::new_v4();
    let inserted = sqlx::query(
        "INSERT INTO campaigns(id,created_by,title_en,title_sw,body_en,body_sw,audience,creation_key,creation_hash) \
         VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT(created_by,creation_key) DO NOTHING",
    )
    .bind(id)
    .bind(ctx.user_id)
    .bind(&payload.content.title_en)
    .bind(&payload.content.title_sw)
    .bind(&payload.content.body_en)
    .bind(&payload.content.body_sw)
    .bind(payload.content.audience)
    .bind(&payload.creation_key)
    .bind(&hash)
    .execute(&mut *tx)
    .await?
    .rows_affected()
        == 1;
    let id = if inserted {
        id
    } else {
        let row = sqlx::query(
            "SELECT id,creation_hash FROM campaigns WHERE created_by=$1 AND creation_key=$2",
        )
        .bind(ctx.user_id)
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
    let campaign = campaign_repo::find(&mut *tx, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Campaign not found.".into()))?;
    if inserted {
        log(&mut tx, &ctx, "CAMPAIGN_CREATE", &campaign).await?;
    }
    tx.commit().await?;
    Ok(Json(json!({"success":true,"data":campaign})))
}

pub async fn update(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
    Json(mut payload): Json<UpdateCampaign>,
) -> Result<Json<serde_json::Value>, AppError> {
    abac::authorize(&ctx, Action::ManageCampaigns)?;
    payload.content.validate()?;
    let mut tx = state.begin_authed(&ctx).await?;
    // Any content edit invalidates whatever review/approval state the
    // campaign was in — it always returns to DRAFT, bumps `content_version`,
    // and clears the approval columns, so a stale approval can never carry
    // over onto edited content. Only reachable before any fan-out has
    // started (DRAFT/PENDING_REVIEW/APPROVED/SCHEDULED) — once a campaign has
    // ever been SENDING/PAUSED/COMPLETED its content is immutable, matching
    // the trigger's own transition matrix.
    let changed = sqlx::query(
        "UPDATE campaigns SET title_en=$1,title_sw=$2,body_en=$3,body_sw=$4,audience=$5, \
         status='DRAFT',version=version+1,content_version=content_version+1,updated_at=NOW(), \
         approved_content_version=NULL,approved_by=NULL,approved_at=NULL \
         WHERE id=$6 AND version=$7 AND status IN ('DRAFT','PENDING_REVIEW','APPROVED','SCHEDULED')",
    )
    .bind(&payload.content.title_en)
    .bind(&payload.content.title_sw)
    .bind(&payload.content.body_en)
    .bind(&payload.content.body_sw)
    .bind(payload.content.audience)
    .bind(id)
    .bind(payload.version)
    .execute(&mut *tx)
    .await?
    .rows_affected()
        == 1;
    finish_mutation(tx, &ctx, id, changed, "CAMPAIGN_EDIT").await
}

pub async fn submit(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<CampaignTransition>,
) -> Result<Json<serde_json::Value>, AppError> {
    abac::authorize(&ctx, Action::ManageCampaigns)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let changed = sqlx::query(
        "UPDATE campaigns SET status='PENDING_REVIEW',version=version+1,updated_at=NOW() \
         WHERE id=$1 AND version=$2 AND status='DRAFT'",
    )
    .bind(id)
    .bind(payload.version)
    .execute(&mut *tx)
    .await?
    .rows_affected()
        == 1;
    finish_mutation(tx, &ctx, id, changed, "CAMPAIGN_SUBMIT").await
}

pub async fn approve(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<CampaignTransition>,
) -> Result<Json<serde_json::Value>, AppError> {
    abac::authorize(&ctx, Action::ManageCampaigns)?;
    let mut tx = state.begin_authed(&ctx).await?;
    // Approval is bound to `content_version`, not `version` — it must still
    // read as "approved" after later lifecycle transitions (schedule/pause/
    // resume) bump `version` for their own optimistic-concurrency check,
    // without that looking like the content changed.
    let changed = sqlx::query(
        "UPDATE campaigns SET status='APPROVED',version=version+1,approved_content_version=content_version, \
         approved_by=$3,approved_at=NOW(),updated_at=NOW() \
         WHERE id=$1 AND version=$2 AND status='PENDING_REVIEW'",
    )
    .bind(id)
    .bind(payload.version)
    .bind(ctx.user_id)
    .execute(&mut *tx)
    .await?
    .rows_affected()
        == 1;
    finish_mutation(tx, &ctx, id, changed, "CAMPAIGN_APPROVE").await
}

/// Sets (or changes) when an already-approved campaign should start sending.
/// "Immediate" is simply a schedule time of now-or-earlier: the dispatch
/// pass's promotion sweep (`run_campaign_dispatch_pass`) picks up any
/// `SCHEDULED` campaign whose `scheduled_for <= now()` on its very next tick,
/// so there is no separate "send now" code path to keep in sync with the
/// scheduled one. Only reachable from APPROVED (first schedule) or SCHEDULED
/// itself (reschedule, before fan-out has started) — never SENDING/PAUSED, so
/// a reschedule can never land on a campaign that already has recipients.
pub async fn schedule(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<ScheduleCampaign>,
) -> Result<Json<serde_json::Value>, AppError> {
    abac::authorize(&ctx, Action::ManageCampaigns)?;
    if payload.scheduled_for < Utc::now() - Duration::minutes(1) {
        return Err(AppError::Validation(
            "Schedule time must not be meaningfully in the past.".into(),
        ));
    }
    let mut tx = state.begin_authed(&ctx).await?;
    let changed = sqlx::query(
        "UPDATE campaigns SET status='SCHEDULED',version=version+1,scheduled_for=$3,updated_at=NOW() \
         WHERE id=$1 AND version=$2 AND status IN ('APPROVED','SCHEDULED')",
    )
    .bind(id)
    .bind(payload.version)
    .bind(payload.scheduled_for)
    .execute(&mut *tx)
    .await?
    .rows_affected()
        == 1;
    finish_mutation(tx, &ctx, id, changed, "CAMPAIGN_SCHEDULE").await
}

/// Pauses a scheduled-but-not-yet-due, or actively sending, campaign. Does
/// not touch `fanout_cursor` — resuming continues fan-out exactly where it
/// left off, never restarting or skipping.
pub async fn pause(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<CampaignTransition>,
) -> Result<Json<serde_json::Value>, AppError> {
    abac::authorize(&ctx, Action::ManageCampaigns)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let changed = sqlx::query(
        "UPDATE campaigns SET status='PAUSED',version=version+1,paused_at=NOW(),updated_at=NOW() \
         WHERE id=$1 AND version=$2 AND status IN ('SCHEDULED','SENDING')",
    )
    .bind(id)
    .bind(payload.version)
    .execute(&mut *tx)
    .await?
    .rows_affected()
        == 1;
    finish_mutation(tx, &ctx, id, changed, "CAMPAIGN_PAUSE").await
}

/// Resumes a paused campaign. If fan-out had already started
/// (`sending_started_at` is set) it goes back to SENDING so the dispatch pass
/// continues from `fanout_cursor`; otherwise it goes back to SCHEDULED, where
/// the normal due-time promotion sweep picks it up again once
/// `scheduled_for` arrives (or immediately, if it already has).
pub async fn resume(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<CampaignTransition>,
) -> Result<Json<serde_json::Value>, AppError> {
    abac::authorize(&ctx, Action::ManageCampaigns)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let current = campaign_repo::find(&mut *tx, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Campaign not found.".into()))?;
    let target = if current.sending_started_at.is_some() {
        CampaignStatus::Sending
    } else {
        CampaignStatus::Scheduled
    };
    let changed = sqlx::query(
        "UPDATE campaigns SET status=$3,version=version+1,paused_at=NULL,updated_at=NOW() \
         WHERE id=$1 AND version=$2 AND status='PAUSED'",
    )
    .bind(id)
    .bind(payload.version)
    .bind(target)
    .execute(&mut *tx)
    .await?
    .rows_affected()
        == 1;
    finish_mutation(tx, &ctx, id, changed, "CAMPAIGN_RESUME").await
}

/// Cancels a campaign at any non-terminal stage, including mid-flight. A
/// single notification already handed to the FCM provider (`SENT_TO_PROVIDER`)
/// is never touched — this schema makes no claim that an accepted FCM message
/// can be recalled — but every not-yet-sent or still-retrying row for this
/// campaign is deleted outright, via the narrowly scoped
/// `notifications_admin_cancel_campaign_delete` RLS policy (only rows linked
/// to a campaign that is *already* CANCELLED), so the dispatch/delivery
/// passes can never pick them up again.
pub async fn cancel(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<CampaignTransition>,
) -> Result<Json<serde_json::Value>, AppError> {
    abac::authorize(&ctx, Action::ManageCampaigns)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let changed = sqlx::query(
        "UPDATE campaigns SET status='CANCELLED',version=version+1,cancelled_at=NOW(),updated_at=NOW(), \
         approved_content_version=NULL,approved_by=NULL,approved_at=NULL \
         WHERE id=$1 AND version=$2 AND status IN ('DRAFT','PENDING_REVIEW','APPROVED','SCHEDULED','SENDING','PAUSED')",
    )
    .bind(id)
    .bind(payload.version)
    .execute(&mut *tx)
    .await?
    .rows_affected()
        == 1;
    if changed {
        sqlx::query(
            "DELETE FROM notifications WHERE related_entity_type='CAMPAIGN' AND related_entity_id=$1 \
             AND NOT (delivery_status='SENT_TO_PROVIDER' OR (delivery_status='FAILED' AND next_delivery_at IS NULL))",
        )
        .bind(id)
        .execute(&mut *tx)
        .await?;
    }
    finish_mutation(tx, &ctx, id, changed, "CAMPAIGN_CANCEL").await
}

/// Aggregate, PII-safe monitoring for one campaign: live audience eligibility
/// count plus the delivery-status breakdown from
/// `campaign_repo::delivery_stats`. Never returns a recipient identity.
/// `sent_to_provider` is reported exactly as that — handed to FCM — and is
/// never relabeled "delivered" or "read", since this schema has no
/// device-delivery or read receipt for push notifications.
pub async fn stats(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    abac::authorize(&ctx, Action::ManageCampaigns)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let campaign = campaign_repo::find(&mut *tx, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Campaign not found.".into()))?;
    let audience_count = campaign_repo::audience_size(&mut *tx, campaign.audience).await?;
    let delivery = campaign_repo::delivery_stats(&mut *tx, id).await?;
    tx.commit().await?;
    Ok(Json(json!({"success":true,"data":{
        "status": campaign.status,
        "audience_count": audience_count,
        "queued": delivery.queued,
        "sent_to_provider": delivery.sent_to_provider,
        "deferred_by_preference_or_quiet_hours": delivery.deferred_by_preference_or_quiet_hours,
        "retry_pending": delivery.retry_pending,
        "permanently_failed": delivery.permanently_failed,
        "sending_started_at": campaign.sending_started_at,
        "completed_at": campaign.completed_at,
        "cancelled_at": campaign.cancelled_at,
    }})))
}

async fn finish_mutation(
    mut tx: sqlx::Transaction<'_, sqlx::Postgres>,
    ctx: &AuthContext,
    id: Uuid,
    changed: bool,
    action: &str,
) -> Result<Json<serde_json::Value>, AppError> {
    let campaign = campaign_repo::find(&mut *tx, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Campaign not found.".into()))?;
    if !changed {
        return Err(AppError::Conflict(
            "Campaign changed or this transition is unavailable. Refresh before retrying.".into(),
        ));
    }
    log(&mut tx, ctx, action, &campaign).await?;
    tx.commit().await?;
    Ok(Json(json!({"success":true,"data":campaign})))
}

async fn log(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ctx: &AuthContext,
    action: &str,
    campaign: &Campaign,
) -> Result<(), AppError> {
    AuditService::log_in_tx(
        &mut **tx,
        AuditLogEntry::new(
            Some(ctx.user_id),
            ctx.role,
            action,
            "CAMPAIGN",
            Some(campaign.id),
            "SUCCESS",
        )
        .with_after(json!({"status":campaign.status,"version":campaign.version,"audience":campaign.audience})),
    )
    .await?;
    Ok(())
}
