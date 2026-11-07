DROP POLICY email_outbox_system_worker ON email_outbox;
DROP POLICY email_outbox_auth_worker ON email_outbox;
REVOKE ALL ON email_outbox FROM hellodoctor_system_worker;
REVOKE ALL ON email_outbox FROM hellodoctor_auth_worker;
DROP INDEX idx_email_outbox_expiry_sweep;
DROP TABLE email_outbox;
