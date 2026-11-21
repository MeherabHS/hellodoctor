-- Security-data retention purge (product owner, 2026-11-21: "security data
-- only"). Approved periods: expired sign-in/OTP/MFA/password-reset codes are
-- deleted 7 days after expiry; ended sessions 90 days after they ended.
-- Nothing clinical, financial, chat, grievance or audit is ever touched.
--
-- Codes are deleted by the auth worker directly (it already holds DELETE on
-- those tables). Sessions need this narrowly scoped function instead of a
-- table-wide DELETE grant: it removes only sessions that ended more than the
-- approved period ago and that no permanent record (audit log, consultation
-- trace, reschedule) still points at, then the device keys of families left
-- with no session. It refuses a period shorter than the approved 90 days.

CREATE INDEX IF NOT EXISTS audit_events_session_id_idx
    ON audit_events (session_id) WHERE session_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS consultation_trace_events_auth_session_id_idx
    ON consultation_trace_events (auth_session_id) WHERE auth_session_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS appointment_reschedules_auth_session_id_idx
    ON appointment_reschedules (auth_session_id) WHERE auth_session_id IS NOT NULL;

CREATE OR REPLACE FUNCTION purge_ended_auth_sessions(retain_days integer, max_rows integer)
RETURNS integer
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public, pg_catalog
AS $$
DECLARE
    families uuid[];
    removed integer;
BEGIN
    IF retain_days IS NULL OR retain_days < 90 THEN
        RAISE EXCEPTION 'session retention below the approved 90 days';
    END IF;
    IF max_rows IS NULL OR max_rows < 1 OR max_rows > 5000 THEN
        RAISE EXCEPTION 'max_rows must be between 1 and 5000';
    END IF;

    WITH victims AS (
        SELECT s.id
        FROM auth_sessions s
        WHERE LEAST(COALESCE(s.revoked_at, 'infinity'::timestamptz),
                    COALESCE(s.rotated_at, 'infinity'::timestamptz),
                    s.expires_at) < now() - make_interval(days => retain_days)
          AND NOT EXISTS (SELECT 1 FROM audit_events a WHERE a.session_id = s.id)
          AND NOT EXISTS (SELECT 1 FROM consultation_trace_events t WHERE t.auth_session_id = s.id)
          AND NOT EXISTS (SELECT 1 FROM appointment_reschedules r WHERE r.auth_session_id = s.id)
        LIMIT max_rows
        FOR UPDATE SKIP LOCKED
    ), gone AS (
        DELETE FROM auth_sessions s USING victims v
        WHERE s.id = v.id
        RETURNING s.token_family_id
    )
    SELECT array_agg(DISTINCT token_family_id), count(*) INTO families, removed FROM gone;

    IF removed > 0 THEN
        DELETE FROM auth_session_device_keys k
        WHERE k.token_family_id = ANY (families)
          AND NOT EXISTS (SELECT 1 FROM auth_sessions s WHERE s.token_family_id = k.token_family_id);
    END IF;
    RETURN COALESCE(removed, 0);
END;
$$;

REVOKE ALL ON FUNCTION purge_ended_auth_sessions(integer, integer) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION purge_ended_auth_sessions(integer, integer) TO hellodoctor_auth_worker;
