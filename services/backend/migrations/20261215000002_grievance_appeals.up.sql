-- Grievance appeals recorded by an Admin on the patient's behalf (product
-- owner, 2026-11-14; docs/admin-ui/ADMIN_REDESIGN_PLAN.md). An appeal is
-- against the current WARNED or DISMISSED decision. An Admin then UPHOLDS it
-- (decision stands) or OVERTURNS it: the decision is kept but marked
-- superseded, and the grievance returns to PENDING_REVIEW to be decided again
-- through the normal adjudication endpoints. Refund decisions are not
-- appealable: their finance flow (refund request) is already under way.
-- Writes go through the finance-worker role, like adjudication itself.

-- One *current* adjudication per grievance instead of one ever.
ALTER TABLE grievance_adjudications
  ADD COLUMN superseded_at TIMESTAMPTZ,
  ADD COLUMN superseded_by_appeal UUID;
ALTER TABLE grievance_adjudications DROP CONSTRAINT grievance_adjudications_grievance_id_key;
CREATE UNIQUE INDEX grievance_adjudications_one_current
  ON grievance_adjudications(grievance_id) WHERE superseded_at IS NULL;
ALTER TABLE grievance_adjudications ADD CONSTRAINT grievance_adjudications_superseded_pair
  CHECK ((superseded_at IS NULL) = (superseded_by_appeal IS NULL));

CREATE TABLE grievance_appeals (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  grievance_id UUID NOT NULL REFERENCES grievance_reports(id) ON DELETE CASCADE,
  appealed_adjudication_id UUID NOT NULL REFERENCES grievance_adjudications(id) ON DELETE RESTRICT,
  reason TEXT NOT NULL CHECK (char_length(btrim(reason)) BETWEEN 8 AND 2000),
  status VARCHAR(20) NOT NULL DEFAULT 'OPEN' CHECK (status IN ('OPEN', 'UPHELD', 'OVERTURNED')),
  recorded_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
  recorded_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  decided_by UUID REFERENCES users(id) ON DELETE RESTRICT,
  decided_at TIMESTAMPTZ,
  decision_note TEXT CHECK (decision_note IS NULL OR char_length(btrim(decision_note)) BETWEEN 8 AND 2000),
  CHECK ((status = 'OPEN') = (decided_at IS NULL)),
  CHECK ((status = 'OPEN') = (decided_by IS NULL)),
  CHECK ((status = 'OPEN') = (decision_note IS NULL))
);
CREATE UNIQUE INDEX grievance_appeals_one_open ON grievance_appeals(grievance_id) WHERE status = 'OPEN';
CREATE INDEX idx_grievance_appeals_recorded ON grievance_appeals(recorded_at DESC);

ALTER TABLE grievance_adjudications ADD CONSTRAINT grievance_adjudications_superseded_by_appeal_fkey
  FOREIGN KEY (superseded_by_appeal) REFERENCES grievance_appeals(id) ON DELETE RESTRICT;

ALTER TABLE grievance_appeals ENABLE ROW LEVEL SECURITY;
ALTER TABLE grievance_appeals FORCE ROW LEVEL SECURITY;
CREATE POLICY grievance_appeals_admin_read ON grievance_appeals FOR SELECT TO hellodoctor_api
  USING (current_setting('app.current_role', true) = 'ADMIN');
CREATE POLICY grievance_appeals_finance_worker_all ON grievance_appeals TO hellodoctor_finance_worker
  USING (true) WITH CHECK (true);
GRANT SELECT ON grievance_appeals TO hellodoctor_api;
GRANT SELECT, INSERT, UPDATE ON grievance_appeals TO hellodoctor_finance_worker;

-- The finance worker may mark an adjudication superseded, and nothing else.
GRANT SELECT, UPDATE (superseded_at, superseded_by_appeal) ON grievance_adjudications TO hellodoctor_finance_worker;
CREATE POLICY grievance_adjudications_finance_worker_supersede ON grievance_adjudications FOR UPDATE
  TO hellodoctor_finance_worker USING (superseded_at IS NULL) WITH CHECK (superseded_at IS NOT NULL);
CREATE POLICY grievance_adjudications_finance_worker_read ON grievance_adjudications FOR SELECT
  TO hellodoctor_finance_worker USING (true);
