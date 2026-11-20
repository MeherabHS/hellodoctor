-- Product owner, 2026-11-20: a CHAT consultation can be messaged only until
-- its booked slot ends; afterwards the history stays readable to both
-- parties but nobody can send. VIDEO consultations keep their 24-hour
-- follow-up chat. New conversations get this from
-- AppointmentService::chat_window_end; this brings existing CHAT
-- conversations in line. One-time backfill under the migrator's own
-- (superuser) connection, like the other backfills in this history.
UPDATE chat_conversations c
SET expires_at = s.end_time
FROM appointments a
JOIN doctor_schedule_slots s ON s.id = a.slot_id
WHERE a.id = c.appointment_id
  AND a.modality = 'CHAT'
  AND c.expires_at > s.end_time;
