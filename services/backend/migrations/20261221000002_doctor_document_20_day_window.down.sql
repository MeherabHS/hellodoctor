DO $$
DECLARE def TEXT; changed TEXT;
BEGIN
  SELECT pg_get_functiondef('doctor_authorized_document(uuid,uuid,uuid,uuid)'::regprocedure) INTO def;
  changed := replace(def, 'doctor_document_window_allowed(p_document_appointment,p_user)',
                          'doctor_own_encounter_allowed(p_document_appointment,p_user)');
  IF changed = def THEN
    RAISE EXCEPTION 'doctor_authorized_document does not have the 20-day window check';
  END IF;
  EXECUTE changed;
END $$;

DROP POLICY intake_documents_doctor_window ON prescription_intake_documents;
CREATE POLICY intake_documents_doctor_window ON prescription_intake_documents AS RESTRICTIVE FOR SELECT TO hellodoctor_api
    USING (current_setting('app.current_role', true) <> 'DOCTOR'
        OR doctor_own_encounter_allowed(appointment_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid));
DROP POLICY prescriptions_doctor_window ON prescriptions;
CREATE POLICY prescriptions_doctor_window ON prescriptions AS RESTRICTIVE FOR SELECT TO hellodoctor_api
    USING (current_setting('app.current_role', true) <> 'DOCTOR'
        OR doctor_own_encounter_allowed(appointment_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid));

DROP FUNCTION doctor_document_window_allowed(UUID, UUID);
