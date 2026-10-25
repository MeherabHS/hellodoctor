-- Split into its own migration, deliberately: PostgreSQL forbids using a
-- newly added enum value anywhere that requires immediate evaluation within
-- the SAME transaction it was added in (a CHECK constraint or a partial
-- index's WHERE predicate, specifically — confirmed by hitting
-- "unsafe use of new value ... New enum values must be committed before they
-- can be used" while developing this migration, not assumed). A
-- `CREATE OR REPLACE FUNCTION` body referencing these values as string
-- literals (like `20261107000002_doctor_verification_revocation` does) is
-- fine in the same transaction, since the body isn't evaluated until a later
-- call — but `20261126000002_campaign_scheduling_and_delivery`'s new CHECK
-- constraints and partial indexes are not, so these four values must commit
-- here first.
ALTER TYPE campaign_status ADD VALUE 'SCHEDULED';
ALTER TYPE campaign_status ADD VALUE 'SENDING';
ALTER TYPE campaign_status ADD VALUE 'PAUSED';
ALTER TYPE campaign_status ADD VALUE 'COMPLETED';
