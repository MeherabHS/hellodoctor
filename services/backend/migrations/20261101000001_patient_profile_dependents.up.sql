-- ============================================================================
-- Multi-profile / family support: one authenticated account (one phone
-- number, one login) can create and manage several patient profiles under
-- itself — e.g. a parent creates profiles for a spouse and children, all
-- reachable from the same login, each with its own independent clinical
-- history (consultations, prescriptions, health vault). No separate login
-- for a dependent profile. Confirmed product scope: exactly one owning
-- account per dependent (no shared/multi-guardian profiles), no path for a
-- dependent to "graduate" into its own login.
--
-- `patient_profiles.user_id` was `NOT NULL UNIQUE` (see
-- 20261001000001_initial_schema) — a strict 1:1 with a real login. This
-- migration makes it nullable and adds a self-referencing
-- `guardian_patient_id`: a dependent row has `user_id = NULL` and
-- `guardian_patient_id` pointing at its owning account's own self-profile
-- `id`. Chosen over a separate `patient_dependents` table specifically
-- because every existing ownership check in this schema already keys off
-- `patient_profiles.id` (appointments, intake documents, prescriptions,
-- chat, grievance's trigger-enforced subject consistency) — a
-- dependent profile shaped identically to a self profile means booking,
-- chat, documents, prescriptions, and grievance all work for a dependent
-- with no new `dependent_id` column threaded through every one of those
-- tables, and no duplicated display logic anywhere a patient name already
-- appears.
-- ============================================================================

ALTER TABLE patient_profiles ALTER COLUMN user_id DROP NOT NULL;
ALTER TABLE patient_profiles ADD COLUMN guardian_patient_id UUID
    REFERENCES patient_profiles(id) ON DELETE RESTRICT;
ALTER TABLE patient_profiles ADD COLUMN relationship VARCHAR(30);

-- `hellodoctor_api` had SELECT and UPDATE on `patient_profiles`
-- (20261003000001_rls_hardening) but never INSERT — the only prior insert
-- path was `hellodoctor_auth_worker`'s own self-registration bootstrap.
-- Creating a dependent profile is a genuinely new insert path for
-- `hellodoctor_api`, gated by the RLS policies below, not by this GRANT
-- alone.
GRANT INSERT ON patient_profiles TO hellodoctor_api;

-- `guard_profile_identity()` (20261026000001_canonical_user_isolation) always
-- required `patient_profiles.user_id` to resolve to a real `users` row with
-- `role='PATIENT'` — correct for a self profile, but a dependent row's
-- `user_id` is NULL by design (no login of its own), so the unconditional
-- `PERFORM ... WHERE id=NEW.user_id ...` always failed with "Profile must
-- match canonical account role" for every dependent insert/update, caught
-- directly (not just reasoned about) by this migration's own integration
-- tests. The profile-identity-immutability check (NEW.id/user_id vs OLD)
-- still runs unconditionally for every row, dependent or not — only the
-- role-lookup is skipped when `user_id IS NULL`, which
-- `patient_profiles_self_xor_dependent` already guarantees means a genuine
-- dependent (a self profile can never have a null `user_id`).
CREATE OR REPLACE FUNCTION guard_profile_identity() RETURNS trigger LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,public AS $$
DECLARE required_role text;
BEGIN
 IF TG_TABLE_NAME='users' THEN
  IF NEW.role IS DISTINCT FROM OLD.role AND
     (EXISTS(SELECT 1 FROM public.patient_profiles WHERE user_id=NEW.id) OR
      EXISTS(SELECT 1 FROM public.doctor_profiles WHERE user_id=NEW.id)) THEN
   RAISE EXCEPTION 'Profile-bearing account role is immutable' USING ERRCODE='23514';
  END IF;
 ELSE
  IF TG_OP='UPDATE' AND (NEW.user_id IS DISTINCT FROM OLD.user_id OR NEW.id IS DISTINCT FROM OLD.id) THEN
   RAISE EXCEPTION 'Canonical profile identity is immutable' USING ERRCODE='23514';
  END IF;
  IF TG_TABLE_NAME='patient_profiles' AND NEW.user_id IS NULL THEN
   RETURN NEW;
  END IF;
  required_role := CASE TG_TABLE_NAME WHEN 'patient_profiles' THEN 'PATIENT' ELSE 'DOCTOR' END;
  PERFORM 1 FROM public.users WHERE id=NEW.user_id AND role::text=required_role FOR SHARE;
  IF NOT FOUND THEN RAISE EXCEPTION 'Profile must match canonical account role' USING ERRCODE='23514'; END IF;
 END IF;
 RETURN NEW;
END $$;
-- A profile is always exactly one of: a real login-linked self profile, or a
-- guardian-managed dependent — never both, never neither. Enforced at the
-- database level, not just by application code.
ALTER TABLE patient_profiles ADD CONSTRAINT patient_profiles_self_xor_dependent CHECK (
    (user_id IS NOT NULL AND guardian_patient_id IS NULL)
    OR (user_id IS NULL AND guardian_patient_id IS NOT NULL)
);
CREATE INDEX idx_patient_profiles_guardian ON patient_profiles(guardian_patient_id)
    WHERE guardian_patient_id IS NOT NULL;

-- Blocks nested dependents (a dependent profile can never itself guardian
-- another dependent) — a database-level backstop alongside the application
-- check in `patient_handlers::create_dependent_profile`, matching this
-- codebase's existing defense-in-depth trigger pattern
-- (`guard_grievance_subject`, 20261026000001_canonical_user_isolation).
CREATE OR REPLACE FUNCTION guard_single_level_dependents()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    IF NEW.guardian_patient_id IS NOT NULL THEN
        IF EXISTS (
            SELECT 1 FROM patient_profiles
            WHERE id = NEW.guardian_patient_id AND guardian_patient_id IS NOT NULL
        ) THEN
            RAISE EXCEPTION 'A dependent profile cannot itself guardian another dependent.';
        END IF;
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER patient_profiles_single_level_dependents
    BEFORE INSERT OR UPDATE OF guardian_patient_id ON patient_profiles
    FOR EACH ROW EXECUTE FUNCTION guard_single_level_dependents();

-- ----------------------------------------------------------------------------
-- RLS helper functions. SECURITY DEFINER + STABLE, matching
-- `fn_doctor_treats_patient`'s existing precedent exactly (bypasses RLS on
-- its own internal lookup rather than recursing back through the policy
-- that calls it).
-- ----------------------------------------------------------------------------

-- True if `p_patient_profile_id` is `p_requesting_user_id`'s own self profile.
CREATE OR REPLACE FUNCTION fn_patient_profile_is_self(p_patient_profile_id uuid, p_requesting_user_id uuid)
RETURNS boolean
LANGUAGE sql
SECURITY DEFINER
SET search_path = public
STABLE
AS $$
    SELECT EXISTS (
        SELECT 1 FROM patient_profiles
        WHERE id = p_patient_profile_id AND user_id = p_requesting_user_id
    );
$$;

-- True if `p_patient_profile_id` belongs to `p_requesting_user_id` — either
-- directly (their own self profile) or as a dependent they guardian. The
-- single replacement for every `p.user_id = current_user_id` ownership check
-- throughout this schema's RLS policies.
CREATE OR REPLACE FUNCTION fn_patient_profile_owned_by_caller(p_patient_profile_id uuid, p_requesting_user_id uuid)
RETURNS boolean
LANGUAGE sql
SECURITY DEFINER
SET search_path = public
STABLE
AS $$
    SELECT EXISTS (
        SELECT 1 FROM patient_profiles pp
        WHERE pp.id = p_patient_profile_id
          AND (pp.user_id = p_requesting_user_id
               OR fn_patient_profile_is_self(pp.guardian_patient_id, p_requesting_user_id))
    );
$$;

-- ----------------------------------------------------------------------------
-- patient_profiles — SELECT/UPDATE/INSERT(-of-dependent) all flow through
-- the same permissive policy's USING clause (a policy with no explicit
-- WITH CHECK uses USING for both, per Postgres RLS semantics), so this one
-- edit additively covers: seeing a dependent's own row, and a guardian
-- inserting a new dependent row (whose own `guardian_patient_id` column is
-- checked directly here, not via the id-lookup helper — the row doesn't
-- exist yet for `fn_patient_profile_owned_by_caller` to find during INSERT).
-- ----------------------------------------------------------------------------
DROP POLICY patient_profiles_access ON patient_profiles;
CREATE POLICY patient_profiles_access ON patient_profiles
USING (
    current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'COMPLIANCE', 'SUPPORT')
    OR user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
    OR fn_patient_profile_is_self(guardian_patient_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    OR fn_doctor_treats_patient(patient_profiles.id, NULLIF(current_setting('app.current_user_id', true), '')::uuid)
);

-- Explicit defense-in-depth author guard for a PATIENT-role caller creating a
-- dependent, matching the existing RESTRICTIVE-policy style used for every
-- other patient-initiated insert in this schema EXACTLY
-- (`booking_patient_author`, `grievance_patient_author`,
-- `intake_document_author_guard`): unconditionally requires `role='PATIENT'`,
-- with no admin bypass — nothing today performs an admin-initiated
-- `patient_profiles` INSERT via the `hellodoctor_api` role (the only real
-- insert paths are `hellodoctor_auth_worker` during OTP registration, which
-- this policy does not apply to at all, and this new dependent-creation
-- endpoint), so this intentionally narrows INSERT to the patient-authored
-- case only, matching its siblings' own stated purpose.
CREATE POLICY dependent_profile_author_guard ON patient_profiles AS RESTRICTIVE FOR INSERT TO hellodoctor_api WITH CHECK (
    current_setting('app.current_role', true) = 'PATIENT'
    AND user_id IS NULL
    AND fn_patient_profile_is_self(guardian_patient_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid)
);

DROP POLICY patient_profile_self_update ON patient_profiles;
CREATE POLICY patient_profile_self_update ON patient_profiles AS RESTRICTIVE FOR UPDATE TO hellodoctor_api USING (
    user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
    OR fn_patient_profile_is_self(guardian_patient_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    OR current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'CLINICAL_ADMIN')
) WITH CHECK (
    user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
    OR fn_patient_profile_is_self(guardian_patient_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    OR current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'CLINICAL_ADMIN')
);

-- ----------------------------------------------------------------------------
-- Every other table's ownership check: replace the bare
-- `p.user_id = current_user_id` sub-condition with
-- `fn_patient_profile_owned_by_caller(p.id, current_user_id)` — additive,
-- same shape, now also true for a guardian acting on a dependent's row.
-- ----------------------------------------------------------------------------

DROP POLICY appointments_access ON appointments;
CREATE POLICY appointments_access ON appointments
USING (
    current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'FINANCE_ADMIN', 'SUPPORT', 'COMPLIANCE')
    OR fn_patient_profile_owned_by_caller(appointments.patient_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    OR EXISTS (SELECT 1 FROM doctor_profiles d WHERE d.id = appointments.doctor_id AND d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
);

DROP POLICY booking_patient_author ON appointments;
CREATE POLICY booking_patient_author ON appointments AS RESTRICTIVE FOR INSERT TO hellodoctor_api WITH CHECK (
    current_setting('app.current_role', true) = 'PATIENT'
    AND fn_patient_profile_owned_by_caller(patient_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid)
);

-- CAUGHT BY THIS MIGRATION'S OWN TEST RUN, NOT BY INSPECTION: an earlier
-- draft of this edit was based on this policy's ORIGINAL text
-- (20261003000001_rls_hardening), not its actual current text — missing
-- that 20261017000001_clinical_and_payout_invariants had already replaced
-- the bare `d.user_id = current_user_id` doctor branch with the real
-- treatment-context-window check `doctor_history_context_allowed(...)` and
-- narrowed the admin role list (dropping FINANCE_ADMIN/SUPPORT from clinical
-- PHI access). The draft would have silently reverted that security fix,
-- letting an assigned doctor read clinical intake PHI outside their approved
-- window again. Fixed by only touching the patient branch, exactly as this
-- migration does for every sibling table.
DROP POLICY appointment_clinical_intake_access ON appointment_clinical_intake;
CREATE POLICY appointment_clinical_intake_access ON appointment_clinical_intake
USING (
    current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'COMPLIANCE')
    OR EXISTS (
        SELECT 1 FROM appointments a
        WHERE a.id = appointment_clinical_intake.appointment_id
          AND fn_patient_profile_owned_by_caller(a.patient_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    )
    OR doctor_history_context_allowed(appointment_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid)
);

-- Role list intentionally ('PLATFORM_ADMIN','CLINICAL_ADMIN','COMPLIANCE')
-- here, NOT the broader list `20261003000001` originally used —
-- `20261014000001_rx_pages_document_authorization` already narrowed this
-- (dropping FINANCE_ADMIN/SUPPORT from clinical PHI access), caught the same
-- way as `appointment_clinical_intake_access` above: by reading this
-- policy's actual current text before editing it, not its original.
DROP POLICY prescription_intake_documents_access ON prescription_intake_documents;
CREATE POLICY prescription_intake_documents_access ON prescription_intake_documents
USING (
    current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'COMPLIANCE')
    OR EXISTS (
        SELECT 1 FROM appointments a
        LEFT JOIN doctor_profiles d ON d.id = a.doctor_id
        WHERE a.id = prescription_intake_documents.appointment_id
          AND (fn_patient_profile_owned_by_caller(a.patient_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid)
               OR d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    )
);

DROP POLICY intake_document_author_guard ON prescription_intake_documents;
CREATE POLICY intake_document_author_guard ON prescription_intake_documents AS RESTRICTIVE FOR INSERT TO hellodoctor_api WITH CHECK (
    current_setting('app.current_role', true) = 'PATIENT'
    AND fn_patient_profile_owned_by_caller(patient_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid)
);

DROP POLICY consultation_sessions_access ON consultation_sessions;
CREATE POLICY consultation_sessions_access ON consultation_sessions
USING (
    current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'COMPLIANCE', 'FINANCE_ADMIN', 'SUPPORT')
    OR EXISTS (
        SELECT 1 FROM appointments a
        LEFT JOIN doctor_profiles d ON d.id = a.doctor_id
        WHERE a.id = consultation_sessions.appointment_id
          AND (fn_patient_profile_owned_by_caller(a.patient_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid)
               OR d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    )
);

-- Same narrower role list as `prescription_intake_documents_access` above,
-- for the same reason (`20261014000001` already tightened this one too).
DROP POLICY prescriptions_access ON prescriptions;
CREATE POLICY prescriptions_access ON prescriptions
USING (
    current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'COMPLIANCE')
    OR EXISTS (
        SELECT 1 FROM appointments a
        LEFT JOIN doctor_profiles d ON d.id = a.doctor_id
        WHERE a.id = prescriptions.appointment_id
          AND (fn_patient_profile_owned_by_caller(a.patient_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid)
               OR d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    )
);

DROP POLICY consultation_telemetry_access ON consultation_telemetry;
CREATE POLICY consultation_telemetry_access ON consultation_telemetry
USING (
    current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'COMPLIANCE', 'FINANCE_ADMIN', 'SUPPORT')
    OR EXISTS (
        SELECT 1 FROM consultation_sessions cs
        JOIN appointments a ON a.id = cs.appointment_id
        LEFT JOIN doctor_profiles d ON d.id = a.doctor_id
        WHERE cs.id = consultation_telemetry.session_id
          AND (fn_patient_profile_owned_by_caller(a.patient_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid)
               OR d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    )
);

DROP POLICY consultation_telemetry_events_access ON consultation_telemetry_events;
CREATE POLICY consultation_telemetry_events_access ON consultation_telemetry_events
USING (
    current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'COMPLIANCE', 'FINANCE_ADMIN', 'SUPPORT')
    OR EXISTS (
        SELECT 1 FROM consultation_sessions cs
        JOIN appointments a ON a.id = cs.appointment_id
        LEFT JOIN doctor_profiles d ON d.id = a.doctor_id
        WHERE cs.id = consultation_telemetry_events.session_id
          AND (fn_patient_profile_owned_by_caller(a.patient_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid)
               OR d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    )
);

DROP POLICY chat_conversations_access ON chat_conversations;
CREATE POLICY chat_conversations_access ON chat_conversations
USING (
    current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'COMPLIANCE', 'FINANCE_ADMIN', 'SUPPORT')
    OR fn_patient_profile_owned_by_caller(chat_conversations.patient_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    OR EXISTS (SELECT 1 FROM doctor_profiles d WHERE d.id = chat_conversations.doctor_id AND d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
);

DROP POLICY chat_messages_access ON chat_messages;
CREATE POLICY chat_messages_access ON chat_messages
USING (
    current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'COMPLIANCE', 'FINANCE_ADMIN', 'SUPPORT')
    OR EXISTS (
        SELECT 1 FROM chat_conversations c
        LEFT JOIN doctor_profiles d ON d.id = c.doctor_id
        WHERE c.id = chat_messages.conversation_id
          AND (fn_patient_profile_owned_by_caller(c.patient_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid)
               OR d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    )
);

DROP POLICY chat_messages_insert ON chat_messages;
CREATE POLICY chat_messages_insert ON chat_messages
    FOR INSERT TO hellodoctor_api WITH CHECK (
        sender_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
        AND conversation_id IN (
            SELECT id FROM chat_conversations WHERE (
                fn_patient_profile_owned_by_caller(patient_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid)
                OR doctor_id IN (SELECT id FROM doctor_profiles WHERE user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
            )
            AND is_locked = FALSE
            AND expires_at > CURRENT_TIMESTAMP
        )
    );

DROP POLICY transactions_access ON transactions;
CREATE POLICY transactions_access ON transactions
USING (
    current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'FINANCE_ADMIN', 'COMPLIANCE')
    OR EXISTS (
        SELECT 1 FROM appointments a
        LEFT JOIN doctor_profiles d ON d.id = a.doctor_id
        WHERE a.id = transactions.appointment_id
          AND (fn_patient_profile_owned_by_caller(a.patient_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid)
               OR d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    )
);

DROP POLICY payment_events_access ON payment_events;
CREATE POLICY payment_events_access ON payment_events
USING (
    current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'FINANCE_ADMIN', 'COMPLIANCE')
    OR EXISTS (
        SELECT 1 FROM transactions t
        JOIN appointments a ON a.id = t.appointment_id
        LEFT JOIN doctor_profiles d ON d.id = a.doctor_id
        WHERE t.id = payment_events.transaction_id
          AND (fn_patient_profile_owned_by_caller(a.patient_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid)
               OR d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    )
);

DROP POLICY grievance_reports_access ON grievance_reports;
CREATE POLICY grievance_reports_access ON grievance_reports
USING (
    current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'COMPLIANCE')
    OR fn_patient_profile_owned_by_caller(grievance_reports.patient_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    OR EXISTS (
        SELECT 1 FROM consultation_sessions cs
        JOIN appointments a ON a.id = cs.appointment_id
        JOIN doctor_profiles d ON d.id = a.doctor_id
        WHERE cs.id = grievance_reports.consultation_id
          AND d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
    )
);

DROP POLICY grievance_reports_insert ON grievance_reports;
CREATE POLICY grievance_reports_insert ON grievance_reports
    FOR INSERT TO hellodoctor_api WITH CHECK (
        fn_patient_profile_owned_by_caller(patient_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    );

DROP POLICY grievance_patient_author ON grievance_reports;
CREATE POLICY grievance_patient_author ON grievance_reports AS RESTRICTIVE FOR INSERT TO hellodoctor_api WITH CHECK (
    current_setting('app.current_role', true) = 'PATIENT'
    AND fn_patient_profile_owned_by_caller(patient_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid)
);

DROP POLICY grievance_adjudications_access ON grievance_adjudications;
CREATE POLICY grievance_adjudications_access ON grievance_adjudications
USING (
    current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'COMPLIANCE')
    OR EXISTS (
        SELECT 1 FROM grievance_reports g
        LEFT JOIN consultation_sessions cs ON cs.id = g.consultation_id
        LEFT JOIN appointments a ON a.id = cs.appointment_id
        LEFT JOIN doctor_profiles d ON d.id = a.doctor_id
        WHERE g.id = grievance_adjudications.grievance_id
          AND (fn_patient_profile_owned_by_caller(g.patient_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid)
               OR d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    )
);
