DROP INDEX IF EXISTS idx_chat_conversations_expiry_unlocked;

CREATE OR REPLACE FUNCTION public.cleanup_consultation_trace(p_limit integer DEFAULT 500)
 RETURNS bigint
 LANGUAGE plpgsql
 SET search_path TO 'public'
AS $function$
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
END $function$;
