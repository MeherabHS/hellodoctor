REVOKE SELECT, UPDATE, INSERT ON email_outbox FROM hellodoctor_payment_worker;

REVOKE SELECT ON patient_profiles FROM hellodoctor_finance_worker;
DROP POLICY patient_profiles_finance_worker_read ON patient_profiles;
REVOKE SELECT (email) ON users FROM hellodoctor_finance_worker;

REVOKE SELECT (id, email) ON users FROM hellodoctor_payment_worker;
DROP POLICY users_payment_worker_read ON users;

REVOKE SELECT ON appointment_reschedules FROM hellodoctor_system_worker;
DROP POLICY appointment_reschedules_system_worker_read ON appointment_reschedules;

REVOKE SELECT ON financial_refund_requests FROM hellodoctor_system_worker;
DROP POLICY financial_refund_requests_system_worker_read ON financial_refund_requests;

REVOKE SELECT ON transactions FROM hellodoctor_system_worker;
DROP POLICY transactions_system_worker_read ON transactions;

REVOKE SELECT, INSERT ON email_outbox FROM hellodoctor_api;
REVOKE SELECT, INSERT ON email_outbox FROM hellodoctor_finance_worker;

DROP POLICY email_outbox_api_worker ON email_outbox;
DROP POLICY email_outbox_payment_worker ON email_outbox;
DROP POLICY email_outbox_finance_worker ON email_outbox;

DROP INDEX idx_email_outbox_related_entity;
DROP INDEX idx_email_outbox_claimable;

ALTER TABLE email_outbox DROP CONSTRAINT email_outbox_delivery_status_check;
ALTER TABLE email_outbox ADD CONSTRAINT email_outbox_delivery_status_check
    CHECK (delivery_status IN (
        'CREATED', 'SENT_TO_PROVIDER', 'FAILED', 'EXPIRED'
    ));

ALTER TABLE email_outbox
    DROP COLUMN related_entity_type,
    DROP COLUMN related_entity_id,
    DROP COLUMN claimed_until,
    DROP COLUMN next_attempt_at;
