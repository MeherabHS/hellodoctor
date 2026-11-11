-- No profile FK: the journal must survive a legitimate hard delete.
CREATE TABLE account_erasure_jobs (
 id UUID PRIMARY KEY,
 profile_kind TEXT NOT NULL CHECK(profile_kind IN ('PATIENT','DOCTOR')),
 profile_id UUID NOT NULL,
 user_id UUID REFERENCES users(id),
 requested_by UUID NOT NULL REFERENCES users(id),
 state TEXT NOT NULL DEFAULT 'PENDING_LOGIN'
   CHECK(state IN ('PENDING_LOGIN','PENDING_PROFILE','PENDING_MEDIA','COMPLETED')),
 outcome TEXT CHECK(outcome IN ('HARD_DELETED','ANONYMIZED')),
 photo_key TEXT,
 photo_bucket TEXT,
 attempts INTEGER NOT NULL DEFAULT 0 CHECK(attempts>=0),
 next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 claim_id UUID,
 lease_until TIMESTAMPTZ,
 last_error_code TEXT,
 created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 UNIQUE(profile_kind,profile_id),
 CHECK(state<>'COMPLETED' OR outcome IS NOT NULL)
);
CREATE INDEX account_erasure_pending ON account_erasure_jobs(next_attempt_at,id)
 WHERE state<>'COMPLETED';
ALTER TABLE account_erasure_jobs ENABLE ROW LEVEL SECURITY;
ALTER TABLE account_erasure_jobs FORCE ROW LEVEL SECURITY;
GRANT SELECT,INSERT ON account_erasure_jobs TO hellodoctor_api;
GRANT UPDATE(state,outcome,updated_at) ON account_erasure_jobs TO hellodoctor_api;
CREATE POLICY erasure_api_admin ON account_erasure_jobs TO hellodoctor_api
 USING(current_setting('app.current_role',true)='ADMIN')
 WITH CHECK(current_setting('app.current_role',true)='ADMIN');
-- Backend-only existence checks freeze photo writes after an erasure request.
CREATE POLICY erasure_api_subject_read ON account_erasure_jobs FOR SELECT TO hellodoctor_api
 USING(user_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid
 OR (current_setting('app.current_role',true)='PATIENT' AND profile_kind='PATIENT' AND profile_id IN(SELECT id FROM patient_profiles)));
GRANT SELECT ON account_erasure_jobs TO hellodoctor_auth_worker;
GRANT UPDATE(state,updated_at) ON account_erasure_jobs TO hellodoctor_auth_worker;
CREATE POLICY erasure_auth_admin ON account_erasure_jobs TO hellodoctor_auth_worker
 USING(current_setting('app.current_role',true)='ADMIN')
 WITH CHECK(current_setting('app.current_role',true)='ADMIN');
GRANT SELECT ON account_erasure_jobs TO hellodoctor_system_worker;
GRANT UPDATE(state,claim_id,lease_until,attempts,next_attempt_at,last_error_code,updated_at)
 ON account_erasure_jobs TO hellodoctor_system_worker;
CREATE POLICY erasure_system_admin ON account_erasure_jobs TO hellodoctor_system_worker
 USING(current_setting('app.current_role',true)='ADMIN')
 WITH CHECK(current_setting('app.current_role',true)='ADMIN');
CREATE FUNCTION protect_erasure_job() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF ROW(OLD.id,OLD.profile_kind,OLD.profile_id,OLD.user_id,OLD.requested_by,OLD.photo_key,OLD.photo_bucket)
    IS DISTINCT FROM ROW(NEW.id,NEW.profile_kind,NEW.profile_id,NEW.user_id,NEW.requested_by,NEW.photo_key,NEW.photo_bucket) THEN
  RAISE EXCEPTION 'Erasure subject and media identity are immutable' USING ERRCODE='23514';
 END IF;
 IF OLD.state<>NEW.state AND NOT (
   (OLD.state='PENDING_LOGIN' AND NEW.state='PENDING_PROFILE') OR
   (OLD.state='PENDING_PROFILE' AND NEW.state='PENDING_MEDIA') OR
   (OLD.state='PENDING_MEDIA' AND NEW.state='COMPLETED')) THEN
  RAISE EXCEPTION 'Invalid erasure phase transition' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END $$;
REVOKE ALL ON FUNCTION protect_erasure_job() FROM PUBLIC;
CREATE TRIGGER protect_erasure_job BEFORE UPDATE ON account_erasure_jobs
 FOR EACH ROW EXECUTE FUNCTION protect_erasure_job();
-- Also reject already-authenticated, in-flight owner updates after intent.
-- Admin remains authorized to perform the journal's bounded scrubbing phase.
CREATE POLICY patient_erasure_frozen ON patient_profiles AS RESTRICTIVE
 FOR UPDATE TO hellodoctor_api
 USING(current_setting('app.current_role',true)='ADMIN' OR NOT EXISTS(
   SELECT 1 FROM account_erasure_jobs j WHERE j.profile_kind='PATIENT' AND j.profile_id=patient_profiles.id))
 WITH CHECK(current_setting('app.current_role',true)='ADMIN' OR NOT EXISTS(
   SELECT 1 FROM account_erasure_jobs j WHERE j.profile_kind='PATIENT' AND j.profile_id=patient_profiles.id));
CREATE POLICY doctor_erasure_frozen ON doctor_profiles AS RESTRICTIVE
 FOR UPDATE TO hellodoctor_api
 USING(current_setting('app.current_role',true)='ADMIN' OR NOT EXISTS(
   SELECT 1 FROM account_erasure_jobs j WHERE j.profile_kind='DOCTOR' AND j.profile_id=doctor_profiles.id))
 WITH CHECK(current_setting('app.current_role',true)='ADMIN' OR NOT EXISTS(
   SELECT 1 FROM account_erasure_jobs j WHERE j.profile_kind='DOCTOR' AND j.profile_id=doctor_profiles.id));
