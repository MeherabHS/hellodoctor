-- Reverses the five-role RLS hardening migration. Drops every policy this
-- migration created, disables RLS on every table it force-enabled RLS on,
-- revokes every grant, then drops the five capability roles.

DROP POLICY IF EXISTS audit_events_select ON audit_events;
DROP POLICY IF EXISTS audit_events_insert ON audit_events;
ALTER TABLE audit_events NO FORCE ROW LEVEL SECURITY;
ALTER TABLE audit_events DISABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS grievance_adjudications_finance_worker_insert ON grievance_adjudications;
DROP POLICY IF EXISTS grievance_adjudications_access ON grievance_adjudications;
ALTER TABLE grievance_adjudications NO FORCE ROW LEVEL SECURITY;
ALTER TABLE grievance_adjudications DISABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS grievance_reports_finance_worker_all ON grievance_reports;
DROP POLICY IF EXISTS grievance_reports_insert ON grievance_reports;
DROP POLICY IF EXISTS grievance_reports_access ON grievance_reports;
ALTER TABLE grievance_reports NO FORCE ROW LEVEL SECURITY;
ALTER TABLE grievance_reports DISABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS disbursement_items_finance_worker_all ON disbursement_items;
DROP POLICY IF EXISTS disbursement_items_access ON disbursement_items;
ALTER TABLE disbursement_items NO FORCE ROW LEVEL SECURITY;
ALTER TABLE disbursement_items DISABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS disbursement_batches_finance_worker_all ON disbursement_batches;
DROP POLICY IF EXISTS disbursement_batches_access ON disbursement_batches;
ALTER TABLE disbursement_batches NO FORCE ROW LEVEL SECURITY;
ALTER TABLE disbursement_batches DISABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS doctor_wallets_api_insert ON doctor_wallets;
DROP POLICY IF EXISTS doctor_wallets_finance_worker_all ON doctor_wallets;
DROP POLICY IF EXISTS doctor_wallets_access ON doctor_wallets;
ALTER TABLE doctor_wallets NO FORCE ROW LEVEL SECURITY;
ALTER TABLE doctor_wallets DISABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS payment_events_finance_worker_all ON payment_events;
DROP POLICY IF EXISTS payment_events_payment_worker_all ON payment_events;
DROP POLICY IF EXISTS payment_events_access ON payment_events;
ALTER TABLE payment_events NO FORCE ROW LEVEL SECURITY;
ALTER TABLE payment_events DISABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS transactions_finance_worker_all ON transactions;
DROP POLICY IF EXISTS transactions_payment_worker_all ON transactions;
DROP POLICY IF EXISTS transactions_access ON transactions;
ALTER TABLE transactions NO FORCE ROW LEVEL SECURITY;
ALTER TABLE transactions DISABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS chat_messages_insert ON chat_messages;
DROP POLICY IF EXISTS chat_messages_access ON chat_messages;
ALTER TABLE chat_messages NO FORCE ROW LEVEL SECURITY;
ALTER TABLE chat_messages DISABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS chat_conversations_system_worker_all ON chat_conversations;
DROP POLICY IF EXISTS chat_conversations_access ON chat_conversations;
ALTER TABLE chat_conversations NO FORCE ROW LEVEL SECURITY;
ALTER TABLE chat_conversations DISABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS consultation_telemetry_access ON consultation_telemetry;
ALTER TABLE consultation_telemetry NO FORCE ROW LEVEL SECURITY;
ALTER TABLE consultation_telemetry DISABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS prescriptions_access ON prescriptions;
ALTER TABLE prescriptions NO FORCE ROW LEVEL SECURITY;
ALTER TABLE prescriptions DISABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS consultation_sessions_finance_worker_select ON consultation_sessions;
DROP POLICY IF EXISTS consultation_sessions_system_worker_all ON consultation_sessions;
DROP POLICY IF EXISTS consultation_sessions_access ON consultation_sessions;
ALTER TABLE consultation_sessions NO FORCE ROW LEVEL SECURITY;
ALTER TABLE consultation_sessions DISABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS prescription_intake_documents_access ON prescription_intake_documents;
ALTER TABLE prescription_intake_documents NO FORCE ROW LEVEL SECURITY;
ALTER TABLE prescription_intake_documents DISABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS appointment_clinical_intake_payment_worker_insert ON appointment_clinical_intake;
DROP POLICY IF EXISTS appointment_clinical_intake_access ON appointment_clinical_intake;
ALTER TABLE appointment_clinical_intake NO FORCE ROW LEVEL SECURITY;
ALTER TABLE appointment_clinical_intake DISABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS appointments_finance_worker_select ON appointments;
DROP POLICY IF EXISTS appointments_system_worker_all ON appointments;
DROP POLICY IF EXISTS appointments_payment_worker_all ON appointments;
DROP POLICY IF EXISTS appointments_access ON appointments;
ALTER TABLE appointments NO FORCE ROW LEVEL SECURITY;
ALTER TABLE appointments DISABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS slots_system_worker_update ON doctor_schedule_slots;
DROP POLICY IF EXISTS slots_payment_worker_update ON doctor_schedule_slots;
DROP POLICY IF EXISTS slots_api_write ON doctor_schedule_slots;
DROP POLICY IF EXISTS slots_select ON doctor_schedule_slots;
ALTER TABLE doctor_schedule_slots NO FORCE ROW LEVEL SECURITY;
ALTER TABLE doctor_schedule_slots DISABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS patient_profiles_auth_worker_insert ON patient_profiles;
DROP POLICY IF EXISTS patient_profiles_access ON patient_profiles;
ALTER TABLE patient_profiles NO FORCE ROW LEVEL SECURITY;
ALTER TABLE patient_profiles DISABLE ROW LEVEL SECURITY;
DROP FUNCTION IF EXISTS fn_doctor_treats_patient(uuid, uuid);

REVOKE ALL PRIVILEGES ON
    users, patient_profiles, doctor_profiles, doctor_schedule_slots,
    appointments, appointment_clinical_intake, prescriptions,
    prescription_intake_documents, consultation_sessions, consultation_telemetry,
    chat_conversations, chat_messages, grievance_reports, grievance_adjudications,
    transactions, doctor_wallets, disbursement_batches, disbursement_items,
    payment_events, system_error_logs, audit_events, idempotency_records
FROM hellodoctor_api;

REVOKE ALL PRIVILEGES ON
    users, auth_sessions, mfa_credentials, phone_otps, pending_mfa_challenges, patient_profiles, doctor_profiles
FROM hellodoctor_auth_worker;

REVOKE ALL PRIVILEGES ON appointments, appointment_clinical_intake, doctor_schedule_slots, transactions, payment_events
FROM hellodoctor_payment_worker;

REVOKE ALL PRIVILEGES ON
    appointments, doctor_schedule_slots, chat_conversations, consultation_sessions, system_error_logs
FROM hellodoctor_system_worker;

REVOKE ALL PRIVILEGES ON
    grievance_reports, grievance_adjudications, transactions, doctor_wallets,
    disbursement_batches, disbursement_items, payment_events, consultation_sessions,
    appointments
FROM hellodoctor_finance_worker;

REVOKE USAGE ON SCHEMA public FROM
    hellodoctor_api, hellodoctor_auth_worker, hellodoctor_payment_worker,
    hellodoctor_system_worker, hellodoctor_finance_worker;
REVOKE CONNECT ON DATABASE hellodoctor FROM
    hellodoctor_api, hellodoctor_auth_worker, hellodoctor_payment_worker,
    hellodoctor_system_worker, hellodoctor_finance_worker;

DO $$
BEGIN
    IF EXISTS (SELECT FROM pg_roles WHERE rolname = 'hellodoctor_api') THEN
        DROP ROLE hellodoctor_api;
    END IF;
    IF EXISTS (SELECT FROM pg_roles WHERE rolname = 'hellodoctor_auth_worker') THEN
        DROP ROLE hellodoctor_auth_worker;
    END IF;
    IF EXISTS (SELECT FROM pg_roles WHERE rolname = 'hellodoctor_payment_worker') THEN
        DROP ROLE hellodoctor_payment_worker;
    END IF;
    IF EXISTS (SELECT FROM pg_roles WHERE rolname = 'hellodoctor_system_worker') THEN
        DROP ROLE hellodoctor_system_worker;
    END IF;
    IF EXISTS (SELECT FROM pg_roles WHERE rolname = 'hellodoctor_finance_worker') THEN
        DROP ROLE hellodoctor_finance_worker;
    END IF;
END
$$;
