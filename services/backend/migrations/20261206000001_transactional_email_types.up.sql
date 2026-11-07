-- Phase 2 email integration: widens the email_outbox.email_type CHECK
-- constraint to admit transactional/onboarding email categories, and grants
-- hellodoctor_system_worker INSERT (it previously only had SELECT, UPDATE —
-- see migrations/20261205000001_email_outbox). Unlike OTP (sent on the
-- hellodoctor_auth_worker pool, inside the login/registration request),
-- every Phase 2 transactional email is sent on the hellodoctor_system_worker
-- pool, AFTER its triggering business transaction has already committed —
-- see services/email_delivery.rs::send_templated_email's module doc for why
-- this must never run inside the triggering transaction.
ALTER TABLE email_outbox DROP CONSTRAINT email_outbox_email_type_check;
ALTER TABLE email_outbox ADD CONSTRAINT email_outbox_email_type_check
    CHECK (email_type IN (
        'PATIENT_OTP', 'DOCTOR_OTP', 'ADMIN_OTP',
        'PATIENT_WELCOME',
        'DOCTOR_APPLICATION_RECEIVED', 'DOCTOR_APPROVED', 'DOCTOR_REJECTED',
        'CONSULTATION_CONFIRMED', 'CONSULTATION_RESCHEDULED',
        'PAYMENT_CONFIRMED', 'REFUND_PROCESSED',
        'PASSWORD_CHANGED'
    ));

GRANT INSERT ON email_outbox TO hellodoctor_system_worker;
