REVOKE SELECT, UPDATE ON doctor_articles FROM hellodoctor_system_worker;
DROP POLICY doctor_articles_system_update ON doctor_articles;
DROP POLICY doctor_articles_system_select ON doctor_articles;
ALTER TABLE doctor_articles DROP COLUMN engagement_notified_at;

DELETE FROM notification_templates WHERE template_key IN (
    'patient_greeting', 'doctor_greeting', 'welcome_greeting', 'doctor_welcome_greeting', 'birthday_greeting',
    'first_consultation_milestone', 'health_education', 'new_article_published',
    'new_year', 'labour_day', 'madaraka_day', 'mazingira_day', 'mashujaa_day',
    'jamhuri_day', 'christmas', 'boxing_day', 'good_friday', 'easter_monday', 'eid_al_fitr', 'eid_al_adha'
);

ALTER TABLE notifications DROP COLUMN frequency_claimed;

REVOKE SELECT, INSERT, UPDATE ON notification_frequency_counters FROM hellodoctor_system_worker;
DROP TABLE notification_frequency_counters;

REVOKE SELECT, INSERT, UPDATE, DELETE ON kenyan_holidays FROM hellodoctor_system_worker;
DROP TABLE kenyan_holidays;

ALTER TABLE notification_preferences
    DROP COLUMN greeting_notifications_enabled,
    DROP COLUMN religious_greetings_enabled;

DELETE FROM notification_type_registry WHERE notification_type IN (
    'DOCTOR_GREETING', 'FIRST_CONSULTATION_MILESTONE', 'NEW_ARTICLE_PUBLISHED'
);

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
