DROP INDEX IF EXISTS idx_auth_sessions_live_family;
DROP INDEX IF EXISTS idx_auth_sessions_all_token_hash;
-- Revocation is a security event, never reversed by a schema rollback.
