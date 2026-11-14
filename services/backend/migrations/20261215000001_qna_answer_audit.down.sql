DROP TRIGGER IF EXISTS trg_guard_health_answer_audit ON health_answers;
DROP FUNCTION IF EXISTS guard_health_answer_audit();
DROP POLICY IF EXISTS health_answers_admin_audit ON health_answers;
DROP TABLE IF EXISTS health_answer_reviews;
DROP INDEX IF EXISTS idx_health_answers_audit_status;
ALTER TABLE health_answers DROP COLUMN IF EXISTS audit_updated_at, DROP COLUMN IF EXISTS audit_status;
