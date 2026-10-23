-- B4 Admin oversight projection and audited resolution records.
-- Provider presence remains immutable evidence written by the system worker;
-- Admins receive read-only access through the same RLS identity used by the
-- existing consultation timeline. Resolution never changes appointment,
-- session or payment state and therefore cannot settle or refund funds.
GRANT SELECT ON consultation_provider_presence_events TO hellodoctor_api;
CREATE POLICY consultation_provider_presence_events_admin_read
    ON consultation_provider_presence_events
    FOR SELECT TO hellodoctor_api
    USING (current_setting('app.current_role', true) = 'ADMIN');

CREATE TABLE consultation_recovery_resolutions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id UUID NOT NULL REFERENCES consultation_sessions(id) ON DELETE RESTRICT,
    outcome VARCHAR(32) NOT NULL CHECK (outcome IN ('RESOLVED_NO_SETTLEMENT', 'PENDING_FINANCE_ACTION')),
    reason TEXT NOT NULL CHECK (char_length(btrim(reason)) BETWEEN 8 AND 1000),
    admin_user_id UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    UNIQUE (session_id)
);
CREATE INDEX idx_consultation_recovery_resolutions_created
    ON consultation_recovery_resolutions(created_at DESC);
GRANT SELECT ON consultation_recovery_resolutions TO hellodoctor_api;

ALTER TABLE consultation_recovery_resolutions ENABLE ROW LEVEL SECURITY;
ALTER TABLE consultation_recovery_resolutions FORCE ROW LEVEL SECURITY;
CREATE POLICY consultation_recovery_resolutions_admin_read
    ON consultation_recovery_resolutions
    FOR SELECT TO hellodoctor_api
    USING (current_setting('app.current_role', true) = 'ADMIN');
-- FORCE RLS also applies to the migration owner. Only the SECURITY DEFINER
-- function below may insert, while the API role has no direct INSERT grant.
CREATE POLICY consultation_recovery_resolutions_internal_insert
    ON consultation_recovery_resolutions
    FOR INSERT TO CURRENT_USER WITH CHECK (true);
CREATE POLICY consultation_recovery_resolutions_system_all
    ON consultation_recovery_resolutions
    FOR ALL TO hellodoctor_system_worker USING (true) WITH CHECK (true);

CREATE OR REPLACE FUNCTION admin_resolve_consultation_recovery(
    p_session_id UUID,
    p_outcome TEXT,
    p_reason TEXT,
    p_admin_user_id UUID
) RETURNS TABLE(resolution_id UUID, result TEXT)
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, public AS $$
DECLARE s consultation_sessions%ROWTYPE;
        existing consultation_recovery_resolutions%ROWTYPE;
BEGIN
    IF current_setting('app.current_role', true) <> 'ADMIN'
       OR p_admin_user_id IS NULL
       OR p_admin_user_id IS DISTINCT FROM NULLIF(current_setting('app.current_user_id', true), '')::uuid THEN
        RAISE EXCEPTION 'ADMIN_REQUIRED' USING ERRCODE = '42501';
    END IF;
    IF p_outcome NOT IN ('RESOLVED_NO_SETTLEMENT', 'PENDING_FINANCE_ACTION')
       OR char_length(btrim(COALESCE(p_reason, ''))) NOT BETWEEN 8 AND 1000 THEN
        RAISE EXCEPTION 'INVALID_RESOLUTION' USING ERRCODE = '22023';
    END IF;

    -- This lock is the same session boundary used by the B4 expiry worker.
    -- A resolution therefore cannot race a transition into NEEDS_REVIEW.
    SELECT * INTO s FROM consultation_sessions WHERE id = p_session_id FOR UPDATE;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'SESSION_NOT_FOUND' USING ERRCODE = 'P0002';
    END IF;
    IF s.recovery_state <> 'NEEDS_REVIEW' OR s.status <> 'PREMATURE_TERMINATION'::session_status_enum THEN
        SELECT * INTO existing FROM consultation_recovery_resolutions WHERE session_id = p_session_id;
        IF FOUND THEN
            IF existing.outcome = p_outcome AND existing.reason = btrim(p_reason) THEN
                RETURN QUERY SELECT existing.id, 'ALREADY_RESOLVED'::text;
                RETURN;
            END IF;
            RAISE EXCEPTION 'RESOLUTION_CONFLICT' USING ERRCODE = '40001';
        END IF;
        RAISE EXCEPTION 'SESSION_NOT_REVIEWABLE' USING ERRCODE = '55000';
    END IF;

    INSERT INTO consultation_recovery_resolutions(session_id, outcome, reason, admin_user_id)
    VALUES (p_session_id, p_outcome, btrim(p_reason), p_admin_user_id)
    ON CONFLICT (session_id) DO NOTHING
    RETURNING id INTO resolution_id;
    IF resolution_id IS NULL THEN
        SELECT * INTO existing FROM consultation_recovery_resolutions WHERE session_id = p_session_id;
        IF existing.outcome = p_outcome AND existing.reason = btrim(p_reason) THEN
            RETURN QUERY SELECT existing.id, 'ALREADY_RESOLVED'::text;
            RETURN;
        END IF;
        RAISE EXCEPTION 'RESOLUTION_CONFLICT' USING ERRCODE = '40001';
    END IF;
    result := 'RESOLVED';
    RETURN NEXT;
END $$;
REVOKE ALL ON FUNCTION admin_resolve_consultation_recovery(UUID,TEXT,TEXT,UUID) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION admin_resolve_consultation_recovery(UUID,TEXT,TEXT,UUID) TO hellodoctor_api;

