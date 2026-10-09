-- Restoring weaker authorization and ownership rules is deliberately unsupported.
DO $$ BEGIN RAISE EXCEPTION 'Clinical and financial invariants require a forward corrective migration'; END $$;
