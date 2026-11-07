-- M8-G-30: single live-consultation reservation per authenticated user_id,
-- regardless of role (Doctor/Patient/guardian) — see
-- docs/MILESTONE_8_SINGLE_LIVE_CONSULTATION_DESIGN.md (Revision 2, approved)
-- for the full design, limitations and what is and is not guaranteed by
-- this table alone. `user_id` as the PRIMARY KEY makes "at most one live
-- reservation per user" database-enforced, not an application convention.
CREATE TABLE consultation_live_reservations (
    user_id        UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    appointment_id UUID NOT NULL REFERENCES appointments(id) ON DELETE CASCADE,
    session_id     UUID NOT NULL REFERENCES consultation_sessions(id) ON DELETE CASCADE,
    state          VARCHAR(10) NOT NULL CHECK (state IN ('PENDING','CONFIRMED')),
    acquired_at    TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    expires_at     TIMESTAMPTZ NOT NULL,
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);

COMMENT ON TABLE consultation_live_reservations IS
    'PENDING: mint-time, local-only, self-expiring in a short window (design
     doc RESERVATION_PENDING_GRACE_SECS) -- deters accidental near-simultaneous
     double-minting only, NOT proof of an active call. CONFIRMED: upgraded
     only by a trusted Agora provider presence webhook event, released on a
     trusted Left event or a long emergency backstop (design doc
     consultation_live_reservation_max_secs) that guards only against a lost
     Left webhook, not a maximum consultation duration. Does not, and cannot,
     prevent an already-admitted Agora RTC connection from continuing --
     see the design doc Section 6.';

CREATE INDEX idx_consultation_live_reservations_session
    ON consultation_live_reservations(session_id);
-- Supports the emergency-backstop sweep/audit query without a sequential scan.
CREATE INDEX idx_consultation_live_reservations_expiry
    ON consultation_live_reservations(expires_at);

-- Mint-time acquisition runs as the ordinary authenticated API role.
GRANT SELECT, INSERT, UPDATE, DELETE ON consultation_live_reservations TO hellodoctor_api;
-- Webhook-path confirm/release/conflict-handling runs as the system worker,
-- the same role `apply_provider_event` already uses for presence tables.
GRANT SELECT, INSERT, UPDATE, DELETE ON consultation_live_reservations TO hellodoctor_system_worker;

ALTER TABLE consultation_live_reservations ENABLE ROW LEVEL SECURITY;
ALTER TABLE consultation_live_reservations FORCE ROW LEVEL SECURITY;

-- A caller may only ever touch their OWN reservation row via the API role --
-- this is the row-level backstop behind the application-level identity
-- resolution in mint_agora_token (ctx.user_id) and apply_provider_event
-- (patient_user_id/guardian_user_id/doctor_user_id resolved from the locked
-- session, never client-supplied).
CREATE POLICY consultation_live_reservations_self_all
    ON consultation_live_reservations
    FOR ALL TO hellodoctor_api
    USING (user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    WITH CHECK (user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid);

-- Admin read access, for the observability the design requires ("backstop
-- expiry should be auditable") -- same scoping as the existing consultation
-- recovery oversight read policy.
CREATE POLICY consultation_live_reservations_admin_read
    ON consultation_live_reservations
    FOR SELECT TO hellodoctor_api
    USING (current_setting('app.current_role', true) = 'ADMIN');

-- System worker (webhook path) has unrestricted access to confirm/release/
-- flag conflicts -- identical scoping to consultation_provider_presence_events.
CREATE POLICY consultation_live_reservations_system_all
    ON consultation_live_reservations
    FOR ALL TO hellodoctor_system_worker USING (true) WITH CHECK (true);

-- FORCE RLS also applies to the migration owner, so the SECURITY DEFINER
-- function below (which locks and deletes a row while executing as its
-- owner, i.e. CURRENT_USER at migration time) needs its own policy --
-- identical rationale and shape to
-- consultation_recovery_resolutions_internal_insert in migration
-- 20261116000001_admin_consultation_recovery_oversight.
CREATE POLICY consultation_live_reservations_internal_admin_release
    ON consultation_live_reservations
    FOR ALL TO CURRENT_USER USING (true) WITH CHECK (true);

-- Narrow Admin manual release -- does not touch consultation, appointment,
-- or financial state. Self-attributes to the calling admin exactly like
-- admin_resolve_consultation_recovery does (migration
-- 20261116000001_admin_consultation_recovery_oversight), which this mirrors.
-- The audit row itself is written by the calling Rust handler via the
-- existing AuditService::log_in_tx (same split of responsibility already
-- used by resolve_consultation_recovery: the DB function performs the
-- mutation and reports what it did; the handler records it in audit_events
-- using the already-correct, maintained Rust helper rather than this
-- migration guessing that table's current full column set).
CREATE OR REPLACE FUNCTION admin_release_live_reservation(
    p_user_id UUID,
    p_reason TEXT,
    p_admin_user_id UUID
) RETURNS TABLE(released BOOLEAN, result TEXT, released_appointment_id UUID, released_session_id UUID)
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, public AS $$
DECLARE existing consultation_live_reservations%ROWTYPE;
BEGIN
    IF current_setting('app.current_role', true) <> 'ADMIN'
       OR p_admin_user_id IS NULL
       OR p_admin_user_id IS DISTINCT FROM NULLIF(current_setting('app.current_user_id', true), '')::uuid THEN
        RAISE EXCEPTION 'ADMIN_REQUIRED' USING ERRCODE = '42501';
    END IF;
    IF char_length(btrim(COALESCE(p_reason, ''))) NOT BETWEEN 8 AND 1000 THEN
        RAISE EXCEPTION 'INVALID_REASON' USING ERRCODE = '22023';
    END IF;

    SELECT * INTO existing FROM consultation_live_reservations WHERE user_id = p_user_id FOR UPDATE;
    IF NOT FOUND THEN
        released := false;
        result := 'NO_RESERVATION';
        released_appointment_id := NULL;
        released_session_id := NULL;
        RETURN NEXT;
        RETURN;
    END IF;

    DELETE FROM consultation_live_reservations WHERE user_id = p_user_id;
    released := true;
    result := 'RELEASED';
    released_appointment_id := existing.appointment_id;
    released_session_id := existing.session_id;
    RETURN NEXT;
END $$;
REVOKE ALL ON FUNCTION admin_release_live_reservation(UUID,TEXT,UUID) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION admin_release_live_reservation(UUID,TEXT,UUID) TO hellodoctor_api;
