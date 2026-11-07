DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM email_outbox WHERE email_type NOT IN ('PATIENT_OTP', 'DOCTOR_OTP', 'ADMIN_OTP')) THEN
        RAISE EXCEPTION 'Cannot downgrade email_outbox.email_type: rows using a Phase 2 category already exist';
    END IF;
END $$;

REVOKE INSERT ON email_outbox FROM hellodoctor_system_worker;

ALTER TABLE email_outbox DROP CONSTRAINT email_outbox_email_type_check;
ALTER TABLE email_outbox ADD CONSTRAINT email_outbox_email_type_check
    CHECK (email_type IN ('PATIENT_OTP', 'DOCTOR_OTP', 'ADMIN_OTP'));
