DROP FUNCTION IF EXISTS purge_ended_auth_sessions(integer, integer);
DROP INDEX IF EXISTS appointment_reschedules_auth_session_id_idx;
DROP INDEX IF EXISTS consultation_trace_events_auth_session_id_idx;
DROP INDEX IF EXISTS audit_events_session_id_idx;
