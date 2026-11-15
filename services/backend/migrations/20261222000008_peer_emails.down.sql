DROP POLICY IF EXISTS peer_profiles_system_worker_read ON peer_profiles;
REVOKE SELECT ON peer_profiles FROM hellodoctor_system_worker;
DELETE FROM email_outbox WHERE email_type IN (
    'PEER_APPLICATION_RECEIVED', 'PEER_APPROVED', 'PEER_REJECTED',
    'PEER_SUSPENDED', 'PEER_REINSTATED');
ALTER TABLE email_outbox DROP CONSTRAINT email_outbox_email_type_check;
ALTER TABLE email_outbox ADD CONSTRAINT email_outbox_email_type_check
    CHECK (email_type IN (
        'PATIENT_OTP', 'DOCTOR_OTP', 'ADMIN_OTP', 'PASSWORD_RESET_OTP', 'PEER_OTP',
        'PATIENT_WELCOME',
        'DOCTOR_APPLICATION_RECEIVED', 'DOCTOR_APPROVED', 'DOCTOR_REJECTED',
        'CONSULTATION_CONFIRMED', 'CONSULTATION_RESCHEDULED',
        'PAYMENT_CONFIRMED', 'REFUND_PROCESSED',
        'PASSWORD_CHANGED',
        'ADMIN_INVITATION'
    ));
