          -- READ-ONLY operator preflight for migration 20261111000001_schedule_exclusivity.
-- Run against the target database BEFORE deploying. Modifies nothing.
--   psql "$DATABASE_URL" -f scripts/preflight_schedule_exclusivity.sql
-- Every result set must be empty. If any row appears, STOP: obtain an
-- operator-approved remediation plan. Paid appointments must never be
-- auto-cancelled or deleted to make the migration pass.
BEGIN READ ONLY;

\echo '== 1. Appointments whose slot no longer exists (must be empty)'
SELECT a.id AS appointment_id, a.slot_id
  FROM appointments a LEFT JOIN doctor_schedule_slots s ON s.id = a.slot_id
 WHERE s.id IS NULL;

\echo '== 2. Overlapping ACTIVE appointments for the same Patient profile (must be empty)'
SELECT a.patient_id, a.id AS appointment_a, a.status AS status_a, b.id AS appointment_b, b.status AS status_b,
       sa.start_time AS start_a, sa.end_time AS end_a, sb.start_time AS start_b, sb.end_time AS end_b
  FROM appointments a
  JOIN doctor_schedule_slots sa ON sa.id = a.slot_id
  JOIN appointments b ON b.patient_id = a.patient_id AND b.id > a.id
  JOIN doctor_schedule_slots sb ON sb.id = b.slot_id
 WHERE a.status IN ('PENDING_PAYMENT','CONFIRMED','WAITING','IN_CONSULTATION')
   AND b.status IN ('PENDING_PAYMENT','CONFIRMED','WAITING','IN_CONSULTATION')
   AND tstzrange(sa.start_time, sa.end_time, '[)') && tstzrange(sb.start_time, sb.end_time, '[)')
 ORDER BY sa.start_time;

\echo '== 3. Overlapping ACTIVE appointments for the same Doctor (must be empty)'
SELECT a.doctor_id, a.id AS appointment_a, a.status AS status_a, b.id AS appointment_b, b.status AS status_b,
       sa.start_time AS start_a, sa.end_time AS end_a, sb.start_time AS start_b, sb.end_time AS end_b
  FROM appointments a
  JOIN doctor_schedule_slots sa ON sa.id = a.slot_id
  JOIN appointments b ON b.doctor_id = a.doctor_id AND b.id > a.id
  JOIN doctor_schedule_slots sb ON sb.id = b.slot_id
 WHERE a.status IN ('PENDING_PAYMENT','CONFIRMED','WAITING','IN_CONSULTATION')
   AND b.status IN ('PENDING_PAYMENT','CONFIRMED','WAITING','IN_CONSULTATION')
   AND tstzrange(sa.start_time, sa.end_time, '[)') && tstzrange(sb.start_time, sb.end_time, '[)')
 ORDER BY sa.start_time;

\echo '== 4. Informational: two active appointments sharing one slot (also caught by 3)'
SELECT slot_id, count(*) AS active_appointments
  FROM appointments
 WHERE status IN ('PENDING_PAYMENT','CONFIRMED','WAITING','IN_CONSULTATION')
 GROUP BY slot_id HAVING count(*) > 1;

\echo '== 5. Informational: appointment.doctor_id differs from its slot.doctor_id'
SELECT a.id AS appointment_id, a.doctor_id AS appointment_doctor, s.doctor_id AS slot_doctor
  FROM appointments a JOIN doctor_schedule_slots s ON s.id = a.slot_id
 WHERE a.doctor_id <> s.doctor_id;

\echo '== 6. Informational: unpaid holds that are already past the configured window'
\echo '      (the sweep will expire these after deploy; set :window_secs, default 900)'
\if :{?window_secs} \else \set window_secs 900 \endif
SELECT id AS appointment_id, created_at, created_at + make_interval(secs => :window_secs) AS legacy_deadline
  FROM appointments
 WHERE status = 'PENDING_PAYMENT'
   AND created_at + make_interval(secs => :window_secs) <= clock_timestamp()
 ORDER BY created_at;

\echo '== 7. Informational: PENDING_PAYMENT rows that already have a non-initiated payment (should be empty)'
SELECT a.id AS appointment_id, t.id AS transaction_id, t.payment_status
  FROM appointments a JOIN transactions t ON t.appointment_id = a.id
 WHERE a.status = 'PENDING_PAYMENT' AND t.payment_status NOT IN ('INITIATED','FAILED');

ROLLBACK;
