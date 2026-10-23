-- Provider-confirmed RTC presence is separate from client telemetry.  The
-- provider event id and sequence are the replay/order boundary; no client
-- heartbeat can write these fields.
ALTER TABLE consultation_sessions
    ADD COLUMN recovery_state VARCHAR(20) NOT NULL DEFAULT 'NONE'
        CHECK (recovery_state IN ('NONE', 'GRACE_PERIOD', 'NEEDS_REVIEW')),
    ADD COLUMN empty_room_deadline TIMESTAMPTZ,
    ADD COLUMN patient_present BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN doctor_present BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN patient_presence_sequence BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN doctor_presence_sequence BIGINT NOT NULL DEFAULT 0;

CREATE INDEX idx_consultation_sessions_presence_grace
    ON consultation_sessions (empty_room_deadline)
    WHERE recovery_state = 'GRACE_PERIOD';

CREATE TABLE consultation_provider_presence_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id UUID NOT NULL REFERENCES consultation_sessions(id) ON DELETE CASCADE,
    provider_event_id VARCHAR(200) NOT NULL UNIQUE,
    channel_name VARCHAR(100) NOT NULL,
    participant_uid BIGINT NOT NULL CHECK (participant_uid > 0),
    participant_role VARCHAR(10) NOT NULL CHECK (participant_role IN ('PATIENT', 'DOCTOR')),
    event_type VARCHAR(10) NOT NULL CHECK (event_type IN ('JOINED', 'LEFT')),
    provider_sequence BIGINT NOT NULL CHECK (provider_sequence >= 0),
    provider_event_at TIMESTAMPTZ NOT NULL,
    received_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);

CREATE UNIQUE INDEX uq_presence_session_participant_sequence
    ON consultation_provider_presence_events(session_id, participant_uid, provider_sequence);
CREATE INDEX idx_presence_events_session
    ON consultation_provider_presence_events(session_id, received_at);

-- The callback worker needs only the three participant identities for the
-- locked session. Keep the broad profile tables out of its grants; this
-- SECURITY DEFINER projection is callable only by the system worker and is
-- never exposed as an API read.
CREATE FUNCTION consultation_participant_users(p_session_id UUID)
RETURNS TABLE(patient_user_id UUID, guardian_user_id UUID, doctor_user_id UUID)
LANGUAGE sql STABLE SECURITY DEFINER SET search_path = public AS $$
    SELECT p.user_id, g.user_id, d.user_id
      FROM consultation_sessions cs
      JOIN appointments a ON a.id=cs.appointment_id
      JOIN patient_profiles p ON p.id=a.patient_id
      LEFT JOIN patient_profiles g ON g.id=p.guardian_patient_id
      JOIN doctor_profiles d ON d.id=a.doctor_id
     WHERE cs.id=p_session_id
$$;
REVOKE ALL ON FUNCTION consultation_participant_users(UUID) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION consultation_participant_users(UUID) TO hellodoctor_system_worker;

GRANT SELECT, INSERT ON consultation_provider_presence_events TO hellodoctor_system_worker;
ALTER TABLE consultation_provider_presence_events ENABLE ROW LEVEL SECURITY;
ALTER TABLE consultation_provider_presence_events FORCE ROW LEVEL SECURITY;
CREATE POLICY consultation_provider_presence_events_system_worker_all
    ON consultation_provider_presence_events
    FOR ALL TO hellodoctor_system_worker USING (true) WITH CHECK (true);
