DROP POLICY chat_sender_guard ON chat_messages;
CREATE POLICY chat_sender_guard ON chat_messages AS RESTRICTIVE FOR INSERT TO hellodoctor_api WITH CHECK (
 sender_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid AND EXISTS(
 SELECT 1 FROM chat_conversations c WHERE c.id=conversation_id AND NOT c.is_locked AND c.expires_at>clock_timestamp()
 AND (fn_patient_profile_owned_by_caller(c.patient_id,sender_id) OR c.doctor_id IN(SELECT id FROM doctor_profiles WHERE user_id=sender_id))));

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
