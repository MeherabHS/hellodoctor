-- Gives 5,000 of the synthetic capacity-test patients (phone prefix +25472) an
-- email and the documented, never-production benchmark password, so
-- examples/auth_stress_test.rs can sign them in the way the Patient app does
-- (email + password, then a one-time code). Synthetic data only.
--
-- The password is AuthStressBenchmark-2026! (same as
-- scripts/seed_auth_stress_doctors.sql); pass its Argon2id hash, produced by
-- `cargo run --release --example print_benchmark_doctor_hash`:
--
--   docker exec -i <postgres> psql -U postgres -d hellodoctor -v ON_ERROR_STOP=1 \
--     -v password_hash='<hash>' < scripts/seed_auth_stress_patients.sql
--
-- Applied after the canonical restore by the login scenario only
-- (services/backend/scripts/assurance/Invoke-AssuranceScenario.ps1 -SetupSql).

BEGIN;

WITH chosen AS (
    SELECT id, substring(phone_number from 7) AS seq
    FROM users
    WHERE phone_number LIKE '+25472%' AND role = 'PATIENT'
    ORDER BY phone_number
    LIMIT 5000
)
UPDATE users u
SET email = 'capacity.patient.' || chosen.seq || '@example.invalid',
    password_hash = :'password_hash',
    updated_at = now()
FROM chosen
WHERE u.id = chosen.id;

SELECT 'auth_stress_patients' AS table_name, count(*)
FROM users
WHERE email LIKE 'capacity.patient.%@example.invalid';

COMMIT;
