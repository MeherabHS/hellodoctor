-- Restores the previous rule for CHAT conversations: 24 hours after the slot
-- ends or after activation (created_at), whichever is later.
UPDATE chat_conversations c
SET expires_at = GREATEST(c.created_at, s.end_time) + INTERVAL '24 hours'
FROM appointments a
JOIN doctor_schedule_slots s ON s.id = a.slot_id
WHERE a.id = c.appointment_id
  AND a.modality = 'CHAT';
