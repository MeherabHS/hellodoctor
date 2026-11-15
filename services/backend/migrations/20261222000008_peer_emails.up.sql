-- Peer Navigator emails (2026-11-15): application received, approved,
-- declined, paused and reinstated go through the transactional outbox like
-- the doctor emails. The dispatch worker renders them from peer_profiles at
-- send time, so it needs read access to that table.
ALTER TABLE email_outbox DROP CONSTRAINT email_outbox_email_type_check;
ALTER TABLE email_outbox ADD CONSTRAINT email_outbox_email_type_check
    CHECK (email_type IN (
        'PATIENT_OTP', 'DOCTOR_OTP', 'ADMIN_OTP', 'PASSWORD_RESET_OTP', 'PEER_OTP',
        'PATIENT_WELCOME',
        'DOCTOR_APPLICATION_RECEIVED', 'DOCTOR_APPROVED', 'DOCTOR_REJECTED',
        'PEER_APPLICATION_RECEIVED', 'PEER_APPROVED', 'PEER_REJECTED',
        'PEER_SUSPENDED', 'PEER_REINSTATED',
        'CONSULTATION_CONFIRMED', 'CONSULTATION_RESCHEDULED',
        'PAYMENT_CONFIRMED', 'REFUND_PROCESSED',
        'PASSWORD_CHANGED',
        'ADMIN_INVITATION'
    ));

GRANT SELECT ON peer_profiles TO hellodoctor_system_worker;
CREATE POLICY peer_profiles_system_worker_read ON peer_profiles
    FOR SELECT TO hellodoctor_system_worker USING (true);
