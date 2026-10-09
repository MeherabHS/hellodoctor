use crate::domain::models::{
    ConsultationSession, ConsultationTelemetry, ConsultationTelemetryEvent, SessionStatus,
};
use sqlx::PgExecutor;
use uuid::Uuid;

pub async fn insert_session<'e, E: PgExecutor<'e>>(
    executor: E,
    session: &ConsultationSession,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO consultation_sessions (id, appointment_id, agora_channel_name, started_at, ended_at, status, created_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7)",
    )
    .bind(session.id)
    .bind(session.appointment_id)
    .bind(&session.agora_channel_name)
    .bind(session.started_at)
    .bind(session.ended_at)
    .bind(session.status)
    .bind(session.created_at)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn find_by_appointment<'e, E: PgExecutor<'e>>(
    executor: E,
    appointment_id: Uuid,
) -> Result<Option<ConsultationSession>, sqlx::Error> {
    sqlx::query_as::<_, ConsultationSession>(
        "SELECT id, appointment_id, agora_channel_name, started_at, ended_at, status, created_at FROM consultation_sessions WHERE appointment_id = $1",
    )
    .bind(appointment_id)
    .fetch_optional(executor)
    .await
}

pub async fn find_by_id<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
) -> Result<Option<ConsultationSession>, sqlx::Error> {
    sqlx::query_as::<_, ConsultationSession>(
        "SELECT id, appointment_id, agora_channel_name, started_at, ended_at, status, created_at FROM consultation_sessions WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(executor)
    .await
}

pub async fn list_active<'e, E: PgExecutor<'e>>(
    executor: E,
) -> Result<Vec<ConsultationSession>, sqlx::Error> {
    sqlx::query_as::<_, ConsultationSession>(
        "SELECT id, appointment_id, agora_channel_name, started_at, ended_at, status, created_at FROM consultation_sessions WHERE status IN ('ACTIVE', 'INITIALIZED')",
    )
    .fetch_all(executor)
    .await
}

/// Whole-table fetch used by admin dashboard handlers to build an in-memory
/// id-keyed lookup map, rather than issuing one query per row in a loop —
/// matches the same pattern already used for `doctor_repo::list`/
/// `patient_repo::list`/`appointment_repo::list_all` elsewhere in those handlers.
pub async fn list_all<'e, E: PgExecutor<'e>>(
    executor: E,
) -> Result<Vec<ConsultationSession>, sqlx::Error> {
    sqlx::query_as::<_, ConsultationSession>(
        "SELECT id, appointment_id, agora_channel_name, started_at, ended_at, status, created_at FROM consultation_sessions",
    )
    .fetch_all(executor)
    .await
}

/// Whole-table fetch, same rationale as `list_all` above.
pub async fn list_all_telemetry<'e, E: PgExecutor<'e>>(
    executor: E,
) -> Result<Vec<ConsultationTelemetry>, sqlx::Error> {
    sqlx::query_as::<_, ConsultationTelemetry>(
        "SELECT id, session_id, call_duration_seconds, connection_state, packet_loss_percent, round_trip_time_ms, premature_end, prescription_issued, captured_at \
         FROM consultation_telemetry",
    )
    .fetch_all(executor)
    .await
}

/// Scoped alternative to `list_all_telemetry` — see
/// appointment_repo::find_by_ids's doc comment for the rationale. Confirmed
/// necessary: under a 12,500-row `consultation_telemetry` table, the
/// unconditional whole-table fetch in `get_command_telemetry` (which only
/// ever needs telemetry for a handful of *currently active* sessions) was
/// measured adding seconds of latency per admin dashboard poll.
pub async fn list_telemetry_by_session_ids<'e, E: PgExecutor<'e>>(
    executor: E,
    session_ids: &[Uuid],
) -> Result<Vec<ConsultationTelemetry>, sqlx::Error> {
    if session_ids.is_empty() {
        return Ok(Vec::new());
    }
    sqlx::query_as::<_, ConsultationTelemetry>(
        "SELECT id, session_id, call_duration_seconds, connection_state, packet_loss_percent, round_trip_time_ms, premature_end, prescription_issued, captured_at \
         FROM consultation_telemetry WHERE session_id = ANY($1)",
    )
    .bind(session_ids)
    .fetch_all(executor)
    .await
}

pub async fn set_status<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    status: SessionStatus,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE consultation_sessions SET status = $2 WHERE id = $1")
        .bind(id)
        .bind(status)
        .execute(executor)
        .await?;
    Ok(())
}

pub async fn insert_telemetry<'e, E: PgExecutor<'e>>(
    executor: E,
    t: &ConsultationTelemetry,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO consultation_telemetry (id, session_id, call_duration_seconds, connection_state, packet_loss_percent, round_trip_time_ms, premature_end, prescription_issued, captured_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9) \
         ON CONFLICT (session_id) DO UPDATE SET call_duration_seconds = EXCLUDED.call_duration_seconds, connection_state = EXCLUDED.connection_state, \
         premature_end = EXCLUDED.premature_end, prescription_issued = EXCLUDED.prescription_issued, captured_at = EXCLUDED.captured_at",
    )
    .bind(t.id)
    .bind(t.session_id)
    .bind(t.call_duration_seconds)
    .bind(&t.connection_state)
    .bind(&t.packet_loss_percent)
    .bind(t.round_trip_time_ms)
    .bind(t.premature_end)
    .bind(t.prescription_issued)
    .bind(t.captured_at)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn find_by_session<'e, E: PgExecutor<'e>>(
    executor: E,
    session_id: Uuid,
) -> Result<Option<ConsultationTelemetry>, sqlx::Error> {
    sqlx::query_as::<_, ConsultationTelemetry>(
        "SELECT id, session_id, call_duration_seconds, connection_state, packet_loss_percent, round_trip_time_ms, premature_end, prescription_issued, captured_at \
         FROM consultation_telemetry WHERE session_id = $1",
    )
    .bind(session_id)
    .fetch_optional(executor)
    .await
}

/// `ingest_seq` is DB-assigned (`BIGSERIAL`), so it is never bound here —
/// callers should pass `0` in the in-memory struct before insert.
pub async fn insert_telemetry_event<'e, E: PgExecutor<'e>>(
    executor: E,
    e: &ConsultationTelemetryEvent,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO consultation_telemetry_events \
         (id, session_id, event_type, actor_perspective, rtt_ms, packet_loss_percent, occurred_at, created_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
    )
    .bind(e.id)
    .bind(e.session_id)
    .bind(&e.event_type)
    .bind(&e.actor_perspective)
    .bind(e.rtt_ms)
    .bind(&e.packet_loss_percent)
    .bind(e.occurred_at)
    .bind(e.created_at)
    .execute(executor)
    .await?;
    Ok(())
}

/// Ordered by `ingest_seq` (insertion order), not just `occurred_at` — this
/// is what lets timeline reconstruction resolve same-timestamp events
/// deterministically, independent of client clock skew between the patient
/// and doctor apps submitting independently.
pub async fn list_telemetry_events_for_session<'e, E: PgExecutor<'e>>(
    executor: E,
    session_id: Uuid,
) -> Result<Vec<ConsultationTelemetryEvent>, sqlx::Error> {
    sqlx::query_as::<_, ConsultationTelemetryEvent>(
        "SELECT id, ingest_seq, session_id, event_type, actor_perspective, rtt_ms, packet_loss_percent, occurred_at, created_at \
         FROM consultation_telemetry_events WHERE session_id = $1 ORDER BY ingest_seq ASC",
    )
    .bind(session_id)
    .fetch_all(executor)
    .await
}
