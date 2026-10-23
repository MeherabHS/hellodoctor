DROP POLICY IF EXISTS notification_type_registry_admin_read ON notification_type_registry;
DROP POLICY IF EXISTS notification_templates_admin_read ON notification_templates;
DROP POLICY IF EXISTS notification_type_registry_system_all ON notification_type_registry;
DROP POLICY IF EXISTS notification_templates_system_all ON notification_templates;
DROP TABLE notification_templates;
DROP TABLE notification_type_registry;
DROP POLICY IF EXISTS notifications_system_delivery_update ON notifications;
DROP POLICY IF EXISTS notification_device_tokens_system_update ON notification_device_tokens;
DROP POLICY IF EXISTS notification_device_tokens_system_select ON notification_device_tokens;
DROP INDEX IF EXISTS uq_notification_device_tokens_push_token;
DROP INDEX IF EXISTS idx_notifications_delivery_due;
ALTER TABLE notifications DROP COLUMN IF EXISTS ttl_seconds;
ALTER TABLE notifications DROP COLUMN IF EXISTS collapse_key;
ALTER TABLE notifications DROP COLUMN IF EXISTS provider_message_id;
ALTER TABLE notifications DROP COLUMN IF EXISTS last_delivery_error;
ALTER TABLE notifications DROP COLUMN IF EXISTS next_delivery_at;
ALTER TABLE notifications DROP COLUMN IF EXISTS delivery_attempts;
ALTER TABLE notifications DROP CONSTRAINT notifications_type_check;
ALTER TABLE notifications ADD CONSTRAINT notifications_type_check CHECK (type IN (
    'DAILY_CONSULTATION_SUMMARY','CONSULTATION_REMINDER_15M','EARNING_CREDITED',
    'PAYOUT_PAID','REGISTRATION_VERIFIED','REGISTRATION_REJECTED','ARTICLE_STATUS_CHANGED',
    'BOOKING_HOLD_CREATED','BOOKING_HOLD_ONE_MINUTE','BOOKING_HOLD_EXPIRED'
));
