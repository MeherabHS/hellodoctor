-- Restores the 2026-11-20 rule: a CHAT consultation closes when its booked
-- slot ends; a VIDEO consultation keeps a follow-up chat until 24 hours after
-- its slot ends or after activation (created_at), whichever is later.
DROP FUNCTION IF EXISTS finish_video_chat_window(UUID);

UPDATE chat_conversations c
SET expires_at = s.end_time
FROM appointments a
JOIN doctor_schedule_slots s ON s.id = a.slot_id
WHERE a.id = c.appointment_id
  AND a.modality = 'CHAT';

UPDATE chat_conversations c
SET expires_at = GREATEST(c.created_at, s.end_time) + INTERVAL '24 hours'
FROM appointments a
JOIN doctor_schedule_slots s ON s.id = a.slot_id
WHERE a.id = c.appointment_id
  AND a.modality = 'VIDEO';
