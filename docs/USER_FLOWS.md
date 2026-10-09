# Phase 3 — End-to-End User Journeys & Reverse-Engineered Flows

> **Document Version:** 1.0.0  
> **Source Baseline:** `index.html` (and `prototype/index.html`)  
> **Target Audience:** Claude Code Autonomous Implementation Agent

---

## 1. Flow Overview & Architectural Classification

This document reverse-engineers all user journeys found in the HelloDoctor platform. Every flow is defined with state transitions, decision points, alternate branches, and error-recovery paths.

Format standard:
`START` ──> `STEP` ──> `DECISION` ──> `ACTION` ──> `RESULT`

---

## 2. Patient Journeys

### FLOW-PATIENT-001: Specialist Discovery & Modality Filtering
```mermaid
flowchart TD
    START([Patient Opens App]) --> A[View-0 Home Dashboard]
    A --> B[Tap 'Doctor Consultation' Card]
    B --> C[View-3 Find a Specialist Screen]
    C --> D{Select Modality Tab}
    D -- 'All Doctors' --> E[Show Complete Specialist List]
    D -- 'Live Video' --> F[Filter for Video-Enabled Physicians]
    D -- 'Instant Chat' --> G[Filter for On-Duty Chat Doctors]
    E & F & G --> H{Select Specialty Filter Chip}
    H -- 'Internal Medicine' --> I[Filter by Specialty]
    H -- 'Cardiology' --> J[Filter by Cardiology]
    I & J --> K[Inspect Doctor Cards: KMPDC ID, Exp, Hospital, Fee]
    K --> RESULT([Select Doctor -> Advance to Slot Picker])
```
- **Happy Path:** Patient selects modality tab, taps specialty chip (e.g., "Internal Medicine"), browses doctor cards displaying verified KMPDC numbers and fees, and taps "Book Appointment".
- **Alternate Path:** Patient taps on doctor card directly to view full qualifications in `view-9` (Doctor Profile) before booking.
- **Empty State:** If no physicians match the filter, the view displays: *"No specialists available for the selected criteria."*
- **Offline State:** Intercepted by `#patientOfflineBanner`; cached doctor directory is shown with a *"Reconnecting..."* status badge.

---

### FLOW-PATIENT-002: Appointment Booking, Multi-Rx Upload (1–5 Photos) & payment hold Payment
```mermaid
flowchart TD
    START([Tap 'Book Appointment' on Doctor Card]) --> A[View-4 Date & Slot Booking Engine]
    A --> B[Select Date Chip: Today / Tomorrow / Upcoming]
    B --> C[Select 15-min Time Slot Chip]
    C --> D[Scroll to Multi-Prescription Upload Dropzone]
    D --> E{Patient Selects Files via Native Picker}
    E -- Total Photos <= 5 --> F[Add Images to Intake Queue]
    E -- Total Photos > 5 --> G[Cap at 5 & Trigger Warning Toast]
    F & G --> H[Render Thumbnail Gallery with Page Labels & Trash Buttons]
    H --> I{Review Attached Documents?}
    I -- Yes --> J[Tap Thumbnail -> Open Multi-Page Viewer Modal]
    I -- No --> K[Select Payment Gateway: bKash / Nagad / M-Pesa]
    J --> K
    K --> L[Tap 'Confirm & Pay ৳ 800']
    L --> M{Is Network Online?}
    M -- Yes --> N[Authorize MFS payment hold Hold & Lock Slot]
    M -- No --> O[Trigger #offlineActionGuardModal & Abort]
    N --> RESULT([Redirect to View-10 Waiting Room])
```
- **Validation Rules:**
  - Date and time slot selection are mandatory before payment is enabled.
  - Image upload accepts `image/*,application/pdf` with a hard limit of 5 photos (`MAX_PRESCRIPTION_IMAGES = 5`)
    **as literally coded in the `index.html` prototype source**, retained as historical/original
    reference — the prototype file itself was not changed. **Production correction:** the actual
    backend/Doctor-Web cap is now 20 photos per consultation at 40 KB each
    (`PATIENT_MAX_IMAGES_PER_CONSULTATION = 20`, `MAX_PRESCRIPTION_IMAGE_BYTES = 40 * 1024`) — see
    docs/PRESCRIPTION_UPLOAD_40KB_20_IMAGE_VERIFICATION.md.
  - Excess photos are rejected/truncated past the cap, and the user receives a warning toast.
- **Payment payment hold Rule:** Funds are placed into `PAYMENT_HELD` in `adminTransactionStore`. The consulting doctor does not receive a balance credit until clinical consultation completes without dispute.

---

### FLOW-PATIENT-003: Virtual Waiting Room & Pre-Flight Device Check
```mermaid
flowchart TD
    START([Arrival in View-10 Waiting Room]) --> A[Start Live Countdown Clock]
    A --> B[Display Queue Position: 'You are #1 in line']
    B --> C[Render Attached Prescription Thumbnails]
    C --> D{Inspect Uploaded Prescriptions?}
    D -- Yes --> E[Tap Thumbnail -> View Full Screen via #labReportModal]
    D -- No --> F[Run Device Hardware Pre-Flight Check]
    E --> F
    F --> G{Camera & Mic Permissions Granted?}
    G -- Granted --> H[Display 'Camera & Mic Ready ✓']
    G -- Denied --> I[Display Warning Banner with Permission Guide]
    H --> J{Countdown Reaches Zero?}
    J -- Doctor Connects --> K[Trigger Incoming Call Audio Chime]
    K --> RESULT([Enter Live Agora SDK Consultation])
```
- **Document Review:** Patient can paginate through their uploaded prescriptions (pages 1 to 5) using `#btnPrevRxPage` and `#btnNextRxPage`.
- **Doctor Delay Handling:** If the physician is delayed by > 5 minutes, an informative status card advises the patient: *"Your doctor is finishing an urgent clinical case. Please stay on this screen."*

---

### FLOW-PATIENT-004: Live Agora SDK Teleconsultation & 24h Follow-up Chat
```mermaid
flowchart TD
    START([Incoming Call Accepted]) --> A[Initialize Agora SDK Agora SDK]
    A --> B[Start Session Telemetry Timer: callSeconds++]
    B --> C[Stream Adaptive 720p/1080p Video & Audio]
    C --> D{Connection Status Check}
    D -- Stable 4G/WiFi --> E[Continue Clinical Assessment]
    D -- Bandwidth Drops < 128kbps --> F[Fallback to Audio-Only Mode]
    D -- Agora Disconnects --> G[Auto-Reconnect Buffer for 30 Seconds]
    E --> H[Doctor Concludes Call & Signs Prescription]
    H --> I[End Agora SDK Session & Save Telemetry Record]
    I --> J[Activate 24-Hour Asynchronous Chat Window]
    J --> K[View-6 Clinical Chat: Dosage Questions & Canned Pills]
    K --> L{24 Hours Elapsed?}
    L -- Yes --> M[Lock Chat Session to Read-Only]
    L -- No --> K
    M --> RESULT([View-11 Post-Consultation Summary])
```
- **Telemetry Auto-Capture:** Every session records actual connected duration (`callSeconds`), Agora connection state, and prescription generation state.
- **Premature Call Drop:** If the call terminates under 30 seconds (`callSeconds < 30`), the session is automatically tagged with `prematureEnd: true`, keeping funds in payment hold pending patient reconnection or grievance filing.

---

### FLOW-PATIENT-005: Clinical Grievance Redressal (Star-Rating-Free Adjudication)
```mermaid
flowchart TD
    START([Patient Taps 'Report an Issue' on View-11 or View-7]) --> A[Open #patientGrievanceModal]
    A --> B[Auto-Populate Doctor Name, KMPDC ID, Consult ID, payment hold Fee]
    B --> C[Silently Bind Auto-Collected Session Telemetry in Background]
    C --> D{Select Incident Target}
    D -- DOCTOR --> E[Render Clinical Conduct Categories]
    D -- SYSTEM --> F[Render Technical Failure Categories]
    E --> G[Select Issue: Rushed Call / No Rx / Abrupt Disconnect]
    F --> H[Select Issue: Agora SDK Video Freeze / MFS Debit Failed]
    G & H --> I[Enter Statement Details in Textarea]
    I --> J[Tap 'Submit Grievance to Medical Board']
    J --> K[Register Incident in adminPatientGrievancesStore as PENDING_REVIEW]
    K --> L{Is Target SYSTEM?}
    L -- Yes --> M[Correlate & Ingest into adminAppErrorLogsStore]
    L -- No --> N[Route to Central Grievance Board Queue]
    M & N --> RESULT([Show In-App Confirmation Toast & Close Modal])
```
- **Zero Star Ratings:** No star rating or public review interface is presented.
- **Privacy Rule:** Raw telemetry metrics (`pgmTelemetryInspector`, `v11TelemetryCard`) are marked with `style="display:none;" aria-hidden="true"`, remaining hidden from the patient while providing the Medical Board with full forensic visibility.
- **Governance Rule:** Patients describe their experience and provide notes, but cannot assign punitive remedies; resolution is reserved for Central Board adjudication.

---

### FLOW-PATIENT-006: Emergency Contact Helpline (Call 16263)
```mermaid
flowchart TD
    START([Patient in Medical Distress Launches App]) --> A[View-0 Home Dashboard]
    A --> B[Emergency Contact Banner Located Directly Above Upcoming Appointments]
    B --> C[Verify Banner Label: 'Emergency Contact' (No '24/7' prefix)]
    C --> D[Tap 'Call 16263' Button]
    D --> E[Trigger System URL Scheme: tel:16263]
    E --> RESULT([Native Phone Dialer Connects to National Emergency Triage])
```

---

### FLOW-PATIENT-007: Bilingual Localization (English & Swahili)
```mermaid
flowchart TD
    START([Patient Opens View-7 Account Settings]) --> A[Tap 'Language (English / Swahili)' Menu Item]
    A --> B[Open #languageModal]
    B --> C{Select Language Option}
    C -- 'English' --> D[Highlight English Checkmark]
    C -- 'Swahili (Kiswahili)' --> E[Highlight Swahili Checkmark]
    D & E --> F[Tap 'Apply Language']
    F --> G[Update localStorage with Language Code]
    G --> H[Update UI Labels & dynamic badge #patientActiveLangBadge]
    H --> RESULT([Display Bilingual Confirmation Toast])
```

---

## 3. Doctor Journeys

### FLOW-DOC-001: Doctor Queue Triage & Duty Toggle
- **Trigger:** Doctor logs in or opens `doc-view-0`.
- **Duty Toggle:** Doctor toggles the on-duty switch (`toggleDoctorDuty()`). The backend updates physician availability status and begins routing incoming consultations.
- **Queue Review:** Doctor reviews the queue list — **DOCTOR_QUEUE_UPCOMING_FIRST (2026-10-07, approved product rule superseding the prior "sorted by appointment time" description)**: eligible upcoming consultations always sort first (nearest `start_time` first), with completed/cancelled/other items following newest-first and never displacing an upcoming one, enforced server-side (`GET /doctor/appointments`) — inspects patient age, reported symptoms, and checks for attached multi-prescription photos.

---

### FLOW-DOC-002: Prescription Photo Upload & E-Prescription Dispatch
- **Step 1:** In `doc-view-1`, the doctor selects an active patient from queue pills.
- **Step 2:** Doctor inspects attached patient documents using the multi-page previewer.
- **Step 3:** Doctor conducts the consultation, then writes the prescription on paper.
- **Step 4:** Doctor takes a photo of the handwritten prescription with their phone/webcam and uploads it via the app. (Alternatively, if no prescription is needed, Doctor taps 'Complete Without Prescription', enters a brief reason, and the consultation is marked as COMPLETED_NO_RX).
- **Step 5:** Doctor clicks "Upload & Dispatch".
- **Result:** The system processes the photo through the security pipeline (malware scan, EXIF stripping), saves it to S3, computes an integrity hash, and delivers the photo to the patient's Health Vault (`view-5`). This unlocks the 24-hour follow-up chat window.

---

### FLOW-DOC-003: Doctor Earnings & 20% Debarred Settlement
```mermaid
flowchart TD
    START([Doctor Opens doc-view-3 Wallet]) --> A[Calculate Total Gross Earnings]
    A --> B[Calculate HeloDoc Platform Charge: Gross * 0.20]
    B --> C[Calculate Final Net Take-Home: Gross - Platform Charge]
    C --> D[Render #docEarningsHeroCard with 3-Tier Breakdown]
    D --> E[Render Mandatory Basis Note: 'Total calculation is based including the platform charge 20%']
    E --> F[Render Itemized Ledger Rows: Gross -> 20% Withheld -> Net Take-Home]
    F --> G{Doctor Actions}
    G -- View Monthly Statement --> H[Open #doctorStatementModal]
    G -- View Single Consult Fee --> I[Open #docConsultationFeeModal]
    G -- View Expected Next Disbursement --> J[View Details of Upcoming Payout]
    J --> RESULT([Note: Earnings are disbursed monthly by Finance Admin])
```

---

### FLOW-DOC-004: Doctor Engagement Notifications (2026-10-07)
- **DOCTOR_DAILY_CONSULTATION_NOTIFICATION:** Once per Kenya-local calendar
  day, at a configurable local time (`DOCTOR_DAILY_SUMMARY_TIME_LOCAL`,
  default 07:00 Africa/Nairobi), each verified active Doctor receives one
  in-app `DAILY_CONSULTATION_SUMMARY` notification counting that day's
  eligible (non-cancelled/expired) consultations — "You have N consultations
  scheduled today" / "You have no consultations scheduled today." Backend
  scheduler, deduplicated per (Doctor, local date) — never created twice.
- **DOCTOR_T15_CONSULTATION_REMINDER:** For each eligible (`CONFIRMED`/
  `WAITING`) consultation, a `CONSULTATION_REMINDER_15M` notification is
  created the moment the window `[start_time - 15m, start_time)` opens — "Your
  consultation starts in 15 minutes," no patient-identifying detail. A
  booking made with under 15 minutes' notice gets an immediate reminder, not
  a skipped/past-dated one. Cancelling the appointment suppresses any
  still-pending (undelivered) reminder.
- **DOCTOR_EARNING_NOTIFICATION:** The moment (and only the moment) a
  consultation's wallet credit genuinely posts inside
  `doctor_complete_consultation` (`PAYMENT_HELD -> SETTLED_TO_DOCTOR`), the
  Doctor receives one `EARNING_CREDITED` notification with the real net
  amount from the ledger — "Consultation earning credited: KES 800." A
  retried/duplicate completion call never creates a second one. Checkout,
  `PAYMENT_HELD`, Finance preparing/approving a disbursement, and Finance
  "approval" never trigger this or any payout-received notification —
  `PAYOUT_PAID` is a separate, not-yet-wired event gated on a real external
  provider `PAID` transition that does not exist in this codebase yet.
- **DOCTOR_VERIFICATION_NOTIFICATION:** A ClinicalAdmin/PlatformAdmin
  transition of `doctor_profiles.is_verified` creates exactly one
  `REGISTRATION_VERIFIED` (on a genuine `false -> true` transition) or
  `REGISTRATION_REJECTED` (deduplicated by doctor profile, so a repeated
  rejection of an already-rejected/pending doctor is a safe no-op) in-app
  notification. No sensitive review detail is placed in the notification
  text.
- **Delivery status:** `PUSH_PROVIDER_CONFIGURATION_REQUIRED` — no FCM/APNs
  integration exists; every notification above is created and fully usable
  as an in-app inbox item (`GET /notifications`, unread count, mark-read),
  but no push is actually sent to a device. The device-token registration
  contract (`POST /notifications/devices`) exists and is disabled (not
  deleted) on logout.
- **Not yet built:** the Flutter notification bell/inbox UI and deep-link
  destinations (Queue / consultation / Wallet / Profile / My Articles) — a
  separate, not-yet-started Flutter-side task.

---

## 4. Central Administration Journeys

### FLOW-ADMIN-001: Doctor Dossier Review (Contact & History)
- **Step 1:** Administrator opens `adminPage_doctors`.
- **Step 2:** Locates physician record (e.g., Dr. Synthetic Doctor Two, SYNTH-KE-0002).
- **Step 3:** Clicks "View Dossier", opening `#adminDoctorHistoryModal`.
- **Step 4:** Administrator inspects verified contact information:
  - **Phone Number:** `+254700000101`
  - **Residential Address:** `Synthetic Nairobi, Kenya`
  - **Official Email:** `dr.anika.dmch@helodoc.com`
  - **Consultation History & Past Disciplinary Warnings**.

---

### FLOW-ADMIN-002: Grievance Adjudication & Dispute Settlement
```mermaid
flowchart TD
    START([Admin Opens adminPage_grievances]) --> A[Select Dispute from Docket]
    A --> B[Open #adminGrievanceDetailModal]
    B --> C[Compare Patient Statement with Auto-Captured Session Telemetry]
    C --> D[Verify Call Duration: e.g. 0m 08s vs Scheduled 15m]
    D --> E[Inspect Agora SDK Diagnostic Logs: Agora Connection Drop]
    E --> F[Inspect Prescription Status: Not Issued]
    F --> G{Board Adjudication Decision}
    G -- Patient Claim Justified --> H[Click 'Disburse Refund' #btnAdjudicateRefund]
    G -- Physician Misconduct --> I[Click 'Issue Warning' #btnAdjudicateWarning]
    G -- Unsubstantiated Claim --> J[Click 'Resolve / Dismiss' #btnAdjudicateResolve]
    H --> K[Disburse payment hold Refund to Patient bKash -> Status REFUNDED]
    I --> L[Log Internal Platform Compliance Warning in Physician Dossier -> Status WARNED]
    J --> M[Release payment hold to Doctor -> Status DISMISSED]
    K & L & M --> RESULT([Update Master Ledger & Notify Both Parties])
```

---

### FLOW-ADMIN-003: Subsystem Diagnostics & Failure Simulation
- **Step 1:** Administrator opens `adminPage_logs`.
- **Step 2:** Filters logs by subsystem (`MFS_BKASH_GATEWAY`, `AGORA_RTC`, `UPLOAD_SECURITY_PIPELINE`).
- **Step 3:** Clicks trace ID (e.g., `TRC-94812-BKASH`) to inspect error stack trace in `#adminLogDetailModal`.
- **Step 4:** To test system resilience, clicks "Simulate Failure", opening `#adminSimulateFailureModal`.
- **Step 5:** Selects scenario (e.g., *bKash IPN Webhook Timeout*), sets severity (*CRITICAL*), and clicks "Execute Simulation".
- **Result:** System simulates the incident, verifies that user-facing error banners display properly, and logs the incident in the audit trail without disrupting live production traffic.

---

### FLOW-ADMIN-004: Monthly Payment Disbursement
- **Step 1:** Administrator opens `adminPage_finance`.
- **Step 2:** Clicks "Initiate Monthly Disbursement".
- **Step 3:** Selects date range (e.g., Previous Month).
- **Step 4:** System groups all unsettled `SETTLED_TO_DOCTOR` transactions, calculates totals per doctor.
- **Step 5:** Admin confirms batch. Payouts are dispatched to doctors' MFS accounts. Doctor wallets are updated with `last_disbursement_at` and `last_disbursement_amount`.
