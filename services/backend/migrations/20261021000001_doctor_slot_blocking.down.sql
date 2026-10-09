-- PostgreSQL cannot safely remove an enum value in place. Never relabel blocked
-- rows as available/cancelled, or rebuild dependent tables during rollback.
DO $$ BEGIN
    RAISE EXCEPTION 'Doctor slot blocking requires a forward corrective migration; BLOCKED rows must be preserved';
END $$;
