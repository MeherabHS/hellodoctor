DROP INDEX IF EXISTS idx_transaction_disbursement_item;
CREATE UNIQUE INDEX IF NOT EXISTS uq_transaction_disbursement_allocation
    ON transactions(disbursement_item_id)
    WHERE disbursement_item_id IS NOT NULL;
