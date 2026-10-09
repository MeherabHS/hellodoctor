-- System Error Service: `system_error_logs` existed in schema since the initial
-- migration but had zero writers anywhere in the application (confirmed via
-- docs/CAPACITY_EVIDENCE_REPORT.md §8.7's live audit: 0 rows, 0 call sites).
-- This migration extends it with the minimum fields a real centralized error
-- service needs (dedup fingerprinting, occurrence counting, a proper
-- OPEN/ACKNOWLEDGED/RESOLVED/IGNORED lifecycle) and grants the write path that
-- was missing. The table has zero production rows, so `is_resolved` (unused by
-- any code) is replaced outright rather than kept alongside a new `status`
-- column — there is no data to migrate and no caller depends on it.

ALTER TABLE system_error_logs
    ADD COLUMN error_code VARCHAR(50),
    ADD COLUMN fingerprint VARCHAR(64),
    ADD COLUMN http_status SMALLINT,
    ADD COLUMN endpoint VARCHAR(200),
    ADD COLUMN occurrence_count INT NOT NULL DEFAULT 1,
    ADD COLUMN first_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    ADD COLUMN last_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    ADD COLUMN status VARCHAR(20) NOT NULL DEFAULT 'OPEN',
    ADD COLUMN acknowledged_by UUID REFERENCES users(id),
    ADD COLUMN acknowledged_at TIMESTAMPTZ,
    ADD COLUMN resolved_by UUID REFERENCES users(id),
    ADD COLUMN resolved_at TIMESTAMPTZ,
    ADD COLUMN resolution_note TEXT,
    DROP COLUMN is_resolved,
    ADD CONSTRAINT chk_system_error_status
        CHECK (status IN ('OPEN', 'ACKNOWLEDGED', 'RESOLVED', 'IGNORED'));

CREATE INDEX idx_system_logs_fingerprint ON system_error_logs(fingerprint, status);
CREATE INDEX idx_system_logs_status ON system_error_logs(status, logged_at);

-- The write path: errors are reported through `hellodoctor_system_worker` (the
-- "background/internal operational concern" role, matching the pre-existing
-- `GRANT SELECT, INSERT ON system_error_logs TO hellodoctor_system_worker`
-- design intent from the original RLS migration's own comment) via one
-- centralized pool handle set once at startup — not per-request, so no new
-- per-handler DB role plumbing is needed. Admin read/acknowledge/resolve runs
-- through the ordinary `hellodoctor_api` role, same as every other admin
-- handler, which already had SELECT; UPDATE is the only grant genuinely
-- missing for that path.
GRANT UPDATE ON system_error_logs TO hellodoctor_api;
-- `hellodoctor_system_worker` also needs UPDATE, not just INSERT: a recurring
-- failure with a matching open fingerprint bumps `occurrence_count` on the
-- existing row (dedup, see `system_error.rs::persist`) rather than inserting a
-- new one — confirmed necessary by a live test failure (`42501 permission
-- denied`) before this grant was added, not assumed.
GRANT UPDATE ON system_error_logs TO hellodoctor_system_worker;
