-- Peer Navigator foundation (docs/peer-navigator/PEER_NAVIGATOR_PLAN.md,
-- Phase 1): peer profiles with Admin approval, and 10-minute support slots.
-- Peers are their own role and tables, not a kind of doctor, so no doctor
-- rule (KMPDC, earnings, Rx, clinical access windows) applies to them.

CREATE TABLE peer_profiles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL UNIQUE REFERENCES users(id) ON DELETE RESTRICT,
    full_name TEXT NOT NULL CHECK (char_length(btrim(full_name)) BETWEEN 2 AND 120),
    county TEXT NOT NULL CHECK (char_length(county) BETWEEN 2 AND 60),
    organization TEXT CHECK (organization IS NULL OR char_length(organization) BETWEEN 1 AND 150),
    bio TEXT CHECK (bio IS NULL OR char_length(bio) BETWEEN 1 AND 500),
    approval_status TEXT NOT NULL DEFAULT 'PENDING'
        CHECK (approval_status IN ('PENDING', 'APPROVED', 'REJECTED', 'SUSPENDED')),
    approval_note TEXT CHECK (approval_note IS NULL OR char_length(approval_note) BETWEEN 1 AND 500),
    reviewed_by UUID REFERENCES users(id) ON DELETE SET NULL,
    reviewed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
CREATE INDEX idx_peer_profiles_status ON peer_profiles (approval_status, created_at);

CREATE TABLE peer_slots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    peer_id UUID NOT NULL REFERENCES peer_profiles(id) ON DELETE CASCADE,
    start_time TIMESTAMPTZ NOT NULL,
    end_time TIMESTAMPTZ NOT NULL,
    status TEXT NOT NULL DEFAULT 'AVAILABLE'
        CHECK (status IN ('AVAILABLE', 'REQUESTED', 'BOOKED', 'CANCELLED')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    CONSTRAINT peer_slot_time CHECK (end_time > start_time),
    CONSTRAINT peer_slot_no_overlap EXCLUDE USING gist (
        peer_id WITH =,
        tstzrange(start_time, end_time, '[)') WITH &&
    ) WHERE (status <> 'CANCELLED')
);
CREATE INDEX idx_peer_slots_peer_start ON peer_slots (peer_id, start_time);
CREATE INDEX idx_peer_slots_open ON peer_slots (start_time) WHERE status = 'AVAILABLE';

-- A peer profile always belongs to a PEER account, and a profile-bearing
-- account's role cannot change afterwards.
CREATE FUNCTION guard_peer_profile_identity() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, public AS $$
BEGIN
    IF TG_TABLE_NAME = 'users' THEN
        IF NEW.role IS DISTINCT FROM OLD.role
           AND EXISTS (SELECT 1 FROM public.peer_profiles WHERE user_id = NEW.id) THEN
            RAISE EXCEPTION 'Profile-bearing account role is immutable' USING ERRCODE = '23514';
        END IF;
        RETURN NEW;
    END IF;
    IF TG_OP = 'UPDATE' AND (NEW.user_id IS DISTINCT FROM OLD.user_id OR NEW.id IS DISTINCT FROM OLD.id) THEN
        RAISE EXCEPTION 'Canonical profile identity is immutable' USING ERRCODE = '23514';
    END IF;
    PERFORM 1 FROM public.users WHERE id = NEW.user_id AND role::text = 'PEER' FOR SHARE;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'Profile must match canonical account role' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER peer_profile_identity BEFORE INSERT OR UPDATE ON peer_profiles
    FOR EACH ROW EXECUTE FUNCTION guard_peer_profile_identity();
CREATE TRIGGER canonical_peer_account_role BEFORE UPDATE OF role ON users
    FOR EACH ROW EXECUTE FUNCTION guard_peer_profile_identity();

-- Only an Admin decides approval; a peer may edit their own profile text.
CREATE FUNCTION guard_peer_approval_fields() RETURNS trigger
LANGUAGE plpgsql SET search_path = pg_catalog, public AS $$
BEGIN
    IF current_setting('app.current_role', true) IS DISTINCT FROM 'ADMIN'
       AND (NEW.approval_status IS DISTINCT FROM OLD.approval_status
            OR NEW.approval_note IS DISTINCT FROM OLD.approval_note
            OR NEW.reviewed_by IS DISTINCT FROM OLD.reviewed_by
            OR NEW.reviewed_at IS DISTINCT FROM OLD.reviewed_at) THEN
        RAISE EXCEPTION 'Only an Admin can change peer approval' USING ERRCODE = '42501';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER peer_approval_fields BEFORE UPDATE ON peer_profiles
    FOR EACH ROW EXECUTE FUNCTION guard_peer_approval_fields();

-- The caller's own peer profile id, evaluated once per statement.
CREATE FUNCTION fn_current_peer_profile_id() RETURNS uuid
LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public AS $$
    SELECT p.id FROM public.peer_profiles p
    WHERE p.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
$$;
REVOKE ALL ON FUNCTION fn_current_peer_profile_id() FROM PUBLIC;
GRANT EXECUTE ON FUNCTION fn_current_peer_profile_id() TO hellodoctor_api;

ALTER TABLE peer_profiles ENABLE ROW LEVEL SECURITY;
ALTER TABLE peer_profiles FORCE ROW LEVEL SECURITY;
-- Own row, Admins, and patients browsing approved peers (handlers select
-- only display columns for patients).
CREATE POLICY peer_profiles_read ON peer_profiles FOR SELECT TO hellodoctor_api USING (
    user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
    OR current_setting('app.current_role', true) = 'ADMIN'
    OR (current_setting('app.current_role', true) = 'PATIENT' AND approval_status = 'APPROVED'));
CREATE POLICY peer_profiles_update ON peer_profiles FOR UPDATE TO hellodoctor_api USING (
    user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
    OR current_setting('app.current_role', true) = 'ADMIN') WITH CHECK (
    user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
    OR current_setting('app.current_role', true) = 'ADMIN');
GRANT SELECT, UPDATE ON peer_profiles TO hellodoctor_api;
-- Sign-up and sign-in run as the auth worker.
CREATE POLICY peer_profiles_auth_read ON peer_profiles FOR SELECT TO hellodoctor_auth_worker USING (true);
CREATE POLICY peer_profiles_auth_insert ON peer_profiles FOR INSERT TO hellodoctor_auth_worker WITH CHECK (
    user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid);
GRANT SELECT, INSERT ON peer_profiles TO hellodoctor_auth_worker;

ALTER TABLE peer_slots ENABLE ROW LEVEL SECURITY;
ALTER TABLE peer_slots FORCE ROW LEVEL SECURITY;
CREATE POLICY peer_slots_read ON peer_slots FOR SELECT TO hellodoctor_api USING (
    peer_id = fn_current_peer_profile_id()
    OR current_setting('app.current_role', true) = 'ADMIN'
    OR (current_setting('app.current_role', true) = 'PATIENT' AND status = 'AVAILABLE'
        AND EXISTS (SELECT 1 FROM peer_profiles p WHERE p.id = peer_slots.peer_id AND p.approval_status = 'APPROVED')));
-- Only an approved peer manages their own slots.
CREATE POLICY peer_slots_owner_write ON peer_slots FOR INSERT TO hellodoctor_api WITH CHECK (
    peer_id = fn_current_peer_profile_id()
    AND EXISTS (SELECT 1 FROM peer_profiles p WHERE p.id = peer_slots.peer_id AND p.approval_status = 'APPROVED'));
CREATE POLICY peer_slots_owner_update ON peer_slots FOR UPDATE TO hellodoctor_api
    USING (peer_id = fn_current_peer_profile_id())
    WITH CHECK (peer_id = fn_current_peer_profile_id());
GRANT SELECT, INSERT, UPDATE ON peer_slots TO hellodoctor_api;

-- Peer sign-in codes go through the same must-deliver OTP outbox.
ALTER TABLE email_outbox DROP CONSTRAINT email_outbox_email_type_check;
ALTER TABLE email_outbox ADD CONSTRAINT email_outbox_email_type_check
    CHECK (email_type IN (
        'PATIENT_OTP', 'DOCTOR_OTP', 'ADMIN_OTP', 'PASSWORD_RESET_OTP', 'PEER_OTP',
        'PATIENT_WELCOME',
        'DOCTOR_APPLICATION_RECEIVED', 'DOCTOR_APPROVED', 'DOCTOR_REJECTED',
        'CONSULTATION_CONFIRMED', 'CONSULTATION_RESCHEDULED',
        'PAYMENT_CONFIRMED', 'REFUND_PROCESSED',
        'PASSWORD_CHANGED',
        'ADMIN_INVITATION'
    ));
