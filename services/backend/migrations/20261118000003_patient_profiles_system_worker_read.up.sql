-- Forward-only fix: doctor_profiles has an explicit unconditional read policy
-- for hellodoctor_system_worker (doctor_worker_read, see
-- migrations/20261026000001_canonical_user_isolation), but patient_profiles
-- never got the equivalent. patient_profiles_access (PUBLIC-applicable) only
-- returns true for an admin-role session variable, an exact user_id match, or
-- the guardian/doctor-treats helper functions -- all of which are NULL/false
-- for the scheduler's bare-pool queries (notification_scheduler.rs's
-- run_hold_notification_pass and ensure_hold_created_notification query
-- `&state.system_db` directly, never through begin_authed_as, so no
-- app.current_role/app.current_user_id session variable is ever set on that
-- connection). The table-level GRANT SELECT already added in
-- 20261113000001_payment_hold_cleanup silently passed the coarse privilege
-- check while RLS then filtered every row to zero -- no error, just an empty
-- result, so a Patient's booking-hold-created/-expired notification was
-- silently never created. Confirmed live by
-- test_expired_unpaid_hold_purges_intake_metadata_and_objects's
-- booking-hold-expired notification assertion.
CREATE POLICY patient_profiles_system_worker_read ON patient_profiles
    FOR SELECT TO hellodoctor_system_worker USING (true);
