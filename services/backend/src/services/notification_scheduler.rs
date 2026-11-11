//! DB-backed due-notification worker for the Doctor Engagement extension.
//! Deliberately not a new broker/queue (Redis or otherwise) — per the
//! approved spec, "a simple DB-backed due-notification worker is acceptable
//! at this scale," and this project already has one Redis instance used for
//! rate-limiting/sessions, not a job queue; adding a second responsibility to
//! it (or a third broker entirely) for two low-volume, low-latency-tolerance
//! notification types would be more infrastructure than the problem needs.
//!
//! Each notification pass is idempotent via `notifications.dedup_key`
//! (`notification_repo::insert_if_new`'s `ON CONFLICT DO NOTHING`), so a
//! crashed/restarted worker or two overlapping ticks can never double-create:
//! - Daily consultation-count summary, once per Doctor per Kenya-local
//!   calendar day, at a configurable local time (`AppConfig::
//!   doctor_daily_summary_time_local`).
//! - T-15-minute pre-consultation reminders, created the moment an eligible
//!   appointment enters its `[start - 15m, start)` window (including
//!   immediately, for a booking made with less than 15 minutes' notice —
//!   never a past-dated job, never a duplicate).

use crate::domain::ids::UserId;
use crate::domain::models::Notification;
use crate::repository::{campaign_repo, notification_repo, AppState};
use crate::services::document_storage_service::DocumentStorageService;
use crate::services::notification_service::{nairobi_local_date, nairobi_offset};
use crate::services::notification_transport::{PushOptions, PushOutcome};
use chrono::{DateTime, Datelike, Duration as ChronoDuration, Timelike, Utc};
use std::time::Duration;
use uuid::Uuid;

/// Starts the background scheduler loop. Call once at startup (see
/// `main.rs`), mirroring `system_error::init`'s pattern of a single
/// `tokio::spawn`'d task driven off `AppState`'s existing `system_db` pool —
/// no new connection pool or role is introduced.
pub fn start(state: AppState) {
    if state.config.recovery_mode {
        tracing::warn!(
            "RECOVERY_MODE=true: notification, campaign, reminder and payment-hold scheduler is disabled"
        );
        return;
    }
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            let now = Utc::now();
            // Keep this isolated CMS pass from delaying the existing notification
            // and campaign passes when content storage is slow. Cancellation rolls
            // back uncommitted work; durable media intents are retried next tick.
            match tokio::time::timeout(
                Duration::from_secs(5),
                super::article_media_service::maintenance(&state),
            )
            .await
            {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    tracing::error!(error = ?error, "article publication/media maintenance failed")
                }
                Err(_) => {
                    tracing::warn!("article maintenance deferred after its five-second budget")
                }
            }
            // Account-erasure recovery only; delivery and notification state are unchanged.
            match tokio::time::timeout(
                Duration::from_secs(5),
                super::data_rights_service::retry_pending_erasures(&state),
            )
            .await
            {
                Ok(Ok(_)) => {}
                _ => {
                    tracing::warn!("Account erasure recovery deferred; durable jobs remain pending")
                }
            }
            // Remediation E1: copy completed-erasure intent into the
            // externally-custodied journal. Never runs while RECOVERY_MODE
            // is true (this entire scheduler already returns early above),
            // which is correct: no new erasures can be requested in recovery
            // mode either (see global_guard's recovery allowlist).
            match tokio::time::timeout(
                Duration::from_secs(5),
                super::dr_reconciliation::sync_outbox_to_journal(&state),
            )
            .await
            {
                Ok(Ok(_)) => {}
                _ => tracing::warn!(
                    "Erasure journal sync deferred; durable outbox rows remain pending"
                ),
            }
            if let Err(error) = super::consultation_trace::maintenance(&state).await {
                tracing::error!(error = ?error, "consultation trace maintenance failed");
            }
            if let Err(error) = run_hold_expiry_sweep(&state).await {
                tracing::error!(error = ?error, "payment-hold expiry sweep failed");
            }
            if let Err(error) = run_expired_upload_cleanup(&state).await {
                tracing::error!(error = ?error, "expired clinical-upload cleanup failed");
            }
            if let Err(error) = run_consultation_presence_sweep(&state).await {
                tracing::error!(error = ?error, "consultation presence grace sweep failed");
            }
            if let Err(error) = run_hold_notification_pass(&state).await {
                tracing::error!(error = ?error, "payment-hold notification pass failed");
            }
            if let Err(e) = run_daily_summary_pass(&state, now).await {
                tracing::error!(error = ?e, "notification scheduler: daily-summary pass failed");
            }
            if let Err(e) = run_t15_reminder_pass(&state, now).await {
                tracing::error!(error = ?e, "notification scheduler: T-15 reminder pass failed");
            }
            if let Err(e) = run_consultation_ready_pass(&state, now).await {
                tracing::error!(error = ?e, "notification scheduler: consultation-ready pass failed");
            }
            if let Err(e) = run_greeting_pass(&state, now).await {
                tracing::error!(error = ?e, "notification scheduler: good-morning greeting pass failed");
            }
            if let Err(e) = run_health_education_pass(&state, now).await {
                tracing::error!(error = ?e, "notification scheduler: health-education pass failed");
            }
            if let Err(e) = run_birthday_pass(&state, now).await {
                tracing::error!(error = ?e, "notification scheduler: birthday pass failed");
            }
            if let Err(e) = run_holiday_pass(&state, now).await {
                tracing::error!(error = ?e, "notification scheduler: Kenyan holiday pass failed");
            }
            if let Err(e) = run_new_article_pass(&state).await {
                tracing::error!(error = ?e, "notification scheduler: new-article engagement pass failed");
            }
            if let Err(e) = run_campaign_dispatch_pass(&state).await {
                tracing::error!(error = ?e, "notification scheduler: campaign dispatch pass failed");
            }
            if let Err(e) = run_push_delivery_pass(&state).await {
                tracing::error!(error = ?e, "notification scheduler: FCM delivery pass failed");
            }
            // Phase 0 email integration: reconciliation only, never a
            // resend — see `email_delivery::run_email_outbox_expiry_sweep`'s
            // doc comment for why OTP email delivery is never retried from
            // here.
            if let Err(e) = crate::email_delivery::run_email_outbox_expiry_sweep(&state).await {
                tracing::error!(error = ?e, "notification scheduler: email outbox expiry sweep failed");
            }
            // Phase 1B: durable fallback for transactional email — anything
            // the post-commit `email_dispatch_worker::dispatch_now` fast
            // path missed (process restart, transient failure now past its
            // backoff, a stale lease recovered from a crashed attempt).
            if let Err(e) =
                super::email_dispatch_worker::run_transactional_email_dispatch_pass(&state).await
            {
                tracing::error!(error = ?e, "notification scheduler: transactional email dispatch pass failed");
            }
        }
    });
}

const CAMPAIGN_FANOUT_BATCH: i64 = 200;
const CAMPAIGN_FANOUT_MAX_BATCHES_PER_TICK: usize = 10;

/// Milestone 6C — Admin campaign scheduling and dispatch. Promotes due
/// `SCHEDULED` campaigns to `SENDING`, then resumably fans each `SENDING`
/// campaign's eligible audience out into the existing `notifications` outbox
/// — never a second send path, never a direct FCM call from here. Each
/// campaign's own work is claimed via `SELECT ... FOR UPDATE SKIP LOCKED` on
/// its single row (`campaign_repo::lock_sending`), so two overlapping ticks
/// or two backend instances can never fan the same campaign out at once; the
/// per-recipient `dedup_key` (`campaign:{id}:{user_id}`, enforced by
/// `notifications`' own unique index via `notification_repo::insert_if_new`)
/// makes a restart or an overlapping run safe even if it somehow raced
/// anyway. Progress is a keyset cursor over `user_id` (never `OFFSET`), so a
/// row inserted or deleted elsewhere in `patient_profiles`/`doctor_profiles`
/// during a long fan-out cannot cause an eligible recipient to be silently
/// skipped or reprocessed. A campaign is marked `COMPLETED` the instant a
/// batch returns fewer candidates than requested — fan-out completion, which
/// is NOT the same thing as FCM delivery (tracked separately, see
/// `campaign_repo::delivery_stats`).
pub async fn run_campaign_dispatch_pass(state: &AppState) -> Result<(), crate::error::AppError> {
    let ctx = crate::auth_context::AuthContext::system();

    // Promotion: a plain conditional UPDATE is race-safe on its own — a
    // second concurrent promotion attempt for the same row simply matches
    // zero rows once the first has committed.
    {
        let mut tx = state
            .begin_authed_as(&ctx, crate::repository::DbRole::SystemWorker)
            .await?;
        sqlx::query(
            "UPDATE campaigns SET status='SENDING',sending_started_at=COALESCE(sending_started_at,NOW()),version=version+1,updated_at=NOW() \
             WHERE status='SCHEDULED' AND scheduled_for<=NOW()",
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
    }

    let sending_ids: Vec<Uuid> = {
        let mut tx = state
            .begin_authed_as(&ctx, crate::repository::DbRole::SystemWorker)
            .await?;
        let ids = sqlx::query_scalar("SELECT id FROM campaigns WHERE status='SENDING'")
            .fetch_all(&mut *tx)
            .await?;
        tx.commit().await?;
        ids
    };

    for campaign_id in sending_ids {
        if let Err(error) = run_one_campaign_fanout(state, &ctx, campaign_id).await {
            tracing::error!(error = ?error, campaign_id = %campaign_id, "campaign fan-out failed");
        }
    }
    Ok(())
}

async fn run_one_campaign_fanout(
    state: &AppState,
    ctx: &crate::auth_context::AuthContext,
    campaign_id: Uuid,
) -> Result<(), crate::error::AppError> {
    let mut tx = state
        .begin_authed_as(ctx, crate::repository::DbRole::SystemWorker)
        .await?;
    // Revalidates campaign state before any dispatch work: if it was paused
    // or cancelled since the caller listed `SENDING` ids, or another worker
    // already holds it, this returns no row and nothing further happens.
    let Some(campaign) = campaign_repo::lock_sending(&mut *tx, campaign_id).await? else {
        tx.rollback().await?;
        return Ok(());
    };
    let (title_en, title_sw, body_en, body_sw) = (
        campaign.title_en.clone(),
        campaign.title_sw.clone(),
        campaign.body_en.clone(),
        campaign.body_sw.clone(),
    );
    let mut cursor = campaign.fanout_cursor;

    for _ in 0..CAMPAIGN_FANOUT_MAX_BATCHES_PER_TICK {
        let candidates = campaign_repo::fanout_candidates(
            &mut *tx,
            campaign.audience,
            cursor,
            CAMPAIGN_FANOUT_BATCH,
        )
        .await?;
        if candidates.is_empty() {
            campaign_repo::complete_fanout(&mut *tx, campaign_id).await?;
            break;
        }
        for candidate in &candidates {
            let (title, body) = if candidate.preferred_language == "sw" {
                (title_sw.as_str(), body_sw.as_str())
            } else {
                (title_en.as_str(), body_en.as_str())
            };
            let dedup_key = format!("campaign:{campaign_id}:{}", candidate.user_id);
            notification_repo::insert_if_new(
                &mut *tx,
                Uuid::new_v4(),
                candidate.user_id,
                "ADMIN_CAMPAIGN",
                title,
                body,
                Some("CAMPAIGN"),
                Some(campaign_id),
                None,
                &dedup_key,
                None,
            )
            .await?;
        }
        let last = candidates.last().expect("checked non-empty above").user_id;
        cursor = Some(last.into_inner());
        campaign_repo::advance_fanout_cursor(&mut *tx, campaign_id, last.into_inner()).await?;
        if (candidates.len() as i64) < CAMPAIGN_FANOUT_BATCH {
            campaign_repo::complete_fanout(&mut *tx, campaign_id).await?;
            break;
        }
    }
    tx.commit().await?;
    Ok(())
}

/// Re-validates a claimed notification against the LIVE authoritative state
/// immediately before dispatch — see the call site's own doc comment for why
/// TTL alone is not sufficient. Every type other than the two time-sensitive
/// ones below is unconditionally eligible: once a `BOOKING_CONFIRMED` or
/// `EARNING_CREDITED` happened, it stays a true historical fact forever, so
/// there is nothing to re-check.
async fn is_still_eligible_for_dispatch(
    state: &AppState,
    connection: &mut sqlx::PgConnection,
    row: &notification_repo::DueNotification,
) -> Result<bool, crate::error::AppError> {
    let Some(appointment_id) = row.related_entity_id else {
        return Ok(true);
    };
    match row.notification_type.as_str() {
        "CONSULTATION_READY" => {
            let before_mins = state.config.agora_token_window_before_mins as i32;
            let after_mins = state.config.agora_token_window_after_mins as i32;
            let (eligible,): (bool,) = sqlx::query_as(
                "SELECT EXISTS( \
                   SELECT 1 FROM appointments a \
                     JOIN doctor_schedule_slots s ON s.id = a.slot_id \
                    WHERE a.id = $1 AND a.status = 'CONFIRMED' \
                      AND clock_timestamp() >= s.start_time - make_interval(mins => $2) \
                      AND clock_timestamp() <= s.end_time + make_interval(mins => $3))",
            )
            .bind(appointment_id)
            .bind(before_mins)
            .bind(after_mins)
            .fetch_one(&mut *connection)
            .await?;
            Ok(eligible)
        }
        "CONSULTATION_REJOIN" => {
            let (eligible,): (bool,) = sqlx::query_as(
                "SELECT EXISTS( \
                   SELECT 1 FROM consultation_sessions \
                    WHERE appointment_id = $1 AND recovery_state = 'GRACE_PERIOD' \
                      AND empty_room_deadline > clock_timestamp())",
            )
            .bind(appointment_id)
            .fetch_one(&mut *connection)
            .await?;
            Ok(eligible)
        }
        _ => Ok(true),
    }
}

/// Milestone 3 delivery-time gating outcome. Deliberately computed only at
/// delivery time, never at creation — the in-app inbox row is always
/// created regardless (requirement: "retaining appropriate in-app
/// notification history"), so none of this can ever make a notification
/// vanish from a user's own history, only decide whether/when a push goes
/// out. Only ever reached for `OPERATIONAL`/`ENGAGEMENT` categories —
/// `notification_delivery_gate` returns `Allow` unconditionally for
/// SECURITY/TRANSACTIONAL/CLINICAL, by construction, so essential records
/// can never be suppressed by a marketing opt-out (requirement 9).
enum DeliveryGate {
    Allow,
    SuppressPermanently(&'static str),
    Defer(DateTime<Utc>, &'static str),
    LostClaim,
}

/// The four recurring "greeting"-style types (Milestones 4/5), gated by the
/// separate, stricter-by-default `greeting_notifications_enabled`
/// preference instead of the general `operational_enabled`/
/// `engagement_enabled` toggle those types would otherwise fall under by
/// category — see that preference's own doc comment in `notification_repo`.
const GREETING_TYPES: [&str; 4] = [
    "PATIENT_GREETING",
    "DOCTOR_GREETING",
    "BIRTHDAY_GREETING",
    "HOLIDAY_GREETING",
];

/// Decides whether a claimed `OPERATIONAL`/`ENGAGEMENT` row should actually
/// be pushed right now: category/greeting opt-out (and, for a religious
/// holiday, the separate `religious_greetings_enabled` opt-in), then an
/// atomic per-type daily frequency-cap claim, then quiet hours (only for
/// types the registry itself allows to be deferred — `quiet_hours_allowed`;
/// see that column's own seeded values for why T-15 reminders/MAINTENANCE
/// are exempt, satisfying requirement 13).
async fn notification_delivery_gate(
    state: &AppState,
    connection: &mut sqlx::PgConnection,
    row: &notification_repo::DueNotification,
) -> Result<DeliveryGate, crate::error::AppError> {
    if !is_still_eligible_for_dispatch(state, connection, row).await? {
        return Ok(DeliveryGate::SuppressPermanently(
            "Suppressed: no longer eligible at dispatch time",
        ));
    }
    if row.related_entity_type.as_deref() == Some("CAMPAIGN") {
        let status: Option<String> =
            sqlx::query_scalar("SELECT status::text FROM campaigns WHERE id=$1")
                .bind(row.related_entity_id)
                .fetch_optional(&mut *connection)
                .await?;
        match status.as_deref() {
            Some("PAUSED" | "SCHEDULED") => {
                return Ok(DeliveryGate::Defer(
                    Utc::now() + ChronoDuration::seconds(60),
                    "Deferred: campaign delivery paused",
                ))
            }
            Some("SENDING" | "COMPLETED") => {}
            _ => {
                return Ok(DeliveryGate::SuppressPermanently(
                    "Suppressed: campaign is no longer eligible",
                ))
            }
        }
    }
    let type_gate =
        notification_repo::notification_type_gate(&mut *connection, &row.notification_type).await?;
    if type_gate.category != "OPERATIONAL" && type_gate.category != "ENGAGEMENT" {
        return Ok(DeliveryGate::Allow);
    }
    let prefs =
        notification_repo::user_notification_preferences(&mut *connection, row.user_id).await?;
    let now = Utc::now();

    if GREETING_TYPES.contains(&row.notification_type.as_str()) {
        if !prefs.greeting_notifications_enabled {
            return Ok(DeliveryGate::SuppressPermanently(
                "Suppressed: greeting notifications disabled by user preference",
            ));
        }
        if row.notification_type == "HOLIDAY_GREETING" {
            // Religious-origin holidays (Good Friday, Easter Monday,
            // Christmas, Eid al-Fitr, Eid al-Adha) need a second, explicit
            // opt-in — applied identically to every user regardless of role,
            // name or any other signal, never inferred.
            let is_religious = match row.related_entity_id {
                Some(holiday_id) => {
                    notification_repo::holiday_is_religious(&mut *connection, holiday_id)
                        .await?
                        .unwrap_or(false)
                }
                None => false,
            };
            if is_religious && !prefs.religious_greetings_enabled {
                return Ok(DeliveryGate::SuppressPermanently(
                    "Suppressed: religious holiday greetings disabled by user preference",
                ));
            }
        }
    } else if type_gate.category == "OPERATIONAL" && !prefs.operational_enabled {
        return Ok(DeliveryGate::SuppressPermanently(
            "Suppressed: operational notifications disabled by user preference",
        ));
    } else if type_gate.category == "ENGAGEMENT" && !prefs.engagement_enabled {
        return Ok(DeliveryGate::SuppressPermanently(
            "Suppressed: engagement notifications disabled by user preference",
        ));
    }

    if type_gate.quiet_hours_allowed && prefs.quiet_hours_enabled {
        let local_now = now.with_timezone(&nairobi_offset());
        let local_time = local_now.time();
        let start = prefs.quiet_hours_start_local;
        let end = prefs.quiet_hours_end_local;
        let in_quiet_hours = if start <= end {
            local_time >= start && local_time < end
        } else {
            // Overnight range (e.g. 22:00 -> 07:00): "in quiet hours" means
            // on either side of midnight.
            local_time >= start || local_time < end
        };
        if in_quiet_hours {
            let local_today = local_now.date_naive();
            let end_date = if start > end && local_time >= start {
                local_today + chrono::Duration::days(1)
            } else {
                local_today
            };
            let end_utc = end_date
                .and_time(end)
                .and_local_timezone(nairobi_offset())
                .unwrap()
                .with_timezone(&Utc);
            return Ok(DeliveryGate::Defer(
                end_utc,
                "Deferred: recipient quiet hours in effect",
            ));
        }
    }

    // Claim only after quiet-hours evaluation. A deferred row must consume a
    // daily slot on the day it is actually eligible for delivery, rather than
    // reserving yesterday's slot while it waits overnight.
    if !row.frequency_claimed {
        let local_today = nairobi_local_date(now);
        let mut tx = sqlx::Connection::begin(&mut *connection).await?;
        if !notification_repo::renew_delivery_claim(&mut *tx, row.id, row.delivery_claim_token)
            .await?
        {
            return Ok(DeliveryGate::LostClaim);
        }
        let claimed = notification_repo::claim_frequency_slot(
            &mut *tx,
            row.user_id,
            &row.notification_type,
            local_today,
            type_gate.max_per_day,
        )
        .await?;
        if claimed {
            notification_repo::mark_frequency_claimed(&mut *tx, row.id, row.delivery_claim_token)
                .await?;
        }
        tx.commit().await?;
        if !claimed {
            return Ok(DeliveryGate::SuppressPermanently(
                "Suppressed: daily frequency limit reached",
            ));
        }
    }
    Ok(DeliveryGate::Allow)
}

const PUSH_MAX_ATTEMPTS: i32 = 5;
const PUSH_MAX_ROWS: usize = 200;
const PUSH_SEND_TIMEOUT: Duration = Duration::from_secs(45);
const PUSH_HEARTBEAT: Duration = Duration::from_secs(10);

enum DispatchResult {
    Outcome(PushOutcome),
    Blocked(DeliveryGate),
    TokenRevoked,
}

async fn token_still_enabled(
    connection: &mut sqlx::PgConnection,
    row: &notification_repo::DueNotification,
    device_id: Uuid,
    token: &str,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM notification_device_tokens WHERE user_id=$1 AND device_id=$2 AND push_token=$3 AND enabled)")
        .bind(row.user_id).bind(device_id).bind(token).fetch_one(connection).await
}

/// Provider IO owns no database transaction. Heartbeats renew the fenced
/// claim and recheck live controls; cancellation drops the provider future.
/// A remote acceptance before cancellation remains fundamentally ambiguous.
async fn send_with_lease(
    state: &AppState,
    connection: &mut sqlx::PgConnection,
    row: &notification_repo::DueNotification,
    device_id: Uuid,
    token: &str,
) -> Result<DispatchResult, crate::error::AppError> {
    // M8-G-13: the Doctor and Patient apps register different Android
    // notification channel sets, so the correct `channel_id` depends on
    // which app will actually receive this push — resolved from the
    // recipient's own role, never trusted from anywhere else.
    let recipient_role: String = sqlx::query_scalar("SELECT role::text FROM users WHERE id=$1")
        .bind(row.user_id)
        .fetch_one(&mut *connection)
        .await?;
    let options = PushOptions {
        notification_id: row.id.to_string(),
        notification_type: row.notification_type.clone(),
        related_entity_type: row.related_entity_type.clone(),
        related_entity_id: row.related_entity_id.map(|id| id.to_string()),
        collapse_key: row.collapse_key.clone(),
        ttl_seconds: row.ttl_seconds,
        recipient_role,
    };
    let send = state
        .notification_transport
        .send_push(token, &row.title, &row.body, &options);
    tokio::pin!(send);
    let deadline = tokio::time::sleep(PUSH_SEND_TIMEOUT);
    tokio::pin!(deadline);
    let mut heartbeat = tokio::time::interval(PUSH_HEARTBEAT);
    heartbeat.tick().await;
    loop {
        tokio::select! {
            biased;
            _ = heartbeat.tick() => {
                if !notification_repo::renew_delivery_claim(&mut *connection,row.id,row.delivery_claim_token).await? {
                    return Ok(DispatchResult::Blocked(DeliveryGate::LostClaim));
                }
                let gate = notification_delivery_gate(state,connection,row).await?;
                if !matches!(gate, DeliveryGate::Allow) { return Ok(DispatchResult::Blocked(gate)); }
                if !token_still_enabled(connection,row,device_id,token).await? { return Ok(DispatchResult::TokenRevoked); }
            }
            _ = &mut deadline => return Ok(DispatchResult::Outcome(PushOutcome::Retryable {
                reason: "Provider acknowledgment timed out; outcome is uncertain".into(),
            })),
            outcome = &mut send => return Ok(DispatchResult::Outcome(outcome)),
        }
    }
}

async fn finish_gate(
    connection: &mut sqlx::PgConnection,
    row: &notification_repo::DueNotification,
    gate: DeliveryGate,
    provider_started: bool,
) -> Result<(), sqlx::Error> {
    let exhausted_deferral = provider_started
        && row.delivery_attempts >= PUSH_MAX_ATTEMPTS
        && matches!(gate, DeliveryGate::Defer(_, _));
    let outcome_metric = match &gate {
        DeliveryGate::Defer(_, _) if exhausted_deferral => {
            "notification_push_permanent_failure_total"
        }
        DeliveryGate::SuppressPermanently(reason) if reason.contains("dispatch time") => {
            "notification_push_stale_suppressed_total"
        }
        DeliveryGate::SuppressPermanently(reason) if reason.contains("campaign") => {
            "notification_push_campaign_suppressed_total"
        }
        DeliveryGate::SuppressPermanently(_) => "notification_push_preference_suppressed_total",
        DeliveryGate::Defer(_, reason) if reason.contains("quiet hours") => {
            "notification_push_quiet_hours_deferred_total"
        }
        DeliveryGate::Defer(_, _) => "notification_push_campaign_deferred_total",
        _ => "notification_push_stale_completion_total",
    };
    let changed = match gate {
        DeliveryGate::SuppressPermanently(reason) => {
            notification_repo::mark_delivery_suppressed(
                connection,
                row.id,
                row.delivery_claim_token,
                reason,
            )
            .await?
        }
        DeliveryGate::Defer(_, _) if exhausted_deferral => {
            notification_repo::mark_delivery_permanent(
                connection,
                row.id,
                row.delivery_claim_token,
                "Delivery retry budget exhausted while deferring an uncertain provider call",
            )
            .await?
        }
        DeliveryGate::Defer(next, reason) => {
            notification_repo::mark_delivery_deferred(
                connection,
                row.id,
                row.delivery_claim_token,
                next,
                reason,
                provider_started,
            )
            .await?
        }
        DeliveryGate::LostClaim => false,
        DeliveryGate::Allow => return Ok(()),
    };
    if changed {
        metrics::counter!(outcome_metric).increment(1);
    } else {
        metrics::counter!("notification_push_stale_completion_total").increment(1);
    }
    Ok(())
}

async fn deliver_claimed(
    state: &AppState,
    claim: &mut notification_repo::DeliveryClaim,
) -> Result<(), crate::error::AppError> {
    let mut row = claim.row.clone();
    let connection = claim.connection();
    if !notification_repo::renew_delivery_claim(&mut *connection, row.id, row.delivery_claim_token)
        .await?
    {
        return Ok(());
    }
    if row.delivery_attempts > PUSH_MAX_ATTEMPTS {
        notification_repo::mark_delivery_permanent(
            connection,
            row.id,
            row.delivery_claim_token,
            "Delivery retry budget exhausted after abandoned claim; inspect uncertain outcome flag",
        )
        .await?;
        return Ok(());
    }
    let gate = notification_delivery_gate(state, connection, &row).await?;
    if !matches!(gate, DeliveryGate::Allow) {
        return Ok(finish_gate(connection, &row, gate, false).await?);
    }
    // The first gate has atomically reserved the frequency slot, if applicable.
    row.frequency_claimed = true;
    let tokens =
        notification_repo::device_tokens::find_enabled_for_user(&mut *connection, row.user_id)
            .await?;
    let mut sent = false;
    let mut retryable = tokens.is_empty();
    let mut provider_message_id = None;
    let mut uncertain = row.delivery_uncertain;
    let mut provider_started = false;
    for (device_id, token) in tokens {
        if !notification_repo::renew_delivery_claim(
            &mut *connection,
            row.id,
            row.delivery_claim_token,
        )
        .await?
        {
            return Ok(());
        }
        let gate = notification_delivery_gate(state, connection, &row).await?;
        if !matches!(gate, DeliveryGate::Allow) {
            if sent {
                break;
            }
            return Ok(finish_gate(connection, &row, gate, provider_started).await?);
        }
        if !token_still_enabled(connection, &row, device_id, &token).await? {
            continue;
        }
        if !notification_repo::begin_provider_send(
            &mut *connection,
            row.id,
            row.delivery_claim_token,
        )
        .await?
        {
            return Ok(());
        }
        provider_started = true;
        let result = send_with_lease(state, connection, &row, device_id, &token).await?;
        if !notification_repo::renew_delivery_claim(
            &mut *connection,
            row.id,
            row.delivery_claim_token,
        )
        .await?
        {
            metrics::counter!("notification_push_stale_completion_total").increment(1);
            return Ok(());
        }
        match result {
            DispatchResult::Blocked(gate) => {
                if sent && !matches!(gate, DeliveryGate::LostClaim) {
                    break;
                }
                return Ok(finish_gate(connection, &row, gate, provider_started).await?);
            }
            DispatchResult::TokenRevoked => {
                uncertain = true;
            }
            DispatchResult::Outcome(PushOutcome::SentToProvider {
                provider_message_id: id,
            }) => {
                sent = true;
                metrics::counter!("notification_push_sent_total").increment(1);
                provider_message_id = provider_message_id.or(id);
            }
            DispatchResult::Outcome(PushOutcome::Permanent { .. }) => {
                let mut tx = sqlx::Connection::begin(&mut *connection).await?;
                // The row lock from renewal fences BOTH token invalidation and
                // uncertainty accounting from cancellation/ownership changes.
                if !notification_repo::renew_delivery_claim(
                    &mut *tx,
                    row.id,
                    row.delivery_claim_token,
                )
                .await?
                {
                    return Ok(());
                }
                notification_repo::record_definite_rejection(
                    &mut *tx,
                    row.id,
                    row.delivery_claim_token,
                    uncertain,
                )
                .await?;
                let disabled = notification_repo::device_tokens::disable_device(
                    &mut *tx,
                    row.user_id,
                    Some(device_id),
                    Some(&token),
                )
                .await?;
                tx.commit().await?;
                metrics::counter!("notification_push_invalid_token_total").increment(disabled);
            }
            DispatchResult::Outcome(PushOutcome::Retryable { .. }) => {
                retryable = true;
                uncertain = true;
            }
            DispatchResult::Outcome(PushOutcome::NoProviderConfigured) => {
                notification_repo::record_definite_rejection(
                    &mut *connection,
                    row.id,
                    row.delivery_claim_token,
                    uncertain,
                )
                .await?;
                retryable = true;
            }
        }
    }
    let outcome = if sent {
        "sent_to_provider"
    } else if retryable && row.delivery_attempts < PUSH_MAX_ATTEMPTS {
        "retry"
    } else {
        "permanent_failure"
    };
    let changed = if sent {
        notification_repo::mark_delivery_sent(
            connection,
            row.id,
            row.delivery_claim_token,
            provider_message_id.as_deref(),
        )
        .await?
    } else if retryable && row.delivery_attempts < PUSH_MAX_ATTEMPTS {
        let exponent = (row.delivery_attempts.max(1) - 1).min(6) as u32;
        let delay = (30_i64 * 2_i64.pow(exponent)).min(3600);
        notification_repo::mark_delivery_retry(
            connection,
            row.id,
            row.delivery_claim_token,
            "FCM provider delivery was temporarily unavailable; inspect uncertain outcome flag",
            Utc::now() + ChronoDuration::seconds(delay),
        )
        .await?
    } else {
        notification_repo::mark_delivery_permanent(
            connection,
            row.id,
            row.delivery_claim_token,
            if retryable {
                "FCM delivery retry budget exhausted"
            } else {
                "No enabled device token accepted the notification"
            },
        )
        .await?
    };
    if changed {
        match outcome {
            "sent_to_provider" => {
                metrics::counter!("notification_push_acknowledged_rows_total").increment(1)
            }
            "retry" => metrics::counter!("notification_push_retry_total").increment(1),
            _ => metrics::counter!("notification_push_permanent_failure_total").increment(1),
        }
    } else {
        metrics::counter!("notification_push_stale_completion_total").increment(1);
    }
    Ok(())
}

/// Existing outbox, system-role pool and provider abstraction. Only one row is
/// leased per worker at a time; no row waits in a preclaimed sequential batch.
/// SENT_TO_PROVIDER means provider acceptance, never confirmed device delivery.
pub async fn run_push_delivery_pass(state: &AppState) -> Result<u64, crate::error::AppError> {
    if !state.notification_transport.is_configured() {
        return Ok(0);
    }
    let started = std::time::Instant::now();
    let mut claimed_total = 0;
    for _ in 0..PUSH_MAX_ROWS {
        if started.elapsed() > Duration::from_secs(60) {
            break;
        }
        let Some(mut claim) = notification_repo::claim_next_notification(&state.system_db).await?
        else {
            break;
        };
        claimed_total += 1;
        deliver_claimed(state, &mut claim).await?;
        claim.release().await?;
    }
    let stats = notification_repo::delivery_queue_stats(&state.system_db).await?;
    for (status, count) in [
        ("pending", stats.pending),
        ("processing", stats.processing),
        ("retrying", stats.retrying),
        ("deferred", stats.deferred),
        ("suppressed", stats.suppressed),
        ("sent_to_provider", stats.sent_to_provider),
        ("permanently_failed", stats.permanently_failed),
        ("uncertain", stats.uncertain),
        ("confirmed_delivered", stats.confirmed_delivered),
        ("legacy_read", stats.legacy_read),
    ] {
        metrics::gauge!("notification_outbox_rows","status"=>status).set(count as f64);
    }
    Ok(claimed_total)
}

/// Expires empty-room grace periods without completing the appointment or
/// touching payment state. The database deadline survives process restarts and
/// `SKIP LOCKED` makes concurrent backend instances safe.
pub async fn run_consultation_presence_sweep(
    state: &AppState,
) -> Result<u64, crate::error::AppError> {
    super::consultation_presence_service::expire_empty_rooms(state).await
}

/// Upper bound per transaction. A backlog is drained over successive ticks
/// rather than in one long transaction that would hold many row locks.
const HOLD_SWEEP_BATCH: i64 = 100;

/// INV-BOOK-03: expires every abandoned payment hold whose deadline has
/// passed, independent of whether anyone tries to book the same slot. Safe to
/// run on several backend instances at once (`FOR UPDATE SKIP LOCKED`) and
/// idempotent (status compare-and-set), so a crashed or duplicated tick cannot
/// double-expire or resurrect a hold. Returns the number expired.
pub async fn run_hold_expiry_sweep(state: &AppState) -> Result<u64, crate::error::AppError> {
    let ctx = crate::auth_context::AuthContext::system();
    let mut total = 0u64;
    // Bounded number of batches per tick so one tick can never run unboundedly.
    for _ in 0..10 {
        let mut tx = state
            .begin_authed_as(&ctx, crate::repository::DbRole::PaymentWorker)
            .await?;
        let expired = crate::repository::appointment_repo::expire_due_holds_batch(
            &mut tx,
            state.config.payment_reservation_window_secs,
            HOLD_SWEEP_BATCH,
        )
        .await?;
        tx.commit().await?;
        total += expired.len() as u64;
        if (expired.len() as i64) < HOLD_SWEEP_BATCH {
            break;
        }
    }
    if total > 0 {
        metrics::counter!("payment_holds_expired_total").increment(total);
        tracing::info!(expired = total, "expired abandoned payment holds");
    }
    Ok(total)
}

/// Retries physical deletion after the expiry transaction has removed all
/// clinical metadata. Access is denied immediately by the EXPIRED appointment
/// state; this job only closes the storage half of the same invariant.
pub async fn run_expired_upload_cleanup(state: &AppState) -> Result<u64, crate::error::AppError> {
    const BATCH: i64 = 50;
    let ctx = crate::auth_context::AuthContext::system();
    let mut completed = 0u64;
    for _ in 0..4 {
        let mut claim_tx = state
            .begin_authed_as(&ctx, crate::repository::DbRole::SystemWorker)
            .await?;
        let jobs =
            crate::repository::appointment_repo::claim_expiry_cleanup_jobs(&mut *claim_tx, BATCH)
                .await?;
        claim_tx.commit().await?;
        if jobs.is_empty() {
            break;
        }
        for job in jobs {
            let mut failure = None;
            for key in &job.object_keys {
                let expected_prefix = format!("intake/{}/", job.appointment_id);
                if !key.starts_with(&expected_prefix) {
                    failure = Some(
                        "Refusing to delete an object outside the appointment namespace.".into(),
                    );
                    break;
                }
                let referenced_elsewhere: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM prescription_intake_documents WHERE object_key=$1)",
                )
                .bind(key)
                .fetch_one(&state.system_db)
                .await?;
                if referenced_elsewhere {
                    failure = Some(
                        "Refusing to delete an object still referenced by another intake record."
                            .into(),
                    );
                    break;
                }
                if let Err(error) = DocumentStorageService::delete_strict(state, key).await {
                    failure = Some(error.to_string());
                    break;
                }
            }
            let mut update_tx = state
                .begin_authed_as(&ctx, crate::repository::DbRole::SystemWorker)
                .await?;
            if let Some(error) = failure {
                let exhausted = crate::repository::appointment_repo::mark_expiry_cleanup_retry(
                    &mut *update_tx,
                    job.appointment_id,
                    &error,
                )
                .await?;
                if exhausted {
                    metrics::counter!("expired_clinical_cleanup_exhausted_total").increment(1);
                    tracing::error!(
                        appointment_id = %job.appointment_id,
                        attempts = job.attempts,
                        error = %error,
                        "expired clinical-record cleanup exhausted its retry budget; operator intervention required"
                    );
                }
            } else {
                crate::repository::appointment_repo::mark_expiry_cleanup_complete(
                    &mut *update_tx,
                    job.appointment_id,
                )
                .await?;
                completed += 1;
            }
            update_tx.commit().await?;
        }
        if completed < BATCH as u64 {
            break;
        }
    }
    Ok(completed)
}

#[derive(Debug, sqlx::FromRow)]
struct HoldNotificationTarget {
    appointment_id: Uuid,
    user_id: UserId,
    hold_expires_at: DateTime<Utc>,
    // Milestone 8 Phase 6 (2026-11-02): this pass runs every 60s and a hold
    // lives up to 10 minutes, so the same still-pending appointment was
    // re-selected on every tick -- confirmed live, 4.6M cumulative
    // notification-template-lookup calls traced back to exactly this
    // (docs/MILESTONE_8_PHASE5_EFFICIENCY_INVESTIGATION.md's
    // notification_templates finding). The unique `dedup_key` already made
    // re-INSERTing safe (insert_if_new catches 23505 and no-ops), but did
    // nothing to stop the wasted user_language()/render() work and the
    // INSERT attempt itself on every tick before that catch. These two
    // columns are computed once per row, index-backed
    // (notifications_dedup_key_unique), so the Rust loop below can skip
    // work it already knows is redundant instead of discovering that after
    // paying for it. Both default back to `true` (do the work) the instant
    // the underlying notification row is ever deleted or a prior attempt
    // failed before insertion -- this is a read of current persisted state
    // each tick, not a separate "already tried" flag, so genuine failures
    // still retry on the very next tick with no special-casing needed.
    needs_hold_created_notification: bool,
    needs_one_minute_notification: bool,
}

/// Creates non-clinical Patient inbox notifications. The unique dedup key is
/// the event identity, so retries and multiple scheduler instances are safe.
pub async fn run_hold_notification_pass(state: &AppState) -> Result<(), sqlx::Error> {
    let now = Utc::now();
    let active: Vec<HoldNotificationTarget> = sqlx::query_as(
        "SELECT a.id AS appointment_id, COALESCE(p.user_id, g.user_id) AS user_id, \
                COALESCE(a.hold_expires_at, a.created_at + make_interval(secs => 600)) AS hold_expires_at, \
                NOT EXISTS (SELECT 1 FROM notifications n WHERE n.dedup_key = 'booking-hold-created:' || a.id::text) AS needs_hold_created_notification, \
                NOT EXISTS (SELECT 1 FROM notifications n WHERE n.dedup_key = 'booking-hold-one-minute:' || a.id::text) AS needs_one_minute_notification \
           FROM appointments a \
           JOIN patient_profiles p ON p.id=a.patient_id \
           LEFT JOIN patient_profiles g ON g.id=p.guardian_patient_id \
          WHERE a.status='PENDING_PAYMENT' AND a.hold_expires_at > clock_timestamp() \
            AND COALESCE(p.user_id, g.user_id) IS NOT NULL",
    )
    .fetch_all(&state.system_db)
    .await?;
    for target in active {
        if target.needs_hold_created_notification {
            insert_hold_created_notification(state, &target).await?;
        }
        if target.needs_one_minute_notification
            && target.hold_expires_at <= now + chrono::Duration::minutes(1)
        {
            let one_minute_key = format!("booking-hold-one-minute:{}", target.appointment_id);
            let language = crate::services::NotificationService::user_language(
                &state.system_db,
                target.user_id,
            )
            .await?;
            let (title, body) = crate::services::NotificationService::render(
                &state.system_db,
                "booking_hold_one_minute",
                &language,
                &[],
                "One minute left to complete payment",
                "Your consultation reservation expires in about one minute. Complete payment now or choose a new slot after expiry.",
            )
            .await?;
            notification_repo::insert_if_new(
                &state.system_db,
                Uuid::new_v4(),
                target.user_id,
                "BOOKING_HOLD_ONE_MINUTE",
                &title,
                &body,
                Some("APPOINTMENT"),
                Some(target.appointment_id),
                Some(target.hold_expires_at - chrono::Duration::minutes(1)),
                &one_minute_key,
                None,
            )
            .await?;
        }
    }

    // Milestone 8 Phase 6: this loop's source rows never go away --
    // `appointment_expiry_cleanup_jobs` rows are updated in place
    // (storage-cleanup bookkeeping in appointment_repo.rs), never deleted,
    // and `a.status='EXPIRED'` is a terminal state that never reverts. Live
    // data at the time this was found: 16,902 already-expired appointments,
    // ALL 16,902 already carrying their BOOKING_HOLD_EXPIRED notification --
    // every one was being re-selected and re-rendered on every single tick,
    // forever, with the wasted volume growing unbounded as more
    // appointments expire over the product's lifetime (unlike the
    // active-holds loop above, which is at least bounded by the 10-minute
    // hold window). Unlike that loop, nothing else in this function needs
    // the full historical list, so filtering directly in the WHERE clause
    // (same index-backed NOT EXISTS) is correct and sufficient here.
    let expired: Vec<(Uuid, UserId)> = sqlx::query_as(
        "SELECT j.appointment_id, COALESCE(p.user_id, g.user_id) \
           FROM appointment_expiry_cleanup_jobs j \
           JOIN appointments a ON a.id=j.appointment_id \
           JOIN patient_profiles p ON p.id=a.patient_id \
           LEFT JOIN patient_profiles g ON g.id=p.guardian_patient_id \
          WHERE a.status='EXPIRED' AND COALESCE(p.user_id, g.user_id) IS NOT NULL \
            AND NOT EXISTS (SELECT 1 FROM notifications n WHERE n.dedup_key = 'booking-hold-expired:' || a.id::text)",
    )
    .fetch_all(&state.system_db)
    .await?;
    for (appointment_id, user_id) in expired {
        let key = format!("booking-hold-expired:{appointment_id}");
        let language =
            crate::services::NotificationService::user_language(&state.system_db, user_id).await?;
        let (title, body) = crate::services::NotificationService::render(
            &state.system_db,
            "booking_hold_expired",
            &language,
            &[],
            "Consultation reservation expired",
            "This unpaid consultation reservation expired. Please make a new booking and upload your records again.",
        )
        .await?;
        notification_repo::insert_if_new(
            &state.system_db,
            Uuid::new_v4(),
            user_id,
            "BOOKING_HOLD_EXPIRED",
            &title,
            &body,
            Some("APPOINTMENT"),
            Some(appointment_id),
            None,
            &key,
            None,
        )
        .await?;
    }
    Ok(())
}

/// Best-effort immediate creation after a booking commits. The periodic pass
/// remains the durable reconciliation path if this post-commit attempt is
/// interrupted.
pub async fn ensure_hold_created_notification(
    state: &AppState,
    appointment_id: Uuid,
) -> Result<(), sqlx::Error> {
    let target: Option<HoldNotificationTarget> = sqlx::query_as(
        "SELECT a.id AS appointment_id, COALESCE(p.user_id, g.user_id) AS user_id, \
                COALESCE(a.hold_expires_at, a.created_at + make_interval(secs => 600)) AS hold_expires_at \
           FROM appointments a \
           JOIN patient_profiles p ON p.id=a.patient_id \
           LEFT JOIN patient_profiles g ON g.id=p.guardian_patient_id \
          WHERE a.id=$1 AND a.status='PENDING_PAYMENT' \
            AND COALESCE(p.user_id, g.user_id) IS NOT NULL",
    )
    .bind(appointment_id)
    .fetch_optional(&state.system_db)
    .await?;
    if let Some(target) = target {
        insert_hold_created_notification(state, &target).await?;
    }
    Ok(())
}

async fn insert_hold_created_notification(
    state: &AppState,
    target: &HoldNotificationTarget,
) -> Result<(), sqlx::Error> {
    let created_key = format!("booking-hold-created:{}", target.appointment_id);
    let language =
        crate::services::NotificationService::user_language(&state.system_db, target.user_id)
            .await?;
    let (title, body) = crate::services::NotificationService::render(
        &state.system_db,
        "booking_hold_created",
        &language,
        &[],
        "Consultation slot reserved",
        "Your consultation slot is reserved for 10 minutes. Complete payment before the timer ends.",
    )
    .await?;
    notification_repo::insert_if_new(
        &state.system_db,
        Uuid::new_v4(),
        target.user_id,
        "BOOKING_HOLD_CREATED",
        &title,
        &body,
        Some("APPOINTMENT"),
        Some(target.appointment_id),
        None,
        &created_key,
        None,
    )
    .await?;
    Ok(())
}

#[derive(Debug, sqlx::FromRow)]
struct VerifiedDoctor {
    profile_id: Uuid,
    user_id: UserId,
}

/// For each verified, active Doctor whose Kenya-local time has reached or
/// passed the configured daily-summary time today, and who doesn't already
/// have one for today (the dedup key, not a separate pre-check, is the real
/// guarantee — this query is just to avoid pointless re-attempts every
/// tick), creates exactly one `DAILY_CONSULTATION_SUMMARY` notification
/// counting today's (Kenya-local calendar day) scheduled/eligible — i.e. not
/// cancelled — appointments for that Doctor.
pub async fn run_daily_summary_pass(
    state: &AppState,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    let configured = parse_hhmm(&state.config.doctor_daily_summary_time_local).unwrap_or((7, 0));
    let local_now = now.with_timezone(&nairobi_offset());
    let today = nairobi_local_date(now);

    if (local_now.hour(), local_now.minute()) < configured {
        return Ok(());
    }

    let doctors: Vec<VerifiedDoctor> = sqlx::query_as(
        "SELECT d.id AS profile_id, d.user_id FROM doctor_profiles d \
         JOIN users u ON u.id = d.user_id \
         WHERE d.professional_verification_status = 'VERIFIED' AND u.is_active = TRUE AND u.deleted_at IS NULL",
    )
    .fetch_all(&state.system_db)
    .await?;

    // Kenya-local calendar day, expressed back in UTC instants, for the
    // `start_time` range comparison below (`doctor_schedule_slots.start_time`
    // is stored in UTC like every other timestamp in this codebase).
    let day_start_utc = today
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_local_timezone(nairobi_offset())
        .unwrap()
        .with_timezone(&Utc);
    let day_end_utc = day_start_utc + chrono::Duration::days(1);

    for doctor in doctors {
        let dedup_key = format!("daily-consultation-summary:{}:{today}", doctor.profile_id);
        if notification_repo::exists_with_dedup_key(&state.system_db, &dedup_key).await? {
            continue;
        }

        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM appointments a JOIN doctor_schedule_slots s ON s.id = a.slot_id \
             WHERE a.doctor_id = $1 AND s.start_time >= $2 AND s.start_time < $3 \
             AND a.status NOT IN ('CANCELLED', 'EXPIRED')",
        )
        .bind(doctor.profile_id)
        .bind(day_start_utc)
        .bind(day_end_utc)
        .fetch_one(&state.system_db)
        .await?;

        let fallback_body = if count == 0 {
            "You have no consultations scheduled today.".to_string()
        } else if count == 1 {
            "You have 1 consultation scheduled today.".to_string()
        } else {
            format!("You have {count} consultations scheduled today.")
        };
        let language =
            crate::services::NotificationService::user_language(&state.system_db, doctor.user_id)
                .await?;
        let count_str = count.to_string();
        let (title, body) = crate::services::NotificationService::render(
            &state.system_db,
            "daily_consultation_summary",
            &language,
            &[("count", &count_str)],
            "Today's consultations",
            &fallback_body,
        )
        .await?;

        notification_repo::insert_if_new(
            &state.system_db,
            Uuid::new_v4(),
            doctor.user_id,
            "DAILY_CONSULTATION_SUMMARY",
            &title,
            &body,
            Some("DOCTOR_PROFILE"),
            Some(doctor.profile_id),
            None,
            &dedup_key,
            Some(serde_json::json!({ "count": count, "local_date": today.to_string() })),
        )
        .await?;
    }
    Ok(())
}

#[derive(Debug, sqlx::FromRow)]
struct DueConsultation {
    appointment_id: Uuid,
    doctor_user_id: UserId,
    doctor_language: String,
    /// The patient-side recipient: the patient's own `user_id`, or their
    /// guardian's when this appointment is for a dependent profile (mirrors
    /// `run_hold_notification_pass`'s identical `COALESCE(p.user_id,
    /// g.user_id)` pattern). `None` only in the pathological case where
    /// neither resolves to a real account — never skips the doctor's own
    /// reminder for that.
    patient_user_id: Option<UserId>,
    patient_language: Option<String>,
    start_time: DateTime<Utc>,
}

/// Creates a `CONSULTATION_REMINDER_15M` for every eligible (`CONFIRMED`/
/// `WAITING`, not yet started) appointment whose reminder window has opened
/// (`start_time - 15m <= now < start_time`) and doesn't already have one —
/// including an appointment booked with under 15 minutes' notice, for which
/// the window opens immediately rather than being scheduled in the past.
/// Notifies both the Doctor and the Patient (or their guardian, for a
/// dependent's appointment) — this is the sole T-15 reminder mechanism for
/// either role; no client app schedules its own local equivalent.
pub async fn run_t15_reminder_pass(
    state: &AppState,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    let window_opens_before = now + chrono::Duration::minutes(15);
    let due: Vec<DueConsultation> = sqlx::query_as(
        "SELECT a.id AS appointment_id, d.user_id AS doctor_user_id, du.preferred_language AS doctor_language, \
                COALESCE(p.user_id, g.user_id) AS patient_user_id, \
                COALESCE(pu.preferred_language, gu.preferred_language) AS patient_language, s.start_time \
         FROM appointments a \
         JOIN doctor_schedule_slots s ON s.id = a.slot_id \
         JOIN doctor_profiles d ON d.id = a.doctor_id \
         JOIN users du ON du.id = d.user_id \
         JOIN patient_profiles p ON p.id = a.patient_id \
         LEFT JOIN patient_profiles g ON g.id = p.guardian_patient_id \
         LEFT JOIN users pu ON pu.id = p.user_id \
         LEFT JOIN users gu ON gu.id = g.user_id \
         WHERE a.status IN ('CONFIRMED', 'WAITING') \
           AND s.start_time > $1 AND s.start_time <= $2",
    )
    .bind(now)
    .bind(window_opens_before)
    .fetch_all(&state.system_db)
    .await?;

    for c in due {
        let scheduled_for = Some(c.start_time - chrono::Duration::minutes(15));
        let doctor_key = format!(
            "consultation-reminder-15m:{}:{}:doctor",
            c.appointment_id,
            c.start_time.to_rfc3339()
        );
        const FALLBACK_TITLE: &str = "Consultation starting soon";
        const FALLBACK_BODY: &str = "Your consultation starts in 15 minutes.";
        let (title, body) = crate::services::NotificationService::render(
            &state.system_db,
            "consultation_reminder_15m",
            &c.doctor_language,
            &[],
            FALLBACK_TITLE,
            FALLBACK_BODY,
        )
        .await?;
        notification_repo::insert_if_new_with_ttl(
            &state.system_db,
            Uuid::new_v4(),
            c.doctor_user_id,
            "CONSULTATION_REMINDER_15M",
            &title,
            &body,
            Some("APPOINTMENT"),
            Some(c.appointment_id),
            scheduled_for,
            &doctor_key,
            None,
            Some(604_800),
        )
        .await?;

        if let Some(patient_user_id) = c.patient_user_id {
            let patient_key = format!(
                "consultation-reminder-15m:{}:{}:patient",
                c.appointment_id,
                c.start_time.to_rfc3339()
            );
            let patient_language = c.patient_language.as_deref().unwrap_or("en");
            let (title, body) = crate::services::NotificationService::render(
                &state.system_db,
                "consultation_reminder_15m",
                patient_language,
                &[],
                FALLBACK_TITLE,
                FALLBACK_BODY,
            )
            .await?;
            notification_repo::insert_if_new(
                &state.system_db,
                Uuid::new_v4(),
                patient_user_id,
                "CONSULTATION_REMINDER_15M",
                &title,
                &body,
                Some("APPOINTMENT"),
                Some(c.appointment_id),
                scheduled_for,
                &patient_key,
                None,
            )
            .await?;
        }
    }
    Ok(())
}

/// Creates a `CONSULTATION_READY` the moment an eligible (`CONFIRMED`,
/// video — the only modality the Agora join window applies to; a chat
/// consultation's conversation is already available from the moment
/// payment confirms it, with no window to wait for) appointment's own
/// authorized join window opens — the SAME window
/// `ConsultationService::mint_agora_token` already enforces
/// (`AGORA_TOKEN_WINDOW_BEFORE_MINS`/`_AFTER_MINS`), never a new clinical
/// state. Notifies both the Doctor and the Patient (or their guardian).
/// `ttl_seconds` is bounded to what remains of that same window, so a
/// delayed push never invites joining a window that has already closed.
pub async fn run_consultation_ready_pass(
    state: &AppState,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    let before_mins = state.config.agora_token_window_before_mins as i32;
    let after_mins = state.config.agora_token_window_after_mins as i32;
    let due: Vec<(Uuid, DateTime<Utc>)> = sqlx::query_as(
        "SELECT a.id, s.end_time \
           FROM appointments a \
           JOIN doctor_schedule_slots s ON s.id = a.slot_id \
          WHERE a.status = 'CONFIRMED' AND a.modality = 'VIDEO' \
            AND clock_timestamp() >= s.start_time - make_interval(mins => $1) \
            AND clock_timestamp() <= s.end_time + make_interval(mins => $2)",
    )
    .bind(before_mins)
    .bind(after_mins)
    .fetch_all(&state.system_db)
    .await?;

    for (appointment_id, end_time) in due {
        let window_end = end_time + chrono::Duration::minutes(after_mins.into());
        let ttl_seconds = (window_end - now).num_seconds().clamp(1, 3600) as i32;
        let mut conn = state.system_db.acquire().await?;
        crate::services::NotificationService::notify_consultation_ready(
            &mut conn,
            appointment_id,
            ttl_seconds,
        )
        .await?;
    }
    Ok(())
}

#[derive(Debug, sqlx::FromRow)]
struct GreetingRecipient {
    user_id: UserId,
    language: String,
}

/// Milestone 4 good-morning engagement greeting — Doctor and Patient, once
/// per Nairobi-local ISO week (Monday, at/after
/// `AppConfig::engagement_greeting_time_local`), never more often
/// ("avoid repetitive daily greetings"). Gated at delivery time by the
/// separate `greeting_notifications_enabled` preference (role-aware
/// default: Doctors on, Patients off — see that preference's own doc
/// comment), not here — this pass always creates the inbox row; only the
/// push decision is gated later.
pub async fn run_greeting_pass(state: &AppState, now: DateTime<Utc>) -> Result<(), sqlx::Error> {
    let configured = parse_hhmm(&state.config.engagement_greeting_time_local).unwrap_or((8, 0));
    let local_now = now.with_timezone(&nairobi_offset());
    if local_now.weekday() != chrono::Weekday::Mon
        || (local_now.hour(), local_now.minute()) < configured
    {
        return Ok(());
    }
    let iso_week = local_now.date_naive().iso_week();
    let week_key = format!("{}-W{:02}", iso_week.year(), iso_week.week());

    // Milestone 8 Phase 6 (2026-11-02): this pass is only meant to greet
    // once per ISO week, but its day/time gate above ("is it Monday, past
    // the configured hour") stays open for the rest of that day -- nothing
    // previously stopped it from re-fetching every verified Doctor/
    // non-placeholder Patient and re-rendering their greeting on every
    // single 60-second tick for the remainder of Monday, every week,
    // forever. Only the terminal `insert_if_new_with_ttl` dedup key
    // (`greeting:{user_id}:{week_key}`) stopped a duplicate ROW from being
    // written -- it did nothing to stop the wasted query/render work before
    // that. Caught live: this was found still firing at ~10,000 calls/tick
    // while investigating an unrelated scheduler pass, on a real Monday.
    // Same fix shape as run_hold_notification_pass immediately above: an
    // index-backed (notifications_dedup_key_unique) NOT EXISTS filter
    // applied directly in each SELECT, since -- unlike that function -- no
    // other concern in this pass needs the already-greeted rows once the
    // week's greeting exists.
    //
    // GREETING_PASS_BATCH bounds the *first* eligible tick's own burst too:
    // measured live at 3,513ms per 1,000 recipients (10,000 recipients ->
    // 35.1s) once the NOT EXISTS fix above was in place. Every other
    // scheduler pass (hold notifications, hold-expiry sweep, T-15 reminders)
    // runs sequentially AFTER this one returns, in the same tokio task --
    // yielding partway through would not help them run any sooner, since
    // this function would still have to finish before control returns to
    // that sequential chain either way. A LIMIT here is what actually fixes
    // it: it bounds this tick's own share of the 60s budget, and the same
    // NOT EXISTS filter above means the remaining backlog is picked up
    // automatically on the next tick (and the one after, etc.) -- a 10,000
    // backlog clears over roughly ten ticks (~10 minutes) of small, bounded
    // bursts instead of one 35-second blocking one. No new state/cursor is
    // needed: "not yet NOT-EXISTS-excluded" already IS the resume position.
    const GREETING_PASS_BATCH: i64 = 1000;
    let doctors: Vec<GreetingRecipient> = sqlx::query_as(
        "SELECT d.user_id, u.preferred_language AS language FROM doctor_profiles d \
         JOIN users u ON u.id = d.user_id \
         WHERE d.professional_verification_status = 'VERIFIED' \
           AND u.is_active = TRUE AND u.deleted_at IS NULL \
           AND NOT EXISTS (SELECT 1 FROM notifications n WHERE n.dedup_key = 'greeting:' || d.user_id::text || ':' || $1) \
         LIMIT $2",
    )
    .bind(&week_key)
    .bind(GREETING_PASS_BATCH)
    .fetch_all(&state.system_db)
    .await?;
    for doctor in doctors {
        let (title, body) = crate::services::NotificationService::render(
            &state.system_db,
            "doctor_greeting",
            &doctor.language,
            &[],
            "Good morning, Doctor",
            "Good morning! Wishing you a productive week ahead with AfyaMkononi.",
        )
        .await?;
        notification_repo::insert_if_new_with_ttl(
            &state.system_db,
            Uuid::new_v4(),
            doctor.user_id,
            "DOCTOR_GREETING",
            &title,
            &body,
            None,
            None,
            None,
            &format!("greeting:{}:{week_key}", doctor.user_id),
            None,
            Some(86_400),
        )
        .await?;
    }

    // Self-profile, non-placeholder patients only — a dependent has no
    // login of their own to greet, and a still-placeholder profile belongs
    // to an account that never finished even the basic onboarding step.
    let patients: Vec<GreetingRecipient> = sqlx::query_as(
        "SELECT p.user_id, u.preferred_language AS language FROM patient_profiles p \
         JOIN users u ON u.id = p.user_id \
         WHERE p.user_id IS NOT NULL AND u.is_active = TRUE AND u.deleted_at IS NULL \
           AND NOT (p.full_name = 'Patient User' AND p.date_of_birth = DATE '1995-01-01' \
                    AND p.gender = 'OTHER') \
           AND NOT EXISTS (SELECT 1 FROM notifications n WHERE n.dedup_key = 'greeting:' || p.user_id::text || ':' || $1) \
         LIMIT $2",
    )
    .bind(&week_key)
    .bind(GREETING_PASS_BATCH)
    .fetch_all(&state.system_db)
    .await?;
    for patient in patients {
        let (title, body) = crate::services::NotificationService::render(
            &state.system_db,
            "patient_greeting",
            &patient.language,
            &[],
            "Good morning",
            "Good morning! Wishing you a healthy week ahead with AfyaMkononi.",
        )
        .await?;
        notification_repo::insert_if_new_with_ttl(
            &state.system_db,
            Uuid::new_v4(),
            patient.user_id,
            "PATIENT_GREETING",
            &title,
            &body,
            None,
            None,
            None,
            &format!("greeting:{}:{week_key}", patient.user_id),
            None,
            Some(86_400),
        )
        .await?;
    }
    Ok(())
}

/// Milestone 4 `HEALTH_EDUCATION` — a once-per-Nairobi-local-ISO-week,
/// Patient-only nudge pointing at the existing Articles feed. Deliberately
/// generic, navigational copy only (never a specific authored health claim
/// this backend would be originating itself) — the one feature in this
/// milestone that could otherwise risk fabricating medical content; see
/// `run_new_article_pass` below for the event-driven companion that DOES
/// reference a specific, genuinely-published, Doctor-authored article.
pub async fn run_health_education_pass(
    state: &AppState,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    let configured = parse_hhmm(&state.config.engagement_greeting_time_local).unwrap_or((8, 0));
    let local_now = now.with_timezone(&nairobi_offset());
    if local_now.weekday() != chrono::Weekday::Mon
        || (local_now.hour(), local_now.minute()) < configured
    {
        return Ok(());
    }
    let iso_week = local_now.date_naive().iso_week();
    let week_key = format!("{}-W{:02}", iso_week.year(), iso_week.week());

    // Milestone 8 Phase 6: same bug, same fix shape as run_greeting_pass
    // immediately above (this pass has the identical "Monday, past a
    // configured hour" gate that stays open all day) -- an index-backed
    // NOT EXISTS against this week's dedup key.
    let patients: Vec<GreetingRecipient> = sqlx::query_as(
        "SELECT p.user_id, u.preferred_language AS language FROM patient_profiles p \
         JOIN users u ON u.id = p.user_id \
         WHERE p.user_id IS NOT NULL AND u.is_active = TRUE AND u.deleted_at IS NULL \
           AND NOT (p.full_name = 'Patient User' AND p.date_of_birth = DATE '1995-01-01' \
                    AND p.gender = 'OTHER') \
           AND NOT EXISTS (SELECT 1 FROM notifications n WHERE n.dedup_key = 'health-education:' || p.user_id::text || ':' || $1)",
    )
    .bind(&week_key)
    .fetch_all(&state.system_db)
    .await?;
    for patient in patients {
        let (title, body) = crate::services::NotificationService::render(
            &state.system_db,
            "health_education",
            &patient.language,
            &[],
            "This week's health tips",
            "New health tips and educational articles are available in AfyaMkononi. Open the app to read them.",
        )
        .await?;
        notification_repo::insert_if_new_with_ttl(
            &state.system_db,
            Uuid::new_v4(),
            patient.user_id,
            "HEALTH_EDUCATION",
            &title,
            &body,
            None,
            None,
            None,
            &format!("health-education:{}:{week_key}", patient.user_id),
            None,
            Some(604_800),
        )
        .await?;
    }
    Ok(())
}

/// Milestone 4 `BIRTHDAY_GREETING` — Patient-only, once per Nairobi-local
/// calendar year, on the Nairobi-local calendar day whose month/day matches
/// `patient_profiles.date_of_birth` exactly (strict equality, never a
/// `<=` catch-up range, so a scheduler outage never sends this late — the
/// same reasoning `kenyan_holidays` lookups use). Routes to the guardian's
/// own account for a dependent profile, exactly like every other
/// guardian-routed notification in this codebase. Excludes the OTP-login
/// placeholder profile shape — a birthdate of `1995-01-01` there is a
/// fabricated default, never a genuinely provided one.
pub async fn run_birthday_pass(state: &AppState, now: DateTime<Utc>) -> Result<(), sqlx::Error> {
    let today = nairobi_local_date(now);
    #[derive(Debug, sqlx::FromRow)]
    struct BirthdayRecipient {
        patient_id: Uuid,
        user_id: UserId,
        language: String,
    }
    let recipients: Vec<BirthdayRecipient> = sqlx::query_as(
        "SELECT p.id AS patient_id, COALESCE(p.user_id, g.user_id) AS user_id, \
                COALESCE(pu.preferred_language, gu.preferred_language) AS language \
           FROM patient_profiles p \
           LEFT JOIN patient_profiles g ON g.id = p.guardian_patient_id \
           LEFT JOIN users pu ON pu.id = p.user_id AND pu.is_active = TRUE AND pu.deleted_at IS NULL \
           LEFT JOIN users gu ON gu.id = g.user_id AND gu.is_active = TRUE AND gu.deleted_at IS NULL \
          WHERE EXTRACT(MONTH FROM p.date_of_birth) = $1 AND EXTRACT(DAY FROM p.date_of_birth) = $2 \
            AND NOT (p.full_name = 'Patient User' AND p.date_of_birth = DATE '1995-01-01' \
                     AND p.gender = 'OTHER') \
            AND COALESCE(p.user_id, g.user_id) IS NOT NULL \
            AND COALESCE(pu.id, gu.id) IS NOT NULL",
    )
    .bind(today.month() as i32)
    .bind(today.day() as i32)
    .fetch_all(&state.system_db)
    .await?;

    for recipient in recipients {
        let (title, body) = crate::services::NotificationService::render(
            &state.system_db,
            "birthday_greeting",
            &recipient.language,
            &[],
            "Happy Birthday!",
            "Happy Birthday from everyone at AfyaMkononi! We wish you good health in the year ahead.",
        )
        .await?;
        notification_repo::insert_if_new_with_ttl(
            &state.system_db,
            Uuid::new_v4(),
            recipient.user_id,
            "BIRTHDAY_GREETING",
            &title,
            &body,
            Some("PATIENT_PROFILE"),
            Some(recipient.patient_id),
            None,
            &format!("birthday:{}:{}", recipient.patient_id, today.year()),
            None,
            Some(86_400),
        )
        .await?;
    }
    Ok(())
}

/// Milestone 5 `HOLIDAY_GREETING` — every active Patient/Doctor account, for
/// every `kenyan_holidays` row observed on this EXACT Nairobi-local date
/// (strict equality — a scheduler outage on the real day is never caught up
/// later; see that table's own doc comment). `related_entity_id` carries
/// the `kenyan_holidays.id`, which the delivery gate
/// (`notification_delivery_gate`) reads back to decide whether a
/// religious-origin holiday additionally needs `religious_greetings_enabled`.
pub async fn run_holiday_pass(state: &AppState, now: DateTime<Utc>) -> Result<(), sqlx::Error> {
    let today = nairobi_local_date(now);
    let holidays = notification_repo::holidays_observed_on(&state.system_db, today).await?;
    if holidays.is_empty() {
        return Ok(());
    }
    // Milestone 8 Phase 6: same bug shape as run_greeting_pass/
    // run_health_education_pass -- while `today` remains an observed
    // holiday, this pass's outer gate (`holidays.is_empty()`) stays open for
    // the whole day, so recipients were re-fetched and re-rendered on every
    // tick regardless of whether they already had this exact holiday's
    // notification. Each holiday has its own dedup-key namespace
    // (`holiday:{holiday_key}:{year}:{user_id}`), so the filter is applied
    // per holiday inside the loop rather than once outside it -- multiple
    // holidays observed on the same calendar day is rare, so this adds at
    // most one extra indexed query per holiday, not per recipient.
    for holiday in &holidays {
        let recipients: Vec<GreetingRecipient> = sqlx::query_as(
            "SELECT id AS user_id, preferred_language AS language FROM users \
              WHERE role IN ('PATIENT', 'DOCTOR') AND is_active = TRUE AND deleted_at IS NULL \
                AND NOT EXISTS (SELECT 1 FROM notifications n WHERE n.dedup_key = $1 || ':' || id::text)",
        )
        .bind(format!("holiday:{}:{}", holiday.holiday_key, today.year()))
        .fetch_all(&state.system_db)
        .await?;
        for recipient in &recipients {
            let (title, body) = crate::services::NotificationService::render(
                &state.system_db,
                &holiday.holiday_key,
                &recipient.language,
                &[],
                "Happy holiday",
                "Wishing you a wonderful day from everyone at AfyaMkononi.",
            )
            .await?;
            notification_repo::insert_if_new_with_ttl(
                &state.system_db,
                Uuid::new_v4(),
                recipient.user_id,
                "HOLIDAY_GREETING",
                &title,
                &body,
                Some("KENYAN_HOLIDAY"),
                Some(holiday.id),
                None,
                &format!(
                    "holiday:{}:{}:{}",
                    holiday.holiday_key,
                    today.year(),
                    recipient.user_id
                ),
                None,
                Some(86_400),
            )
            .await?;
        }
    }
    Ok(())
}

/// Milestone 4 `NEW_ARTICLE_PUBLISHED` — fans out once per newly-published
/// Doctor article to every active Patient, then marks the article processed
/// (`engagement_notified_at`) so it is never rescanned — a Patient who
/// registers long after an article was published is never retroactively
/// notified about it ("prevent historical events from being sent late").
/// `{article_title}` is the one and only dynamic value in this template —
/// genuine, already Doctor-authored and already-published content, never
/// text this backend originates itself.
pub async fn run_new_article_pass(state: &AppState) -> Result<(), sqlx::Error> {
    #[derive(Debug, sqlx::FromRow)]
    struct PendingArticle {
        id: Uuid,
        title: String,
        version: i32,
        published_at: DateTime<Utc>,
    }
    // Keep the row lock until fan-out and the processed marker commit. This
    // makes overlapping scheduler instances partition articles instead of
    // both rendering and attempting the same fan-out concurrently.
    let mut tx = state.system_db.begin().await?;
    let articles: Vec<PendingArticle> = sqlx::query_as(
        "SELECT id, title, version, published_at FROM doctor_articles \
          WHERE status = 'PUBLISHED' AND engagement_notified_at IS NULL \
          ORDER BY published_at ASC LIMIT 20 FOR UPDATE SKIP LOCKED",
    )
    .fetch_all(&mut *tx)
    .await?;
    if articles.is_empty() {
        tx.commit().await?;
        return Ok(());
    }

    let patients: Vec<GreetingRecipient> = sqlx::query_as(
        "SELECT p.user_id, u.preferred_language AS language FROM patient_profiles p \
         JOIN users u ON u.id = p.user_id \
         WHERE p.user_id IS NOT NULL AND u.is_active = TRUE AND u.deleted_at IS NULL",
    )
    .fetch_all(&mut *tx)
    .await?;

    for article in articles {
        if article.published_at < Utc::now() - ChronoDuration::days(7) {
            sqlx::query(
                "UPDATE doctor_articles SET engagement_notified_at = clock_timestamp(), version = $2 \
                  WHERE id = $1 AND status = 'PUBLISHED' AND engagement_notified_at IS NULL",
            )
            .bind(article.id)
            .bind(article.version + 1)
            .execute(&mut *tx)
            .await?;
            continue;
        }
        for patient in &patients {
            let (title, body) = crate::services::NotificationService::render(
                &mut *tx,
                "new_article_published",
                &patient.language,
                &[],
                "New health article available",
                "A new health article is available in AfyaMkononi. Open the app to read it.",
            )
            .await?;
            notification_repo::insert_if_new_with_ttl(
                &mut *tx,
                Uuid::new_v4(),
                patient.user_id,
                "NEW_ARTICLE_PUBLISHED",
                &title,
                &body,
                Some("DOCTOR_ARTICLE"),
                Some(article.id),
                None,
                &format!("new-article:{}:{}", article.id, patient.user_id),
                Some(serde_json::json!({"article_title": article.title.clone()})),
                Some(604_800),
            )
            .await?;
        }
        // Matches `guard_doctor_article_update`'s requirement that every
        // update bump `version` by exactly one; the `status = 'PUBLISHED'`
        // guard means this silently no-ops (0 rows) if the article was
        // archived between the SELECT above and this UPDATE, rather than
        // tripping that trigger's own `OLD.status = 'ARCHIVED'` rejection.
        sqlx::query(
            "UPDATE doctor_articles SET engagement_notified_at = clock_timestamp(), version = $2 \
              WHERE id = $1 AND status = 'PUBLISHED' AND engagement_notified_at IS NULL",
        )
        .bind(article.id)
        .bind(article.version + 1)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

fn parse_hhmm(s: &str) -> Option<(u32, u32)> {
    let (h, m) = s.split_once(':')?;
    Some((h.parse().ok()?, m.parse().ok()?))
}

/// Exposed for the admin "mark all read"/listing-adjacent timeline, and for
/// tests that want to assert on a raw row's `metadata` without re-deriving
/// the whole `Notification` struct shape.
pub async fn find_by_dedup_key(
    state: &AppState,
    dedup_key: &str,
) -> Result<Option<Notification>, sqlx::Error> {
    sqlx::query_as::<_, Notification>("SELECT * FROM notifications WHERE dedup_key = $1")
        .bind(dedup_key)
        .fetch_optional(&state.system_db)
        .await
}
