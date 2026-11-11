-- Additive top-up for the 10,000-registered-user / 2,000-active-session
-- scalability objective (Milestone 8 Workstream 2). Does NOT touch or
-- replace the existing 1,150-patient historical dataset from
-- seed_capacity_dataset.sql -- this only ADDS 8,850 more patients (seq
-- 1151..10000) using the identical pattern (same phone prefix, same table
-- shape) so examples/capacity_test.rs's existing
-- "WHERE phone_number LIKE '+25472%' ORDER BY phone_number" loader picks
-- them up automatically with no harness code change. New patients get no
-- appointment/chat/transaction history (not needed for a short run; the
-- test harness books fresh appointments for them against already-seeded
-- AVAILABLE slots) -- a deliberate scope reduction to keep this a "quick"
-- top-up, not a full historical-depth dataset for all 10,000.
--
-- Run once against Config A's postgres:
--   docker exec -i hellodoctor-confa-postgres psql -U hellodoctor -d hellodoctor \
--     < scripts/seed_capacity_dataset_scaleup_10k.sql

BEGIN;

WITH p AS (
    SELECT gs AS seq, gen_random_uuid() AS user_id
    FROM generate_series(1151, 10000) AS gs
)
INSERT INTO users (id, phone_number, email, password_hash, role, preferred_language, is_active, is_verified, created_at, updated_at)
SELECT user_id,
       '+25472' || lpad(seq::text, 7, '0'),
       NULL,
       NULL,
       'PATIENT',
       CASE WHEN seq % 5 = 0 THEN 'sw' ELSE 'en' END,
       TRUE,
       TRUE,
       now() - (180 - (seq % 170)) * interval '1 day',
       now()
FROM p;

INSERT INTO patient_profiles (id, user_id, full_name, date_of_birth, gender, blood_group, weight_kg,
    emergency_contact_phone, emergency_contact_relation, created_at, updated_at)
SELECT gen_random_uuid(), u.id,
       'Capacity Test Patient ' || substring(u.phone_number from 7),
       (DATE '1970-01-01' + (('x' || substring(md5(u.id::text), 1, 6))::bit(24)::int % 15000)),
       (ARRAY['MALE','FEMALE','OTHER']::gender_enum[])[1 + (('x' || substring(md5(u.id::text), 1, 6))::bit(24)::int % 3)],
       (ARRAY['A+','B+','O+','AB+','A-','O-'])[1 + (('x' || substring(md5(u.id::text), 1, 6))::bit(24)::int % 6)],
       (50 + (('x' || substring(md5(u.id::text), 1, 6))::bit(24)::int % 40))::numeric,
       '+25473' || substring(u.phone_number from 7),
       'Spouse',
       now() - 30 * interval '1 day',
       now()
FROM users u
LEFT JOIN patient_profiles pp ON pp.user_id = u.id
WHERE u.role = 'PATIENT' AND u.phone_number LIKE '+25472%' AND pp.id IS NULL;

COMMIT;

-- Report final counts.
SELECT 'patients_total' AS metric, count(*) AS n FROM patient_profiles
UNION ALL
SELECT 'users_patient_role', count(*) FROM users WHERE role = 'PATIENT'
UNION ALL
SELECT 'available_slots', count(*) FROM doctor_schedule_slots WHERE status = 'AVAILABLE';
