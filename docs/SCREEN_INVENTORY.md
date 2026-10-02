# Phase 2 — Complete Screen & Modal Inventory

> **Document Version:** 1.0.0  
> **Source Baseline:** `index.html` (and `prototype/index.html`)  
> **Target Audience:** Claude Code Autonomous Implementation Agent

---

## 1. Inventory Summary & Classification Scheme

Every screen, viewport, modal, and overlay present in the HelloDoctor prototype has been cataloged with a stable identifier (`HD-*`), hierarchical component trees, interaction models, and state variations.

Labels used:
- `OBSERVED`: Directly verified in DOM structure, CSS, or JS event handlers.
- `INFERRED`: Logically deduced from state transitions and navigation flows.
- `RECOMMENDED`: Required for production Flutter/Rust architecture.
- `UNKNOWN`: Not determinable from prototype; flagged for safe defaults.

---

## 2. Patient Mobile Portal (`#patientAppShell`)

### HD-PATIENT-HOME (`view-0`)
- **Screen Name:** Patient Home Dashboard
- **User Role:** `patient`
- **Purpose:** Primary landing screen for registered patients. Provides immediate access to emergency hotlines, upcoming appointments, and core clinical service entry points.
- **Entry Points:** App launch (default tab), bottom navigation "Queue/Home" item, back-navigation from specialist booking.
- **Exit Points:** 
  - `switchScreen(1)`: Core Services Directory
  - `switchScreen(3)`: Find a Specialist Directory
  - `switchScreen(7)`: Patient Account & Family Profiles
  - `switchScreen(6)`: Active Consultation Chat
  - `tel:16263`: System Phone Dialer (Emergency Contact)
- **UI Hierarchy:**
  ```
  HD-PATIENT-HOME
  └── Header Bar (Greeting "Rafiq Ahmed 👋", Notification Bell, Profile Avatar)
  └── Core Services 2x2 Grid
      ├── "Doctor Consultation" Card (Instant Video & Chat)
      ├── "Order Medicine" Card (Pharmacy Delivery)
      ├── "Diagnostic Tests" Card (Home Sample Collection)
      └── "Health Vault" Card (Prescriptions & Lab Reports)
  └── Emergency Contact Helpline Strip (#patientEmergencyHelplineStrip)
      ├── Ambulance Icon & Urgent Red Accent (#DC2626)
      ├── Label "Emergency Contact" (Strictly without "24/7" prefix)
      └── One-Tap Dialing CTA "Call 16263"
  └── Upcoming Consultation Section
      └── Active Consultation Card (Dr. Sabrina Akter, Video, Countdown Timer)
  └── Bottom Navigation Bar (Queue/Home, Services, Schedule, Blogs, Account)
  ```
- **Components:** Top header, 2x2 grid cards, emergency helpline banner, upcoming appointment hero card, bottom nav bar.
- **Interactions:**
  - Tapping avatar -> navigates to Account (`view-7`).
  - Tapping Emergency Contact -> invokes native dialer `tel:16263`.
  - Tapping "Doctor Consultation" -> navigates to Find Specialist (`view-3`).
  - Tapping Upcoming Appointment -> navigates to Waiting Room (`view-10`) or Consultation Chat (`view-6`).
- **State Variations:**
  - `initial`: Hero card displays scheduled appointment with countdown.
  - `loading`: Skeleton shimmer cards (`view-14`) rendered during fetch.
  - `empty`: If no upcoming appointment, card renders "No active consultations. Book a doctor today."
  - `network failure`: Displays `#patientOfflineBanner` at the top of the viewport.
- **Data Requirements:** Patient full name, upcoming consultation object (doctor name, specialty, scheduled timestamp, modality, appointment ID).
- **Backend Requirements (Rust):** `GET /api/v1/patient/home-summary`, `GET /api/v1/appointments/upcoming`.
- **Prototype Reference:** `index.html` lines containing `id="view-0"`.
- **Visual Requirements:**
  - Background: `#F8F6F0` (`OBSERVED`).
  - Emergency Banner Background: `#FEE2E2`, Border: `#FCA5A5`, Text: `#DC2626` (`OBSERVED`).
  - Card Corner Radius: `16px` (`OBSERVED`).

---

### HD-PATIENT-SERVICES (`view-1`)
- **Screen Name:** Core Healthcare Services Hub
- **User Role:** `patient`
- **Purpose:** Comprehensive directory of all clinical and diagnostic offerings.
- **Entry Points:** Bottom navigation "Services" tab, home screen service card.
- **Exit Points:** 
  - `switchScreen(3)`: Book a Specialist Doctor
  - `openDoctorDirectoryForChat()`: Instant Chat Consultation
  - `switchScreen(12)`: Free Health Q&A
  - `switchScreen(5)`: Health Vault (EMR)
  - `openPharmacyCheckout()`: Medicine Home Delivery Modal
- **UI Hierarchy:**
  ```
  HD-PATIENT-SERVICES
  └── Top Search Bar ("Search symptoms, doctors, or tests")
  └── Primary Service Section: "All Available Services"
      ├── "Book a Specialist Doctor" (Video Telehealth, 800 BDT)
      ├── "Instant GP Chat" (General Practitioner, 500 BDT)
      ├── "Prescription Medicine Order" (Home delivery in 2 hours)
      └── "Home Diagnostic Sample Collection" (CBC, Lipid, HbA1c)
  └── Bottom Navigation Bar
  ```
- **Components:** Search header, service cards with iconography and pricing tags, bottom nav.
- **Interactions:**
  - Tapping "Book a Specialist Doctor" opens Directory (`view-3`).
  - Tapping "Instant GP Chat" launches live doctor chat queue.
- **State Variations:** Standard populated view; search filter state dynamically hides non-matching service cards.
- **Backend Requirements (Rust):** `GET /api/v1/services/catalog`.
- **Prototype Reference:** `index.html` lines containing `id="view-1"`.

---

### HD-PATIENT-FIND-SPECIALIST (`view-3`)
- **Screen Name:** Specialist Discovery & Directory
- **User Role:** `patient`
- **Purpose:** Allows patients to browse and filter BMDC-verified specialist physicians without commercial star ratings.
- **Entry Points:** Home card "Find Doctor", Services tab "Book Specialist".
- **Exit Points:** 
  - `switchScreen(9)`: Doctor Public Profile & Credentials
  - `switchScreen(4)`: Date & Slot Booking Engine
  - `switchScreen(0)`: Back to Home
- **UI Hierarchy:**
  ```
  HD-PATIENT-FIND-SPECIALIST
  └── Top Navigation Bar (Back Arrow, Title "Find a Specialist")
  └── Modality Selector Tabs:
      ├── "All Doctors" (`setDoctorDirectoryModality('all')`)
      ├── "Live Video" (`setDoctorDirectoryModality('video')`)
      └── "Instant Chat" (`setDoctorDirectoryModality('chat')`)
  └── Specialty Filter Chips Carousel:
      ├── "All Specialties"
      ├── "Internal Medicine"
      ├── "Cardiology"
      ├── "Dermatology"
      └── "Pediatrics"
  └── Verified Doctor Card List:
      ├── Doctor Portrait Avatar
      ├── Doctor Name (e.g., "Dr. Sabrina Akter")
      ├── BMDC Registration ID (e.g., "BMDC #45821")
      ├── Clinical Experience Badge (e.g., "12 Years Exp")
      ├── Hospital Affiliation (e.g., "Dhaka Medical College Hospital")
      ├── Consultation Fee (e.g., "৳ 800")
      └── Action Button "Book Appointment"
  ```
- **Components:** Modality tab bar, horizontal scrolling filter chips, specialist cards, booking CTA buttons.
- **Interactions:**
  - Tapping modality tab filters list by video/chat availability.
  - Tapping specialty chip filters doctor list in real time.
  - Tapping doctor card opens Doctor Profile (`view-9`).
  - Tapping "Book Appointment" advances to Slot Picker (`view-4`).
- **State Variations:**
  - `populated`: Displays list of matching physicians.
  - `empty`: "No specialists available for the selected criteria."
  - `loading`: Animated shimmer cards (`.sk-shimmer`).
- **Data Requirements:** Doctor ID, name, BMDC number, specialty, experience years, hospital, fee, modality flags.
- **Backend Requirements (Rust):** `GET /api/v1/doctors?modality=video&specialty=Internal+Medicine&page=1&limit=20`.
- **Prototype Reference:** `index.html` lines containing `id="view-3"`.

---

### HD-PATIENT-BOOK-SLOT (`view-4`)
- **Screen Name:** Appointment Scheduling & Multi-Rx Intake
- **User Role:** `patient`
- **Purpose:** Selects consultation date, 15-minute slot, uploads 1 to 5 previous prescription/lab photos, and initiates MFS escrow payment.
- **Entry Points:** Doctor profile or directory booking button.
- **Exit Points:**
  - `confirmAppointment()`: Moves to Waiting Room (`view-10`) upon payment.
  - Back arrow: Returns to Specialist Directory (`view-3`).
- **UI Hierarchy:**
  ```
  HD-PATIENT-BOOK-SLOT
  └── Top App Bar (Back Arrow, Title "Select Date & Time")
  └── Selected Doctor Summary Header (Avatar, Name, BMDC, Fee)
  └── Calendar Date Carousel (Today, Tomorrow, Day + 2, Day + 3)
  └── Time Slot Grid (Morning, Afternoon, Evening 15-min chips)
  └── Multi-Prescription Upload Dropzone (#patientUploadDropzone)
      ├── Hidden File Input (#patientPreConsultFileInput multiple)
      ├── Instructions ("Upload up to 5 previous prescription or lab photos")
      └── Uploaded Thumbnail Gallery Strip (#patientPrescriptionGalleryContainer)
          └── Itemized Thumbnails with Page Numbers & Delete Buttons
  └── Payment Gateway Selector (bKash, Nagad, Card / M-Pesa)
  └── Sticky Footer CTA ("Confirm & Pay ৳ 800")
  ```
- **Components:** Doctor header snippet, date picker carousel, slot chips, multi-photo file dropzone, thumbnail preview list, gateway radio pills, fixed bottom checkout bar.
- **Interactions:**
  - Tapping date chip updates available slot chips.
  - Selecting file invokes `handlePatientFileSelection(e)`:
    - Enforces `MAX_PRESCRIPTION_IMAGES = 5`.
    - Triggers warning toast if user selects > 5 files.
    - Appends valid files to `patientPrescriptionImages` array and updates preview gallery.
  - Tapping thumbnail trash icon invokes `removePatientPrescriptionImage(index)`.
  - Tapping "Confirm & Pay" locks slot, deposits funds into escrow (`adminTransactionStore`), and transitions to Waiting Room.
- **State Variations:**
  - `slot unselected`: Confirm button disabled (`opacity: 0.5`).
  - `uploading`: Renders progress indicator on thumbnail.
  - `max upload reached`: Dropzone visually disables add button.
  - `offline`: Intercepted by `#offlineActionGuardModal`.
- **Data Requirements:** Doctor schedule slots, uploaded image blobs/data URLs, selected gateway, patient identifier.
- **Backend Requirements (Rust):** `GET /api/v1/doctors/{id}/slots?date=YYYY-MM-DD`, `POST /api/v1/appointments/book` (atomic slot reservation with escrow payment authorization).
- **Prototype Reference:** `index.html` lines containing `id="view-4"`.

---

### HD-PATIENT-WAITING-ROOM (`view-10`)
- **Screen Name:** Virtual Pre-Flight Waiting Room
- **User Role:** `patient`
- **Purpose:** Pre-consultation holding area where the patient monitors queue position, countdown timer, checks camera/mic permissions, and reviews uploaded prescription photos.
- **Entry Points:** Successful booking from `view-4`, home screen active consultation card.
- **Exit Points:**
  - `simulateIncomingDoctorCall()`: Doctor initiates WebRTC video call.
  - `openMultiPagePrescriptionViewer(0)`: Launches full-screen document inspection modal.
  - `switchScreen(0)`: Return to home while preserving queue position.
- **UI Hierarchy:**
  ```
  HD-PATIENT-WAITING-ROOM
  └── Top Status Bar (Queue Position "You are #1 in line", Scheduled Time)
  └── Live Countdown Timer Card (e.g., "Consultation starts in 04:32")
  └── Doctor Information Card (Dr. Sabrina Akter, BMDC #45821)
  └── Uploaded Prescriptions Strip (#waitingRoomPrescriptionCard)
      ├── Badge (#wrPrescriptionCountBadge "3 Photos Attached")
      └── Horizontal Thumbnail Strip (#wrPrescriptionThumbnailsStrip)
  └── Device Pre-Flight Check Panel:
      ├── Camera Permission Status ("Ready ✓")
      ├── Microphone Permission Status ("Ready ✓")
      └── Network Latency Indicator ("4G / Low Latency")
  └── Emergency Cancel / Reschedule CTA
  ```
- **Components:** Queue progress bar, countdown clock, attached prescription carousel, hardware check list, call-in trigger button.
- **Interactions:**
  - Tapping any prescription thumbnail opens `#labReportModal` via `openMultiPagePrescriptionViewer(pageIndex)`.
  - Tapping "Test Hardware" triggers browser device permission prompts and shows toast: *"Camera & Mic Ready"*.
- **State Variations:**
  - `waiting`: Countdown active.
  - `doctor ready`: Sound chime alert plays, banner turns Emerald Green (`#059669`).
  - `doctor late`: After 5 minutes past schedule, displays: *"Doctor is finishing an urgent case. Please stay on this screen."*
- **Backend Requirements (Rust):** WebSocket queue updates (`ws://.../patient/queue/{appointment_id}`).
- **Prototype Reference:** `index.html` lines containing `id="view-10"`.

---

### HD-PATIENT-POST-CONSULT (`view-11`)
- **Screen Name:** Post-Consultation Summary & Telemetry Record
- **User Role:** `patient`
- **Purpose:** Displays consultation completion details, download link for the signed digital prescription, follow-up advice, and provides access to the Clinical Grievance Redressal modal.
- **Entry Points:** Termination of video consultation or end of 24h chat window.
- **Exit Points:**
  - `switchScreen(5)`: Open Health Vault to inspect prescription PDF.
  - `openPatientGrievanceModal(...)`: File a clinical or system grievance.
  - `switchScreen(0)`: Return to Home Dashboard.
- **UI Hierarchy:**
  ```
  HD-PATIENT-POST-CONSULT
  └── Completion Hero Icon (Emerald Checkmark)
  └── Title "Consultation Completed Successfully"
  └── Doctor Summary (Dr. Sabrina Akter, Internal Medicine)
  └── Digital Prescription Card:
      ├── Prescription Reference ID ("RX-20260930-841")
      ├── Medication Count ("3 Medicines Prescribed")
      └── CTA "Download Official Prescription PDF"
  └── Hidden Forensic Telemetry Card (#v11TelemetryCard aria-hidden="true" style="display:none;")
      └── Background Session Record (Call Duration, Agora RTC RTT, Escrow ID)
  └── Grievance Trigger Button ("Having an issue with this consultation? Report to Medical Administration")
  └── Home Navigation CTA
  ```
- **Components:** Success header, digital Rx preview card, action buttons, hidden telemetry container.
- **Interactions:**
  - Tapping "Download Prescription" fetches official PDF from vault.
  - Tapping "Report Grievance" opens `#patientGrievanceModal` with pre-filled metadata.
- **Privacy & Security Rule:** Raw telemetry is strictly hidden from patient UI (`display: none; aria-hidden="true"`) to prevent confusion and maintain clinical trust, while remaining bound to background reporting payloads.
- **Backend Requirements (Rust):** `GET /api/v1/consultations/{id}/summary`, `GET /api/v1/prescriptions/{id}/pdf`.
- **Prototype Reference:** `index.html` lines containing `id="view-11"`.

---

### HD-PATIENT-ACCOUNT-SETTINGS (`view-7`)
- **Screen Name:** Patient Account, Family Profiles & Localization
- **User Role:** `patient`
- **Purpose:** Manages patient profile information, dependent family member records, application language settings (English / Swahili), and session logout.
- **Entry Points:** Top bar avatar on Home screen, bottom nav "Account" tab.
- **Exit Points:**
  - `openLanguageModal()`: Opens bilingual language selector modal (`#languageModal`).
  - `handleLogout()`: Clears authentication state and resets session.
- **UI Hierarchy:**
  ```
  HD-PATIENT-ACCOUNT-SETTINGS
  └── Profile Card (Patient Name "Rafiq Ahmed", Phone "+880 1712-345678", National ID)
  └── Family Profiles Section ("Add Dependent: Spouse, Children, Parents")
  └── Settings Menu:
      ├── Language Selector Item (#patientLangMenuItem)
      │   ├── Label: "Language (English / Swahili)"
      │   └── Dynamic Active Badge (#patientActiveLangBadge "English")
      ├── Notification Preferences
      ├── Security & Biometric Lock
      ├── Terms of Clinical Service & DGHS Compliance
      └── Logout Button (#DC2626)
  ```
- **Components:** Profile header, dependent pills, menu list tiles, language modal trigger.
- **Interactions:**
  - Tapping language item invokes `openLanguageModal()`.
  - Tapping logout invokes `handleLogout()`.
- **Backend Requirements (Rust):** `GET /api/v1/patient/profile`, `POST /api/v1/auth/logout`.
- **Prototype Reference:** `index.html` lines containing `id="view-7"`.

---

### Remaining Patient Viewports
- **HD-PATIENT-REMINDERS (`view-2`):** Floating toast and persistent pill reminders for upcoming 30-minute consultations and daily medication schedules.
- **HD-PATIENT-HEALTH-VAULT (`view-5`):** Longitudinal electronic medical record repository archiving past prescriptions, CBC lab reports, and clinical notes.
- **HD-PATIENT-CONSULT-CHAT (`view-6`):** 24-hour asynchronous clinical messaging console with canned medical queries and auto-expiry countdown.
- **HD-PATIENT-CONSULT-ROUTING (`view-8`):** Urgency triage screen directing patients to emergency helpline, urgent GP on-call, or scheduled specialist care.
- **HD-PATIENT-DOCTOR-PROFILE (`view-9`):** Full credentials view displaying BMDC registration, degrees, clinical experience, and hospital appointments without star ratings.
- **HD-PATIENT-FREE-QNA (`view-12`):** Community medical Q&A forum where patients post anonymous health questions answered by on-duty medical officers.
- **HD-PATIENT-BLOGS (`view-13`):** Health education feed offering preventative lifestyle, diabetes care, and epidemic guidance articles.
- **HD-PATIENT-LOADING-SHIMMER (`view-14`):** Global skeleton loader preventing layout shifts during high-latency network queries.

---

## 3. Doctor Mobile Portal (`#doctorAppShell`)

### HD-DOC-QUEUE (`doc-view-0`)
- **Screen Name:** Doctor On-Duty Queue & Patient Triage
- **User Role:** `doctor`
- **Purpose:** On-call physician dashboard displaying real-time patient queue, availability duty toggle, and quick triage shortcuts.
- **Entry Points:** Physician portal launch, bottom nav "Queue" tab.
- **Exit Points:**
  - `switchDoctorTab(1)`: Smart Rx Scanner & Dispatch
  - `switchDoctorTab(2)`: Consultation Schedule
  - `switchDoctorTab(3)`: Wallet & Earnings (20% Debarred)
  - `switchDoctorTab(4)`: 24h Consultation Chats
  - `switchDoctorTab(5)`: Open Q&A Triage
- **UI Hierarchy:**
  ```
  HD-DOC-QUEUE
  └── Doctor Top Profile Bar:
      ├── Avatar & Name (Dr. Sabrina Akter, BMDC #45821)
      └── Instant Duty Toggle Switch (`toggleDoctorDuty()`)
  └── Queue Search & Filter Bar
  └── Queue Filter Tabs: "All Patients", "Upcoming (8)", "Completed (14)"
  └── Live Patient Queue Cards:
      ├── Patient Name, Age, Gender (Rafiq Ahmed, 34M)
      ├── Reason for Consultation ("Persistent high fever & body aches")
      ├── Multi-Rx Attachment Indicator ("3 Prescriptions Uploaded")
      └── Action CTA: "Start Video Telehealth Call"
  └── Bottom Navigation Bar (Queue, Chat, Scan Rx, Schedule, Wallet, Q&A)
  ```
- **Backend Requirements (Rust):** `GET /api/v1/doctor/queue`, `POST /api/v1/doctor/duty-status`.
- **Prototype Reference:** `index.html` lines containing `id="doc-view-0"`.

---

### HD-DOC-WALLET-EARNINGS (`doc-view-3`)
- **Screen Name:** Doctor Wallet & Transparent 20% Fee Debarment
- **User Role:** `doctor`
- **Purpose:** Displays total earnings strictly debarred of the 20% HeloDoc platform fee before showing net income, with itemized consultation breakdowns and withdrawal triggers.
- **Entry Points:** Doctor bottom nav "Wallet" tab.
- **Exit Points:**
  - `openDoctorStatementModal()`: Opens full monthly statement modal.
  - `openConsultationFeeDetailModal(...)`: Opens individual consultation fee breakdown.
  - Withdrawal Action: Dispatches net earnings to verified bKash merchant account.
- **UI Hierarchy:**
  ```
  HD-DOC-WALLET-EARNINGS
  └── Header Bar ("Doctor Earnings & Wallet")
  └── Transparent Earnings Hero Card (#docEarningsHeroCard):
      ├── Row 1: "Total Earnings (Gross)" (#docGrossEarningsVal "৳ 35,562.50")
      ├── Row 2: "HeloDoc Platform Charge (20% Withheld)" (#docWithheldFeeVal "- ৳ 7,112.50")
      ├── Divider Line
      ├── Row 3: "Final Earning (Net Take-Home)" (#docFinalEarningsVal "৳ 28,450.00")
      └── Mandatory Calculation Basis Note (#docEarningsCalcNote):
          "Total calculation is based including the platform charge 20%."
  └── Quick Withdrawal CTA Bar ("Withdraw ৳ 28,450.00 to bKash 01713-445566")
  └── Statement Modal Trigger ("View Full Monthly Statement")
  └── Itemized Consultation Ledger (#docConsultationsLedgerContainer):
      └── Rows for each consultation showing:
          Patient Name, Date, Gross Fee (৳ 800), 20% Withheld (- ৳ 160), Net Take-Home (৳ 640)
  ```
- **Mathematical Enforcement:**
  $$\text{Withheld Fee} = \text{Total Gross} \times 0.20$$
  $$\text{Final Net Take-Home} = \text{Total Gross} - \text{Withheld Fee} = \text{Total Gross} \times 0.80$$
- **Backend Requirements (Rust):** `GET /api/v1/doctor/wallet/summary`, `POST /api/v1/doctor/wallet/withdraw`.
- **Prototype Reference:** `index.html` lines containing `id="doc-view-3"`.

---

### Remaining Doctor Viewports
- **HD-DOC-RX-SCANNER (`doc-view-1`):** Digitizes physical prescriptions via camera capture, binds them to active patient records, and dispatches electronic prescriptions.
- **HD-DOC-SCHEDULE (`doc-view-2`):** Slot management interface allowing doctors to generate 15-minute consultation slots across morning, afternoon, and evening shifts.
- **HD-DOC-CHAT-DESK (`doc-view-4`):** Active 24-hour asynchronous chat desk with filtering by urgency and canned clinical response templates.
- **HD-DOC-QNA-TRIAGE (`doc-view-5`):** Community Q&A triage queue for answering general public health inquiries.

---

## 4. Doctor Web Clinical Workstation (`#doctorWebShell`)

### HD-DOCWEB-WORKSTATION (`#doctorWebShell`)
- **Screen Name:** Desktop Clinical Workstation & Telehealth Console
- **User Role:** `doctor`
- **Purpose:** Full-screen desktop workstation for hospital and clinic environments featuring dual-pane teleconsultation, live EMR record inspection, DGDA-compliant prescription authoring, and revenue KPI widgets.
- **Entry Points:** Workbench shell switcher `'doctor-web'`, web desktop login.
- **UI Hierarchy:**
  ```
  HD-DOCWEB-WORKSTATION
  └── Top Command Bar (#docWebHeader):
      ├── Doctor Credentials (Dr. Sabrina Akter, MBBS, FCPS)
      ├── Daily Revenue Widget:
          "Total Gross: ৳ 10,500 | 20% Fee: - ৳ 2,100 | Net Final: ৳ 8,400"
      └── Duty Toggle Switch
  └── Dual-Pane Workstation Body:
      ├── Left Pane: Live Telehealth Video Feed (#docWebVideoFeed)
      │   ├── 1080p Agora RTC video feed
      │   ├── Audio/Video Controls, Screen Sharing
      │   └── Call End & Prescribe Trigger
      └── Right Pane: Split-Screen Longitudinal EMR
          ├── Tab 1: Patient Clinical History (#docWebPatientHistory)
          │   ├── Past Consultations & Chronic Conditions
          │   └── Attached Multi-Prescription Photos (1 to 5 thumbnails)
          └── Tab 2: Cloud Rx Composer (#docWebRxComposer)
              ├── DGDA Drug Search & Auto-complete
              ├── Dosage, Frequency & Duration Matrix
              ├── Diagnostic Investigation Checklist
              └── Digital Signature & Dispatch Button
  ```
- **Backend Requirements (Rust):** Agora token service, `POST /api/v1/prescriptions/sign-and-dispatch`.
- **Prototype Reference:** `index.html` lines containing `id="doctorWebShell"`.

---

## 5. Central Administration Portal (`#adminPortalShell`)

### HD-ADMIN-COMMAND (`adminPage_command`)
- **Screen Name:** Executive Command Center
- **User Role:** `admin`
- **Purpose:** Real-time platform pulse monitoring live consultations, server performance, and active alerts.
- **Backend Requirements (Rust):** `GET /api/v1/admin/kpis`.

### HD-ADMIN-DOCTORS (`adminPage_doctors`)
- **Screen Name:** Doctor Roster & Physician Dossiers
- **User Role:** `admin`
- **Purpose:** Master registry of all credentialed doctors. Each entry links to `#adminDoctorHistoryModal` containing verified phone numbers, residential addresses, emails, consultation histories, and disciplinary records.
- **Backend Requirements (Rust):** `GET /api/v1/admin/doctors`, `GET /api/v1/admin/doctors/{id}/dossier`.

### HD-ADMIN-PATIENTS (`adminPage_patients`)
- **Screen Name:** Patient Directory & Account Oversight
- **User Role:** `admin`
- **Purpose:** Complete database of registered patients with consultation counts, lifetime value, and dispute flags.
- **Backend Requirements (Rust):** `GET /api/v1/admin/patients`.

### HD-ADMIN-BMDC (`adminPage_bmdc`)
- **Screen Name:** BMDC Medical Credentialing Engine
- **User Role:** `admin`
- **Purpose:** Automated and manual verification of medical council licenses, credential renewal monitoring, and practice suspension controls.
- **Backend Requirements (Rust):** `GET /api/v1/admin/bmdc/verifications`, `POST /api/v1/admin/bmdc/verify`.

### HD-ADMIN-SLOTS (`adminPage_slots`)
- **Screen Name:** Telehealth Capacity & Slot Management
- **User Role:** `admin`
- **Purpose:** Real-time capacity manager balancing consultation volume across specialties.
- **Backend Requirements (Rust):** `GET /api/v1/admin/slots/capacity`.

### HD-ADMIN-COMPLIANCE (`adminPage_compliance`)
- **Screen Name:** Clinical Protocol & Regulatory Oversight
- **User Role:** `admin`
- **Purpose:** Audits consultation durations, prescription completeness, and drug schedule compliance per DGDA/DGHS rules.
- **Backend Requirements (Rust):** `GET /api/v1/admin/compliance/audit-logs`.

### HD-ADMIN-LOGS (`adminPage_logs`)
- **Screen Name:** App Incident Diagnostics & Failure Simulators
- **User Role:** `admin`
- **Purpose:** Subsystem error diagnostics covering bKash/Nagad webhooks, Agora RTC disconnects, and DGDA EMR synchronization errors. Includes the failure simulator `#adminSimulateFailureModal` (dev/staging only).
- **Backend Requirements (Rust):** `GET /api/v1/admin/logs/incidents`, `POST /api/v1/admin/logs/simulate-failure`.

### HD-ADMIN-GRIEVANCES (`adminPage_grievances`)
- **Screen Name:** Grievance Arbitration & Disciplinary Board
- **User Role:** `admin`
- **Purpose:** Adjudicates patient disputes against doctors or system technical failures using auto-collected session telemetry. Provides one-click escrow refunds (`btnAdjudicateRefund`) and Internal Platform Compliance warnings (`btnAdjudicateWarning`).
- **Backend Requirements (Rust):** `GET /api/v1/admin/grievances`, `POST /api/v1/admin/grievances/{id}/adjudicate`.

### HD-ADMIN-FINANCE (`adminPage_finance`)
- **Screen Name:** Omnichannel Payment & Escrow Master Ledger
- **User Role:** `admin`
- **Purpose:** Tracks gross merchandise value (GMV), escrow reserves, 20% platform revenue, and MFS reconciliations with CSV export capabilities.
- **Backend Requirements (Rust):** `GET /api/v1/admin/finance/ledger`, `POST /api/v1/admin/finance/reconcile-webhooks`.

---

## 6. Interactive System Modals

| Modal Identifier | DOM Element ID | User Role | Purpose & Clinical Actions |
|---|---|---|---|
| `HD-MODAL-GRIEVANCE` | `#patientGrievanceModal` | `patient` | Captures patient complaints; binds background Agora RTC/payment telemetry silently without exposing raw diagnostics to the user. |
| `HD-MODAL-DOC-STATEMENT` | `#doctorStatementModal` | `doctor` | Itemized monthly earnings ledger showing gross amounts, 20% platform fees, and net take-home pay. |
| `HD-MODAL-CONSULT-FEE` | `#docConsultationFeeModal`| `doctor` | Displays 3-tier fee calculations for single consultations (Gross -> 20% Fee -> Net). |
| `HD-MODAL-LANGUAGE` | `#languageModal` | `patient` | In-memory language selection between English and Swahili (Kiswahili). |
| `HD-MODAL-RX-VIEWER` | `#labReportModal` | `shared` | Multi-page image viewer for inspecting uploaded prescriptions and lab reports (pages 1 to 5). |
| `HD-MODAL-DOC-DOSSIER` | `#adminDoctorHistoryModal` | `admin` | Displays a physician's full profile: verified phone number, residential address, email, consultation logs, and disciplinary history. |
| `HD-MODAL-ADMIN-GRIEVANCE`| `#adminGrievanceDetailModal`| `admin` | Deep forensic investigation console displaying call durations, Agora RTC SDK packet logs, and dispute adjudication controls. |
| `HD-MODAL-TXN-DETAIL` | `#adminTransactionDetailModal`| `admin` | Audit log inspector for transactions, displaying MFS gateway payloads and escrow states. |
| `HD-MODAL-LOG-DETAIL` | `#adminLogDetailModal` | `admin` | Displays subsystem stack traces and provides retry or refund options. |
| `HD-MODAL-SIM-FAILURE` | `#adminSimulateFailureModal`| `admin` | Diagnostic tool for simulating payment drops, Agora RTC timeouts, and gateway failures. |
| `HD-MODAL-OFFLINE-GUARD` | `#offlineActionGuardModal` | `shared` | Intercepts destructive actions while offline and explains network requirements. |
