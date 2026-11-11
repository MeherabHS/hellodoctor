-- Phase 2: Secure Email OTP Password Recovery (Patient and Doctor only —
-- Admin deliberately has no self-service reset path; see
-- services/password_reset_service.rs). State is derived from column
-- presence/expiry rather than a literal status column:
--   REQUESTED      -> row exists, authorization_hash IS NULL
--   OTP_VERIFIED   -> row exists, authorization_hash IS NOT NULL
--   EXPIRED/FAILED -> detected at read time (otp_expires_at/attempt_count),
--                     row deleted immediately once detected
--   COMPLETED      -> row deleted on successful password update
--   INVALIDATED    -> a newer request for the same user deleted this row
-- The OTP code and the post-verification "reset_authorization" bearer value
-- are both stored only as a SHA-256 hash (crypto::hash_code), identical to
-- every other short-lived-secret table in this schema — the plaintext
-- authorization value is handed to the client exactly once (the verify-otp
-- response) and never persisted or logged anywhere.
CREATE TABLE password_reset_requests (
    token VARCHAR(64) PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    code_hash VARCHAR(128) NOT NULL,
    attempt_count SMALLINT NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    max_attempts SMALLINT NOT NULL DEFAULT 5,
    otp_expires_at TIMESTAMPTZ NOT NULL,
    authorization_hash VARCHAR(128),
    authorization_expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX password_reset_requests_user_id_idx ON password_reset_requests(user_id);

-- Enforced at the database level, in addition to the service-layer check,
-- that at most one row can ever be in the OTP_VERIFIED state for a given
-- authorization secret.
CREATE UNIQUE INDEX password_reset_requests_authorization_hash_idx
    ON password_reset_requests(authorization_hash) WHERE authorization_hash IS NOT NULL;

ALTER TABLE password_reset_requests ENABLE ROW LEVEL SECURITY;
ALTER TABLE password_reset_requests FORCE ROW LEVEL SECURITY;

-- Unauthenticated-flow table (like patient_auth_challenges / pending_mfa_challenges):
-- only the auth worker, running the three public password-reset endpoints,
-- ever touches this table. No patient/doctor/admin-facing SELECT policy
-- exists or should ever be added here.
CREATE POLICY password_reset_requests_auth_worker ON password_reset_requests
    FOR ALL TO hellodoctor_auth_worker USING (true) WITH CHECK (true);

GRANT SELECT, INSERT, UPDATE, DELETE ON password_reset_requests TO hellodoctor_auth_worker;

-- New OTP category for the email_outbox durable-tracking table, delivered on
-- the hellodoctor_auth_worker pool exactly like PATIENT_OTP/DOCTOR_OTP/
-- ADMIN_OTP (a reset code, like a login code, must fail the triggering
-- request if delivery fails — never the silent-best-effort
-- TransactionalEmailKind path used for already-succeeded business events).
ALTER TABLE email_outbox DROP CONSTRAINT email_outbox_email_type_check;
ALTER TABLE email_outbox ADD CONSTRAINT email_outbox_email_type_check
    CHECK (email_type IN (
        'PATIENT_OTP', 'DOCTOR_OTP', 'ADMIN_OTP', 'PASSWORD_RESET_OTP',
        'PATIENT_WELCOME',
        'DOCTOR_APPLICATION_RECEIVED', 'DOCTOR_APPROVED', 'DOCTOR_REJECTED',
        'CONSULTATION_CONFIRMED', 'CONSULTATION_RESCHEDULED',
        'PAYMENT_CONFIRMED', 'REFUND_PROCESSED',
        'PASSWORD_CHANGED'
    ));
