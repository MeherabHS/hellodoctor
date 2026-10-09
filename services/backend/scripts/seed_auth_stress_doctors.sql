-- Dedicated small pool of Doctor accounts for examples/auth_stress_test.rs
-- (PRE_PATIENT_AZURE_4X8_BENCHMARK §7). Separate from the main
-- seed_capacity_dataset.sql doctors, which carry a dummy, non-matching
-- password hash ("not used (no login benchmark hits this flow)" per that
-- script's own comment) — these need a REAL Argon2id hash of a known
-- benchmark-only password so real password verification is genuinely
-- exercised, not bypassed.
--
-- The password is fixed and documented, never a real credential:
--   AuthStressBenchmark-2026!
-- Its hash below was produced once by
-- `cargo run --release --example print_benchmark_doctor_hash`, never
-- generated at request time.
--
--   docker exec -i hellodoctor-bench-a-postgres psql -U hellodoctor -d hellodoctor \
--     -v num_auth_doctors=100 -v password_hash='REPLACED_AT_RUNTIME' \
--     < scripts/seed_auth_stress_doctors.sql

BEGIN;

WITH d AS (
    SELECT gs AS seq, gen_random_uuid() AS user_id
    FROM generate_series(1, :num_auth_doctors) AS gs
)
INSERT INTO users (id, phone_number, email, password_hash, role, preferred_language, is_active, is_verified, created_at, updated_at)
SELECT user_id,
       '+25475' || lpad(seq::text, 7, '0'),
       'authstress' || seq || '@capacity-test.local',
       :'password_hash',
       'DOCTOR',
       'en',
       TRUE,
       TRUE,
       now() - 30 * interval '1 day',
       now()
FROM d;

WITH d AS (
    SELECT u.id AS user_id, u.phone_number,
           row_number() OVER (ORDER BY u.phone_number) AS seq
    FROM users u WHERE u.role = 'DOCTOR' AND u.phone_number LIKE '+25475%'
)
INSERT INTO doctor_profiles (id, user_id, full_name, license_number, license_authority, license_country,
    primary_specialty, experience_years, current_hospital, qualifications,
    consultation_fee_video, consultation_fee_chat, residential_address, verified_phone,
    is_on_duty, is_verified, professional_verification_status, created_at, updated_at)
SELECT gen_random_uuid(), user_id,
       'Dr. Auth Stress ' || seq,
       'SYNTH-KE-AUTHSTRESS-' || lpad(seq::text, 6, '0'),
       'KMPDC', 'KE',
       'General Medicine', 10, 'Auth Stress Test Hospital',
       ARRAY['MBBS'],
       400, 200,
       'Synthetic Nairobi, Kenya',
       phone_number,
       FALSE, TRUE, 'VERIFIED',
       now() - 30 * interval '1 day',
       now()
FROM d;

COMMIT;

SELECT 'auth_stress_doctors' AS table_name, count(*) FROM users WHERE phone_number LIKE '+25475%';
