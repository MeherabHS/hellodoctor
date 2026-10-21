DROP POLICY doctor_private_delete ON doctor_profiles;
REVOKE DELETE ON patient_profiles, doctor_profiles FROM hellodoctor_api;
