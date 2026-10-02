# Phase 5 — Domain Model & Healthcare Entity Specification

> **Document Version:** 1.0.0  
> **Target Frameworks:** Rust Domain Models (`serde`, `validator`), Flutter Models (`freezed`, `json_serializable`)  
> **Compliance Standard:** Healthcare Privacy (PHI), Medical Credentialing, Financial Auditability

---

## 1. Domain Model Overview

This document defines the domain entities reverse-engineered from the HelloDoctor platform. Every entity includes field definitions, type specifications, validation constraints, lifecycle states, and sensitive data classifications.

### Data Sensitivity Classifications
- **PHI (Protected Health Information):** High confidentiality. Requires encryption at rest and in transit. Strict role-based access. (Note: RLS provides defense-in-depth but application-layer ABAC is authoritative. RLS must never contradict ABAC.)
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
  - `role`: `UserRole` (`PATIENT`, `DOCTOR`, `PLATFORM_ADMIN`, `CLINICAL_ADMIN`, `FINANCE_ADMIN`, `SUPPORT`, `COMPLIANCE`, `SECURITY_ADMIN`)
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

### 2.2a `MfaCredential`
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
- **Sensitivity:** `IDENTITY` / Authentication secret.

---

### 2.3 `AuditEvent`
- **Purpose:** Immutable healthcare and system audit trail. (Immutability enforced by granting INSERT/SELECT only to the app role, and considering append-only archival.)
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
- **Ownership:** Patient. Accessible by the consulting doctor only during `CONFIRMED`, `WAITING`, or `IN_CONSULTATION`, and for up to 24 hours after `completed_at`.

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
  - `clinical_outcome`: `String` (Optional, CHECK constraint: 'COMPLETED_WITH_RX', 'COMPLETED_NO_RX', 'REFERRED', 'ESCALATED')
  - `completed_at`: `DateTime<Utc>` (Optional; required when status becomes `COMPLETED`)
  - `created_at`: `DateTime<Utc>`
  - `updated_at`: `DateTime<Utc>`
- **Sensitivity:** `PHI` / `FINANCIAL`

### 2.7a `AppointmentClinicalIntake`
- **Purpose:** Isolates clinical free text from operational appointment/payment fields so non-clinical staff cannot read it.
- **Fields:**
  - `appointment_id`: `UUID` (Primary Key, Foreign Key -> `Appointment.id`)
  - `chief_complaint`: `String` (Optional)
  - `clinical_notes`: `String` (Doctor's internal notes during consultation)
  - `created_at`: `DateTime<Utc>`
  - `updated_at`: `DateTime<Utc>`
- **Sensitivity:** `PHI`

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

### 2.10 `Prescription`
- **Purpose:** Secure record of the assigned doctor's photographed handwritten prescription, or an explicit no-prescription clinical outcome. V1 does not provide native structured medicine authoring.
- **Fields:**
  - `id`: `UUID` (Primary Key)
  - `appointment_id`: `UUID` (Foreign Key -> `Appointment.id`, Required, Unique)
  - `doctor_id`: `UUID` (Foreign Key -> `DoctorProfile.id`, Required)
  - `patient_id`: `UUID` (Foreign Key -> `PatientProfile.id`, Required)
  - `rx_number`: `String` (Unique)
  - `integrity_verification_hash`: `String` (HMAC integrity hash, not a legal digital signature)
  - `rx_image_object_bucket`: `String` (Required unless `is_no_rx_required = true`)
  - `rx_image_object_key`: `String` (Required unless `is_no_rx_required = true`)
  - `doctor_notes`: `String` (Optional, max 500 chars)
  - `is_no_rx_required`: `Boolean`
  - `no_rx_reason`: `String` (Required when `is_no_rx_required = true`)
  - `created_at`: `DateTime<Utc>`
- **Business Rule:** Images use the quarantine/malware-scan/re-encoding pipeline and private object storage. The API returns an opaque document ID, never an object-storage URL.
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
  - `payment_session_id`: `UUID` (Opaque, unique, persisted before redirect)
  - `payment_status`: `PaymentStatus` (`INITIATED`, `PAYMENT_HELD`, `SETTLED_TO_DOCTOR`, `DISBURSED`, `REFUNDED_TO_PATIENT`, `FAILED`)
  - `created_at`: `DateTime<Utc>`
  - `settled_at`: `DateTime<Utc>` (Optional)
- **Fields (`DoctorWallet`):**
  - `doctor_id`: `UUID` (Primary Key, Foreign Key -> `DoctorProfile.id`)
  - `lifetime_gross`: `Decimal`
  - `lifetime_fee_withheld`: `Decimal`
  - `lifetime_net`: `Decimal`
  - `pending_disbursement`: `Decimal` (Accrued settled net awaiting the next Finance-admin batch; not independently withdrawable)
  - `last_disbursement_at`: `DateTime<Utc>` (Optional)
  - `last_disbursement_amount`: `Decimal`
- **Sensitivity:** `FINANCIAL`

---

### 2.13 `ChatConversation` & `ChatMessage`
- **Purpose:** Asynchronous clinical chats within a 24-hour window.
- **Fields (`ChatConversation`):**
  - `id`: `UUID` (Primary Key)
  - `appointment_id`: `UUID` (Foreign Key -> `Appointment.id`, Required, Unique)
  - `patient_id`: `UUID` (Foreign Key -> `PatientProfile.id`, Required)
  - `doctor_id`: `UUID` (Foreign Key -> `DoctorProfile.id`, Required)
  - `expires_at`: `DateTime<Utc>`
  - `is_locked`: `Boolean`
  - `created_at`: `DateTime<Utc>`
- **Fields (`ChatMessage`):**
  - `id`: `UUID` (Primary Key)
  - `conversation_id`: `UUID` (Foreign Key -> `ChatConversation.id`, Required)
  - `sender_id`: `UUID` (Foreign Key -> `User.id`, Required)
  - `content`: `String` (Required)
  - `attachment_bucket`: `String` (Optional)
  - `attachment_object_key`: `String` (Optional)
  - `attachment_mime`: `String` (Optional)
  - `attachment_size`: `u64` (Optional)
  - `attachment_hash`: `String` (Optional)
  - `is_read`: `Boolean` (Default: `false`)
  - `sent_at`: `DateTime<Utc>`
- **Sensitivity:** `PHI`

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
