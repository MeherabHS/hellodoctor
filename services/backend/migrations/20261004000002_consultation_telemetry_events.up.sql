-- Consultation telemetry EVENT TIMELINE: detailed, ordered, per-perspective
-- (PATIENT/DOCTOR/SERVER) call lifecycle events supplementing the existing
-- single aggregate `consultation_telemetry` row. Powers admin incident
-- reconstruction (reconnect/disconnect duration, technical-complaint
-- correlation) — see docs/CHECKPOINT.md §8.2.
--
-- Deliberately numeric/enum-only — no free-text client-supplied field. This
-- table is pure connection-quality telemetry and must never become a vector
-- for PHI to enter storage (see CLAUDE.md's PHI-logging prohibition). The
-- 'SERVER_ERROR' vocabulary entry is reserved for a future internal writer —
-- nothing populates it in this pass (same honest-scope-cut pattern already
-- used for `system_error_logs.stack_trace`).
CREATE TABLE consultation_telemetry_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ingest_seq BIGSERIAL NOT NULL,
    session_id UUID NOT NULL REFERENCES consultation_sessions(id) ON DELETE CASCADE,
    event_type VARCHAR(40) NOT NULL CHECK (event_type IN (
        'CALL_INITIATED', 'PATIENT_JOINED', 'DOCTOR_JOINED',
        'PATIENT_RECONNECTING', 'PATIENT_RECONNECTED', 'PATIENT_DISCONNECTED',
        'DOCTOR_RECONNECTING', 'DOCTOR_RECONNECTED', 'DOCTOR_DISCONNECTED',
        'NETWORK_DEGRADED', 'NETWORK_RESTORED', 'CALL_ENDED', 'SERVER_ERROR'
    )),
    actor_perspective VARCHAR(10) NOT NULL CHECK (actor_perspective IN ('PATIENT', 'DOCTOR', 'SERVER')),
    rtt_ms INT,
    packet_loss_percent NUMERIC(5, 2),
    occurred_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_consultation_telemetry_events_session ON consultation_telemetry_events(session_id, ingest_seq);

-- Writes go through `hellodoctor_api` — the same role that already owns the
-- aggregate `consultation_telemetry` row (see migrations/20261003000001_rls_hardening) —
-- rather than inventing a sixth DB role.
GRANT SELECT, INSERT ON consultation_telemetry_events TO hellodoctor_api;
-- `ingest_seq BIGSERIAL`'s implicit sequence needs its own grant — table
-- INSERT alone does not cover it. Caught live, not assumed: the first real
-- insert attempt under this role failed with a genuine
-- `permission denied for sequence ..._ingest_seq_seq`.
GRANT USAGE, SELECT ON SEQUENCE consultation_telemetry_events_ingest_seq_seq TO hellodoctor_api;

ALTER TABLE consultation_telemetry_events ENABLE ROW LEVEL SECURITY;
ALTER TABLE consultation_telemetry_events FORCE ROW LEVEL SECURITY;

-- Identical ownership shape to `consultation_telemetry_access` (any edit to
-- one should be mirrored in the other). No explicit WITH CHECK is given, so
-- this same expression governs INSERT too: the backend only ever inserts a
-- row after resolving the caller as a genuine PATIENT/DOCTOR participant in
-- the session (see ConsultationService::record_telemetry_events), inside a
-- transaction where app.current_user_id/app.current_role are already set to
-- that caller's real identity — so the ownership check here is a real,
-- non-bypassable backstop, not a rubber stamp.
CREATE POLICY consultation_telemetry_events_access ON consultation_telemetry_events
USING (
    current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'COMPLIANCE', 'FINANCE_ADMIN', 'SUPPORT')
    OR EXISTS (
        SELECT 1 FROM consultation_sessions cs
        JOIN appointments a ON a.id = cs.appointment_id
        LEFT JOIN patient_profiles p ON p.id = a.patient_id
        LEFT JOIN doctor_profiles d ON d.id = a.doctor_id
        WHERE cs.id = consultation_telemetry_events.session_id
          AND (p.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
               OR d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    )
);
