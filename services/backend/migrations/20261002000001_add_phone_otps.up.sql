-- Real OTP storage (hashed, rate-limited) replacing the previous in-memory map.
CREATE TABLE phone_otps (
    phone_number VARCHAR(20) PRIMARY KEY,
    code_hash VARCHAR(128) NOT NULL,
    attempts SMALLINT NOT NULL DEFAULT 0,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
