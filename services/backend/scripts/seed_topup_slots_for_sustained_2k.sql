-- Additive appointment-slot top-up so a 40-minute / 2,000-session sustained
-- run doesn't run dry mid-test (observed consumption ~603 slots/min at 2,000
-- sessions; a 40-min run needs ~24k+, only 11,002 remained). Extends the
-- same 40 synthetic doctors' AVAILABLE future slots further out (days
-- 91-250, same 12-slots/day pattern as the original seed's days 1-90) --
-- does not touch any existing row.
BEGIN;

INSERT INTO doctor_schedule_slots (id, doctor_id, start_time, end_time, status, created_at)
SELECT gen_random_uuid(),
       dp.id,
       (now()::date + day_offset * interval '1 day') + (slot_idx * 2) * interval '1 hour',
       (now()::date + day_offset * interval '1 day') + (slot_idx * 2) * interval '1 hour' + interval '15 minutes',
       'AVAILABLE',
       now()
FROM doctor_profiles dp
CROSS JOIN generate_series(91, 250) AS day_offset
CROSS JOIN generate_series(0, 11) AS slot_idx
WHERE dp.license_number LIKE 'SYNTH-KE-CAP-%';

COMMIT;

SELECT count(*) AS available_slots_now FROM doctor_schedule_slots WHERE status = 'AVAILABLE';
