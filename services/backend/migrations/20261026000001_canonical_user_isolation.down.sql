DROP FUNCTION compliance_trace(text,uuid,text);
DROP TRIGGER audit_request_context ON audit_events;
DROP FUNCTION capture_audit_request_context();
GRANT SELECT ON health_questions TO hellodoctor_api;
ALTER TABLE health_questions DROP CONSTRAINT health_questions_author_patient_id_fkey;
ALTER TABLE health_questions ADD CONSTRAINT health_questions_author_patient_id_fkey FOREIGN KEY(author_patient_id) REFERENCES patient_profiles(id) ON DELETE SET NULL;
DROP POLICY doctor_articles_published_read ON doctor_articles;
CREATE POLICY doctor_articles_published_read ON doctor_articles FOR SELECT USING (
    status='PUBLISHED' AND current_setting('app.current_role',true) IN
      ('PATIENT','DOCTOR','PLATFORM_ADMIN','CLINICAL_ADMIN','FINANCE_ADMIN','SUPPORT','COMPLIANCE','SECURITY_ADMIN')
    AND EXISTS (SELECT 1 FROM doctor_profiles d JOIN users u ON u.id=d.user_id
      WHERE d.id=author_id AND d.professional_verification_status='VERIFIED'
        AND d.deleted_at IS NULL AND u.is_active AND u.deleted_at IS NULL)
);
DROP POLICY patient_profile_self_update ON patient_profiles;
DROP POLICY wallet_owner_insert ON doctor_wallets;
DROP POLICY grievance_patient_author ON grievance_reports;
DROP POLICY booking_patient_author ON appointments;
DROP POLICY intake_document_author_guard ON prescription_intake_documents;
DROP POLICY rx_author_guard ON prescriptions;
DROP POLICY chat_sender_guard ON chat_messages;
GRANT UPDATE ON chat_messages TO hellodoctor_api;
DROP POLICY system_errors_operations ON system_error_logs;
DROP POLICY device_tokens_auth_logout_read ON notification_device_tokens;
DROP POLICY device_tokens_auth_logout_update ON notification_device_tokens;
REVOKE SELECT,UPDATE ON notification_device_tokens FROM hellodoctor_auth_worker;
CREATE POLICY notification_device_tokens_backend_select ON notification_device_tokens FOR SELECT TO hellodoctor_api USING(true);
CREATE POLICY notification_device_tokens_backend_disable ON notification_device_tokens FOR UPDATE TO hellodoctor_api USING(true) WITH CHECK(true);
DROP POLICY system_errors_worker ON system_error_logs;
ALTER TABLE system_error_logs NO FORCE ROW LEVEL SECURITY;
ALTER TABLE system_error_logs DISABLE ROW LEVEL SECURITY;
DROP POLICY idempotency_subject ON idempotency_records;
ALTER TABLE idempotency_records NO FORCE ROW LEVEL SECURITY;
ALTER TABLE idempotency_records DISABLE ROW LEVEL SECURITY;
DROP POLICY users_api_read ON users;
GRANT SELECT ON users TO hellodoctor_api;
DROP POLICY users_auth_bootstrap ON users;
DROP POLICY users_scheduler_read ON users;
ALTER TABLE users NO FORCE ROW LEVEL SECURITY;
ALTER TABLE users DISABLE ROW LEVEL SECURITY;
DROP POLICY doctor_private_read ON doctor_profiles;
DROP POLICY doctor_private_insert ON doctor_profiles;
DROP POLICY doctor_private_update ON doctor_profiles;
DROP POLICY doctor_auth_read ON doctor_profiles;
DROP POLICY doctor_auth_insert ON doctor_profiles;
DROP POLICY doctor_worker_read ON doctor_profiles;
ALTER TABLE doctor_profiles NO FORCE ROW LEVEL SECURITY;
ALTER TABLE doctor_profiles DISABLE ROW LEVEL SECURITY;
DROP VIEW doctor_public_directory;
DROP TRIGGER grievance_canonical_subject ON grievance_reports;
DROP TRIGGER appointment_grievance_subject ON appointments;
DROP TRIGGER consultation_grievance_subject ON consultation_sessions;
DROP FUNCTION guard_grievance_subject();
ALTER TABLE chat_conversations DROP CONSTRAINT chat_appointment_participants;
ALTER TABLE prescriptions DROP CONSTRAINT prescription_appointment_participants;
ALTER TABLE prescription_intake_documents DROP CONSTRAINT document_appointment_patient;
ALTER TABLE appointment_clinical_intake DROP CONSTRAINT intake_appointment_patient;
ALTER TABLE appointments DROP CONSTRAINT appointment_identity_participants;
ALTER TABLE appointments DROP CONSTRAINT appointment_identity_patient;
ALTER TABLE appointments DROP CONSTRAINT appointment_slot_doctor;
ALTER TABLE doctor_schedule_slots DROP CONSTRAINT slots_identity_doctor;
DROP TRIGGER canonical_account_role ON users;
DROP TRIGGER canonical_patient_commit ON patient_profiles;
DROP TRIGGER canonical_doctor_commit ON doctor_profiles;
DROP TRIGGER canonical_doctor_identity ON doctor_profiles;
DROP TRIGGER canonical_patient_identity ON patient_profiles;
DROP FUNCTION guard_profile_identity();
CREATE OR REPLACE FUNCTION fn_doctor_treats_patient(p_patient_profile_id uuid,p_requesting_user_id uuid)
RETURNS boolean LANGUAGE sql SECURITY DEFINER SET search_path=public STABLE AS $$
 SELECT EXISTS(SELECT 1 FROM appointments a JOIN doctor_profiles d ON d.id=a.doctor_id
 WHERE a.patient_id=p_patient_profile_id AND d.user_id=p_requesting_user_id);
$$;
REVOKE ALL ON FUNCTION fn_doctor_treats_patient(uuid,uuid) FROM hellodoctor_api,hellodoctor_auth_worker,hellodoctor_payment_worker,hellodoctor_finance_worker,hellodoctor_system_worker;
GRANT EXECUTE ON FUNCTION fn_doctor_treats_patient(uuid,uuid) TO PUBLIC;
