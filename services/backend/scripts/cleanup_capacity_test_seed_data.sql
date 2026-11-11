-- Milestone 8 wrap-up: removes the synthetic capacity-test dataset (patients
-- +25472%, doctors +25471%, admins +25474%) and everything created against
-- it across Workstreams 1-3 (bookings, transactions, chat, notifications,
-- audit/idempotency rows, etc.) from Config A's database. Scoped precisely
-- by the project's own long-standing synthetic phone-number convention
-- (scripts/seed_capacity_dataset.sql) -- confirmed before running that
-- patient_profiles contains ONLY this synthetic data (count matched exactly
-- what was seeded), so this cannot touch any other dev data.
--
-- Deletion order follows the real FK dependency graph (every table with a
-- FOREIGN KEY to users/patient_profiles/doctor_profiles/appointments/
-- consultation_sessions/chat_conversations/transactions was enumerated via
-- information_schema before writing this, not guessed). Wrapped in one
-- transaction so a missed table fails safely (whole thing rolls back) rather
-- than leaving a half-deleted state.
--
-- Run once against Config A's postgres:
--   docker exec -i hellodoctor-confa-postgres psql -U hellodoctor -d hellodoctor \
--     < scripts/cleanup_capacity_test_seed_data.sql
--
-- Full reusable Milestone 8 benchmark-dataset workflow, in order:
--   1. seed_capacity_dataset.sql            -- base: 1,150 patients/40 doctors/10 admins + slots
--   2. seed_capacity_dataset_scaleup_10k.sql -- additive top-up to 10,000 patients
--   3. seed_topup_slots_for_sustained_2k.sql -- additive slot top-up for long (30min+) sustained runs
--   4. cleanup_capacity_test_seed_data.sql   -- (this file) wipe it all back to zero when done
-- Note: step 4's scoped, dependency-ordered DELETEs got pathologically slow
-- on consultation_trace_events (400k+ rows, no index on conversation_id/
-- consultation_id) -- fixed ad hoc with CREATE INDEX directly on Config A
-- (not a tracked migration; consider promoting to a real migration if this
-- table stays this large going forward). For an all-synthetic database like
-- Config A, `TRUNCATE users CASCADE;` is a valid, much faster alternative to
-- this whole script -- confirmed safe here since every row traces back to a
-- seeded synthetic user -- but double-check that still holds before reusing
-- it against a database that might also hold non-synthetic data.

BEGIN;

CREATE TEMP TABLE synth_users AS
SELECT id, role FROM users
WHERE phone_number LIKE '+25471%' OR phone_number LIKE '+25472%' OR phone_number LIKE '+25474%';

CREATE TEMP TABLE synth_patients AS
SELECT pp.id, pp.user_id FROM patient_profiles pp
JOIN synth_users su ON su.id = pp.user_id;

CREATE TEMP TABLE synth_doctors AS
SELECT dp.id, dp.user_id FROM doctor_profiles dp
JOIN synth_users su ON su.id = dp.user_id;

CREATE TEMP TABLE synth_appointments AS
SELECT a.id FROM appointments a
WHERE a.patient_id IN (SELECT id FROM synth_patients)
   OR a.doctor_id IN (SELECT id FROM synth_doctors);

CREATE TEMP TABLE synth_consultations AS
SELECT cs.id FROM consultation_sessions cs
WHERE cs.appointment_id IN (SELECT id FROM synth_appointments);

CREATE TEMP TABLE synth_conversations AS
SELECT cc.id FROM chat_conversations cc
WHERE cc.appointment_id IN (SELECT id FROM synth_appointments);

CREATE TEMP TABLE synth_transactions AS
SELECT t.id FROM transactions t
WHERE t.appointment_id IN (SELECT id FROM synth_appointments);

CREATE TEMP TABLE synth_grievances AS
SELECT g.id FROM grievance_reports g
WHERE g.patient_id IN (SELECT id FROM synth_patients)
   OR g.consultation_id IN (SELECT id FROM synth_consultations);

-- Level 0: leaves of consultation/transaction (most are ON DELETE CASCADE
-- from their parent and would go automatically, but deleted explicitly
-- here for a clean, auditable step-by-step count).
DELETE FROM consultation_provider_presence_events WHERE session_id IN (SELECT id FROM synth_consultations);
DELETE FROM consultation_telemetry WHERE session_id IN (SELECT id FROM synth_consultations);
DELETE FROM consultation_telemetry_events WHERE session_id IN (SELECT id FROM synth_consultations);
DELETE FROM chat_read_acknowledgements WHERE conversation_id IN (SELECT id FROM synth_conversations);
DELETE FROM chat_messages WHERE conversation_id IN (SELECT id FROM synth_conversations);
DELETE FROM financial_economic_effects WHERE transaction_id IN (SELECT id FROM synth_transactions);
DELETE FROM financial_refund_requests WHERE transaction_id IN (SELECT id FROM synth_transactions);
DELETE FROM monthly_statement_transactions WHERE transaction_id IN (SELECT id FROM synth_transactions);
DELETE FROM payment_events WHERE transaction_id IN (SELECT id FROM synth_transactions);

-- Level 1: direct dependents of consultation_sessions / grievance_reports.
DELETE FROM consultation_recovery_resolutions WHERE session_id IN (SELECT id FROM synth_consultations);
DELETE FROM consultation_trace_events WHERE consultation_id IN (SELECT id FROM synth_consultations)
   OR appointment_id IN (SELECT id FROM synth_appointments)
   OR conversation_id IN (SELECT id FROM synth_conversations);
DELETE FROM consultation_trace_holds WHERE consultation_id IN (SELECT id FROM synth_consultations)
   OR appointment_id IN (SELECT id FROM synth_appointments);
DELETE FROM grievance_adjudications WHERE grievance_id IN (SELECT id FROM synth_grievances);
DELETE FROM grievance_reports WHERE id IN (SELECT id FROM synth_grievances);

-- Level 2: dependents of appointments.
DELETE FROM appointment_clinical_intake WHERE appointment_id IN (SELECT id FROM synth_appointments);
DELETE FROM appointment_expiry_cleanup_jobs WHERE appointment_id IN (SELECT id FROM synth_appointments);
DELETE FROM appointment_reschedules WHERE appointment_id IN (SELECT id FROM synth_appointments);
DELETE FROM prescription_intake_documents WHERE appointment_id IN (SELECT id FROM synth_appointments);
DELETE FROM prescriptions WHERE appointment_id IN (SELECT id FROM synth_appointments);
DELETE FROM chat_conversations WHERE id IN (SELECT id FROM synth_conversations);
DELETE FROM consultation_sessions WHERE id IN (SELECT id FROM synth_consultations);
DELETE FROM transactions WHERE id IN (SELECT id FROM synth_transactions);

-- Level 3: health Q&A, doctor financial rows (defensive — likely zero rows,
-- included because the FK scan found them, not skipped as an assumption).
DELETE FROM health_answers WHERE doctor_id IN (SELECT id FROM synth_doctors);
DELETE FROM health_questions WHERE author_patient_id IN (SELECT id FROM synth_patients);
DELETE FROM doctor_articles WHERE author_id IN (SELECT id FROM synth_doctors);
DELETE FROM disbursement_items WHERE doctor_id IN (SELECT id FROM synth_doctors);
DELETE FROM financial_adjustments WHERE doctor_id IN (SELECT id FROM synth_doctors);
DELETE FROM monthly_payout_statements WHERE doctor_id IN (SELECT id FROM synth_doctors);
DELETE FROM doctor_wallets WHERE doctor_id IN (SELECT id FROM synth_doctors);

-- Level 4: appointments themselves, now unblocked.
DELETE FROM appointments WHERE id IN (SELECT id FROM synth_appointments);

-- Level 5: admin_articles authored/approved by a synthetic admin (the
-- Workstream-3 E2E test articles) and their own dependents.
DELETE FROM article_media WHERE article_id IN (
    SELECT id FROM admin_articles WHERE author_id IN (SELECT id FROM synth_users) OR approved_by IN (SELECT id FROM synth_users)
);
-- Schema note (found 2026-11-04, re-running this script after a long gap):
-- article_quiz_questions/article_quiz_options no longer carry article_id
-- directly -- quiz content was restructured behind article_quiz_revisions
-- (revision_id -> article_quiz_revisions.article_id) at some point after this
-- script was last written. Updated both DELETEs below to join through that
-- table instead of the no-longer-existing q.article_id/options.article_id
-- columns; confirmed via \d against the live schema before editing.
DELETE FROM article_quiz_options WHERE (revision_id, question_id) IN (
    SELECT q.revision_id, q.id FROM article_quiz_questions q
    JOIN article_quiz_revisions r ON r.id = q.revision_id
    JOIN admin_articles a ON a.id = r.article_id
    WHERE a.author_id IN (SELECT id FROM synth_users) OR a.approved_by IN (SELECT id FROM synth_users)
);
DELETE FROM article_quiz_questions WHERE revision_id IN (
    SELECT r.id FROM article_quiz_revisions r
    JOIN admin_articles a ON a.id = r.article_id
    WHERE a.author_id IN (SELECT id FROM synth_users) OR a.approved_by IN (SELECT id FROM synth_users)
);
DELETE FROM article_quiz_revisions WHERE approved_by IN (SELECT id FROM synth_users) OR edited_by IN (SELECT id FROM synth_users) OR created_by IN (SELECT id FROM synth_users);
DELETE FROM article_quiz_publications WHERE article_id IN (
    SELECT id FROM admin_articles WHERE author_id IN (SELECT id FROM synth_users) OR approved_by IN (SELECT id FROM synth_users)
);
DELETE FROM admin_articles WHERE author_id IN (SELECT id FROM synth_users) OR approved_by IN (SELECT id FROM synth_users);

-- Level 6: everything else referencing users directly (NO ACTION/RESTRICT
-- FKs that would otherwise block deleting the user row).
DELETE FROM account_erasure_jobs WHERE user_id IN (SELECT id FROM synth_users) OR requested_by IN (SELECT id FROM synth_users);
DELETE FROM audit_events WHERE actor_id IN (SELECT id FROM synth_users);
DELETE FROM idempotency_records WHERE user_id IN (SELECT id FROM synth_users);
DELETE FROM legacy_admin_role_assignments WHERE user_id IN (SELECT id FROM synth_users);
DELETE FROM legal_acceptances WHERE user_id IN (SELECT id FROM synth_users);
DELETE FROM notification_device_tokens WHERE user_id IN (SELECT id FROM synth_users);
DELETE FROM notifications WHERE user_id IN (SELECT id FROM synth_users);
DELETE FROM patient_data_requests WHERE user_id IN (SELECT id FROM synth_users);
DELETE FROM platform_payment_methods WHERE created_by IN (SELECT id FROM synth_users);
DELETE FROM system_error_logs WHERE impacted_user_id IN (SELECT id FROM synth_users) OR resolved_by IN (SELECT id FROM synth_users) OR acknowledged_by IN (SELECT id FROM synth_users);
DELETE FROM disbursement_batches WHERE initiated_by IN (SELECT id FROM synth_users);
DELETE FROM campaigns WHERE approved_by IN (SELECT id FROM synth_users) OR created_by IN (SELECT id FROM synth_users);
DELETE FROM financial_periods WHERE finalized_by IN (SELECT id FROM synth_users);

-- Level 7: profiles (doctor_schedule_slots cascades automatically from
-- doctor_profiles; patient_payment_methods / doctor_payout_* cascade too).
DELETE FROM doctor_profiles WHERE id IN (SELECT id FROM synth_doctors);
DELETE FROM patient_profiles WHERE id IN (SELECT id FROM synth_patients);

-- Level 8: the users themselves (auth_sessions, notification_preferences/
-- frequency_counters, patient_auth_challenges, pending_mfa_challenges all
-- cascade automatically).
DELETE FROM users WHERE id IN (SELECT id FROM synth_users);

-- Verification before commit.
SELECT 'remaining_synthetic_users' AS metric, count(*) AS n FROM users WHERE phone_number LIKE '+25471%' OR phone_number LIKE '+25472%' OR phone_number LIKE '+25474%'
UNION ALL SELECT 'remaining_patient_profiles', count(*) FROM patient_profiles
UNION ALL SELECT 'remaining_doctor_profiles', count(*) FROM doctor_profiles
UNION ALL SELECT 'remaining_appointments', count(*) FROM appointments
UNION ALL SELECT 'remaining_transactions', count(*) FROM transactions
UNION ALL SELECT 'remaining_doctor_schedule_slots', count(*) FROM doctor_schedule_slots
UNION ALL SELECT 'remaining_notifications', count(*) FROM notifications
UNION ALL SELECT 'remaining_grievance_reports', count(*) FROM grievance_reports;

COMMIT;
