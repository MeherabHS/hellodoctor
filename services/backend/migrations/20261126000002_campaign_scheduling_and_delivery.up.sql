-- Milestone 6C — Campaign scheduling, dispatch and pause/cancel safety.
-- Extends the Milestone 6B `campaigns` entity with the states needed for
-- scheduling, sending, pausing and completion. Delivery itself continues to
-- go exclusively through the existing `notifications` outbox/scheduler/FCM
-- transport (`ADMIN_CAMPAIGN`, already registered since 20261117000001) —
-- no second send path, no new scheduler process, no new database.
--
-- The four new `campaign_status` values were added in the prior migration
-- (`20261126000001_campaign_status_new_values`), not here — see that file's
-- own comment for why using a newly added enum value in a CHECK constraint
-- or a partial index's WHERE predicate requires it to already be committed.
--
-- Replace the two 6B CHECK constraints that referenced `approved_version` by
-- name lookup (they were declared inline in `CREATE TABLE`, so Postgres
-- auto-named them) rather than guessing the generated name. Must run BEFORE
-- the column rename below: `ALTER TABLE ... RENAME COLUMN` rewrites existing
-- constraint definitions to the new name too, which would make an
-- `ILIKE '%approved_version%'` search afterward fail to match
-- `approved_content_version` (it is not a substring of it), silently leaving
-- the stale 6B constraints in place — hit and confirmed while developing
-- this migration (approving a campaign failed a since-renamed CHECK that
-- still compared against the old `version` column), not a hypothetical.
DO $$
DECLARE r record;
BEGIN
  FOR r IN SELECT conname FROM pg_constraint
    WHERE conrelid = 'campaigns'::regclass AND contype = 'c'
      AND pg_get_constraintdef(oid) ILIKE '%approved_version%'
  LOOP
    EXECUTE format('ALTER TABLE campaigns DROP CONSTRAINT %I', r.conname);
  END LOOP;
END $$;

-- `version` remains the pure optimistic-concurrency/row-revision counter
-- every mutation bumps (unchanged behavior for existing API callers).
-- `content_version` is new: it advances ONLY when title/body/audience
-- actually change (an edit), never for a lifecycle-only transition
-- (submit/approve/schedule/pause/resume/cancel/fan-out progress). Approval
-- is bound to `content_version`, not `version` — this is what makes
-- "approval survives scheduling/pausing" and "approval is invalidated by an
-- edit" both true at once, which plain `version` alone could not express
-- once lifecycle transitions after APPROVED also needed to bump a revision
-- counter for their own optimistic-concurrency checks.
ALTER TABLE campaigns ADD COLUMN content_version INTEGER NOT NULL DEFAULT 1 CHECK (content_version > 0);
ALTER TABLE campaigns RENAME COLUMN approved_version TO approved_content_version;

ALTER TABLE campaigns
    ADD COLUMN scheduled_for TIMESTAMPTZ,
    ADD COLUMN sending_started_at TIMESTAMPTZ,
    ADD COLUMN paused_at TIMESTAMPTZ,
    ADD COLUMN completed_at TIMESTAMPTZ,
    -- Keyset-pagination cursor (last processed user_id) for the resumable,
    -- bounded fan-out worker — see `run_campaign_dispatch_pass`. NULL means
    -- fan-out has not inserted anything yet.
    ADD COLUMN fanout_cursor UUID;

CREATE INDEX campaigns_scheduled_due ON campaigns(scheduled_for) WHERE status = 'SCHEDULED';
CREATE INDEX campaigns_sending ON campaigns(id) WHERE status = 'SENDING';

ALTER TABLE campaigns ADD CONSTRAINT campaigns_approval_bound CHECK (
    (status IN ('APPROVED', 'SCHEDULED', 'SENDING', 'PAUSED', 'COMPLETED'))
    = (approved_content_version IS NOT NULL AND approved_by IS NOT NULL AND approved_at IS NOT NULL)
);
ALTER TABLE campaigns ADD CONSTRAINT campaigns_approved_content_version_matches CHECK (
    approved_content_version IS NULL OR approved_content_version = content_version
);
ALTER TABLE campaigns ADD CONSTRAINT campaigns_paused_at_matches_status CHECK (
    (status = 'PAUSED') = (paused_at IS NOT NULL)
);
ALTER TABLE campaigns ADD CONSTRAINT campaigns_completed_at_matches_status CHECK (
    (status = 'COMPLETED') = (completed_at IS NOT NULL)
);
ALTER TABLE campaigns ADD CONSTRAINT campaigns_schedule_required CHECK (
    status NOT IN ('SCHEDULED', 'SENDING', 'PAUSED', 'COMPLETED') OR scheduled_for IS NOT NULL
);
ALTER TABLE campaigns ADD CONSTRAINT campaigns_sending_started_consistent CHECK (
    sending_started_at IS NULL OR status IN ('SENDING', 'PAUSED', 'COMPLETED', 'CANCELLED')
);

-- Full transition matrix, including the four new states:
--   DRAFT          -> DRAFT | PENDING_REVIEW | CANCELLED
--   PENDING_REVIEW -> DRAFT | PENDING_REVIEW | APPROVED | CANCELLED
--   APPROVED       -> DRAFT | SCHEDULED | CANCELLED
--   SCHEDULED      -> DRAFT | SCHEDULED (reschedule) | SENDING | PAUSED | CANCELLED
--   SENDING        -> SENDING (fan-out cursor progress) | PAUSED | COMPLETED | CANCELLED
--   PAUSED         -> SCHEDULED | SENDING | CANCELLED
--   COMPLETED, CANCELLED -> terminal
-- An edit (content change) is only reachable while not yet sending anything
-- (DRAFT/PENDING_REVIEW/APPROVED/SCHEDULED) and always forces DRAFT — a
-- campaign that has ever started fanning out can no longer have its content
-- changed at all, only paused/cancelled.
CREATE OR REPLACE FUNCTION guard_campaign_update() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
  bound_states campaign_status[] := ARRAY['APPROVED','SCHEDULED','SENDING','PAUSED','COMPLETED']::campaign_status[];
BEGIN
  IF NEW.id IS DISTINCT FROM OLD.id OR NEW.created_by IS DISTINCT FROM OLD.created_by
    OR NEW.creation_key IS DISTINCT FROM OLD.creation_key OR NEW.creation_hash IS DISTINCT FROM OLD.creation_hash
    OR NEW.created_at IS DISTINCT FROM OLD.created_at
    OR OLD.status IN ('CANCELLED', 'COMPLETED')
    OR NEW.version <> OLD.version + 1 THEN
    RAISE EXCEPTION 'Invalid campaign transition or identity change' USING ERRCODE = '23514';
  END IF;
  IF NOT (
    (OLD.status = 'DRAFT' AND NEW.status IN ('DRAFT', 'PENDING_REVIEW', 'CANCELLED')) OR
    (OLD.status = 'PENDING_REVIEW' AND NEW.status IN ('DRAFT', 'PENDING_REVIEW', 'APPROVED', 'CANCELLED')) OR
    (OLD.status = 'APPROVED' AND NEW.status IN ('DRAFT', 'SCHEDULED', 'CANCELLED')) OR
    (OLD.status = 'SCHEDULED' AND NEW.status IN ('DRAFT', 'SCHEDULED', 'SENDING', 'PAUSED', 'CANCELLED')) OR
    (OLD.status = 'SENDING' AND NEW.status IN ('SENDING', 'PAUSED', 'COMPLETED', 'CANCELLED')) OR
    (OLD.status = 'PAUSED' AND NEW.status IN ('SCHEDULED', 'SENDING', 'CANCELLED'))
  ) THEN
    RAISE EXCEPTION 'Invalid campaign status transition' USING ERRCODE = '23514';
  END IF;
  IF NOT (NEW.status = ANY(bound_states))
    AND (NEW.approved_content_version IS NOT NULL OR NEW.approved_by IS NOT NULL OR NEW.approved_at IS NOT NULL) THEN
    RAISE EXCEPTION 'Approval metadata must be cleared whenever status is not in an approved-or-later state' USING ERRCODE = '23514';
  END IF;
  IF NEW.status = 'APPROVED' AND NEW.approved_content_version IS DISTINCT FROM NEW.content_version THEN
    RAISE EXCEPTION 'Approval must record the exact content version being approved' USING ERRCODE = '23514';
  END IF;
  IF OLD.status = ANY(bound_states) AND NEW.status = ANY(bound_states)
    AND NEW.approved_content_version IS DISTINCT FROM OLD.approved_content_version THEN
    RAISE EXCEPTION 'Approved content version cannot change once bound' USING ERRCODE = '23514';
  END IF;
  IF NEW.content_version <> OLD.content_version AND NEW.status <> 'DRAFT' THEN
    RAISE EXCEPTION 'A content change must return the campaign to DRAFT' USING ERRCODE = '23514';
  END IF;
  RETURN NEW;
END $$;

-- Delivery worker access: the fan-out/promotion pass runs with no admin
-- session context (same shape as every other scheduler pass — see
-- `notification_scheduler.rs`'s use of `AuthContext::system()` +
-- `DbRole::SystemWorker`), so it needs its own unconditional policy rather
-- than the Admin-only ones from 6B.
CREATE POLICY campaigns_system_worker_all ON campaigns FOR ALL TO hellodoctor_system_worker USING (true) WITH CHECK (true);
GRANT SELECT, UPDATE ON campaigns TO hellodoctor_system_worker;

-- Cancelling a campaign must stop any not-yet-sent recipient notification
-- from ever being retried again. Scoped as narrowly as possible: an Admin
-- (`hellodoctor_api`) may only delete `notifications` rows that (a) are tied
-- to a campaign via `related_entity_type`/`related_entity_id`, and (b) that
-- campaign is ALREADY `CANCELLED` — never a general-purpose notification
-- delete. A row already `SENT_TO_PROVIDER` is never touched by this policy
-- either way (the handler's own DELETE predicate excludes it): an FCM
-- message already accepted by Google cannot be recalled, and this schema
-- makes no claim otherwise.
CREATE POLICY notifications_admin_cancel_campaign_delete ON notifications FOR DELETE TO hellodoctor_api USING (
    current_setting('app.current_role', true) = 'ADMIN'
    AND related_entity_type = 'CAMPAIGN'
    AND EXISTS (SELECT 1 FROM campaigns c WHERE c.id = notifications.related_entity_id AND c.status = 'CANCELLED')
);
GRANT DELETE ON notifications TO hellodoctor_api;
