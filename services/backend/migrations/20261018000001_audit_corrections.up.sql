-- Corrections from the 2026-10-07 four-agent Doctor system audit
-- (docs/DOCTOR_SYSTEM_FINAL_MULTI_AGENT_AUDIT.md).

-- GAP-C03: an approved refund had no resolution path at all — `status`
-- could never move past 'APPROVED' because INSERT was the only privilege
-- ever granted. Column-scoped (not blanket) UPDATE: the money/ownership
-- columns (amount, currency, transaction_id, grievance_id, approved_by,
-- approved_at, previous_payment_status) stay immutable even to Finance.
GRANT UPDATE (status, provider_reference, refunded_at) ON financial_refund_requests TO hellodoctor_finance_worker;

-- GAP-C05: this deployment is Kenya/KES-only; a doctor row inserted by any
-- future path that forgets to bind `license_country` explicitly must not
-- silently default to Bangladesh. Every current INSERT path already binds
-- 'KE' explicitly, so this is a pure defense-in-depth default change with no
-- behavioral effect on existing code.
ALTER TABLE doctor_profiles ALTER COLUMN license_country SET DEFAULT 'KE';
