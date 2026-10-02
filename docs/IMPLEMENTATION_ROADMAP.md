# Phase 21 — Sequential Implementation Roadmap

> **Document Version:** 1.0.0  
> **Target Execution Agent:** Claude Code Autonomous Engineer  
> **Workflow Pattern:** Milestone-Driven TDD (Test-Driven Development) with Continuous Progress Updates

---

## 1. Roadmap Overview & Phased Milestones

```
M0-M1: Architecture Remediation & Security Model
  │
M2: Canonical State Model & Database v2
  │
M3: API v2 & Auth (Ed25519)
  │
M4: Agora ADR & Implementation Setup
  │
M5-M6: Specialist Discovery (Zero Stars) & Emergency Banner
  │
M7-M9: Atomic Booking, 1-5 Rx Upload Engine & Virtual Waiting Room
  │
M10-M12: Agora Telehealth, Telemetry, 24h Chat & E-Prescriptions
  │
M13: Doctor Wallet & Transparent 20% Fee Debarment Settlement
  │
M14-M15: Clinical Grievance Board, Admin Dossiers & Ledger
  │
M16-M17: Localization (Swahili/English/Future Bangla), Offline Guards
  │
M-SECURITY: Security Gates & Verification
  │
M18: Production Sign-off
```

---

## 2. Milestone Specifications

### Milestone M0: Architecture Remediation
- **Prerequisites:** Git repository initialized.
- **Expected Artifacts:** Updated specification documents.
- **Deliverables:** Completed architecture remediation.

### Milestone M1: Security Model
- **Prerequisites:** M0.
- **Deliverables:** Establish BOLA/IDOR authorization models, ABAC policies.

### Milestone M2: Canonical State Model & Database v2
- **Prerequisites:** M1.
- **Expected Artifacts:**
  - `services/backend/migrations/` containing full DDL migrations per `docs/DATABASE_SCHEMA.md`.
- **Deliverables:** Up and Down migration scripts for all tables.
- **Tests:** Database migration runner test via Docker/Testcontainers.

### Milestone M3: API v2 & Auth
- **Prerequisites:** M2.
- **Expected Artifacts:**
  - `api/v1/auth_handlers.rs`, `services/auth_service.rs`, `domain/models/user.rs`, `auth_sessions` table.
- **Deliverables:** Phone OTP registration/login, Argon2id hashing, Ed25519 JWT issuance, refresh token rotation.
- **Tests:** OTP generation, expiration, rate limiting, and token verification tests.

### Milestone M4: Agora ADR & Implementation Setup
- **Prerequisites:** M3.
- **Expected Artifacts:** Flutter & Rust Workspaces.
- **Deliverables:** Working compile checks (`cargo check`, `flutter analyze`). Light/Dark theme configuration.

### Milestone M5: Specialist Discovery & Star-Rating Elimination
- **Prerequisites:** M4.
- **Expected Artifacts:**
  - `apps/mobile/lib/features/doctor_discovery/`.
  - Rust endpoints: `GET /api/v1/doctors`, `GET /api/v1/doctors/{id}`.
- **Deliverables:** Modality tabs, specialty filter carousel, doctor cards with BMDC badges and fees (zero star ratings).
- **Tests:** Assert zero star widgets rendered.

### Milestone M6: Emergency Contact Helpline & Patient Home
- **Prerequisites:** M5.
- **Expected Artifacts:**
  - `apps/mobile/lib/features/home/presentation/widgets/emergency_helpline_strip.dart`.
- **Deliverables:** Emergency Contact banner positioned directly above Upcoming Appointments with one-tap dialing (configurable per country, e.g. `16263` for BD).
- **Tests:** Widget test asserting DOM position and native dialer trigger.

### Milestone M7: Atomic Slot Booking & Escrow Hold
- **Prerequisites:** M5.
- **Expected Artifacts:**
  - `services/backend/src/services/appointment_service.rs`.
- **Deliverables:** Row-locked slot booking with automatic 20% platform charge debarment and deposit into `ESCROW_HELD`.
- **Tests:** Concurrency test proving prevented double-booking under parallel requests.

***SECURITY GATE: BOLA/IDOR testing verified on Appointment APIs.***

### Milestone M8: Multi-Prescription Upload Engine (Max 5 Images)
- **Prerequisites:** M7.
- **Expected Artifacts:**
  - `features/appointment_booking/presentation/widgets/prescription_dropzone.dart`.
  - Endpoint: `POST /api/v1/appointments/{id}/prescriptions/upload`.
- **Deliverables:** Multi-file picker with 5-image hard limit, delete triggers, and full-screen multi-page viewer modal.
- **Tests:** Unit test asserting 6th photo rejection with warning toast.

***SECURITY GATE: Upload security validation (magic-byte validation, malware scanning, quarantine).***

### Milestone M9: Virtual Waiting Room & Device Readiness
- **Prerequisites:** M8.
- **Deliverables:** Live countdown clock, queue position indicator, camera/mic pre-flight checks, attached prescription thumbnails.

### Milestone M10: Agora RTC Telehealth Video & Telemetry Auto-Capture
- **Prerequisites:** M9.
- **Expected Artifacts:**
  - `features/consultation/` (`agora_rtc_engine` integration in Flutter), Agora token endpoint `POST /api/v1/telehealth/agora-token` in Rust.
- **Deliverables:** Adaptive 720p/1080p video feed via Agora SD-RTN, automatic call duration tracking from `onRtcStats`, premature end detection (<30s), telemetry upload.
- **Tests:** Agora token generation unit test, channel join mock test, premature termination flag verification.

### Milestone M11: 24-Hour Clinical Chat Desk
- **Prerequisites:** M10.
- **Deliverables:** Bi-directional chat with canned clinical pills and automatic 24-hour expiration lock.

### Milestone M12: DGDA E-Prescriptions & Health Vault EMR
- **Prerequisites:** M10.
- **Expected Artifacts:**
  - `features/health_vault/`, Rust digital signature service.
- **Deliverables:** Digital prescription authoring, HMAC-SHA256 integrity verification, PDF generation, vault archive.
- **Tests:** PDF generation test, cryptographic integrity verification.

### Milestone M13: Doctor Wallet & 20% Debarred Settlement
- **Prerequisites:** M10.
- **Deliverables:** 3-tier earnings calculation ($G \to C \to N$), mandatory basis note, itemized ledger rows, MFS payout trigger.

### Milestone M14: Clinical Grievance Redressal & Board Arbitration
- **Prerequisites:** M10, M13.
- **Expected Artifacts:**
  - `features/grievance_redressal/`, Admin Grievance Docket.
- **Deliverables:** Patient grievance modal with silent telemetry binding, admin adjudication desk with one-click escrow refund and Internal Platform Compliance Warning tools.

### Milestone M15: Central Admin Consoles & Doctor Dossiers
- **Prerequisites:** M14.
- **Expected Artifacts:**
  - Admin Portal with 9 specialized views (`command`, `doctors`, `patients`, `bmdc`, `slots`, `compliance`, `logs`, `grievances`, `finance`).
- **Deliverables:** Doctor dossiers with verified phone and residence, error log diagnostics, payment and settlement ledger.

### Milestone M16: Bilingual Localization & Future Expansion
- **Deliverables:** In-memory language toggle with badge update.
- **Note:** Bangla localization for Bangladesh market should be planned for future milestone.

### Milestone M17: Offline Guards
- **Deliverables:** Offline action interceptor modal (`#offlineActionGuardModal`), shimmer loading states.

### Milestone M-SECURITY: Security Gates & Verification
- **Prerequisites:** M0 through M17.
- **Deliverables:** OWASP review, ABAC verification, RLS audit, file upload security, push notification PHI audit.

***SECURITY GATE: PHI caching audit complete.***

### Milestone M18: Final Quality Assurance & Production Verification
- **Prerequisites:** M-SECURITY.
- **Deliverables:** Complete test execution, pixel-fidelity audit against prototype. Full regression suite passing 100%.
