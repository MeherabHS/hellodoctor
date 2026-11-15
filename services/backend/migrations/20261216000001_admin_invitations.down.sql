-- Admin accounts created through invitations stay (they are ordinary users);
-- only the invitation records and the email type are removed. Existing
-- ADMIN_INVITATION outbox rows are kept: the restored constraint is NOT VALID
-- so they are not rewritten or deleted.
ALTER TABLE email_outbox DROP CONSTRAINT email_outbox_email_type_check;
ALTER TABLE email_outbox ADD CONSTRAINT email_outbox_email_type_check
    CHECK (email_type IN (
        'PATIENT_OTP', 'DOCTOR_OTP', 'ADMIN_OTP', 'PASSWORD_RESET_OTP',
        'PATIENT_WELCOME',
        'DOCTOR_APPLICATION_RECEIVED', 'DOCTOR_APPROVED', 'DOCTOR_REJECTED',
        'CONSULTATION_CONFIRMED', 'CONSULTATION_RESCHEDULED',
        'PAYMENT_CONFIRMED', 'REFUND_PROCESSED',
        'PASSWORD_CHANGED'
    )) NOT VALID;

DROP TABLE IF EXISTS admin_invitations;
DROP FUNCTION IF EXISTS guard_admin_invitation();
