-- Patient-saved payment methods ("the body waiting for the heart" — the
-- real payment-gateway API key/driver). Mirrors doctor_payout_destinations'
-- already-proven shape (multiple rows per owner, exactly one active primary
-- enforced by a partial unique index) rather than inventing a new pattern:
-- a patient can save more than one payment method and pick a default, the
-- same way a doctor already can for payouts. No raw secret (PAN, PIN, OTP)
-- is ever accepted or stored — only a provider-issued reference, encrypted
-- at rest the same way doctor payout references and TOTP secrets already
-- are (crypto::encrypt_secret), with the masked display precomputed once at
-- save time so it never needs decrypting just to render in a list.
CREATE TABLE patient_payment_methods (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    patient_id UUID NOT NULL REFERENCES patient_profiles(id) ON DELETE CASCADE,
    gateway gateway_enum NOT NULL,
    provider_code VARCHAR(40) NOT NULL,
    provider_reference VARCHAR(180) NOT NULL,
    masked_destination VARCHAR(80) NOT NULL,
    binding_status VARCHAR(30) NOT NULL,
    verification_status VARCHAR(30) NOT NULL,
    is_primary BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_patient_payment_methods_patient ON patient_payment_methods(patient_id);
CREATE UNIQUE INDEX uq_patient_primary_payment_method
    ON patient_payment_methods(patient_id)
    WHERE is_primary = TRUE AND binding_status = 'ACTIVE';

-- Lets a confirmed booking/payment reference exactly which saved method the
-- patient chose, without requiring one (a patient may still pay ad hoc,
-- exactly as today, by sending gateway details with no saved method at all
-- — this column is nullable and purely additive).
ALTER TABLE transactions ADD COLUMN IF NOT EXISTS payment_method_id UUID REFERENCES patient_payment_methods(id);

ALTER TABLE patient_payment_methods ENABLE ROW LEVEL SECURITY;
ALTER TABLE patient_payment_methods FORCE ROW LEVEL SECURITY;
CREATE POLICY patient_payment_method_owner ON patient_payment_methods
    USING (current_setting('app.current_role', true) = 'ADMIN'
        OR fn_patient_profile_owned_by_caller(patient_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid))
    WITH CHECK (fn_patient_profile_owned_by_caller(patient_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid));
GRANT SELECT, INSERT, UPDATE, DELETE ON patient_payment_methods TO hellodoctor_api;
GRANT SELECT ON patient_payment_methods TO hellodoctor_finance_worker;
-- AppointmentService::book_appointment re-validates a selected saved
-- payment method (ownership + active status) inside its own payment-worker
-- transaction — confirmed directly by reading that function before writing
-- this grant, not assumed — so the payment-worker role needs read access
-- too, the same way it already reads doctor_profiles/appointments there.
GRANT SELECT ON patient_payment_methods TO hellodoctor_payment_worker;
