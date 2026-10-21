CREATE FUNCTION doctor_own_encounter_allowed(p_appointment UUID,p_user UUID) RETURNS BOOLEAN
LANGUAGE sql SECURITY DEFINER SET search_path=public STABLE AS $$
 SELECT EXISTS(SELECT 1 FROM appointments a JOIN doctor_profiles d ON d.id=a.doctor_id JOIN users u ON u.id=d.user_id
 WHERE a.id=p_appointment AND d.user_id=p_user AND p_user=NULLIF(current_setting('app.current_user_id',true),'')::uuid
 AND current_setting('app.current_role',true)='DOCTOR' AND d.professional_verification_status='VERIFIED'
 AND u.is_active AND u.deleted_at IS NULL AND d.deleted_at IS NULL
 AND a.status<>'PENDING_PAYMENT'
 AND EXISTS(SELECT 1 FROM consultation_sessions cs WHERE cs.appointment_id=a.id))
$$;
REVOKE ALL ON FUNCTION doctor_own_encounter_allowed(UUID,UUID) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION doctor_own_encounter_allowed(UUID,UUID) TO hellodoctor_api;

-- Retained encounter identity must not reopen the complete mutable Patient
-- profile after the broader treatment window closes. Return only display name.
CREATE FUNCTION doctor_encounter_patient_name(p_appointment UUID) RETURNS TEXT
LANGUAGE sql SECURITY DEFINER SET search_path=public STABLE AS $$
 SELECT p.full_name FROM appointments a JOIN patient_profiles p ON p.id=a.patient_id
 JOIN doctor_profiles d ON d.id=a.doctor_id JOIN users u ON u.id=d.user_id
 WHERE a.id=p_appointment AND d.user_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid
 AND current_setting('app.current_role',true)='DOCTOR' AND d.professional_verification_status='VERIFIED'
 AND d.deleted_at IS NULL AND u.is_active AND u.deleted_at IS NULL
 AND (a.status IN ('CONFIRMED','WAITING','IN_CONSULTATION')
      OR doctor_own_encounter_allowed(a.id,d.user_id))
$$;
REVOKE ALL ON FUNCTION doctor_encounter_patient_name(UUID) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION doctor_encounter_patient_name(UUID) TO hellodoctor_api;

-- Save only the definitions this migration changes, for reversible rollback.
CREATE TABLE encounter_policy_rollback(kind TEXT,identity TEXT,definition TEXT);
DO $$ DECLARE f RECORD; p RECORD; def TEXT; changed TEXT; BEGIN
 FOR f IN SELECT proc.oid FROM pg_proc proc JOIN pg_namespace n ON n.oid=proc.pronamespace WHERE n.nspname='public'
 AND proc.proname IN ('doctor_authorized_document','doctor_patient_history','doctor_patient_history_detail') LOOP
  def:=pg_get_functiondef(f.oid); INSERT INTO encounter_policy_rollback VALUES('FUNCTION',f.oid::regprocedure::text,def);
  changed:=def;
  IF f.oid::regproc::text='doctor_authorized_document' THEN
   changed:=replace(changed,'doctor_history_context_allowed(p_current,p_user)','doctor_own_encounter_allowed(p_document_appointment,p_user)');
   changed:=replace(changed,'historical_a.patient_id=current_a.patient_id','historical_a.patient_id=current_a.patient_id AND historical_a.doctor_id=current_a.doctor_id');
  ELSE
   changed:=replace(changed,'doctor_history_context_allowed(p_current_appointment,p_doctor_user)','doctor_own_encounter_allowed(p_current_appointment,p_doctor_user)');
   changed:=replace(changed,'a.patient_id=current_a.patient_id','a.patient_id=current_a.patient_id AND a.doctor_id=current_a.doctor_id');
  END IF;
  EXECUTE changed;
 END LOOP;
 FOR p IN SELECT * FROM pg_policies WHERE schemaname='public' AND policyname IN ('prescriptions_doctor_window','intake_documents_doctor_window','appointment_clinical_intake_access') LOOP
  def:=format('CREATE POLICY %I ON public.%I AS %s FOR %s TO %s USING (%s)',p.policyname,p.tablename,p.permissive,p.cmd,
    (SELECT string_agg(quote_ident(x),',') FROM unnest(p.roles)x),p.qual);
  INSERT INTO encounter_policy_rollback VALUES('POLICY',format('%I ON public.%I',p.policyname,p.tablename),def);
  EXECUTE format('DROP POLICY %I ON public.%I',p.policyname,p.tablename);
  EXECUTE replace(def,'doctor_history_context_allowed','doctor_own_encounter_allowed');
 END LOOP;
END $$;
CREATE TABLE appointment_reschedules (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(), appointment_id UUID NOT NULL REFERENCES appointments(id),
 original_slot_id UUID NOT NULL REFERENCES doctor_schedule_slots(id), new_slot_id UUID NOT NULL REFERENCES doctor_schedule_slots(id),
 original_start TIMESTAMPTZ NOT NULL,new_start TIMESTAMPTZ NOT NULL,
 actor_user_id UUID NOT NULL REFERENCES users(id), auth_session_id UUID REFERENCES auth_sessions(id),
 request_id TEXT, reason TEXT NOT NULL CHECK(length(reason) BETWEEN 8 AND 300),
 grievance_id UUID REFERENCES grievance_reports(id),created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
ALTER TABLE appointment_reschedules ENABLE ROW LEVEL SECURITY;
ALTER TABLE appointment_reschedules FORCE ROW LEVEL SECURITY;
CREATE POLICY reschedule_read ON appointment_reschedules FOR SELECT TO hellodoctor_api USING(consultation_participant_allowed(appointment_id));
CREATE POLICY reschedule_worker ON appointment_reschedules TO hellodoctor_payment_worker USING(current_setting('app.current_role',true)='ADMIN') WITH CHECK(current_setting('app.current_role',true)='ADMIN' AND actor_user_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid);
GRANT SELECT ON appointment_reschedules TO hellodoctor_api;
GRANT SELECT,INSERT ON appointment_reschedules TO hellodoctor_payment_worker;
GRANT INSERT ON audit_events TO hellodoctor_payment_worker;
