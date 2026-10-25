-- Notification Milestone 2 — Consultation Lifecycle Notifications.
--
-- CONSULTATION_READY/CONSULTATION_REJOIN/CONSULTATION_INTERRUPTED/
-- CONSULTATION_NEEDS_REVIEW were already registered in
-- 20261117000001_fcm_delivery_and_templates — no CHECK/registry change
-- needed for those four. Only two genuinely new notification kinds are
-- added here: a consultation reaching its authorized completion, and an
-- Admin's recorded recovery-review resolution. Neither is a new clinical
-- *status* (CLAUDE.md's "do not add fictional consultation statuses") — both
-- ride on states that already exist (`doctor_complete_consultation`'s
-- `COMPLETED` result, `admin_resolve_consultation_recovery`'s own
-- `consultation_recovery_resolutions` row).

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
    'CONSULTATION_COMPLETED', 'CONSULTATION_RECOVERY_RESOLVED'
));

INSERT INTO notification_type_registry(notification_type, category, optional_engagement, quiet_hours_allowed, max_per_day)
VALUES
 ('CONSULTATION_COMPLETED','CLINICAL',false,true,10),
 ('CONSULTATION_RECOVERY_RESOLVED','CLINICAL',false,true,10)
ON CONFLICT (notification_type) DO NOTHING;
