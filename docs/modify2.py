import re
import sys

def process_db_schema():
    with open('c:/Users/CUBE/Desktop/helodoc old site/docs/DATABASE_SCHEMA.md', 'r', encoding='utf-8') as f:
        content = f.read()

    # FIX 1
    content = content.replace('ESCROW_HELD', 'PAYMENT_HELD')
    content = content.replace('escrow', 'payment_held')

    # FIX 4
    service_old = r'-- Service-write policies\. Table GRANTs and these RLS policies are both.*?USING \(TRUE\);'
    service_new = """-- ============================================================================
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
    );"""
    content = re.sub(service_old, service_new, content, flags=re.DOTALL)

    # FIX 5 
    content = content.replace(
        "OR current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'SUPPORT')", 
        "OR current_setting('app.current_role', true) = 'PLATFORM_ADMIN'"
    )

    # FIX 6
    old_time_bound = "status IN ('CONFIRMED', 'WAITING', 'IN_CONSULTATION', 'COMPLETED')"
    if old_time_bound in content:
        content = content.replace(old_time_bound, "(status IN ('CONFIRMED', 'WAITING', 'IN_CONSULTATION')\n                    OR (status = 'COMPLETED' AND updated_at > CURRENT_TIMESTAMP - INTERVAL '24 hours'))")
    else:
        # Check if already has completed_at
        content = content.replace("OR (status = 'COMPLETED' AND completed_at > CURRENT_TIMESTAMP - INTERVAL '24 hours')", "OR (status = 'COMPLETED' AND updated_at > CURRENT_TIMESTAMP - INTERVAL '24 hours')")


    # FIX 7
    content = content.replace("'PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'COMPLIANCE', 'SECURITY_ADMIN'", "'PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'COMPLIANCE'")

    # FIX 8
    wallet_def = """CREATE TABLE doctor_wallets (
    doctor_id UUID PRIMARY KEY REFERENCES doctor_profiles(id) ON DELETE RESTRICT,
    lifetime_gross NUMERIC(12, 2) NOT NULL DEFAULT 0.00,
    lifetime_fee_withheld NUMERIC(12, 2) NOT NULL DEFAULT 0.00,
    lifetime_net NUMERIC(12, 2) NOT NULL DEFAULT 0.00,
    pending_disbursement NUMERIC(12, 2) NOT NULL DEFAULT 0.00 CHECK (pending_disbursement >= 0),
    last_disbursement_at TIMESTAMPTZ,
    last_disbursement_amount NUMERIC(12, 2) NOT NULL DEFAULT 0.00,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);"""
    wallet_new = """CREATE TABLE doctor_wallets (
    doctor_id UUID PRIMARY KEY REFERENCES doctor_profiles(id) ON DELETE RESTRICT,
    lifetime_gross NUMERIC(12, 2) NOT NULL DEFAULT 0.00,
    lifetime_fee_withheld NUMERIC(12, 2) NOT NULL DEFAULT 0.00,
    lifetime_net NUMERIC(12, 2) NOT NULL DEFAULT 0.00,
    pending_disbursement NUMERIC(12, 2) NOT NULL DEFAULT 0.00 CHECK (pending_disbursement >= 0),
    last_disbursement_at TIMESTAMPTZ,
    last_disbursement_amount NUMERIC(12, 2) NOT NULL DEFAULT 0.00,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
-- Doctors view earnings only; they cannot withdraw. Finance Admin disburses monthly."""
    content = content.replace(wallet_def, wallet_new)

    with open('c:/Users/CUBE/Desktop/helodoc old site/docs/DATABASE_SCHEMA.md', 'w', encoding='utf-8') as f:
        f.write(content)


def process_domain_model():
    with open('c:/Users/CUBE/Desktop/helodoc old site/docs/DOMAIN_MODEL.md', 'r', encoding='utf-8') as f:
        content = f.read()

    content = content.replace('ESCROW_HELD', 'PAYMENT_HELD')

    # FIX MfaCredential
    mfa_old = """### 2.2a `MfaCredential`
- **Purpose:** Persistent administrator MFA enrollment and recovery state.
- **Fields:**
  - `id`: `UUID` (Primary Key)
  - `user_id`: `UUID` (Foreign Key -> `User.id`, Required)
  - `method`: `MfaMethod` (`TOTP`, `PASSKEY`)
  - `totp_secret_ciphertext`: `Bytes` (KMS envelope-encrypted; never logged)
  - `encryption_key_id`: `String`
  - `encryption_nonce`: `Bytes`
  - `recovery_code_hashes`: `Vec<String>` (Individually salted hashes)
  - `enabled_at`: `DateTime<Utc>` (Optional until enrollment confirmation)
  - `last_verified_at`: `DateTime<Utc>` (Optional)
  - `revoked_at`: `DateTime<Utc>` (Optional)
- **Sensitivity:** `IDENTITY` / Authentication secret."""
    mfa_new = """### 2.2a `MfaCredential`
- **Purpose:** Persistent administrator MFA enrollment and recovery state.
- **Fields:**
  - `id`: `UUID` (Primary Key)
  - `user_id`: `UUID` (Foreign Key -> `User.id`, Required)
  - `method`: `MfaMethod` (`TOTP`, `PASSKEY`)
  - `totp_secret_encrypted`: `Bytes` (KMS envelope-encrypted; never logged)
  - `recovery_codes_hash`: `Vec<String>` (Array of Argon2id hashed one-time recovery codes)
  - `is_enabled`: `Boolean` (Default: `false`)
  - `enabled_at`: `DateTime<Utc>` (Optional until enrollment confirmation)
  - `last_verified_at`: `DateTime<Utc>` (Optional)
  - `revoked_at`: `DateTime<Utc>` (Optional)
  - `created_at`: `DateTime<Utc>`
- **Sensitivity:** `IDENTITY` / Authentication secret."""
    
    if mfa_old in content:
        content = content.replace(mfa_old, mfa_new)
    else:
        print('DOMAIN MFA OLD NOT FOUND')

    # Add disbursement model
    disbursement_model = """
---

### 2.14 `DisbursementBatch` & `DisbursementItem`
- **Purpose:** Monthly settlement runs for doctor earnings.
- **Fields (`DisbursementBatch`):**
  - `id`: `UUID` (Primary Key)
  - `batch_number`: `String`
  - `initiated_by`: `UUID` (Foreign Key -> `User.id`, Required)
  - `period_start`: `Date`
  - `period_end`: `Date`
  - `total_doctors`: `i32`
  - `total_gross`: `Decimal`
  - `total_platform_fee`: `Decimal`
  - `total_net_disbursed`: `Decimal`
  - `status`: `String` (`PENDING`, `PROCESSING`, `COMPLETED`, `FAILED`)
  - `created_at`: `DateTime<Utc>`
  - `completed_at`: `DateTime<Utc>`
- **Fields (`DisbursementItem`):**
  - `id`: `UUID` (Primary Key)
  - `batch_id`: `UUID` (Foreign Key -> `DisbursementBatch.id`)
  - `doctor_id`: `UUID` (Foreign Key -> `DoctorProfile.id`)
  - `gross_amount`: `Decimal`
  - `platform_fee`: `Decimal`
  - `net_amount`: `Decimal`
  - `gateway`: `PaymentGateway`
  - `gateway_reference`: `String`
  - `status`: `String` (`PENDING`, `SENT`, `CONFIRMED`, `FAILED`)
  - `created_at`: `DateTime<Utc>`
  - `confirmed_at`: `DateTime<Utc>`
- **Sensitivity:** `FINANCIAL`
"""
    if '2.14' not in content:
        content += disbursement_model

    # Clinical Notes
    clinical_old = """### 2.7a `AppointmentClinicalIntake`
- **Purpose:** Isolates clinical free text from operational appointment/payment fields so non-clinical staff cannot read it.
- **Fields:**
  - `appointment_id`: `UUID` (Primary Key, Foreign Key -> `Appointment.id`)
  - `patient_id`: `UUID` (Foreign Key -> `PatientProfile.id`)
  - `chief_complaint`: `String` (Optional)
  - `created_at`: `DateTime<Utc>`
  - `updated_at`: `DateTime<Utc>`"""
    clinical_new = """### 2.7a `AppointmentClinicalIntake`
- **Purpose:** Isolates clinical free text from operational appointment/payment fields so non-clinical staff cannot read it.
- **Fields:**
  - `appointment_id`: `UUID` (Primary Key, Foreign Key -> `Appointment.id`)
  - `chief_complaint`: `String` (Optional)
  - `clinical_notes`: `String` (Doctor's internal notes during consultation)
  - `created_at`: `DateTime<Utc>`
  - `updated_at`: `DateTime<Utc>`"""
    if clinical_old in content:
        content = content.replace(clinical_old, clinical_new)
    else:
        print('DOMAIN CLINICAL OLD NOT FOUND')
        
    with open('c:/Users/CUBE/Desktop/helodoc old site/docs/DOMAIN_MODEL.md', 'w', encoding='utf-8') as f:
        f.write(content)

process_db_schema()
process_domain_model()
