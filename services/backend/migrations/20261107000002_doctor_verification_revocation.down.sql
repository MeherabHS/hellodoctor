-- PostgreSQL cannot drop a value from an enum type (no ALTER TYPE ... DROP
-- VALUE exists), so 'REVOKED' cannot be removed from
-- professional_verification_status_enum here. This is a documented,
-- unavoidable PostgreSQL limitation, not an oversight: reverting the trigger
-- below means the value can never be written by any code path again, so its
-- mere presence in the type is inert.
CREATE OR REPLACE FUNCTION guard_professional_verification() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF current_user IN ('hellodoctor_api', 'hellodoctor_auth_worker') THEN
        IF TG_OP = 'INSERT' THEN
            IF NEW.is_verified OR NEW.professional_verification_status <> 'PENDING'
               OR NEW.professional_verification_reviewed_at IS NOT NULL
               OR NEW.professional_verification_reviewed_by IS NOT NULL
               OR NEW.professional_verification_note IS NOT NULL THEN
                RAISE EXCEPTION 'New Doctor registration must await professional review' USING ERRCODE='23514';
            END IF;
            IF NEW.license_authority <> 'KMPDC' OR NEW.license_country <> 'KE' THEN
                RAISE EXCEPTION 'Kenya professional registration requires KMPDC and KE' USING ERRCODE='23514';
            END IF;
        ELSIF NEW.professional_verification_status IS DISTINCT FROM OLD.professional_verification_status
              OR NEW.professional_verification_reviewed_at IS DISTINCT FROM OLD.professional_verification_reviewed_at
              OR NEW.professional_verification_reviewed_by IS DISTINCT FROM OLD.professional_verification_reviewed_by
              OR NEW.professional_verification_note IS DISTINCT FROM OLD.professional_verification_note THEN
            IF current_user <> 'hellodoctor_api'
               OR current_setting('app.current_role', true) NOT IN ('ADMIN')
               OR NULLIF(current_setting('app.current_role', true), '') IS NULL
               OR OLD.professional_verification_status <> 'PENDING'
               OR NEW.professional_verification_status NOT IN ('VERIFIED', 'REJECTED')
               OR NEW.professional_verification_reviewed_at IS NULL
               OR NEW.professional_verification_reviewed_by IS DISTINCT FROM NULLIF(current_setting('app.current_user_id', true), '')::uuid THEN
                RAISE EXCEPTION 'Professional review requires an authorized pending-to-terminal transition' USING ERRCODE='23514';
            END IF;
        END IF;
    END IF;
    NEW.is_verified := (NEW.professional_verification_status = 'VERIFIED');
    IF NOT NEW.is_verified THEN NEW.is_on_duty := false; END IF;
    RETURN NEW;
END $$;
