-- Recreates mfa_credentials exactly as accumulated by
-- 20261001000001_initial_schema, 20261107000001_totp_replay_protection, and
-- 20261110000004_admin_totp_algorithm. Schema-reversible only: any
-- previously-enrolled credential rows are not recoverable, same as any
-- other DROP TABLE revert.
CREATE TYPE mfa_method_enum AS ENUM ('TOTP', 'PASSKEY');

CREATE TABLE mfa_credentials (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    method mfa_method_enum NOT NULL DEFAULT 'TOTP',
    totp_secret_encrypted BYTEA NOT NULL,
    recovery_codes_hash TEXT[],
    is_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    enabled_at TIMESTAMPTZ,
    last_verified_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_totp_step BIGINT,
    totp_algorithm TEXT NOT NULL DEFAULT 'SHA1' CHECK (totp_algorithm IN ('SHA1','SHA256'))
);
CREATE UNIQUE INDEX idx_mfa_user_method ON mfa_credentials(user_id, method) WHERE revoked_at IS NULL;
GRANT SELECT, INSERT, UPDATE ON mfa_credentials TO hellodoctor_auth_worker;
