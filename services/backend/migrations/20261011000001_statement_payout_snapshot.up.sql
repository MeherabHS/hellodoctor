-- Preserve the destination shown on a finalized statement. A Doctor changing
-- their primary destination later must not rewrite historical financial data.
ALTER TABLE monthly_payout_statements
    ADD COLUMN IF NOT EXISTS payout_destination_id UUID REFERENCES doctor_payout_destinations(id),
    ADD COLUMN IF NOT EXISTS provider_code VARCHAR(40),
    ADD COLUMN IF NOT EXISTS provider_reference_snapshot VARCHAR(180),
    ADD COLUMN IF NOT EXISTS masked_destination_snapshot VARCHAR(80);

CREATE INDEX IF NOT EXISTS idx_monthly_payout_statements_destination
    ON monthly_payout_statements(payout_destination_id);
