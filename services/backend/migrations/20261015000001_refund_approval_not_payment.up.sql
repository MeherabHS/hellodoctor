ALTER TYPE grievance_status_enum ADD VALUE 'REFUND_APPROVED';
ALTER TYPE payment_status_enum ADD VALUE 'REFUND_PENDING';

CREATE TABLE financial_refund_requests (
    id UUID PRIMARY KEY,
    grievance_id UUID NOT NULL UNIQUE REFERENCES grievance_reports(id),
    transaction_id UUID NOT NULL UNIQUE REFERENCES transactions(id),
    approved_by UUID NOT NULL REFERENCES users(id),
    approved_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    currency CHAR(3) NOT NULL CHECK (currency='KES'),
    amount NUMERIC(12,2) NOT NULL CHECK (amount>0),
    previous_payment_status TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'APPROVED' CHECK (status IN ('APPROVED','PROCESSING','REFUNDED','FAILED','RECONCILIATION_REQUIRED')),
    provider_reference TEXT,
    refunded_at TIMESTAMPTZ,
    CHECK (status <> 'REFUNDED' OR (provider_reference IS NOT NULL AND refunded_at IS NOT NULL))
);
ALTER TABLE financial_refund_requests ENABLE ROW LEVEL SECURITY;
ALTER TABLE financial_refund_requests FORCE ROW LEVEL SECURITY;
CREATE POLICY financial_refund_finance ON financial_refund_requests TO hellodoctor_finance_worker
    USING (current_setting('app.current_role',true) IN ('PLATFORM_ADMIN','FINANCE_ADMIN','CLINICAL_ADMIN'))
    WITH CHECK (current_setting('app.current_role',true) IN ('PLATFORM_ADMIN','FINANCE_ADMIN','CLINICAL_ADMIN'));
GRANT SELECT,INSERT ON financial_refund_requests TO hellodoctor_finance_worker;
-- Provider result processing is deliberately not exposed to an Admin action.
