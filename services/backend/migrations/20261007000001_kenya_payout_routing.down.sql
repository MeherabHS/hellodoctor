ALTER TABLE disbursement_items DROP COLUMN IF EXISTS masked_destination_snapshot;
ALTER TABLE disbursement_items DROP COLUMN IF EXISTS provider_reference_snapshot;
ALTER TABLE disbursement_items DROP COLUMN IF EXISTS currency;
ALTER TABLE disbursement_items DROP COLUMN IF EXISTS payout_destination_id;
DROP TABLE IF EXISTS doctor_payout_destinations;
ALTER TABLE transactions DROP CONSTRAINT IF EXISTS transactions_currency_kenya;
ALTER TABLE transactions DROP COLUMN IF EXISTS currency;
