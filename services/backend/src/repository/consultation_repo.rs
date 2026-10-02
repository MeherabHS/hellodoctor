use crate::domain::models::{ConsultationSession, ConsultationTelemetry, SessionStatus};
use sqlx::PgExecutor;
use uuid::Uuid;

pub async fn insert_session<'e, E: PgExecutor<'e>>(executor: E, session: &ConsultationSession) -> Result<(), sqlx::Error> {
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

pub async fn find_by_appointment<'e, E: PgExecutor<'e>>(executor: E, appointment_id: Uuid) -> Result<Option<ConsultationSession>, sqlx::Error> {
    sqlx::query_as::<_, ConsultationSession>(
        "SELECT id, appointment_id, agora_channel_name, started_at, ended_at, status, created_at FROM consultation_sessions WHERE appointment_id = $1",
    )
    .bind(appointment_id)
    .fetch_optional(executor)
    .await
}

pub async fn find_by_id<'e, E: PgExecutor<'e>>(executor: E, id: Uuid) -> Result<Option<ConsultationSession>, sqlx::Error> {
    sqlx::query_as::<_, ConsultationSession>(
        "SELECT id, appointment_id, agora_channel_name, started_at, ended_at, status, created_at FROM consultation_sessions WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(executor)
    .await
}

pub async fn list_active<'e, E: PgExecutor<'e>>(executor: E) -> Result<Vec<ConsultationSession>, sqlx::Error> {
    sqlx::query_as::<_, ConsultationSession>(
        "SELECT id, appointment_id, agora_channel_name, started_at, ended_at, status, created_at FROM consultation_sessions WHERE status IN ('ACTIVE', 'INITIALIZED')",
    )
    .fetch_all(executor)
    .await
}

pub async fn set_status<'e, E: PgExecutor<'e>>(executor: E, id: Uuid, status: SessionStatus) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE consultation_sessions SET status = $2 WHERE id = $1")
        .bind(id)
        .bind(status)
        .execute(executor)
        .await?;
    Ok(())
}

pub async fn insert_telemetry<'e, E: PgExecutor<'e>>(executor: E, t: &ConsultationTelemetry) -> Result<(), sqlx::Error> {
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

pub async fn find_by_session<'e, E: PgExecutor<'e>>(executor: E, session_id: Uuid) -> Result<Option<ConsultationTelemetry>, sqlx::Error> {
    sqlx::query_as::<_, ConsultationTelemetry>(
        "SELECT id, session_id, call_duration_seconds, connection_state, packet_loss_percent, round_trip_time_ms, premature_end, prescription_issued, captured_at \
         FROM consultation_telemetry WHERE session_id = $1",
    )
    .bind(session_id)
    .fetch_optional(executor)
    .await
}
