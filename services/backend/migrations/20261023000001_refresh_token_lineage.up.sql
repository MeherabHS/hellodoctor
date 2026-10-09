-- Legacy rotation overwrote consumed hashes. Their history cannot be rebuilt,
-- so require reauthentication for any family already rotated before cutover.
-- Stop legacy server instances before deploying this migration/new code.
UPDATE auth_sessions SET revoked_at = CURRENT_TIMESTAMP
WHERE revoked_at IS NULL AND token_family_id IN (
    SELECT token_family_id FROM auth_sessions WHERE rotated_at IS NOT NULL
);

-- Retain unique token identities after consumption/revocation. Each family
-- has at most one unconsumed, unrevoked leaf; rotation consumes the old leaf
-- and inserts its successor inside the same family-locked transaction.
CREATE UNIQUE INDEX idx_auth_sessions_all_token_hash ON auth_sessions(refresh_token_hash);
CREATE UNIQUE INDEX idx_auth_sessions_live_family ON auth_sessions(token_family_id)
    WHERE rotated_at IS NULL AND revoked_at IS NULL;
