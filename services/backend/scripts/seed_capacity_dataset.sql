-- Deterministic synthetic dataset for the constrained-VPS capacity test
-- (docs/CONSTRAINED_CAPACITY_TEST.md). Approximates a mature ~1,200-user
-- pilot: multiple months of appointment/chat/transaction/telemetry history,
-- not just one row per user. No real PHI — all names/content are placeholder
-- strings, generated deterministically (not random()) so re-running this
-- script against a fresh database reproduces identical row counts.
--
-- Run against the CAPACITY-TEST postgres container only, never the dev one:
--   docker exec -i hellodoctor-capacity-postgres psql -U hellodoctor -d hellodoctor \
--     < scripts/seed_capacity_dataset.sql
--
-- Simplifications, documented rather than hidden: a historical slot's own
-- `status` is set to BOOKED whenever it has an appointment regardless of that
-- appointment's final status (real cancellation-releases-the-slot timing
-- isn't modeled); transaction `payment_status` is a coarse mapping from
-- appointment status, not a full settlement-cycle simulation.

BEGIN;

-- ============================================================================
-- 0. Parameters (edit here, not scattered through the script)
-- ============================================================================
DO $$
DECLARE
    num_doctors CONSTANT INT := 40;
    num_patients CONSTANT INT := 1150;
    num_admins CONSTANT INT := 10;
BEGIN
    RAISE NOTICE 'Seeding % doctors, % patients, % admins', num_doctors, num_patients, num_admins;
END $$;

-- ============================================================================
-- 1. Users + profiles
-- ============================================================================

-- Doctors
WITH d AS (
    SELECT gs AS seq,
           gen_random_uuid() AS user_id,
           gen_random_uuid() AS profile_id
    FROM generate_series(1, 40) AS gs
)
INSERT INTO users (id, phone_number, email, password_hash, role, preferred_language, is_active, is_verified, created_at, updated_at)
SELECT user_id,
       '+25471' || lpad(seq::text, 7, '0'),
       'doctor' || seq || '@capacity-test.local',
       '$argon2id$v=19$m=19456,t=2,p=1$ZGVtb3NhbHQ$ZGVtb2hhc2g',  -- placeholder, not used (no login benchmark hits this flow)
       'DOCTOR',
       'en',
       TRUE,
       TRUE,
       now() - (120 - (seq % 60)) * interval '1 day',
       now()
FROM d;

WITH d AS (
    SELECT u.id AS user_id, u.phone_number,
           row_number() OVER (ORDER BY u.phone_number) AS seq
    FROM users u WHERE u.role = 'DOCTOR' AND u.phone_number LIKE '+25471%'
)
INSERT INTO doctor_profiles (id, user_id, full_name, license_number, license_authority, license_country,
    primary_specialty, experience_years, current_hospital, qualifications,
    consultation_fee_video, consultation_fee_chat, residential_address, verified_phone,
    is_on_duty, is_verified, professional_verification_status, created_at, updated_at)
SELECT gen_random_uuid(), user_id,
       'Dr. Capacity Test ' || seq,
       'SYNTH-KE-CAP-' || lpad(seq::text, 6, '0'),
       'KMPDC',
       'KE',
       (ARRAY['General Medicine','Cardiology','Dermatology','Pediatrics','Gynecology','Psychiatry','Orthopedics','ENT'])[1 + (seq % 8)],
       5 + (seq % 20),
       'Capacity Test Hospital ' || (1 + (seq % 5)),
       ARRAY['MBBS'],
       (300 + (seq % 10) * 50)::numeric,
       (150 + (seq % 10) * 25)::numeric,
       'Synthetic Nairobi, Kenya',
       phone_number,
       (seq % 4 != 0),
       TRUE,
       'VERIFIED',
       now() - 120 * interval '1 day',
       now()
FROM d;

-- Patients
WITH p AS (
    SELECT gs AS seq, gen_random_uuid() AS user_id
    FROM generate_series(1, 1150) AS gs
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

WITH p AS (
    SELECT u.id AS user_id, row_number() OVER (ORDER BY u.phone_number) AS seq
    FROM users u WHERE u.role = 'PATIENT' AND u.phone_number LIKE '+25472%'
)
INSERT INTO patient_profiles (id, user_id, full_name, date_of_birth, gender, blood_group, weight_kg,
    emergency_contact_phone, emergency_contact_relation, created_at, updated_at)
SELECT gen_random_uuid(), user_id,
       'Capacity Test Patient ' || seq,
       (DATE '1970-01-01' + (seq % 15000)::int)::date,
       (ARRAY['MALE','FEMALE','OTHER']::gender_enum[])[1 + (seq % 3)],
       (ARRAY['A+','B+','O+','AB+','A-','O-'])[1 + (seq % 6)],
       (50 + (seq % 40))::numeric,
       '+25473' || lpad(seq::text, 7, '0'),
       'Spouse',
       now() - 180 * interval '1 day',
       now()
FROM p;

-- Admins, spread across the five admin roles
WITH a AS (
    SELECT gs AS seq, gen_random_uuid() AS user_id
    FROM generate_series(1, 10) AS gs
)
INSERT INTO users (id, phone_number, email, password_hash, role, preferred_language, is_active, is_verified, created_at, updated_at)
SELECT user_id,
       '+25474' || lpad(seq::text, 7, '0'),
       'admin' || seq || '@capacity-test.local',
       '$argon2id$v=19$m=19456,t=2,p=1$ZGVtb3NhbHQ$ZGVtb2hhc2g',
       (ARRAY['PLATFORM_ADMIN','CLINICAL_ADMIN','FINANCE_ADMIN','SUPPORT','COMPLIANCE','SECURITY_ADMIN']::user_role_enum[])[1 + (seq % 6)],
       'en', TRUE, TRUE,
       now() - 150 * interval '1 day',
       now()
FROM a;

-- ============================================================================
-- 2. Doctor schedule slots — historical (past, mostly booked) + future (available)
-- ============================================================================

-- Historical: 4 slots/day per doctor over the past 120 days = 480/doctor.
INSERT INTO doctor_schedule_slots (id, doctor_id, start_time, end_time, status, created_at)
SELECT gen_random_uuid(),
       dp.id,
       (now()::date - day_offset * interval '1 day') + (9 + slot_idx * 2) * interval '1 hour',
       (now()::date - day_offset * interval '1 day') + (9 + slot_idx * 2) * interval '1 hour' + interval '15 minutes',
       CASE WHEN (day_offset + slot_idx) % 10 < 7 THEN 'BOOKED' ELSE 'CANCELLED' END::slot_status_enum,
       now() - day_offset * interval '1 day'
FROM doctor_profiles dp
CROSS JOIN generate_series(1, 120) AS day_offset
CROSS JOIN generate_series(0, 3) AS slot_idx
WHERE dp.license_number LIKE 'SYNTH-KE-CAP-%';

-- Future: 1,080/doctor over the next 90 days (12 slots/day, every 2 hours) —
-- available for the live capacity-test load generator to actually book
-- against. Sized generously (43,200 total) against the combined booking
-- volume across every planned RPS stage (200/300/400/500/600 RPS tests plus
-- the non-contended/contention retests), not just the primary 300-RPS run,
-- so the test never runs dry mid-run and starts reporting false "no slots
-- available" business-rejects that look like a capacity problem but aren't.
INSERT INTO doctor_schedule_slots (id, doctor_id, start_time, end_time, status, created_at)
SELECT gen_random_uuid(),
       dp.id,
       (now()::date + day_offset * interval '1 day') + (slot_idx * 2) * interval '1 hour',
       (now()::date + day_offset * interval '1 day') + (slot_idx * 2) * interval '1 hour' + interval '15 minutes',
       'AVAILABLE',
       now()
FROM doctor_profiles dp
CROSS JOIN generate_series(1, 90) AS day_offset
CROSS JOIN generate_series(0, 11) AS slot_idx
WHERE dp.license_number LIKE 'SYNTH-KE-CAP-%';

-- ============================================================================
-- 3. Appointments + clinical intake + transactions + payment events, one
--    chain per BOOKED historical slot.
-- ============================================================================

CREATE TEMP TABLE tmp_appt AS
WITH booked_slots AS (
    SELECT s.id AS slot_id, s.doctor_id, s.start_time, s.created_at,
           row_number() OVER (ORDER BY s.doctor_id, s.start_time) AS rn
    FROM doctor_schedule_slots s
    JOIN doctor_profiles dp ON dp.id = s.doctor_id
    WHERE s.status = 'BOOKED' AND dp.license_number LIKE 'SYNTH-KE-CAP-%'
),
patient_pool AS (
    SELECT pp.id AS patient_id, row_number() OVER (ORDER BY pp.id) AS prn, count(*) OVER () AS pcount
    FROM patient_profiles pp
    JOIN users u ON u.id = pp.user_id
    WHERE u.phone_number LIKE '+25472%'
)
SELECT
    gen_random_uuid() AS appointment_id,
    bs.slot_id,
    bs.doctor_id,
    pp.patient_id,
    bs.start_time,
    bs.created_at,
    CASE WHEN bs.rn % 2 = 0 THEN 'VIDEO' ELSE 'CHAT' END::modality_enum AS modality,
    CASE
        WHEN bs.rn % 100 < 85 THEN 'COMPLETED'
        WHEN bs.rn % 100 < 92 THEN 'CANCELLED'
        WHEN bs.rn % 100 < 97 THEN 'REFUNDED'
        WHEN bs.rn % 100 < 99 THEN 'DISPUTED'
        ELSE 'PREMATURE_TERMINATION'
    END::appointment_status_enum AS status,
    bs.rn AS rn
FROM booked_slots bs
JOIN patient_pool pp ON pp.prn = 1 + (bs.rn % pp.pcount);

INSERT INTO appointments (id, appointment_number, patient_id, doctor_id, slot_id, modality, status,
    consultation_fee, clinical_outcome, completed_at, created_at, updated_at)
SELECT
    ta.appointment_id,
    'APT-' || to_char(ta.start_time, 'YYYYMMDD') || '-' || upper(left(ta.appointment_id::text, 8)),
    ta.patient_id, ta.doctor_id, ta.slot_id, ta.modality, ta.status,
    CASE WHEN ta.modality = 'VIDEO' THEN dp.consultation_fee_video ELSE dp.consultation_fee_chat END,
    CASE WHEN ta.status = 'COMPLETED' THEN
        (ARRAY['COMPLETED_WITH_RX','COMPLETED_NO_RX','REFERRED'])[1 + (ta.rn % 3)]
    ELSE NULL END,
    CASE WHEN ta.status IN ('COMPLETED','REFUNDED','DISPUTED','PREMATURE_TERMINATION') THEN ta.start_time + interval '30 minutes' ELSE NULL END,
    ta.created_at, ta.start_time
FROM tmp_appt ta
JOIN doctor_profiles dp ON dp.id = ta.doctor_id;

INSERT INTO appointment_clinical_intake (appointment_id, patient_id, chief_complaint, clinical_notes, created_at, updated_at)
SELECT ta.appointment_id, ta.patient_id,
       'Capacity-test synthetic complaint ' || (ta.rn % 20),
       CASE WHEN ta.status = 'COMPLETED' THEN 'Synthetic clinical note ' || ta.rn ELSE NULL END,
       ta.created_at, ta.start_time
FROM tmp_appt ta;

CREATE TEMP TABLE tmp_txn AS
SELECT
    gen_random_uuid() AS txn_id,
    ta.appointment_id,
    gen_random_uuid() AS payment_session_id,
    (ARRAY['BKASH','NAGAD','CARD','MPESA']::gateway_enum[])[1 + (ta.rn % 4)] AS gateway,
    CASE WHEN ta.modality = 'VIDEO' THEN dp.consultation_fee_video ELSE dp.consultation_fee_chat END AS gross,
    CASE
        WHEN ta.status = 'COMPLETED' THEN 'SETTLED_TO_DOCTOR'
        WHEN ta.status = 'REFUNDED' THEN 'REFUNDED_TO_PATIENT'
        ELSE 'PAYMENT_HELD'
    END::payment_status_enum AS payment_status,
    ta.rn
FROM tmp_appt ta
JOIN doctor_profiles dp ON dp.id = ta.doctor_id;

INSERT INTO transactions (id, transaction_number, appointment_id, payment_session_id, gateway, gateway_reference,
    gross_amount, platform_fee_amount, net_amount, payment_status, created_at, settled_at)
SELECT txn_id, 'TXN-' || upper(left(txn_id::text, 8)), appointment_id, payment_session_id, gateway,
       'CAP-REF-' || upper(left(txn_id::text, 10)),
       gross, round(gross * 0.20, 2), round(gross * 0.80, 2), payment_status,
       now(), CASE WHEN payment_status = 'SETTLED_TO_DOCTOR' THEN now() ELSE NULL END
FROM tmp_txn;

INSERT INTO payment_events (id, transaction_id, gateway, provider_event_id, provider_event_type, event_type,
    gateway_reference, gateway_response_code, amount, created_at)
SELECT gen_random_uuid(), txn_id, gateway, 'CAP-EVT-' || gen_random_uuid()::text, 'PAYMENT_SUCCESS', 'PAYMENT_SUCCESS',
       'CAP-REF-' || upper(left(txn_id::text, 10)), '00', gross, now()
FROM tmp_txn;

-- ============================================================================
-- 4. Consultation sessions + telemetry (skip pure CANCELLED appointments —
--    a consultation never started for those).
-- ============================================================================

CREATE TEMP TABLE tmp_session AS
SELECT gen_random_uuid() AS session_id, ta.appointment_id, ta.status, ta.start_time, ta.rn
FROM tmp_appt ta
WHERE ta.status != 'CANCELLED';

INSERT INTO consultation_sessions (id, appointment_id, agora_channel_name, started_at, ended_at, status, created_at)
SELECT session_id, appointment_id,
       'rtc_cap_' || replace(session_id::text, '-', ''),
       start_time,
       start_time + interval '20 minutes',
       CASE WHEN status = 'PREMATURE_TERMINATION' THEN 'PREMATURE_TERMINATION' ELSE 'COMPLETED' END::session_status_enum,
       start_time
FROM tmp_session;

INSERT INTO consultation_telemetry (id, session_id, call_duration_seconds, connection_state, packet_loss_percent,
    round_trip_time_ms, premature_end, prescription_issued, captured_at)
SELECT gen_random_uuid(), session_id,
       180 + (rn % 1200),
       'DISCONNECTED',
       (rn % 5)::numeric / 10,
       80 + (rn % 150),
       (status = 'PREMATURE_TERMINATION'),
       (status = 'COMPLETED' AND rn % 3 = 0),
       start_time + interval '20 minutes'
FROM tmp_session;

-- ============================================================================
-- 5. Chat conversations + messages, one conversation per appointment.
-- ============================================================================

CREATE TEMP TABLE tmp_conv AS
SELECT gen_random_uuid() AS conv_id, ta.appointment_id, ta.patient_id, ta.doctor_id, ta.start_time, ta.rn
FROM tmp_appt ta;

INSERT INTO chat_conversations (id, appointment_id, patient_id, doctor_id, expires_at, is_locked, created_at)
SELECT conv_id, appointment_id, patient_id, doctor_id, start_time + interval '24 hours', TRUE, start_time
FROM tmp_conv;

INSERT INTO chat_messages (id, conversation_id, sender_id, content, is_read, sent_at)
SELECT gen_random_uuid(), tc.conv_id,
       CASE WHEN m.i % 2 = 0 THEN pu.id ELSE du.id END,
       'Capacity-test synthetic chat message ' || m.i,
       TRUE,
       tc.start_time + (m.i * 7) * interval '1 minute'
FROM tmp_conv tc
JOIN patient_profiles pp ON pp.id = tc.patient_id
JOIN users pu ON pu.id = pp.user_id
JOIN doctor_profiles dp ON dp.id = tc.doctor_id
JOIN users du ON du.id = dp.user_id
CROSS JOIN generate_series(0, 2 + (tc.rn % 6)) AS m(i);

-- ============================================================================
-- 6. Grievances — ~2% of non-cancelled appointments.
-- ============================================================================

INSERT INTO grievance_reports (id, grievance_number, patient_id, consultation_id, target_type, category, claim_summary, status, created_at)
SELECT gen_random_uuid(),
       'GRV-CAP-' || lpad(ts.rn::text, 6, '0'),
       ta.patient_id,
       ts.session_id,
       CASE WHEN ts.rn % 3 = 0 THEN 'SYSTEM' ELSE 'DOCTOR' END::grievance_target_enum,
       (ARRAY['TECHNICAL_CALL_ISSUE','SERVICE_QUALITY','BILLING_DISPUTE'])[1 + (ts.rn % 3)],
       'Capacity-test synthetic grievance narrative ' || ts.rn,
       (ARRAY['PENDING_REVIEW','UNDER_INVESTIGATION','DISMISSED','REFUNDED','WARNED']::grievance_status_enum[])[1 + (ts.rn % 5)],
       ts.start_time + interval '1 day'
FROM tmp_session ts
JOIN tmp_appt ta ON ta.appointment_id = ts.appointment_id
WHERE ts.rn % 50 = 0;

-- ============================================================================
-- 7. Doctor wallets — aggregate of each doctor's settled transactions.
-- ============================================================================

INSERT INTO doctor_wallets (doctor_id, lifetime_gross, lifetime_fee_withheld, lifetime_net, pending_disbursement, updated_at)
SELECT ta.doctor_id,
       COALESCE(sum(t.gross_amount), 0),
       COALESCE(sum(t.platform_fee_amount), 0),
       COALESCE(sum(t.net_amount), 0),
       0,
       now()
FROM tmp_appt ta
JOIN transactions t ON t.appointment_id = ta.appointment_id
WHERE t.payment_status = 'SETTLED_TO_DOCTOR'
GROUP BY ta.doctor_id;

-- ============================================================================
-- 8. A modest amount of synthetic admin audit history.
-- ============================================================================

INSERT INTO audit_events (id, actor_id, actor_role, action, resource_type, resource_id, result, reason, created_at)
SELECT gen_random_uuid(), au.id, au.role,
       (ARRAY['DOCTOR_PROFESSIONAL_VERIFY','GRIEVANCE_ADJUDICATED','DISBURSEMENT_BATCH_APPROVED'])[1 + (gs % 3)],
       (ARRAY['DOCTOR_PROFILE','GRIEVANCE','DISBURSEMENT_BATCH'])[1 + (gs % 3)],
       gen_random_uuid(),
       'SUCCESS',
       'Capacity-test synthetic admin action ' || gs,
       now() - (gs % 150) * interval '1 day'
FROM generate_series(1, 300) AS gs
JOIN LATERAL (
    SELECT id, role FROM users WHERE phone_number LIKE '+25474%'
    OFFSET (gs % 10) LIMIT 1
) au ON TRUE;

DROP TABLE tmp_appt, tmp_txn, tmp_session, tmp_conv;

COMMIT;

-- ============================================================================
-- 9. Report actual row counts (not invented — printed from the data just inserted)
-- ============================================================================
SELECT 'users' AS table_name, count(*) FROM users
UNION ALL SELECT 'patient_profiles', count(*) FROM patient_profiles
UNION ALL SELECT 'doctor_profiles', count(*) FROM doctor_profiles
UNION ALL SELECT 'doctor_schedule_slots', count(*) FROM doctor_schedule_slots
UNION ALL SELECT 'doctor_schedule_slots_available', count(*) FROM doctor_schedule_slots WHERE status = 'AVAILABLE'
UNION ALL SELECT 'appointments', count(*) FROM appointments
UNION ALL SELECT 'appointment_clinical_intake', count(*) FROM appointment_clinical_intake
UNION ALL SELECT 'transactions', count(*) FROM transactions
UNION ALL SELECT 'payment_events', count(*) FROM payment_events
UNION ALL SELECT 'consultation_sessions', count(*) FROM consultation_sessions
UNION ALL SELECT 'consultation_telemetry', count(*) FROM consultation_telemetry
UNION ALL SELECT 'chat_conversations', count(*) FROM chat_conversations
UNION ALL SELECT 'chat_messages', count(*) FROM chat_messages
UNION ALL SELECT 'grievance_reports', count(*) FROM grievance_reports
UNION ALL SELECT 'doctor_wallets', count(*) FROM doctor_wallets
UNION ALL SELECT 'audit_events', count(*) FROM audit_events
ORDER BY 1;
