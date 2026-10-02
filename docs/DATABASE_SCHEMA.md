# Phase 6 — PostgreSQL Database Schema & Relational Design

> **Document Version:** 1.0.0  
> **Database Engine:** PostgreSQL 16+ (ACID compliant, Row-Level Security, B-Tree & GIN indexing)  
> **ORM / Query Engine:** Rust `sqlx` with compile-time verified queries (`sqlx::query!`)  
> **Target Audience:** Claude Code Autonomous Implementation Agent

---

## 1. Entity-Relationship Overview

```
User (1) ──────┬────── (1) PatientProfile (1) ────── (N) Appointment
               │                                            │
               └────── (1) DoctorProfile (1) ────── (N)     │
                               │                            │
                               ├────── (1) DoctorWallet     │
                               │                            │
                               └────── (N) ScheduleSlot ────┤
                                                            │
┌───────────────────────────────────────────────────────────┴────────────────────────────────────────┐
│                                                                                                    │
├────── (N) PrescriptionIntakeDocument (Max 5 photos per appointment)                                │
├────── (1) ConsultationSession (1) ────── (1) ConsultationTelemetry                                │
├────── (1) Prescription (secure handwritten-Rx photo)                                               │
├────── (N) ChatMessage                                                                              │
├────── (N) GrievanceReport (1) ────── (1) GrievanceAdjudication                                      │
└────── (1) Transaction (Payment holding ledger)                                                     │
```

---

## 2. Production PostgreSQL DDL Script

```sql
-- Enable UUID and Cryptographic Extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pgcrypto";
CREATE EXTENSION IF NOT EXISTS btree_gist;

-- ============================================================================
-- 1. USERS & AUTHENTICATION
-- ============================================================================
CREATE TYPE user_role_enum AS ENUM (
    'PATIENT', 'DOCTOR', 'PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'FINANCE_ADMIN', 'SUPPORT', 'COMPLIANCE', 'SECURITY_ADMIN'
);

CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    phone_number VARCHAR(20) NOT NULL UNIQUE,
    email VARCHAR(255) UNIQUE,
    password_hash VARCHAR(255),
    role user_role_enum NOT NULL DEFAULT 'PATIENT',
    preferred_language VARCHAR(5) NOT NULL DEFAULT 'en', -- 'en' or 'sw'
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    is_verified BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    deleted_at TIMESTAMPTZ
);

CREATE INDEX idx_users_phone ON users(phone_number) WHERE deleted_at IS NULL;
CREATE INDEX idx_users_role ON users(role);

CREATE TABLE auth_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_family_id UUID NOT NULL,
    refresh_token_hash VARCHAR(128) NOT NULL,
    device_fingerprint VARCHAR(255),
    ip_hash VARCHAR(64),
    user_agent VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_used_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    rotated_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX idx_auth_sessions_user ON auth_sessions(user_id) WHERE revoked_at IS NULL;
CREATE INDEX idx_auth_sessions_family ON auth_sessions(token_family_id);
CREATE UNIQUE INDEX idx_auth_sessions_token ON auth_sessions(refresh_token_hash) WHERE revoked_at IS NULL;

-- ============================================================================
-- 1b. MFA CREDENTIALS (TOTP / Recovery for Doctors & Admins)
-- ============================================================================
CREATE TYPE mfa_method_enum AS ENUM ('TOTP', 'PASSKEY');

CREATE TABLE mfa_credentials (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    method mfa_method_enum NOT NULL DEFAULT 'TOTP',
    totp_secret_encrypted BYTEA NOT NULL, -- Encrypted via KMS/envelope encryption, NOT hashed
    recovery_codes_hash TEXT[], -- Array of Argon2id hashed one-time recovery codes
    is_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    enabled_at TIMESTAMPTZ,
    last_verified_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE UNIQUE INDEX idx_mfa_user_method ON mfa_credentials(user_id, method) WHERE revoked_at IS NULL;

-- Notes:
-- The TOTP secret must be stored ENCRYPTED (not hashed) because the server
-- needs the plaintext secret to verify time-based codes.
-- Use envelope encryption: encrypt with a data encryption key (DEK),
-- which is itself encrypted by a KMS master key.
--
-- MFA Enrollment Flow:
--   1. Server generates TOTP secret, encrypts with DEK, stores in DB
--   2. Server returns QR code / otpauth:// URI to admin's authenticator app
--   3. Admin enters verification code to confirm enrollment
--   4. is_enabled = TRUE, enabled_at = now()
--
-- MFA Reset / Lost Device:
--   1. Admin uses one of 8 pre-generated recovery codes (one-time use)
--   2. After recovery code used, hash is removed from array
--   3. If all codes exhausted, PLATFORM_ADMIN must reset MFA manually
--   4. Reset requires re-enrollment and generates new codes

-- ============================================================================
-- 2. PATIENT PROFILES
-- ============================================================================
CREATE TYPE gender_enum AS ENUM ('MALE', 'FEMALE', 'OTHER');

CREATE TABLE patient_profiles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL UNIQUE REFERENCES users(id) ON DELETE RESTRICT,
    full_name VARCHAR(120) NOT NULL,
    date_of_birth DATE NOT NULL,
    gender gender_enum NOT NULL,
    blood_group VARCHAR(5),
    weight_kg NUMERIC(5, 2),
    emergency_contact_phone VARCHAR(20),
    emergency_contact_relation VARCHAR(50),
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_patient_profiles_user ON patient_profiles(user_id);

-- ============================================================================
-- 3. DOCTOR PROFILES & DOSSIERS
-- ============================================================================
CREATE TABLE doctor_profiles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL UNIQUE REFERENCES users(id) ON DELETE RESTRICT,
    full_name VARCHAR(120) NOT NULL,
    license_number VARCHAR(50) NOT NULL UNIQUE,
    license_authority VARCHAR(50) NOT NULL,
    license_country VARCHAR(3) NOT NULL DEFAULT 'BD',
    primary_specialty VARCHAR(80) NOT NULL,
    experience_years SMALLINT NOT NULL CHECK (experience_years >= 0),
    current_hospital VARCHAR(200) NOT NULL,
    qualifications TEXT[] NOT NULL DEFAULT '{}',
    consultation_fee_video NUMERIC(10, 2) NOT NULL CHECK (consultation_fee_video >= 0),
    consultation_fee_chat NUMERIC(10, 2) NOT NULL CHECK (consultation_fee_chat >= 0),
    residential_address TEXT NOT NULL,
    verified_phone VARCHAR(20) NOT NULL,
    is_on_duty BOOLEAN NOT NULL DEFAULT FALSE,
    is_verified BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    deleted_at TIMESTAMPTZ
);

CREATE INDEX idx_doctor_specialty ON doctor_profiles(primary_specialty);
CREATE INDEX idx_doctor_license ON doctor_profiles(license_number);
CREATE INDEX idx_doctor_duty ON doctor_profiles(is_on_duty) WHERE is_on_duty = TRUE;

-- ============================================================================
-- 4. DOCTOR SCHEDULE SLOTS
-- ============================================================================
CREATE TYPE slot_status_enum AS ENUM ('AVAILABLE', 'LOCKED_IN_PAYMENT', 'BOOKED', 'CANCELLED');

CREATE TABLE doctor_schedule_slots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    doctor_id UUID NOT NULL REFERENCES doctor_profiles(id) ON DELETE CASCADE,
    start_time TIMESTAMPTZ NOT NULL,
    end_time TIMESTAMPTZ NOT NULL,
    status slot_status_enum NOT NULL DEFAULT 'AVAILABLE',
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT chk_slot_time CHECK (end_time > start_time),
    CONSTRAINT no_overlapping_slots EXCLUDE USING gist (
        doctor_id WITH =,
        tstzrange(start_time, end_time) WITH &&
    ) WHERE (status != 'CANCELLED')
);

CREATE INDEX idx_slots_doctor_date ON doctor_schedule_slots(doctor_id, start_time);
CREATE INDEX idx_slots_available ON doctor_schedule_slots(doctor_id, status) WHERE status = 'AVAILABLE';

-- ============================================================================
-- 5. APPOINTMENTS
-- ============================================================================
CREATE TYPE appointment_status_enum AS ENUM (
    'PENDING_PAYMENT', 'CONFIRMED', 'WAITING', 'IN_CONSULTATION', 'COMPLETED', 'CANCELLED', 'EXPIRED', 'PREMATURE_TERMINATION', 'DISPUTED', 'REFUNDED'
);
CREATE TYPE modality_enum AS ENUM ('VIDEO', 'CHAT');

CREATE TABLE appointments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    appointment_number VARCHAR(40) NOT NULL UNIQUE,
    patient_id UUID NOT NULL REFERENCES patient_profiles(id) ON DELETE RESTRICT,
    doctor_id UUID NOT NULL REFERENCES doctor_profiles(id) ON DELETE RESTRICT,
    slot_id UUID NOT NULL REFERENCES doctor_schedule_slots(id) ON DELETE RESTRICT,
    modality modality_enum NOT NULL DEFAULT 'VIDEO',
    status appointment_status_enum NOT NULL DEFAULT 'PENDING_PAYMENT',
    consultation_fee NUMERIC(10, 2) NOT NULL CHECK (consultation_fee >= 0),
    clinical_outcome VARCHAR(50) CHECK (
        clinical_outcome IS NULL
        OR clinical_outcome IN ('COMPLETED_WITH_RX', 'COMPLETED_NO_RX', 'REFERRED', 'ESCALATED')
    ),
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_appointments_patient ON appointments(patient_id, status);
CREATE INDEX idx_appointments_doctor ON appointments(doctor_id, status);

-- ============================================================================
-- 5b. APPOINTMENT CLINICAL INTAKE (PHI-separated from operational data)
-- ============================================================================
CREATE TABLE appointment_clinical_intake (
    appointment_id UUID PRIMARY KEY REFERENCES appointments(id) ON DELETE RESTRICT,
    chief_complaint TEXT,
    clinical_notes TEXT, -- Doctor's internal notes during consultation
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

ALTER TABLE appointment_clinical_intake ENABLE ROW LEVEL SECURITY;
ALTER TABLE appointment_clinical_intake FORCE ROW LEVEL SECURITY;

-- Only patient, assigned doctor, and clinical/compliance admins can see PHI
CREATE POLICY clinical_intake_select ON appointment_clinical_intake
    FOR SELECT USING (
        appointment_id IN (
            SELECT id FROM appointments WHERE
                patient_id IN (SELECT id FROM patient_profiles WHERE user_id = current_setting('app.current_user_id', true)::UUID)
                OR doctor_id IN (SELECT id FROM doctor_profiles WHERE user_id = current_setting('app.current_user_id', true)::UUID)
        )
        OR current_setting('app.current_role', true) IN ('CLINICAL_ADMIN', 'COMPLIANCE')
    );

-- Note: FINANCE_ADMIN, SUPPORT, SECURITY_ADMIN cannot see this table.
-- They access the operational appointments table which has fee, status, slot
-- but no clinical text.

-- ============================================================================
-- 6. MULTI-PRESCRIPTION INTAKE DOCUMENTS (MAX 5 IMAGES PER APPOINTMENT)
-- ============================================================================
CREATE TABLE prescription_intake_documents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    appointment_id UUID NOT NULL REFERENCES appointments(id) ON DELETE RESTRICT,
    patient_id UUID NOT NULL REFERENCES patient_profiles(id) ON DELETE RESTRICT,
    page_number SMALLINT NOT NULL CHECK (page_number >= 1 AND page_number <= 5),
    object_bucket VARCHAR(100) NOT NULL,
    object_key VARCHAR(500) NOT NULL,
    content_hash VARCHAR(128),
    file_size_bytes BIGINT NOT NULL CHECK (file_size_bytes <= 10485760), -- 10 MB limit
    mime_type VARCHAR(50) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uq_appointment_page UNIQUE (appointment_id, page_number)
);

CREATE INDEX idx_prescription_intake_apt ON prescription_intake_documents(appointment_id);

-- ============================================================================
-- 7. CONSULTATION SESSIONS & TELEMETRY
-- ============================================================================
CREATE TYPE session_status_enum AS ENUM ('INITIALIZED', 'ACTIVE', 'COMPLETED', 'PREMATURE_TERMINATION');

CREATE TABLE consultation_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    appointment_id UUID NOT NULL UNIQUE REFERENCES appointments(id) ON DELETE RESTRICT,
    agora_channel_name VARCHAR(100) NOT NULL UNIQUE,
    started_at TIMESTAMPTZ,
    ended_at TIMESTAMPTZ,
    status session_status_enum NOT NULL DEFAULT 'INITIALIZED',
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE consultation_telemetry (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id UUID NOT NULL UNIQUE REFERENCES consultation_sessions(id) ON DELETE CASCADE,
    call_duration_seconds INT NOT NULL DEFAULT 0,
    connection_state VARCHAR(50) NOT NULL,
    packet_loss_percent NUMERIC(5, 2) DEFAULT 0.00,
    round_trip_time_ms INT DEFAULT 0,
    premature_end BOOLEAN NOT NULL DEFAULT FALSE,
    prescription_issued BOOLEAN NOT NULL DEFAULT FALSE,
    captured_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- ============================================================================
-- 8. HANDWRITTEN PRESCRIPTION PHOTOS
-- ============================================================================
CREATE TABLE prescriptions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    appointment_id UUID NOT NULL UNIQUE REFERENCES appointments(id) ON DELETE RESTRICT,
    doctor_id UUID NOT NULL REFERENCES doctor_profiles(id) ON DELETE RESTRICT,
    patient_id UUID NOT NULL REFERENCES patient_profiles(id) ON DELETE RESTRICT,
    rx_number VARCHAR(50) NOT NULL UNIQUE,
    integrity_verification_hash VARCHAR(128),
    rx_image_object_bucket VARCHAR(100),
    rx_image_object_key VARCHAR(500),
    doctor_notes TEXT,
    is_no_rx_required BOOLEAN NOT NULL DEFAULT FALSE,
    no_rx_reason TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Note: prescription_items table is removed because prescriptions are photo-based, not digitally composed.


-- ============================================================================
-- 9. ASYNCHRONOUS CLINICAL CHATS (24-HOUR WINDOW)
-- ============================================================================
CREATE TABLE chat_conversations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    appointment_id UUID NOT NULL UNIQUE REFERENCES appointments(id) ON DELETE RESTRICT,
    patient_id UUID NOT NULL REFERENCES patient_profiles(id),
    doctor_id UUID NOT NULL REFERENCES doctor_profiles(id),
    expires_at TIMESTAMPTZ NOT NULL,
    is_locked BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE chat_messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    conversation_id UUID NOT NULL REFERENCES chat_conversations(id) ON DELETE CASCADE,
    sender_id UUID NOT NULL REFERENCES users(id),
    content TEXT NOT NULL,
    attachment_bucket VARCHAR(100),
    attachment_object_key VARCHAR(500),
    attachment_mime VARCHAR(50),
    attachment_size BIGINT,
    attachment_hash VARCHAR(128),
    is_read BOOLEAN NOT NULL DEFAULT FALSE,
    sent_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_chat_messages_conv ON chat_messages(conversation_id, sent_at);

-- ============================================================================
-- 10. CLINICAL GRIEVANCES & ADJUDICATION
-- ============================================================================
CREATE TYPE grievance_target_enum AS ENUM ('DOCTOR', 'SYSTEM');
CREATE TYPE grievance_status_enum AS ENUM ('PENDING_REVIEW', 'UNDER_INVESTIGATION', 'REFUNDED', 'WARNED', 'DISMISSED');

CREATE TABLE grievance_reports (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    grievance_number VARCHAR(50) NOT NULL UNIQUE,
    patient_id UUID NOT NULL REFERENCES patient_profiles(id) ON DELETE RESTRICT,
    consultation_id UUID NOT NULL REFERENCES consultation_sessions(id) ON DELETE RESTRICT,
    target_type grievance_target_enum NOT NULL,
    category VARCHAR(150) NOT NULL,
    claim_summary TEXT NOT NULL,
    status grievance_status_enum NOT NULL DEFAULT 'PENDING_REVIEW',
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE grievance_adjudications (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    grievance_id UUID NOT NULL UNIQUE REFERENCES grievance_reports(id) ON DELETE CASCADE,
    adjudicated_by_admin_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    board_remedy VARCHAR(200) NOT NULL,
    action_type VARCHAR(50) NOT NULL,
    audit_notes TEXT NOT NULL,
    adjudicated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- ============================================================================
-- 11. TRANSACTIONS & 20% DEBARRED WALLET LEDGER
-- ============================================================================
CREATE TYPE gateway_enum AS ENUM ('BKASH', 'NAGAD', 'CARD', 'MPESA');
CREATE TYPE payment_status_enum AS ENUM ('INITIATED', 'PAYMENT_HELD', 'SETTLED_TO_DOCTOR', 'DISBURSED', 'REFUNDED_TO_PATIENT', 'FAILED');

CREATE TABLE transactions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    transaction_number VARCHAR(50) NOT NULL UNIQUE,
    appointment_id UUID NOT NULL REFERENCES appointments(id) ON DELETE RESTRICT,
    payment_session_id UUID NOT NULL DEFAULT gen_random_uuid() UNIQUE,
    gateway gateway_enum NOT NULL,
    gateway_reference VARCHAR(100),
    gross_amount NUMERIC(10, 2) NOT NULL CHECK (gross_amount >= 0),
    platform_fee_amount NUMERIC(10, 2) NOT NULL CHECK (platform_fee_amount >= 0), -- Strictly 20%
    net_amount NUMERIC(10, 2) NOT NULL CHECK (net_amount >= 0),                  -- Strictly 80%
    payment_status payment_status_enum NOT NULL DEFAULT 'INITIATED',
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    settled_at TIMESTAMPTZ,
    CONSTRAINT chk_fee_math CHECK (gross_amount = platform_fee_amount + net_amount)
);
CREATE UNIQUE INDEX uq_transactions_gateway_reference
ON transactions(gateway, gateway_reference)
WHERE gateway_reference IS NOT NULL;

CREATE TABLE doctor_wallets (
    doctor_id UUID PRIMARY KEY REFERENCES doctor_profiles(id) ON DELETE RESTRICT,
    lifetime_gross NUMERIC(12, 2) NOT NULL DEFAULT 0.00,
    lifetime_fee_withheld NUMERIC(12, 2) NOT NULL DEFAULT 0.00,
    lifetime_net NUMERIC(12, 2) NOT NULL DEFAULT 0.00,
    pending_disbursement NUMERIC(12, 2) NOT NULL DEFAULT 0.00 CHECK (pending_disbursement >= 0),
    last_disbursement_at TIMESTAMPTZ,
    last_disbursement_amount NUMERIC(12, 2) NOT NULL DEFAULT 0.00,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
-- Doctors view earnings only; they cannot withdraw. Finance Admin disburses monthly.

CREATE TABLE disbursement_batches (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    batch_number VARCHAR(50) NOT NULL UNIQUE,
    initiated_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    period_start DATE NOT NULL,
    period_end DATE NOT NULL,
    total_doctors INT NOT NULL DEFAULT 0,
    total_gross NUMERIC(12, 2) NOT NULL DEFAULT 0.00,
    total_platform_fee NUMERIC(12, 2) NOT NULL DEFAULT 0.00,
    total_net_disbursed NUMERIC(12, 2) NOT NULL DEFAULT 0.00,
    status VARCHAR(20) NOT NULL DEFAULT 'PENDING', -- PENDING, PROCESSING, COMPLETED, FAILED
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at TIMESTAMPTZ
);

CREATE TABLE disbursement_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    batch_id UUID NOT NULL REFERENCES disbursement_batches(id) ON DELETE RESTRICT,
    doctor_id UUID NOT NULL REFERENCES doctor_profiles(id) ON DELETE RESTRICT,
    gross_amount NUMERIC(10, 2) NOT NULL,
    platform_fee NUMERIC(10, 2) NOT NULL,
    net_amount NUMERIC(10, 2) NOT NULL,
    gateway gateway_enum NOT NULL,
    gateway_reference VARCHAR(100),
    status VARCHAR(20) NOT NULL DEFAULT 'PENDING', -- PENDING, SENT, CONFIRMED, FAILED
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    confirmed_at TIMESTAMPTZ
);
CREATE INDEX idx_disbursement_items_batch ON disbursement_items(batch_id);
CREATE INDEX idx_disbursement_items_doctor ON disbursement_items(doctor_id);

CREATE TABLE payment_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    transaction_id UUID NOT NULL REFERENCES transactions(id) ON DELETE RESTRICT,
    gateway gateway_enum NOT NULL,
    provider_event_id VARCHAR(150) NOT NULL,
    provider_event_type VARCHAR(50) NOT NULL,
    event_type VARCHAR(50) NOT NULL, -- normalized internal event name
    gateway_reference VARCHAR(100),
    gateway_response_code VARCHAR(20),
    amount NUMERIC(10,2),
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_payment_events_txn ON payment_events(transaction_id, created_at);
CREATE UNIQUE INDEX uq_payment_events_provider_id
ON payment_events(gateway, provider_event_id);

-- ============================================================================
-- 12. AUDIT LOGS & SUBSYSTEM INCIDENTS
-- ============================================================================
CREATE TABLE system_error_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    trace_id VARCHAR(64) NOT NULL UNIQUE,
    subsystem VARCHAR(50) NOT NULL, -- 'MFS_BKASH_GATEWAY', 'AGORA_RTC', etc.
    component VARCHAR(100) NOT NULL,
    severity VARCHAR(20) NOT NULL,  -- 'INFO', 'WARN', 'ERROR', 'CRITICAL'
    action_attempted VARCHAR(150) NOT NULL,
    error_message TEXT NOT NULL,
    stack_trace TEXT,
    impacted_user_id UUID REFERENCES users(id),
    is_resolved BOOLEAN NOT NULL DEFAULT FALSE,
    logged_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_system_logs_subsystem ON system_error_logs(subsystem, is_resolved);
CREATE INDEX idx_system_logs_trace ON system_error_logs(trace_id);

CREATE TABLE audit_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    actor_id UUID REFERENCES users(id),
    actor_role user_role_enum NOT NULL,
    action VARCHAR(100) NOT NULL,
    resource_type VARCHAR(50) NOT NULL,
    resource_id UUID,
    session_id UUID REFERENCES auth_sessions(id),
    request_id VARCHAR(64),
    ip_hash VARCHAR(64),
    result VARCHAR(20) NOT NULL DEFAULT 'SUCCESS',
    reason TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_audit_actor ON audit_events(actor_id, created_at);
CREATE INDEX idx_audit_resource ON audit_events(resource_type, resource_id);
CREATE INDEX idx_audit_action ON audit_events(action, created_at);

-- Immutability enforcement:
-- The application database role (hellodoctor_app) must be granted
-- INSERT and SELECT only on audit_events. No UPDATE or DELETE.
--
-- GRANT INSERT, SELECT ON audit_events TO hellodoctor_app;
-- REVOKE UPDATE, DELETE ON audit_events FROM hellodoctor_app;
--
-- For stronger tamper-evidence in production, consider:
-- - Periodic hash-chain integrity checks
-- - Append-only archival to immutable storage (e.g., S3 Object Lock)

CREATE TABLE idempotency_records (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    key_hash VARCHAR(128) NOT NULL,
    user_id UUID NOT NULL REFERENCES users(id),
    route VARCHAR(200) NOT NULL,
    request_hash VARCHAR(128) NOT NULL,
    response_status SMALLINT NOT NULL,
    response_body_ref UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMPTZ NOT NULL
);
CREATE UNIQUE INDEX idx_idempotency_key
ON idempotency_records(user_id, route, key_hash);

-- Expired rows cleaned by periodic maintenance job:
-- DELETE FROM idempotency_records WHERE expires_at < now();

-- ============================================================================
-- 13. ROW LEVEL SECURITY (RLS) POLICIES
-- ============================================================================
-- IMPORTANT: Application must use SET LOCAL within transactions:
--   SET LOCAL app.current_user_id = '<uuid>';
--   SET LOCAL app.current_role = '<role>';
--   SET LOCAL app.current_patient_id = '<patient-profile-uuid-or-empty>';
--   SET LOCAL app.current_doctor_id = '<doctor-profile-uuid-or-empty>';
-- SET LOCAL automatically resets when the transaction ends,
-- preventing identity leakage across pooled connections.
--
-- Design principle: Application-layer ABAC is authoritative.
-- RLS provides defense-in-depth for selected PHI tables.
-- RLS policies do not encode the entire ABAC engine but must
-- never contradict it.

-- Capability roles are NOLOGIN roles assumed only by separate, authenticated
-- service processes/connections. End-user JWT claims can never select a DB role.
-- None of these roles receives BYPASSRLS or table ownership.
CREATE ROLE hellodoctor_api NOLOGIN;
CREATE ROLE hellodoctor_auth_worker NOLOGIN;
CREATE ROLE hellodoctor_payment_worker NOLOGIN;
CREATE ROLE hellodoctor_system_worker NOLOGIN;
CREATE ROLE hellodoctor_finance_worker NOLOGIN;

REVOKE ALL ON ALL TABLES IN SCHEMA public FROM PUBLIC;
GRANT SELECT ON users, patient_profiles, doctor_profiles, doctor_schedule_slots,
    appointments, appointment_clinical_intake, prescriptions,
    prescription_intake_documents, consultation_sessions,
    chat_conversations, chat_messages, grievance_reports, transactions,
    doctor_wallets, disbursement_items TO hellodoctor_api;
GRANT UPDATE ON patient_profiles TO hellodoctor_api;
GRANT INSERT ON appointments, appointment_clinical_intake,
    prescriptions, prescription_intake_documents, chat_conversations,
    chat_messages, grievance_reports TO hellodoctor_api;
GRANT SELECT, INSERT, UPDATE ON users, auth_sessions, mfa_credentials
    TO hellodoctor_auth_worker;
GRANT SELECT, UPDATE ON appointments, doctor_schedule_slots, transactions
    TO hellodoctor_payment_worker;
GRANT INSERT ON appointments, appointment_clinical_intake, transactions, payment_events TO hellodoctor_payment_worker;
GRANT SELECT, UPDATE ON appointments, chat_conversations, chat_messages
    TO hellodoctor_system_worker;
GRANT SELECT, UPDATE ON grievance_reports, transactions, doctor_wallets,
    disbursement_batches, disbursement_items TO hellodoctor_finance_worker;
GRANT INSERT ON grievance_adjudications, disbursement_batches,
    disbursement_items, payment_events TO hellodoctor_finance_worker;

-- Enable and FORCE RLS across all sensitive PHI, clinical, and financial tables
ALTER TABLE patient_profiles ENABLE ROW LEVEL SECURITY;
ALTER TABLE patient_profiles FORCE ROW LEVEL SECURITY;

ALTER TABLE appointments ENABLE ROW LEVEL SECURITY;
ALTER TABLE appointments FORCE ROW LEVEL SECURITY;

ALTER TABLE appointment_clinical_intake ENABLE ROW LEVEL SECURITY;
ALTER TABLE appointment_clinical_intake FORCE ROW LEVEL SECURITY;

ALTER TABLE prescriptions ENABLE ROW LEVEL SECURITY;
ALTER TABLE prescriptions FORCE ROW LEVEL SECURITY;

ALTER TABLE prescription_intake_documents ENABLE ROW LEVEL SECURITY;
ALTER TABLE prescription_intake_documents FORCE ROW LEVEL SECURITY;

ALTER TABLE chat_conversations ENABLE ROW LEVEL SECURITY;
ALTER TABLE chat_conversations FORCE ROW LEVEL SECURITY;

ALTER TABLE chat_messages ENABLE ROW LEVEL SECURITY;
ALTER TABLE chat_messages FORCE ROW LEVEL SECURITY;

ALTER TABLE doctor_wallets ENABLE ROW LEVEL SECURITY;
ALTER TABLE doctor_wallets FORCE ROW LEVEL SECURITY;

ALTER TABLE grievance_reports ENABLE ROW LEVEL SECURITY;
ALTER TABLE grievance_reports FORCE ROW LEVEL SECURITY;

ALTER TABLE transactions ENABLE ROW LEVEL SECURITY;
ALTER TABLE transactions FORCE ROW LEVEL SECURITY;

ALTER TABLE disbursement_items ENABLE ROW LEVEL SECURITY;
ALTER TABLE disbursement_items FORCE ROW LEVEL SECURITY;

-- 1. patient_profiles Policies
CREATE POLICY patient_profiles_select ON patient_profiles
    FOR SELECT USING (
        user_id = current_setting('app.current_user_id', true)::UUID
        OR current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'COMPLIANCE')
        OR (
            current_setting('app.current_role', true) = 'DOCTOR'
            AND id IN (
                SELECT patient_id FROM appointments
                WHERE doctor_id = (
                    SELECT id FROM doctor_profiles
                    WHERE user_id = current_setting('app.current_user_id', true)::UUID
                )
                AND (
                    status IN ('CONFIRMED', 'WAITING', 'IN_CONSULTATION')
                    OR (status = 'COMPLETED' AND updated_at > CURRENT_TIMESTAMP - INTERVAL '24 hours')
                )
            )
        )
    );

CREATE POLICY patient_profiles_update ON patient_profiles
    FOR UPDATE USING (
        user_id = current_setting('app.current_user_id', true)::UUID
    ) WITH CHECK (
        user_id = current_setting('app.current_user_id', true)::UUID
    );

-- Support has no direct UPDATE permission on clinical/demographic profiles.
-- Approved corrections use audited, field-specific service operations.

-- 2. appointments Policies
CREATE POLICY appointments_select ON appointments
    FOR SELECT USING (
        patient_id = NULLIF(current_setting('app.current_patient_id', true), '')::UUID
        OR doctor_id = NULLIF(current_setting('app.current_doctor_id', true), '')::UUID
        OR current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'FINANCE_ADMIN', 'SUPPORT', 'COMPLIANCE')
    );

CREATE POLICY appointments_insert ON appointments
    FOR INSERT WITH CHECK (
        patient_id = NULLIF(current_setting('app.current_patient_id', true), '')::UUID
        OR current_setting('app.current_role', true) = 'PLATFORM_ADMIN'
    );

-- Operational appointment rows contain no free-text clinical intake. Finance
-- and Support may read them through ABAC-filtered endpoints; SECURITY_ADMIN may not.
CREATE POLICY appointment_clinical_intake_select ON appointment_clinical_intake
    FOR SELECT USING (
        patient_id = NULLIF(current_setting('app.current_patient_id', true), '')::UUID
        OR appointment_id IN (
            SELECT id FROM appointments
            WHERE doctor_id = NULLIF(current_setting('app.current_doctor_id', true), '')::UUID
              AND (
                  status IN ('CONFIRMED', 'WAITING', 'IN_CONSULTATION')
                  OR (status = 'COMPLETED' AND updated_at > CURRENT_TIMESTAMP - INTERVAL '24 hours')
              )
        )
        OR current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'COMPLIANCE')
    );

CREATE POLICY appointment_clinical_intake_insert ON appointment_clinical_intake
    FOR INSERT WITH CHECK (
        patient_id = NULLIF(current_setting('app.current_patient_id', true), '')::UUID
    );

-- 3. prescriptions Policies
CREATE POLICY prescriptions_select ON prescriptions
    FOR SELECT USING (
        patient_id IN (SELECT id FROM patient_profiles WHERE user_id = current_setting('app.current_user_id', true)::UUID)
        OR appointment_id IN (
            SELECT id FROM appointments
            WHERE doctor_id = NULLIF(current_setting('app.current_doctor_id', true), '')::UUID
              AND (
                  status IN ('CONFIRMED', 'WAITING', 'IN_CONSULTATION')
                  OR (status = 'COMPLETED' AND updated_at > CURRENT_TIMESTAMP - INTERVAL '24 hours')
              )
        )
        OR current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'COMPLIANCE')
    );

CREATE POLICY prescriptions_insert ON prescriptions
    FOR INSERT WITH CHECK (
        doctor_id IN (SELECT id FROM doctor_profiles WHERE user_id = current_setting('app.current_user_id', true)::UUID)
        AND current_setting('app.current_role', true) = 'DOCTOR'
    );

-- 4. prescription_intake_documents Policies
CREATE POLICY prescription_intake_docs_select ON prescription_intake_documents
    FOR SELECT USING (
        patient_id IN (SELECT id FROM patient_profiles WHERE user_id = current_setting('app.current_user_id', true)::UUID)
        OR appointment_id IN (
            SELECT id FROM appointments
            WHERE doctor_id = NULLIF(current_setting('app.current_doctor_id', true), '')::UUID
              AND (
                  status IN ('CONFIRMED', 'WAITING', 'IN_CONSULTATION')
                  OR (status = 'COMPLETED' AND updated_at > CURRENT_TIMESTAMP - INTERVAL '24 hours')
              )
        )
        OR current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'COMPLIANCE')
    );

CREATE POLICY prescription_intake_docs_insert ON prescription_intake_documents
    FOR INSERT WITH CHECK (
        patient_id IN (SELECT id FROM patient_profiles WHERE user_id = current_setting('app.current_user_id', true)::UUID)
    );

-- 5. chat_conversations & chat_messages Policies
CREATE POLICY chat_conversations_select ON chat_conversations
    FOR SELECT USING (
        patient_id IN (SELECT id FROM patient_profiles WHERE user_id = current_setting('app.current_user_id', true)::UUID)
        OR doctor_id IN (SELECT id FROM doctor_profiles WHERE user_id = current_setting('app.current_user_id', true)::UUID)
        OR current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'COMPLIANCE')
    );

CREATE POLICY chat_messages_select ON chat_messages
    FOR SELECT USING (
        conversation_id IN (
            SELECT id FROM chat_conversations WHERE
                patient_id IN (SELECT id FROM patient_profiles WHERE user_id = current_setting('app.current_user_id', true)::UUID)
                OR doctor_id IN (SELECT id FROM doctor_profiles WHERE user_id = current_setting('app.current_user_id', true)::UUID)
        )
        OR current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'COMPLIANCE')
    );

CREATE POLICY chat_messages_insert ON chat_messages
    FOR INSERT WITH CHECK (
        sender_id = current_setting('app.current_user_id', true)::UUID
        AND conversation_id IN (
            SELECT id FROM chat_conversations WHERE (
                patient_id IN (SELECT id FROM patient_profiles WHERE user_id = current_setting('app.current_user_id', true)::UUID)
                OR doctor_id IN (SELECT id FROM doctor_profiles WHERE user_id = current_setting('app.current_user_id', true)::UUID)
            )
            AND is_locked = FALSE
            AND expires_at > CURRENT_TIMESTAMP
        )
    );

-- 6. doctor_wallets Policies
CREATE POLICY doctor_wallets_select ON doctor_wallets
    FOR SELECT USING (
        doctor_id IN (SELECT id FROM doctor_profiles WHERE user_id = current_setting('app.current_user_id', true)::UUID)
        OR current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'FINANCE_ADMIN')
    );

-- 7. grievance_reports Policies
CREATE POLICY grievance_reports_select ON grievance_reports
    FOR SELECT USING (
        patient_id IN (SELECT id FROM patient_profiles WHERE user_id = current_setting('app.current_user_id', true)::UUID)
        OR current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'COMPLIANCE', 'SUPPORT')
    );

CREATE POLICY grievance_reports_insert ON grievance_reports
    FOR INSERT WITH CHECK (
        patient_id IN (SELECT id FROM patient_profiles WHERE user_id = current_setting('app.current_user_id', true)::UUID)
        AND current_setting('app.current_role', true) = 'PATIENT'
    );

-- 8. transactions & disbursement_items Policies
CREATE POLICY transactions_select ON transactions
    FOR SELECT USING (
        appointment_id IN (
            SELECT id FROM appointments WHERE
                patient_id IN (SELECT id FROM patient_profiles WHERE user_id = current_setting('app.current_user_id', true)::UUID)
                OR doctor_id IN (SELECT id FROM doctor_profiles WHERE user_id = current_setting('app.current_user_id', true)::UUID)
        )
        OR current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'FINANCE_ADMIN')
    );

CREATE POLICY disbursement_items_select ON disbursement_items
    FOR SELECT USING (
        doctor_id IN (SELECT id FROM doctor_profiles WHERE user_id = current_setting('app.current_user_id', true)::UUID)
        OR current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'FINANCE_ADMIN')
    );

-- ============================================================================
-- 13b. SERVICE-ROLE WRITE POLICIES
-- ============================================================================
-- The application uses multiple logical service contexts via SET LOCAL:
--   app.current_role = 'SERVICE_API'          -- Normal API request handler
--   app.current_role = 'SERVICE_PAYMENT'      -- Payment webhook processor
--   app.current_role = 'SERVICE_SETTLEMENT'   -- Consultation completion & settlement
--   app.current_role = 'SERVICE_SYSTEM'       -- Background jobs (chat lock, slot expiry)
--   app.current_role = 'SERVICE_FINANCE'      -- Monthly disbursement batch
--
-- These are NOT user-facing roles. They are internal service contexts
-- set by the Axum middleware when processing internal/system operations.
-- They are added to user_role_enum is NOT needed; they are checked only
-- in RLS policies via current_setting('app.current_role').

-- Appointments: API and payment worker can update status
CREATE POLICY appointments_service_update ON appointments
    FOR UPDATE USING (
        current_setting('app.current_role', true) IN (
            'SERVICE_API', 'SERVICE_PAYMENT', 'SERVICE_SETTLEMENT', 'SERVICE_SYSTEM',
            'PLATFORM_ADMIN', 'SUPPORT'
        )
    );

-- Slots: API and payment worker can update status
CREATE POLICY slots_service_update ON doctor_schedule_slots
    FOR UPDATE USING (
        current_setting('app.current_role', true) IN (
            'SERVICE_API', 'SERVICE_PAYMENT', 'SERVICE_SYSTEM',
            'PLATFORM_ADMIN'
        )
    );
ALTER TABLE doctor_schedule_slots ENABLE ROW LEVEL SECURITY;
ALTER TABLE doctor_schedule_slots FORCE ROW LEVEL SECURITY;

CREATE POLICY slots_select ON doctor_schedule_slots
    FOR SELECT USING (true); -- Slot availability is public information

CREATE POLICY slots_insert ON doctor_schedule_slots
    FOR INSERT WITH CHECK (
        doctor_id IN (SELECT id FROM doctor_profiles WHERE user_id = current_setting('app.current_user_id', true)::UUID)
        OR current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'SERVICE_SYSTEM')
    );

-- Transactions: Payment and settlement workers update payment_status
CREATE POLICY transactions_service_update ON transactions
    FOR UPDATE USING (
        current_setting('app.current_role', true) IN (
            'SERVICE_PAYMENT', 'SERVICE_SETTLEMENT', 'SERVICE_FINANCE',
            'FINANCE_ADMIN', 'PLATFORM_ADMIN'
        )
    );

CREATE POLICY transactions_service_insert ON transactions
    FOR INSERT WITH CHECK (
        current_setting('app.current_role', true) IN (
            'SERVICE_API', 'SERVICE_PAYMENT',
            'PLATFORM_ADMIN'
        )
    );

-- Grievances: Clinical admin and compliance can update status
CREATE POLICY grievance_reports_service_update ON grievance_reports
    FOR UPDATE USING (
        current_setting('app.current_role', true) IN (
            'CLINICAL_ADMIN', 'COMPLIANCE', 'PLATFORM_ADMIN'
        )
    );

-- Chat: System worker locks expired conversations
CREATE POLICY chat_conversations_service_update ON chat_conversations
    FOR UPDATE USING (
        current_setting('app.current_role', true) IN (
            'SERVICE_SYSTEM', 'PLATFORM_ADMIN'
        )
    );

CREATE POLICY chat_conversations_insert ON chat_conversations
    FOR INSERT WITH CHECK (
        current_setting('app.current_role', true) IN (
            'SERVICE_API', 'SERVICE_SETTLEMENT', 'PLATFORM_ADMIN'
        )
    );

-- Doctor wallets: Settlement and finance workers update balances
CREATE POLICY doctor_wallets_service_update ON doctor_wallets
    FOR UPDATE USING (
        current_setting('app.current_role', true) IN (
            'SERVICE_SETTLEMENT', 'SERVICE_FINANCE',
            'FINANCE_ADMIN', 'PLATFORM_ADMIN'
        )
    );

-- Disbursement items: Finance worker updates
CREATE POLICY disbursement_items_service_update ON disbursement_items
    FOR UPDATE USING (
        current_setting('app.current_role', true) IN (
            'SERVICE_FINANCE', 'FINANCE_ADMIN', 'PLATFORM_ADMIN'
        )
    );

CREATE POLICY disbursement_items_insert ON disbursement_items
    FOR INSERT WITH CHECK (
        current_setting('app.current_role', true) IN (
            'SERVICE_FINANCE', 'FINANCE_ADMIN', 'PLATFORM_ADMIN'
        )
    );

-- Prescriptions: Doctor inserts via service context
CREATE POLICY prescriptions_service_insert ON prescriptions
    FOR INSERT WITH CHECK (
        current_setting('app.current_role', true) IN ('SERVICE_API', 'DOCTOR')
    );

-- Clinical intake: Doctor and API service can insert and update
CREATE POLICY clinical_intake_insert ON appointment_clinical_intake
    FOR INSERT WITH CHECK (
        current_setting('app.current_role', true) IN ('SERVICE_API', 'DOCTOR', 'PATIENT')
    );

CREATE POLICY clinical_intake_update ON appointment_clinical_intake
    FOR UPDATE USING (
        current_setting('app.current_role', true) IN ('SERVICE_API', 'DOCTOR', 'CLINICAL_ADMIN')
    );
CREATE POLICY appointments_payment_worker_insert ON appointments
    FOR INSERT TO hellodoctor_payment_worker WITH CHECK (TRUE);
CREATE POLICY appointment_clinical_intake_payment_worker_insert ON appointment_clinical_intake
    FOR INSERT TO hellodoctor_payment_worker WITH CHECK (TRUE);
CREATE POLICY appointments_payment_worker_update ON appointments
    FOR UPDATE TO hellodoctor_payment_worker USING (TRUE) WITH CHECK (TRUE);
CREATE POLICY appointments_system_worker_select ON appointments
    FOR SELECT TO hellodoctor_system_worker USING (TRUE);
CREATE POLICY appointments_system_worker_update ON appointments
    FOR UPDATE TO hellodoctor_system_worker USING (TRUE) WITH CHECK (TRUE);

CREATE POLICY transactions_payment_worker_select ON transactions
    FOR SELECT TO hellodoctor_payment_worker USING (TRUE);
CREATE POLICY transactions_payment_worker_insert ON transactions
    FOR INSERT TO hellodoctor_payment_worker WITH CHECK (TRUE);
CREATE POLICY transactions_payment_worker_update ON transactions
    FOR UPDATE TO hellodoctor_payment_worker USING (TRUE) WITH CHECK (TRUE);
CREATE POLICY transactions_finance_worker_select ON transactions
    FOR SELECT TO hellodoctor_finance_worker USING (TRUE);
CREATE POLICY transactions_finance_worker_update ON transactions
    FOR UPDATE TO hellodoctor_finance_worker USING (TRUE) WITH CHECK (TRUE);

CREATE POLICY chat_conversations_system_worker_select ON chat_conversations
    FOR SELECT TO hellodoctor_system_worker USING (TRUE);
CREATE POLICY chat_conversations_system_worker_update ON chat_conversations
    FOR UPDATE TO hellodoctor_system_worker USING (TRUE) WITH CHECK (TRUE);
CREATE POLICY chat_messages_system_worker_select ON chat_messages
    FOR SELECT TO hellodoctor_system_worker USING (TRUE);
CREATE POLICY chat_messages_system_worker_update ON chat_messages
    FOR UPDATE TO hellodoctor_system_worker USING (TRUE) WITH CHECK (TRUE);

CREATE POLICY grievance_reports_finance_worker_select ON grievance_reports
    FOR SELECT TO hellodoctor_finance_worker USING (TRUE);
CREATE POLICY grievance_reports_finance_worker_update ON grievance_reports
    FOR UPDATE TO hellodoctor_finance_worker USING (TRUE) WITH CHECK (TRUE);
CREATE POLICY doctor_wallets_finance_worker_select ON doctor_wallets
    FOR SELECT TO hellodoctor_finance_worker USING (TRUE);
CREATE POLICY doctor_wallets_finance_worker_update ON doctor_wallets
    FOR UPDATE TO hellodoctor_finance_worker USING (TRUE) WITH CHECK (TRUE);
CREATE POLICY disbursement_items_finance_worker_select ON disbursement_items
    FOR SELECT TO hellodoctor_finance_worker USING (TRUE);
CREATE POLICY disbursement_items_finance_worker_insert ON disbursement_items
    FOR INSERT TO hellodoctor_finance_worker WITH CHECK (TRUE);
CREATE POLICY disbursement_items_finance_worker_update ON disbursement_items
    FOR UPDATE TO hellodoctor_finance_worker USING (TRUE) WITH CHECK (TRUE);
```

The API role cannot perform workflow state transitions. Booking/payment code uses the payment-worker connection, timed lifecycle jobs use the system-worker connection, and adjudication/disbursement code uses the finance-worker connection. No ordinary API or administrator session receives `BYPASSRLS`; admin JWT roles remain subject to the same ABAC and RLS rules.
