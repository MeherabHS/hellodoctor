-- ============================================================================
-- Row-Level Security + five-role capability hardening.
--
-- Supersedes the earlier single-role draft of this migration (never executed,
-- never shipped — replaced cleanly in place rather than layering a corrective
-- migration on top of unused SQL, per CLAUDE.md migration discipline).
--
-- Five NOLOGIN-at-creation capability roles replace one generic app role, so a
-- bug or compromise in one operational domain cannot reach another:
--   hellodoctor_api             ordinary patient/doctor/admin request handling
--   hellodoctor_auth_worker     auth_sessions / mfa_credentials / OTP / login bootstrap
--   hellodoctor_payment_worker  booking + payment-state transitions
--   hellodoctor_system_worker   background lifecycle jobs (slot/chat/consultation expiry)
--   hellodoctor_finance_worker  grievance adjudication, wallet settlement, disbursement
--
-- Organizing invariant (already stated in docs/AUTH_SECURITY.md §12.3, carried
-- forward unchanged): "The ordinary API role cannot update payment, appointment,
-- wallet, grievance, or chat [conversation] lifecycle state." Every GRANT below
-- follows from that sentence: hellodoctor_api gets read access and ordinary
-- patient/doctor-authored content writes; every *lifecycle state transition* on
-- appointments/transactions/doctor_wallets/grievance_reports/chat_conversations/
-- disbursement_* is granted only to the worker role whose job that transition is.
--
-- None of the five roles is LOGIN at creation time, has BYPASSRLS, owns any
-- table, or is SUPERUSER. LOGIN + password are granted by the application at
-- startup via a parameterized `ALTER ROLE ... LOGIN PASSWORD $1` using secrets
-- read from the environment (see main.rs::bootstrap_capability_role_logins) —
-- never embedded in this committed SQL file.
--
-- Deliberately NOT under RLS here, with reasons (unchanged from the prior
-- design, re-verified against the five-role grant model below):
--   - users, auth_sessions, mfa_credentials, phone_otps, pending_mfa_challenges:
--     the login/MFA bootstrap flow itself, which runs before any caller identity
--     exists. Only hellodoctor_auth_worker is ever granted anything on these
--     tables, so the GRANT boundary alone is the control — no other role can
--     reach them regardless of RLS.
--   - doctor_profiles: intentionally public directory data (any patient can
--     browse any doctor's profile by design).
--   - system_error_logs, idempotency_records: internal plumbing, gated by ABAC
--     (Action::ViewLogs) at the application layer, never ownership-filtered
--     back to a specific patient/doctor through any endpoint.
-- ============================================================================

DO $$
BEGIN
    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'hellodoctor_api') THEN
        CREATE ROLE hellodoctor_api NOLOGIN NOBYPASSRLS;
    END IF;
    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'hellodoctor_auth_worker') THEN
        CREATE ROLE hellodoctor_auth_worker NOLOGIN NOBYPASSRLS;
    END IF;
    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'hellodoctor_payment_worker') THEN
        CREATE ROLE hellodoctor_payment_worker NOLOGIN NOBYPASSRLS;
    END IF;
    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'hellodoctor_system_worker') THEN
        CREATE ROLE hellodoctor_system_worker NOLOGIN NOBYPASSRLS;
    END IF;
    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'hellodoctor_finance_worker') THEN
        CREATE ROLE hellodoctor_finance_worker NOLOGIN NOBYPASSRLS;
    END IF;
END
$$;

GRANT CONNECT ON DATABASE hellodoctor TO
    hellodoctor_api, hellodoctor_auth_worker, hellodoctor_payment_worker,
    hellodoctor_system_worker, hellodoctor_finance_worker;
GRANT USAGE ON SCHEMA public TO
    hellodoctor_api, hellodoctor_auth_worker, hellodoctor_payment_worker,
    hellodoctor_system_worker, hellodoctor_finance_worker;

-- ----------------------------------------------------------------------------
-- hellodoctor_api — ordinary authenticated request handling. Read access is
-- broad (every table an authenticated patient/doctor/admin might legitimately
-- see, RLS-filtered below); write access is limited to patient/doctor-authored
-- content and profile self-edits. No lifecycle-state writes (see header).
-- ----------------------------------------------------------------------------
GRANT SELECT ON
    users, patient_profiles, doctor_profiles, doctor_schedule_slots,
    appointments, appointment_clinical_intake, prescriptions,
    prescription_intake_documents, consultation_sessions, consultation_telemetry,
    chat_conversations, chat_messages, grievance_reports, grievance_adjudications,
    transactions, doctor_wallets, disbursement_batches, disbursement_items,
    payment_events, system_error_logs, audit_events, idempotency_records
TO hellodoctor_api;

GRANT INSERT ON
    doctor_profiles, doctor_schedule_slots, appointment_clinical_intake,
    prescriptions, prescription_intake_documents, chat_messages,
    consultation_sessions, consultation_telemetry, grievance_reports,
    audit_events, idempotency_records,
    -- One-time bootstrap row alongside doctor_profiles creation (admin "create
    -- doctor" flow), not a settlement write — ongoing wallet UPDATEs remain
    -- finance-worker-only.
    doctor_wallets
TO hellodoctor_api;

GRANT UPDATE ON
    patient_profiles, doctor_profiles, doctor_schedule_slots,
    appointment_clinical_intake, consultation_sessions, consultation_telemetry,
    chat_messages
TO hellodoctor_api;

-- ----------------------------------------------------------------------------
-- hellodoctor_auth_worker — login/MFA/OTP bootstrap, plus the one patient
-- self-registration insert that happens at the end of first-time OTP verify
-- (see repository/mod.rs::begin_authed usage in auth_service.rs).
-- ----------------------------------------------------------------------------
GRANT SELECT, INSERT, UPDATE ON
    users, auth_sessions, mfa_credentials
TO hellodoctor_auth_worker;
-- phone_otps additionally needs DELETE: a consumed or expired OTP is deleted
-- outright (otp_repo::delete), not just marked.
GRANT SELECT, INSERT, UPDATE, DELETE ON phone_otps TO hellodoctor_auth_worker;
GRANT SELECT, INSERT, UPDATE, DELETE ON pending_mfa_challenges TO hellodoctor_auth_worker;
GRANT INSERT, SELECT ON patient_profiles TO hellodoctor_auth_worker;
-- Doctor login (step 1) looks doctors up by license number to find the
-- associated user_id; doctor_profiles is public directory data (no RLS), so
-- this is a plain read grant, not a privilege escalation.
GRANT SELECT ON doctor_profiles TO hellodoctor_auth_worker;

-- ----------------------------------------------------------------------------
-- hellodoctor_payment_worker — booking + payment-state transitions only.
-- ----------------------------------------------------------------------------
GRANT SELECT, UPDATE ON appointments, doctor_schedule_slots, transactions TO hellodoctor_payment_worker;
GRANT INSERT ON appointments, appointment_clinical_intake, transactions, payment_events TO hellodoctor_payment_worker;
GRANT SELECT ON payment_events TO hellodoctor_payment_worker;

-- ----------------------------------------------------------------------------
-- hellodoctor_system_worker — background lifecycle jobs: slot expiry, chat
-- conversation locking at the 24h window, consultation session state.
-- ----------------------------------------------------------------------------
GRANT SELECT, UPDATE ON
    appointments, doctor_schedule_slots, chat_conversations, consultation_sessions
TO hellodoctor_system_worker;
GRANT INSERT ON chat_conversations TO hellodoctor_system_worker;
GRANT SELECT, INSERT ON system_error_logs TO hellodoctor_system_worker;

-- ----------------------------------------------------------------------------
-- hellodoctor_finance_worker — grievance adjudication, wallet settlement,
-- disbursement batches. This is the only role ever granted anything on
-- grievance_adjudications/disbursement_batches, so even a non-financial
-- adjudication (warn/dismiss) runs through this connection — matching
-- docs/DATABASE_SCHEMA.md's role-grant design, which grants that table only
-- to the finance-worker role.
-- ----------------------------------------------------------------------------
GRANT SELECT, UPDATE ON
    grievance_reports, transactions, doctor_wallets, disbursement_batches, disbursement_items
TO hellodoctor_finance_worker;
GRANT INSERT ON
    grievance_adjudications, disbursement_batches, disbursement_items, payment_events,
    -- wallet_repo::credit is an INSERT ... ON CONFLICT DO UPDATE upsert (first
    -- settlement for a doctor has no existing row yet), so finance-worker
    -- needs INSERT here too, not just UPDATE.
    doctor_wallets
TO hellodoctor_finance_worker;
GRANT SELECT ON payment_events TO hellodoctor_finance_worker;
-- Refund adjudication reads the grievance's consultation_sessions row to find
-- the appointment, and from there the transaction to refund.
GRANT SELECT ON consultation_sessions TO hellodoctor_finance_worker;
-- Monthly disbursement batching joins settled transactions against
-- appointments to find each transaction's doctor_id.
GRANT SELECT ON appointments TO hellodoctor_finance_worker;

-- ============================================================================
-- Row-Level Security policies.
--
-- Pattern: a plain (no `TO`) policy applies to every role, so it is used for
-- the human-facing ownership/admin-visibility rule that only hellodoctor_api
-- connections ever actually exercise (worker roles have no `app.current_user_id`
-- set and simply fail that USING clause, which is fine — they rely on their own
-- `TO <worker_role>` policy instead, permissive policies are OR'd together).
-- A `TO <worker_role>` policy with `USING (true)` grants that one worker
-- unconditional row access *within the columns/table its GRANT already allows*
-- — the privilege boundary is the GRANT, RLS here just satisfies FORCE RLS's
-- requirement that every role have an explicit policy, including workers.
-- ============================================================================

-- ----------------------------------------------------------------------------
-- patient_profiles
-- ----------------------------------------------------------------------------
-- patient_profiles_access (below) needs to know whether the requesting doctor
-- has ever had an appointment with this patient — but appointments is itself
-- FORCE RLS, and appointments_access correlates back against patient_profiles.
-- A direct correlated subquery here would make Postgres evaluate
-- appointments_access while evaluating patient_profiles_access, which
-- re-evaluates patient_profiles_access, forever ("infinite recursion detected
-- in policy for relation patient_profiles" — hit and confirmed by actually
-- running this migration, not a hypothetical). A SECURITY DEFINER function
-- breaks the cycle: it executes as its owner (the superuser running this
-- migration), who always bypasses RLS regardless of FORCE, so the inner
-- lookup never re-enters appointments_access.
CREATE OR REPLACE FUNCTION fn_doctor_treats_patient(p_patient_profile_id uuid, p_requesting_user_id uuid)
RETURNS boolean
LANGUAGE sql
SECURITY DEFINER
SET search_path = public
STABLE
AS $$
    SELECT EXISTS (
        SELECT 1 FROM appointments a
        JOIN doctor_profiles d ON d.id = a.doctor_id
        WHERE a.patient_id = p_patient_profile_id
          AND d.user_id = p_requesting_user_id
    );
$$;

ALTER TABLE patient_profiles ENABLE ROW LEVEL SECURITY;
ALTER TABLE patient_profiles FORCE ROW LEVEL SECURITY;

CREATE POLICY patient_profiles_access ON patient_profiles
USING (
    current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'COMPLIANCE', 'SUPPORT')
    OR user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
    OR fn_doctor_treats_patient(patient_profiles.id, NULLIF(current_setting('app.current_user_id', true), '')::uuid)
);

CREATE POLICY patient_profiles_auth_worker_insert ON patient_profiles
    FOR INSERT TO hellodoctor_auth_worker WITH CHECK (true);

-- ----------------------------------------------------------------------------
-- doctor_schedule_slots — public read (directory), ownership/worker write.
-- ----------------------------------------------------------------------------
ALTER TABLE doctor_schedule_slots ENABLE ROW LEVEL SECURITY;
ALTER TABLE doctor_schedule_slots FORCE ROW LEVEL SECURITY;

CREATE POLICY slots_select ON doctor_schedule_slots FOR SELECT USING (true);

CREATE POLICY slots_api_write ON doctor_schedule_slots
    FOR ALL TO hellodoctor_api USING (
        current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'SUPPORT')
        OR doctor_id IN (SELECT id FROM doctor_profiles WHERE user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    ) WITH CHECK (
        current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'SUPPORT')
        OR doctor_id IN (SELECT id FROM doctor_profiles WHERE user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    );

CREATE POLICY slots_payment_worker_update ON doctor_schedule_slots
    FOR UPDATE TO hellodoctor_payment_worker USING (true) WITH CHECK (true);
CREATE POLICY slots_system_worker_update ON doctor_schedule_slots
    FOR UPDATE TO hellodoctor_system_worker USING (true) WITH CHECK (true);

-- ----------------------------------------------------------------------------
-- appointments — no hellodoctor_api UPDATE grant exists, so only a SELECT
-- policy is needed for that role; payment/system workers get full access
-- within their GRANTs.
-- ----------------------------------------------------------------------------
ALTER TABLE appointments ENABLE ROW LEVEL SECURITY;
ALTER TABLE appointments FORCE ROW LEVEL SECURITY;

CREATE POLICY appointments_access ON appointments
USING (
    current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'FINANCE_ADMIN', 'SUPPORT', 'COMPLIANCE')
    OR EXISTS (SELECT 1 FROM patient_profiles p WHERE p.id = appointments.patient_id AND p.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    OR EXISTS (SELECT 1 FROM doctor_profiles d WHERE d.id = appointments.doctor_id AND d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
);

CREATE POLICY appointments_payment_worker_all ON appointments
    FOR ALL TO hellodoctor_payment_worker USING (true) WITH CHECK (true);
CREATE POLICY appointments_system_worker_all ON appointments
    FOR ALL TO hellodoctor_system_worker USING (true) WITH CHECK (true);
CREATE POLICY appointments_finance_worker_select ON appointments
    FOR SELECT TO hellodoctor_finance_worker USING (true);

-- ----------------------------------------------------------------------------
-- appointment_clinical_intake / prescription_intake_documents / prescriptions
-- / consultation_sessions / consultation_telemetry — all hellodoctor_api
-- read+write (ownership-scoped), appointment-shaped ownership check.
-- ----------------------------------------------------------------------------
ALTER TABLE appointment_clinical_intake ENABLE ROW LEVEL SECURITY;
ALTER TABLE appointment_clinical_intake FORCE ROW LEVEL SECURITY;
CREATE POLICY appointment_clinical_intake_access ON appointment_clinical_intake
USING (
    current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'COMPLIANCE', 'FINANCE_ADMIN', 'SUPPORT')
    OR EXISTS (
        SELECT 1 FROM appointments a
        LEFT JOIN patient_profiles p ON p.id = a.patient_id
        LEFT JOIN doctor_profiles d ON d.id = a.doctor_id
        WHERE a.id = appointment_clinical_intake.appointment_id
          AND (p.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
               OR d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    )
);
CREATE POLICY appointment_clinical_intake_payment_worker_insert ON appointment_clinical_intake
    FOR INSERT TO hellodoctor_payment_worker WITH CHECK (true);

ALTER TABLE prescription_intake_documents ENABLE ROW LEVEL SECURITY;
ALTER TABLE prescription_intake_documents FORCE ROW LEVEL SECURITY;
CREATE POLICY prescription_intake_documents_access ON prescription_intake_documents
USING (
    current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'COMPLIANCE', 'FINANCE_ADMIN', 'SUPPORT')
    OR EXISTS (
        SELECT 1 FROM appointments a
        LEFT JOIN patient_profiles p ON p.id = a.patient_id
        LEFT JOIN doctor_profiles d ON d.id = a.doctor_id
        WHERE a.id = prescription_intake_documents.appointment_id
          AND (p.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
               OR d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    )
);

ALTER TABLE consultation_sessions ENABLE ROW LEVEL SECURITY;
ALTER TABLE consultation_sessions FORCE ROW LEVEL SECURITY;
CREATE POLICY consultation_sessions_access ON consultation_sessions
USING (
    current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'COMPLIANCE', 'FINANCE_ADMIN', 'SUPPORT')
    OR EXISTS (
        SELECT 1 FROM appointments a
        LEFT JOIN patient_profiles p ON p.id = a.patient_id
        LEFT JOIN doctor_profiles d ON d.id = a.doctor_id
        WHERE a.id = consultation_sessions.appointment_id
          AND (p.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
               OR d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    )
);
CREATE POLICY consultation_sessions_system_worker_all ON consultation_sessions
    FOR ALL TO hellodoctor_system_worker USING (true) WITH CHECK (true);
CREATE POLICY consultation_sessions_finance_worker_select ON consultation_sessions
    FOR SELECT TO hellodoctor_finance_worker USING (true);

ALTER TABLE prescriptions ENABLE ROW LEVEL SECURITY;
ALTER TABLE prescriptions FORCE ROW LEVEL SECURITY;
CREATE POLICY prescriptions_access ON prescriptions
USING (
    current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'COMPLIANCE', 'FINANCE_ADMIN', 'SUPPORT')
    OR EXISTS (
        SELECT 1 FROM appointments a
        LEFT JOIN patient_profiles p ON p.id = a.patient_id
        LEFT JOIN doctor_profiles d ON d.id = a.doctor_id
        WHERE a.id = prescriptions.appointment_id
          AND (p.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
               OR d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    )
);

ALTER TABLE consultation_telemetry ENABLE ROW LEVEL SECURITY;
ALTER TABLE consultation_telemetry FORCE ROW LEVEL SECURITY;
CREATE POLICY consultation_telemetry_access ON consultation_telemetry
USING (
    current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'COMPLIANCE', 'FINANCE_ADMIN', 'SUPPORT')
    OR EXISTS (
        SELECT 1 FROM consultation_sessions cs
        JOIN appointments a ON a.id = cs.appointment_id
        LEFT JOIN patient_profiles p ON p.id = a.patient_id
        LEFT JOIN doctor_profiles d ON d.id = a.doctor_id
        WHERE cs.id = consultation_telemetry.session_id
          AND (p.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
               OR d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    )
);

-- ----------------------------------------------------------------------------
-- chat_conversations / chat_messages — hellodoctor_api has no UPDATE grant on
-- chat_conversations (lifecycle state: is_locked/expires_at), only system_worker
-- does. chat_messages.is_read is not lifecycle state, so api keeps UPDATE there.
-- ----------------------------------------------------------------------------
ALTER TABLE chat_conversations ENABLE ROW LEVEL SECURITY;
ALTER TABLE chat_conversations FORCE ROW LEVEL SECURITY;
CREATE POLICY chat_conversations_access ON chat_conversations
USING (
    current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'COMPLIANCE', 'FINANCE_ADMIN', 'SUPPORT')
    OR EXISTS (SELECT 1 FROM patient_profiles p WHERE p.id = chat_conversations.patient_id AND p.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    OR EXISTS (SELECT 1 FROM doctor_profiles d WHERE d.id = chat_conversations.doctor_id AND d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
);
CREATE POLICY chat_conversations_system_worker_all ON chat_conversations
    FOR ALL TO hellodoctor_system_worker USING (true) WITH CHECK (true);

ALTER TABLE chat_messages ENABLE ROW LEVEL SECURITY;
ALTER TABLE chat_messages FORCE ROW LEVEL SECURITY;
CREATE POLICY chat_messages_access ON chat_messages
USING (
    current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'COMPLIANCE', 'FINANCE_ADMIN', 'SUPPORT')
    OR EXISTS (
        SELECT 1 FROM chat_conversations c
        LEFT JOIN patient_profiles p ON p.id = c.patient_id
        LEFT JOIN doctor_profiles d ON d.id = c.doctor_id
        WHERE c.id = chat_messages.conversation_id
          AND (p.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
               OR d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    )
);
CREATE POLICY chat_messages_insert ON chat_messages
    FOR INSERT TO hellodoctor_api WITH CHECK (
        sender_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
        AND conversation_id IN (
            SELECT id FROM chat_conversations WHERE (
                patient_id IN (SELECT id FROM patient_profiles WHERE user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
                OR doctor_id IN (SELECT id FROM doctor_profiles WHERE user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
            )
            AND is_locked = FALSE
            AND expires_at > CURRENT_TIMESTAMP
        )
    );

-- ----------------------------------------------------------------------------
-- transactions — no hellodoctor_api UPDATE grant; payment/finance workers do.
-- ----------------------------------------------------------------------------
ALTER TABLE transactions ENABLE ROW LEVEL SECURITY;
ALTER TABLE transactions FORCE ROW LEVEL SECURITY;
CREATE POLICY transactions_access ON transactions
USING (
    current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'FINANCE_ADMIN', 'COMPLIANCE')
    OR EXISTS (
        SELECT 1 FROM appointments a
        LEFT JOIN patient_profiles p ON p.id = a.patient_id
        LEFT JOIN doctor_profiles d ON d.id = a.doctor_id
        WHERE a.id = transactions.appointment_id
          AND (p.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
               OR d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    )
);
CREATE POLICY transactions_payment_worker_all ON transactions
    FOR ALL TO hellodoctor_payment_worker USING (true) WITH CHECK (true);
CREATE POLICY transactions_finance_worker_all ON transactions
    FOR ALL TO hellodoctor_finance_worker USING (true) WITH CHECK (true);

-- ----------------------------------------------------------------------------
-- payment_events — mirrors transactions' ownership shape. Added under RLS in
-- this rewrite (the prior draft left it unprotected) because it is now
-- reachable by hellodoctor_api's blanket SELECT grant, used by admin finance
-- views — without RLS a buggy/unfiltered query could leak cross-patient data.
-- ----------------------------------------------------------------------------
ALTER TABLE payment_events ENABLE ROW LEVEL SECURITY;
ALTER TABLE payment_events FORCE ROW LEVEL SECURITY;
CREATE POLICY payment_events_access ON payment_events
USING (
    current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'FINANCE_ADMIN', 'COMPLIANCE')
    OR EXISTS (
        SELECT 1 FROM transactions t
        JOIN appointments a ON a.id = t.appointment_id
        LEFT JOIN patient_profiles p ON p.id = a.patient_id
        LEFT JOIN doctor_profiles d ON d.id = a.doctor_id
        WHERE t.id = payment_events.transaction_id
          AND (p.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
               OR d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    )
);
CREATE POLICY payment_events_payment_worker_all ON payment_events
    FOR ALL TO hellodoctor_payment_worker USING (true) WITH CHECK (true);
CREATE POLICY payment_events_finance_worker_all ON payment_events
    FOR ALL TO hellodoctor_finance_worker USING (true) WITH CHECK (true);

-- ----------------------------------------------------------------------------
-- doctor_wallets / disbursement_batches / disbursement_items — finance-worker
-- only for writes; hellodoctor_api is SELECT-only (read own earnings / admin).
-- ----------------------------------------------------------------------------
ALTER TABLE doctor_wallets ENABLE ROW LEVEL SECURITY;
ALTER TABLE doctor_wallets FORCE ROW LEVEL SECURITY;
CREATE POLICY doctor_wallets_access ON doctor_wallets
USING (
    current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'FINANCE_ADMIN')
    OR EXISTS (SELECT 1 FROM doctor_profiles d WHERE d.id = doctor_wallets.doctor_id AND d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
);
CREATE POLICY doctor_wallets_finance_worker_all ON doctor_wallets
    FOR ALL TO hellodoctor_finance_worker USING (true) WITH CHECK (true);
-- One-time bootstrap insert alongside doctor_profiles creation. ABAC already
-- gates who may reach this (Action::ManageDoctorCredentialing — ClinicalAdmin
-- or PlatformAdmin), so RLS does not re-derive that admin-role list here.
CREATE POLICY doctor_wallets_api_insert ON doctor_wallets
    FOR INSERT TO hellodoctor_api WITH CHECK (true);

ALTER TABLE disbursement_batches ENABLE ROW LEVEL SECURITY;
ALTER TABLE disbursement_batches FORCE ROW LEVEL SECURITY;
CREATE POLICY disbursement_batches_access ON disbursement_batches
USING (current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'FINANCE_ADMIN'));
CREATE POLICY disbursement_batches_finance_worker_all ON disbursement_batches
    FOR ALL TO hellodoctor_finance_worker USING (true) WITH CHECK (true);

ALTER TABLE disbursement_items ENABLE ROW LEVEL SECURITY;
ALTER TABLE disbursement_items FORCE ROW LEVEL SECURITY;
CREATE POLICY disbursement_items_access ON disbursement_items
USING (
    current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'FINANCE_ADMIN')
    OR EXISTS (SELECT 1 FROM doctor_profiles d WHERE d.id = disbursement_items.doctor_id AND d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
);
CREATE POLICY disbursement_items_finance_worker_all ON disbursement_items
    FOR ALL TO hellodoctor_finance_worker USING (true) WITH CHECK (true);

-- ----------------------------------------------------------------------------
-- grievance_reports / grievance_adjudications — filing is hellodoctor_api
-- (patient-authored INSERT only, no UPDATE grant exists for api); adjudication
-- writes (refund AND warn AND dismiss — all three) are finance-worker-only,
-- matching docs/DATABASE_SCHEMA.md's grant design exactly.
-- ----------------------------------------------------------------------------
ALTER TABLE grievance_reports ENABLE ROW LEVEL SECURITY;
ALTER TABLE grievance_reports FORCE ROW LEVEL SECURITY;
CREATE POLICY grievance_reports_access ON grievance_reports
USING (
    current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'COMPLIANCE')
    OR EXISTS (SELECT 1 FROM patient_profiles p WHERE p.id = grievance_reports.patient_id AND p.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    OR EXISTS (
        SELECT 1 FROM consultation_sessions cs
        JOIN appointments a ON a.id = cs.appointment_id
        JOIN doctor_profiles d ON d.id = a.doctor_id
        WHERE cs.id = grievance_reports.consultation_id
          AND d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
    )
);
CREATE POLICY grievance_reports_insert ON grievance_reports
    FOR INSERT TO hellodoctor_api WITH CHECK (
        patient_id IN (SELECT id FROM patient_profiles WHERE user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    );
CREATE POLICY grievance_reports_finance_worker_all ON grievance_reports
    FOR ALL TO hellodoctor_finance_worker USING (true) WITH CHECK (true);

ALTER TABLE grievance_adjudications ENABLE ROW LEVEL SECURITY;
ALTER TABLE grievance_adjudications FORCE ROW LEVEL SECURITY;
CREATE POLICY grievance_adjudications_access ON grievance_adjudications
USING (
    current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'COMPLIANCE')
    OR EXISTS (
        SELECT 1 FROM grievance_reports g
        LEFT JOIN patient_profiles p ON p.id = g.patient_id
        LEFT JOIN consultation_sessions cs ON cs.id = g.consultation_id
        LEFT JOIN appointments a ON a.id = cs.appointment_id
        LEFT JOIN doctor_profiles d ON d.id = a.doctor_id
        WHERE g.id = grievance_adjudications.grievance_id
          AND (p.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
               OR d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    )
);
CREATE POLICY grievance_adjudications_finance_worker_insert ON grievance_adjudications
    FOR INSERT TO hellodoctor_finance_worker WITH CHECK (true);

-- ----------------------------------------------------------------------------
-- audit_events — writable by all five roles (every service logs its own
-- actions); human visibility is further restricted by ABAC (Action::ViewLogs
-- → SECURITY_ADMIN/PLATFORM_ADMIN only), not by RLS — RLS here is the coarse
-- defense-in-depth backstop, consistent with the prior design's own rationale.
-- ----------------------------------------------------------------------------
ALTER TABLE audit_events ENABLE ROW LEVEL SECURITY;
ALTER TABLE audit_events FORCE ROW LEVEL SECURITY;
CREATE POLICY audit_events_insert ON audit_events FOR INSERT WITH CHECK (true);
CREATE POLICY audit_events_select ON audit_events FOR SELECT USING (
    current_setting('app.current_role', true) IN
        ('PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'FINANCE_ADMIN', 'SUPPORT', 'COMPLIANCE', 'SECURITY_ADMIN')
    OR current_user <> 'hellodoctor_api'
);
