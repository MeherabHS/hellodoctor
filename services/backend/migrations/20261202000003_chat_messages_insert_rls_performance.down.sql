-- Reverts chat_messages_insert to the exact shape it had immediately before
-- this migration (the uncorrelated-subquery form from
-- 20261107000003_chat_rls_backstop's own up migration), not the original
-- 20261003000001_rls_hardening shape (which lacked the
-- professional_verification_status/deleted_at check) -- a down-migration
-- reverses what ITS OWN up-migration changed, nothing more.
DROP POLICY chat_messages_insert ON chat_messages;
CREATE POLICY chat_messages_insert ON chat_messages
    FOR INSERT TO hellodoctor_api WITH CHECK (
        sender_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
        AND conversation_id IN (
            SELECT id FROM chat_conversations WHERE (
                fn_patient_profile_owned_by_caller(patient_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid)
                OR doctor_id IN (SELECT id FROM doctor_profiles WHERE user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
                    AND professional_verification_status = 'VERIFIED' AND deleted_at IS NULL)
            )
            AND is_locked = FALSE
            AND expires_at > CURRENT_TIMESTAMP
        )
    );
