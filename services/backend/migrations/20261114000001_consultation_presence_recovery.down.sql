DROP POLICY IF EXISTS consultation_provider_presence_events_system_worker_all
    ON consultation_provider_presence_events;
REVOKE ALL ON FUNCTION consultation_participant_users(UUID) FROM hellodoctor_system_worker;
DROP FUNCTION IF EXISTS consultation_participant_users(UUID);
ALTER TABLE consultation_provider_presence_events NO FORCE ROW LEVEL SECURITY;
ALTER TABLE consultation_provider_presence_events DISABLE ROW LEVEL SECURITY;
DROP TABLE IF EXISTS consultation_provider_presence_events;
DROP INDEX IF EXISTS idx_consultation_sessions_presence_grace;
ALTER TABLE consultation_sessions
    DROP COLUMN IF EXISTS doctor_presence_sequence,
    DROP COLUMN IF EXISTS patient_presence_sequence,
    DROP COLUMN IF EXISTS doctor_present,
    DROP COLUMN IF EXISTS patient_present,
    DROP COLUMN IF EXISTS empty_room_deadline,
    DROP COLUMN IF EXISTS recovery_state;
