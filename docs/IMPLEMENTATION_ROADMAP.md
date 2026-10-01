# Phase 21 — Sequential Implementation Roadmap

> **Document Version:** 1.0.0  
> **Target Execution Agent:** Claude Code Autonomous Engineer  
> **Workflow Pattern:** Milestone-Driven TDD (Test-Driven Development) with Continuous Progress Updates

---

## 1. Roadmap Overview & Phased Milestones

```
M0-M3: Foundation (PostgreSQL + Rust Axum Core + Project Setup)
  │
M4-M5: Auth, Security & Flutter Theme Scaffolding
  │
M6-M8: Patient Core, Specialist Discovery (Zero Stars) & Emergency Banner
  │
M9-M11: Atomic Booking, 1-5 Rx Upload Engine & Virtual Waiting Room
  │
M12-M14: WebRTC Telehealth, Telemetry, 24h Chat & DGDA E-Prescriptions
  │
M15: Doctor Wallet & Transparent 20% Fee Debarment Settlement
  │
M16-M17: Clinical Grievance Board, Admin Dossiers & Escrow Ledger
  │
M18-M19: Localization (Swahili/English), Offline Guards & Production Sign-off
```

---

## 2. Milestone Specifications

### Milestone M0: Monorepo Setup & Workspace Scaffolding
- **Prerequisites:** Git repository initialized.
- **Expected Artifacts:**
  - `apps/mobile/` (Flutter project initialized with `flutter create apps/mobile`).
  - `services/backend/` (Rust Cargo workspace with `Cargo.toml`).
  - `apps/web/` (Next.js / TypeScript workspace for Admin & Doctor Workstation).
- **Deliverables:** Working compile checks (`cargo check`, `flutter analyze`).
- **Tests:** Baseline compilation smoke test.

---

### Milestone M1: PostgreSQL Schema & Migrations
- **Prerequisites:** M0.
- **Expected Artifacts:**
  - `services/backend/migrations/` containing full DDL migrations per `docs/DATABASE_SCHEMA.md`.
- **Deliverables:** Up and Down migration scripts for all 12 tables.
- **Tests:** Database migration runner test via Docker/Testcontainers.

---

### Milestone M2: Rust Backend Foundation & Core Middleware
- **Prerequisites:** M1.
- **Expected Artifacts:**
  - `services/backend/src/main.rs`, `config.rs`, `error.rs`.
  - Middleware: JWT authentication, rate limiting, request ID tracing, CORS.
- **Deliverables:** Health check endpoint (`GET /healthz`), global error handling mapper.
- **Tests:** Router test verifying CORS, Request ID injection, and error formatting.

---

### Milestone M3: Authentication & Security Engine
- **Prerequisites:** M2.
- **Expected Artifacts:**
  - `api/v1/auth_handlers.rs`, `services/auth_service.rs`, `domain/models/user.rs`.
- **Deliverables:** Phone OTP registration/login, Argon2id hashing, RS256 JWT issuance.
- **Tests:** OTP generation, expiration, rate limiting, and token verification tests.

---

### Milestone M4: Flutter Mobile Scaffolding & Design Tokens
- **Prerequisites:** M0.
- **Expected Artifacts:**
  - `apps/mobile/lib/core/theme/` (Colors, Typography, Spacing from `docs/UI_DESIGN_SYSTEM.md`).
  - Core network client (`Dio` with interceptors) and secure storage wrapper.
- **Deliverables:** Light/Dark theme configuration, Plus Jakarta Sans typography.
- **Tests:** Theme consistency test, design token snapshot verification.

---

### Milestone M5: Specialist Discovery & Star-Rating Elimination
- **Prerequisites:** M3, M4.
- **Expected Artifacts:**
  - `apps/mobile/lib/features/doctor_discovery/`.
  - Rust endpoints: `GET /api/v1/doctors`, `GET /api/v1/doctors/{id}`.
- **Deliverables:** Modality tabs ('All', 'Video', 'Chat'), specialty filter carousel, doctor cards with BMDC badges and fees (zero star ratings).
- **Tests:** Assert zero star widgets rendered; test modality and specialty filters.

---

### Milestone M6: Emergency Contact Helpline & Patient Home
- **Prerequisites:** M5.
- **Expected Artifacts:**
  - `apps/mobile/lib/features/home/presentation/widgets/emergency_helpline_strip.dart`.
- **Deliverables:** Emergency Contact banner positioned directly above Upcoming Appointments with one-tap dialing to `16263` (no "24/7" prefix).
- **Tests:** Widget test asserting DOM position and native dialer trigger.

---

### Milestone M7: Atomic Slot Booking & Escrow Hold
- **Prerequisites:** M5.
- **Expected Artifacts:**
  - `services/backend/src/services/appointment_service.rs`.
  - Flutter booking page with date and slot chips.
- **Deliverables:** Row-locked slot booking with automatic 20% platform charge debarment and deposit into `ESCROW_HELD`.
- **Tests:** Concurrency test proving zero double-booking under 10 parallel requests.

---

### Milestone M8: Multi-Prescription Upload Engine (Max 5 Images)
- **Prerequisites:** M7.
- **Expected Artifacts:**
  - `features/appointment_booking/presentation/widgets/prescription_dropzone.dart`.
  - Endpoint: `POST /api/v1/appointments/{id}/prescriptions/upload`.
- **Deliverables:** Multi-file picker with 5-image hard limit, delete triggers, and full-screen multi-page viewer modal (`#labReportModal`).
- **Tests:** Unit test asserting 6th photo rejection with warning toast.

---

### Milestone M9: Virtual Waiting Room & Device Readiness
- **Prerequisites:** M8.
- **Expected Artifacts:**
  - `features/waiting_room/presentation/pages/waiting_room_page.dart`.
- **Deliverables:** Live countdown clock, queue position indicator, camera/mic pre-flight checks, attached prescription thumbnails.
- **Tests:** Countdown timer verification, permission grant mock test.

---

### Milestone M10: Agora RTC Telehealth Video & Telemetry Auto-Capture
- **Prerequisites:** M9.
- **Expected Artifacts:**
  - `features/consultation/` (`agora_rtc_engine` integration in Flutter), Agora token endpoint `POST /api/v1/telehealth/agora-token` in Rust.
- **Deliverables:** Adaptive 720p/1080p video feed via Agora SD-RTN, automatic call duration tracking from `onRtcStats`, premature end detection (<30s), telemetry upload to `POST /api/v1/consultations/{id}/telemetry`.
- **Tests:** Agora token generation unit test, channel join mock test, premature termination flag verification.

---

### Milestone M11: 24-Hour Clinical Chat Desk
- **Prerequisites:** M10.
- **Expected Artifacts:**
  - `features/clinical_chat/`, WebSocket chat endpoint `GET /api/v1/chat/ws/{id}`.
- **Deliverables:** Bi-directional chat with canned clinical pills and automatic 24-hour expiration lock.
- **Tests:** WebSocket message broadcast, 24-hour lock expiry test.

---

### Milestone M12: DGDA E-Prescriptions & Health Vault EMR
- **Prerequisites:** M10.
- **Expected Artifacts:**
  - `features/health_vault/`, Rust digital signature service.
- **Deliverables:** Digital prescription authoring, HMAC-SHA256 signature hashing, PDF generation, vault archive.
- **Tests:** PDF generation test, cryptographic signature verification.

---

### Milestone M13: Doctor Wallet & 20% Debarred Settlement
- **Prerequisites:** M10.
- **Expected Artifacts:**
  - `features/doctor_wallet/`, Endpoint `GET /api/v1/doctor/wallet/summary`.
- **Deliverables:** 3-tier earnings calculation ($G \to C \to N$), mandatory basis note, itemized ledger rows, MFS payout trigger.
- **Tests:** Exact arithmetic test asserting withheld fee is exactly 20% of gross.

---

### Milestone M14: Clinical Grievance Redressal & Board Arbitration
- **Prerequisites:** M10, M13.
- **Expected Artifacts:**
  - `features/grievance_redressal/`, Admin Grievance Docket.
- **Deliverables:** Patient grievance modal with silent telemetry binding, admin adjudication desk with one-click escrow refund and BMDC warning tools.
- **Tests:** End-to-end dispute filing -> telemetry inspection -> escrow refund disbursal test.

---

### Milestone M15: Central Admin Consoles & Doctor Dossiers
- **Prerequisites:** M14.
- **Expected Artifacts:**
  - Admin Portal with 9 specialized views (`command`, `doctors`, `patients`, `bmdc`, `slots`, `compliance`, `logs`, `grievances`, `finance`).
- **Deliverables:** Doctor dossiers with verified phone and residence, error log diagnostics with failure simulator, master escrow ledger.
- **Tests:** Admin access control tests, failure simulation test, CSV export test.

---

### Milestone M16: Bilingual Localization (English & Swahili) & Offline Guards
- **Prerequisites:** M4.
- **Expected Artifacts:**
  - `core/localization/` (English and Swahili translations), `core/network/offline_guard.dart`.
- **Deliverables:** In-memory language toggle with badge update, offline action interceptor modal (`#offlineActionGuardModal`), shimmer loading states.
- **Tests:** Language switcher test, offline network mock interception test.

---

### Milestone M17: Final Quality Assurance, Security Audit & Production Verification
- **Prerequisites:** M0 through M16.
- **Deliverables:** Complete test execution, OWASP mobile security review, pixel-fidelity audit against prototype.
- **Tests:** Full regression suite passing 100%.
