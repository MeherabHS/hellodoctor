-- ============================================================================
-- Doctor payout method setup.
--
-- A doctor's wallet summary (doctor_wallets / list_own_appointments / the
-- disbursement service) already computes gross/withheld/net correctly, but
-- there was previously nowhere for a doctor to tell Finance *where* a
-- scheduled disbursement should actually go (mobile money number, bank card,
-- etc). This table stores exactly that — a destination, not a live payment
-- rail. Creating/updating it never moves money and never calls an external
-- payment provider; DEFERRED_EXTERNAL_PAYMENT_INTEGRATION (CLAUDE.md) is
-- unchanged. Finance reads this when actually processing a disbursement
-- batch (manual/off-platform today, same as the rest of the disbursement
-- flow), exactly the same deferred boundary doctor_wallets already has.
--
-- The raw account identifier (mobile money number / card reference) is
-- encrypted at rest with the same AES-256-GCM scheme already used for TOTP
-- secrets (crypto::encrypt_secret, TOTP_ENCRYPTION_KEY_B64 — a general-purpose
-- at-rest key, not actually TOTP-specific). Only a last-4 fragment is kept in
-- the clear, for the doctor to visually confirm which method is on file
-- without the backend ever needing to decrypt it just to render the wallet
-- screen.
-- ============================================================================

CREATE TYPE payout_method_type_enum AS ENUM ('MPESA', 'BKASH', 'BANK_CARD', 'BANK_TRANSFER');

CREATE TABLE doctor_payout_methods (
    doctor_id UUID PRIMARY KEY REFERENCES doctor_profiles(id) ON DELETE CASCADE,
    method_type payout_method_type_enum NOT NULL,
    display_label VARCHAR(120) NOT NULL,
    account_identifier_encrypted BYTEA NOT NULL,
    account_identifier_last4 VARCHAR(4) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

ALTER TABLE doctor_payout_methods ENABLE ROW LEVEL SECURITY;
ALTER TABLE doctor_payout_methods FORCE ROW LEVEL SECURITY;

-- A doctor may only ever see/edit their own payout method; Finance (and
-- Platform Admin, for support/escalation parity with every other financial
-- table in this migration set) may read every doctor's, to actually process
-- disbursements, but never write one on a doctor's behalf.
CREATE POLICY doctor_payout_methods_self ON doctor_payout_methods
USING (
    EXISTS (
        SELECT 1 FROM doctor_profiles d
        WHERE d.id = doctor_payout_methods.doctor_id
          AND d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
    )
);

CREATE POLICY doctor_payout_methods_finance_read ON doctor_payout_methods
    FOR SELECT
    USING (current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'FINANCE_ADMIN'));

GRANT SELECT, INSERT, UPDATE ON doctor_payout_methods TO hellodoctor_api;
GRANT SELECT ON doctor_payout_methods TO hellodoctor_finance_worker;
