-- Existing secrets are SHA1 unless explicitly enrolled as SHA256.
ALTER TABLE mfa_credentials ADD COLUMN totp_algorithm TEXT NOT NULL DEFAULT 'SHA1'
    CHECK (totp_algorithm IN ('SHA1','SHA256'));
