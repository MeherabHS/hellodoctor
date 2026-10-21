-- h22 remediation: a disqualified-but-formerly-VERIFIED Doctor currently has
-- no code path that ever withdraws clinical capability (CLAUDE.md Sec.2.1).
-- Add a narrow, audited VERIFIED->REVOKED terminal transition alongside the
-- existing PENDING->{VERIFIED,REJECTED} one, reusing the same reviewed_at/
-- reviewed_by/note columns (actor, timestamp, reason) the existing review
-- flow already uses, rather than inventing parallel bookkeeping.
ALTER TYPE professional_verification_status_enum ADD VALUE 'REVOKED';

-- Extend the existing trigger (as rewritten to the canonical 'ADMIN' role by
-- 20261106000002_single_admin's function sweep) with a second authorized
-- transition: VERIFIED->REVOKED, under the identical admin/reviewer/
-- reviewed_at checks the PENDING->terminal path already enforces. Every
-- other original invariant (INSERT guard, role check, reviewer-identity
-- check) is preserved verbatim.
CREATE OR REPLACE FUNCTION guard_professional_verification() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF current_user IN ('hellodoctor_api', 'hellodoctor_auth_worker') THEN
        IF TG_OP = 'INSERT' THEN
            IF NEW.is_verified OR NEW.professional_verification_status <> 'PENDING'
               OR NEW.professional_verification_reviewed_at IS NOT NULL
               OR NEW.professional_verification_reviewed_by IS NOT NULL
               OR NEW.professional_verification_note IS NOT NULL THEN
                RAISE EXCEPTION 'New Doctor registration must await professional review' USING ERRCODE='23514';
            END IF;
            IF NEW.license_authority <> 'KMPDC' OR NEW.license_country <> 'KE' THEN
                RAISE EXCEPTION 'Kenya professional registration requires KMPDC and KE' USING ERRCODE='23514';
            END IF;
        ELSIF NEW.professional_verification_status IS DISTINCT FROM OLD.professional_verification_status
              OR NEW.professional_verification_reviewed_at IS DISTINCT FROM OLD.professional_verification_reviewed_at
              OR NEW.professional_verification_reviewed_by IS DISTINCT FROM OLD.professional_verification_reviewed_by
              OR NEW.professional_verification_note IS DISTINCT FROM OLD.professional_verification_note THEN
            IF current_user <> 'hellodoctor_api'
               OR current_setting('app.current_role', true) NOT IN ('ADMIN')
               OR NULLIF(current_setting('app.current_role', true), '') IS NULL
               OR NOT (
                    (OLD.professional_verification_status = 'PENDING' AND NEW.professional_verification_status IN ('VERIFIED', 'REJECTED'))
                    OR (OLD.professional_verification_status = 'VERIFIED' AND NEW.professional_verification_status = 'REVOKED')
                  )
               OR NEW.professional_verification_reviewed_at IS NULL
               OR NEW.professional_verification_reviewed_by IS DISTINCT FROM NULLIF(current_setting('app.current_user_id', true), '')::uuid THEN
                RAISE EXCEPTION 'Professional review requires an authorized pending-to-terminal or verified-to-revoked transition' USING ERRCODE='23514';
            END IF;
        END IF;
    END IF;
    NEW.is_verified := (NEW.professional_verification_status = 'VERIFIED');
    IF NOT NEW.is_verified THEN NEW.is_on_duty := false; END IF;
    RETURN NEW;
END $$;
