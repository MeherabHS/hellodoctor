DROP INDEX IF EXISTS idx_monthly_payout_statements_destination;
ALTER TABLE monthly_payout_statements
    DROP COLUMN IF EXISTS masked_destination_snapshot,
    DROP COLUMN IF EXISTS provider_reference_snapshot,
    DROP COLUMN IF EXISTS provider_code,
    DROP COLUMN IF EXISTS payout_destination_id;
