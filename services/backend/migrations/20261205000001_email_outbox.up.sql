-- Phase 0 email integration: durable delivery tracking for outbound email
-- (currently OTP only — see services/email_transport.rs and
-- services/email_delivery.rs). Deliberately NOT a copy of the `notifications`
-- table: there is no user-facing inbox here, no `user_id` FK (an OTP
-- challenge can predate the account it belongs to, e.g. patient
-- self-registration), and the OTP code itself is never persisted anywhere —
-- this table tracks only the metadata of a send attempt (recipient, type,
-- status, attempts, provider message id, a sanitized failure reason), never
-- the message content.
CREATE TABLE email_outbox (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email_type VARCHAR(40) NOT NULL CHECK (email_type IN (
        'PATIENT_OTP', 'DOCTOR_OTP', 'ADMIN_OTP'
    )),
    recipient_email VARCHAR(255) NOT NULL,
    dedup_key VARCHAR(200) NOT NULL,
    delivery_status VARCHAR(20) NOT NULL DEFAULT 'CREATED' CHECK (delivery_status IN (
        'CREATED', 'SENT_TO_PROVIDER', 'FAILED', 'EXPIRED'
    )),
    delivery_attempts INT NOT NULL DEFAULT 0,
    last_delivery_error VARCHAR(500),
    provider_message_id VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT email_outbox_dedup_key_unique UNIQUE (dedup_key)
);

-- Supports the periodic expiry-reconciliation sweep
-- (notification_scheduler -> email_delivery::run_email_outbox_expiry_sweep):
-- rows that never reached a terminal status before their underlying OTP
-- challenge expired (e.g. the process crashed mid-send).
CREATE INDEX idx_email_outbox_expiry_sweep ON email_outbox(expires_at)
    WHERE delivery_status = 'CREATED';

ALTER TABLE email_outbox ENABLE ROW LEVEL SECURITY;
ALTER TABLE email_outbox FORCE ROW LEVEL SECURITY;

-- Operational delivery metadata only, never a user-facing inbox: no owner
-- (patient/doctor/admin) SELECT policy exists or should ever be added here.
-- Only the auth worker (creates/updates a row around its own send attempt,
-- during login/registration) and the system worker (periodic expiry sweep)
-- may touch this table — mirrors the role separation already established for
-- `notifications` (migrations/20261020000001_doctor_engagement_notifications).
CREATE POLICY email_outbox_auth_worker ON email_outbox
    FOR ALL TO hellodoctor_auth_worker USING (true) WITH CHECK (true);
CREATE POLICY email_outbox_system_worker ON email_outbox
    FOR ALL TO hellodoctor_system_worker USING (true) WITH CHECK (true);

GRANT SELECT, INSERT, UPDATE ON email_outbox TO hellodoctor_auth_worker;
GRANT SELECT, UPDATE ON email_outbox TO hellodoctor_system_worker;
