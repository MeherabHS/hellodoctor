-- A doctor may open the prescriptions and patient uploads of their own
-- consultations only while the consultation is live and for 20 days after it
-- ended (product-owner instruction, 2026-11-14). Before this, access to their
-- own past consultations' documents never ended (20261106000004). Patients
-- and Admins are unaffected; the 24-hour late-Rx upload rule is unaffected.
--
-- The clock starts at the session end, else the recorded completion, else
-- the slot end (for a consultation that ended without either timestamp).
CREATE FUNCTION doctor_document_window_allowed(p_appointment UUID, p_user UUID) RETURNS BOOLEAN
LANGUAGE sql SECURITY DEFINER SET search_path=public STABLE AS $$
 SELECT doctor_own_encounter_allowed(p_appointment, p_user) AND EXISTS (
   SELECT 1 FROM appointments a
   JOIN doctor_schedule_slots s ON s.id = a.slot_id
   LEFT JOIN consultation_sessions cs ON cs.appointment_id = a.id
   WHERE a.id = p_appointment
     AND ( (a.status IN ('CONFIRMED','WAITING','IN_CONSULTATION') AND cs.ended_at IS NULL)
        OR CURRENT_TIMESTAMP < COALESCE(cs.ended_at, a.completed_at, s.end_time) + INTERVAL '20 days' ))
$$;
REVOKE ALL ON FUNCTION doctor_document_window_allowed(UUID, UUID) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION doctor_document_window_allowed(UUID, UUID) TO hellodoctor_api;

-- Row visibility for doctors (lists, summaries, counts).
DROP POLICY prescriptions_doctor_window ON prescriptions;
CREATE POLICY prescriptions_doctor_window ON prescriptions AS RESTRICTIVE FOR SELECT TO hellodoctor_api
    USING (current_setting('app.current_role', true) <> 'DOCTOR'
        OR doctor_document_window_allowed(appointment_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid));
DROP POLICY intake_documents_doctor_window ON prescription_intake_documents;
CREATE POLICY intake_documents_doctor_window ON prescription_intake_documents AS RESTRICTIVE FOR SELECT TO hellodoctor_api
    USING (current_setting('app.current_role', true) <> 'DOCTOR'
        OR doctor_document_window_allowed(appointment_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid));

-- File access for doctors (the download-link lookup).
DO $$
DECLARE def TEXT; changed TEXT;
BEGIN
  SELECT pg_get_functiondef('doctor_authorized_document(uuid,uuid,uuid,uuid)'::regprocedure) INTO def;
  changed := replace(def, 'doctor_own_encounter_allowed(p_document_appointment,p_user)',
                          'doctor_document_window_allowed(p_document_appointment,p_user)');
  IF changed = def THEN
    RAISE EXCEPTION 'doctor_authorized_document no longer has the expected encounter check';
  END IF;
  EXECUTE changed;
END $$;
