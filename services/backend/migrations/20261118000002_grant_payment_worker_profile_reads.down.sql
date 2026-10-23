REVOKE EXECUTE ON FUNCTION doctor_own_encounter_allowed(UUID, UUID) FROM hellodoctor_payment_worker;
REVOKE SELECT ON doctor_profiles, patient_profiles FROM hellodoctor_payment_worker;
