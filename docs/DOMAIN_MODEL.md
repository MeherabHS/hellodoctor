# Phase 5 — Domain Model & Healthcare Entity Specification

> **Document Version:** 1.0.0  
> **Target Frameworks:** Rust Domain Models (`serde`, `validator`), Flutter Models (`freezed`, `json_serializable`)  
> **Compliance Standard:** Healthcare Privacy (PHI), Medical Credentialing, Financial Auditability

---

## 1. Domain Model Overview

This document defines the domain entities reverse-engineered from the HelloDoctor platform. Every entity includes field definitions, type specifications, validation constraints, lifecycle states, and sensitive data classifications.

### Data Sensitivity Classifications
- **PHI (Protected Health Information):** High confidentiality. Requires encryption at rest and in transit. Strict role-based access.
- **FINANCIAL:** High integrity. Subject to strict double-entry ledger audits.
- **IDENTITY:** PII (Personally Identifiable Information). Subject to GDPR / local privacy regulations.
- **PUBLIC / OPERATIONAL:** Non-sensitive operational data.

---

## 2. Core Domain Entities

### 2.1 `User`
- **Purpose:** Central authentication and account entity.
- **Fields:**
  - `id`: `UUID` (Primary Key, Required)
  - `phone_number`: `String` (Required, Unique, E.164 format: `+880...` or `+254...`)
  - `email`: `String` (Optional, Unique)
  - `password_hash`: `String` (Optional, Argon2id. Nullable for OTP-only users)
  - `role`: `UserRole` (`PATIENT`, `DOCTOR`, `ADMIN`, `PLATFORM_ADMIN`, `CLINICAL_ADMIN`, `FINANCE_ADMIN`, `SUPPORT`, `COMPLIANCE`)
  - `is_active`: `Boolean` (Default: `true`)
  - `is_verified`: `Boolean` (Default: `false`)
  - `preferred_language`: `String` (Default: `'en'`, values: `'en'`, `'sw'`)
  - `created_at`: `DateTime<Utc>`
  - `updated_at`: `DateTime<Utc>`
- **Sensitivity:** `IDENTITY`
- **Ownership:** Self.
- **Lifecycle:** Created on registration -> Verified via OTP -> Deactivated upon account closure.

---

### 2.2 `AuthSession`
- **Purpose:** Secure session tracking and device fingerprinting.
- **Fields:**
  - `id`: `UUID` (Primary Key)
  - `user_id`: `UUID` (Foreign Key -> `User.id`, Required)
  - `token_family_id`: `UUID`
  - `refresh_token_hash`: `String`
  - `device_fingerprint`: `String`
  - `ip_hash`: `String`
  - `user_agent`: `String`
  - `created_at`: `DateTime<Utc>`
  - `last_used_at`: `DateTime<Utc>`
  - `rotated_at`: `DateTime<Utc>` (Optional)
  - `revoked_at`: `DateTime<Utc>` (Optional)
  - `expires_at`: `DateTime<Utc>`
- **Sensitivity:** `OPERATIONAL`

---

### 2.3 `AuditEvent`
- **Purpose:** Immutable healthcare and system audit trail.
- **Fields:**
  - `id`: `UUID` (Primary Key)
  - `actor_id`: `UUID`
  - `actor_role`: `UserRole`
  - `action`: `String`
  - `resource_type`: `String`
  - `resource_id`: `UUID`
  - `session_id`: `UUID`
  - `request_id`: `String`
  - `ip_hash`: `String`
  - `result`: `String`
  - `reason`: `String`
  - `created_at`: `DateTime<Utc>`
- **Sensitivity:** `OPERATIONAL` / Forensic

---

### 2.4 `PatientProfile`
- **Purpose:** Clinical identity and demographic details for patients.
- **Fields:**
  - `id`: `UUID` (Primary Key, Required)
  - `user_id`: `UUID` (Foreign Key -> `User.id`, Required, Unique)
  - `full_name`: `String` (Required, 2–100 chars)
  - `date_of_birth`: `Date` (Required)
  - `gender`: `Gender` (`MALE`, `FEMALE`, `OTHER`)
  - `blood_group`: `BloodGroup` (Optional: `A+`, `A-`, `B+`, `B-`, `AB+`, `AB-`, `O+`, `O-`)
  - `weight_kg`: `Decimal` (Optional, 1.0–300.0)
  - `emergency_contact_phone`: `String` (Optional)
  - `emergency_contact_relation`: `String` (Optional)
  - `created_at`: `DateTime<Utc>`
  - `updated_at`: `DateTime<Utc>`
- **Sensitivity:** `PHI` / `IDENTITY`
- **Ownership:** Patient. Accessible by consulting doctor during active appointment.

---

### 2.5 `DoctorProfile`
- **Purpose:** Professional physician profile, clinical credentials, and practice parameters.
- **Fields:**
  - `id`: `UUID` (Primary Key, Required)
  - `user_id`: `UUID` (Foreign Key -> `User.id`, Required, Unique)
  - `full_name`: `String` (Required, prefixed with "Dr.")
  - `license_number`: `String` (Required, Unique, e.g., "123456")
  - `license_authority`: `String` (Required)
  - `license_country`: `String` (Required, Default 'BD')
  - `primary_specialty`: `MedicalSpecialty` (Required)
  - `secondary_specialties`: `Vec<MedicalSpecialty>`
  - `experience_years`: `u8` (Required, 0–70)
  - `current_hospital`: `String` (Required)
  - `qualifications`: `Vec<String>`
  - `consultation_fee_video`: `Decimal` (Required)
  - `consultation_fee_chat`: `Decimal` (Required)
  - `residential_address`: `String` (Required, for verification and dossier)
  - `verified_phone`: `String` (Required)
  - `is_on_duty`: `Boolean` (Default: `false`)
  - `is_verified`: `Boolean` (Default: `false`)
  - `created_at`: `DateTime<Utc>`
  - `updated_at`: `DateTime<Utc>`
- **Sensitivity:** `OPERATIONAL` / Contact info is `IDENTITY`.
- **Ownership:** Doctor. Governed by Admin.

---

### 2.6 `DoctorScheduleSlot`
- **Purpose:** Availability blocks for teleconsultation.
- **Fields:**
  - `id`: `UUID` (Primary Key)
  - `doctor_id`: `UUID` (Foreign Key -> `DoctorProfile.id`, Required)
  - `start_time`: `DateTime<Utc>` (Required)
  - `end_time`: `DateTime<Utc>` (Required)
  - `status`: `SlotStatus` (`AVAILABLE`, `LOCKED_IN_PAYMENT`, `BOOKED`, `CANCELLED`)
  - `created_at`: `DateTime<Utc>`
- **Validation:** `end_time > start_time`. No overlapping slots for the same doctor.
- **Sensitivity:** `OPERATIONAL`

---

### 2.7 `Appointment`
- **Purpose:** Scheduled consultation booking linking patient, doctor, and slot.
- **Fields:**
  - `id`: `UUID` (Primary Key)
  - `appointment_number`: `String` (e.g., "APT-20261001-9481")
  - `patient_id`: `UUID` (Foreign Key -> `PatientProfile.id`, Required)
  - `doctor_id`: `UUID` (Foreign Key -> `DoctorProfile.id`, Required)
  - `slot_id`: `UUID` (Foreign Key -> `DoctorScheduleSlot.id`, Required)
  - `modality`: `ConsultationModality` (`VIDEO`, `CHAT`)
  - `status`: `AppointmentStatus` (`PENDING_PAYMENT`, `CONFIRMED`, `WAITING`, `IN_CONSULTATION`, `COMPLETED`, `CANCELLED`, `EXPIRED`, `PREMATURE_TERMINATION`, `DISPUTED`, `REFUNDED`)
  - `consultation_fee`: `Decimal` (Required)
  - `chief_complaint`: `String` (Optional)
  - `clinical_outcome`: `String` (Optional: 'COMPLETED_WITH_RX', 'COMPLETED_NO_RX', 'REFERRED', 'ESCALATED')
  - `created_at`: `DateTime<Utc>`
  - `updated_at`: `DateTime<Utc>`
- **Sensitivity:** `PHI` / `FINANCIAL`

---

### 2.8 `PrescriptionIntakeDocument` (Max 5 Images)
- **Purpose:** Physical prescription or lab report uploaded by patient prior to consultation.
- **Fields:**
  - `id`: `UUID` (Primary Key)
  - `appointment_id`: `UUID` (Foreign Key -> `Appointment.id`, Required)
  - `patient_id`: `UUID` (Foreign Key -> `PatientProfile.id`, Required)
  - `page_number`: `u8` (Required, 1 to 5)
  - `object_bucket`: `String` (Required)
  - `object_key`: `String` (Required)
  - `content_hash`: `String` (Optional)
  - `file_size_bytes`: `u64` (Max 10 MB)
  - `mime_type`: `String` (`image/jpeg`, `image/png`, `application/pdf`)
  - `created_at`: `DateTime<Utc>`
- **Business Rule:** Total documents per `appointment_id` must not exceed 5.
- **Sensitivity:** `PHI`

---

### 2.9 `ConsultationSession` & `ConsultationTelemetry`
- **Purpose:** Live teleconsultation record and objective technical telemetry.
- **Fields (`ConsultationSession`):**
  - `id`: `UUID` (Primary Key)
  - `appointment_id`: `UUID` (Foreign Key -> `Appointment.id`, Required, Unique)
  - `agora_channel_name`: `String` (Agora RTC channel identifier)
  - `started_at`: `DateTime<Utc>`
  - `ended_at`: `DateTime<Utc>`
  - `status`: `SessionStatus` (`INITIALIZED`, `ACTIVE`, `COMPLETED`, `PREMATURE_TERMINATION`)
- **Fields (`ConsultationTelemetry`):**
  - `id`: `UUID` (Primary Key)
  - `session_id`: `UUID` (Foreign Key -> `ConsultationSession.id`, Required, Unique)
  - `call_duration_seconds`: `u32` (Actual connected duration)
  - `connection_state`: `String` (e.g., "COMPLETED", "FAILED", "DISCONNECTED")
  - `packet_loss_percent`: `Decimal`
  - `round_trip_time_ms`: `u32`
  - `premature_end`: `Boolean` (`call_duration_seconds < 30`)
  - `prescription_issued`: `Boolean` (Default: `false`)
- **Sensitivity:** `OPERATIONAL` / Forensic Evidence.

---

### 2.10 `Prescription` & `PrescriptionItem`
- **Purpose:** Official e-prescription authored and finalized by physician.
- **Fields (`Prescription`):**
  - `id`: `UUID` (Primary Key)
  - `appointment_id`: `UUID` (Foreign Key -> `Appointment.id`, Required, Unique)
  - `doctor_id`: `UUID` (Foreign Key -> `DoctorProfile.id`, Required)
  - `patient_id`: `UUID` (Foreign Key -> `PatientProfile.id`, Required)
  - `rx_number`: `String` (Unique)
  - `diagnosis_notes`: `String`
  - `investigations_advised`: `Vec<String>`
  - `follow_up_date`: `Date`
  - `integrity_verification_hash`: `String` (HMAC integrity hash, not a legal digital signature)
  - `pdf_object_bucket`: `String`
  - `pdf_object_key`: `String`
  - `created_at`: `DateTime<Utc>`
- **Fields (`PrescriptionItem`):**
  - `id`: `UUID` (Primary Key)
  - `prescription_id`: `UUID` (Foreign Key -> `Prescription.id`, Required)
  - `brand_name`: `String`
  - `generic_name`: `String`
  - `dosage_form`: `String`
  - `strength`: `String`
  - `dosage_frequency`: `String`
  - `duration_days`: `u16`
  - `instructions`: `String`
- **Sensitivity:** `PHI`

---

### 2.11 `GrievanceReport` & `GrievanceAdjudication`
- **Purpose:** Formal dispute filing and Central Governance Board arbitration.
- **Fields (`GrievanceReport`):**
  - `id`: `UUID` (Primary Key)
  - `grievance_number`: `String`
  - `patient_id`: `UUID` (Foreign Key -> `PatientProfile.id`, Required)
  - `consultation_id`: `UUID` (Foreign Key -> `ConsultationSession.id`, Required)
  - `target_type`: `GrievanceTarget` (`DOCTOR`, `SYSTEM`)
  - `category`: `String`
  - `claim_summary`: `String`
  - `status`: `GrievanceStatus` (`PENDING_REVIEW`, `UNDER_INVESTIGATION`, `REFUNDED`, `WARNED`, `DISMISSED`)
  - `created_at`: `DateTime<Utc>`
- **Fields (`GrievanceAdjudication`):**
  - `id`: `UUID` (Primary Key)
  - `grievance_id`: `UUID` (Foreign Key -> `GrievanceReport.id`, Required, Unique)
  - `adjudicated_by_admin_id`: `UUID` (Foreign Key -> `User.id`, Required)
  - `board_remedy`: `String`
  - `action_type`: `AdjudicationAction` (`REFUND_DISBURSED`, `DOCTOR_WARNED`, `CLAIM_DISMISSED`)
  - `audit_notes`: `String`
  - `adjudicated_at`: `DateTime<Utc>`
- **Sensitivity:** `OPERATIONAL` / Forensic Record.

---

### 2.12 `Transaction` & `DoctorWallet`
- **Purpose:** Financial accounting, payment holding, and 20% platform charge debarment.
- **Fields (`Transaction`):**
  - `id`: `UUID` (Primary Key)
  - `transaction_number`: `String`
  - `appointment_id`: `UUID` (Foreign Key -> `Appointment.id`, Required)
  - `gateway`: `PaymentGateway` (`BKASH`, `NAGAD`, `CARD`, `MPESA`)
  - `gateway_reference`: `String`
  - `gross_amount`: `Decimal`
  - `platform_fee_amount`: `Decimal`
  - `net_amount`: `Decimal`
  - `payment_status`: `PaymentStatus` (`INITIATED`, `ESCROW_HELD`, `SETTLED_TO_DOCTOR`, `DISBURSED`, `REFUNDED_TO_PATIENT`, `FAILED`)
  - `created_at`: `DateTime<Utc>`
  - `settled_at`: `DateTime<Utc>` (Optional)
- **Fields (`DoctorWallet`):**
  - `doctor_id`: `UUID` (Primary Key, Foreign Key -> `DoctorProfile.id`)
  - `lifetime_gross_earnings`: `Decimal`
  - `lifetime_platform_fee_withheld`: `Decimal`
  - `lifetime_net_earnings`: `Decimal`
  - `current_withdrawable_balance`: `Decimal`
  - `last_payout_at`: `DateTime<Utc>` (Optional)
- **Sensitivity:** `FINANCIAL`
