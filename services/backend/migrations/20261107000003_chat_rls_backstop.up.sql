-- Hardening note from Cloudflare security audit run-1: the Doctor branch of
-- chat RLS admits any doctor_profiles row matching the caller's user_id,
-- without itself checking professional_verification_status/deleted_at (the
-- stricter pattern doctor_own_encounter_allowed already uses elsewhere).
-- Today this is backstopped by chat_service.rs calling
-- ctx.verified_doctor_profile_id(state) before every chat read AND write, so
-- this migration only tightens the WRITE-side (INSERT) policies to match
-- what the application layer already enforces for all live chat access —
-- it changes no observable behavior today, it only removes the single
-- points of future regression where an app-layer slip would have had no DB
-- backstop. The READ policies (chat_conversations_access,
-- chat_messages_access) are deliberately left unchanged: a Doctor's
-- historical read access to their own completed-consultation chat threads
-- must survive professional-verification revocation (h22), and gating reads
-- on live VERIFIED status would silently break that.
DROP POLICY chat_sender_guard ON chat_messages;
CREATE POLICY chat_sender_guard ON chat_messages AS RESTRICTIVE FOR INSERT TO hellodoctor_api WITH CHECK (
 sender_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid AND EXISTS(
 SELECT 1 FROM chat_conversations c WHERE c.id=conversation_id AND NOT c.is_locked AND c.expires_at>clock_timestamp()
 AND (fn_patient_profile_owned_by_caller(c.patient_id,sender_id)
      OR c.doctor_id IN(SELECT id FROM doctor_profiles WHERE user_id=sender_id
          AND professional_verification_status='VERIFIED' AND deleted_at IS NULL))));

DROP POLICY chat_messages_insert ON chat_messages;
CREATE POLICY chat_messages_insert ON chat_messages
    FOR INSERT TO hellodoctor_api WITH CHECK (
        sender_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
        AND conversation_id IN (
            SELECT id FROM chat_conversations WHERE (
                fn_patient_profile_owned_by_caller(patient_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid)
                OR doctor_id IN (SELECT id FROM doctor_profiles WHERE user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
                    AND professional_verification_status='VERIFIED' AND deleted_at IS NULL)
            )
            AND is_locked = FALSE
            AND expires_at > CURRENT_TIMESTAMP
        )
    );
