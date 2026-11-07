-- Add provenance, never classify or repair historical obligations automatically.
ALTER TABLE transactions ADD COLUMN provider_request_state TEXT NOT NULL DEFAULT 'UNKNOWN'
    CHECK (provider_request_state IN ('UNKNOWN','NOT_STARTED','STARTED'));
ALTER TABLE transactions ADD COLUMN reconciliation_required BOOLEAN NOT NULL DEFAULT FALSE;
COMMENT ON COLUMN transactions.provider_request_state IS
 'New checkouts explicitly record NOT_STARTED. A provider adapter must set STARTED durably BEFORE sending a request. UNKNOWN historical rows require reviewed reconciliation.';
CREATE INDEX transactions_reconciliation_required ON transactions(created_at,id)
 WHERE reconciliation_required;
