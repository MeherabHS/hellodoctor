-- The platform's own merchant/collection configuration (the "heart" this
-- table is built to receive): which payment provider(s) the platform itself
-- is registered with, and the credentials used to call them. Deliberately
-- separate from patient_payment_methods (how a patient pays) and
-- doctor_payout_destinations (where a doctor gets paid) — this is neither;
-- it is the platform's own merchant identity. ADMIN-only, FORCE RLS, no
-- patient/doctor grant at all. More than one method may be active at once
-- (e.g. M-Pesa and a card aggregator simultaneously) — "primary" is scoped
-- per provider_code, not a single platform-wide choice, since two
-- different gateway types are not interchangeable alternatives of each
-- other the way a patient's two M-Pesa numbers are.
CREATE TABLE platform_payment_methods (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider_code VARCHAR(40) NOT NULL,
    label VARCHAR(80) NOT NULL,
    currency CHAR(3) NOT NULL DEFAULT 'KES' CHECK (currency = 'KES'),
    masked_display VARCHAR(80) NOT NULL,
    credentials_encrypted TEXT NOT NULL,
    is_primary BOOLEAN NOT NULL DEFAULT FALSE,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE UNIQUE INDEX uq_platform_primary_payment_method_per_provider
    ON platform_payment_methods(provider_code)
    WHERE is_primary = TRUE AND is_active = TRUE;

ALTER TABLE platform_payment_methods ENABLE ROW LEVEL SECURITY;
ALTER TABLE platform_payment_methods FORCE ROW LEVEL SECURITY;
-- Write access (INSERT/UPDATE/DELETE) stays ADMIN-only. Read access must
-- also allow PATIENT/DOCTOR sessions: `payment_provider_verification`
-- (services/payment_provider_verification.rs) looks up the platform's
-- active provider for a gateway from inside a patient's own
-- save-payment-method request and a doctor's own bind-payout-destination
-- request — confirmed by reading that module before writing this policy,
-- not assumed. This never leaks `credentials_encrypted` to a patient or
-- doctor: that column is excluded from every patient/doctor-facing
-- response at the application layer (`#[serde(skip_serializing)]` on
-- `PlatformPaymentMethod`, and `list_platform_payment_methods`/
-- `platform_payment_method_json` are only ever called from Admin handlers)
-- — this policy only grants row visibility, not a route that returns it.
CREATE POLICY platform_payment_method_admin_write ON platform_payment_methods
    FOR ALL
    USING (current_setting('app.current_role', true) = 'ADMIN')
    WITH CHECK (current_setting('app.current_role', true) = 'ADMIN');
CREATE POLICY platform_payment_method_verification_read ON platform_payment_methods
    FOR SELECT
    USING (current_setting('app.current_role', true) IN ('PATIENT', 'DOCTOR') AND is_active = TRUE);
GRANT SELECT, INSERT, UPDATE, DELETE ON platform_payment_methods TO hellodoctor_api;

-- Records which platform merchant configuration a given saved method/
-- destination was (simulated-)verified against — added here, not in
-- 20261108000001, since platform_payment_methods did not exist yet at that
-- point in migration order.
ALTER TABLE patient_payment_methods ADD COLUMN platform_payment_method_id UUID REFERENCES platform_payment_methods(id);
ALTER TABLE doctor_payout_destinations ADD COLUMN IF NOT EXISTS platform_payment_method_id UUID REFERENCES platform_payment_methods(id);
