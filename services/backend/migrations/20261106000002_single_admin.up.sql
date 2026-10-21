-- Preserve every historical audit role and each individual human account UUID.
CREATE TABLE legacy_admin_role_assignments (
 user_id UUID PRIMARY KEY REFERENCES users(id), legacy_role user_role_enum NOT NULL
);
INSERT INTO legacy_admin_role_assignments SELECT id,role FROM users
WHERE role::text IN ('PLATFORM_ADMIN','CLINICAL_ADMIN','FINANCE_ADMIN','SUPPORT','COMPLIANCE','SECURITY_ADMIN');
UPDATE users SET role='ADMIN' WHERE id IN (SELECT user_id FROM legacy_admin_role_assignments);
UPDATE auth_sessions SET revoked_at=COALESCE(revoked_at,clock_timestamp())
WHERE user_id IN (SELECT user_id FROM legacy_admin_role_assignments);

-- Save definitions for a reversible change; never rewrite applied migrations.
CREATE TABLE admin_policy_rollback (kind TEXT NOT NULL, identity TEXT NOT NULL, definition TEXT NOT NULL);
DO $$
DECLARE p RECORD; f RECORD; old_sql TEXT; new_sql TEXT; role_name TEXT; role_sql TEXT;
BEGIN
 FOR p IN SELECT * FROM pg_policies WHERE schemaname='public' LOOP
  SELECT string_agg(quote_ident(x),',') INTO role_sql FROM unnest(p.roles) x;
  old_sql:=format('CREATE POLICY %I ON public.%I AS %s FOR %s TO %s%s%s',p.policyname,p.tablename,p.permissive,p.cmd,role_sql,
    CASE WHEN p.qual IS NULL THEN '' ELSE ' USING ('||p.qual||')' END,
    CASE WHEN p.with_check IS NULL THEN '' ELSE ' WITH CHECK ('||p.with_check||')' END);
  new_sql:=old_sql;
  FOREACH role_name IN ARRAY ARRAY['PLATFORM_ADMIN','CLINICAL_ADMIN','FINANCE_ADMIN','SUPPORT','COMPLIANCE','SECURITY_ADMIN'] LOOP
   new_sql:=replace(new_sql,quote_literal(role_name),quote_literal('ADMIN'));
  END LOOP;
  IF new_sql<>old_sql THEN
   INSERT INTO admin_policy_rollback VALUES ('POLICY',format('%I ON public.%I',p.policyname,p.tablename),old_sql);
   EXECUTE format('DROP POLICY %I ON public.%I',p.policyname,p.tablename); EXECUTE new_sql;
  END IF;
 END LOOP;
 FOR f IN SELECT proc.oid FROM pg_proc proc JOIN pg_namespace n ON n.oid=proc.pronamespace
 WHERE n.nspname='public' AND proc.prokind='f' AND proc.prolang IN (SELECT oid FROM pg_language WHERE lanname IN ('sql','plpgsql')) LOOP
  old_sql:=pg_get_functiondef(f.oid); new_sql:=old_sql;
  FOREACH role_name IN ARRAY ARRAY['PLATFORM_ADMIN','CLINICAL_ADMIN','FINANCE_ADMIN','SUPPORT','COMPLIANCE','SECURITY_ADMIN'] LOOP
   new_sql:=replace(new_sql,quote_literal(role_name),quote_literal('ADMIN'));
  END LOOP;
  IF new_sql<>old_sql THEN
   INSERT INTO admin_policy_rollback VALUES ('FUNCTION',f.oid::regprocedure::text,old_sql); EXECUTE new_sql;
  END IF;
 END LOOP;
END $$;
CREATE FUNCTION reject_active_legacy_admin_role() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NEW.role::text IN ('PLATFORM_ADMIN','CLINICAL_ADMIN','FINANCE_ADMIN','SUPPORT','COMPLIANCE','SECURITY_ADMIN') THEN
  RAISE EXCEPTION 'Legacy human Admin roles are historical only; assign ADMIN' USING ERRCODE='23514';
 END IF; RETURN NEW;
END $$;
CREATE TRIGGER canonical_admin_assignment BEFORE INSERT OR UPDATE OF role ON users
FOR EACH ROW EXECUTE FUNCTION reject_active_legacy_admin_role();
