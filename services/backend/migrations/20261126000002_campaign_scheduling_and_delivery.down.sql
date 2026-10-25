REVOKE DELETE ON notifications FROM hellodoctor_api;
DROP POLICY IF EXISTS notifications_admin_cancel_campaign_delete ON notifications;

REVOKE SELECT, UPDATE ON campaigns FROM hellodoctor_system_worker;
DROP POLICY IF EXISTS campaigns_system_worker_all ON campaigns;

-- Restore the exact 6B trigger (DRAFT/PENDING_REVIEW/APPROVED/CANCELLED only).
CREATE OR REPLACE FUNCTION guard_campaign_update() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF NEW.id IS DISTINCT FROM OLD.id OR NEW.created_by IS DISTINCT FROM OLD.created_by
    OR NEW.creation_key IS DISTINCT FROM OLD.creation_key OR NEW.creation_hash IS DISTINCT FROM OLD.creation_hash
    OR NEW.created_at IS DISTINCT FROM OLD.created_at OR OLD.status = 'CANCELLED'
    OR NEW.version <> OLD.version + 1 THEN
    RAISE EXCEPTION 'Invalid campaign transition or identity change' USING ERRCODE = '23514';
  END IF;
  IF NOT (
    (OLD.status = 'DRAFT' AND NEW.status IN ('DRAFT', 'PENDING_REVIEW', 'CANCELLED')) OR
    (OLD.status = 'PENDING_REVIEW' AND NEW.status IN ('DRAFT', 'PENDING_REVIEW', 'APPROVED', 'CANCELLED')) OR
    (OLD.status = 'APPROVED' AND NEW.status IN ('DRAFT', 'APPROVED', 'CANCELLED'))
  ) THEN
    RAISE EXCEPTION 'Invalid campaign status transition' USING ERRCODE = '23514';
  END IF;
  IF NEW.status <> 'APPROVED'
    AND (NEW.approved_content_version IS NOT NULL OR NEW.approved_by IS NOT NULL OR NEW.approved_at IS NOT NULL) THEN
    RAISE EXCEPTION 'Approval metadata must be cleared whenever status is not APPROVED' USING ERRCODE = '23514';
  END IF;
  IF NEW.status = 'APPROVED' AND NEW.approved_content_version IS DISTINCT FROM NEW.version THEN
    RAISE EXCEPTION 'Approval must record the exact version being approved' USING ERRCODE = '23514';
  END IF;
  RETURN NEW;
END $$;

ALTER TABLE campaigns DROP CONSTRAINT IF EXISTS campaigns_sending_started_consistent;
ALTER TABLE campaigns DROP CONSTRAINT IF EXISTS campaigns_schedule_required;
ALTER TABLE campaigns DROP CONSTRAINT IF EXISTS campaigns_completed_at_matches_status;
ALTER TABLE campaigns DROP CONSTRAINT IF EXISTS campaigns_paused_at_matches_status;
ALTER TABLE campaigns DROP CONSTRAINT IF EXISTS campaigns_approved_content_version_matches;
ALTER TABLE campaigns DROP CONSTRAINT IF EXISTS campaigns_approval_bound;

DROP INDEX IF EXISTS campaigns_sending;
DROP INDEX IF EXISTS campaigns_scheduled_due;

ALTER TABLE campaigns
    DROP COLUMN IF EXISTS fanout_cursor,
    DROP COLUMN IF EXISTS completed_at,
    DROP COLUMN IF EXISTS paused_at,
    DROP COLUMN IF EXISTS sending_started_at,
    DROP COLUMN IF EXISTS scheduled_for;

ALTER TABLE campaigns RENAME COLUMN approved_content_version TO approved_version;
ALTER TABLE campaigns DROP COLUMN IF EXISTS content_version;

-- Explicit, collision-proof names — NOT a guess at Postgres's original
-- auto-generated names for the 6B inline `CREATE TABLE` CHECKs. The third
-- 6B inline CHECK (`(status = 'CANCELLED') = (cancelled_at IS NOT NULL)`)
-- was never touched by this migration's `up.sql` (its definition never
-- mentions `approved_version`, so the DO block there never drops it) and is
-- still present under whatever name Postgres originally gave it — reusing
-- that same auto-generated name here for a different constraint caused a
-- genuine `42710 duplicate_object` failure on `migrator.undo()`, caught by
-- this project's own migration-backfill regression tests, not assumed.
ALTER TABLE campaigns ADD CONSTRAINT campaigns_legacy_approval_bound CHECK (
    (status = 'APPROVED') = (approved_version IS NOT NULL AND approved_by IS NOT NULL AND approved_at IS NOT NULL)
);
ALTER TABLE campaigns ADD CONSTRAINT campaigns_legacy_approved_version_matches CHECK (
    approved_version IS NULL OR approved_version = version
);

-- The new `campaign_status` enum values themselves are not removable here —
-- see `20261126000001_campaign_status_new_values.down.sql`'s own comment.
