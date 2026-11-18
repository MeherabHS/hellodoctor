-- Read receipts for doctor-patient consultation chat (2026-11-17).
--
-- Chat messages are immutable clinical records: hellodoctor_api has no
-- UPDATE on chat_messages (its is_read column is never set). Read state is
-- therefore kept here, one row per participant per conversation: "this
-- person has read the conversation up to last_read_at". The other party's
-- marker gives the Sent/Read ticks; no message row is ever modified.

CREATE TABLE chat_read_markers (
    conversation_id UUID NOT NULL REFERENCES chat_conversations(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    last_read_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (conversation_id, user_id)
);

ALTER TABLE chat_read_markers ENABLE ROW LEVEL SECURITY;
ALTER TABLE chat_read_markers FORCE ROW LEVEL SECURITY;

-- Visible to the conversation's parties (each sees the other's receipt) and,
-- read-only, to admins, exactly like chat_messages_access.
CREATE POLICY chat_read_markers_select ON chat_read_markers FOR SELECT
    USING (
        current_setting('app.current_role', true) = 'ADMIN'
        OR EXISTS (
            SELECT 1 FROM chat_conversations c WHERE c.id = chat_read_markers.conversation_id
                AND (
                    fn_patient_profile_owned_by_caller(c.patient_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid)
                    OR c.doctor_id = fn_current_doctor_profile_id()
                )
        )
    );

-- Only a party writes, and only its own row: an admin viewing a chat never
-- marks it read, and nobody marks it read for someone else.
CREATE POLICY chat_read_markers_insert ON chat_read_markers FOR INSERT
    WITH CHECK (
        user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
        AND EXISTS (
            SELECT 1 FROM chat_conversations c WHERE c.id = chat_read_markers.conversation_id
                AND (
                    fn_patient_profile_owned_by_caller(c.patient_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid)
                    OR c.doctor_id = fn_current_doctor_profile_id()
                )
        )
    );

CREATE POLICY chat_read_markers_update ON chat_read_markers FOR UPDATE
    USING (user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    WITH CHECK (user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid);

GRANT SELECT, INSERT, UPDATE ON chat_read_markers TO hellodoctor_api;
