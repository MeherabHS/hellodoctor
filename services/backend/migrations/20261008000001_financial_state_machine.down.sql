DROP TABLE IF EXISTS financial_adjustments;
DROP TABLE IF EXISTS monthly_payout_statements;
DROP TABLE IF EXISTS financial_periods;
ALTER TABLE disbursement_items DROP COLUMN IF EXISTS reconciliation_state;
ALTER TABLE disbursement_items DROP COLUMN IF EXISTS paid_at;
ALTER TABLE disbursement_items DROP COLUMN IF EXISTS approved_at;
ALTER TABLE disbursement_items DROP COLUMN IF EXISTS approved_by;
ALTER TABLE transactions DROP COLUMN IF EXISTS disbursement_item_id;
