# Phase 28 — Legacy Prototype to Production Architecture Mapping

> **Document Version:** 1.0.0  
> **Source Baseline:** HTML/CSS/JS Prototype (`index.html`)  
> **Target Production:** Flutter Mobile (iOS/Android) + Rust Backend (Axum/SQLx) + PostgreSQL  
> **Target Audience:** Claude Code Autonomous Implementation Agent

---

## 1. Architectural Concept Mapping

| Prototype Web Concept | Production Flutter Equivalent | Production Rust Backend Equivalent | Production PostgreSQL Equivalent |
|---|---|---|---|
| Monolithic `index.html` | Modular Flutter App with `go_router` | Modular Axum Router with nested routes | Relational tables in `public` schema |
| Single-page container `#patientAppShell` | `PatientAppScaffold` with Bottom Navigation | Protected endpoints with `Role::Patient` | Data scoped by `patient_id` |
| View containers (`#view-0` to `#view-14`) | Distinct Screen Widgets in `features/*/presentation/pages/` | Resource-specific REST endpoints (`/api/v1/*`) | Distinct entity tables |
| CSS Design Tokens (`tokens.css`) | `AppTheme`, `AppColors`, `AppTypography` | Not applicable | Not applicable |
| `switchScreen(idx)` JS function | `context.goNamed(...)` or `context.pushNamed(...)` | Not applicable | Not applicable |
| Global JS store `doctorEarningsStore` | `DoctorWalletBloc` / `WalletState` | `SettlementService::get_wallet_summary` | `doctor_wallets`, `transactions` |
| Global JS store `adminTransactionStore` | `AdminFinanceBloc` / `TransactionsState` | `TransactionRepository::list_transactions` | `transactions` table with ACID constraints |
| Global JS store `adminPatientGrievancesStore` | `GrievanceBloc` / `GrievanceDocketState`| `GrievanceService::list_grievances` | `grievance_reports`, `grievance_adjudications` |
| File input `#patientPreConsultFileInput` | `image_picker` / `file_picker` plugin | Multipart stream handler in Axum | S3/MinIO bucket + `prescription_intake_documents` |
| `renderDoctorEarnings()` JS function | `DoctorEarningsHeroCard` widget builder | Computed DTO in `SettlementService` | Query aggregating gross and 20% platform cut |
| `submitPatientGrievance()` JS function | `GrievanceBloc.add(SubmitGrievanceEvent)` | `POST /api/v1/grievances` handler | Insert into `grievance_reports` |
| `openMultiPagePrescriptionViewer()` | `PrescriptionViewerModal` with `PageView` | Pre-signed S3 URL generator | Metadata query in `prescription_intake_documents` |
| Browser `localStorage` | `flutter_secure_storage` & `shared_preferences` | Redis / PostgreSQL user preferences | `users.preferred_language` |
| Modal overlay `#offlineActionGuardModal` | `OfflineActionGuardDialog` triggered by connectivity bloc | Rejected via `Idempotency-Key` or network error | Not applicable |

---

## 2. Screen & Component Translation Matrix

### 2.1 Patient Screens
- **`view-0` (Home Dashboard):** Translated into `PatientHomeScreen` (Flutter). Contains `EmergencyHelplineStrip`, `UpcomingConsultationHeroCard`, and `CoreServicesGrid`.
- **`view-1` (Core Services):** Translated into `CoreServicesScreen` with search bar and filtered service catalog.
- **`view-3` (Find Specialist):** Translated into `SpecialistDirectoryScreen` with modality segmented controls, specialty chips, and `DoctorCard` list.
- **`view-4` (Book Slot):** Translated into `AppointmentBookingScreen` with horizontal calendar, slot grid, `PrescriptionDropzone`, and payment gateway radio list.
- **`view-5` (Health Vault):** Translated into `HealthVaultScreen` with prescription cards, PDF download triggers, and lab report tabs.
- **`view-6` (24h Chat):** Translated into `ClinicalChatScreen` with message bubbles, typing indicators, canned medical query pills, and countdown timer.
- **`view-7` (Account):** Translated into `PatientAccountScreen` with profile details, language modal trigger, and logout button.
- **`view-10` (Waiting Room):** Translated into `WaitingRoomScreen` with circular countdown timer, attached prescription strip, and hardware check panel.
- **`view-11` (Post-Consultation):** Translated into `PostConsultationScreen` with download prescription button and discrete grievance filing button.
- **`view-14` (Shimmer Loading):** Translated into `ShimmerPlaceholderCard` utilizing the `shimmer` package.

### 2.2 Doctor Screens
- **`doc-view-0` (Doctor Queue):** Translated into `DoctorQueueScreen` with on-duty switch and live patient triage cards.
- **`doc-view-1` (Smart Rx Scanner):** Translated into `RxComposerScreen` with camera OCR capture, DGDA drug auto-complete, and digital signature CTA.
- **`doc-view-2` (Schedule):** Translated into `DoctorScheduleScreen` with 15-minute slot generator modal.
- **`doc-view-3` (Wallet & Earnings):** Translated into `DoctorWalletScreen` featuring `DoctorEarningsHeroCard` with 3-tier math and itemized ledger.
- **`doc-view-4` (Chat Desk):** Translated into `DoctorChatDeskScreen` with active patient list and clinical quick-replies.
- **`doc-view-5` (Q&A Triage):** Translated into `DoctorQnaScreen` for answering community inquiries.

### 2.3 Doctor Web Workstation & Central Admin
- **`#doctorWebShell`:** Translated into desktop clinical console (Flutter Web or Next.js) with 1080p WebRTC feed on left, split EMR and prescription authoring on right.
- **`adminPage_*`:** Translated into Next.js / TypeScript Admin Portal or Flutter Web desktop application with 9 management dashboards.
