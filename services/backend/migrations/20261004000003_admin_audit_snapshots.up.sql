-- Admin change audit trail: before/after snapshots + narrowing global
-- admin-audit-history visibility to a single designated "Super Admin"
-- authority. Closes the two real gaps `docs/CAPACITY_EVIDENCE_REPORT.md`
-- §8.8 found and documented honestly rather than claimed as already passing:
--   1. `audit_events` had no before/after-snapshot capability at all.
--   2. The RLS SELECT policy allowed six roles to read the global trail
--      rather than one designated highest authority.
--
-- Designated authority: SECURITY_ADMIN. This is a judgment call made in this
-- pass (no prior doc pinned down which role "Super Admin" means) — chosen
-- because it is the one role whose entire purpose is independent security/
-- compliance oversight, and because a clean separation of duty (the general
-- operational admin role, PLATFORM_ADMIN, should not also be the sole judge
-- of its own audit trail) is a standard control for this kind of feature.
-- Reversible: narrowing this further, or picking a different role, is a
-- one-line change to the policy below and to `abac::Action::ViewLogs`.
ALTER TABLE audit_events
    ADD COLUMN before_data JSONB,
    ADD COLUMN after_data JSONB,
    ADD COLUMN changed_fields TEXT[];

DROP POLICY audit_events_select ON audit_events;
CREATE POLICY audit_events_select ON audit_events FOR SELECT USING (
    current_setting('app.current_role', true) = 'SECURITY_ADMIN'
    OR current_user <> 'hellodoctor_api'
);
