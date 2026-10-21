-- Drop the referencing columns before the table they reference, or the
-- FK constraint blocks the table drop.
ALTER TABLE doctor_payout_destinations DROP COLUMN IF EXISTS platform_payment_method_id;
ALTER TABLE patient_payment_methods DROP COLUMN IF EXISTS platform_payment_method_id;
DROP TABLE IF EXISTS platform_payment_methods;
