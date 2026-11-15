-- Admin invitations (product owner, 2026-11-14): a signed-in Admin invites
-- another person by email; the invitee opens the emailed link, sets their own
-- phone number and password, and becomes an Admin. Only a SHA-256 hash of the
-- single-use token is stored. Invitations expire and can be revoked.

CREATE TABLE admin_invitations (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  email TEXT NOT NULL CHECK (email = lower(btrim(email)) AND char_length(email) BETWEEN 3 AND 254),
  token_hash TEXT NOT NULL UNIQUE CHECK (char_length(token_hash) = 64),
  invited_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  expires_at TIMESTAMPTZ NOT NULL,
  accepted_at TIMESTAMPTZ,
  accepted_user_id UUID REFERENCES users(id) ON DELETE RESTRICT,
  revoked_at TIMESTAMPTZ,
  revoked_by UUID REFERENCES users(id) ON DELETE RESTRICT,
  CHECK (expires_at > created_at),
  CHECK ((accepted_at IS NULL) = (accepted_user_id IS NULL)),
  CHECK ((revoked_at IS NULL) = (revoked_by IS NULL)),
  CHECK (accepted_at IS NULL OR revoked_at IS NULL)
);
-- At most one open (neither accepted nor revoked) invitation per email.
CREATE UNIQUE INDEX admin_invitations_one_open ON admin_invitations(email)
  WHERE accepted_at IS NULL AND revoked_at IS NULL;
CREATE INDEX idx_admin_invitations_created ON admin_invitations(created_at DESC);

ALTER TABLE admin_invitations ENABLE ROW LEVEL SECURITY;
ALTER TABLE admin_invitations FORCE ROW LEVEL SECURITY;
-- Signed-in Admins (API role) manage invitations.
CREATE POLICY admin_invitations_admin_read ON admin_invitations FOR SELECT TO hellodoctor_api
  USING (current_setting('app.current_role', true) = 'ADMIN');
CREATE POLICY admin_invitations_admin_insert ON admin_invitations FOR INSERT TO hellodoctor_api
  WITH CHECK (current_setting('app.current_role', true) = 'ADMIN'
    AND invited_by = NULLIF(current_setting('app.current_user_id', true), '')::uuid
    AND accepted_at IS NULL AND revoked_at IS NULL);
CREATE POLICY admin_invitations_admin_revoke ON admin_invitations FOR UPDATE TO hellodoctor_api
  USING (current_setting('app.current_role', true) = 'ADMIN' AND accepted_at IS NULL)
  WITH CHECK (current_setting('app.current_role', true) = 'ADMIN' AND accepted_at IS NULL
    AND revoked_by = NULLIF(current_setting('app.current_user_id', true), '')::uuid);
GRANT SELECT, INSERT ON admin_invitations TO hellodoctor_api;
GRANT UPDATE (revoked_at, revoked_by) ON admin_invitations TO hellodoctor_api;
-- The unauthenticated accept step runs on the auth worker (it also creates the user).
CREATE POLICY admin_invitations_auth_worker ON admin_invitations TO hellodoctor_auth_worker
  USING (true) WITH CHECK (true);
GRANT SELECT ON admin_invitations TO hellodoctor_auth_worker;
GRANT UPDATE (accepted_at, accepted_user_id) ON admin_invitations TO hellodoctor_auth_worker;

-- Defence in depth, whatever the application code does: identity columns
-- never change, a finished (accepted or revoked) invitation is final, an
-- expired or revoked invitation cannot be accepted, and an accepted one
-- cannot be revoked.
CREATE OR REPLACE FUNCTION guard_admin_invitation() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
  IF NEW.email IS DISTINCT FROM OLD.email OR NEW.token_hash IS DISTINCT FROM OLD.token_hash
     OR NEW.invited_by IS DISTINCT FROM OLD.invited_by OR NEW.created_at IS DISTINCT FROM OLD.created_at
     OR NEW.expires_at IS DISTINCT FROM OLD.expires_at THEN
    RAISE EXCEPTION 'Admin invitation identity columns are immutable' USING ERRCODE = 'check_violation';
  END IF;
  IF OLD.accepted_at IS NOT NULL OR OLD.revoked_at IS NOT NULL THEN
    RAISE EXCEPTION 'Admin invitation is already finished' USING ERRCODE = 'check_violation';
  END IF;
  IF NEW.accepted_at IS NOT NULL AND (NEW.revoked_at IS NOT NULL OR OLD.expires_at <= now()) THEN
    RAISE EXCEPTION 'Admin invitation is expired or revoked' USING ERRCODE = 'check_violation';
  END IF;
  RETURN NEW;
END $$;
CREATE TRIGGER trg_guard_admin_invitation BEFORE UPDATE ON admin_invitations
  FOR EACH ROW EXECUTE FUNCTION guard_admin_invitation();

-- The invitation email goes through the existing transactional outbox.
ALTER TABLE email_outbox DROP CONSTRAINT email_outbox_email_type_check;
ALTER TABLE email_outbox ADD CONSTRAINT email_outbox_email_type_check
    CHECK (email_type IN (
        'PATIENT_OTP', 'DOCTOR_OTP', 'ADMIN_OTP', 'PASSWORD_RESET_OTP',
        'PATIENT_WELCOME',
        'DOCTOR_APPLICATION_RECEIVED', 'DOCTOR_APPROVED', 'DOCTOR_REJECTED',
        'CONSULTATION_CONFIRMED', 'CONSULTATION_RESCHEDULED',
        'PAYMENT_CONFIRMED', 'REFUND_PROCESSED',
        'PASSWORD_CHANGED',
        'ADMIN_INVITATION'
    ));
