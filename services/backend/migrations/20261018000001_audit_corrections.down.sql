ALTER TABLE doctor_profiles ALTER COLUMN license_country SET DEFAULT 'BD';
REVOKE UPDATE (status, provider_reference, refunded_at) ON financial_refund_requests FROM hellodoctor_finance_worker;
