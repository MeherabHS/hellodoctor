-- Admin-executed Data Protection Act 2019 (Kenya) rights: §38 portability
-- (export — already servable with existing SELECT grants, no change needed
-- here) and §40 erasure. Erasure always attempts a true DELETE on the
-- profile row first, inside a savepoint; Postgres's own FK RESTRICT
-- constraints (appointments/grievance_reports/doctor_wallets/etc. all
-- reference patient_profiles/doctor_profiles with ON DELETE RESTRICT or no
-- action, i.e. RESTRICT by default) are the single source of truth for
-- "does this profile have retained clinical, professional, or financial
-- history" — not a hand-maintained list of tables to check. When the DELETE
-- is blocked, the caller rolls back just that savepoint and anonymizes the
-- profile in place instead. Financial records (transactions, disbursements)
-- are never targeted by either path, so the Income Tax Act/VAT Act 2013 §43
-- five-year retention requirement is satisfied by construction.
--
-- Neither path is reachable without this grant: hellodoctor_api already had
-- UPDATE on both tables (migration 20261003000001) but never DELETE.
GRANT DELETE ON patient_profiles, doctor_profiles TO hellodoctor_api;

-- patient_profiles_access (migration 20261101000001) is a permissive,
-- no-FOR-clause policy — it already applies to every command including
-- DELETE, and already allows ADMIN — so DELETE on patient_profiles needs no
-- new policy, only the GRANT above. doctor_profiles has no such catch-all
-- policy (only FOR SELECT/INSERT/UPDATE policies exist for hellodoctor_api,
-- migration 20261026000001), so without this policy, FORCE ROW LEVEL
-- SECURITY would silently deny every DELETE regardless of the GRANT.
CREATE POLICY doctor_private_delete ON doctor_profiles FOR DELETE TO hellodoctor_api USING (
    current_setting('app.current_role', true) = 'ADMIN'
);
