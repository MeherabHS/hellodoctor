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
├────── (1) Prescription (1) ────── (N) PrescriptionItem                                             │
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
    'PATIENT', 'DOCTOR', 'ADMIN', 'PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'FINANCE_ADMIN', 'SUPPORT', 'COMPLIANCE'
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
    chief_complaint TEXT,
    clinical_outcome VARCHAR(50),
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_appointments_patient ON appointments(patient_id, status);
CREATE INDEX idx_appointments_doctor ON appointments(doctor_id, status);

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
-- 8. E-PRESCRIPTIONS & ITEMS
-- ============================================================================
CREATE TABLE prescriptions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    appointment_id UUID NOT NULL UNIQUE REFERENCES appointments(id) ON DELETE RESTRICT,
    doctor_id UUID NOT NULL REFERENCES doctor_profiles(id) ON DELETE RESTRICT,
    patient_id UUID NOT NULL REFERENCES patient_profiles(id) ON DELETE RESTRICT,
    rx_number VARCHAR(50) NOT NULL UNIQUE,
    diagnosis_notes TEXT NOT NULL,
    investigations_advised TEXT[] DEFAULT '{}',
    follow_up_date DATE,
    integrity_verification_hash VARCHAR(128) NOT NULL,
    pdf_object_bucket VARCHAR(100) NOT NULL,
    pdf_object_key VARCHAR(500) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE prescription_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    prescription_id UUID NOT NULL REFERENCES prescriptions(id) ON DELETE CASCADE,
    brand_name VARCHAR(150) NOT NULL,
    generic_name VARCHAR(150) NOT NULL,
    dosage_form VARCHAR(50) NOT NULL, -- Tablet, Syrup, Injection
    strength VARCHAR(50) NOT NULL,    -- 500mg, 10ml
    dosage_frequency VARCHAR(30) NOT NULL, -- 1+0+1
    duration_days SMALLINT NOT NULL CHECK (duration_days > 0),
    instructions VARCHAR(200) NOT NULL
);

CREATE INDEX idx_rx_items_prescription ON prescription_items(prescription_id);

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
    attachment_url TEXT,
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
CREATE TYPE payment_status_enum AS ENUM ('INITIATED', 'ESCROW_HELD', 'SETTLED_TO_DOCTOR', 'DISBURSED', 'REFUNDED_TO_PATIENT', 'FAILED');

CREATE TABLE transactions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    transaction_number VARCHAR(50) NOT NULL UNIQUE,
    appointment_id UUID NOT NULL REFERENCES appointments(id) ON DELETE RESTRICT,
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

CREATE TABLE doctor_wallets (
    doctor_id UUID PRIMARY KEY REFERENCES doctor_profiles(id) ON DELETE RESTRICT,
    lifetime_gross NUMERIC(12, 2) NOT NULL DEFAULT 0.00,
    lifetime_fee_withheld NUMERIC(12, 2) NOT NULL DEFAULT 0.00,
    lifetime_net NUMERIC(12, 2) NOT NULL DEFAULT 0.00,
    current_withdrawable_balance NUMERIC(12, 2) NOT NULL DEFAULT 0.00,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE payment_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    transaction_id UUID NOT NULL REFERENCES transactions(id) ON DELETE RESTRICT,
    event_type VARCHAR(50) NOT NULL,
    gateway_reference VARCHAR(100),
    gateway_response_code VARCHAR(20),
    amount NUMERIC(10,2),
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_payment_events_txn ON payment_events(transaction_id, created_at);

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
CREATE UNIQUE INDEX idx_idempotency_key ON idempotency_records(key_hash) WHERE expires_at > CURRENT_TIMESTAMP;

-- ============================================================================
-- 13. ROW LEVEL SECURITY (RLS) POLICIES
-- ============================================================================
-- Enable RLS
ALTER TABLE patient_profiles ENABLE ROW LEVEL SECURITY;
ALTER TABLE appointments ENABLE ROW LEVEL SECURITY;
ALTER TABLE prescriptions ENABLE ROW LEVEL SECURITY;

-- patient_profiles: Users can view/edit their own profiles.
CREATE POLICY patient_profiles_self_access ON patient_profiles
    FOR ALL
    USING (user_id = current_setting('app.current_user_id')::UUID);

-- appointments: Patients see their own, Doctors see theirs.
CREATE POLICY appointments_patient_access ON appointments
    FOR SELECT
    USING (patient_id IN (SELECT id FROM patient_profiles WHERE user_id = current_setting('app.current_user_id')::UUID));

CREATE POLICY appointments_doctor_access ON appointments
    FOR SELECT
    USING (doctor_id IN (SELECT id FROM doctor_profiles WHERE user_id = current_setting('app.current_user_id')::UUID));

-- prescriptions: Patients see their own, Doctors see theirs.
CREATE POLICY prescriptions_patient_access ON prescriptions
    FOR SELECT
    USING (patient_id IN (SELECT id FROM patient_profiles WHERE user_id = current_setting('app.current_user_id')::UUID));

CREATE POLICY prescriptions_doctor_access ON prescriptions
    FOR SELECT
    USING (doctor_id IN (SELECT id FROM doctor_profiles WHERE user_id = current_setting('app.current_user_id')::UUID));
```
