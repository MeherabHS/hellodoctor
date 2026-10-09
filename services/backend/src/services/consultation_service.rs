use crate::auth_context::AuthContext;
use crate::domain::models::*;
use crate::error::AppError;
use crate::repository::{
    appointment_repo, consultation_repo, doctor_repo, grievance_repo, patient_repo,
    system_error_repo, transaction_repo, AppState,
};
use crate::services::agora_token_service::{AgoraTokenResponse, AgoraTokenService};
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
        let is_patient = patient_repo::find_by_user_id(&mut *tx, user_id)
            .await?
            .map(|p| p.id == apt.patient_id)
            .unwrap_or(false);
        let is_doctor = doctor_repo::find_by_id(&mut *tx, apt.doctor_id)
            .await?
            .map(|d| d.user_id == user_id && d.is_professionally_verified())
            .unwrap_or(false);

        if !is_patient && !is_doctor {
            return Err(AppError::Forbidden(
                "Caller is not an authorized participant in this consultation.".into(),
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
                };
                consultation_repo::insert_session(&mut *tx, &s).await?;
                s
            }
        };

        tx.commit().await?;

        AgoraTokenService::mint(
            &session.agora_channel_name,
            user_id,
            agora_app_id,
            agora_certificate,
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

        let is_patient = patient_repo::find_by_user_id(&mut *tx, ctx.user_id)
            .await?
            .map(|p| p.id == apt.patient_id)
            .unwrap_or(false);
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

        let mut participant_user_ids = Vec::new();
        if let Some(p) = patient_repo::find_by_id(&mut *tx, apt.patient_id).await? {
            participant_user_ids.push(p.user_id);
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
        // Captured *before* calling the atomic completion function below:
        // `doctor_complete_consultation` only performs the authoritative
        // wallet credit when the transaction is still `PAYMENT_HELD` at that
        // exact moment (see the SQL function's own `IF payment_row.
        // payment_status='PAYMENT_HELD'` guard) — on a retried/duplicate
        // completion call the status has already moved on, so the credit
        // (and therefore this notification) naturally never fires twice.
        // Deliberately does not modify the already-tested, security-definer
        // SQL function itself to report this — reading the pre-call state in
        // Rust is less invasive and keeps that frozen function untouched.
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
            "COMPLETED" => {}
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

        // `EARNING_CREDITED` — only when this exact call was the one that
        // held `PAYMENT_HELD` beforehand (see the capture above), so it
        // fires once per appointment regardless of how many times
        // completion is retried, in the same transaction as the credit
        // itself (a rollback here means no notification, matching the spec's
        // "cannot fabricate a credit that rolled back" requirement).
        if let Some(txn) = pre_call_payment_held {
            if let Some(appointment) =
                appointment_repo::find_by_id(&mut **tx, appointment_id).await?
            {
                if let Some(doctor) =
                    doctor_repo::find_by_id(&mut **tx, appointment.doctor_id).await?
                {
                    crate::services::NotificationService::notify_earning_credited(
                        &mut **tx,
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
