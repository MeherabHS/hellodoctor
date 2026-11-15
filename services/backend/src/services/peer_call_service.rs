//! Peer Navigator voice/video calls (Phase 3b, migration
//! `20261222000007_peer_calls`). The timing rules are the doctor
//! consultation's, reused as-is: `join_window::evaluate` with the same
//! configured minutes, tokens capped to the remaining window, presence from
//! the same Agora webhook, and the same `EMPTY_ROOM_GRACE` (180 s) after
//! which the room ends. Peers have no clinical review step, so an ended room
//! simply ends; the chat stays open.

use crate::auth_context::AuthContext;
use crate::domain::ids::UserId;
use crate::domain::join_window::{self, JoinEligibility};
use crate::error::AppError;
use crate::repository::{AppState, DbRole};
use crate::services::agora_token_service::{
    derive_uid, AgoraTokenResponse, AgoraTokenService, DEFAULT_TOKEN_EXPIRE_SECS,
};
use crate::services::consultation_presence_service::{ProviderPresenceEvent, ProviderPresenceKind};
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

/// Prefix that routes an Agora channel to peer calls rather than doctor
/// consultations (doctor channels are `rtc_…`).
pub const PEER_CHANNEL_PREFIX: &str = "peer_";

pub fn channel_for(request_id: Uuid) -> String {
    format!("{PEER_CHANNEL_PREFIX}{}", request_id.simple())
}

#[derive(Debug, sqlx::FromRow)]
struct CallContext {
    status: String,
    patient_user_id: UserId,
    peer_user_id: UserId,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct PeerCallState {
    pub join: JoinEligibility,
    /// INITIALIZED, ACTIVE or ENDED; `None` before anyone has joined.
    pub session_status: Option<String>,
}

async fn load(tx: &mut sqlx::PgConnection, request_id: Uuid) -> Result<CallContext, AppError> {
    sqlx::query_as::<_, CallContext>(
        "SELECT r.status, r.patient_user_id, r.peer_user_id, s.start_time, s.end_time \
         FROM peer_requests r JOIN peer_slots s ON s.id = r.slot_id WHERE r.id = $1",
    )
    .bind(request_id)
    .fetch_optional(tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Support session not found.".into()))
}

fn eligibility(state: &AppState, call: &CallContext, now: DateTime<Utc>) -> JoinEligibility {
    // An accepted request is the peer equivalent of a confirmed video
    // consultation; everything else uses the doctor rule unchanged.
    let status = if call.status == "ACCEPTED" {
        "CONFIRMED"
    } else {
        "CLOSED"
    };
    join_window::evaluate(
        status,
        "VIDEO",
        call.start_time,
        call.end_time,
        now,
        state.config.agora_token_window_before_mins,
        state.config.agora_token_window_after_mins,
    )
}

#[derive(sqlx::FromRow)]
struct PresenceRow {
    request_id: Uuid,
    patient_user_id: UserId,
    peer_user_id: UserId,
    recovery_state: String,
    empty_room_deadline: Option<DateTime<Utc>>,
    patient_presence_sequence: i64,
    peer_presence_sequence: i64,
    patient_present: bool,
    peer_present: bool,
}

pub struct PeerCallService;

impl PeerCallService {
    /// Whether the caller can join now, and the call room's state.
    pub async fn state(
        state: &AppState,
        ctx: &AuthContext,
        request_id: Uuid,
    ) -> Result<PeerCallState, AppError> {
        let mut tx = state.begin_authed(ctx).await?;
        let call = load(&mut tx, request_id).await?;
        let session_status: Option<String> =
            sqlx::query_scalar("SELECT status FROM peer_call_sessions WHERE request_id = $1")
                .bind(request_id)
                .fetch_optional(&mut *tx)
                .await?;
        tx.commit().await?;
        let mut join = eligibility(state, &call, Utc::now());
        if session_status.as_deref() == Some("ENDED") && join.eligible {
            join.eligible = false;
            join.reason = join_window::REASON_WINDOW_CLOSED;
        }
        Ok(PeerCallState {
            join,
            session_status,
        })
    }

    /// A call token for the caller, inside the join window only and never
    /// after the room has ended.
    pub async fn mint_token(
        state: &AppState,
        ctx: &AuthContext,
        request_id: Uuid,
    ) -> Result<AgoraTokenResponse, AppError> {
        let now = Utc::now();
        let mut tx = state.begin_authed(ctx).await?;
        let call = load(&mut tx, request_id).await?;
        let join = eligibility(state, &call, now);
        if !join.eligible {
            return Err(AppError::JoinDenied {
                reason: join.reason,
                message: join_window::denial_message(join.reason).into(),
            });
        }
        let channel = channel_for(request_id);
        sqlx::query(
            "INSERT INTO peer_call_sessions (request_id, channel_name, patient_user_id, peer_user_id) \
             VALUES ($1,$2,$3,$4) ON CONFLICT (request_id) DO NOTHING",
        )
        .bind(request_id)
        .bind(&channel)
        .bind(call.patient_user_id)
        .bind(call.peer_user_id)
        .execute(&mut *tx)
        .await?;
        let status: String =
            sqlx::query_scalar("SELECT status FROM peer_call_sessions WHERE request_id = $1")
                .bind(request_id)
                .fetch_one(&mut *tx)
                .await?;
        tx.commit().await?;
        if status == "ENDED" {
            return Err(AppError::JoinDenied {
                reason: join_window::REASON_WINDOW_CLOSED,
                message: "This support call has ended. You can keep chatting.".into(),
            });
        }
        let expire_secs = (join.join_close_at - now)
            .num_seconds()
            .clamp(0, DEFAULT_TOKEN_EXPIRE_SECS as i64)
            .max(60) as u32;
        AgoraTokenService::mint(
            &channel,
            ctx.user_id,
            &state.config.agora_app_id,
            &state.config.agora_app_certificate,
            expire_secs,
        )
    }

    /// Agora join/leave for a `peer_` channel: the same ordering and
    /// empty-room rules as doctor consultations.
    pub async fn apply_presence(
        state: &AppState,
        event: &ProviderPresenceEvent,
    ) -> Result<bool, AppError> {
        let ctx = AuthContext::system();
        let mut tx = state.begin_authed_as(&ctx, DbRole::SystemWorker).await?;
        let Some(PresenceRow {
            request_id,
            patient_user_id: patient,
            peer_user_id: peer,
            recovery_state: recovery,
            empty_room_deadline: deadline,
            patient_presence_sequence: patient_seq,
            peer_presence_sequence: peer_seq,
            patient_present: patient_on,
            peer_present: peer_on,
        }) = sqlx::query_as::<_, PresenceRow>(
            "SELECT request_id, patient_user_id, peer_user_id, recovery_state, empty_room_deadline,                     patient_presence_sequence, peer_presence_sequence, patient_present, peer_present              FROM peer_call_sessions WHERE channel_name = $1 FOR UPDATE",
        )
        .bind(&event.channel_name)
        .fetch_optional(&mut *tx)
        .await?
        else {
            return Err(AppError::NotFound("Peer call channel not found.".into()));
        };
        let role = if derive_uid(&event.channel_name, patient) == event.participant_uid {
            "PATIENT"
        } else if derive_uid(&event.channel_name, peer) == event.participant_uid {
            "PEER"
        } else {
            return Err(AppError::Forbidden(
                "Provider participant is not a party to this support call.".into(),
            ));
        };
        if recovery == "ENDED" {
            tx.commit().await?;
            return Ok(false);
        }
        // A join after the empty-room deadline is too late: the room ends.
        if recovery == "GRACE_PERIOD" && deadline.is_some_and(|d| d <= Utc::now()) {
            Self::end_room(&mut tx, request_id).await?;
            tx.commit().await?;
            return Ok(false);
        }
        let (current_seq, other_present) = if role == "PATIENT" {
            (patient_seq, peer_on)
        } else {
            (peer_seq, patient_on)
        };
        if event.provider_sequence <= current_seq {
            tx.commit().await?;
            return Ok(false);
        }
        let present = matches!(event.kind, ProviderPresenceKind::Joined);
        let query = if role == "PATIENT" {
            "UPDATE peer_call_sessions
                SET patient_present = $2, patient_presence_sequence = $3,
                    recovery_state = CASE WHEN $2 OR peer_present THEN 'NONE' ELSE 'GRACE_PERIOD' END,
                    empty_room_deadline = CASE WHEN $2 OR peer_present THEN NULL ELSE clock_timestamp() + $4 * interval '1 second' END,
                    status = CASE WHEN $2 AND status = 'INITIALIZED' THEN 'ACTIVE' ELSE status END,
                    started_at = CASE WHEN $2 THEN COALESCE(started_at, clock_timestamp()) ELSE started_at END
              WHERE request_id = $1"
        } else {
            "UPDATE peer_call_sessions
                SET peer_present = $2, peer_presence_sequence = $3,
                    recovery_state = CASE WHEN $2 OR patient_present THEN 'NONE' ELSE 'GRACE_PERIOD' END,
                    empty_room_deadline = CASE WHEN $2 OR patient_present THEN NULL ELSE clock_timestamp() + $4 * interval '1 second' END,
                    status = CASE WHEN $2 AND status = 'INITIALIZED' THEN 'ACTIVE' ELSE status END,
                    started_at = CASE WHEN $2 THEN COALESCE(started_at, clock_timestamp()) ELSE started_at END
              WHERE request_id = $1"
        };
        let _ = other_present;
        sqlx::query(query)
            .bind(request_id)
            .bind(present)
            .bind(event.provider_sequence)
            .bind(
                crate::services::consultation_presence_service::EMPTY_ROOM_GRACE.num_seconds()
                    as f64,
            )
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(true)
    }

    async fn end_room(tx: &mut sqlx::PgConnection, request_id: Uuid) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE peer_call_sessions SET recovery_state = 'ENDED', status = 'ENDED', \
             ended_at = COALESCE(ended_at, clock_timestamp()) WHERE request_id = $1",
        )
        .bind(request_id)
        .execute(tx)
        .await?;
        Ok(())
    }

    /// Scheduler sweep, run with the doctor empty-room sweep: rooms empty
    /// past the grace end.
    pub async fn expire_empty_rooms(state: &AppState) -> Result<u64, AppError> {
        let ctx = AuthContext::system();
        let mut tx = state.begin_authed_as(&ctx, DbRole::SystemWorker).await?;
        let ended = sqlx::query(
            "UPDATE peer_call_sessions SET recovery_state = 'ENDED', status = 'ENDED', \
                    ended_at = COALESCE(ended_at, clock_timestamp()) \
             WHERE request_id IN (SELECT request_id FROM peer_call_sessions \
                   WHERE recovery_state = 'GRACE_PERIOD' AND empty_room_deadline <= clock_timestamp() \
                     AND NOT patient_present AND NOT peer_present \
                   FOR UPDATE SKIP LOCKED LIMIT 100)",
        )
        .execute(&mut *tx)
        .await?
        .rows_affected();
        tx.commit().await?;
        Ok(ended)
    }
}
