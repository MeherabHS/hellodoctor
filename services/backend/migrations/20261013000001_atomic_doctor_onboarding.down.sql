DROP TRIGGER IF EXISTS doctor_self_registration_guard ON doctor_profiles;
DROP FUNCTION IF EXISTS guard_doctor_self_registration();
DROP POLICY IF EXISTS doctor_wallets_auth_bootstrap ON doctor_wallets;
REVOKE INSERT ON doctor_profiles, doctor_wallets FROM hellodoctor_auth_worker;
REVOKE SELECT ON doctor_wallets FROM hellodoctor_auth_worker;
REVOKE INSERT ON audit_events FROM hellodoctor_auth_worker;
