-- Stage A extension: durable cleanup for unpaid, expired booking holds.
-- The appointment row is the access-control authority.  This table retains
-- only private object keys needed for physical storage deletion; clinical
-- metadata is removed in the same expiry transaction.

ALTER TABLE notifications DROP CONSTRAINT notifications_type_check;
ALTER TABLE notifications ADD CONSTRAINT notifications_type_check CHECK (type IN (
    'DAILY_CONSULTATION_SUMMARY',
    'CONSULTATION_REMINDER_15M',
    'EARNING_CREDITED',
    'PAYOUT_PAID',
    'REGISTRATION_VERIFIED',
    'REGISTRATION_REJECTED',
    'ARTICLE_STATUS_CHANGED',
    'BOOKING_HOLD_CREATED',
    'BOOKING_HOLD_ONE_MINUTE',
    'BOOKING_HOLD_EXPIRED'
));

CREATE FUNCTION protect_payment_hold_deadline() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.hold_expires_at IS DISTINCT FROM OLD.hold_expires_at THEN
        RAISE EXCEPTION 'A booking hold deadline is immutable' USING ERRCODE = 'check_violation';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER appointments_hold_deadline_immutable
    BEFORE UPDATE OF hold_expires_at ON appointments
    FOR EACH ROW EXECUTE FUNCTION protect_payment_hold_deadline();

CREATE TABLE appointment_expiry_cleanup_jobs (
    appointment_id UUID PRIMARY KEY REFERENCES appointments(id) ON DELETE RESTRICT,
    object_bucket VARCHAR(100) NOT NULL,
    object_keys TEXT[] NOT NULL DEFAULT '{}',
    state VARCHAR(20) NOT NULL DEFAULT 'PENDING'
        CHECK (state IN ('PENDING', 'RUNNING', 'COMPLETED', 'FAILED')),
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    lease_until TIMESTAMPTZ,
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    completed_at TIMESTAMPTZ
);

CREATE INDEX idx_appointment_expiry_cleanup_due
    ON appointment_expiry_cleanup_jobs (next_attempt_at, appointment_id)
    WHERE state <> 'COMPLETED';

ALTER TABLE appointment_expiry_cleanup_jobs ENABLE ROW LEVEL SECURITY;
ALTER TABLE appointment_expiry_cleanup_jobs FORCE ROW LEVEL SECURITY;
CREATE POLICY appointment_expiry_cleanup_jobs_system_worker ON appointment_expiry_cleanup_jobs
    FOR ALL TO hellodoctor_system_worker USING (true) WITH CHECK (true);
GRANT SELECT, INSERT, UPDATE ON appointment_expiry_cleanup_jobs TO hellodoctor_system_worker;
CREATE POLICY appointment_expiry_cleanup_jobs_payment_worker ON appointment_expiry_cleanup_jobs
    FOR INSERT TO hellodoctor_payment_worker WITH CHECK (true);
GRANT INSERT ON appointment_expiry_cleanup_jobs TO hellodoctor_payment_worker;
GRANT SELECT ON patient_profiles TO hellodoctor_system_worker;

-- Expiry removes only the appointment's own intake rows.  The restrictive
-- system-worker policy is deliberately separate from patient/doctor access.
CREATE POLICY appointment_clinical_intake_system_expiry_cleanup
    ON appointment_clinical_intake
    FOR DELETE TO hellodoctor_system_worker USING (true);
CREATE POLICY appointment_clinical_intake_system_expiry_select
    ON appointment_clinical_intake
    FOR SELECT TO hellodoctor_system_worker USING (true);
CREATE POLICY prescription_intake_documents_system_expiry_cleanup
    ON prescription_intake_documents
    FOR DELETE TO hellodoctor_system_worker USING (true);
CREATE POLICY prescription_intake_documents_system_expiry_select
    ON prescription_intake_documents
    FOR SELECT TO hellodoctor_system_worker USING (true);
GRANT SELECT, DELETE ON appointment_clinical_intake, prescription_intake_documents
    TO hellodoctor_system_worker;

CREATE POLICY appointment_clinical_intake_payment_expiry_cleanup
    ON appointment_clinical_intake
    FOR DELETE TO hellodoctor_payment_worker USING (
        EXISTS (SELECT 1 FROM appointments a
                WHERE a.id=appointment_clinical_intake.appointment_id
                  AND a.status='PENDING_PAYMENT')
    );
CREATE POLICY appointment_clinical_intake_payment_expiry_select
    ON appointment_clinical_intake
    FOR SELECT TO hellodoctor_payment_worker USING (
        EXISTS (SELECT 1 FROM appointments a
                WHERE a.id=appointment_clinical_intake.appointment_id
                  AND a.status='PENDING_PAYMENT')
    );
CREATE POLICY prescription_intake_documents_payment_expiry_cleanup
    ON prescription_intake_documents
    FOR DELETE TO hellodoctor_payment_worker USING (
        EXISTS (SELECT 1 FROM appointments a
                WHERE a.id=prescription_intake_documents.appointment_id
                  AND a.status='PENDING_PAYMENT')
    );
CREATE POLICY prescription_intake_documents_payment_expiry_select
    ON prescription_intake_documents
    FOR SELECT TO hellodoctor_payment_worker USING (
        EXISTS (SELECT 1 FROM appointments a
                WHERE a.id=prescription_intake_documents.appointment_id
                  AND a.status='PENDING_PAYMENT')
    );
GRANT SELECT, DELETE ON appointment_clinical_intake, prescription_intake_documents
    TO hellodoctor_payment_worker;

