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
use crate::repository::{notification_repo, AppState};
use crate::services::document_storage_service::DocumentStorageService;
use crate::services::notification_service::{nairobi_local_date, nairobi_offset};
use crate::services::notification_transport::{PushOptions, PushOutcome};
use chrono::{DateTime, Duration as ChronoDuration, Timelike, Utc};
use std::time::Duration;
use uuid::Uuid;

/// Starts the background scheduler loop. Call once at startup (see
/// `main.rs`), mirroring `system_error::init`'s pattern of a single
/// `tokio::spawn`'d task driven off `AppState`'s existing `system_db` pool —
/// no new connection pool or role is introduced.
pub fn start(state: AppState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            let now = Utc::now();
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
            if let Err(e) = run_push_delivery_pass(&state).await {
                tracing::error!(error = ?e, "notification scheduler: FCM delivery pass failed");
            }
        }
    });
}

const PUSH_BATCH: i64 = 50;
const PUSH_MAX_ATTEMPTS: i32 = 5;

/// Delivers already-created inbox rows through the configured provider. The
/// inbox remains the source of truth: a provider response can only update
/// delivery metadata and never changes appointment, consultation or payment
/// state. In development/test, NoopTransport leaves rows in the inbox without
/// claiming them, so local behavior remains deterministic.
pub async fn run_push_delivery_pass(state: &AppState) -> Result<u64, crate::error::AppError> {
    if !state.notification_transport.is_configured() {
        return Ok(0);
    }

    let ctx = crate::auth_context::AuthContext::system();
    let mut claimed_total = 0u64;
    for _ in 0..4 {
        let mut claim_tx = state
            .begin_authed_as(&ctx, crate::repository::DbRole::SystemWorker)
            .await?;
        let rows = notification_repo::claim_due_notifications(&mut *claim_tx, PUSH_BATCH).await?;
        claim_tx.commit().await?;
        if rows.is_empty() {
            break;
        }
        claimed_total += rows.len() as u64;

        for row in rows {
            let tokens = notification_repo::device_tokens::find_enabled_for_user(
                &state.system_db,
                row.user_id,
            )
            .await?;
            let mut sent = false;
            let mut retryable = tokens.is_empty();
            let mut provider_message_id = None;

            for (device_id, token) in tokens {
                let options = PushOptions {
                    notification_id: row.id.to_string(),
                    notification_type: row.notification_type.clone(),
                    related_entity_type: row.related_entity_type.clone(),
                    related_entity_id: row.related_entity_id.map(|id| id.to_string()),
                    collapse_key: row.collapse_key.clone(),
                    ttl_seconds: row.ttl_seconds,
                };
                match state
                    .notification_transport
                    .send_push(&token, &row.title, &row.body, &options)
                    .await
                {
                    PushOutcome::SentToProvider {
                        provider_message_id: message_id,
                    } => {
                        sent = true;
                        provider_message_id = provider_message_id.or(message_id);
                        metrics::counter!("notification_push_sent_total").increment(1);
                    }
                    PushOutcome::Permanent { reason } => {
                        notification_repo::device_tokens::disable_device(
                            &state.system_db,
                            row.user_id,
                            Some(device_id),
                            Some(&token),
                        )
                        .await?;
                        metrics::counter!("notification_push_invalid_token_total").increment(1);
                        tracing::info!(
                            notification_id = %row.id,
                            notification_type = %row.notification_type,
                            "disabled a rejected notification device token: {}",
                            reason
                        );
                    }
                    PushOutcome::Retryable { reason } => {
                        retryable = true;
                        tracing::debug!(
                            notification_id = %row.id,
                            notification_type = %row.notification_type,
                            "provider delivery will be retried: {}",
                            reason
                        );
                    }
                    PushOutcome::NoProviderConfigured => {}
                }
            }

            let mut update_tx = state
                .begin_authed_as(&ctx, crate::repository::DbRole::SystemWorker)
                .await?;
            if sent {
                notification_repo::mark_delivery_sent(
                    &mut *update_tx,
                    row.id,
                    provider_message_id.as_deref(),
                )
                .await?;
            } else if retryable && row.delivery_attempts < PUSH_MAX_ATTEMPTS {
                let exponent = (row.delivery_attempts.max(1) - 1).min(6) as u32;
                let delay = (30_i64 * 2_i64.pow(exponent)).min(3600);
                notification_repo::mark_delivery_retry(
                    &mut *update_tx,
                    row.id,
                    "FCM provider delivery was temporarily unavailable",
                    Utc::now() + ChronoDuration::seconds(delay),
                )
                .await?;
                metrics::counter!("notification_push_retry_total").increment(1);
            } else {
                notification_repo::mark_delivery_permanent(
                    &mut *update_tx,
                    row.id,
                    if retryable {
                        "FCM delivery retry budget exhausted"
                    } else {
                        "No enabled device token is registered for this account"
                    },
                )
                .await?;
                metrics::counter!("notification_push_permanent_failure_total").increment(1);
            }
            update_tx.commit().await?;
        }
        if claimed_total < PUSH_BATCH as u64 {
            break;
        }
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
}

/// Creates non-clinical Patient inbox notifications. The unique dedup key is
/// the event identity, so retries and multiple scheduler instances are safe.
pub async fn run_hold_notification_pass(state: &AppState) -> Result<(), sqlx::Error> {
    let now = Utc::now();
    let active: Vec<HoldNotificationTarget> = sqlx::query_as(
        "SELECT a.id AS appointment_id, COALESCE(p.user_id, g.user_id) AS user_id, \
                COALESCE(a.hold_expires_at, a.created_at + make_interval(secs => 600)) AS hold_expires_at \
           FROM appointments a \
           JOIN patient_profiles p ON p.id=a.patient_id \
           LEFT JOIN patient_profiles g ON g.id=p.guardian_patient_id \
          WHERE a.status='PENDING_PAYMENT' AND a.hold_expires_at > clock_timestamp() \
            AND COALESCE(p.user_id, g.user_id) IS NOT NULL",
    )
    .fetch_all(&state.system_db)
    .await?;
    for target in active {
        insert_hold_created_notification(state, &target).await?;
        if target.hold_expires_at <= now + chrono::Duration::minutes(1) {
            let one_minute_key = format!("booking-hold-one-minute:{}", target.appointment_id);
            notification_repo::insert_if_new(
                &state.system_db,
                Uuid::new_v4(), target.user_id, "BOOKING_HOLD_ONE_MINUTE",
                "One minute left to complete payment",
                "Your consultation reservation expires in about one minute. Complete payment now or choose a new slot after expiry.",
                Some("APPOINTMENT"),
                Some(target.appointment_id),
                Some(target.hold_expires_at - chrono::Duration::minutes(1)),
                &one_minute_key,
                None,
            ).await?;
        }
    }

    let expired: Vec<(Uuid, UserId)> = sqlx::query_as(
        "SELECT j.appointment_id, COALESCE(p.user_id, g.user_id) \
           FROM appointment_expiry_cleanup_jobs j \
           JOIN appointments a ON a.id=j.appointment_id \
           JOIN patient_profiles p ON p.id=a.patient_id \
           LEFT JOIN patient_profiles g ON g.id=p.guardian_patient_id \
          WHERE a.status='EXPIRED' AND COALESCE(p.user_id, g.user_id) IS NOT NULL",
    )
    .fetch_all(&state.system_db)
    .await?;
    for (appointment_id, user_id) in expired {
        let key = format!("booking-hold-expired:{appointment_id}");
        notification_repo::insert_if_new(
            &state.system_db,
            Uuid::new_v4(), user_id, "BOOKING_HOLD_EXPIRED",
            "Consultation reservation expired",
            "This unpaid consultation reservation expired. Please make a new booking and upload your records again.",
            Some("APPOINTMENT"), Some(appointment_id), None, &key, None,
        ).await?;
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
    notification_repo::insert_if_new(
        &state.system_db,
        Uuid::new_v4(),
        target.user_id,
        "BOOKING_HOLD_CREATED",
        "Consultation slot reserved",
        "Your consultation slot is reserved for 10 minutes. Complete payment before the timer ends.",
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

        let body = if count == 0 {
            "You have no consultations scheduled today.".to_string()
        } else if count == 1 {
            "You have 1 consultation scheduled today.".to_string()
        } else {
            format!("You have {count} consultations scheduled today.")
        };

        notification_repo::insert_if_new(
            &state.system_db,
            Uuid::new_v4(),
            doctor.user_id,
            "DAILY_CONSULTATION_SUMMARY",
            "Today's consultations",
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
    start_time: DateTime<Utc>,
}

/// Creates a `CONSULTATION_REMINDER_15M` for every eligible (`CONFIRMED`/
/// `WAITING`, not yet started) appointment whose reminder window has opened
/// (`start_time - 15m <= now < start_time`) and doesn't already have one —
/// including an appointment booked with under 15 minutes' notice, for which
/// the window opens immediately rather than being scheduled in the past.
pub async fn run_t15_reminder_pass(
    state: &AppState,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    let window_opens_before = now + chrono::Duration::minutes(15);
    let due: Vec<DueConsultation> = sqlx::query_as(
        "SELECT a.id AS appointment_id, d.user_id AS doctor_user_id, s.start_time \
         FROM appointments a \
         JOIN doctor_schedule_slots s ON s.id = a.slot_id \
         JOIN doctor_profiles d ON d.id = a.doctor_id \
         WHERE a.status IN ('CONFIRMED', 'WAITING') \
           AND s.start_time > $1 AND s.start_time <= $2",
    )
    .bind(now)
    .bind(window_opens_before)
    .fetch_all(&state.system_db)
    .await?;

    for c in due {
        let dedup_key = format!(
            "consultation-reminder-15m:{}:{}",
            c.appointment_id,
            c.start_time.to_rfc3339()
        );
        notification_repo::insert_if_new(
            &state.system_db,
            Uuid::new_v4(),
            c.doctor_user_id,
            "CONSULTATION_REMINDER_15M",
            "Consultation starting soon",
            "Your consultation starts in 15 minutes.",
            Some("APPOINTMENT"),
            Some(c.appointment_id),
            Some(c.start_time - chrono::Duration::minutes(15)),
            &dedup_key,
            None,
        )
        .await?;
    }
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
