-- Forward-only fix (no historical notification migration is edited): the
-- notifications_type_check rewrite in 20261113000001_payment_hold_cleanup
-- accidentally dropped 'NEW_CHAT_MESSAGE' (added by
-- 20261106000003_consultation_trace) from the allowed type list, and
-- 20261117000001_fcm_delivery_and_templates carried the omission forward
-- while introducing an unused 'CHAT_MESSAGE' placeholder instead.
-- chat_service.rs and both Flutter apps (local_notification_service.dart in
-- apps/mobile and apps/mobile_patient) have always used 'NEW_CHAT_MESSAGE' --
-- this restores the actual application type rather than renaming it, which
-- would also require coordinated Flutter-side changes across two apps and
-- their tests. Confirmed live: every chat-message-send path was returning a
-- 500 (notifications_type_check violation) since 20261113 landed.
ALTER TABLE notifications DROP CONSTRAINT notifications_type_check;
ALTER TABLE notifications ADD CONSTRAINT notifications_type_check CHECK (type IN (
    'REGISTRATION_SUBMITTED', 'REGISTRATION_VERIFIED', 'REGISTRATION_REJECTED',
    'PROFESSIONAL_VERIFICATION_REVOKED', 'ACCOUNT_SUSPENDED', 'ACCOUNT_REACTIVATED',
    'ACCOUNT_UPDATED', 'BOOKING_HOLD_CREATED', 'BOOKING_HOLD_ONE_MINUTE',
    'BOOKING_HOLD_EXPIRED', 'BOOKING_CONFIRMED', 'BOOKING_POSTPONED',
    'BOOKING_RESCHEDULED', 'BOOKING_CANCELLED', 'PAYMENT_HOLD_UPDATED',
    'CONSULTATION_REMINDER_15M', 'CONSULTATION_READY', 'CONSULTATION_REJOIN',
    'CONSULTATION_INTERRUPTED', 'CONSULTATION_NEEDS_REVIEW',
    'CHAT_MESSAGE', 'DOCTOR_REPLY', 'NEW_CHAT_MESSAGE', 'PRESCRIPTION_AVAILABLE',
    'MEDICAL_RECORD_UPDATED', 'EARNING_CREDITED', 'PAYOUT_PAID',
    'DAILY_CONSULTATION_SUMMARY', 'PATIENT_GREETING', 'HOLIDAY_GREETING',
    'BIRTHDAY_GREETING', 'WELCOME_GREETING', 'HEALTH_EDUCATION',
    'SERVICE_ANNOUNCEMENT', 'MAINTENANCE', 'ADMIN_CAMPAIGN',
    'ARTICLE_STATUS_CHANGED'
));

INSERT INTO notification_type_registry(notification_type, category, optional_engagement, quiet_hours_allowed, max_per_day)
VALUES ('NEW_CHAT_MESSAGE', 'CLINICAL', false, true, 30)
ON CONFLICT (notification_type) DO NOTHING;
