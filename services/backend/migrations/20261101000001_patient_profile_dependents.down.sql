-- Reverses 20261101000001_patient_profile_dependents.up.sql exactly, restoring
-- every policy to its pre-migration text and dropping the new schema objects.
-- Destructive only in the sense any down-migration is: a dependent row
-- (user_id IS NULL) cannot exist once `patient_profiles_self_xor_dependent`
-- and `guardian_patient_id` are gone — this down-migration will fail loudly
-- (FK/constraint violation) rather than silently discard dependent data if
-- any exists at the time it runs, which is the correct, safe failure mode.

DROP POLICY grievance_adjudications_access ON grievance_adjudications;
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

DROP POLICY grievance_patient_author ON grievance_reports;
CREATE POLICY grievance_patient_author ON grievance_reports AS RESTRICTIVE FOR INSERT TO hellodoctor_api WITH CHECK (
 current_setting('app.current_role',true)='PATIENT' AND patient_id IN(SELECT id FROM patient_profiles WHERE user_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid));

DROP POLICY grievance_reports_insert ON grievance_reports;
CREATE POLICY grievance_reports_insert ON grievance_reports
    FOR INSERT TO hellodoctor_api WITH CHECK (
        patient_id IN (SELECT id FROM patient_profiles WHERE user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    );

DROP POLICY grievance_reports_access ON grievance_reports;
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

DROP POLICY payment_events_access ON payment_events;
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

DROP POLICY transactions_access ON transactions;
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

DROP POLICY chat_messages_insert ON chat_messages;
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

DROP POLICY chat_messages_access ON chat_messages;
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

DROP POLICY chat_conversations_access ON chat_conversations;
CREATE POLICY chat_conversations_access ON chat_conversations
USING (
    current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'COMPLIANCE', 'FINANCE_ADMIN', 'SUPPORT')
    OR EXISTS (SELECT 1 FROM patient_profiles p WHERE p.id = chat_conversations.patient_id AND p.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    OR EXISTS (SELECT 1 FROM doctor_profiles d WHERE d.id = chat_conversations.doctor_id AND d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
);

DROP POLICY consultation_telemetry_events_access ON consultation_telemetry_events;
CREATE POLICY consultation_telemetry_events_access ON consultation_telemetry_events
USING (
    current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'COMPLIANCE', 'FINANCE_ADMIN', 'SUPPORT')
    OR EXISTS (
        SELECT 1 FROM consultation_sessions cs
        JOIN appointments a ON a.id = cs.appointment_id
        LEFT JOIN patient_profiles p ON p.id = a.patient_id
        LEFT JOIN doctor_profiles d ON d.id = a.doctor_id
        WHERE cs.id = consultation_telemetry_events.session_id
          AND (p.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
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
        LEFT JOIN patient_profiles p ON p.id = a.patient_id
        LEFT JOIN doctor_profiles d ON d.id = a.doctor_id
        WHERE cs.id = consultation_telemetry.session_id
          AND (p.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
               OR d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    )
);

-- Restores the actual 20261014000001 text (narrower role list), not
-- 20261003000001's broader one — see the matching comment in the up migration.
DROP POLICY prescriptions_access ON prescriptions;
CREATE POLICY prescriptions_access ON prescriptions
USING (
    current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'COMPLIANCE')
    OR EXISTS (
        SELECT 1 FROM appointments a
        LEFT JOIN patient_profiles p ON p.id = a.patient_id
        LEFT JOIN doctor_profiles d ON d.id = a.doctor_id
        WHERE a.id = prescriptions.appointment_id
          AND (p.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
               OR d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    )
);

DROP POLICY consultation_sessions_access ON consultation_sessions;
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

DROP POLICY intake_document_author_guard ON prescription_intake_documents;
CREATE POLICY intake_document_author_guard ON prescription_intake_documents AS RESTRICTIVE FOR INSERT TO hellodoctor_api WITH CHECK (
 current_setting('app.current_role',true)='PATIENT' AND patient_id IN(
 SELECT id FROM patient_profiles WHERE user_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid));

-- Restores the actual 20261014000001 text (narrower role list), not
-- 20261003000001's broader one — see the matching comment in the up migration.
DROP POLICY prescription_intake_documents_access ON prescription_intake_documents;
CREATE POLICY prescription_intake_documents_access ON prescription_intake_documents
USING (
    current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'COMPLIANCE')
    OR EXISTS (
        SELECT 1 FROM appointments a
        LEFT JOIN patient_profiles p ON p.id = a.patient_id
        LEFT JOIN doctor_profiles d ON d.id = a.doctor_id
        WHERE a.id = prescription_intake_documents.appointment_id
          AND (p.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
               OR d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    )
);

-- Restores the actual pre-this-migration state (as last written by
-- 20261017000001_clinical_and_payout_invariants), not the older
-- 20261003000001 text — see the matching comment in the up migration.
DROP POLICY appointment_clinical_intake_access ON appointment_clinical_intake;
CREATE POLICY appointment_clinical_intake_access ON appointment_clinical_intake
USING (
    current_setting('app.current_role',true) IN ('PLATFORM_ADMIN','CLINICAL_ADMIN','COMPLIANCE')
    OR EXISTS (SELECT 1 FROM appointments a JOIN patient_profiles p ON p.id=a.patient_id
        WHERE a.id=appointment_clinical_intake.appointment_id
        AND p.user_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid)
    OR doctor_history_context_allowed(appointment_id,NULLIF(current_setting('app.current_user_id',true),'')::uuid)
);

DROP POLICY booking_patient_author ON appointments;
CREATE POLICY booking_patient_author ON appointments AS RESTRICTIVE FOR INSERT TO hellodoctor_api WITH CHECK (
 current_setting('app.current_role',true)='PATIENT' AND patient_id IN(SELECT id FROM patient_profiles WHERE user_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid));

DROP POLICY appointments_access ON appointments;
CREATE POLICY appointments_access ON appointments
USING (
    current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'FINANCE_ADMIN', 'SUPPORT', 'COMPLIANCE')
    OR EXISTS (SELECT 1 FROM patient_profiles p WHERE p.id = appointments.patient_id AND p.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    OR EXISTS (SELECT 1 FROM doctor_profiles d WHERE d.id = appointments.doctor_id AND d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
);

DROP POLICY patient_profile_self_update ON patient_profiles;
CREATE POLICY patient_profile_self_update ON patient_profiles AS RESTRICTIVE FOR UPDATE TO hellodoctor_api USING (
 user_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid OR current_setting('app.current_role',true) IN ('PLATFORM_ADMIN','CLINICAL_ADMIN')) WITH CHECK (
 user_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid OR current_setting('app.current_role',true) IN ('PLATFORM_ADMIN','CLINICAL_ADMIN'));

DROP POLICY dependent_profile_author_guard ON patient_profiles;

DROP POLICY patient_profiles_access ON patient_profiles;
CREATE POLICY patient_profiles_access ON patient_profiles
USING (
    current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'COMPLIANCE', 'SUPPORT')
    OR user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
    OR fn_doctor_treats_patient(patient_profiles.id, NULLIF(current_setting('app.current_user_id', true), '')::uuid)
);

DROP FUNCTION fn_patient_profile_owned_by_caller(uuid, uuid);
DROP FUNCTION fn_patient_profile_is_self(uuid, uuid);

DROP TRIGGER patient_profiles_single_level_dependents ON patient_profiles;
DROP FUNCTION guard_single_level_dependents();

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
  required_role := CASE TG_TABLE_NAME WHEN 'patient_profiles' THEN 'PATIENT' ELSE 'DOCTOR' END;
  PERFORM 1 FROM public.users WHERE id=NEW.user_id AND role::text=required_role FOR SHARE;
  IF NOT FOUND THEN RAISE EXCEPTION 'Profile must match canonical account role' USING ERRCODE='23514'; END IF;
 END IF;
 RETURN NEW;
END $$;

REVOKE INSERT ON patient_profiles FROM hellodoctor_api;

DROP INDEX idx_patient_profiles_guardian;
ALTER TABLE patient_profiles DROP CONSTRAINT patient_profiles_self_xor_dependent;
ALTER TABLE patient_profiles DROP COLUMN relationship;
ALTER TABLE patient_profiles DROP COLUMN guardian_patient_id;
ALTER TABLE patient_profiles ALTER COLUMN user_id SET NOT NULL;
