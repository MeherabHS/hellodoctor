DO $$
DECLARE f TEXT; def TEXT; changed TEXT;
BEGIN
  FOREACH f IN ARRAY ARRAY['doctor_patient_history(uuid,uuid,bigint,bigint)','doctor_patient_history_detail(uuid,uuid,uuid)'] LOOP
    SELECT pg_get_functiondef(f::regprocedure) INTO def;
    changed := replace(def, '''files_access_ended'',NOT doctor_document_window_allowed(a.id,p_doctor_user),', '');
    IF changed = def THEN
      RAISE EXCEPTION '% does not carry files_access_ended', f;
    END IF;
    EXECUTE changed;
  END LOOP;
END $$;
