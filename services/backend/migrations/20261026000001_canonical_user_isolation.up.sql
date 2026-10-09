-- Canonical identity remains users.id. No new capability roles or ownership copies.
-- Refuse inconsistent existing profile mappings; never guess or reassign real users.
DO $$ BEGIN
 IF EXISTS(SELECT 1 FROM patient_profiles p JOIN users u ON u.id=p.user_id WHERE u.role<>'PATIENT')
 OR EXISTS(SELECT 1 FROM doctor_profiles d JOIN users u ON u.id=d.user_id WHERE u.role<>'DOCTOR') THEN
  RAISE EXCEPTION 'Repair role/profile inconsistencies explicitly before migration';
 END IF;
END $$;

-- This legacy relationship helper participates in Patient-profile RLS. A
-- caller-supplied Doctor UUID must never reveal another Doctor's relationship.
-- Upcoming assigned appointments need operational demographics; historical
-- access ends with the existing approved clinical follow-up window.
CREATE OR REPLACE FUNCTION fn_doctor_treats_patient(p_patient_profile_id uuid,p_requesting_user_id uuid)
RETURNS boolean LANGUAGE sql SECURITY DEFINER SET search_path=pg_catalog,public STABLE AS $$
 SELECT EXISTS(
  SELECT 1 FROM public.appointments a JOIN public.doctor_profiles d ON d.id=a.doctor_id
  JOIN public.users u ON u.id=d.user_id
  LEFT JOIN public.consultation_sessions cs ON cs.appointment_id=a.id
  WHERE a.patient_id=p_patient_profile_id AND d.user_id=p_requesting_user_id
  AND p_requesting_user_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid
  AND current_setting('app.current_role',true)='DOCTOR'
  AND d.professional_verification_status='VERIFIED' AND d.deleted_at IS NULL
  AND u.is_active AND u.deleted_at IS NULL
  AND ((a.status IN('CONFIRMED','WAITING','IN_CONSULTATION') AND cs.ended_at IS NULL)
       OR public.doctor_history_context_allowed(a.id,p_requesting_user_id))
 );
$$;
REVOKE ALL ON FUNCTION fn_doctor_treats_patient(uuid,uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION fn_doctor_treats_patient(uuid,uuid) TO hellodoctor_api,hellodoctor_auth_worker,hellodoctor_payment_worker,hellodoctor_finance_worker,hellodoctor_system_worker;

CREATE FUNCTION guard_profile_identity() RETURNS trigger LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,public AS $$
DECLARE required_role text;
BEGIN
 IF TG_TABLE_NAME='users' THEN
  IF NEW.role IS DISTINCT FROM OLD.role AND
     (EXISTS(SELECT 1 FROM public.patient_profiles WHERE user_id=NEW.id) OR
      EXISTS(SELECT 1 FROM public.doctor_profiles WHERE user_id=NEW.id)) THEN
   RAISE EXCEPTION 'Profile-bearing account role is immutable' USING ERRCODE='23514';
  END IF;
 ELSE
  IF TG_OP='UPDATE' AND (NEW.user_id IS DISTINCT FROM OLD.user_id OR NEW.id IS DISTINCT FROM OLD.id) THEN
   RAISE EXCEPTION 'Canonical profile identity is immutable' USING ERRCODE='23514';
  END IF;
  required_role := CASE TG_TABLE_NAME WHEN 'patient_profiles' THEN 'PATIENT' ELSE 'DOCTOR' END;
  PERFORM 1 FROM public.users WHERE id=NEW.user_id AND role::text=required_role FOR SHARE;
  IF NOT FOUND THEN RAISE EXCEPTION 'Profile must match canonical account role' USING ERRCODE='23514'; END IF;
 END IF;
 RETURN NEW;
END $$;
REVOKE ALL ON FUNCTION guard_profile_identity() FROM PUBLIC;
CREATE TRIGGER canonical_patient_identity BEFORE INSERT OR UPDATE ON patient_profiles FOR EACH ROW EXECUTE FUNCTION guard_profile_identity();
CREATE TRIGGER canonical_doctor_identity BEFORE INSERT OR UPDATE ON doctor_profiles FOR EACH ROW EXECUTE FUNCTION guard_profile_identity();
CREATE TRIGGER canonical_account_role BEFORE UPDATE OF role ON users FOR EACH ROW EXECUTE FUNCTION guard_profile_identity();
-- Recheck at commit as well: profile creation and a concurrent role mutation
-- cannot commit an inconsistent mapping after waiting on the user row lock.
CREATE CONSTRAINT TRIGGER canonical_patient_commit AFTER INSERT OR UPDATE ON patient_profiles DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION guard_profile_identity();
CREATE CONSTRAINT TRIGGER canonical_doctor_commit AFTER INSERT OR UPDATE ON doctor_profiles DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION guard_profile_identity();

-- Composite FKs make redundant domain IDs agree with their authoritative parent.
ALTER TABLE doctor_schedule_slots ADD CONSTRAINT slots_identity_doctor UNIQUE(id,doctor_id);
ALTER TABLE appointments ADD CONSTRAINT appointment_slot_doctor FOREIGN KEY(slot_id,doctor_id) REFERENCES doctor_schedule_slots(id,doctor_id) ON DELETE RESTRICT DEFERRABLE INITIALLY IMMEDIATE;
ALTER TABLE appointments ADD CONSTRAINT appointment_identity_patient UNIQUE(id,patient_id);
ALTER TABLE appointments ADD CONSTRAINT appointment_identity_participants UNIQUE(id,patient_id,doctor_id);
ALTER TABLE appointment_clinical_intake ADD CONSTRAINT intake_appointment_patient FOREIGN KEY(appointment_id,patient_id) REFERENCES appointments(id,patient_id) ON DELETE RESTRICT DEFERRABLE INITIALLY IMMEDIATE;
ALTER TABLE prescription_intake_documents ADD CONSTRAINT document_appointment_patient FOREIGN KEY(appointment_id,patient_id) REFERENCES appointments(id,patient_id) ON DELETE RESTRICT DEFERRABLE INITIALLY IMMEDIATE;
ALTER TABLE prescriptions ADD CONSTRAINT prescription_appointment_participants FOREIGN KEY(appointment_id,patient_id,doctor_id) REFERENCES appointments(id,patient_id,doctor_id) ON DELETE RESTRICT DEFERRABLE INITIALLY IMMEDIATE;
ALTER TABLE chat_conversations ADD CONSTRAINT chat_appointment_participants FOREIGN KEY(appointment_id,patient_id,doctor_id) REFERENCES appointments(id,patient_id,doctor_id) ON DELETE RESTRICT DEFERRABLE INITIALLY IMMEDIATE;

CREATE FUNCTION guard_grievance_subject() RETURNS trigger LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,public AS $$
BEGIN
 IF TG_TABLE_NAME='grievance_reports' THEN
  PERFORM 1 FROM public.consultation_sessions cs JOIN public.appointments a ON a.id=cs.appointment_id WHERE cs.id=NEW.consultation_id AND a.patient_id=NEW.patient_id FOR SHARE OF cs,a;
  IF NOT FOUND THEN
   RAISE EXCEPTION 'Grievance patient must match consultation' USING ERRCODE='23514';
  END IF;
 ELSIF TG_TABLE_NAME='appointments' THEN
 IF EXISTS(
  SELECT 1 FROM public.grievance_reports g JOIN public.consultation_sessions cs ON cs.id=g.consultation_id WHERE cs.appointment_id=NEW.id AND g.patient_id<>NEW.patient_id) THEN
  RAISE EXCEPTION 'Appointment subject conflicts with retained grievance' USING ERRCODE='23514';
 END IF;
 ELSIF TG_TABLE_NAME='consultation_sessions' THEN
 IF EXISTS(
  SELECT 1 FROM public.grievance_reports g WHERE g.consultation_id=NEW.id AND NOT EXISTS(SELECT 1 FROM public.appointments a WHERE a.id=NEW.appointment_id AND a.patient_id=g.patient_id)) THEN
  RAISE EXCEPTION 'Grievance patient must match consultation' USING ERRCODE='23514';
 END IF;
 END IF;
 RETURN NEW;
END $$;
REVOKE ALL ON FUNCTION guard_grievance_subject() FROM PUBLIC;
CREATE TRIGGER grievance_canonical_subject BEFORE INSERT OR UPDATE ON grievance_reports FOR EACH ROW EXECUTE FUNCTION guard_grievance_subject();
CREATE TRIGGER appointment_grievance_subject BEFORE UPDATE OF patient_id ON appointments FOR EACH ROW EXECUTE FUNCTION guard_grievance_subject();
CREATE TRIGGER consultation_grievance_subject BEFORE UPDATE OF appointment_id ON consultation_sessions FOR EACH ROW EXECUTE FUNCTION guard_grievance_subject();
DO $$ BEGIN
 IF EXISTS(SELECT 1 FROM grievance_reports g JOIN consultation_sessions cs ON cs.id=g.consultation_id JOIN appointments a ON a.id=cs.appointment_id WHERE g.patient_id<>a.patient_id) THEN
  RAISE EXCEPTION 'Repair inconsistent grievance subjects explicitly before migration';
 END IF;
END $$;

-- Public professional data has a separate projection. Private columns/identity
-- mappings are never returned through the directory view, even for a raw API query.
CREATE VIEW doctor_public_directory WITH (security_barrier=true) AS
 SELECT d.id,d.full_name,d.license_number,d.license_authority,d.license_country,
 d.primary_specialty,d.experience_years,d.current_hospital,d.qualifications,
 d.consultation_fee_video,d.consultation_fee_chat,d.is_on_duty,d.is_verified,
 d.professional_verification_status,d.created_at,d.updated_at,u.is_active AS account_active
 FROM doctor_profiles d JOIN users u ON u.id=d.user_id
 WHERE d.deleted_at IS NULL AND u.deleted_at IS NULL AND (u.is_active OR current_setting('app.current_role',true) IN ('PLATFORM_ADMIN','CLINICAL_ADMIN','FINANCE_ADMIN','COMPLIANCE','SECURITY_ADMIN','SUPPORT'))
 ;
GRANT SELECT ON doctor_public_directory TO hellodoctor_api;
ALTER TABLE doctor_profiles ENABLE ROW LEVEL SECURITY;
ALTER TABLE doctor_profiles FORCE ROW LEVEL SECURITY;
CREATE POLICY doctor_private_read ON doctor_profiles FOR SELECT TO hellodoctor_api USING (
 user_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid OR
 current_setting('app.current_role',true) IN ('PLATFORM_ADMIN','CLINICAL_ADMIN','FINANCE_ADMIN','COMPLIANCE'));
CREATE POLICY doctor_private_insert ON doctor_profiles FOR INSERT TO hellodoctor_api WITH CHECK (
 current_setting('app.current_role',true) IN ('PLATFORM_ADMIN','CLINICAL_ADMIN'));
CREATE POLICY doctor_private_update ON doctor_profiles FOR UPDATE TO hellodoctor_api USING (
 user_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid OR
 current_setting('app.current_role',true) IN ('PLATFORM_ADMIN','CLINICAL_ADMIN')) WITH CHECK (
 user_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid OR
 current_setting('app.current_role',true) IN ('PLATFORM_ADMIN','CLINICAL_ADMIN'));
CREATE POLICY doctor_auth_read ON doctor_profiles FOR SELECT TO hellodoctor_auth_worker USING(true);
CREATE POLICY doctor_auth_insert ON doctor_profiles FOR INSERT TO hellodoctor_auth_worker WITH CHECK (
 user_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid OR
 current_setting('app.current_role',true) IN ('PLATFORM_ADMIN','CLINICAL_ADMIN'));
CREATE POLICY doctor_worker_read ON doctor_profiles FOR SELECT TO hellodoctor_system_worker,hellodoctor_finance_worker USING(true);

ALTER TABLE users ENABLE ROW LEVEL SECURITY;
ALTER TABLE users FORCE ROW LEVEL SECURITY;
CREATE POLICY users_api_read ON users FOR SELECT TO hellodoctor_api USING (
 id=NULLIF(current_setting('app.current_user_id',true),'')::uuid OR
 current_setting('app.current_role',true) IN ('PLATFORM_ADMIN','CLINICAL_ADMIN','FINANCE_ADMIN','COMPLIANCE','SECURITY_ADMIN','SUPPORT'));
CREATE POLICY users_auth_bootstrap ON users TO hellodoctor_auth_worker USING(true) WITH CHECK(true);
CREATE POLICY users_scheduler_read ON users FOR SELECT TO hellodoctor_system_worker USING(true);
-- Ordinary request-serving SQL needs account state, not password hashes.
REVOKE SELECT ON users FROM hellodoctor_api;
GRANT SELECT(id,phone_number,email,role,preferred_language,is_active,is_verified,created_at,updated_at,deleted_at) ON users TO hellodoctor_api;

ALTER TABLE idempotency_records ENABLE ROW LEVEL SECURITY;
ALTER TABLE idempotency_records FORCE ROW LEVEL SECURITY;
CREATE POLICY idempotency_subject ON idempotency_records TO hellodoctor_api USING (
 user_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid) WITH CHECK (
 user_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid);
ALTER TABLE system_error_logs ENABLE ROW LEVEL SECURITY;
ALTER TABLE system_error_logs FORCE ROW LEVEL SECURITY;
CREATE POLICY system_errors_operations ON system_error_logs TO hellodoctor_api USING (
 current_setting('app.current_role',true) IN ('SECURITY_ADMIN','PLATFORM_ADMIN')) WITH CHECK (
 current_setting('app.current_role',true) IN ('SECURITY_ADMIN','PLATFORM_ADMIN'));
CREATE POLICY system_errors_worker ON system_error_logs TO hellodoctor_system_worker USING(true) WITH CHECK(true);

-- Logout used to require unrestricted API SELECT/UPDATE on push-token rows.
-- Resolve the subject from the refresh token inside the existing Auth Worker
-- transaction instead. Ordinary API requests retain only owner policies.
DROP POLICY notification_device_tokens_backend_select ON notification_device_tokens;
DROP POLICY notification_device_tokens_backend_disable ON notification_device_tokens;
GRANT SELECT,UPDATE ON notification_device_tokens TO hellodoctor_auth_worker;
CREATE POLICY device_tokens_auth_logout_read ON notification_device_tokens FOR SELECT TO hellodoctor_auth_worker USING(user_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid);
CREATE POLICY device_tokens_auth_logout_update ON notification_device_tokens FOR UPDATE TO hellodoctor_auth_worker USING(user_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid) WITH CHECK(user_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid);

-- Restrictive policies cannot be bypassed by another permissive FOR ALL policy.
CREATE POLICY chat_sender_guard ON chat_messages AS RESTRICTIVE FOR INSERT TO hellodoctor_api WITH CHECK (
 sender_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid AND EXISTS (
 SELECT 1 FROM chat_conversations c WHERE c.id=conversation_id AND NOT c.is_locked AND c.expires_at>now()
 AND (c.patient_id IN(SELECT id FROM patient_profiles WHERE user_id=sender_id)
 OR c.doctor_id IN(SELECT id FROM doctor_profiles WHERE user_id=sender_id))));
-- Read receipts are the only existing message mutation. The API capability
-- cannot rewrite a persisted sender, conversation, body or attachment history.
REVOKE UPDATE ON chat_messages FROM hellodoctor_api;
GRANT UPDATE(is_read) ON chat_messages TO hellodoctor_api;
CREATE POLICY rx_author_guard ON prescriptions AS RESTRICTIVE FOR INSERT TO hellodoctor_api WITH CHECK (
 current_setting('app.current_role',true)='DOCTOR' AND doctor_id IN(
 SELECT id FROM doctor_profiles WHERE user_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid
 AND professional_verification_status='VERIFIED') AND doctor_history_context_allowed(appointment_id,NULLIF(current_setting('app.current_user_id',true),'')::uuid));
CREATE POLICY intake_document_author_guard ON prescription_intake_documents AS RESTRICTIVE FOR INSERT TO hellodoctor_api WITH CHECK (
 current_setting('app.current_role',true)='PATIENT' AND patient_id IN(
 SELECT id FROM patient_profiles WHERE user_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid));
CREATE POLICY patient_profile_self_update ON patient_profiles AS RESTRICTIVE FOR UPDATE TO hellodoctor_api USING (
 user_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid OR current_setting('app.current_role',true) IN ('PLATFORM_ADMIN','CLINICAL_ADMIN')) WITH CHECK (
 user_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid OR current_setting('app.current_role',true) IN ('PLATFORM_ADMIN','CLINICAL_ADMIN'));
CREATE POLICY booking_patient_author ON appointments AS RESTRICTIVE FOR INSERT TO hellodoctor_api WITH CHECK (
 current_setting('app.current_role',true)='PATIENT' AND patient_id IN(SELECT id FROM patient_profiles WHERE user_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid));
CREATE POLICY grievance_patient_author ON grievance_reports AS RESTRICTIVE FOR INSERT TO hellodoctor_api WITH CHECK (
 current_setting('app.current_role',true)='PATIENT' AND patient_id IN(SELECT id FROM patient_profiles WHERE user_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid));
CREATE POLICY wallet_owner_insert ON doctor_wallets AS RESTRICTIVE FOR INSERT TO hellodoctor_api WITH CHECK (
 current_setting('app.current_role',true) IN ('PLATFORM_ADMIN','CLINICAL_ADMIN','FINANCE_ADMIN') OR doctor_id IN(SELECT id FROM doctor_profiles WHERE user_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid));

-- Public article visibility uses the safe projection rather than private-profile RLS.
DROP POLICY doctor_articles_published_read ON doctor_articles;
CREATE POLICY doctor_articles_published_read ON doctor_articles FOR SELECT USING (
 status='PUBLISHED' AND current_setting('app.current_role',true) IN ('PATIENT','DOCTOR','PLATFORM_ADMIN','CLINICAL_ADMIN','FINANCE_ADMIN','SUPPORT','COMPLIANCE','SECURITY_ADMIN') AND EXISTS(SELECT 1 FROM doctor_public_directory d WHERE d.id=author_id AND d.professional_verification_status='VERIFIED' AND d.account_active));

-- Preserve account-originated anonymous Q&A linkage internally, not in Doctor DTOs.
ALTER TABLE health_questions DROP CONSTRAINT health_questions_author_patient_id_fkey;
ALTER TABLE health_questions ADD CONSTRAINT health_questions_author_patient_id_fkey FOREIGN KEY(author_patient_id) REFERENCES patient_profiles(id) ON DELETE RESTRICT;
REVOKE SELECT ON health_questions FROM hellodoctor_api;
GRANT SELECT(id,question_text,category,status,created_at,updated_at) ON health_questions TO hellodoctor_api;

-- Existing audit rows are untouched. Request metadata is filled at INSERT only.
CREATE FUNCTION capture_audit_request_context() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF current_user='hellodoctor_api' AND
  (NEW.actor_id IS DISTINCT FROM NULLIF(current_setting('app.current_user_id',true),'')::uuid OR NEW.actor_role::text IS DISTINCT FROM current_setting('app.current_role',true)) THEN
   RAISE EXCEPTION 'Audit actor must match authenticated context' USING ERRCODE='42501';
 END IF;
 IF current_user='hellodoctor_api' AND
  ((NEW.session_id IS NOT NULL AND NEW.session_id IS DISTINCT FROM NULLIF(current_setting('app.current_session_id',true),'')::uuid)
   OR (NEW.request_id IS NOT NULL AND NEW.request_id IS DISTINCT FROM NULLIF(current_setting('app.current_request_id',true),''))) THEN
   RAISE EXCEPTION 'Audit request metadata must match authenticated context' USING ERRCODE='42501';
 END IF;
 NEW.session_id := COALESCE(NEW.session_id,NULLIF(current_setting('app.current_session_id',true),'')::uuid);
 NEW.request_id := COALESCE(NEW.request_id,NULLIF(current_setting('app.current_request_id',true),''));
 RETURN NEW;
END $$;
CREATE TRIGGER audit_request_context BEFORE INSERT ON audit_events FOR EACH ROW EXECUTE FUNCTION capture_audit_request_context();

-- One audited investigation primitive, stable UUID keys, IDs only (no PHI,
-- credentials, tokens, arbitrary SQL, bulk enumeration or outside-authority role).
CREATE FUNCTION compliance_trace(p_kind text,p_id uuid,p_reason text) RETURNS jsonb
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,public AS $$
DECLARE actor uuid; actor_role text; target uuid; answer jsonb; event_id uuid:=gen_random_uuid();
BEGIN
 actor:=NULLIF(current_setting('app.current_user_id',true),'')::uuid;
 actor_role:=current_setting('app.current_role',true);
 IF actor_role NOT IN ('COMPLIANCE','PLATFORM_ADMIN') OR actor IS NULL OR NOT EXISTS(
 SELECT 1 FROM public.users WHERE id=actor AND role::text=actor_role AND is_active AND deleted_at IS NULL) THEN
  RAISE EXCEPTION 'Compliance trace not authorized' USING ERRCODE='42501';
 END IF;
 IF p_reason IS NULL OR p_kind IS NULL OR p_id IS NULL OR length(trim(p_reason)) NOT BETWEEN 8 AND 300 OR p_kind NOT IN ('USER','PATIENT','DOCTOR','APPOINTMENT','CONSULTATION') THEN
  RAISE EXCEPTION 'Stable target and investigation reason required' USING ERRCODE='22023';
 END IF;
 SELECT CASE p_kind WHEN 'USER' THEN (SELECT id FROM public.users WHERE id=p_id)
 WHEN 'PATIENT' THEN (SELECT user_id FROM public.patient_profiles WHERE id=p_id)
 WHEN 'DOCTOR' THEN (SELECT user_id FROM public.doctor_profiles WHERE id=p_id)
 WHEN 'APPOINTMENT' THEN (SELECT p.user_id FROM public.appointments a JOIN public.patient_profiles p ON p.id=a.patient_id WHERE a.id=p_id)
 WHEN 'CONSULTATION' THEN (SELECT p.user_id FROM public.consultation_sessions cs JOIN public.appointments a ON a.id=cs.appointment_id JOIN public.patient_profiles p ON p.id=a.patient_id WHERE cs.id=p_id) END INTO target;
 WITH profiles AS (SELECT id,'PATIENT' AS kind FROM public.patient_profiles WHERE user_id=target UNION ALL SELECT id,'DOCTOR' FROM public.doctor_profiles WHERE user_id=target),
 a AS (SELECT ap.* FROM public.appointments ap WHERE (p_kind IN('APPOINTMENT','CONSULTATION') AND ap.id=CASE p_kind WHEN 'APPOINTMENT' THEN p_id ELSE (SELECT appointment_id FROM public.consultation_sessions WHERE id=p_id) END)
 OR (p_kind NOT IN('APPOINTMENT','CONSULTATION') AND (ap.patient_id IN(SELECT id FROM profiles WHERE kind='PATIENT') OR ap.doctor_id IN(SELECT id FROM profiles WHERE kind='DOCTOR')))),
 resources AS (
 SELECT 'APPOINTMENT'::text kind,id FROM a UNION ALL SELECT 'CONSULTATION',id FROM public.consultation_sessions WHERE appointment_id IN(SELECT id FROM a)
 UNION SELECT kind||'_PROFILE',id FROM profiles WHERE p_kind NOT IN('APPOINTMENT','CONSULTATION')
 UNION SELECT 'USER',target WHERE target IS NOT NULL AND p_kind NOT IN('APPOINTMENT','CONSULTATION')
 UNION ALL SELECT 'PRESCRIPTION',id FROM public.prescriptions WHERE appointment_id IN(SELECT id FROM a)
 UNION ALL SELECT 'DOCUMENT',id FROM public.prescription_intake_documents WHERE appointment_id IN(SELECT id FROM a)
 UNION ALL SELECT 'CHAT',id FROM public.chat_conversations WHERE appointment_id IN(SELECT id FROM a)
 UNION ALL SELECT 'TRANSACTION',id FROM public.transactions WHERE appointment_id IN(SELECT id FROM a)
 UNION ALL SELECT 'GRIEVANCE',id FROM public.grievance_reports WHERE consultation_id IN(SELECT id FROM public.consultation_sessions WHERE appointment_id IN(SELECT id FROM a))
 UNION ALL SELECT 'DISBURSEMENT',disbursement_item_id FROM public.transactions WHERE appointment_id IN(SELECT id FROM a) AND disbursement_item_id IS NOT NULL
 UNION SELECT 'DISBURSEMENT',id FROM public.disbursement_items WHERE doctor_id IN(SELECT id FROM profiles WHERE kind='DOCTOR') AND p_kind NOT IN('APPOINTMENT','CONSULTATION')
 UNION SELECT 'WALLET',doctor_id FROM public.doctor_wallets WHERE doctor_id IN(SELECT id FROM profiles WHERE kind='DOCTOR') AND p_kind NOT IN('APPOINTMENT','CONSULTATION')
 UNION SELECT 'PAYOUT_METHOD',doctor_id FROM public.doctor_payout_methods WHERE doctor_id IN(SELECT id FROM profiles WHERE kind='DOCTOR') AND p_kind NOT IN('APPOINTMENT','CONSULTATION')
 UNION SELECT 'PAYOUT_DESTINATION',id FROM public.doctor_payout_destinations WHERE doctor_id IN(SELECT id FROM profiles WHERE kind='DOCTOR') AND p_kind NOT IN('APPOINTMENT','CONSULTATION')
 UNION SELECT 'STATEMENT',id FROM public.monthly_payout_statements WHERE doctor_id IN(SELECT id FROM profiles WHERE kind='DOCTOR') AND p_kind NOT IN('APPOINTMENT','CONSULTATION')
 UNION SELECT 'NOTIFICATION',id FROM public.notifications WHERE user_id=target AND p_kind NOT IN('APPOINTMENT','CONSULTATION')
 UNION SELECT 'DEVICE',device_id FROM public.notification_device_tokens WHERE user_id=target AND p_kind NOT IN('APPOINTMENT','CONSULTATION')
 UNION SELECT 'HEALTH_QUESTION',id FROM public.health_questions WHERE author_patient_id IN(SELECT id FROM profiles WHERE kind='PATIENT') AND p_kind NOT IN('APPOINTMENT','CONSULTATION')
 UNION SELECT 'HEALTH_ANSWER',id FROM public.health_answers WHERE doctor_id IN(SELECT id FROM profiles WHERE kind='DOCTOR') AND p_kind NOT IN('APPOINTMENT','CONSULTATION')
 UNION SELECT 'FINANCIAL_ADJUSTMENT',id FROM public.financial_adjustments WHERE doctor_id IN(SELECT id FROM profiles WHERE kind='DOCTOR') AND p_kind NOT IN('APPOINTMENT','CONSULTATION')
 UNION SELECT 'AUTH_SESSION',id FROM public.auth_sessions WHERE user_id=target AND p_kind NOT IN('APPOINTMENT','CONSULTATION')
 UNION SELECT 'SYSTEM_ERROR',id FROM public.system_error_logs WHERE impacted_user_id=target AND p_kind NOT IN('APPOINTMENT','CONSULTATION')
 UNION ALL SELECT 'ARTICLE',id FROM public.doctor_articles WHERE author_id IN(SELECT id FROM profiles WHERE kind='DOCTOR') AND p_kind NOT IN('APPOINTMENT','CONSULTATION'))
 SELECT jsonb_build_object('target_user_id',target,'profiles',COALESCE((SELECT jsonb_agg(to_jsonb(p)) FROM profiles p),'[]'),
 'participants',COALESCE((SELECT jsonb_agg(jsonb_build_object('appointment_id',a.id,'patient_profile_id',a.patient_id,'patient_user_id',p.user_id,'doctor_profile_id',a.doctor_id,'doctor_user_id',d.user_id)) FROM a JOIN public.patient_profiles p ON p.id=a.patient_id JOIN public.doctor_profiles d ON d.id=a.doctor_id),'[]'),
 'resources',COALESCE((SELECT jsonb_agg(to_jsonb(r)) FROM resources r),'[]'),
 'audit_event_ids',COALESCE((SELECT jsonb_agg(e.id) FROM public.audit_events e WHERE (p_kind NOT IN('APPOINTMENT','CONSULTATION') AND e.actor_id=target) OR e.resource_id IN(SELECT id FROM resources)),'[]'),
 'result',CASE WHEN target IS NULL THEN 'NOT_FOUND' ELSE 'FOUND' END,'access_audit_id',event_id) INTO answer;
 INSERT INTO public.audit_events(id,actor_id,actor_role,action,resource_type,resource_id,result,reason,created_at)
 VALUES(event_id,actor,actor_role::public.user_role_enum,'COMPLIANCE_TRACE','TRACE_'||p_kind,p_id,CASE WHEN target IS NULL THEN 'NOT_FOUND' ELSE 'SUCCESS' END,trim(p_reason),now());
 RETURN answer;
END $$;
REVOKE ALL ON FUNCTION compliance_trace(text,uuid,text) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION compliance_trace(text,uuid,text) TO hellodoctor_api;
