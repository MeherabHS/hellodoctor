-- Remediation E1: durable outbox from real account-erasure completion to an
-- externally-custodied, HMAC-chained journal (ops/dr/erasure_journal.py
-- compatible format), plus a recovery-instance reconciliation gate that must
-- pass before a restored environment may resume ordinary authentication.
-- Both tables are intentionally plain Postgres rows: they are the durable
-- record of application-side intent and evidence, not the tamper-evident
-- source of truth itself (that remains the externally-stored journal file).

CREATE TABLE durable_erasure_journal_outbox (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
 erasure_job_id UUID NOT NULL REFERENCES account_erasure_jobs(id),
 event_id TEXT NOT NULL UNIQUE,
 subject_ref TEXT NOT NULL,
 state TEXT NOT NULL DEFAULT 'COMPLETED' CHECK(state IN ('COMPLETED')),
 retention_class TEXT NOT NULL DEFAULT 'clinical-retention',
 recorded_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 journal_synced_at TIMESTAMPTZ,
 journal_sync_attempts INTEGER NOT NULL DEFAULT 0 CHECK(journal_sync_attempts>=0),
 last_sync_error TEXT,
 created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
CREATE INDEX durable_erasure_journal_outbox_pending
 ON durable_erasure_journal_outbox(created_at,id) WHERE journal_synced_at IS NULL;
ALTER TABLE durable_erasure_journal_outbox ENABLE ROW LEVEL SECURITY;
ALTER TABLE durable_erasure_journal_outbox FORCE ROW LEVEL SECURITY;
GRANT SELECT,INSERT ON durable_erasure_journal_outbox TO hellodoctor_system_worker;
GRANT UPDATE(journal_synced_at,journal_sync_attempts,last_sync_error)
 ON durable_erasure_journal_outbox TO hellodoctor_system_worker;
CREATE POLICY erasure_outbox_system ON durable_erasure_journal_outbox TO hellodoctor_system_worker
 USING(current_setting('app.current_role',true)='ADMIN')
 WITH CHECK(current_setting('app.current_role',true)='ADMIN');
CREATE FUNCTION protect_erasure_outbox() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF ROW(OLD.erasure_job_id,OLD.event_id,OLD.subject_ref,OLD.state,OLD.retention_class)
    IS DISTINCT FROM ROW(NEW.erasure_job_id,NEW.event_id,NEW.subject_ref,NEW.state,NEW.retention_class) THEN
  RAISE EXCEPTION 'Erasure outbox event identity is immutable' USING ERRCODE='23514';
 END IF;
 IF OLD.journal_synced_at IS NOT NULL AND NEW.journal_synced_at IS NULL THEN
  RAISE EXCEPTION 'A synced outbox entry cannot be unmarked' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END $$;
REVOKE ALL ON FUNCTION protect_erasure_outbox() FROM PUBLIC;
CREATE TRIGGER protect_erasure_outbox BEFORE UPDATE ON durable_erasure_journal_outbox
 FOR EACH ROW EXECUTE FUNCTION protect_erasure_outbox();

-- One row per real disaster-recovery incident. Created (status
-- AWAITING_RECONCILIATION) by the application itself the moment it boots
-- with RECOVERY_MODE=true, so the obligation exists even if the operator
-- never runs the reconciliation tool. A row can only move forward
-- (AWAITING_RECONCILIATION/REVIEW_REQUIRED -> PASSED/FAILED/REVIEW_REQUIRED);
-- it can never be silently downgraded once PASSED.
CREATE TABLE dr_recovery_state (
 recovery_instance_id TEXT PRIMARY KEY,
 recovery_point TEXT,
 status TEXT NOT NULL DEFAULT 'AWAITING_RECONCILIATION'
   CHECK(status IN ('AWAITING_RECONCILIATION','REVIEW_REQUIRED','PASSED','FAILED')),
 journal_tail_sequence INTEGER,
 journal_tail_mac TEXT,
 details JSONB,
 created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
ALTER TABLE dr_recovery_state ENABLE ROW LEVEL SECURITY;
ALTER TABLE dr_recovery_state FORCE ROW LEVEL SECURITY;
GRANT SELECT,INSERT ON dr_recovery_state TO hellodoctor_system_worker;
GRANT UPDATE(status,recovery_point,journal_tail_sequence,journal_tail_mac,details,updated_at)
 ON dr_recovery_state TO hellodoctor_system_worker;
CREATE POLICY dr_recovery_state_system ON dr_recovery_state TO hellodoctor_system_worker
 USING(current_setting('app.current_role',true)='ADMIN')
 WITH CHECK(current_setting('app.current_role',true)='ADMIN');
CREATE FUNCTION protect_dr_recovery_state() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF OLD.status='PASSED' AND NEW.status<>'PASSED' THEN
  RAISE EXCEPTION 'Recovery reconciliation state cannot be downgraded after passing' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END $$;
REVOKE ALL ON FUNCTION protect_dr_recovery_state() FROM PUBLIC;
CREATE TRIGGER protect_dr_recovery_state BEFORE UPDATE ON dr_recovery_state
 FOR EACH ROW EXECUTE FUNCTION protect_dr_recovery_state();
