DELETE FROM email_outbox WHERE email_type = 'PEER_OTP';
ALTER TABLE email_outbox DROP CONSTRAINT email_outbox_email_type_check;
ALTER TABLE email_outbox ADD CONSTRAINT email_outbox_email_type_check
    CHECK (email_type IN (
        'PATIENT_OTP', 'DOCTOR_OTP', 'ADMIN_OTP', 'PASSWORD_RESET_OTP',
        'PATIENT_WELCOME',
        'DOCTOR_APPLICATION_RECEIVED', 'DOCTOR_APPROVED', 'DOCTOR_REJECTED',
        'CONSULTATION_CONFIRMED', 'CONSULTATION_RESCHEDULED',
        'PAYMENT_CONFIRMED', 'REFUND_PROCESSED',
        'PASSWORD_CHANGED',
        'ADMIN_INVITATION'
    ));
DROP TABLE IF EXISTS peer_slots;
DROP TRIGGER IF EXISTS canonical_peer_account_role ON users;
DROP TABLE IF EXISTS peer_profiles;
DROP FUNCTION IF EXISTS fn_current_peer_profile_id();
DROP FUNCTION IF EXISTS guard_peer_approval_fields();
DROP FUNCTION IF EXISTS guard_peer_profile_identity();
