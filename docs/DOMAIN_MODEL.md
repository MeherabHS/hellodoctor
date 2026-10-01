# Phase 5 — Domain Model & Healthcare Entity Specification

> **Document Version:** 1.0.0  
> **Target Frameworks:** Rust Domain Models (`serde`, `validator`), Flutter Models (`freezed`, `json_serializable`)  
> **Compliance Standard:** Healthcare Privacy (PHI), BMDC Credentialing, Financial Auditability

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
  - `password_hash`: `String` (Required, Argon2id)
  - `role`: `UserRole` (`PATIENT`, `DOCTOR`, `ADMIN`)
  - `is_active`: `Boolean` (Default: `true`)
  - `is_verified`: `Boolean` (Default: `false`)
  - `preferred_language`: `String` (Default: `'en'`, values: `'en'`, `'sw'`)
  - `created_at`: `DateTime<Utc>`
  - `updated_at`: `DateTime<Utc>`
- **Sensitivity:** `IDENTITY`
- **Ownership:** Self.
- **Lifecycle:** Created on registration -> Verified via OTP -> Deactivated upon account closure.

---

### 2.2 `PatientProfile`
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

### 2.3 `DoctorProfile`
- **Purpose:** Professional physician profile, clinical credentials, and practice parameters.
- **Fields:**
  - `id`: `UUID` (Primary Key, Required)
  - `user_id`: `UUID` (Foreign Key -> `User.id`, Required, Unique)
  - `full_name`: `String` (Required, prefixed with "Dr.")
  - `bmdc_number`: `String` (Required, Unique, e.g., "BMDC #45821")
  - `primary_specialty`: `MedicalSpecialty` (Required, e.g., `INTERNAL_MEDICINE`, `CARDIOLOGY`)
  - `secondary_specialties`: `Vec<MedicalSpecialty>`
  - `experience_years`: `u8` (Required, 0–70)
  - `current_hospital`: `String` (Required, e.g., "Dhaka Medical College Hospital")
  - `qualifications`: `Vec<String>` (e.g., `["MBBS", "FCPS", "MD"]`)
  - `consultation_fee_video`: `Decimal` (Required, e.g., `800.00`)
  - `consultation_fee_chat`: `Decimal` (Required, e.g., `500.00`)
  - `residential_address`: `String` (Required, for verification and dossier)
  - `verified_phone`: `String` (Required)
  - `is_on_duty`: `Boolean` (Default: `false`)
  - `is_verified`: `Boolean` (Default: `false`)
  - `created_at`: `DateTime<Utc>`
  - `updated_at`: `DateTime<Utc>`
- **Sensitivity:** `OPERATIONAL` / Contact info is `IDENTITY`.
- **Ownership:** Doctor. Governed by Admin.

---

### 2.4 `DoctorScheduleSlot`
- **Purpose:** 15-minute or 20-minute availability blocks for teleconsultation.
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

### 2.5 `Appointment`
- **Purpose:** Scheduled consultation booking linking patient, doctor, and slot.
- **Fields:**
  - `id`: `UUID` (Primary Key)
  - `appointment_number`: `String` (e.g., "APT-20261001-9481")
  - `patient_id`: `UUID` (Foreign Key -> `PatientProfile.id`, Required)
  - `doctor_id`: `UUID` (Foreign Key -> `DoctorProfile.id`, Required)
  - `slot_id`: `UUID` (Foreign Key -> `DoctorScheduleSlot.id`, Required)
  - `modality`: `ConsultationModality` (`VIDEO`, `CHAT`)
  - `status`: `AppointmentStatus` (`PENDING_PAYMENT`, `CONFIRMED`, `WAITING`, `IN_CONSULTATION`, `COMPLETED`, `CANCELLED`, `DISPUTED`)
  - `consultation_fee`: `Decimal` (Required)
  - `chief_complaint`: `String` (Optional, max 500 chars)
  - `created_at`: `DateTime<Utc>`
  - `updated_at`: `DateTime<Utc>`
- **Sensitivity:** `PHI` / `FINANCIAL`

---

### 2.6 `PrescriptionIntakeDocument` (Max 5 Images)
- **Purpose:** Physical prescription or lab report uploaded by patient prior to consultation.
- **Fields:**
  - `id`: `UUID` (Primary Key)
  - `appointment_id`: `UUID` (Foreign Key -> `Appointment.id`, Required)
  - `patient_id`: `UUID` (Foreign Key -> `PatientProfile.id`, Required)
  - `page_number`: `u8` (Required, 1 to 5)
  - `file_url`: `String` (Encrypted object storage URL)
  - `file_size_bytes`: `u64` (Max 10,485,760 bytes = 10 MB)
  - `mime_type`: `String` (`image/jpeg`, `image/png`, `application/pdf`)
  - `created_at`: `DateTime<Utc>`
- **Business Rule:** Total documents per `appointment_id` must not exceed 5.
- **Sensitivity:** `PHI`

---

### 2.7 `ConsultationSession` & `ConsultationTelemetry`
- **Purpose:** Live teleconsultation record and objective technical telemetry.
- **Fields (`ConsultationSession`):**
  - `id`: `UUID` (Primary Key)
  - `appointment_id`: `UUID` (Foreign Key -> `Appointment.id`, Required, Unique)
  - `room_id`: `String` (WebRTC room identifier)
  - `started_at`: `DateTime<Utc>`
  - `ended_at`: `DateTime<Utc>`
  - `status`: `SessionStatus` (`INITIALIZED`, `ACTIVE`, `COMPLETED`, `PREMATURE_TERMINATION`)
- **Fields (`ConsultationTelemetry`):**
  - `id`: `UUID` (Primary Key)
  - `session_id`: `UUID` (Foreign Key -> `ConsultationSession.id`, Required, Unique)
  - `call_duration_seconds`: `u32` (Actual connected duration)
  - `ice_connection_state`: `String` (e.g., "COMPLETED", "FAILED", "DISCONNECTED")
  - `packet_loss_percent`: `Decimal`
  - `round_trip_time_ms`: `u32`
  - `premature_end`: `Boolean` (`call_duration_seconds < 30`)
  - `prescription_issued`: `Boolean` (Default: `false`)
- **Sensitivity:** `OPERATIONAL` / Forensic Evidence.

---

### 2.8 `Prescription` & `PrescriptionItem`
- **Purpose:** Official legal e-prescription authored and digitally signed by physician.
- **Fields (`Prescription`):**
  - `id`: `UUID` (Primary Key)
  - `appointment_id`: `UUID` (Foreign Key -> `Appointment.id`, Required, Unique)
  - `doctor_id`: `UUID` (Foreign Key -> `DoctorProfile.id`, Required)
  - `patient_id`: `UUID` (Foreign Key -> `PatientProfile.id`, Required)
  - `rx_number`: `String` (Unique, e.g., "RX-20261001-841")
  - `diagnosis_notes`: `String` (Clinical summary)
  - `investigations_advised`: `Vec<String>` (Lab tests: e.g., "CBC with ESR")
  - `follow_up_date`: `Date` (Optional)
  - `digital_signature_hash`: `String` (HMAC-SHA256 signature)
  - `pdf_storage_url`: `String`
  - `created_at`: `DateTime<Utc>`
- **Fields (`PrescriptionItem`):**
  - `id`: `UUID` (Primary Key)
  - `prescription_id`: `UUID` (Foreign Key -> `Prescription.id`, Required)
  - `brand_name`: `String` (e.g., "Napa Extra")
  - `generic_name`: `String` (e.g., "Paracetamol + Caffeine")
  - `dosage_form`: `String` (e.g., "Tablet", "Syrup")
  - `strength`: `String` (e.g., "500mg + 65mg")
  - `dosage_frequency`: `String` (e.g., "1+0+1")
  - `duration_days`: `u16` (e.g., 5)
  - `instructions`: `String` (e.g., "After meal")
- **Sensitivity:** `PHI`

---

### 2.9 `GrievanceReport` & `GrievanceAdjudication`
- **Purpose:** Formal dispute filing and Central Governance Board arbitration.
- **Fields (`GrievanceReport`):**
  - `id`: `UUID` (Primary Key)
  - `grievance_number`: `String` (e.g., "GRV-20261001-73")
  - `patient_id`: `UUID` (Foreign Key -> `PatientProfile.id`, Required)
  - `consultation_id`: `UUID` (Foreign Key -> `ConsultationSession.id`, Required)
  - `target_type`: `GrievanceTarget` (`DOCTOR`, `SYSTEM`)
  - `category`: `String` (e.g., "Rushed Consultation", "WebRTC Video Freeze")
  - `claim_summary`: `String` (Patient narrative)
  - `status`: `GrievanceStatus` (`PENDING_REVIEW`, `REFUNDED`, `WARNED`, `DISMISSED`)
  - `created_at`: `DateTime<Utc>`
- **Fields (`GrievanceAdjudication`):**
  - `id`: `UUID` (Primary Key)
  - `grievance_id`: `UUID` (Foreign Key -> `GrievanceReport.id`, Required, Unique)
  - `adjudicated_by_admin_id`: `UUID` (Foreign Key -> `User.id`, Required)
  - `board_remedy`: `String` (e.g., "Escrow Refund Disbursed (৳800)")
  - `action_type`: `AdjudicationAction` (`REFUND_DISBURSED`, `DOCTOR_WARNED`, `CLAIM_DISMISSED`)
  - `audit_notes`: `String`
  - `adjudicated_at`: `DateTime<Utc>`
- **Sensitivity:** `OPERATIONAL` / Forensic Record.

---

### 2.10 `Transaction` & `DoctorWallet`
- **Purpose:** Financial accounting, escrow locking, and 20% platform charge debarment.
- **Fields (`Transaction`):**
  - `id`: `UUID` (Primary Key)
  - `transaction_number`: `String` (Unique, e.g., "TXN-BK-94812")
  - `appointment_id`: `UUID` (Foreign Key -> `Appointment.id`, Required)
  - `gateway`: `PaymentGateway` (`BKASH`, `NAGAD`, `CARD`, `MPESA`)
  - `gateway_reference`: `String` (IPN Webhook ID)
  - `gross_amount`: `Decimal` (e.g., `800.00`)
  - `platform_fee_amount`: `Decimal` (Strictly `gross_amount * 0.20`, e.g., `160.00`)
  - `net_amount`: `Decimal` (Strictly `gross_amount * 0.80`, e.g., `640.00`)
  - `escrow_status`: `EscrowStatus` (`ESCROW_HELD`, `SETTLED_TO_DOCTOR`, `REFUNDED_TO_PATIENT`)
  - `created_at`: `DateTime<Utc>`
  - `settled_at`: `DateTime<Utc>` (Optional)
- **Fields (`DoctorWallet`):**
  - `doctor_id`: `UUID` (Primary Key, Foreign Key -> `DoctorProfile.id`)
  - `lifetime_gross_earnings`: `Decimal` (Default: `0.00`)
  - `lifetime_platform_fee_withheld`: `Decimal` (Default: `0.00`)
  - `lifetime_net_earnings`: `Decimal` (Default: `0.00`)
  - `current_withdrawable_balance`: `Decimal` (Default: `0.00`)
  - `last_payout_at`: `DateTime<Utc>` (Optional)
- **Sensitivity:** `FINANCIAL`
