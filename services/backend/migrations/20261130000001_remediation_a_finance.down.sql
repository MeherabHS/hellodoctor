DO $$ BEGIN
 IF EXISTS(SELECT 1 FROM transactions WHERE reconciliation_required) THEN
  RAISE EXCEPTION 'Review outstanding reconciliation obligations before rollback';
 END IF;
END $$;
DROP INDEX transactions_reconciliation_required;
ALTER TABLE transactions DROP COLUMN reconciliation_required;
ALTER TABLE transactions DROP COLUMN provider_request_state;
