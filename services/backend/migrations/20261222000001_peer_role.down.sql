-- PostgreSQL enum values cannot be removed while any row or historical audit
-- may use them (see test_enum_add_value_migration_reversibility_limitation_pattern).
-- The peer tables' own down-migration removes every peer profile; the 'PEER'
-- enum value itself is left in place, as with 20261106000001's 'ADMIN'.
SELECT 1;
