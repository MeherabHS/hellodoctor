use crate::auth_context::AuthContext;
use crate::domain::models::*;
use crate::error::AppError;
use crate::repository::{
    appointment_repo, consultation_repo, doctor_repo, grievance_repo, patient_repo,
    system_error_repo, transaction_repo, AppState,
};
use crate::services::agora_token_service::{
    AgoraTokenResponse, AgoraTokenService, DEFAULT_TOKEN_EXPIRE_SECS,
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use std::collections::HashMap;
use uuid::Uuid;

/// Client-submittable event vocabulary. `SERVER_ERROR` is a reserved
/// vocabulary entry (also present in the DB `CHECK` constraint) that nothing
/// writes yet in this pass — see migrations/20261004000002's own comment.
pub const TELEMETRY_EVENT_TYPES: &[&str] = &[
    "CALL_INITIATED",
    "PATIENT_JOINED",
    "DOCTOR_JOINED",
    "PATIENT_RECONNECTING",
    "PATIENT_RECONNECTED",
    "PATIENT_DISCONNECTED",
    "DOCTOR_RECONNECTING",
    "DOCTOR_RECONNECTED",
    "DOCTOR_DISCONNECTED",
    "NETWORK_DEGRADED",
    "NETWORK_RESTORED",
    "CALL_ENDED",
];

/// Bounds a single ingestion call — telemetry ingestion must stay bounded and
/// non-blocking, never a vector for an unbounded write per the constraint in
/// docs/CHECKPOINT.md §8.
pub const MAX_TELEMETRY_EVENTS_PER_BATCH: usize = 20;

/// Handler-independent shape for one incoming event — the handler maps its
/// `axum`/`serde` request DTO into this before calling the service, matching
/// the existing `record_telemetry` convention of services taking plain data
/// rather than request types.
pub struct NewTelemetryEvent {
    pub event_type: String,
    pub rtt_ms: Option<i32>,
    pub packet_loss_percent: Option<BigDecimal>,
    pub occurred_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct ReconnectWindow {
    pub perspective: String,
    pub disconnected_at: DateTime<Utc>,
    pub reconnected_at: Option<DateTime<Utc>>,
    pub duration_seconds: Option<i64>,
}

pub struct ConsultationService;

impl ConsultationService {
    /// Mint secure, unpredictable Agora RTC Token (server-side only)
    pub async fn mint_agora_token(
        state: &AppState,
        ctx: &AuthContext,
        appointment_id: Uuid,
        agora_app_id: &str,
        agora_certificate: &str,
    ) -> Result<AgoraTokenResponse, AppError> {
        let user_id = ctx.user_id;
        let mut tx = state.begin_authed(ctx).await?;

        let apt = crate::repository::appointment_repo::find_by_id(&mut *tx, appointment_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Appointment record not found.".into()))?;

        // `apt.patient_id`/`apt.doctor_id` are patient_profiles/doctor_profiles
        // ids, not users.id — comparing either straight against `ctx.user_id`
        // would never match (different UUID spaces entirely; this was a real,
        // pre-existing bug that made the patient side of this check
        // unreachable, caught only by testing with a genuinely distinct
        // user_id vs. profile_id rather than a demo context where the two
        // happened to be conflated).
        //
        // `apt.patient_id` may be a dependent profile (no `user_id` of its
        // own, no login) — in that case the real authorized participant is
        // whoever guardians it, so this walks up exactly one level
        // (dependents cannot nest) rather than only matching a direct self
        // profile. Without this, a guardian joining their dependent's own
        // consultation would be wrongly rejected as "not an authorized
        // participant" — caught directly by this service's own test, not
        // just reasoned about.
        let is_patient = match patient_repo::find_by_id(&mut *tx, apt.patient_id).await? {
            Some(p) if p.user_id == Some(user_id) => true,
            Some(p) => match p.guardian_patient_id {
                Some(guardian_id) => patient_repo::find_by_id(&mut *tx, guardian_id)
                    .await?
                    .map(|g| g.user_id == Some(user_id))
                    .unwrap_or(false),
                None => false,
            },
            None => false,
        };
        let is_doctor = doctor_repo::find_by_id(&mut *tx, apt.doctor_id)
            .await?
            .map(|d| d.user_id == user_id && d.is_professionally_verified())
            .unwrap_or(false);

        if !is_patient && !is_doctor {
            return Err(AppError::Forbidden(
                "Caller is not an authorized participant in this consultation.".into(),
            ));
        }

        if apt.modality != Modality::Video {
            return Err(AppError::Forbidden(
                "Video access is only available for video consultations.".into(),
            ));
        }

        if apt.status != AppointmentStatus::Confirmed
            && apt.status != AppointmentStatus::InConsultation
        {
            return Err(AppError::Conflict(
                "Consultation room is only accessible for CONFIRMED appointments.".into(),
            ));
        }

        // Temporal window gate (docs/AUTH_SECURITY.md §10.2, Pass 2 fix —
        // previously entirely absent: a confirmed appointment's status alone
        // let a participant mint a working RTC token arbitrarily early or
        // indefinitely late). Configurable via `AGORA_TOKEN_WINDOW_*_MINS`
        // rather than hardcoded, since the real policy is a product
        // decision. Scoped to the two already-verified participants only —
        // this is a conformance/business-rule gate, not a cross-tenant
        // authorization boundary (that's the ownership check above).
        let slot = doctor_repo::find_slot(&mut *tx, apt.slot_id)
            .await?
            .ok_or_else(|| {
                AppError::internal("Schedule slot for a confirmed appointment is missing.".into())
            })?;
        let now = Utc::now();
        let window_start =
            slot.start_time - Duration::minutes(state.config.agora_token_window_before_mins);
        let window_end =
            slot.end_time + Duration::minutes(state.config.agora_token_window_after_mins);
        if now < window_start || now > window_end {
            return Err(AppError::Forbidden(
                "The consultation room is not open yet, or this appointment's window has closed."
                    .into(),
            ));
        }

        let session = match consultation_repo::find_by_appointment(&mut *tx, appointment_id).await?
        {
            Some(s) => s,
            None => {
                let s = ConsultationSession {
                    id: Uuid::new_v4(),
                    appointment_id,
                    agora_channel_name: format!(
                        "rtc_{}",
                        Uuid::new_v4().to_string().replace('-', "")
                    ),
                    started_at: Some(Utc::now()),
                    ended_at: None,
                    status: SessionStatus::Active,
                    created_at: Utc::now(),
                    recovery_state: "NONE".into(),
                    empty_room_deadline: None,
                    patient_present: false,
                    doctor_present: false,
                    patient_presence_sequence: 0,
                    doctor_presence_sequence: 0,
                };
                consultation_repo::insert_session(&mut *tx, &s).await?;
                // Another authorized participant may have created the session
                // concurrently. Always mint for the stored shared channel.
                consultation_repo::find_by_appointment(&mut *tx, appointment_id)
                    .await?
                    .ok_or_else(|| {
                        AppError::internal("Consultation session was not persisted.".into())
                    })?
            }
        };

        if session.recovery_state == "NEEDS_REVIEW" {
            return Err(AppError::Conflict(
                "This consultation needs review before it can be rejoined.".into(),
            ));
        }

        // M8-G-30: at most one live-consultation reservation per user_id,
        // across all roles. This only prevents a NEW token from being
        // minted — it is a prospective, mint-time-only control. It does not
        // and cannot prove or disprove actual RTC participation; see
        // docs/MILESTONE_8_SINGLE_LIVE_CONSULTATION_DESIGN.md for the full,
        // explicitly scoped set of guarantees (and non-guarantees) this
        // provides on its own, before the Agora provider webhook is
        // configured.
        if !consultation_repo::acquire_live_reservation(
            &mut *tx,
            user_id.into(),
            appointment_id,
            session.id,
            consultation_repo::RESERVATION_PENDING_GRACE_SECS,
        )
        .await?
        {
            return Err(AppError::Conflict(
                "You are already live in another consultation.".into(),
            ));
        }

        tx.commit().await?;

        // Bound the token's real cryptographic expiry (and every privilege it
        // grants) to whatever remains of the already-verified authorization
        // window, capped at `DEFAULT_TOKEN_EXPIRE_SECS` — a token must never
        // outlive the appointment window that justified minting it. The
        // window check above already guarantees `now <= window_end`; the
        // `max(60, ...)` floor only guards the boundary instant where that
        // remainder would otherwise round down to a useless near-zero token.
        let remaining_secs = (window_end - now).num_seconds();
        let expire_secs = remaining_secs
            .clamp(0, DEFAULT_TOKEN_EXPIRE_SECS as i64)
            .max(60) as u32;

        AgoraTokenService::mint(
            &session.agora_channel_name,
            user_id,
            agora_app_id,
            agora_certificate,
            expire_secs,
        )
    }

    /// Record session telemetry received from Agora onRtcStats
    pub async fn record_telemetry(
        state: &AppState,
        ctx: &AuthContext,
        appointment_id: Uuid,
        call_duration_seconds: i32,
        connection_state: String,
        prescription_issued: bool,
    ) -> Result<ConsultationTelemetry, AppError> {
        let mut tx = state.begin_authed(ctx).await?;

        let session = consultation_repo::find_by_appointment(&mut *tx, appointment_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Consultation session not found.".into()))?;

        let premature_end = call_duration_seconds < 30;

        let telemetry = ConsultationTelemetry {
            id: Uuid::new_v4(),
            session_id: session.id,
            call_duration_seconds,
            connection_state,
            packet_loss_percent: None,
            round_trip_time_ms: None,
            premature_end,
            prescription_issued,
            captured_at: Utc::now(),
        };

        consultation_repo::insert_telemetry(&mut *tx, &telemetry).await?;
        tx.commit().await?;
        Ok(telemetry)
    }

    /// Record a bounded batch of detailed call-lifecycle events. Fail-open by
    /// design (docs/CHECKPOINT.md §8): a caller who isn't a genuine
    /// participant is still rejected (that's an auth boundary, not a
    /// telemetry-persistence concern), but once authorized, individual
    /// malformed events (unknown `event_type`, out-of-range metrics) are
    /// silently skipped rather than failing the whole batch — ingesting
    /// observability data must never be able to break an active consultation.
    /// `actor_perspective` is always derived here from the caller's verified
    /// identity, never trusted from client input.
    pub async fn record_telemetry_events(
        state: &AppState,
        ctx: &AuthContext,
        appointment_id: Uuid,
        events: Vec<NewTelemetryEvent>,
    ) -> Result<usize, AppError> {
        let mut tx = state.begin_authed(ctx).await?;

        let apt = crate::repository::appointment_repo::find_by_id(&mut *tx, appointment_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Appointment record not found.".into()))?;

        // Same guardian-aware ownership resolution as `mint_agora_token` —
        // `apt.patient_id` may be a dependent profile with no `user_id` of
        // its own, so the real authorized participant is whoever guardians
        // it.
        let is_patient = match patient_repo::find_by_id(&mut *tx, apt.patient_id).await? {
            Some(p) if p.user_id == Some(ctx.user_id) => true,
            Some(p) => match p.guardian_patient_id {
                Some(guardian_id) => patient_repo::find_by_id(&mut *tx, guardian_id)
                    .await?
                    .map(|g| g.user_id == Some(ctx.user_id))
                    .unwrap_or(false),
                None => false,
            },
            None => false,
        };
        let perspective = if is_patient {
            "PATIENT"
        } else {
            let is_doctor = doctor_repo::find_by_id(&mut *tx, apt.doctor_id)
                .await?
                .map(|d| d.user_id == ctx.user_id)
                .unwrap_or(false);
            if !is_doctor {
                return Err(AppError::Forbidden(
                    "Caller is not an authorized participant in this consultation.".into(),
                ));
            }
            "DOCTOR"
        };

        let session = consultation_repo::find_by_appointment(&mut *tx, appointment_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Consultation session not found.".into()))?;

        let zero = BigDecimal::from(0);
        let hundred = BigDecimal::from(100);
        let mut accepted = 0usize;
        for input in events.into_iter().take(MAX_TELEMETRY_EVENTS_PER_BATCH) {
            if !TELEMETRY_EVENT_TYPES.contains(&input.event_type.as_str()) {
                continue;
            }
            if let Some(rtt) = input.rtt_ms {
                if rtt < 0 {
                    continue;
                }
            }
            if let Some(pl) = &input.packet_loss_percent {
                if *pl < zero || *pl > hundred {
                    continue;
                }
            }

            let event = ConsultationTelemetryEvent {
                id: Uuid::new_v4(),
                ingest_seq: 0,
                session_id: session.id,
                event_type: input.event_type,
                actor_perspective: perspective.to_string(),
                rtt_ms: input.rtt_ms,
                packet_loss_percent: input.packet_loss_percent,
                occurred_at: input.occurred_at.unwrap_or_else(Utc::now),
                created_at: Utc::now(),
            };
            consultation_repo::insert_telemetry_event(&mut *tx, &event).await?;
            accepted += 1;
        }

        tx.commit().await?;
        Ok(accepted)
    }

    /// Admin incident-reconstruction view for one consultation: the ordered
    /// event timeline, reconstructed reconnect windows per perspective, the
    /// pre-existing aggregate telemetry row, correlated grievances, and a
    /// best-effort (time-window + participant overlap, not a hard FK) list of
    /// system errors that may be related — see
    /// `system_error_repo::find_related`'s own doc comment for why this is
    /// heuristic rather than exact.
    pub async fn get_session_timeline(
        state: &AppState,
        ctx: &AuthContext,
        session_id: Uuid,
    ) -> Result<serde_json::Value, AppError> {
        let mut tx = state.begin_authed(ctx).await?;

        let session = consultation_repo::find_by_id(&mut *tx, session_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Consultation session not found.".into()))?;

        let apt = crate::repository::appointment_repo::find_by_id(&mut *tx, session.appointment_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Appointment record not found.".into()))?;

        let events =
            consultation_repo::list_telemetry_events_for_session(&mut *tx, session_id).await?;
        let aggregate = consultation_repo::find_by_session(&mut *tx, session_id).await?;
        let grievances = grievance_repo::find_by_consultation_id(&mut *tx, session_id).await?;
        let provider_presence_events: Vec<serde_json::Value> = sqlx::query_scalar(
            "SELECT jsonb_build_object(
                'provider_event_id', provider_event_id,
                'event_type', event_type,
                'participant_uid', participant_uid,
                'participant_role', participant_role,
                'provider_sequence', provider_sequence,
                'provider_event_at', provider_event_at,
                'received_at', received_at
             )
             FROM consultation_provider_presence_events
             WHERE session_id = $1
             ORDER BY provider_sequence, received_at, id
             LIMIT 500",
        )
        .bind(session_id)
        .fetch_all(&mut *tx)
        .await?;
        let recovery_resolution: Option<serde_json::Value> = sqlx::query_scalar(
            "SELECT jsonb_build_object('resolution_id', id, 'outcome', outcome,
                    'reason', reason, 'admin_user_id', admin_user_id, 'created_at', created_at)
             FROM consultation_recovery_resolutions WHERE session_id=$1",
        )
        .bind(session_id)
        .fetch_optional(&mut *tx)
        .await?;

        let mut participant_user_ids = Vec::new();
        if let Some(p) = patient_repo::find_by_id(&mut *tx, apt.patient_id).await? {
            match p.user_id {
                Some(uid) => participant_user_ids.push(uid),
                // A dependent profile has no login of its own — the real
                // authenticated participant is whoever guardians it. Walk up
                // exactly one level (dependents cannot nest, enforced by a DB
                // trigger) so a dependent-booked consultation's incidents are
                // still correlated to the account that actually joined.
                None => {
                    if let Some(guardian_id) = p.guardian_patient_id {
                        if let Some(g) = patient_repo::find_by_id(&mut *tx, guardian_id).await? {
                            if let Some(uid) = g.user_id {
                                participant_user_ids.push(uid);
                            }
                        }
                    }
                }
            }
        }
        if let Some(d) = doctor_repo::find_by_id(&mut *tx, apt.doctor_id).await? {
            participant_user_ids.push(d.user_id);
        }
        let since = session.started_at.unwrap_or(session.created_at) - Duration::minutes(2);
        let until = session.ended_at.unwrap_or_else(Utc::now) + Duration::minutes(2);
        let related_system_errors =
            system_error_repo::find_related(&mut *tx, &participant_user_ids, since, until).await?;

        tx.commit().await?;

        let reconnect_windows = reconstruct_reconnect_windows(&events);

        Ok(serde_json::json!({
            "session_id": session.id,
            "appointment_id": session.appointment_id,
            "started_at": session.started_at,
            "ended_at": session.ended_at,
            "status": session.status,
            "interruption_reason": if session.recovery_state == "NEEDS_REVIEW" {
                Some("EMPTY_ROOM_GRACE_EXPIRED")
            } else {
                None::<&str>
            },
            "recovery_state": session.recovery_state,
            "empty_room_deadline": session.empty_room_deadline,
            "patient_present": session.patient_present,
            "doctor_present": session.doctor_present,
            "patient_presence_sequence": session.patient_presence_sequence,
            "doctor_presence_sequence": session.doctor_presence_sequence,
            "provider_presence_events": provider_presence_events,
            "provider_presence_semantics": "Signed Agora provider callbacks are authoritative for presence. Client telemetry and reconnect_windows below are supplementary and cannot change recovery state.",
            "recovery_resolution": recovery_resolution,
            "events": events,
            "reconnect_windows": reconnect_windows,
            "aggregate_telemetry": aggregate,
            "related_grievances": grievances,
            "related_system_errors": related_system_errors,
            "related_system_errors_note": "Best-effort correlation by time-window + participant overlap, not a hard link — see system_error_repo::find_related.",
        }))
    }

    /// Completion and exactly-once settlement share one narrowly authorized
    /// database transaction. A failure in wallet or audit writes rolls back the
    /// clinical transition as well; no cross-pool partial success is reported.
    pub async fn complete_consultation(
        state: &AppState,
        ctx: &AuthContext,
        appointment_id: Uuid,
        outcome: ClinicalOutcome,
    ) -> Result<(), AppError> {
        let mut tx = state.begin_authed(ctx).await?;
        Self::complete_in_transaction(&mut tx, ctx, appointment_id, outcome).await?;
        tx.commit().await?;
        Ok(())
    }

    pub(super) async fn complete_in_transaction(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        ctx: &AuthContext,
        appointment_id: Uuid,
        outcome: ClinicalOutcome,
    ) -> Result<(), AppError> {
        // This snapshot supplies notification amounts only. The SQL function's
        // locked ALREADY_COMPLETED decision below is the concurrency gate;
        // an unlocked pre-call status cannot decide who earned the effect.
        let pre_call_payment_held =
            transaction_repo::find_by_appointment(&mut **tx, appointment_id)
                .await?
                .filter(|t| t.payment_status == PaymentStatus::PaymentHeld);

        let result: String = sqlx::query_scalar("SELECT doctor_complete_consultation($1,$2)")
            .bind(appointment_id)
            .bind(outcome.as_db_str())
            .fetch_one(&mut **tx)
            .await?;
        match result.as_str() {
            "COMPLETED" => {
                crate::services::NotificationService::notify_consultation_completed(
                    tx,
                    appointment_id,
                )
                .await?;
                crate::services::NotificationService::notify_first_consultation_milestone(
                    tx,
                    appointment_id,
                )
                .await?;
            }
            // The SQL function decides this while holding authoritative rows.
            // A stale pre-call snapshot must not repeat semantic side effects.
            "ALREADY_COMPLETED" => return Ok(()),
            "NOT_FOUND" => return Err(AppError::NotFound("Appointment not found.".into())),
            "FORBIDDEN" => {
                return Err(AppError::Forbidden(
                    "An active, verified assigned Doctor is required.".into(),
                ))
            }
            "PAYMENT_NOT_CONFIRMED" => {
                return Err(AppError::Conflict(
                    "Payment has not been confirmed; completion was not recorded.".into(),
                ))
            }
            "INVALID_SESSION_STATE" => {
                return Err(AppError::Conflict(
                    "This consultation was interrupted and requires review before completion."
                        .into(),
                ))
            }
            "SESSION_NOT_FOUND" => {
                return Err(AppError::Conflict(
                    "The consultation session is not available for completion.".into(),
                ))
            }
            "PERIOD_CLOSED" => {
                return Err(AppError::Conflict(
                    "The accounting period is closed; Finance review is required.".into(),
                ))
            }
            _ => {
                return Err(AppError::Conflict(
                    "The consultation cannot be completed in its current state.".into(),
                ))
            }
        }

        crate::services::consultation_trace::record(
            tx,
            appointment_id,
            None,
            crate::services::consultation_trace::Event::accepted(
                "CONSULTATION_COMPLETED",
                Some(("APPOINTMENT", appointment_id)),
            ),
        )
        .await?;
        // Only the authoritative first completion reaches this point. Its
        // notification commits/rolls back with the wallet credit and audit.
        if let Some(txn) = pre_call_payment_held {
            if let Some(appointment) =
                appointment_repo::find_by_id(&mut **tx, appointment_id).await?
            {
                if let Some(doctor) =
                    doctor_repo::find_by_id(&mut **tx, appointment.doctor_id).await?
                {
                    crate::services::NotificationService::notify_earning_credited(
                        tx,
                        doctor.user_id,
                        appointment_id,
                        &txn.net_amount,
                        &txn.currency,
                    )
                    .await?;
                }
            }
        }

        crate::services::audit_service::AuditService::log_in_tx(
            &mut **tx,
            crate::services::audit_service::AuditLogEntry::new(
                Some(ctx.user_id),
                ctx.role,
                "CONSULTATION_COMPLETED",
                "APPOINTMENT",
                Some(appointment_id),
                "SUCCESS",
            )
            .with_after(serde_json::json!({"status":"COMPLETED","outcome":outcome.as_db_str()})),
        )
        .await?;
        Ok(())
    }
}

/// Pairs each `*_DISCONNECTED`/`*_RECONNECTING` event with the following
/// `*_RECONNECTED` event for the same perspective (PATIENT/DOCTOR), in
/// ingestion order, to reconstruct how long each reconnect took. A
/// disconnect with no matching reconnect by the end of the event list is
/// reported with `reconnected_at: None` (still down / call ended abruptly).
fn reconstruct_reconnect_windows(events: &[ConsultationTelemetryEvent]) -> Vec<ReconnectWindow> {
    let mut open: HashMap<&'static str, DateTime<Utc>> = HashMap::new();
    let mut windows = Vec::new();

    for e in events {
        match e.event_type.as_str() {
            "PATIENT_DISCONNECTED" | "PATIENT_RECONNECTING" => {
                open.entry("PATIENT").or_insert(e.occurred_at);
            }
            "DOCTOR_DISCONNECTED" | "DOCTOR_RECONNECTING" => {
                open.entry("DOCTOR").or_insert(e.occurred_at);
            }
            "PATIENT_RECONNECTED" => {
                if let Some(start) = open.remove("PATIENT") {
                    windows.push(ReconnectWindow {
                        perspective: "PATIENT".into(),
                        disconnected_at: start,
                        reconnected_at: Some(e.occurred_at),
                        duration_seconds: Some((e.occurred_at - start).num_seconds()),
                    });
                }
            }
            "DOCTOR_RECONNECTED" => {
                if let Some(start) = open.remove("DOCTOR") {
                    windows.push(ReconnectWindow {
                        perspective: "DOCTOR".into(),
                        disconnected_at: start,
                        reconnected_at: Some(e.occurred_at),
                        duration_seconds: Some((e.occurred_at - start).num_seconds()),
                    });
                }
            }
            _ => {}
        }
    }

    for (perspective, start) in open {
        windows.push(ReconnectWindow {
            perspective: perspective.to_string(),
            disconnected_at: start,
            reconnected_at: None,
            duration_seconds: None,
        });
    }
    windows.sort_by_key(|w| w.disconnected_at);
    windows
}

#[cfg(test)]
mod reconnect_window_tests {
    use super::*;

    fn ev(event_type: &str, occurred_at: DateTime<Utc>) -> ConsultationTelemetryEvent {
        ConsultationTelemetryEvent {
            id: Uuid::new_v4(),
            ingest_seq: 0,
            session_id: Uuid::new_v4(),
            event_type: event_type.to_string(),
            actor_perspective: "PATIENT".to_string(),
            rtt_ms: None,
            packet_loss_percent: None,
            occurred_at,
            created_at: occurred_at,
        }
    }

    #[test]
    fn pairs_disconnect_with_reconnect() {
        let t0 = Utc::now();
        let events = vec![
            ev("PATIENT_DISCONNECTED", t0),
            ev("PATIENT_RECONNECTED", t0 + Duration::seconds(7)),
        ];
        let windows = reconstruct_reconnect_windows(&events);
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].perspective, "PATIENT");
        assert_eq!(windows[0].duration_seconds, Some(7));
    }

    #[test]
    fn unresolved_disconnect_has_no_reconnected_at() {
        let t0 = Utc::now();
        let events = vec![ev("DOCTOR_DISCONNECTED", t0)];
        let windows = reconstruct_reconnect_windows(&events);
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].perspective, "DOCTOR");
        assert!(windows[0].reconnected_at.is_none());
        assert!(windows[0].duration_seconds.is_none());
    }
}
