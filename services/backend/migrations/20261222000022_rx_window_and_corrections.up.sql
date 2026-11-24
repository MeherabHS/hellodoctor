-- Smart Rx follow-ups (product owner, 2026-11-24).
--
-- 1. RX_WINDOW_CLOSING: the doctor is reminded 4 hours before a completed
--    consultation's 24-hour prescription window closes with no decision.
-- 2. Prescription correction notes: a prescription stays final once
--    dispatched; the doctor can add a written correction the patient sees
--    (PRESCRIPTION_CORRECTED). Notes are append-only: no update or delete.
--    Admin roles get no access, because the note is clinical text.
-- 3. fn_admin_rx_follow_ups: what Admin may see about prescriptions that need
--    following up, without any clinical content: consultations whose window
--    closed with no decision, and video consultations both apps joined that
--    the provider callback never marked started.

INSERT INTO notification_type_registry(notification_type, category, optional_engagement, quiet_hours_allowed, max_per_day) VALUES
    ('RX_WINDOW_CLOSING', 'CLINICAL', false, true, 20),
    ('PRESCRIPTION_CORRECTED', 'CLINICAL', false, true, 10)
ON CONFLICT (notification_type) DO NOTHING;
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
    'PEER_REQUEST_NEW', 'PEER_REQUEST_ACCEPTED', 'PEER_REQUEST_DECLINED', 'PEER_REQUEST_CANCELLED',
    'RX_WINDOW_CLOSING', 'PRESCRIPTION_CORRECTED'
));

CREATE TABLE prescription_corrections (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    appointment_id UUID NOT NULL REFERENCES appointments(id) ON DELETE RESTRICT,
    doctor_id UUID NOT NULL REFERENCES doctor_profiles(id),
    note TEXT NOT NULL CHECK (length(btrim(note)) BETWEEN 3 AND 1000),
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
CREATE INDEX idx_prescription_corrections_appointment ON prescription_corrections(appointment_id, created_at);
ALTER TABLE prescription_corrections ENABLE ROW LEVEL SECURITY;
ALTER TABLE prescription_corrections FORCE ROW LEVEL SECURITY;

-- The assigned doctor and the patient (or their guardian) read the notes.
CREATE POLICY prescription_corrections_read ON prescription_corrections FOR SELECT TO hellodoctor_api
USING (
    doctor_id = fn_current_doctor_profile_id()
    OR EXISTS (
        SELECT 1 FROM appointments a
        WHERE a.id = prescription_corrections.appointment_id
          AND fn_patient_profile_owned_by_caller(a.patient_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    )
);
-- Only the assigned doctor writes, only for their own consultation whose
-- prescription was dispatched (COMPLETED_WITH_RX).
CREATE POLICY prescription_corrections_insert ON prescription_corrections FOR INSERT TO hellodoctor_api
WITH CHECK (
    current_setting('app.current_role', true) = 'DOCTOR'
    AND doctor_id = fn_current_doctor_profile_id()
    AND EXISTS (
        SELECT 1 FROM appointments a
        WHERE a.id = prescription_corrections.appointment_id
          AND a.doctor_id = prescription_corrections.doctor_id
          AND a.status = 'COMPLETED' AND a.clinical_outcome = 'COMPLETED_WITH_RX'
    )
);
GRANT SELECT, INSERT ON prescription_corrections TO hellodoctor_api;

CREATE FUNCTION fn_admin_rx_follow_ups(p_days integer)
RETURNS TABLE (
    kind text,
    appointment_id uuid,
    appointment_number text,
    doctor_name text,
    modality text,
    ended_at timestamptz,
    closed_at timestamptz,
    slot_start timestamptz
)
LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public AS $$
    SELECT 'WINDOW_CLOSED_NO_DECISION', a.id, a.appointment_number::text, d.full_name::text, a.modality::text,
           COALESCE(cs.ended_at, a.completed_at),
           COALESCE(cs.ended_at, a.completed_at) + INTERVAL '24 hours', s.start_time
      FROM appointments a
      JOIN doctor_profiles d ON d.id = a.doctor_id
      JOIN doctor_schedule_slots s ON s.id = a.slot_id
      LEFT JOIN consultation_sessions cs ON cs.appointment_id = a.id
     WHERE current_setting('app.current_role', true) = 'ADMIN'
       AND a.status = 'COMPLETED'
       AND a.clinical_outcome IS DISTINCT FROM 'COMPLETED_WITH_RX'
       AND a.clinical_outcome IS DISTINCT FROM 'COMPLETED_NO_RX'
       AND COALESCE(cs.ended_at, a.completed_at) + INTERVAL '24 hours' <= clock_timestamp()
       AND COALESCE(cs.ended_at, a.completed_at) + INTERVAL '24 hours' > clock_timestamp() - make_interval(days => GREATEST(LEAST(p_days, 30), 1))
    UNION ALL
    SELECT 'JOIN_SIGNAL_MISSING', a.id, a.appointment_number::text, d.full_name::text, a.modality::text,
           NULL::timestamptz, NULL::timestamptz, s.start_time
      FROM appointments a
      JOIN doctor_profiles d ON d.id = a.doctor_id
      JOIN doctor_schedule_slots s ON s.id = a.slot_id
      JOIN consultation_sessions cs ON cs.appointment_id = a.id
     WHERE current_setting('app.current_role', true) = 'ADMIN'
       AND a.modality = 'VIDEO' AND a.status IN ('CONFIRMED', 'WAITING')
       AND s.start_time > clock_timestamp() - make_interval(days => GREATEST(LEAST(p_days, 30), 1))
       AND EXISTS (SELECT 1 FROM consultation_telemetry_events e WHERE e.session_id = cs.id AND e.event_type = 'DOCTOR_JOINED' AND e.actor_perspective = 'DOCTOR')
       AND EXISTS (SELECT 1 FROM consultation_telemetry_events e WHERE e.session_id = cs.id AND e.event_type = 'PATIENT_JOINED' AND e.actor_perspective = 'PATIENT')
    ORDER BY 1, 7 DESC NULLS LAST, 8 DESC
$$;
REVOKE ALL ON FUNCTION fn_admin_rx_follow_ups(integer) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION fn_admin_rx_follow_ups(integer) TO hellodoctor_api;
