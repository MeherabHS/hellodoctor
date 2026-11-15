-- Patient-history rows and encounter detail tell the doctor when their
-- 20-day file window (20261221000002) has closed, so the client can say
-- "access ended" instead of advertising files it can no longer open.
DO $$
DECLARE f TEXT; def TEXT; changed TEXT;
BEGIN
  FOREACH f IN ARRAY ARRAY['doctor_patient_history(uuid,uuid,bigint,bigint)','doctor_patient_history_detail(uuid,uuid,uuid)'] LOOP
    SELECT pg_get_functiondef(f::regprocedure) INTO def;
    changed := replace(def, 'SELECT jsonb_build_object(''historical_appointment_id'',a.id,',
      'SELECT jsonb_build_object(''files_access_ended'',NOT doctor_document_window_allowed(a.id,p_doctor_user),''historical_appointment_id'',a.id,');
    IF changed = def THEN
      RAISE EXCEPTION '% no longer has the expected select list', f;
    END IF;
    EXECUTE changed;
  END LOOP;
END $$;
