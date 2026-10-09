-- Kenya deployment: internal manual professional-registration review.
-- Registration is not an annual practice licence or an external registry check.
CREATE TYPE professional_verification_status_enum AS ENUM ('PENDING', 'VERIFIED', 'REJECTED');
ALTER TABLE doctor_profiles ALTER COLUMN license_authority SET DEFAULT 'KMPDC';
ALTER TABLE doctor_profiles ALTER COLUMN license_country SET DEFAULT 'KE';
ALTER TABLE doctor_profiles
    ADD COLUMN professional_verification_status professional_verification_status_enum NOT NULL DEFAULT 'PENDING',
    ADD COLUMN professional_verification_reviewed_at TIMESTAMPTZ,
    ADD COLUMN professional_verification_reviewed_by UUID REFERENCES users(id),
    ADD COLUMN professional_verification_note TEXT;

-- The latest successful historical review is evidence; false alone is not.
-- Legacy action names here are migration history, never newly generated actions.
WITH latest_review AS (
    SELECT DISTINCT ON (resource_id) resource_id, action, created_at, actor_id, reason
    FROM audit_events
    WHERE resource_type = 'DOCTOR_PROFILE'
      AND action IN ('BMDC_VERIFY', 'BMDC_REJECT')
      AND result IN ('SUCCESS', 'REJECTED')
    ORDER BY resource_id, created_at DESC, id DESC
)
UPDATE doctor_profiles d SET
    professional_verification_status = CASE
        WHEN r.action = 'BMDC_REJECT' THEN 'REJECTED'
        WHEN r.action = 'BMDC_VERIFY' OR d.is_verified THEN 'VERIFIED'
        ELSE 'PENDING'
    END::professional_verification_status_enum,
    professional_verification_reviewed_at = r.created_at,
    professional_verification_reviewed_by = (SELECT id FROM users WHERE id = r.actor_id),
    professional_verification_note = r.reason
FROM (SELECT p.id, r.* FROM doctor_profiles p LEFT JOIN latest_review r ON r.resource_id=p.id) r
WHERE d.id=r.id;
UPDATE doctor_profiles SET
    is_verified = (professional_verification_status = 'VERIFIED'),
    is_on_duty = CASE WHEN professional_verification_status = 'VERIFIED' THEN is_on_duty ELSE false END;
ALTER TABLE doctor_profiles ADD CONSTRAINT doctor_professional_verification_consistent
    CHECK (is_verified = (professional_verification_status = 'VERIFIED'));
CREATE INDEX doctor_pending_professional_verification ON doctor_profiles(created_at, id)
    WHERE professional_verification_status = 'PENDING' AND deleted_at IS NULL;

-- Compatibility boolean is derived. It never decides professional review state.
-- Existing capability roles and account suspension controls are unchanged.
CREATE FUNCTION guard_professional_verification() RETURNS trigger LANGUAGE plpgsql AS $$
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
               OR current_setting('app.current_role', true) NOT IN ('PLATFORM_ADMIN', 'CLINICAL_ADMIN')
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
CREATE TRIGGER doctor_professional_verification_guard BEFORE INSERT OR UPDATE ON doctor_profiles
    FOR EACH ROW EXECUTE FUNCTION guard_professional_verification();

-- Update the existing professional gate rather than changing its role/ownership policy.
DROP POLICY health_answers_doctor_write ON health_answers;
CREATE POLICY health_answers_doctor_write ON health_answers FOR INSERT TO hellodoctor_api
    WITH CHECK (current_setting('app.current_role',true)='DOCTOR' AND doctor_id IN
        (SELECT id FROM doctor_profiles WHERE user_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid
         AND professional_verification_status='VERIFIED'));
