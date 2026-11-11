//! Server side consultation recovery driven only by provider-confirmed RTC
//! presence. Client SDK callbacks remain telemetry and cannot change these
//! fields.

use crate::auth_context::AuthContext;
use crate::domain::ids::UserId;
use crate::error::AppError;
use crate::repository::{AppState, DbRole};
use crate::services::agora_token_service::derive_uid;
use chrono::{DateTime, Duration, Utc};
use sqlx::Row;
use uuid::Uuid;

pub const EMPTY_ROOM_GRACE: Duration = Duration::seconds(180);

#[derive(Debug, Clone, Copy)]
pub enum ProviderPresenceKind {
    Joined,
    Left,
}

impl ProviderPresenceKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Joined => "JOINED",
            Self::Left => "LEFT",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProviderPresenceEvent {
    pub provider_event_id: String,
    pub channel_name: String,
    pub participant_uid: u32,
    pub kind: ProviderPresenceKind,
    pub provider_sequence: i64,
    pub provider_event_at: DateTime<Utc>,
}

/// Applies one authenticated provider event. The session row is locked before
/// the event is interpreted, so a join, leave and expiry worker cannot race
/// into a state that resurrects a needs-review consultation.
pub async fn apply_provider_event(
    state: &AppState,
    event: ProviderPresenceEvent,
) -> Result<bool, AppError> {
    if event.provider_event_id.trim().is_empty()
        || event.provider_event_id.len() > 200
        || event.provider_sequence <= 0
        || event.participant_uid == 0
    {
        return Err(AppError::Validation(
            "Invalid provider presence event.".into(),
        ));
    }

    let ctx = AuthContext::system();
    let mut tx = state.begin_authed_as(&ctx, DbRole::SystemWorker).await?;
    let session = sqlx::query(
        "SELECT cs.id, cs.appointment_id, cs.recovery_state, cs.empty_room_deadline,
                cs.patient_present, cs.doctor_present,
                cs.patient_presence_sequence, cs.doctor_presence_sequence,
                u.patient_user_id, u.guardian_user_id, u.doctor_user_id
           FROM consultation_sessions cs
           CROSS JOIN LATERAL consultation_participant_users(cs.id) u
          WHERE cs.agora_channel_name=$1
          FOR UPDATE OF cs",
    )
    .bind(&event.channel_name)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Consultation channel not found.".into()))?;

    let session_id: Uuid = session.get("id");
    let appointment_id: Uuid = session.get("appointment_id");
    let patient_user_id: Option<Uuid> = session.get("patient_user_id");
    let guardian_user_id: Option<Uuid> = session.get("guardian_user_id");
    let doctor_user_id: Uuid = session.get("doctor_user_id");
    let doctor_match =
        derive_uid(&event.channel_name, UserId::from(doctor_user_id)) == event.participant_uid;
    let patient_match = patient_user_id
        .into_iter()
        .chain(guardian_user_id)
        .any(|id| derive_uid(&event.channel_name, UserId::from(id)) == event.participant_uid);
    let role = match (patient_match, doctor_match) {
        (true, false) => "PATIENT",
        (false, true) => "DOCTOR",
        _ => {
            return Err(AppError::Forbidden(
                "Provider participant is not an authorized consultation user.".into(),
            ))
        }
    };
    // M8-G-30: the real authenticated human behind this event — whichever
    // specific id (direct patient, guardian, or doctor) actually matched
    // above, never re-derived. This is the identity the cross-session live
    // reservation is keyed on.
    let resolved_user_id: Uuid = if role == "DOCTOR" {
        doctor_user_id
    } else {
        patient_user_id
            .into_iter()
            .chain(guardian_user_id)
            .find(|id| derive_uid(&event.channel_name, UserId::from(*id)) == event.participant_uid)
            .expect("role==PATIENT implies patient_match was true, so one candidate matches")
    };

    let inserted = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO consultation_provider_presence_events
             (session_id, provider_event_id, channel_name, participant_uid, participant_role,
              event_type, provider_sequence, provider_event_at)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
         ON CONFLICT DO NOTHING
         RETURNING id",
    )
    .bind(session_id)
    .bind(&event.provider_event_id)
    .bind(&event.channel_name)
    .bind(i64::from(event.participant_uid))
    .bind(role)
    .bind(event.kind.as_str())
    .bind(event.provider_sequence)
    .bind(event.provider_event_at)
    .fetch_optional(&mut *tx)
    .await?
    .is_some();

    if !inserted {
        tx.commit().await?;
        return Ok(false);
    }

    let recovery_state: String = session.get("recovery_state");
    if recovery_state == "NEEDS_REVIEW" {
        tx.commit().await?;
        return Ok(false);
    }
    if matches!(event.kind, ProviderPresenceKind::Joined)
        && recovery_state == "GRACE_PERIOD"
        && sqlx::query_scalar::<_, bool>(
            "SELECT COALESCE(clock_timestamp() >= $1::timestamptz, false)",
        )
        .bind(session.get::<Option<DateTime<Utc>>, _>("empty_room_deadline"))
        .fetch_one(&mut *tx)
        .await?
    {
        let transitioned = sqlx::query(
            "UPDATE consultation_sessions
                SET recovery_state='NEEDS_REVIEW',
                    status='PREMATURE_TERMINATION'::session_status_enum,
                    ended_at=COALESCE(ended_at, clock_timestamp())
              WHERE id=$1 AND recovery_state='GRACE_PERIOD'",
        )
        .bind(session_id)
        .execute(&mut *tx)
        .await?
        .rows_affected()
            > 0;
        if transitioned {
            crate::services::NotificationService::notify_consultation_interrupted(
                &mut tx,
                session_id,
                appointment_id,
            )
            .await?;
            crate::services::NotificationService::notify_consultation_needs_review(
                &mut tx,
                session_id,
                appointment_id,
            )
            .await?;
        }
        tracing::warn!(%appointment_id, "late provider join rejected after empty-room deadline");
        tx.commit().await?;
        return Ok(false);
    }
    let current_sequence: i64 = if role == "PATIENT" {
        session.get("patient_presence_sequence")
    } else {
        session.get("doctor_presence_sequence")
    };
    if event.provider_sequence <= current_sequence {
        tx.commit().await?;
        return Ok(false);
    }

    let present = matches!(event.kind, ProviderPresenceKind::Joined);
    // Captured from the row locked at the top of this function (before this
    // event's own change) — the only way the room becomes fully empty is
    // this event's own role going absent while the OTHER role was already
    // absent too, with the session not already past 'NONE'.
    let other_role_present: bool = if role == "PATIENT" {
        session.get("doctor_present")
    } else {
        session.get("patient_present")
    };
    let entering_grace_period = !present && !other_role_present && recovery_state == "NONE";
    let query = if role == "PATIENT" {
        "UPDATE consultation_sessions
            SET patient_present=$2, patient_presence_sequence=$3,
                recovery_state=CASE WHEN $2 OR doctor_present THEN 'NONE' ELSE 'GRACE_PERIOD' END,
                empty_room_deadline=CASE WHEN $2 OR doctor_present THEN NULL ELSE clock_timestamp()+interval '180 seconds' END,
                status=CASE WHEN $2 AND status='INITIALIZED' THEN 'ACTIVE'::session_status_enum ELSE status END
          WHERE id=$1"
    } else {
        "UPDATE consultation_sessions
            SET doctor_present=$2, doctor_presence_sequence=$3,
                recovery_state=CASE WHEN $2 OR patient_present THEN 'NONE' ELSE 'GRACE_PERIOD' END,
                empty_room_deadline=CASE WHEN $2 OR patient_present THEN NULL ELSE clock_timestamp()+interval '180 seconds' END,
                status=CASE WHEN $2 AND status='INITIALIZED' THEN 'ACTIVE'::session_status_enum ELSE status END
          WHERE id=$1"
    };
    sqlx::query(query)
        .bind(session_id)
        .bind(present)
        .bind(event.provider_sequence)
        .execute(&mut *tx)
        .await?;

    // M8-G-30: confirm/release the cross-session single-live-reservation
    // claim for the real human behind this accepted event, using exactly
    // the identity already authenticated above — never client-supplied,
    // never derived from telemetry. See
    // docs/MILESTONE_8_SINGLE_LIVE_CONSULTATION_DESIGN.md §6-§8: a genuine
    // conflict here (this user already provider-confirmed live in a
    // different, still-live session) is recorded for admin review rather
    // than silently resolved either way, and this session's own presence
    // above is left exactly as just recorded regardless — a physically real
    // join must never be erased from this session's own bookkeeping just
    // because of a reservation conflict.
    if present {
        let confirmed = crate::repository::consultation_repo::confirm_live_reservation(
            &mut *tx,
            resolved_user_id,
            appointment_id,
            session_id,
            state.config.consultation_live_reservation_max_secs,
        )
        .await?;
        if !confirmed {
            let other: Option<(Uuid, Uuid)> = sqlx::query_as(
                "SELECT appointment_id, session_id FROM consultation_live_reservations WHERE user_id=$1",
            )
            .bind(resolved_user_id)
            .fetch_optional(&mut *tx)
            .await?;
            let (other_appointment_id, other_session_id) =
                other.unwrap_or((Uuid::nil(), Uuid::nil()));
            crate::services::AuditService::log_in_tx(
                &mut *tx,
                crate::services::AuditLogEntry::new(
                    None,
                    ctx.role,
                    "CONCURRENT_LIVE_PARTICIPATION_DETECTED",
                    "CONSULTATION_SESSION",
                    Some(session_id),
                    "FLAGGED",
                )
                .with_after(serde_json::json!({
                    "user_id": resolved_user_id,
                    "this_appointment_id": appointment_id,
                    "this_session_id": session_id,
                    "other_appointment_id": other_appointment_id,
                    "other_session_id": other_session_id,
                })),
            )
            .await?;
        }
    } else {
        crate::repository::consultation_repo::release_live_reservation(
            &mut *tx,
            resolved_user_id,
            session_id,
        )
        .await?;
    }

    if present {
        sqlx::query(
            "UPDATE appointments
                SET status='IN_CONSULTATION'::appointment_status_enum,
                    updated_at=clock_timestamp()
              WHERE id=$1 AND status='CONFIRMED'::appointment_status_enum",
        )
        .bind(appointment_id)
        .execute(&mut *tx)
        .await?;
    }
    if entering_grace_period {
        let deadline = Utc::now() + Duration::seconds(180);
        crate::services::NotificationService::notify_consultation_rejoin(
            &mut tx,
            session_id,
            appointment_id,
            deadline,
        )
        .await?;
    }
    tx.commit().await?;
    Ok(true)
}

/// Multi-instance safe, restart-safe deadline worker. It only interrupts the
/// live session; appointment completion and financial settlement remain behind
/// the existing explicit Doctor completion path.
pub async fn expire_empty_rooms(state: &AppState) -> Result<u64, AppError> {
    let ctx = AuthContext::system();
    let mut tx = state.begin_authed_as(&ctx, DbRole::SystemWorker).await?;
    let rows: Vec<(Uuid, Uuid)> = sqlx::query_as(
        "WITH due AS (
            SELECT id FROM consultation_sessions
             WHERE recovery_state='GRACE_PERIOD'
               AND empty_room_deadline <= clock_timestamp()
               AND patient_present=false AND doctor_present=false
             FOR UPDATE SKIP LOCKED LIMIT 100
         )
         UPDATE consultation_sessions cs
            SET recovery_state='NEEDS_REVIEW',
                status='PREMATURE_TERMINATION'::session_status_enum,
                ended_at=COALESCE(cs.ended_at, clock_timestamp())
           FROM due WHERE cs.id=due.id
         RETURNING cs.id, cs.appointment_id",
    )
    .fetch_all(&mut *tx)
    .await?;
    let count = rows.len() as u64;
    for (session_id, appointment_id) in &rows {
        crate::services::NotificationService::notify_consultation_interrupted(
            &mut tx,
            *session_id,
            *appointment_id,
        )
        .await?;
        crate::services::NotificationService::notify_consultation_needs_review(
            &mut tx,
            *session_id,
            *appointment_id,
        )
        .await?;
    }
    tx.commit().await?;
    if count > 0 {
        metrics::counter!("consultations_interrupted_needs_review_total").increment(count);
    }
    Ok(count)
}
