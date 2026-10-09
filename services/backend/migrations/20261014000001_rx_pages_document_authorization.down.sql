-- Restoring the old one-page model would destroy additional prescription pages.
-- This data-preserving migration is intentionally forward-only.
DO $$ BEGIN RAISE EXCEPTION 'Rx page migration is forward-only; restore a reviewed backup to revert'; END $$;
