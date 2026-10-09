-- A disbursement item aggregates many settled transactions for one doctor.
-- Allocation is unique per transaction (the column is single-valued); it must
-- not be unique per item, otherwise the second transaction in an item fails.
DROP INDEX IF EXISTS uq_transaction_disbursement_allocation;
CREATE INDEX IF NOT EXISTS idx_transaction_disbursement_item
    ON transactions(disbursement_item_id)
    WHERE disbursement_item_id IS NOT NULL;
