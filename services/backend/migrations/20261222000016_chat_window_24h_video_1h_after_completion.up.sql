-- Product owner, 2026-11-21: the chat windows swap.
--   * A CHAT consultation can be messaged for 24 hours after its booked slot
--     ends (or after activation, if that is later).
--   * A VIDEO consultation's chat closes 1 hour after the consultation is
--     completed; until it is completed, 1 hour after its booked slot ends.
-- New conversations get this from AppointmentService::chat_window_end, and
-- completion calls finish_video_chat_window below. This brings existing
-- conversations in line, under the migrator's own (superuser) connection
-- like the other backfills in this history.

UPDATE chat_conversations c
SET expires_at = GREATEST(c.created_at, s.end_time) + INTERVAL '24 hours'
FROM appointments a
JOIN doctor_schedule_slots s ON s.id = a.slot_id
WHERE a.id = c.appointment_id
  AND a.modality = 'CHAT';

UPDATE chat_conversations c
SET expires_at = COALESCE(a.completed_at, s.end_time) + INTERVAL '1 hour'
FROM appointments a
JOIN doctor_schedule_slots s ON s.id = a.slot_id
WHERE a.id = c.appointment_id
  AND a.modality = 'VIDEO';

-- Called by the completing Doctor's transaction right after
-- doctor_complete_consultation returns COMPLETED. It can only ever set a
-- completed VIDEO consultation's chat to close 1 hour after that completion,
-- derived from server state, so it needs no caller check of its own.
-- hellodoctor_api otherwise may update only last_message_at on conversations.
CREATE OR REPLACE FUNCTION finish_video_chat_window(p_appointment UUID)
RETURNS VOID LANGUAGE sql SECURITY DEFINER SET search_path=pg_catalog,public AS $$
    UPDATE chat_conversations c
    SET expires_at = a.completed_at + INTERVAL '1 hour'
    FROM appointments a
    WHERE a.id = p_appointment
      AND c.appointment_id = a.id
      AND a.modality = 'VIDEO'
      AND a.status = 'COMPLETED'
      AND a.completed_at IS NOT NULL;
$$;
REVOKE ALL ON FUNCTION finish_video_chat_window(UUID) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION finish_video_chat_window(UUID) TO hellodoctor_api;
