CREATE TABLE patient_auth_challenges (
    token VARCHAR(64) PRIMARY KEY,
    email VARCHAR(255) NOT NULL,
    phone_number VARCHAR(32),
    password_hash VARCHAR(255),
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    flow VARCHAR(16) NOT NULL CHECK (flow IN ('REGISTER', 'LOGIN')),
    code_hash VARCHAR(128) NOT NULL,
    attempts SMALLINT NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX patient_auth_challenges_email_idx ON patient_auth_challenges(email);
GRANT SELECT, INSERT, UPDATE, DELETE ON patient_auth_challenges TO hellodoctor_auth_worker;
