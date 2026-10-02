-- Short-lived second-factor challenges issued after step-1 password verification
-- (doctor license+password, admin email+password) while awaiting OTP/TOTP.
CREATE TABLE pending_mfa_challenges (
    token VARCHAR(64) PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    code_hash VARCHAR(128) NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
