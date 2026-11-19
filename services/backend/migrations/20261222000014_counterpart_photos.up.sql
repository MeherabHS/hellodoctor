-- Profile photos for Peer Navigators, and the "other person's photo" in a
-- consultation or peer session (product owner, 2026-11-19: doctors and peers
-- saw initials instead of the patient's saved photo, patients saw initials
-- instead of the peer's).
--
-- Peer photos use the same object-reference columns as patient and doctor
-- photos (20261112000001_profile_pictures): the encrypted bytes live in the
-- private object store; all columns are nullable, so existing peers simply
-- have no photo until they upload one. The existing peer_profiles_update
-- policy (own row or Admin) already scopes who may write them.
ALTER TABLE peer_profiles
    ADD COLUMN profile_picture_object_key TEXT,
    ADD COLUMN profile_picture_bucket TEXT,
    ADD COLUMN profile_picture_content_hash TEXT,
    ADD COLUMN profile_picture_mime_type TEXT,
    ADD COLUMN profile_picture_size_bytes INTEGER,
    ADD COLUMN profile_picture_updated_at TIMESTAMPTZ;

-- The counterpart's photo in one consultation: the doctor of the appointment
-- gets the patient profile's photo; the patient (or guardian) who owns the
-- appointment gets the doctor's photo. Anyone else gets NULL. Same narrowly
-- scoped SECURITY DEFINER shape as doctor_public_photo: the access condition
-- is the appointments_access policy's own patient/doctor branches, evaluated
-- inside the function, never a blanket RLS bypass. Only the storage
-- reference is returned, never any other patient field.
CREATE FUNCTION appointment_counterpart_photo(p_appointment_id UUID)
RETURNS JSONB LANGUAGE sql SECURITY DEFINER SET search_path = pg_catalog, public STABLE AS $$
    SELECT CASE
        WHEN a.doctor_id = fn_current_doctor_profile_id() THEN (
            SELECT jsonb_build_object('bucket', p.profile_picture_bucket, 'key', p.profile_picture_object_key,
                'hash', p.profile_picture_content_hash, 'mime', p.profile_picture_mime_type)
            FROM patient_profiles p
            WHERE p.id = a.patient_id AND p.profile_picture_object_key IS NOT NULL)
        WHEN fn_patient_profile_owned_by_caller(a.patient_id,
                NULLIF(current_setting('app.current_user_id', true), '')::uuid) THEN (
            SELECT jsonb_build_object('bucket', d.profile_picture_bucket, 'key', d.profile_picture_object_key,
                'hash', d.profile_picture_content_hash, 'mime', d.profile_picture_mime_type)
            FROM doctor_profiles d
            WHERE d.id = a.doctor_id AND d.deleted_at IS NULL AND d.profile_picture_object_key IS NOT NULL)
        END
    FROM appointments a
    WHERE a.id = p_appointment_id
      AND current_setting('app.current_role', true) IN ('PATIENT', 'DOCTOR')
$$;
REVOKE ALL ON FUNCTION appointment_counterpart_photo(UUID) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION appointment_counterpart_photo(UUID) TO hellodoctor_api;

-- The counterpart's photo in one peer request (and so its chat): the peer of
-- the request gets the patient profile's photo; the patient who made it gets
-- the peer's photo. Anyone else, Admins included, gets NULL.
CREATE FUNCTION peer_request_counterpart_photo(p_request_id UUID)
RETURNS JSONB LANGUAGE sql SECURITY DEFINER SET search_path = pg_catalog, public STABLE AS $$
    SELECT CASE
        WHEN current_setting('app.current_role', true) = 'PEER' AND r.peer_user_id = caller.id THEN (
            SELECT jsonb_build_object('bucket', p.profile_picture_bucket, 'key', p.profile_picture_object_key,
                'hash', p.profile_picture_content_hash, 'mime', p.profile_picture_mime_type)
            FROM patient_profiles p
            WHERE p.id = r.patient_id AND p.profile_picture_object_key IS NOT NULL)
        WHEN current_setting('app.current_role', true) = 'PATIENT' AND r.patient_user_id = caller.id THEN (
            SELECT jsonb_build_object('bucket', pp.profile_picture_bucket, 'key', pp.profile_picture_object_key,
                'hash', pp.profile_picture_content_hash, 'mime', pp.profile_picture_mime_type)
            FROM peer_profiles pp
            WHERE pp.id = r.peer_id AND pp.profile_picture_object_key IS NOT NULL)
        END
    FROM peer_requests r
    CROSS JOIN (SELECT NULLIF(current_setting('app.current_user_id', true), '')::uuid AS id) caller
    WHERE r.id = p_request_id
$$;
REVOKE ALL ON FUNCTION peer_request_counterpart_photo(UUID) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION peer_request_counterpart_photo(UUID) TO hellodoctor_api;
