-- Notification Milestone 1 — Essential Transactional Events.
--
-- Adds the 3 refund-lifecycle notification types that have no registered
-- type string at all yet (BOOKING_CONFIRMED/BOOKING_RESCHEDULED/
-- PRESCRIPTION_AVAILABLE were already registered in
-- 20261117000001_fcm_delivery_and_templates — no CHECK/registry change
-- needed for those three), and a single reusable SECURITY DEFINER function
-- that resolves an appointment's doctor/patient (or patient's guardian)
-- notification recipients.
--
-- Deliberately NOT a blanket RLS bypass: this mirrors chat_helpers.up.sql's
-- `chat_counterparty_user` exactly, gated by the same
-- `consultation_participant_allowed(appointment_id)` check already used
-- throughout `consultation_trace` — true only for an ADMIN-role caller, the
-- assigned/verified Doctor, or the owning Patient. This already covers every
-- real call site this milestone needs for the *recipient-resolution* read
-- (the payment webhook and reschedule handler run as an ADMIN-role context
-- on hellodoctor_payment_worker, the Doctor's own prescription upload runs
-- as DOCTOR on hellodoctor_api, and grievance refund adjudication runs as
-- ADMIN on hellodoctor_finance_worker), since `consultation_participant_allowed`
-- was already granted to exactly these roles.
--
-- The actual notification INSERT is a separate privilege, confirmed missing
-- by a real failing test ("permission denied for table notifications"), not
-- assumed: `notifications_backend_insert` (20261020000001, extended for
-- Finance in 20261029000001) never named hellodoctor_payment_worker, which
-- `BOOKING_CONFIRMED`/`BOOKING_RESCHEDULED` both need (hellodoctor_api and
-- hellodoctor_finance_worker already had it).

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
    'REFUND_APPROVED', 'REFUND_PROCESSING', 'REFUND_COMPLETED'
));

INSERT INTO notification_type_registry(notification_type, category, optional_engagement, quiet_hours_allowed, max_per_day)
VALUES
 ('REFUND_APPROVED','TRANSACTIONAL',false,true,10),
 ('REFUND_PROCESSING','TRANSACTIONAL',false,true,10),
 ('REFUND_COMPLETED','TRANSACTIONAL',false,true,10)
ON CONFLICT (notification_type) DO NOTHING;

CREATE FUNCTION appointment_notification_recipients(p_appointment UUID)
RETURNS TABLE(doctor_user_id UUID, patient_user_id UUID)
LANGUAGE sql SECURITY DEFINER SET search_path=public STABLE AS $$
 SELECT d.user_id, COALESCE(p.user_id, g.user_id)
   FROM appointments a
   JOIN doctor_profiles d ON d.id = a.doctor_id
   JOIN patient_profiles p ON p.id = a.patient_id
   LEFT JOIN patient_profiles g ON g.id = p.guardian_patient_id
  WHERE a.id = p_appointment AND consultation_participant_allowed(p_appointment)
$$;
REVOKE ALL ON FUNCTION appointment_notification_recipients(UUID) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION appointment_notification_recipients(UUID)
    TO hellodoctor_api, hellodoctor_payment_worker, hellodoctor_finance_worker;

DROP POLICY notifications_backend_insert ON notifications;
CREATE POLICY notifications_backend_insert ON notifications
    FOR INSERT
    TO hellodoctor_api, hellodoctor_system_worker, hellodoctor_finance_worker, hellodoctor_payment_worker
    WITH CHECK (true);

GRANT INSERT ON notifications TO hellodoctor_payment_worker;
