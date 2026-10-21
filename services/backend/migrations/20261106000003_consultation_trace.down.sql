DO $$ BEGIN IF EXISTS(SELECT 1 FROM consultation_trace_holds WHERE release_at IS NULL) THEN RAISE EXCEPTION 'Preserved grievance trace prevents destructive rollback'; END IF; END $$;
ALTER TABLE notifications DROP CONSTRAINT notifications_type_check;
-- New-type notification rows are retained on rollback; schema accepts historical events.
ALTER TABLE notifications ADD CONSTRAINT notifications_type_check CHECK(type IN ('DAILY_CONSULTATION_SUMMARY','CONSULTATION_REMINDER_15M','EARNING_CREDITED','PAYOUT_PAID','REGISTRATION_VERIFIED','REGISTRATION_REJECTED','ARTICLE_STATUS_CHANGED','NEW_CHAT_MESSAGE'));
DROP POLICY chat_sender_guard ON chat_messages;
CREATE POLICY chat_sender_guard ON chat_messages AS RESTRICTIVE FOR INSERT TO hellodoctor_api WITH CHECK(sender_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid AND EXISTS(SELECT 1 FROM chat_conversations c WHERE c.id=conversation_id AND NOT c.is_locked AND c.expires_at>NOW() AND (c.patient_id IN(SELECT id FROM patient_profiles WHERE user_id=sender_id) OR c.doctor_id IN(SELECT id FROM doctor_profiles WHERE user_id=sender_id))));
DROP TRIGGER chat_mutation_wall_clock ON chat_messages;
DROP FUNCTION guard_chat_mutation_time();
DROP TABLE chat_read_acknowledgements;
DROP INDEX chat_logical_message;
ALTER TABLE chat_messages DROP COLUMN idempotency_key_hash;
ALTER TABLE chat_conversations DROP COLUMN locked_at, DROP COLUMN lock_reason;
DROP TRIGGER grievance_trace_hold ON grievance_reports;
DROP FUNCTION hold_grievance_trace();
DROP FUNCTION cleanup_consultation_trace(INTEGER);
DROP TABLE consultation_trace_holds;
DROP TABLE consultation_trace_events;
DROP FUNCTION guard_consultation_trace();
DROP FUNCTION consultation_participant_allowed(UUID);
