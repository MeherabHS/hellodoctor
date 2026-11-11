-- Financial module deferment: lets FINANCIAL_MODULE_MODE=deferred book an
-- appointment straight to CONFIRMED with no payment transaction at all,
-- while every existing payment/earnings/settlement code path remains fully
-- intact for FINANCIAL_MODULE_MODE=enabled (the default, unchanged
-- behavior). Recorded once per appointment at booking time, from whichever
-- mode was in effect *then* -- flipping the server-wide mode later never
-- retroactively changes what an already-booked appointment owes.
CREATE TYPE payment_policy_enum AS ENUM ('REQUIRED', 'NOT_REQUIRED');

ALTER TABLE appointments
    ADD COLUMN payment_policy payment_policy_enum NOT NULL DEFAULT 'REQUIRED';

-- Every appointment that exists before this migration was booked under the
-- only policy that has ever existed so far -- make that explicit rather
-- than relying on the column default to describe history it didn't create.
UPDATE appointments SET payment_policy = 'REQUIRED';
