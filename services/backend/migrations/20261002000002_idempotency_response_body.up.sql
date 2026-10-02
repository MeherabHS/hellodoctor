-- Store the actual replayable response body alongside the idempotency record.
ALTER TABLE idempotency_records ADD COLUMN response_body TEXT;
