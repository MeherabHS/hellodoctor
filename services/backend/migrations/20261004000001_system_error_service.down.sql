REVOKE UPDATE ON system_error_logs FROM hellodoctor_system_worker;
REVOKE UPDATE ON system_error_logs FROM hellodoctor_api;

DROP INDEX IF EXISTS idx_system_logs_status;
DROP INDEX IF EXISTS idx_system_logs_fingerprint;

ALTER TABLE system_error_logs
    DROP CONSTRAINT IF EXISTS chk_system_error_status,
    ADD COLUMN is_resolved BOOLEAN NOT NULL DEFAULT FALSE,
    DROP COLUMN IF EXISTS resolution_note,
    DROP COLUMN IF EXISTS resolved_at,
    DROP COLUMN IF EXISTS resolved_by,
    DROP COLUMN IF EXISTS acknowledged_at,
    DROP COLUMN IF EXISTS acknowledged_by,
    DROP COLUMN IF EXISTS status,
    DROP COLUMN IF EXISTS last_seen_at,
    DROP COLUMN IF EXISTS first_seen_at,
    DROP COLUMN IF EXISTS occurrence_count,
    DROP COLUMN IF EXISTS endpoint,
    DROP COLUMN IF EXISTS http_status,
    DROP COLUMN IF EXISTS fingerprint,
    DROP COLUMN IF EXISTS error_code;
