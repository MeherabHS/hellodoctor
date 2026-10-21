-- PostgreSQL enum values cannot safely be removed while historical audit uses them.
-- The subsequent role-migration down migration restores account assignments.
SELECT 1;
