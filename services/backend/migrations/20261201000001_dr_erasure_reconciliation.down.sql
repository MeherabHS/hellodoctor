DROP TRIGGER protect_dr_recovery_state ON dr_recovery_state;
DROP FUNCTION protect_dr_recovery_state();
DROP TABLE dr_recovery_state;
DROP TRIGGER protect_erasure_outbox ON durable_erasure_journal_outbox;
DROP FUNCTION protect_erasure_outbox();
DROP TABLE durable_erasure_journal_outbox;
