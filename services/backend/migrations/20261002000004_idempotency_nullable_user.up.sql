-- No request-authentication middleware exists yet to resolve a caller identity at the HTTP layer
-- (handlers currently use fixed demo actor ids per route group), so idempotency can only be keyed
-- by (route, key_hash) for now. user_id is kept for when real session-based auth middleware lands.
DROP INDEX IF EXISTS idx_idempotency_key;
ALTER TABLE idempotency_records ALTER COLUMN user_id DROP NOT NULL;
CREATE UNIQUE INDEX idx_idempotency_key ON idempotency_records(route, key_hash);
