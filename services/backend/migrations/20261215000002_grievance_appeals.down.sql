-- Restoring "one adjudication per grievance ever" is only possible while no
-- decision has been superseded by an overturned appeal; refuse otherwise
-- rather than deleting adjudication history.
DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM grievance_adjudications WHERE superseded_at IS NOT NULL) THEN
    RAISE EXCEPTION 'Cannot revert grievance appeals: superseded adjudications exist. Export and reconcile them first.';
  END IF;
END $$;

DROP POLICY IF EXISTS grievance_adjudications_finance_worker_read ON grievance_adjudications;
DROP POLICY IF EXISTS grievance_adjudications_finance_worker_supersede ON grievance_adjudications;
REVOKE SELECT, UPDATE (superseded_at, superseded_by_appeal) ON grievance_adjudications FROM hellodoctor_finance_worker;
REVOKE SELECT ON grievance_adjudications FROM hellodoctor_finance_worker;
ALTER TABLE grievance_adjudications DROP CONSTRAINT IF EXISTS grievance_adjudications_superseded_by_appeal_fkey;
DROP TABLE IF EXISTS grievance_appeals;
ALTER TABLE grievance_adjudications DROP CONSTRAINT IF EXISTS grievance_adjudications_superseded_pair;
DROP INDEX IF EXISTS grievance_adjudications_one_current;
ALTER TABLE grievance_adjudications DROP COLUMN IF EXISTS superseded_by_appeal, DROP COLUMN IF EXISTS superseded_at;
ALTER TABLE grievance_adjudications ADD CONSTRAINT grievance_adjudications_grievance_id_key UNIQUE (grievance_id);
