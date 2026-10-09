-- Grievance reference numbers must be deterministically unique under
-- concurrency, not probabilistically unique. The previous scheme truncated a
-- fresh UUID to its first 4 hex characters (16 bits) per calendar day, which
-- collided under real concurrent load (found live during the pre-Patient 4x8
-- capacity benchmark: duplicate-key violations on
-- grievance_reports_grievance_number_key growing with RPS). A Postgres
-- sequence is atomic under concurrency by construction — two transactions
-- can never receive the same value. Gaps (an aborted transaction leaving a
-- sequence value unused) are an accepted, standard cost of human-facing
-- reference numbers; gapless numbering is deliberately not attempted.
CREATE SEQUENCE grievance_number_seq START 1;
GRANT USAGE ON SEQUENCE grievance_number_seq TO hellodoctor_api;

ALTER TABLE grievance_reports ALTER COLUMN grievance_number
    SET DEFAULT ('GRV-' || to_char(now(), 'YYYYMMDD') || '-' || lpad(nextval('grievance_number_seq')::text, 8, '0'));
