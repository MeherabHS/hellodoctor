-- Peer Navigator, Phase 2 (docs/peer-navigator/PEER_NAVIGATOR_PLAN.md):
-- a patient requests one of a peer's open slots; the peer accepts or
-- declines; the patient may cancel while it is pending. The slot moves
-- AVAILABLE -> REQUESTED -> BOOKED (accepted) or back to AVAILABLE.
--
-- Privacy: the peer sees only a first-name-and-initial display name and an
-- age range, snapshotted from the patient's own profile when the request is
-- made, plus the topic the patient wrote. Peers never read patient records.

CREATE TABLE peer_requests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    patient_id UUID NOT NULL REFERENCES patient_profiles(id) ON DELETE RESTRICT,
    patient_user_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    peer_id UUID NOT NULL REFERENCES peer_profiles(id) ON DELETE RESTRICT,
    peer_user_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    slot_id UUID NOT NULL REFERENCES peer_slots(id) ON DELETE RESTRICT,
    support_type TEXT NOT NULL CHECK (support_type IN ('CHAT', 'CALL')),
    topic TEXT NOT NULL CHECK (char_length(topic) BETWEEN 3 AND 300),
    patient_display_name TEXT NOT NULL CHECK (char_length(patient_display_name) BETWEEN 1 AND 40),
    patient_age_range TEXT CHECK (patient_age_range IS NULL OR char_length(patient_age_range) <= 10),
    status TEXT NOT NULL DEFAULT 'PENDING'
        CHECK (status IN ('PENDING', 'ACCEPTED', 'DECLINED', 'CANCELLED', 'COMPLETED')),
    decline_note TEXT CHECK (decline_note IS NULL OR char_length(decline_note) BETWEEN 1 AND 300),
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    decided_at TIMESTAMPTZ,
    cancelled_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ
);
-- One live request per slot; declined and cancelled requests free it.
CREATE UNIQUE INDEX peer_requests_one_live_per_slot ON peer_requests (slot_id)
    WHERE status IN ('PENDING', 'ACCEPTED', 'COMPLETED');
CREATE INDEX idx_peer_requests_peer ON peer_requests (peer_user_id, status, created_at DESC);
CREATE INDEX idx_peer_requests_patient ON peer_requests (patient_user_id, created_at DESC);

-- Only the allowed transitions, by the allowed party; request details are
-- immutable once made.
CREATE FUNCTION guard_peer_request_update() RETURNS trigger
LANGUAGE plpgsql SET search_path = pg_catalog, public AS $$
DECLARE
    caller uuid := NULLIF(current_setting('app.current_user_id', true), '')::uuid;
    caller_role text := current_setting('app.current_role', true);
BEGIN
    IF (NEW.patient_id, NEW.patient_user_id, NEW.peer_id, NEW.peer_user_id, NEW.slot_id,
        NEW.support_type, NEW.topic, NEW.patient_display_name, NEW.patient_age_range, NEW.created_at)
       IS DISTINCT FROM
       (OLD.patient_id, OLD.patient_user_id, OLD.peer_id, OLD.peer_user_id, OLD.slot_id,
        OLD.support_type, OLD.topic, OLD.patient_display_name, OLD.patient_age_range, OLD.created_at) THEN
        RAISE EXCEPTION 'Peer request details are immutable' USING ERRCODE = '42501';
    END IF;
    IF NEW.status = OLD.status THEN
        RETURN NEW;
    END IF;
    IF caller_role = 'PATIENT' AND caller = OLD.patient_user_id
       AND OLD.status = 'PENDING' AND NEW.status = 'CANCELLED' THEN
        RETURN NEW;
    END IF;
    IF caller_role = 'PEER' AND caller = OLD.peer_user_id AND (
           (OLD.status = 'PENDING' AND NEW.status IN ('ACCEPTED', 'DECLINED'))
        OR (OLD.status = 'ACCEPTED' AND NEW.status = 'COMPLETED')) THEN
        RETURN NEW;
    END IF;
    RAISE EXCEPTION 'Peer request transition % -> % is not allowed', OLD.status, NEW.status
        USING ERRCODE = '42501';
END $$;
CREATE TRIGGER peer_request_transition BEFORE UPDATE ON peer_requests
    FOR EACH ROW EXECUTE FUNCTION guard_peer_request_update();

ALTER TABLE peer_requests ENABLE ROW LEVEL SECURITY;
ALTER TABLE peer_requests FORCE ROW LEVEL SECURITY;
CREATE POLICY peer_requests_parties_read ON peer_requests FOR SELECT TO hellodoctor_api USING (
    patient_user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
    OR peer_user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid);
CREATE POLICY peer_requests_patient_insert ON peer_requests FOR INSERT TO hellodoctor_api WITH CHECK (
    current_setting('app.current_role', true) = 'PATIENT'
    AND patient_user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
    AND status = 'PENDING'
    AND fn_patient_profile_owned_by_caller(patient_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    AND EXISTS (SELECT 1 FROM peer_profiles p
                WHERE p.id = peer_requests.peer_id AND p.user_id = peer_requests.peer_user_id
                  AND p.approval_status = 'APPROVED'));
CREATE POLICY peer_requests_parties_update ON peer_requests FOR UPDATE TO hellodoctor_api
    USING (patient_user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
        OR peer_user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    WITH CHECK (patient_user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
        OR peer_user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid);
GRANT SELECT, INSERT, UPDATE ON peer_requests TO hellodoctor_api;

-- Slots: a patient may claim an open slot (AVAILABLE -> REQUESTED) and free a
-- slot they requested once their request is cancelled. The peer moves their
-- own slot REQUESTED -> BOOKED (accept) or back to AVAILABLE (decline), and
-- may cancel an open slot. Times never change after creation.
CREATE FUNCTION guard_peer_slot_update() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, public AS $$
DECLARE
    caller uuid := NULLIF(current_setting('app.current_user_id', true), '')::uuid;
    caller_role text := current_setting('app.current_role', true);
    owner_user uuid;
BEGIN
    IF (NEW.peer_id, NEW.start_time, NEW.end_time, NEW.created_at)
       IS DISTINCT FROM (OLD.peer_id, OLD.start_time, OLD.end_time, OLD.created_at) THEN
        RAISE EXCEPTION 'Peer slot times are immutable' USING ERRCODE = '42501';
    END IF;
    IF NEW.status = OLD.status THEN
        RETURN NEW;
    END IF;
    SELECT user_id INTO owner_user FROM public.peer_profiles WHERE id = OLD.peer_id;
    IF caller_role = 'PEER' AND caller = owner_user AND (
           (OLD.status = 'AVAILABLE' AND NEW.status = 'CANCELLED')
        OR (OLD.status = 'REQUESTED' AND NEW.status IN ('BOOKED', 'AVAILABLE'))) THEN
        RETURN NEW;
    END IF;
    IF caller_role = 'PATIENT' AND OLD.status = 'AVAILABLE' AND NEW.status = 'REQUESTED' THEN
        RETURN NEW;
    END IF;
    IF caller_role = 'PATIENT' AND OLD.status = 'REQUESTED' AND NEW.status = 'AVAILABLE'
       AND EXISTS (SELECT 1 FROM public.peer_requests r
                   WHERE r.slot_id = OLD.id AND r.patient_user_id = caller AND r.status = 'CANCELLED')
       AND NOT EXISTS (SELECT 1 FROM public.peer_requests r
                       WHERE r.slot_id = OLD.id AND r.status IN ('PENDING', 'ACCEPTED', 'COMPLETED')) THEN
        RETURN NEW;
    END IF;
    RAISE EXCEPTION 'Peer slot transition % -> % is not allowed', OLD.status, NEW.status
        USING ERRCODE = '42501';
END $$;
CREATE TRIGGER peer_slot_transition BEFORE UPDATE ON peer_slots
    FOR EACH ROW EXECUTE FUNCTION guard_peer_slot_update();

-- Patients also see the slots behind their own requests, and may update an
-- open or requested slot (the trigger above limits what they can change).
DROP POLICY peer_slots_read ON peer_slots;
CREATE POLICY peer_slots_read ON peer_slots FOR SELECT TO hellodoctor_api USING (
    peer_id = fn_current_peer_profile_id()
    OR current_setting('app.current_role', true) = 'ADMIN'
    OR (current_setting('app.current_role', true) = 'PATIENT' AND status = 'AVAILABLE'
        AND EXISTS (SELECT 1 FROM peer_profiles p WHERE p.id = peer_slots.peer_id AND p.approval_status = 'APPROVED'))
    OR EXISTS (SELECT 1 FROM peer_requests r WHERE r.slot_id = peer_slots.id
               AND r.patient_user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid));
CREATE POLICY peer_slots_patient_update ON peer_slots FOR UPDATE TO hellodoctor_api
    USING (current_setting('app.current_role', true) = 'PATIENT' AND status IN ('AVAILABLE', 'REQUESTED'))
    WITH CHECK (current_setting('app.current_role', true) = 'PATIENT' AND status IN ('AVAILABLE', 'REQUESTED'));

-- Notifications (no topic text or health detail in any of them).
INSERT INTO notification_type_registry(notification_type, category, optional_engagement, quiet_hours_allowed, max_per_day) VALUES
    ('PEER_REQUEST_NEW', 'TRANSACTIONAL', false, true, 30),
    ('PEER_REQUEST_ACCEPTED', 'TRANSACTIONAL', false, true, 30),
    ('PEER_REQUEST_DECLINED', 'TRANSACTIONAL', false, true, 30),
    ('PEER_REQUEST_CANCELLED', 'TRANSACTIONAL', false, true, 30);
ALTER TABLE notifications DROP CONSTRAINT notifications_type_check;
ALTER TABLE notifications ADD CONSTRAINT notifications_type_check CHECK (type IN (
    'REGISTRATION_SUBMITTED', 'REGISTRATION_VERIFIED', 'REGISTRATION_REJECTED',
    'PROFESSIONAL_VERIFICATION_REVOKED', 'ACCOUNT_SUSPENDED', 'ACCOUNT_REACTIVATED',
    'ACCOUNT_UPDATED', 'BOOKING_HOLD_CREATED', 'BOOKING_HOLD_ONE_MINUTE',
    'BOOKING_HOLD_EXPIRED', 'BOOKING_CONFIRMED', 'BOOKING_POSTPONED',
    'BOOKING_RESCHEDULED', 'BOOKING_CANCELLED', 'PAYMENT_HOLD_UPDATED',
    'CONSULTATION_REMINDER_15M', 'CONSULTATION_READY', 'CONSULTATION_REJOIN',
    'CONSULTATION_INTERRUPTED', 'CONSULTATION_NEEDS_REVIEW',
    'CHAT_MESSAGE', 'NEW_CHAT_MESSAGE', 'DOCTOR_REPLY', 'PRESCRIPTION_AVAILABLE',
    'MEDICAL_RECORD_UPDATED', 'EARNING_CREDITED', 'PAYOUT_PAID',
    'DAILY_CONSULTATION_SUMMARY', 'PATIENT_GREETING', 'HOLIDAY_GREETING',
    'BIRTHDAY_GREETING', 'WELCOME_GREETING', 'HEALTH_EDUCATION',
    'SERVICE_ANNOUNCEMENT', 'MAINTENANCE', 'ADMIN_CAMPAIGN',
    'ARTICLE_STATUS_CHANGED',
    'REFUND_APPROVED', 'REFUND_PROCESSING', 'REFUND_COMPLETED',
    'CONSULTATION_COMPLETED', 'CONSULTATION_RECOVERY_RESOLVED',
    'DOCTOR_GREETING', 'FIRST_CONSULTATION_MILESTONE', 'NEW_ARTICLE_PUBLISHED',
    'PEER_REQUEST_NEW', 'PEER_REQUEST_ACCEPTED', 'PEER_REQUEST_DECLINED', 'PEER_REQUEST_CANCELLED'
));
