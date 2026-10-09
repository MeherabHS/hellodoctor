-- Account, Doctor profile and empty wallet must commit or roll back together.
GRANT INSERT ON doctor_profiles, doctor_wallets TO hellodoctor_auth_worker;
GRANT SELECT ON doctor_wallets TO hellodoctor_auth_worker;
GRANT INSERT ON audit_events TO hellodoctor_auth_worker;
CREATE POLICY doctor_wallets_auth_bootstrap ON doctor_wallets FOR INSERT TO hellodoctor_auth_worker
    WITH CHECK (lifetime_gross=0 AND lifetime_fee_withheld=0 AND lifetime_net=0 AND pending_disbursement=0
        AND (current_setting('app.current_role', true) IN ('PLATFORM_ADMIN','CLINICAL_ADMIN')
        OR doctor_id IN (SELECT id FROM doctor_profiles WHERE user_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid)));

CREATE FUNCTION guard_doctor_self_registration() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF current_user='hellodoctor_auth_worker' AND current_setting('app.current_role',true)='DOCTOR'
       AND (NEW.user_id IS DISTINCT FROM NULLIF(current_setting('app.current_user_id',true),'')::uuid
            OR NEW.is_verified OR NEW.is_on_duty OR NEW.license_country <> 'KE') THEN
        RAISE EXCEPTION 'Doctor registration cannot assign clinical privileges' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER doctor_self_registration_guard BEFORE INSERT ON doctor_profiles
    FOR EACH ROW EXECUTE FUNCTION guard_doctor_self_registration();
