-- Kenya-only payout routing foundation. Provider credentials and execution
-- remain outside this contract; only provider-issued references are stored.
ALTER TABLE transactions ADD COLUMN IF NOT EXISTS currency CHAR(3) NOT NULL DEFAULT 'KES';
ALTER TABLE transactions ADD CONSTRAINT transactions_currency_kenya CHECK (currency = 'KES');

CREATE TABLE doctor_payout_destinations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    doctor_id UUID NOT NULL REFERENCES doctor_profiles(id) ON DELETE CASCADE,
    provider_code VARCHAR(40) NOT NULL,
    method_type VARCHAR(30) NOT NULL,
    currency CHAR(3) NOT NULL DEFAULT 'KES' CHECK (currency = 'KES'),
    provider_reference VARCHAR(180) NOT NULL,
    masked_destination VARCHAR(80) NOT NULL,
    binding_status VARCHAR(30) NOT NULL,
    verification_status VARCHAR(30) NOT NULL,
    payout_capable BOOLEAN NOT NULL DEFAULT FALSE,
    is_primary BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_doctor_payout_destinations_doctor ON doctor_payout_destinations(doctor_id);
CREATE UNIQUE INDEX uq_doctor_primary_kenya_payout
    ON doctor_payout_destinations(doctor_id, currency)
    WHERE is_primary = TRUE AND binding_status = 'ACTIVE' AND payout_capable = TRUE;

ALTER TABLE disbursement_items ADD COLUMN IF NOT EXISTS payout_destination_id UUID REFERENCES doctor_payout_destinations(id);
ALTER TABLE disbursement_items ADD COLUMN IF NOT EXISTS currency CHAR(3) NOT NULL DEFAULT 'KES';
ALTER TABLE disbursement_items ADD COLUMN IF NOT EXISTS provider_reference_snapshot VARCHAR(180);
ALTER TABLE disbursement_items ADD COLUMN IF NOT EXISTS masked_destination_snapshot VARCHAR(80);

ALTER TABLE doctor_payout_destinations ENABLE ROW LEVEL SECURITY;
CREATE POLICY doctor_payout_destination_owner ON doctor_payout_destinations
    USING (current_setting('app.current_role', true) IN ('PLATFORM_ADMIN','FINANCE_ADMIN')
        OR doctor_id IN (SELECT id FROM doctor_profiles WHERE user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid))
    WITH CHECK (doctor_id IN (SELECT id FROM doctor_profiles WHERE user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid));
GRANT SELECT, INSERT, UPDATE ON doctor_payout_destinations TO hellodoctor_api;
GRANT SELECT ON doctor_payout_destinations TO hellodoctor_finance_worker;
