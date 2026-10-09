-- Financial state machine: preparation/allocation is distinct from payout.
ALTER TABLE transactions
    ADD COLUMN IF NOT EXISTS disbursement_item_id UUID REFERENCES disbursement_items(id);
ALTER TABLE transactions DROP CONSTRAINT IF EXISTS transactions_disbursement_item_id_fkey;
ALTER TABLE transactions ADD CONSTRAINT transactions_disbursement_item_id_fkey
    FOREIGN KEY (disbursement_item_id) REFERENCES disbursement_items(id) DEFERRABLE INITIALLY DEFERRED;
CREATE UNIQUE INDEX IF NOT EXISTS uq_transaction_disbursement_allocation
    ON transactions(disbursement_item_id) WHERE disbursement_item_id IS NOT NULL;

ALTER TABLE disbursement_items ALTER COLUMN status TYPE VARCHAR(40);
ALTER TABLE disbursement_items ADD COLUMN IF NOT EXISTS approved_by UUID REFERENCES users(id);
ALTER TABLE disbursement_items ADD COLUMN IF NOT EXISTS approved_at TIMESTAMPTZ;
ALTER TABLE disbursement_items ADD COLUMN IF NOT EXISTS paid_at TIMESTAMPTZ;
ALTER TABLE disbursement_items ADD COLUMN IF NOT EXISTS reconciliation_state VARCHAR(40) NOT NULL DEFAULT 'NOT_REQUIRED';

CREATE TABLE IF NOT EXISTS financial_periods (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    currency CHAR(3) NOT NULL CHECK (currency = 'KES'),
    period_year SMALLINT NOT NULL,
    period_month SMALLINT NOT NULL CHECK (period_month BETWEEN 1 AND 12),
    status VARCHAR(20) NOT NULL DEFAULT 'OPEN',
    opened_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    finalized_at TIMESTAMPTZ,
    finalized_by UUID REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(currency, period_year, period_month)
);

CREATE TABLE IF NOT EXISTS monthly_payout_statements (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    doctor_id UUID NOT NULL REFERENCES doctor_profiles(id) ON DELETE RESTRICT,
    financial_period_id UUID NOT NULL REFERENCES financial_periods(id) ON DELETE RESTRICT,
    currency CHAR(3) NOT NULL CHECK (currency = 'KES'),
    gross_eligible NUMERIC(12,2) NOT NULL DEFAULT 0 CHECK (gross_eligible >= 0),
    platform_charge NUMERIC(12,2) NOT NULL DEFAULT 0 CHECK (platform_charge >= 0),
    adjustments NUMERIC(12,2) NOT NULL DEFAULT 0,
    final_payable NUMERIC(12,2) NOT NULL DEFAULT 0 CHECK (final_payable >= 0),
    status VARCHAR(30) NOT NULL DEFAULT 'ESTIMATED',
    snapshot_created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(doctor_id, financial_period_id)
);

CREATE TABLE IF NOT EXISTS financial_adjustments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    doctor_id UUID NOT NULL REFERENCES doctor_profiles(id) ON DELETE RESTRICT,
    currency CHAR(3) NOT NULL CHECK (currency = 'KES'),
    period_year SMALLINT NOT NULL,
    period_month SMALLINT NOT NULL CHECK (period_month BETWEEN 1 AND 12),
    amount NUMERIC(12,2) NOT NULL,
    category VARCHAR(40) NOT NULL,
    reason TEXT NOT NULL,
    created_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

ALTER TABLE financial_periods ENABLE ROW LEVEL SECURITY;
ALTER TABLE monthly_payout_statements ENABLE ROW LEVEL SECURITY;
ALTER TABLE financial_adjustments ENABLE ROW LEVEL SECURITY;
CREATE POLICY financial_periods_finance_read ON financial_periods FOR SELECT
    USING (current_setting('app.current_role', true) IN ('PLATFORM_ADMIN','FINANCE_ADMIN','COMPLIANCE'));
CREATE POLICY statements_owner_or_finance ON monthly_payout_statements FOR SELECT
    USING (current_setting('app.current_role', true) IN ('PLATFORM_ADMIN','FINANCE_ADMIN','COMPLIANCE') OR doctor_id IN (SELECT id FROM doctor_profiles WHERE user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid));
CREATE POLICY adjustments_finance_read ON financial_adjustments FOR SELECT
    USING (current_setting('app.current_role', true) IN ('PLATFORM_ADMIN','FINANCE_ADMIN','COMPLIANCE'));
GRANT SELECT ON financial_periods, monthly_payout_statements TO hellodoctor_api;
GRANT SELECT, INSERT, UPDATE ON financial_periods, monthly_payout_statements, financial_adjustments TO hellodoctor_finance_worker;
GRANT SELECT ON doctor_profiles TO hellodoctor_finance_worker;
