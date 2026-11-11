use crate::domain::models::{
    ConsultationSession, ConsultationTelemetry, ConsultationTelemetryEvent, SessionStatus,
};
use sqlx::PgExecutor;
use uuid::Uuid;

/// M8-G-30: how long a *mint-time-only*, not-yet-provider-confirmed
/// reservation claim lives before it self-expires and becomes reclaimable.
/// Deliberately short and independent of the Agora token's own (up to
/// one-hour) cryptographic expiry — see
/// docs/MILESTONE_8_SINGLE_LIVE_CONSULTATION_DESIGN.md §5 for why reusing
/// the token's own TTL here was considered and rejected. This only deters
/// an accidental near-simultaneous double-mint; it is not, and must never be
/// treated as, proof that a participant has actually left Agora.
pub const RESERVATION_PENDING_GRACE_SECS: i64 = 120;

pub async fn insert_session<'e, E: PgExecutor<'e>>(
    executor: E,
    session: &ConsultationSession,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO consultation_sessions (id, appointment_id, agora_channel_name, started_at, ended_at, status, created_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7) ON CONFLICT (appointment_id) DO NOTHING",
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
        "SELECT id, appointment_id, agora_channel_name, started_at, ended_at, status, created_at, recovery_state, empty_room_deadline, patient_present, doctor_present, patient_presence_sequence, doctor_presence_sequence FROM consultation_sessions WHERE appointment_id = $1",
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
        "SELECT id, appointment_id, agora_channel_name, started_at, ended_at, status, created_at, recovery_state, empty_room_deadline, patient_present, doctor_present, patient_presence_sequence, doctor_presence_sequence FROM consultation_sessions WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(executor)
    .await
}

pub async fn list_active<'e, E: PgExecutor<'e>>(
    executor: E,
) -> Result<Vec<ConsultationSession>, sqlx::Error> {
    sqlx::query_as::<_, ConsultationSession>(
        "SELECT id, appointment_id, agora_channel_name, started_at, ended_at, status, created_at, recovery_state, empty_room_deadline, patient_present, doctor_present, patient_presence_sequence, doctor_presence_sequence FROM consultation_sessions WHERE status IN ('ACTIVE', 'INITIALIZED')",
    )
    .fetch_all(executor)
    .await
}

/// Sessions visible in the Admin command monitor. Interrupted sessions stay
/// discoverable until their review decision is recorded; they must not be
/// hidden just because the RTC channel is no longer active.
pub async fn list_monitoring<'e, E: PgExecutor<'e>>(
    executor: E,
) -> Result<Vec<ConsultationSession>, sqlx::Error> {
    sqlx::query_as::<_, ConsultationSession>(
        "SELECT id, appointment_id, agora_channel_name, started_at, ended_at, created_at, status, recovery_state, empty_room_deadline, patient_present, doctor_present, patient_presence_sequence, doctor_presence_sequence
         FROM consultation_sessions
         WHERE status IN ('ACTIVE', 'INITIALIZED', 'PREMATURE_TERMINATION')
            OR recovery_state = 'NEEDS_REVIEW'
         ORDER BY (recovery_state = 'NEEDS_REVIEW') DESC, COALESCE(ended_at, started_at, created_at) DESC
         LIMIT 200",
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
        "SELECT id, appointment_id, agora_channel_name, started_at, ended_at, status, created_at, recovery_state, empty_room_deadline, patient_present, doctor_present, patient_presence_sequence, doctor_presence_sequence FROM consultation_sessions",
    )
    .fetch_all(executor)
    .await
}

/// M8-G-20: scoped alternative to `list_all` for `list_admin_grievances`,
/// which only ever needs the sessions actually referenced by the (typically
/// small) grievance list currently being displayed — mirrors
/// `appointment_repo::find_by_ids`'s established rationale and shape.
pub async fn find_by_ids<'e, E: PgExecutor<'e>>(
    executor: E,
    ids: &[Uuid],
) -> Result<Vec<ConsultationSession>, sqlx::Error> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    sqlx::query_as::<_, ConsultationSession>(
        "SELECT id, appointment_id, agora_channel_name, started_at, ended_at, status, created_at, recovery_state, empty_room_deadline, patient_present, doctor_present, patient_presence_sequence, doctor_presence_sequence FROM consultation_sessions WHERE id = ANY($1)",
    )
    .bind(ids)
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

/// M8-G-30 mint-time acquire/refresh. Atomic: a single `INSERT ... ON
/// CONFLICT ... WHERE ... RETURNING` that can never raise a unique-violation
/// error for the "someone else already holds it" case — the `WHERE` clause
/// makes that outcome a deterministic zero-row result instead of a database
/// error (docs/MILESTONE_8_SINGLE_LIVE_CONSULTATION_DESIGN.md §4). A
/// same-appointment re-mint against an existing `CONFIRMED` row is always
/// admitted as a no-op that never downgrades its state or shrinks its
/// `expires_at` (§4 issue 2 fix) — only a same-appointment `PENDING` row is
/// refreshed forward. Returns `true` if the caller now legitimately holds
/// the single reservation for `user_id`.
pub async fn acquire_live_reservation<'e, E: PgExecutor<'e>>(
    executor: E,
    user_id: Uuid,
    appointment_id: Uuid,
    session_id: Uuid,
    pending_grace_secs: i64,
) -> Result<bool, sqlx::Error> {
    let acquired: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO consultation_live_reservations \
             (user_id, appointment_id, session_id, state, acquired_at, expires_at, updated_at) \
         VALUES ($1,$2,$3,'PENDING',clock_timestamp(), \
                 clock_timestamp() + make_interval(secs => $4), clock_timestamp()) \
         ON CONFLICT (user_id) DO UPDATE SET \
             appointment_id = EXCLUDED.appointment_id, \
             session_id     = EXCLUDED.session_id, \
             state      = CASE WHEN consultation_live_reservations.appointment_id = EXCLUDED.appointment_id \
                                 AND consultation_live_reservations.state = 'CONFIRMED' \
                            THEN consultation_live_reservations.state ELSE 'PENDING' END, \
             expires_at = CASE WHEN consultation_live_reservations.appointment_id = EXCLUDED.appointment_id \
                                 AND consultation_live_reservations.state = 'CONFIRMED' \
                            THEN consultation_live_reservations.expires_at ELSE EXCLUDED.expires_at END, \
             updated_at = clock_timestamp() \
         WHERE consultation_live_reservations.appointment_id = EXCLUDED.appointment_id \
            OR (consultation_live_reservations.state = 'PENDING' AND consultation_live_reservations.expires_at <= clock_timestamp()) \
            OR (consultation_live_reservations.state = 'CONFIRMED' AND consultation_live_reservations.expires_at <= clock_timestamp()) \
            OR EXISTS (SELECT 1 FROM consultation_sessions cs WHERE cs.id = consultation_live_reservations.session_id AND cs.status = 'COMPLETED') \
            OR EXISTS (SELECT 1 FROM appointments a WHERE a.id = consultation_live_reservations.appointment_id AND a.status IN ('CANCELLED','EXPIRED')) \
         RETURNING user_id",
    )
    .bind(user_id)
    .bind(appointment_id)
    .bind(session_id)
    .bind(pending_grace_secs)
    .fetch_optional(executor)
    .await?;
    Ok(acquired.is_some())
}

/// M8-G-30 webhook-path upgrade to `CONFIRMED` on an accepted, provider-
/// trusted Joined event — same atomic shape as `acquire_live_reservation`,
/// and likewise never downgrades a same-session row's own freshly-computed
/// backstop. Returns `true` if this session now legitimately holds the
/// reservation. `false` means a genuine, unresolved conflict with a
/// *different*, still-live session exists for this `user_id` — the caller
/// (`apply_provider_event`) must not treat this as an error: this session's
/// own presence is still recorded normally (the join is physically real),
/// and a `CONCURRENT_LIVE_PARTICIPATION_DETECTED` audit event is written
/// instead (design doc §6) — this function only reports the fact, callers
/// decide what to do about it.
pub async fn confirm_live_reservation<'e, E: PgExecutor<'e>>(
    executor: E,
    user_id: Uuid,
    appointment_id: Uuid,
    session_id: Uuid,
    backstop_secs: i64,
) -> Result<bool, sqlx::Error> {
    let confirmed: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO consultation_live_reservations \
             (user_id, appointment_id, session_id, state, acquired_at, expires_at, updated_at) \
         VALUES ($1,$2,$3,'CONFIRMED',clock_timestamp(), \
                 clock_timestamp() + make_interval(secs => $4), clock_timestamp()) \
         ON CONFLICT (user_id) DO UPDATE SET \
             appointment_id = EXCLUDED.appointment_id, \
             session_id     = EXCLUDED.session_id, \
             state          = 'CONFIRMED', \
             expires_at     = EXCLUDED.expires_at, \
             updated_at     = clock_timestamp() \
         WHERE consultation_live_reservations.appointment_id = EXCLUDED.appointment_id \
            OR (consultation_live_reservations.state = 'PENDING' AND consultation_live_reservations.expires_at <= clock_timestamp()) \
            OR (consultation_live_reservations.state = 'CONFIRMED' AND consultation_live_reservations.expires_at <= clock_timestamp()) \
            OR EXISTS (SELECT 1 FROM consultation_sessions cs WHERE cs.id = consultation_live_reservations.session_id AND cs.status = 'COMPLETED') \
            OR EXISTS (SELECT 1 FROM appointments a WHERE a.id = consultation_live_reservations.appointment_id AND a.status IN ('CANCELLED','EXPIRED')) \
         RETURNING user_id",
    )
    .bind(user_id)
    .bind(appointment_id)
    .bind(session_id)
    .bind(backstop_secs)
    .fetch_optional(executor)
    .await?;
    Ok(confirmed.is_some())
}

/// M8-G-30 release on an accepted, provider-trusted Left event. Scoped to
/// `session_id` so it can never delete a reservation that legitimately
/// points at a *different* session for this user (the §6 conflict case) —
/// only this exact (user, session) pair's own reservation is ever removed
/// here.
pub async fn release_live_reservation<'e, E: PgExecutor<'e>>(
    executor: E,
    user_id: Uuid,
    session_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM consultation_live_reservations WHERE user_id=$1 AND session_id=$2")
        .bind(user_id)
        .bind(session_id)
        .execute(executor)
        .await?;
    Ok(())
}
