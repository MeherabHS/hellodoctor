DROP POLICY IF EXISTS prescription_intake_documents_system_expiry_cleanup
    ON prescription_intake_documents;
DROP POLICY IF EXISTS appointment_clinical_intake_system_expiry_cleanup
    ON appointment_clinical_intake;
DROP POLICY IF EXISTS appointment_clinical_intake_system_expiry_select
    ON appointment_clinical_intake;
DROP POLICY IF EXISTS prescription_intake_documents_payment_expiry_cleanup
    ON prescription_intake_documents;
DROP POLICY IF EXISTS appointment_clinical_intake_payment_expiry_cleanup
    ON appointment_clinical_intake;
DROP POLICY IF EXISTS prescription_intake_documents_payment_expiry_select
    ON prescription_intake_documents;
DROP POLICY IF EXISTS appointment_clinical_intake_payment_expiry_select
    ON appointment_clinical_intake;
DROP POLICY IF EXISTS prescription_intake_documents_system_expiry_select
    ON prescription_intake_documents;
REVOKE SELECT, DELETE ON appointment_clinical_intake, prescription_intake_documents
    FROM hellodoctor_system_worker;
REVOKE SELECT, DELETE ON appointment_clinical_intake, prescription_intake_documents
    FROM hellodoctor_payment_worker;
REVOKE SELECT ON patient_profiles FROM hellodoctor_system_worker;
REVOKE INSERT ON appointment_expiry_cleanup_jobs FROM hellodoctor_payment_worker;

DROP TABLE appointment_expiry_cleanup_jobs;

DROP TRIGGER IF EXISTS appointments_hold_deadline_immutable ON appointments;
DROP FUNCTION IF EXISTS protect_payment_hold_deadline();

ALTER TABLE notifications DROP CONSTRAINT notifications_type_check;
ALTER TABLE notifications ADD CONSTRAINT notifications_type_check CHECK (type IN (
    'DAILY_CONSULTATION_SUMMARY',
    'CONSULTATION_REMINDER_15M',
    'EARNING_CREDITED',
    'PAYOUT_PAID',
    'REGISTRATION_VERIFIED',
    'REGISTRATION_REJECTED',
    'ARTICLE_STATUS_CHANGED'
));
