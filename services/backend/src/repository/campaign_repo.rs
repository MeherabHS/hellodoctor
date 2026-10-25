use crate::domain::campaigns::{Campaign, CampaignAudience, CampaignStatus};
use crate::domain::ids::UserId;
use serde::Serialize;
use sqlx::PgExecutor;
use uuid::Uuid;

const SELECT: &str = "SELECT id,created_by,title_en,title_sw,body_en,body_sw,audience,status,version,content_version,approved_content_version,approved_by,approved_at,created_at,updated_at,scheduled_for,sending_started_at,paused_at,completed_at,cancelled_at,fanout_cursor FROM campaigns";

pub async fn list<'e, E: PgExecutor<'e>>(
    executor: E,
    status: Option<CampaignStatus>,
    limit: i64,
    offset: i64,
) -> Result<Vec<Campaign>, sqlx::Error> {
    sqlx::query_as(&format!(
        "{SELECT} WHERE ($1::campaign_status IS NULL OR status=$1) ORDER BY created_at DESC, id LIMIT $2 OFFSET $3"
    ))
    .bind(status)
    .bind(limit)
    .bind(offset)
    .fetch_all(executor)
    .await
}

pub async fn find<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
) -> Result<Option<Campaign>, sqlx::Error> {
    sqlx::query_as(&format!("{SELECT} WHERE id=$1"))
        .bind(id)
        .fetch_optional(executor)
        .await
}

/// Claims exclusive processing of exactly this one `SENDING` campaign for
/// the lifetime of the caller's transaction. Returns `None` if another
/// worker already holds it, or if it is no longer `SENDING` (paused/
/// cancelled since the caller last listed candidates) — the caller's own
/// revalidation-before-dispatch, not a separate check. See
/// `notification_scheduler::run_one_campaign_fanout`.
pub async fn lock_sending<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
) -> Result<Option<Campaign>, sqlx::Error> {
    sqlx::query_as(&format!(
        "{SELECT} WHERE id=$1 AND status='SENDING' FOR UPDATE SKIP LOCKED"
    ))
    .bind(id)
    .fetch_optional(executor)
    .await
}

/// Advances the fan-out keyset cursor after a batch, without otherwise
/// touching the campaign's lifecycle state. `version` still bumps (every
/// mutation does), but this never changes `status`.
pub async fn advance_fanout_cursor<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    cursor: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE campaigns SET fanout_cursor=$2,version=version+1,updated_at=NOW() WHERE id=$1 AND status='SENDING'",
    )
    .bind(id)
    .bind(cursor)
    .execute(executor)
    .await?;
    Ok(())
}

/// Marks fan-out complete: every eligible recipient at the time of the last
/// batch has a `notifications` row (not necessarily delivered yet — see
/// `delivery_stats` for that distinction).
pub async fn complete_fanout<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE campaigns SET status='COMPLETED',completed_at=NOW(),version=version+1,updated_at=NOW() WHERE id=$1 AND status='SENDING'",
    )
    .bind(id)
    .execute(executor)
    .await?;
    Ok(())
}

/// A PII-safe eligible-recipient count for the given audience selector —
/// returns only a number, never any row identifying a specific Patient or
/// Doctor. Mirrors the same eligibility filters the notification scheduler's
/// own role-scoped broadcast passes already use (verified, active, non-
/// deleted accounts only): see `notification_scheduler.rs`'s greeting pass.
pub async fn audience_size<'e, E: PgExecutor<'e>>(
    executor: E,
    audience: CampaignAudience,
) -> Result<i64, sqlx::Error> {
    let (want_patients, want_doctors) = audience_flags(audience);
    sqlx::query_scalar(
        "SELECT \
           (CASE WHEN $1 THEN (SELECT COUNT(*) FROM patient_profiles p JOIN users u ON u.id=p.user_id WHERE p.user_id IS NOT NULL AND u.is_active AND u.deleted_at IS NULL) ELSE 0 END) + \
           (CASE WHEN $2 THEN (SELECT COUNT(*) FROM doctor_profiles d JOIN users u ON u.id=d.user_id WHERE d.professional_verification_status='VERIFIED' AND d.deleted_at IS NULL AND u.is_active AND u.deleted_at IS NULL) ELSE 0 END)",
    )
    .bind(want_patients)
    .bind(want_doctors)
    .fetch_one(executor)
    .await
}

fn audience_flags(audience: CampaignAudience) -> (bool, bool) {
    match audience {
        CampaignAudience::Patients => (true, false),
        CampaignAudience::Doctors => (false, true),
        CampaignAudience::Both => (true, true),
    }
}

/// One page of eligible recipients for fan-out, in stable `user_id` keyset
/// order (`user_id > cursor`, never `OFFSET`) — the standard way to paginate
/// a set that can gain or lose rows between pages without silently skipping
/// or reprocessing anyone: a row inserted or deleted elsewhere in the table
/// never shifts where an already-fixed cursor value falls relative to it.
/// Returning fewer than `limit` rows is this pass's own signal that the
/// audience has been fully walked — see `run_campaign_dispatch_pass`.
#[derive(Debug, sqlx::FromRow)]
pub struct FanoutCandidate {
    pub user_id: UserId,
    pub preferred_language: String,
}

pub async fn fanout_candidates<'e, E: PgExecutor<'e>>(
    executor: E,
    audience: CampaignAudience,
    cursor: Option<Uuid>,
    limit: i64,
) -> Result<Vec<FanoutCandidate>, sqlx::Error> {
    let (want_patients, want_doctors) = audience_flags(audience);
    sqlx::query_as(
        "( \
            SELECT u.id AS user_id, u.preferred_language FROM patient_profiles p JOIN users u ON u.id = p.user_id \
             WHERE $1 AND p.user_id IS NOT NULL AND u.is_active AND u.deleted_at IS NULL \
               AND ($2::uuid IS NULL OR u.id > $2) \
         ) UNION ALL ( \
            SELECT u.id, u.preferred_language FROM doctor_profiles d JOIN users u ON u.id = d.user_id \
             WHERE $3 AND d.professional_verification_status = 'VERIFIED' AND d.deleted_at IS NULL \
               AND u.is_active AND u.deleted_at IS NULL AND ($2::uuid IS NULL OR u.id > $2) \
         ) ORDER BY user_id LIMIT $4",
    )
    .bind(want_patients)
    .bind(cursor)
    .bind(want_doctors)
    .bind(limit)
    .fetch_all(executor)
    .await
}

/// Aggregate, campaign-scoped delivery statistics for Admin monitoring.
/// Every bucket is derived from columns the existing push-delivery pass
/// already writes (`notification_repo::mark_delivery_*`) — no new
/// `notifications` columns, no per-recipient identity ever returned.
/// `SENT_TO_PROVIDER` is reported under its own name, deliberately never
/// relabeled "delivered" or "read": this schema has no device-delivery
/// receipt, only "handed to FCM".
#[derive(Debug, Default, Serialize, sqlx::FromRow)]
pub struct CampaignDeliveryStats {
    pub queued: i64,
    pub sent_to_provider: i64,
    pub deferred_by_preference_or_quiet_hours: i64,
    pub retry_pending: i64,
    pub permanently_failed: i64,
}

pub async fn delivery_stats<'e, E: PgExecutor<'e>>(
    executor: E,
    campaign_id: Uuid,
) -> Result<CampaignDeliveryStats, sqlx::Error> {
    sqlx::query_as(
        "SELECT \
           COUNT(*) FILTER (WHERE delivery_status IN ('CREATED','DUE')) AS queued, \
           COUNT(*) FILTER (WHERE delivery_status='SENT_TO_PROVIDER') AS sent_to_provider, \
           COUNT(*) FILTER (WHERE delivery_status='FAILED' AND (last_delivery_error LIKE 'Deferred:%' OR last_delivery_error LIKE 'Suppressed:%')) AS deferred_by_preference_or_quiet_hours, \
           COUNT(*) FILTER (WHERE delivery_status='FAILED' AND next_delivery_at IS NOT NULL AND last_delivery_error NOT LIKE 'Deferred:%' AND last_delivery_error NOT LIKE 'Suppressed:%') AS retry_pending, \
           COUNT(*) FILTER (WHERE delivery_status='FAILED' AND next_delivery_at IS NULL AND last_delivery_error NOT LIKE 'Suppressed:%') AS permanently_failed \
         FROM notifications WHERE related_entity_type='CAMPAIGN' AND related_entity_id=$1",
    )
    .bind(campaign_id)
    .fetch_one(executor)
    .await
}
