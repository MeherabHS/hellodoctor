# Phase 4 — JavaScript Interaction Reverse Engineering Specification

> **Document Version:** 1.0.0  
> **Source Baseline:** `index.html` (Script Blocks 1 & 2)  
> **Target Audience:** Claude Code Autonomous Implementation Agent

---

## 1. Overview & Analysis Methodology

This document reverse-engineers the client-side JavaScript execution environment found in the HelloDoctor prototype. It details event triggers, state mutations, DOM manipulations, and maps them to their production **Flutter (Bloc/Cubit)** and **Rust (Axum/SQLx)** equivalents.

---

## 2. Global State Variables & Stores

| Prototype Variable | Scope | Type | Purpose | Production Equivalent |
|---|---|---|---|---|
| `currentScreenIndex` | Global | Number (0–14) | Tracks active mobile patient viewport | Flutter `GoRouter` / Navigation Stack |
| `selectedDoctorId` | Global | String | ID of doctor selected for booking/profile | `AppointmentBloc.selectedDoctor` |
| `patientPrescriptionImages` | Global | Array of Objects | Uploaded pre-consultation files (up to 5) | `PrescriptionIntakeBloc.images` |
| `doctorEarningsStore` | Global | Object | 3-tier doctor earnings & ledger records | `DoctorWalletRepository` / Rust DB |
| `adminPatientGrievancesStore` | Global | Array of Objects | Central grievance & dispute docket | PostgreSQL `grievance_reports` |
| `consultationTelemetryStore` | Global | Object | Session metrics (duration, Agora state, Rx) | PostgreSQL `consultation_telemetry` |
| `adminTransactionStore` | Global | Array of Objects | Master payment hold transaction ledger | PostgreSQL `transactions` |
| `adminDoctorHistoryStore` | Global | Object | Doctor dossiers (phone, residence, logs) | PostgreSQL `doctor_dossiers` |
| `adminAppErrorLogsStore` | Global | Array of Objects | System error logs & diagnostics | PostgreSQL `system_error_logs` |
| `activeDoctorChatPatient` | Global | String | Patient ID in active doctor chat desk | `ChatBloc.activeThread` |
| `isAppOnline` | Global | Boolean | Network state flag | Flutter `connectivity_plus` Stream |

---

## 3. Detailed Interaction Specifications

### INT-001: Multi-Prescription File Upload & Cap Enforcement
- **Trigger:** Change event on `#patientPreConsultFileInput` (`handlePatientFileSelection(e)`).
- **Current State:** `patientPrescriptionImages` contains $N$ files ($0 \le N \le 5$).
- **Action:** User selects $M$ image/PDF files from native file picker.
- **State Mutation:**
  - Evaluates $N + M$.
  - If $N + M > 5$, accepted count is capped at $5 - N$. Remaining files are omitted.
  - Converts accepted files via `FileReader.readAsDataURL()`.
  - Appends valid file objects `{ name, size, dataUrl, pageIndex }` to `patientPrescriptionImages`.
- **UI Change:**
  - Invokes `renderPatientPrescriptionGallery()`.
  - Injects thumbnail cards with page badges (`Page 1 of 3`) and delete buttons into `#patientPrescriptionList`.
  - Updates waiting room strip `#wrPrescriptionThumbnailsStrip` and count badge `#wrPrescriptionCountBadge`.
  - If excess files were selected, shows warning toast: *"Maximum 5 prescription photos allowed. Extra photos were omitted."*
- **Data Change:** Array length updated; thumbnails cached in memory.
- **Navigation Consequence:** Remains on `view-4`.
- **Backend Equivalent (Rust):**
  - Multi-part form upload: `POST /api/v1/appointments/{id}/prescriptions`.
  - Server validates: total count $\le 5$, file size $\le 10\text{ MB}$, MIME type $\in \{\text{image/jpeg}, \text{image/png}, \text{application/pdf}\}$.
  - Saves files to S3/MinIO bucket; persists metadata in `prescription_intake_documents`.
- **Error Handling:** Invalid file types trigger an error toast; files exceeding size limits are rejected.

---

### INT-002: Prescription Removal
- **Trigger:** Click on trash button in thumbnail gallery (`removePatientPrescriptionImage(index)`).
- **Current State:** File exists at index in `patientPrescriptionImages`.
- **Action:** Splices image from array: `patientPrescriptionImages.splice(index, 1)`.
- **State Mutation:** Re-indexes remaining items.
- **UI Change:** Re-renders `#patientPrescriptionList` and `#wrPrescriptionThumbnailsStrip`.
- **Data Change:** Array decremented by 1.
- **Navigation Consequence:** Remains on current screen.
- **Backend Equivalent (Rust):** `DELETE /api/v1/appointments/{id}/prescriptions/{document_id}`.

---

### INT-003: Multi-Page Prescription Inspection
- **Trigger:** Click on prescription thumbnail in Waiting Room or Gallery (`openMultiPagePrescriptionViewer(pageIndex)`).
- **Current State:** Document gallery visible.
- **Action:** Opens full-screen inspection modal `#labReportModal`.
- **State Mutation:** Sets `currentActivePrescriptionPageIndex = pageIndex`.
- **UI Change:**
  - Sets `#labReportModal.style.display = 'flex'`.
  - Displays target image in `#pdfUploadedPhotoImg`.
  - Displays pagination controls: `#pdfMultiPageBar.style.display = 'flex'`.
  - Updates indicator: `#pdfMultiPageDisplay.textContent = "Page X of Y"`.
- **Navigation Consequence:** Modal overlay over current view.
- **Backend Equivalent (Rust):** Client-side cached image rendering or pre-signed S3 URL retrieval.

---

### INT-004: Doctor Earnings 3-Tier Calculation & Wallet Rendering
- **Trigger:** Doctor navigates to `doc-view-3` via `switchDoctorTab(3)`.
- **Current State:** Raw consultation transactions exist in `doctorEarningsStore`.
- **Action:** Invokes `renderDoctorEarnings()`.
- **State Mutation:**
  - Reads `doctorEarningsStore.totalGross` (e.g., `35562.50`).
  - Computes withheld fee: `withheldFee = totalGross * 0.20` (`7112.50`).
  - Computes net earnings: `finalNet = totalGross - withheldFee` (`28450.00`).
- **UI Change:**
  - Injects `৳ 35,562.50` into `#docGrossEarningsVal`.
  - Injects `- ৳ 7,112.50` into `#docWithheldFeeVal`.
  - Injects `৳ 28,450.00` into `#docFinalEarningsVal`.
  - Displays mandatory note `#docEarningsCalcNote`: *"Total calculation is based including the platform charge 20%."*
  - Populates itemized transaction ledger `#docConsultationsLedgerContainer`.
- **Backend Equivalent (Rust):** `GET /api/v1/doctor/wallet/summary` returning JSON:
  ```json
  {
    "total_gross": 35562.50,
    "platform_fee_percent": 20.0,
    "withheld_fee": 7112.50,
    "final_net": 28450.00,
    "basis_note": "Total calculation is based including the platform charge 20%."
  }
  ```

---

### INT-005: Clinical Grievance Submission & Telemetry Correlation
- **Trigger:** Form submit click on `#patientGrievanceModal` (`submitPatientGrievance()`).
- **Current State:** Modal open with pre-filled consultation details.
- **Action:** Validates input fields and packages the claim with auto-captured session telemetry.
- **State Mutation:**
  - Reads description from `#pgmDescriptionInput` and category from `#pgmCategorySelect`.
  - Determines target (`DOCTOR` vs. `SYSTEM`).
  - Reads session telemetry record from `consultationTelemetryStore`.
  - Creates dispute object with status `PENDING_REVIEW` and board remedy `Under Governance Review`.
  - Prepends record to `adminPatientGrievancesStore`.
  - If target is `SYSTEM`, automatically logs incident into `adminAppErrorLogsStore` under component `PatientGrievanceIncidentReporter`.
- **UI Change:**
  - Closes `#patientGrievanceModal`.
  - Displays in-app confirmation toast: *"Grievance Lodged: Medical Administration is reviewing your claim."*
  - Increments grievance badge in Admin Portal `#adminGrievanceNavBadge`.
- **Backend Equivalent (Rust):** `POST /api/v1/grievances` (stores report in DB and emits notification to admin dashboard).
- **Error Handling:** If statement text is empty (< 10 chars), shows validation warning: *"Please provide details regarding the issue."*

---

### INT-006: Admin Dispute Adjudication (payment hold Refund Disbursal)
- **Trigger:** Admin clicks "Disburse Refund" (`adjudicateGrievanceRefund(grvId)`).
- **Current State:** Dispute in `PENDING_REVIEW` state; payment hold funds in `PAYMENT_HELD`.
- **Action:**
  - Updates grievance status to `REFUNDED`.
  - Sets board remedy to `payment hold Refund Disbursed (৳800)`.
  - Logs audit entry with timestamp and admin identifier.
  - Updates matching transaction in `adminTransactionStore` to `REFUNDED`.
- **UI Change:**
  - Updates docket row badge to `REFUNDED` (Purple / Blue accent).
  - Displays toast: *"Refund Disbursed: Full fee refunded to patient wallet via bKash."*
- **Backend Equivalent (Rust):** `POST /api/v1/admin/grievances/{id}/refund` executes transactional payment hold release via MFS payment gateway API.

---

### INT-007: Admin Dispute Adjudication (Internal Platform Compliance Warning)
- **Trigger:** Admin clicks "Issue Warning" (`adjudicateGrievanceWarning(grvId)`).
- **Current State:** Dispute filed against physician conduct.
- **Action:**
  - Updates grievance status to `WARNED`.
  - Sets board remedy to `Internal Platform Compliance Warning Logged`.
  - Appends an internal compliance entry into the target doctor's platform dossier in `adminDoctorHistoryStore`.
- **UI Change:**
  - Updates docket status.
  - Displays toast: *"Doctor Warned: Formal clinical misconduct warning registered."*
- **Backend Equivalent (Rust):** `POST /api/v1/admin/grievances/{id}/warn` appends disciplinary record to `doctor_dossiers`.

---

### INT-008: Bilingual Language Switching (English / Swahili)
- **Trigger:** Selection and confirmation in `#languageModal` (`confirmAppLanguage()`).
- **Current State:** User in Account settings (`view-7`).
- **Action:** Reads selected language code (`'en'` or `'sw'`).
- **State Mutation:** Writes to `localStorage.setItem('helodoc_lang', code)`.
- **UI Change:**
  - Updates `#patientActiveLangBadge` text (`"English"` or `"Swahili"`).
  - Closes `#languageModal`.
  - Shows confirmation toast: *"Language Updated: Application language switched."*
- **Backend Equivalent (Rust):** `PATCH /api/v1/patient/preferences` with body `{"language": "sw"}`.

---

### INT-009: Offline Action Interception
- **Trigger:** User initiates payment or appointment booking while `navigator.onLine === false`.
- **Current State:** Browser detected offline network event.
- **Action:** Network check intercepts event prior to dispatching state changes.
- **UI Change:**
  - Displays `#offlineActionGuardModal`.
  - Explains network requirements and informs user that actions are preserved.
- **Backend Equivalent (Rust):** Server rejects stale requests using idempotent request tokens (`Idempotency-Key` header).
