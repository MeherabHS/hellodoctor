CREATE TABLE consultation_trace_events (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
 appointment_id UUID NOT NULL REFERENCES appointments(id) ON DELETE RESTRICT,
 consultation_id UUID REFERENCES consultation_sessions(id) ON DELETE RESTRICT,
 conversation_id UUID REFERENCES chat_conversations(id) ON DELETE RESTRICT,
 actor_user_id UUID REFERENCES users(id), actor_role TEXT NOT NULL,
 auth_session_id UUID REFERENCES auth_sessions(id), request_id TEXT, trace_id TEXT,
 event_type TEXT NOT NULL, event_time_server TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 resource_type TEXT, resource_id UUID, result TEXT NOT NULL,
 reason_code TEXT, client_type TEXT, client_build TEXT,
 safe_metadata JSONB NOT NULL DEFAULT '{}',
 expires_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()+INTERVAL '5 days',
 created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
CREATE INDEX consultation_trace_order ON consultation_trace_events(appointment_id,event_time_server,id);
CREATE INDEX consultation_trace_expiry ON consultation_trace_events(expires_at);
CREATE TABLE consultation_trace_holds (
 grievance_id UUID PRIMARY KEY REFERENCES grievance_reports(id) ON DELETE RESTRICT,
 consultation_id UUID NOT NULL REFERENCES consultation_sessions(id),
 appointment_id UUID NOT NULL REFERENCES appointments(id),
 held_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 release_at TIMESTAMPTZ, held_by UUID REFERENCES users(id)
);
CREATE INDEX consultation_trace_hold_appointment ON consultation_trace_holds(appointment_id) WHERE release_at IS NULL;

CREATE FUNCTION consultation_participant_allowed(p_appointment UUID) RETURNS BOOLEAN
LANGUAGE sql SECURITY DEFINER SET search_path=public STABLE AS $$
 SELECT EXISTS(SELECT 1 FROM appointments a JOIN doctor_profiles d ON d.id=a.doctor_id
 WHERE a.id=p_appointment AND (
  current_setting('app.current_role',true)='ADMIN'
  OR (current_setting('app.current_role',true)='PATIENT' AND fn_patient_profile_owned_by_caller(a.patient_id,NULLIF(current_setting('app.current_user_id',true),'')::uuid))
  OR (current_setting('app.current_role',true)='DOCTOR' AND d.user_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid AND d.professional_verification_status='VERIFIED')
 ))
$$;
REVOKE ALL ON FUNCTION consultation_participant_allowed(UUID) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION consultation_participant_allowed(UUID) TO hellodoctor_api,hellodoctor_system_worker,hellodoctor_payment_worker,hellodoctor_finance_worker,hellodoctor_auth_worker;
ALTER TABLE consultation_trace_events ENABLE ROW LEVEL SECURITY;
ALTER TABLE consultation_trace_events FORCE ROW LEVEL SECURITY;
CREATE POLICY trace_read ON consultation_trace_events FOR SELECT TO hellodoctor_api
 USING(current_setting('app.current_role',true)='ADMIN');
CREATE POLICY trace_insert ON consultation_trace_events FOR INSERT TO hellodoctor_api
 WITH CHECK(consultation_participant_allowed(appointment_id));
CREATE POLICY trace_workers ON consultation_trace_events TO hellodoctor_system_worker,hellodoctor_payment_worker,hellodoctor_finance_worker,hellodoctor_auth_worker USING(true) WITH CHECK(true);
GRANT SELECT,INSERT ON consultation_trace_events TO hellodoctor_api;
GRANT SELECT,INSERT ON consultation_trace_events TO hellodoctor_payment_worker,hellodoctor_finance_worker,hellodoctor_auth_worker;
GRANT SELECT,INSERT,DELETE ON consultation_trace_events TO hellodoctor_system_worker;
ALTER TABLE consultation_trace_holds ENABLE ROW LEVEL SECURITY;
ALTER TABLE consultation_trace_holds FORCE ROW LEVEL SECURITY;
CREATE POLICY holds_admin_read ON consultation_trace_holds FOR SELECT TO hellodoctor_api USING(current_setting('app.current_role',true)='ADMIN');
CREATE POLICY holds_worker_read ON consultation_trace_holds FOR SELECT TO hellodoctor_system_worker USING(true);
GRANT SELECT ON consultation_trace_holds TO hellodoctor_api,hellodoctor_system_worker;

CREATE FUNCTION guard_consultation_trace() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE retention INTEGER:=COALESCE(NULLIF(current_setting('app.trace_retention_days',true),''),'5')::integer;
BEGIN
 IF retention NOT BETWEEN 3 AND 5 THEN RAISE EXCEPTION 'Invalid trace retention'; END IF;
 NEW.actor_user_id:=NULLIF(NULLIF(current_setting('app.current_user_id',true),''),'00000000-0000-0000-0000-000000000000')::uuid;
 NEW.actor_role:=CASE WHEN NEW.actor_user_id IS NULL THEN 'SYSTEM' ELSE current_setting('app.current_role',true) END;
 NEW.auth_session_id:=NULLIF(current_setting('app.current_session_id',true),'')::uuid;
 NEW.request_id:=NULLIF(current_setting('app.current_request_id',true),''); NEW.trace_id:=NEW.request_id;
 NEW.event_time_server:=clock_timestamp(); NEW.created_at:=NEW.event_time_server;
 NEW.expires_at:=NEW.event_time_server+make_interval(days=>retention);
 IF EXISTS(SELECT 1 FROM jsonb_object_keys(NEW.safe_metadata) k WHERE k NOT IN (
 'message_id','notification_id','grievance_id','transaction_id','original_slot_id','new_slot_id',
 'original_start','new_start','client_occurred_at','client_reported','acknowledgement','page_number','status','previous_status','count')) THEN
  RAISE EXCEPTION 'Unsafe trace metadata key';
 END IF;
 IF NEW.consultation_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM consultation_sessions s WHERE s.id=NEW.consultation_id AND s.appointment_id=NEW.appointment_id) THEN RAISE EXCEPTION 'Trace consultation mismatch'; END IF;
 IF NEW.conversation_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM chat_conversations c WHERE c.id=NEW.conversation_id AND c.appointment_id=NEW.appointment_id) THEN RAISE EXCEPTION 'Trace conversation mismatch'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER consultation_trace_context BEFORE INSERT ON consultation_trace_events FOR EACH ROW EXECUTE FUNCTION guard_consultation_trace();

CREATE FUNCTION hold_grievance_trace() RETURNS trigger LANGUAGE plpgsql SECURITY DEFINER SET search_path=public AS $$
DECLARE appointment UUID;
BEGIN
 SELECT appointment_id INTO appointment FROM consultation_sessions WHERE id=NEW.consultation_id;
 PERFORM pg_advisory_xact_lock(hashtextextended('consultation-trace:'||appointment::text,0));
 INSERT INTO consultation_trace_holds(grievance_id,consultation_id,appointment_id,held_by)
 VALUES(NEW.id,NEW.consultation_id,appointment,NULLIF(NULLIF(current_setting('app.current_user_id',true),''),'00000000-0000-0000-0000-000000000000')::uuid);
 RETURN NEW;
END $$;
CREATE TRIGGER grievance_trace_hold AFTER INSERT ON grievance_reports FOR EACH ROW EXECUTE FUNCTION hold_grievance_trace();
-- Existing investigations must preserve subsequent traces too, even though
-- their grievance INSERT predates this ledger and cannot fire its new trigger.
INSERT INTO consultation_trace_holds(grievance_id,consultation_id,appointment_id)
SELECT g.id,g.consultation_id,s.appointment_id FROM grievance_reports g
JOIN consultation_sessions s ON s.id=g.consultation_id
ON CONFLICT DO NOTHING;


-- Maintenance has no blanket DELETE privilege on any clinical/business table.
CREATE FUNCTION cleanup_consultation_trace(p_limit INTEGER DEFAULT 500) RETURNS BIGINT
LANGUAGE plpgsql SECURITY INVOKER SET search_path=public AS $$
DECLARE r RECORD; deleted BIGINT:=0; affected BIGINT;
BEGIN
 FOR r IN SELECT e.id,e.appointment_id FROM consultation_trace_events e WHERE e.expires_at<=clock_timestamp()
 AND NOT EXISTS(SELECT 1 FROM consultation_trace_holds h WHERE h.appointment_id=e.appointment_id AND h.release_at IS NULL)
 ORDER BY e.expires_at,e.id LIMIT LEAST(GREATEST(p_limit,1),1000) LOOP
  PERFORM pg_advisory_xact_lock(hashtextextended('consultation-trace:'||r.appointment_id::text,0));
  IF NOT EXISTS(SELECT 1 FROM consultation_trace_holds WHERE appointment_id=r.appointment_id AND release_at IS NULL) THEN
   DELETE FROM consultation_trace_events WHERE id=r.id AND expires_at<=clock_timestamp();
   GET DIAGNOSTICS affected=ROW_COUNT; deleted:=deleted+affected;
  END IF;
 END LOOP; RETURN deleted;
END $$;
REVOKE ALL ON FUNCTION cleanup_consultation_trace(INTEGER) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION cleanup_consultation_trace(INTEGER) TO hellodoctor_system_worker;

ALTER TABLE chat_conversations ADD COLUMN locked_at TIMESTAMPTZ, ADD COLUMN lock_reason TEXT;
ALTER TABLE chat_messages ADD COLUMN idempotency_key_hash TEXT;
CREATE UNIQUE INDEX chat_logical_message ON chat_messages(conversation_id,sender_id,idempotency_key_hash) WHERE idempotency_key_hash IS NOT NULL;
CREATE TABLE chat_read_acknowledgements (
 conversation_id UUID NOT NULL REFERENCES chat_conversations(id), user_id UUID NOT NULL REFERENCES users(id),
 message_id UUID NOT NULL REFERENCES chat_messages(id), first_acknowledged_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(user_id,message_id)
);
ALTER TABLE chat_read_acknowledgements ENABLE ROW LEVEL SECURITY;
ALTER TABLE chat_read_acknowledgements FORCE ROW LEVEL SECURITY;
CREATE POLICY read_ack_participant ON chat_read_acknowledgements USING(
 EXISTS(SELECT 1 FROM chat_conversations c WHERE c.id=conversation_id AND consultation_participant_allowed(c.appointment_id)))
 WITH CHECK(user_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid AND EXISTS(
 SELECT 1 FROM chat_messages m JOIN chat_conversations c ON c.id=m.conversation_id
 WHERE m.id=message_id AND m.conversation_id=chat_read_acknowledgements.conversation_id AND consultation_participant_allowed(c.appointment_id)));
GRANT SELECT,INSERT ON chat_read_acknowledgements TO hellodoctor_api;

DROP POLICY chat_sender_guard ON chat_messages;
CREATE POLICY chat_sender_guard ON chat_messages AS RESTRICTIVE FOR INSERT TO hellodoctor_api WITH CHECK (
 sender_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid AND EXISTS(
 SELECT 1 FROM chat_conversations c WHERE c.id=conversation_id AND NOT c.is_locked AND c.expires_at>clock_timestamp()
 AND (fn_patient_profile_owned_by_caller(c.patient_id,sender_id) OR c.doctor_id IN(SELECT id FROM doctor_profiles WHERE user_id=sender_id))));
CREATE FUNCTION guard_chat_mutation_time() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NOT EXISTS(SELECT 1 FROM chat_conversations c JOIN appointments a ON a.id=c.appointment_id
 WHERE c.id=NEW.conversation_id AND NOT c.is_locked AND c.expires_at>clock_timestamp()
 AND a.status IN ('CONFIRMED','WAITING','IN_CONSULTATION','COMPLETED')) THEN
  RAISE EXCEPTION 'Clinical chat mutation window closed' USING ERRCODE='42501';
 END IF; RETURN NEW;
END $$;
CREATE TRIGGER chat_mutation_wall_clock BEFORE INSERT ON chat_messages FOR EACH ROW EXECUTE FUNCTION guard_chat_mutation_time();

ALTER TABLE notifications DROP CONSTRAINT notifications_type_check;
ALTER TABLE notifications ADD CONSTRAINT notifications_type_check CHECK(type IN (
 'DAILY_CONSULTATION_SUMMARY','CONSULTATION_REMINDER_15M','EARNING_CREDITED','PAYOUT_PAID',
 'REGISTRATION_VERIFIED','REGISTRATION_REJECTED','ARTICLE_STATUS_CHANGED','NEW_CHAT_MESSAGE'));
