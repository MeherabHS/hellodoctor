-- Pass 1 of the multi-agent security/correctness audit (docs/CHECKPOINT.md):
-- schema changes backing the MFA brute-force fix, the document-download
-- quarantine/security-status gate, and removal of a dead-but-dangerous RLS
-- bypass clause. The two financial race-condition fixes (wallet double-
-- credit, disbursement double-pay) need no schema change — they're fixed by
-- making existing UPDATE statements conditional, in application code.

-- ----------------------------------------------------------------------------
-- Doctor MFA brute-force fix: a per-challenge attempt counter, checked and
-- incremented atomically (via `SELECT ... FOR UPDATE`) in doctor_verify_otp.
-- Previously a verified password-login session could be hammered with
-- unlimited OTP guesses — only the generic 120/min global IP limit applied.
-- ----------------------------------------------------------------------------
ALTER TABLE pending_mfa_challenges
    ADD COLUMN attempt_count SMALLINT NOT NULL DEFAULT 0,
    ADD COLUMN max_attempts SMALLINT NOT NULL DEFAULT 5;

-- ----------------------------------------------------------------------------
-- Document-download security-status gate. Today every row that exists was
-- already synchronously validated at upload time (image: full decode/re-
-- encode; PDF: structural active-content/encryption rejection — see
-- UploadSecurityService) — a row that failed validation is never inserted at
-- all, so every existing/future row is 'CLEAN' by construction. The column
-- exists so a *future* async re-scan (real ClamAV/antivirus integration,
-- not implemented yet — see UploadSecurityService's own doc comment) can
-- downgrade a row to 'REJECTED' after the fact without any API/schema
-- change, and so the download endpoint has something real to gate on now
-- rather than retrofitting it later.
-- ----------------------------------------------------------------------------
ALTER TABLE prescription_intake_documents
    ADD COLUMN security_status VARCHAR(20) NOT NULL DEFAULT 'CLEAN'
        CHECK (security_status IN ('CLEAN', 'REJECTED'));

ALTER TABLE prescriptions
    ADD COLUMN security_status VARCHAR(20) NOT NULL DEFAULT 'CLEAN'
        CHECK (security_status IN ('CLEAN', 'REJECTED'));

-- ----------------------------------------------------------------------------
-- Remove the dead-but-dangerous audit_events RLS bypass clause. The Super-
-- Admin-only restriction added in migrations/20261004000003 kept
-- `OR current_user <> 'hellodoctor_api'` for the worker-role pools — today
-- genuinely inert (no worker role has ever been granted SELECT on
-- audit_events; AuditService::log always writes via the API role), but it
-- would silently re-open the just-closed hole the moment any worker role
-- is ever granted SELECT here, with no warning. Switched to pure positive
-- authorization: only the named role may read.
-- ----------------------------------------------------------------------------
DROP POLICY audit_events_select ON audit_events;
CREATE POLICY audit_events_select ON audit_events FOR SELECT USING (
    current_setting('app.current_role', true) = 'SECURITY_ADMIN'
);
