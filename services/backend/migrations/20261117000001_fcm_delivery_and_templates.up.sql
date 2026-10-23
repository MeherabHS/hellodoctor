-- Stage D: FCM delivery metadata, token ownership hardening and the single
-- notification registry. This is forward-only; no historical notification
-- migration is edited.

ALTER TABLE notifications DROP CONSTRAINT notifications_type_check;
ALTER TABLE notifications ADD CONSTRAINT notifications_type_check CHECK (type IN (
    'REGISTRATION_SUBMITTED', 'REGISTRATION_VERIFIED', 'REGISTRATION_REJECTED',
    'PROFESSIONAL_VERIFICATION_REVOKED', 'ACCOUNT_SUSPENDED', 'ACCOUNT_REACTIVATED',
    'ACCOUNT_UPDATED', 'BOOKING_HOLD_CREATED', 'BOOKING_HOLD_ONE_MINUTE',
    'BOOKING_HOLD_EXPIRED', 'BOOKING_CONFIRMED', 'BOOKING_POSTPONED',
    'BOOKING_RESCHEDULED', 'BOOKING_CANCELLED', 'PAYMENT_HOLD_UPDATED',
    'CONSULTATION_REMINDER_15M', 'CONSULTATION_READY', 'CONSULTATION_REJOIN',
    'CONSULTATION_INTERRUPTED', 'CONSULTATION_NEEDS_REVIEW',
    'CHAT_MESSAGE', 'DOCTOR_REPLY', 'PRESCRIPTION_AVAILABLE',
    'MEDICAL_RECORD_UPDATED', 'EARNING_CREDITED', 'PAYOUT_PAID',
    'DAILY_CONSULTATION_SUMMARY', 'PATIENT_GREETING', 'HOLIDAY_GREETING',
    'BIRTHDAY_GREETING', 'WELCOME_GREETING', 'HEALTH_EDUCATION',
    'SERVICE_ANNOUNCEMENT', 'MAINTENANCE', 'ADMIN_CAMPAIGN',
    'ARTICLE_STATUS_CHANGED'
));

ALTER TABLE notifications
    ADD COLUMN delivery_attempts INTEGER NOT NULL DEFAULT 0 CHECK (delivery_attempts >= 0),
    ADD COLUMN next_delivery_at TIMESTAMPTZ,
    ADD COLUMN last_delivery_error VARCHAR(500),
    ADD COLUMN provider_message_id VARCHAR(200),
    ADD COLUMN collapse_key VARCHAR(128),
    ADD COLUMN ttl_seconds INTEGER CHECK (ttl_seconds IS NULL OR ttl_seconds BETWEEN 0 AND 2419200);

CREATE INDEX idx_notifications_delivery_due
    ON notifications(delivery_status, next_delivery_at, created_at)
    WHERE delivery_status IN ('CREATED', 'DUE', 'FAILED');

-- A registration token is a provider credential for one account only. Existing
-- duplicate rows are retained as history but disabled before the unique index.
WITH ranked AS (
    SELECT device_id, ROW_NUMBER() OVER (
        PARTITION BY push_token ORDER BY enabled DESC, updated_at DESC, device_id DESC
    ) AS rn
    FROM notification_device_tokens
)
UPDATE notification_device_tokens t
SET enabled = FALSE, updated_at = clock_timestamp()
FROM ranked r
WHERE r.device_id = t.device_id AND r.rn > 1;
CREATE UNIQUE INDEX uq_notification_device_tokens_push_token
    ON notification_device_tokens(push_token);

CREATE POLICY notification_device_tokens_system_select
    ON notification_device_tokens FOR SELECT TO hellodoctor_system_worker USING (true);
CREATE POLICY notification_device_tokens_system_update
    ON notification_device_tokens FOR UPDATE TO hellodoctor_system_worker
    USING (true) WITH CHECK (true);
GRANT SELECT ON notification_device_tokens TO hellodoctor_system_worker;
GRANT UPDATE ON notification_device_tokens TO hellodoctor_system_worker;

CREATE POLICY notifications_system_delivery_update
    ON notifications FOR UPDATE TO hellodoctor_system_worker
    USING (true) WITH CHECK (true);

-- Transactional template registry. Templates contain only safe placeholders;
-- callers choose a registry key and never supply arbitrary push text.
CREATE TABLE notification_templates (
    template_key VARCHAR(80) NOT NULL,
    locale VARCHAR(5) NOT NULL CHECK (locale IN ('en', 'sw')),
    category VARCHAR(20) NOT NULL CHECK (category IN ('TRANSACTIONAL', 'SECURITY', 'CLINICAL', 'OPERATIONAL', 'ENGAGEMENT')),
    notification_type VARCHAR(40) NOT NULL,
    title_template VARCHAR(255) NOT NULL,
    body_template VARCHAR(1000) NOT NULL,
    ttl_seconds INTEGER CHECK (ttl_seconds IS NULL OR ttl_seconds BETWEEN 0 AND 2419200),
    collapse_key VARCHAR(128),
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    PRIMARY KEY (template_key, locale)
);

CREATE TABLE notification_type_registry (
    notification_type VARCHAR(40) PRIMARY KEY,
    category VARCHAR(20) NOT NULL CHECK (category IN ('TRANSACTIONAL', 'SECURITY', 'CLINICAL', 'OPERATIONAL', 'ENGAGEMENT')),
    optional_engagement BOOLEAN NOT NULL DEFAULT FALSE,
    quiet_hours_allowed BOOLEAN NOT NULL DEFAULT TRUE,
    max_per_day INTEGER NOT NULL DEFAULT 20 CHECK (max_per_day BETWEEN 1 AND 1000)
);

-- The template FK is added after the registry exists so the migration is
-- valid on a fresh schema as well as an upgraded one.
ALTER TABLE notification_templates
    ADD CONSTRAINT notification_templates_notification_type_fkey
    FOREIGN KEY (notification_type) REFERENCES notification_type_registry(notification_type);

INSERT INTO notification_type_registry(notification_type, category, optional_engagement, quiet_hours_allowed, max_per_day)
SELECT type, CASE
    WHEN type LIKE 'REGISTRATION%' OR type LIKE 'ACCOUNT%' THEN 'SECURITY'
    WHEN type LIKE 'CONSULTATION%' OR type IN ('PRESCRIPTION_AVAILABLE','MEDICAL_RECORD_UPDATED','CHAT_MESSAGE','DOCTOR_REPLY') THEN 'CLINICAL'
    WHEN type LIKE 'BOOKING%' OR type LIKE 'PAYMENT%' OR type IN ('EARNING_CREDITED','PAYOUT_PAID') THEN 'TRANSACTIONAL'
    WHEN type IN ('DAILY_CONSULTATION_SUMMARY','MAINTENANCE','SERVICE_ANNOUNCEMENT') THEN 'OPERATIONAL'
    ELSE 'ENGAGEMENT' END,
    type IN ('PATIENT_GREETING','HOLIDAY_GREETING','BIRTHDAY_GREETING','WELCOME_GREETING','HEALTH_EDUCATION','ADMIN_CAMPAIGN'),
    type NOT IN ('CONSULTATION_READY','CONSULTATION_REJOIN','CONSULTATION_INTERRUPTED','CONSULTATION_NEEDS_REVIEW'),
    CASE WHEN type IN ('CHAT_MESSAGE','DOCTOR_REPLY') THEN 30 ELSE 10 END
FROM (SELECT DISTINCT type FROM notifications) existing
ON CONFLICT (notification_type) DO NOTHING;

-- Seed every known type even before its first event is emitted.
INSERT INTO notification_type_registry(notification_type, category, optional_engagement, quiet_hours_allowed, max_per_day)
VALUES
 ('REGISTRATION_SUBMITTED','SECURITY',false,true,10),('REGISTRATION_VERIFIED','SECURITY',false,true,10),('REGISTRATION_REJECTED','SECURITY',false,true,10),('PROFESSIONAL_VERIFICATION_REVOKED','SECURITY',false,true,10),('ACCOUNT_SUSPENDED','SECURITY',false,true,10),('ACCOUNT_REACTIVATED','SECURITY',false,true,10),('ACCOUNT_UPDATED','SECURITY',false,true,10),
 ('BOOKING_HOLD_CREATED','TRANSACTIONAL',false,true,10),('BOOKING_HOLD_ONE_MINUTE','TRANSACTIONAL',false,false,10),('BOOKING_HOLD_EXPIRED','TRANSACTIONAL',false,true,10),('BOOKING_CONFIRMED','TRANSACTIONAL',false,true,10),('BOOKING_POSTPONED','TRANSACTIONAL',false,true,10),('BOOKING_RESCHEDULED','TRANSACTIONAL',false,true,10),('BOOKING_CANCELLED','TRANSACTIONAL',false,true,10),('PAYMENT_HOLD_UPDATED','TRANSACTIONAL',false,true,10),
 ('CONSULTATION_REMINDER_15M','CLINICAL',false,false,10),('CONSULTATION_READY','CLINICAL',false,false,10),('CONSULTATION_REJOIN','CLINICAL',false,false,20),('CONSULTATION_INTERRUPTED','CLINICAL',false,false,20),('CONSULTATION_NEEDS_REVIEW','CLINICAL',false,true,10),('CHAT_MESSAGE','CLINICAL',false,true,30),('DOCTOR_REPLY','CLINICAL',false,true,30),('PRESCRIPTION_AVAILABLE','CLINICAL',false,true,10),('MEDICAL_RECORD_UPDATED','CLINICAL',false,true,10),
 ('EARNING_CREDITED','TRANSACTIONAL',false,true,10),('PAYOUT_PAID','TRANSACTIONAL',false,true,10),('DAILY_CONSULTATION_SUMMARY','OPERATIONAL',false,true,3),('PATIENT_GREETING','ENGAGEMENT',true,true,1),('HOLIDAY_GREETING','ENGAGEMENT',true,true,3),('BIRTHDAY_GREETING','ENGAGEMENT',true,true,1),('WELCOME_GREETING','ENGAGEMENT',true,true,1),('HEALTH_EDUCATION','ENGAGEMENT',true,true,3),('SERVICE_ANNOUNCEMENT','OPERATIONAL',false,true,5),('MAINTENANCE','OPERATIONAL',false,false,5),('ADMIN_CAMPAIGN','ENGAGEMENT',true,true,3),('ARTICLE_STATUS_CHANGED','OPERATIONAL',false,true,10)
ON CONFLICT (notification_type) DO NOTHING;

GRANT SELECT ON notification_templates, notification_type_registry TO hellodoctor_api, hellodoctor_system_worker;
GRANT INSERT, UPDATE, DELETE ON notification_templates, notification_type_registry TO hellodoctor_system_worker;
ALTER TABLE notification_templates ENABLE ROW LEVEL SECURITY;
ALTER TABLE notification_type_registry ENABLE ROW LEVEL SECURITY;
ALTER TABLE notification_templates FORCE ROW LEVEL SECURITY;
ALTER TABLE notification_type_registry FORCE ROW LEVEL SECURITY;
CREATE POLICY notification_templates_system_all ON notification_templates FOR ALL TO hellodoctor_system_worker USING (true) WITH CHECK (true);
CREATE POLICY notification_type_registry_system_all ON notification_type_registry FOR ALL TO hellodoctor_system_worker USING (true) WITH CHECK (true);
CREATE POLICY notification_templates_admin_read ON notification_templates FOR SELECT TO hellodoctor_api USING (current_setting('app.current_role',true)='ADMIN');
CREATE POLICY notification_type_registry_admin_read ON notification_type_registry FOR SELECT TO hellodoctor_api USING (current_setting('app.current_role',true)='ADMIN');
