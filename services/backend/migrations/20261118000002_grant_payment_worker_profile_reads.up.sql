-- Forward-only fix: hellodoctor_payment_worker was never granted SELECT on
-- doctor_profiles or patient_profiles. appointments_access,
-- appointment_clinical_intake_access and prescription_intake_documents_access
-- (migrations/20261003000001_rls_hardening) are PUBLIC-applicable RLS
-- policies (no TO clause) that join both tables; Postgres requires every
-- role touching the protected table to have read access to tables referenced
-- by ANY applicable combined policy, even when a role-specific USING(true)
-- policy (e.g. appointments_payment_worker_all) already grants unconditional
-- access. Without this grant, payment_worker's expiry cleanup
-- (expire_locked_hold, used by both the background hold-expiry sweep and the
-- reclaim-on-booking path in expire_abandoned_hold_for_slot) failed closed
-- with "permission denied for table doctor_profiles" the moment it touched
-- appointment_clinical_intake or prescription_intake_documents -- confirmed
-- live by test_expired_unpaid_hold_purges_intake_metadata_and_objects and
-- test_expired_payment_hold_reclaimed_by_new_booking. Read-only: this never
-- grants payment_worker any write access to clinical profile data.
GRANT SELECT ON doctor_profiles, patient_profiles TO hellodoctor_payment_worker;

-- Same root cause, one function deeper: 20261106000004_encounter_history_reschedule
-- rewrote appointment_clinical_intake_access (and prescription_intake_documents'
-- own doctor-window policy) to call the SECURITY DEFINER function
-- doctor_own_encounter_allowed(...) instead of a raw doctor_profiles join, and
-- granted EXECUTE on it only to hellodoctor_api. The function's SECURITY
-- DEFINER body covers its own internal table reads, but EXECUTE on the
-- function itself is still required by every role whose RLS policy
-- evaluation can call it -- confirmed live by the same two tests, one layer
-- further in, immediately after the grant above landed.
GRANT EXECUTE ON FUNCTION doctor_own_encounter_allowed(UUID, UUID) TO hellodoctor_payment_worker;
