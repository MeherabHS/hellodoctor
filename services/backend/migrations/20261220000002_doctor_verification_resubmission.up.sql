-- Doctor Web redesign (2026-11-14, product-owner approved): a Doctor whose professional
-- registration was REJECTED may upload supporting documents and resubmit, which returns
-- the registration to PENDING for a new Admin review. A PENDING Doctor may also add
-- documents. Nothing else about the review model changes.
CREATE TABLE doctor_verification_submissions (
    id UUID PRIMARY KEY,
    doctor_id UUID NOT NULL REFERENCES doctor_profiles(id),
    message TEXT CHECK (char_length(message) BETWEEN 1 AND 1000),
    previous_status professional_verification_status_enum NOT NULL CHECK (previous_status IN ('PENDING', 'REJECTED')),
    previous_note TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX doctor_verification_submissions_doctor ON doctor_verification_submissions(doctor_id, created_at DESC);
CREATE TABLE doctor_verification_documents (
    id UUID PRIMARY KEY,
    submission_id UUID NOT NULL REFERENCES doctor_verification_submissions(id),
    doctor_id UUID NOT NULL REFERENCES doctor_profiles(id),
    object_key TEXT NOT NULL UNIQUE CHECK (object_key = 'doctor-verification/' || doctor_id::text || '/' || id::text || '.bin'),
    mime_type TEXT NOT NULL CHECK (mime_type IN ('image/jpeg', 'application/pdf')),
    content_hash TEXT NOT NULL,
    size_bytes INTEGER NOT NULL CHECK (size_bytes BETWEEN 1 AND 10485760),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX doctor_verification_documents_doctor ON doctor_verification_documents(doctor_id, created_at DESC);

ALTER TABLE doctor_verification_submissions ENABLE ROW LEVEL SECURITY;
ALTER TABLE doctor_verification_submissions FORCE ROW LEVEL SECURITY;
ALTER TABLE doctor_verification_documents ENABLE ROW LEVEL SECURITY;
ALTER TABLE doctor_verification_documents FORCE ROW LEVEL SECURITY;
CREATE POLICY doctor_verification_submissions_access ON doctor_verification_submissions FOR SELECT TO hellodoctor_api USING (
    current_setting('app.current_role', true) = 'ADMIN'
    OR (current_setting('app.current_role', true) = 'DOCTOR' AND EXISTS (
        SELECT 1 FROM doctor_profiles d WHERE d.id = doctor_id AND d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)));
CREATE POLICY doctor_verification_submissions_insert ON doctor_verification_submissions FOR INSERT TO hellodoctor_api WITH CHECK (
    current_setting('app.current_role', true) = 'DOCTOR' AND EXISTS (
        SELECT 1 FROM doctor_profiles d WHERE d.id = doctor_id AND d.deleted_at IS NULL
          AND d.professional_verification_status IN ('PENDING', 'REJECTED')
          AND d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid));
CREATE POLICY doctor_verification_documents_access ON doctor_verification_documents FOR SELECT TO hellodoctor_api USING (
    current_setting('app.current_role', true) = 'ADMIN'
    OR (current_setting('app.current_role', true) = 'DOCTOR' AND EXISTS (
        SELECT 1 FROM doctor_profiles d WHERE d.id = doctor_id AND d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)));
CREATE POLICY doctor_verification_documents_insert ON doctor_verification_documents FOR INSERT TO hellodoctor_api WITH CHECK (
    current_setting('app.current_role', true) = 'DOCTOR' AND EXISTS (
        SELECT 1 FROM doctor_profiles d WHERE d.id = doctor_id AND d.deleted_at IS NULL
          AND d.professional_verification_status IN ('PENDING', 'REJECTED')
          AND d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    AND EXISTS (SELECT 1 FROM doctor_verification_submissions s WHERE s.id = submission_id AND s.doctor_id = doctor_verification_documents.doctor_id));
GRANT SELECT, INSERT ON doctor_verification_submissions, doctor_verification_documents TO hellodoctor_api;

-- Same guard as 20261107000002 plus one transition: REJECTED -> PENDING, made only by
-- the Doctor who owns the profile, and only by clearing the review fields.
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
            IF current_user = 'hellodoctor_api'
               AND current_setting('app.current_role', true) = 'DOCTOR'
               AND OLD.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
               AND OLD.professional_verification_status = 'REJECTED'
               AND NEW.professional_verification_status = 'PENDING'
               AND NEW.professional_verification_reviewed_at IS NULL
               AND NEW.professional_verification_reviewed_by IS NULL
               AND NEW.professional_verification_note IS NULL THEN
                NULL; -- Doctor resubmission.
            ELSIF current_user <> 'hellodoctor_api'
               OR current_setting('app.current_role', true) NOT IN ('ADMIN')
               OR NULLIF(current_setting('app.current_role', true), '') IS NULL
               OR NOT (
                    (OLD.professional_verification_status = 'PENDING' AND NEW.professional_verification_status IN ('VERIFIED', 'REJECTED'))
                    OR (OLD.professional_verification_status = 'VERIFIED' AND NEW.professional_verification_status = 'REVOKED')
                  )
               OR NEW.professional_verification_reviewed_at IS NULL
               OR NEW.professional_verification_reviewed_by IS DISTINCT FROM NULLIF(current_setting('app.current_user_id', true), '')::uuid THEN
                RAISE EXCEPTION 'Professional review requires an authorized pending-to-terminal or verified-to-revoked transition' USING ERRCODE='23514';
            END IF;
        END IF;
    END IF;
    NEW.is_verified := (NEW.professional_verification_status = 'VERIFIED');
    IF NOT NEW.is_verified THEN NEW.is_on_duty := false; END IF;
    RETURN NEW;
END $$;
