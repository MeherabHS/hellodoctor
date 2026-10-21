-- ============================================================================
-- Gap #6 (plan `enumerated-wishing-pearl.md` §4/§6): a patient has no way to
-- submit a Free Health Q&A question. `20261005000001_doctor_contract_qna`
-- created `health_questions`/`health_answers` with a public `SELECT USING
-- (true)` read policy and `GRANT UPDATE` for the Doctor answer-flow, but
-- never granted INSERT and never added an INSERT policy at all — confirmed
-- directly by reading that migration, not inferred. This additively closes
-- that gap using the same guardian-OR-self ownership shape
-- `20261101000001_patient_profile_dependents` already established for every
-- other patient-authored table (`fn_patient_profile_owned_by_caller`), so a
-- guardian can ask a question on behalf of a dependent profile exactly like
-- booking, intake uploads, and grievances already allow.
--
-- `20261026000001_canonical_user_isolation` already revoked full SELECT and
-- re-granted only a column-scoped one excluding `author_patient_id` ("Preserve
-- account-originated anonymous Q&A linkage internally, not in Doctor DTOs") —
-- confirming the product's anonymous-to-doctors design is enforced at the
-- grant level. INSERT is not column-scoped here: writing `author_patient_id`
-- doesn't leak it to anyone — only a later SELECT could, and that grant is
-- untouched by this migration.
-- ============================================================================

GRANT INSERT ON health_questions TO hellodoctor_api;

-- Permissive: a caller (self or guardian) may insert a question authored as
-- any patient profile they own.
CREATE POLICY health_questions_patient_insert ON health_questions
    FOR INSERT TO hellodoctor_api WITH CHECK (
        fn_patient_profile_owned_by_caller(author_patient_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    );

-- Restrictive defense-in-depth author guard, matching `grievance_patient_author`
-- / `booking_patient_author` / `intake_document_author_guard` exactly: narrows
-- the permissive policy above to PATIENT-role callers inserting a genuinely
-- new OPEN question (never pre-seeding ANSWERED/CLOSED through this path).
CREATE POLICY health_questions_patient_author_guard ON health_questions AS RESTRICTIVE FOR INSERT TO hellodoctor_api WITH CHECK (
    current_setting('app.current_role', true) = 'PATIENT'
    AND status = 'OPEN'
);
