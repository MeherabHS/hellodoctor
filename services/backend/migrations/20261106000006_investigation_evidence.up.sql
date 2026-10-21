-- Privileged read is deliberate and remains tied to a specific ADMIN account.
CREATE POLICY notifications_admin_evidence ON notifications FOR SELECT TO hellodoctor_api
 USING(current_setting('app.current_role',true)='ADMIN');
CREATE FUNCTION consultation_lock_transition() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NEW.is_locked AND NOT OLD.is_locked THEN
  NEW.locked_at:=clock_timestamp();NEW.lock_reason:=COALESCE(NEW.lock_reason,'WINDOW_CLOSED');
 END IF;RETURN NEW;
END $$;
CREATE TRIGGER chat_lock_transition BEFORE UPDATE OF is_locked ON chat_conversations
 FOR EACH ROW EXECUTE FUNCTION consultation_lock_transition();

CREATE FUNCTION session_related_to_consultation(p_session UUID,p_appointment UUID) RETURNS BOOLEAN
LANGUAGE sql SECURITY DEFINER SET search_path=public STABLE AS $$
 SELECT current_setting('app.current_role',true)='ADMIN' AND EXISTS(
  SELECT 1 FROM auth_sessions original JOIN auth_sessions used ON used.token_family_id=original.token_family_id
  JOIN consultation_trace_events e ON e.auth_session_id=used.id
  WHERE original.id=p_session AND e.appointment_id=p_appointment)
$$;
REVOKE ALL ON FUNCTION session_related_to_consultation(UUID,UUID) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION session_related_to_consultation(UUID,UUID) TO hellodoctor_api;
