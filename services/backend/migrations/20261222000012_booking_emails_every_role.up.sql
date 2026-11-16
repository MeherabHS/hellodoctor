-- Booking emails for every role (2026-11-15): the Doctor gets "new
-- consultation booked"; Peer Navigators and patients get peer-support
-- session emails (requested, booked, confirmed, declined, cancelled). The
-- dispatch worker renders them from the request at send time, so it needs
-- read access to peer_requests and peer_slots. It never reads the topic
-- into an email.
ALTER TABLE email_outbox DROP CONSTRAINT email_outbox_email_type_check;
ALTER TABLE email_outbox ADD CONSTRAINT email_outbox_email_type_check
    CHECK (email_type IN (
        'PATIENT_OTP', 'DOCTOR_OTP', 'ADMIN_OTP', 'PASSWORD_RESET_OTP', 'PEER_OTP',
        'PATIENT_WELCOME',
        'DOCTOR_APPLICATION_RECEIVED', 'DOCTOR_APPROVED', 'DOCTOR_REJECTED',
        'PEER_APPLICATION_RECEIVED', 'PEER_APPROVED', 'PEER_REJECTED',
        'PEER_SUSPENDED', 'PEER_REINSTATED',
        'CONSULTATION_CONFIRMED', 'CONSULTATION_RESCHEDULED',
        'CONSULTATION_BOOKED_DOCTOR',
        'PEER_SESSION_REQUESTED', 'PEER_SESSION_BOOKED', 'PEER_SESSION_CANCELLED',
        'PEER_SESSION_CONFIRMED', 'PEER_SESSION_DECLINED',
        'PAYMENT_CONFIRMED', 'REFUND_PROCESSED',
        'PASSWORD_CHANGED',
        'ADMIN_INVITATION'
    ));

GRANT SELECT ON peer_requests TO hellodoctor_system_worker;
GRANT SELECT ON peer_slots TO hellodoctor_system_worker;
CREATE POLICY peer_requests_system_worker_read ON peer_requests
    FOR SELECT TO hellodoctor_system_worker USING (true);
CREATE POLICY peer_slots_system_worker_read ON peer_slots
    FOR SELECT TO hellodoctor_system_worker USING (true);
