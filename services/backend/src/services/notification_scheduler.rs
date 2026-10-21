//! DB-backed due-notification worker for the Doctor Engagement extension.
//! Deliberately not a new broker/queue (Redis or otherwise) — per the
//! approved spec, "a simple DB-backed due-notification worker is acceptable
//! at this scale," and this project already has one Redis instance used for
//! rate-limiting/sessions, not a job queue; adding a second responsibility to
//! it (or a third broker entirely) for two low-volume, low-latency-tolerance
//! notification types would be more infrastructure than the problem needs.
//!
//! Two independent passes, each idempotent via `notifications.dedup_key`
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
use crate::services::notification_service::{nairobi_local_date, nairobi_offset};
use chrono::{DateTime, Timelike, Utc};
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
            if let Err(e) = run_daily_summary_pass(&state, now).await {
                tracing::error!(error = ?e, "notification scheduler: daily-summary pass failed");
            }
            if let Err(e) = run_t15_reminder_pass(&state, now).await {
                tracing::error!(error = ?e, "notification scheduler: T-15 reminder pass failed");
            }
        }
    });
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
