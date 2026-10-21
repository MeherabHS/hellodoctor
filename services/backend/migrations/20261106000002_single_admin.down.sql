DROP TRIGGER canonical_admin_assignment ON users;
DROP FUNCTION reject_active_legacy_admin_role();
DO $$ DECLARE r RECORD; BEGIN
 FOR r IN SELECT * FROM admin_policy_rollback WHERE kind='POLICY' LOOP
  EXECUTE 'DROP POLICY '||r.identity; EXECUTE r.definition;
 END LOOP;
 FOR r IN SELECT * FROM admin_policy_rollback WHERE kind='FUNCTION' LOOP EXECUTE r.definition; END LOOP;
END $$;
UPDATE users u SET role=r.legacy_role FROM legacy_admin_role_assignments r WHERE u.id=r.user_id;
UPDATE users SET role='PLATFORM_ADMIN' WHERE role='ADMIN';
DROP TABLE admin_policy_rollback;
DROP TABLE legacy_admin_role_assignments;
