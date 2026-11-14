-- Q&A answer audit (product owner, 2026-11-14; docs/admin-ui/ADMIN_REDESIGN_PLAN.md).
-- An Admin marks a doctor's answer APPROVED or FLAGGED with a note. Every
-- review is kept. A FLAGGED answer is hidden from patients until an Admin
-- approves it again. The patient author is never exposed to the Admin.

ALTER TABLE health_answers
  ADD COLUMN audit_status VARCHAR(20) NOT NULL DEFAULT 'UNREVIEWED'
    CHECK (audit_status IN ('UNREVIEWED', 'APPROVED', 'FLAGGED')),
  ADD COLUMN audit_updated_at TIMESTAMPTZ;

CREATE INDEX idx_health_answers_audit_status ON health_answers(audit_status, created_at DESC);

CREATE TABLE health_answer_reviews (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  answer_id UUID NOT NULL REFERENCES health_answers(id) ON DELETE CASCADE,
  decision VARCHAR(20) NOT NULL CHECK (decision IN ('APPROVED', 'FLAGGED')),
  note TEXT NOT NULL CHECK (char_length(btrim(note)) BETWEEN 1 AND 1000),
  reviewed_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
  reviewed_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_health_answer_reviews_answer ON health_answer_reviews(answer_id, reviewed_at DESC);

ALTER TABLE health_answer_reviews ENABLE ROW LEVEL SECURITY;
ALTER TABLE health_answer_reviews FORCE ROW LEVEL SECURITY;
CREATE POLICY health_answer_reviews_admin_read ON health_answer_reviews FOR SELECT TO hellodoctor_api
  USING (current_setting('app.current_role', true) = 'ADMIN');
CREATE POLICY health_answer_reviews_admin_insert ON health_answer_reviews FOR INSERT TO hellodoctor_api
  WITH CHECK (current_setting('app.current_role', true) = 'ADMIN'
    AND reviewed_by = NULLIF(current_setting('app.current_user_id', true), '')::uuid);
GRANT SELECT, INSERT ON health_answer_reviews TO hellodoctor_api;

-- Admins may change only the audit columns of an answer.
CREATE POLICY health_answers_admin_audit ON health_answers FOR UPDATE TO hellodoctor_api
  USING (current_setting('app.current_role', true) = 'ADMIN')
  WITH CHECK (current_setting('app.current_role', true) = 'ADMIN');

CREATE OR REPLACE FUNCTION guard_health_answer_audit() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
  IF current_setting('app.current_role', true) = 'ADMIN'
     AND (NEW.answer_text IS DISTINCT FROM OLD.answer_text
          OR NEW.doctor_id IS DISTINCT FROM OLD.doctor_id
          OR NEW.question_id IS DISTINCT FROM OLD.question_id
          OR NEW.created_at IS DISTINCT FROM OLD.created_at
          OR NEW.updated_at IS DISTINCT FROM OLD.updated_at) THEN
    RAISE EXCEPTION 'An Admin review may change only the audit status of an answer'
      USING ERRCODE = 'check_violation';
  END IF;
  RETURN NEW;
END $$;

CREATE TRIGGER trg_guard_health_answer_audit BEFORE UPDATE ON health_answers
  FOR EACH ROW EXECUTE FUNCTION guard_health_answer_audit();
