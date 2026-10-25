-- Admin notification campaigns — Milestone 6B: backend foundations only.
-- Dispatch, scheduling and fan-out are explicitly out of scope here: this
-- migration adds only the campaign entity and its draft -> review -> approval
-- lifecycle. Delivery will later reuse the existing `notifications` outbox
-- and scheduler (the `ADMIN_CAMPAIGN` notification type already registered
-- in notification_type_registry since 20261117000001) rather than a second
-- notification pipeline — no recipient/delivery table is added here.
CREATE TYPE campaign_status AS ENUM ('DRAFT', 'PENDING_REVIEW', 'APPROVED', 'CANCELLED');
CREATE TYPE campaign_audience AS ENUM ('PATIENTS', 'DOCTORS', 'BOTH');

CREATE TABLE campaigns (
    id UUID PRIMARY KEY,
    created_by UUID NOT NULL REFERENCES users(id),
    title_en TEXT NOT NULL CHECK (char_length(title_en) BETWEEN 1 AND 160),
    title_sw TEXT NOT NULL CHECK (char_length(title_sw) BETWEEN 1 AND 160),
    body_en TEXT NOT NULL CHECK (char_length(body_en) BETWEEN 1 AND 600),
    body_sw TEXT NOT NULL CHECK (char_length(body_sw) BETWEEN 1 AND 600),
    audience campaign_audience NOT NULL,
    status campaign_status NOT NULL DEFAULT 'DRAFT',
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    approved_version INTEGER,
    approved_by UUID REFERENCES users(id),
    approved_at TIMESTAMPTZ,
    creation_key TEXT NOT NULL CHECK (char_length(creation_key) BETWEEN 16 AND 64),
    creation_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    cancelled_at TIMESTAMPTZ,
    UNIQUE (created_by, creation_key),
    -- Approval is always for the exact current version, never stale: these two
    -- biconditionals are what make "edits cannot inherit an old approval" a
    -- schema-level invariant rather than only an application convention.
    CHECK ((status = 'APPROVED') = (approved_version IS NOT NULL AND approved_by IS NOT NULL AND approved_at IS NOT NULL)),
    CHECK ((status = 'CANCELLED') = (cancelled_at IS NOT NULL)),
    CHECK (approved_version IS NULL OR approved_version = version)
);
CREATE INDEX campaigns_created_at ON campaigns(created_at DESC, id);
CREATE INDEX campaigns_status ON campaigns(status, created_at DESC, id);

ALTER TABLE campaigns ENABLE ROW LEVEL SECURITY;
ALTER TABLE campaigns FORCE ROW LEVEL SECURITY;

-- Campaigns are a shared Admin operational resource: any canonical ADMIN can
-- see and manage any campaign (same shape as the grievance/finance admin
-- tables), not scoped to its own creator. There is no Patient/Doctor read
-- policy at all — campaigns are never directly visible to recipients; only
-- the eventual per-recipient `notifications` row will be, once a later phase
-- wires up dispatch.
CREATE POLICY campaigns_admin_read ON campaigns FOR SELECT USING (
    current_setting('app.current_role', true) = 'ADMIN'
);
CREATE POLICY campaigns_admin_insert ON campaigns FOR INSERT WITH CHECK (
    current_setting('app.current_role', true) = 'ADMIN'
    AND created_by = NULLIF(current_setting('app.current_user_id', true), '')::uuid
    AND status = 'DRAFT'
);
CREATE POLICY campaigns_admin_update ON campaigns FOR UPDATE USING (
    current_setting('app.current_role', true) = 'ADMIN'
) WITH CHECK (
    current_setting('app.current_role', true) = 'ADMIN'
);

-- Keep identity immutable, enforce one-version-at-a-time optimistic
-- concurrency, and restrict status transitions to the lifecycle below.
-- CANCELLED is terminal. An edit (content change) always moves the row back
-- to DRAFT regardless of its prior status, and whenever the resulting status
-- is not APPROVED the approval columns must already be cleared — enforced
-- here, not just by the table-level CHECK, so a direct SQL UPDATE through the
-- ordinary API capability role cannot silently leave a stale approval behind.
--
--   DRAFT          -> DRAFT | PENDING_REVIEW | CANCELLED
--   PENDING_REVIEW -> DRAFT | PENDING_REVIEW | APPROVED | CANCELLED
--   APPROVED       -> DRAFT | APPROVED | CANCELLED
--   CANCELLED      -> (terminal)
CREATE FUNCTION guard_campaign_update() RETURNS trigger LANGUAGE plpgsql AS $$
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
    AND (NEW.approved_version IS NOT NULL OR NEW.approved_by IS NOT NULL OR NEW.approved_at IS NOT NULL) THEN
    RAISE EXCEPTION 'Approval metadata must be cleared whenever status is not APPROVED' USING ERRCODE = '23514';
  END IF;
  IF NEW.status = 'APPROVED' AND NEW.approved_version IS DISTINCT FROM NEW.version THEN
    RAISE EXCEPTION 'Approval must record the exact version being approved' USING ERRCODE = '23514';
  END IF;
  RETURN NEW;
END $$;
CREATE TRIGGER campaign_update_guard BEFORE UPDATE ON campaigns
FOR EACH ROW EXECUTE FUNCTION guard_campaign_update();
GRANT SELECT, INSERT, UPDATE ON campaigns TO hellodoctor_api;
