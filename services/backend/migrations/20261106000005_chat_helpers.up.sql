CREATE FUNCTION bind_chat_message_key(p_message UUID,p_hash TEXT) RETURNS VOID
LANGUAGE plpgsql SECURITY DEFINER SET search_path=public AS $$
BEGIN
 IF length(p_hash)<>64 THEN RAISE EXCEPTION 'Invalid message key'; END IF;
 UPDATE chat_messages SET idempotency_key_hash=p_hash WHERE id=p_message
 AND sender_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid AND idempotency_key_hash IS NULL;
 IF NOT FOUND THEN RAISE EXCEPTION 'Message replay identity cannot be changed' USING ERRCODE='42501'; END IF;
END $$;
CREATE FUNCTION chat_counterparty_user(p_conversation UUID) RETURNS UUID
LANGUAGE sql SECURITY DEFINER SET search_path=public STABLE AS $$
 SELECT CASE WHEN d.user_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid THEN COALESCE(p.user_id,g.user_id) ELSE d.user_id END
 FROM chat_conversations c JOIN doctor_profiles d ON d.id=c.doctor_id JOIN patient_profiles p ON p.id=c.patient_id
 LEFT JOIN patient_profiles g ON g.id=p.guardian_patient_id
 WHERE c.id=p_conversation AND consultation_participant_allowed(c.appointment_id)
$$;
REVOKE ALL ON FUNCTION bind_chat_message_key(UUID,TEXT),chat_counterparty_user(UUID) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION bind_chat_message_key(UUID,TEXT),chat_counterparty_user(UUID) TO hellodoctor_api;

CREATE FUNCTION grievance_belongs_to_appointment(p_grievance UUID,p_appointment UUID) RETURNS BOOLEAN LANGUAGE sql SECURITY DEFINER SET search_path=public STABLE AS $$
 SELECT current_setting('app.current_role',true)='ADMIN' AND EXISTS(SELECT 1 FROM grievance_reports g JOIN consultation_sessions cs ON cs.id=g.consultation_id WHERE g.id=p_grievance AND cs.appointment_id=p_appointment)
$$;
REVOKE ALL ON FUNCTION grievance_belongs_to_appointment(UUID,UUID) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION grievance_belongs_to_appointment(UUID,UUID) TO hellodoctor_payment_worker;
