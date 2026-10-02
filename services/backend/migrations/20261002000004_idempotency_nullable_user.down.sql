DROP INDEX IF EXISTS idx_idempotency_key;
DELETE FROM idempotency_records WHERE user_id IS NULL;
ALTER TABLE idempotency_records ALTER COLUMN user_id SET NOT NULL;
CREATE UNIQUE INDEX idx_idempotency_key ON idempotency_records(user_id, route, key_hash);
