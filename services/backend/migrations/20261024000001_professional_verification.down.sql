-- Review states cannot be losslessly represented by the legacy boolean.
-- Refuse rollback if it would discard rejection/review evidence; take an explicit
-- archival/export decision before rolling this deployed migration back.
DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM doctor_profiles WHERE professional_verification_status='REJECTED'
               OR professional_verification_reviewed_at IS NOT NULL) THEN
        RAISE EXCEPTION 'Archive professional review evidence before rollback';
    END IF;
END $$;
DROP POLICY health_answers_doctor_write ON health_answers;
CREATE POLICY health_answers_doctor_write ON health_answers FOR INSERT TO hellodoctor_api
    WITH CHECK (current_setting('app.current_role',true)='DOCTOR' AND doctor_id IN
        (SELECT id FROM doctor_profiles WHERE user_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid AND is_verified));
DROP TRIGGER doctor_professional_verification_guard ON doctor_profiles;
DROP FUNCTION guard_professional_verification();
DROP INDEX doctor_pending_professional_verification;
ALTER TABLE doctor_profiles DROP CONSTRAINT doctor_professional_verification_consistent,
    DROP COLUMN professional_verification_note,
    DROP COLUMN professional_verification_reviewed_by,
    DROP COLUMN professional_verification_reviewed_at,
    DROP COLUMN professional_verification_status;
DROP TYPE professional_verification_status_enum;
ALTER TABLE doctor_profiles ALTER COLUMN license_authority DROP DEFAULT;
-- KE was already the prior deployment default; do not restore a foreign country.
