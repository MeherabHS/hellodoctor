# Phase 20 — End-to-End Requirement Traceability Matrix

> **Document Version:** 1.0.0  
> **Traceability Mandate:** Every prototype capability must have a 1:1 mapping through UI, API, Domain Services, Database Entities, and Verification Suites.  
> **Target Audience:** Claude Code Autonomous Implementation Agent

---

## 1. Master Traceability Matrix

| Prototype Feature | Req ID | Screen ID | Flutter Feature Module | API Endpoint | Rust Service | DB Entity | Test Suite | Acceptance Criteria |
|---|---|---|---|---|---|---|---|---|
| **Specialist Discovery & Modality Tabs** | REQ-001 | `HD-PATIENT-FIND-SPECIALIST` (`view-3`) | `doctor_discovery` | `GET /api/v1/doctors` | `DoctorService::list_doctors` | `DoctorProfile` | `test_doctor_directory_suite.dart` | `AC-001` |
| **Star Rating Elimination & BMDC Display** | REQ-002 | `HD-PATIENT-FIND-SPECIALIST` (`view-3`) | `doctor_discovery` | `GET /api/v1/doctors` | `DoctorService::get_doctor` | `DoctorProfile` | `test_grievance_suite.dart` | `AC-001` |
| **Multi-Prescription Upload (1-5 Photos)** | REQ-003 | `HD-PATIENT-BOOK-SLOT` (`view-4`) | `appointment_booking` | `POST /prescriptions/upload` | `PrescriptionService::upload_intake` | `PrescriptionIntakeDocument` | `test_multi_rx_upload_suite.dart` | `AC-002` |
| **Multi-Page Prescription Viewer** | REQ-004 | `HD-MODAL-RX-VIEWER` (`#labReportModal`) | `waiting_room` | Pre-signed S3 URLs | `PrescriptionService::get_images` | `PrescriptionIntakeDocument` | `test_multi_rx_upload_suite.dart` | `AC-003` |
| **Atomic Slot Booking & Escrow Hold** | REQ-005 | `HD-PATIENT-BOOK-SLOT` (`view-4`) | `appointment_booking` | `POST /appointments/book` | `AppointmentService::book_atomic` | `DoctorScheduleSlot`, `Transaction` | `test_admin_finance_suite.rs` | `AC-004`, `AC-005` |
| **Virtual Waiting Room & Pre-Flight** | REQ-006 | `HD-PATIENT-WAITING-ROOM` (`view-10`) | `waiting_room` | `GET /appointments/{id}/status` | `QueueService::get_status` | `Appointment` | `test_offline_loading_suite.dart`| `AC-003` |
| **WebRTC Telehealth Video & Telemetry** | REQ-007 | `HD-DOCWEB-WORKSTATION` | `consultation` | `POST /consultations/telemetry`| `TelemetryService::record` | `ConsultationTelemetry` | `test_grievance_suite.rs` | `AC-006` |
| **24-Hour Clinical Chat Desk** | REQ-008 | `HD-PATIENT-CONSULT-CHAT` (`view-6`) | `clinical_chat` | `GET /chat/ws/{id}` | `ChatService::handle_message` | `ChatConversation`, `ChatMessage` | `test_chat_suite.rs` | `AC-007` |
| **DGDA Digital Prescription Signing** | REQ-009 | `HD-DOC-RX-SCANNER` (`doc-view-1`) | `consultation` | `POST /prescriptions/sign` | `PrescriptionService::sign_rx` | `Prescription`, `PrescriptionItem` | `test_rx_signing_suite.rs` | `AC-008` |
| **Clinical Grievance Filing (Zero Stars)** | REQ-010 | `HD-MODAL-GRIEVANCE` | `grievance_redressal` | `POST /api/v1/grievances` | `GrievanceService::lodge` | `GrievanceReport` | `test_grievance_suite.dart` | `AC-009` |
| **Grievance Escrow Refund Disbursal** | REQ-011 | `HD-ADMIN-GRIEVANCES` | `admin_grievances` | `POST /grievances/{id}/refund` | `SettlementService::refund_escrow` | `Transaction`, `GrievanceAdjudication` | `test_admin_finance_suite.rs` | `AC-010` |
| **BMDC Disciplinary Warning Logging** | REQ-012 | `HD-ADMIN-GRIEVANCES` | `admin_grievances` | `POST /grievances/{id}/warn` | `GovernanceService::issue_warning` | `DoctorProfile`, `GrievanceAdjudication` | `test_grievance_suite.rs` | `AC-011` |
| **Doctor Earnings 20% Debarred Math** | REQ-013 | `HD-DOC-WALLET-EARNINGS` (`doc-view-3`)| `doctor_wallet` | `GET /doctor/wallet/summary` | `SettlementService::get_wallet` | `DoctorWallet`, `Transaction` | `test_doctor_earnings_suite.dart` | `AC-008` |
| **Doctor Wallet MFS Withdrawal** | REQ-014 | `HD-DOC-WALLET-EARNINGS` (`doc-view-3`)| `doctor_wallet` | `POST /doctor/wallet/withdraw` | `SettlementService::disburse_payout` | `DoctorWallet`, `Transaction` | `test_doctor_earnings_suite.rs` | `AC-008` |
| **Emergency Contact Banner (Call 16263)** | REQ-015 | `HD-PATIENT-HOME` (`view-0`) | `home` | Native dialer (`tel:16263`) | None (Native Telco Scheme) | None | `test_language_and_layout_suite.dart` | `AC-012` |
| **Bilingual Localization (English & Swahili)**| REQ-016 | `HD-MODAL-LANGUAGE` | `core/localization` | `PATCH /patient/preferences` | `UserService::update_language` | `User.preferred_language` | `test_language_and_layout_suite.dart` | `AC-013` |
| **Offline Action Guarding & Shimmers** | REQ-017 | `HD-MODAL-OFFLINE-GUARD` | `core/network` | None (Client Network Interceptor)| None | None | `test_offline_loading_suite.dart`| `AC-014` |
| **Doctor Dossiers (Phone & Residence)** | REQ-018 | `HD-ADMIN-DOCTORS` | `admin_doctors` | `GET /admin/doctors/{id}/dossier`| `AdminService::get_doctor_dossier` | `DoctorProfile` | `test_doctor_directory_suite.rs` | `AC-018` |
| **System Error Logs & Failure Simulator**| REQ-019 | `HD-ADMIN-LOGS` | `admin_logs` | `POST /admin/logs/simulate` | `DiagnosticsService::simulate` | `SystemErrorLog` | `test_admin_logs.rs` | `AC-019` |
| **Master Escrow Ledger & Webhook Reconcile**| REQ-020 | `HD-ADMIN-FINANCE` | `admin_finance` | `POST /admin/finance/reconcile` | `SettlementService::reconcile` | `Transaction` | `test_admin_finance_suite.rs` | `AC-020` |
