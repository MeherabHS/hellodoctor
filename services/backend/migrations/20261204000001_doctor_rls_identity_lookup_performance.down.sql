-- Restores the exact pre-migration policy definitions (byte-for-byte
-- equivalent logic to what was live before 20261204000001's up migration),
-- then drops the now-unused helper function.
DROP POLICY consultation_sessions_access ON consultation_sessions;
CREATE POLICY consultation_sessions_access ON consultation_sessions
    USING (
        current_setting('app.current_role', true) = ANY (ARRAY['ADMIN','ADMIN','ADMIN','ADMIN','ADMIN'])
        OR EXISTS (
            SELECT 1 FROM appointments a LEFT JOIN doctor_profiles d ON d.id = a.doctor_id
            WHERE a.id = consultation_sessions.appointment_id
                AND (
                    fn_patient_profile_owned_by_caller(a.patient_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid)
                    OR d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
                )
        )
    );

DROP POLICY chat_messages_access ON chat_messages;
CREATE POLICY chat_messages_access ON chat_messages
    USING (
        current_setting('app.current_role', true) = ANY (ARRAY['ADMIN','ADMIN','ADMIN','ADMIN','ADMIN'])
        OR EXISTS (
            SELECT 1 FROM chat_conversations c LEFT JOIN doctor_profiles d ON d.id = c.doctor_id
            WHERE c.id = chat_messages.conversation_id
                AND (
                    fn_patient_profile_owned_by_caller(c.patient_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid)
                    OR d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
                )
        )
    );

DROP POLICY chat_conversations_access ON chat_conversations;
CREATE POLICY chat_conversations_access ON chat_conversations
    USING (
        current_setting('app.current_role', true) = ANY (ARRAY['ADMIN','ADMIN','ADMIN','ADMIN','ADMIN'])
        OR fn_patient_profile_owned_by_caller(patient_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid)
        OR EXISTS (
            SELECT 1 FROM doctor_profiles d
            WHERE d.id = chat_conversations.doctor_id
                AND d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
        )
    );

DROP POLICY appointments_access ON appointments;
CREATE POLICY appointments_access ON appointments
    USING (
        current_setting('app.current_role', true) = ANY (ARRAY['ADMIN','ADMIN','ADMIN','ADMIN','ADMIN'])
        OR fn_patient_profile_owned_by_caller(patient_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid)
        OR EXISTS (
            SELECT 1 FROM doctor_profiles d
            WHERE d.id = appointments.doctor_id
                AND d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
        )
    );

DROP FUNCTION IF EXISTS public.fn_current_doctor_profile_id();
