# Phase 18 — Deterministic Acceptance Criteria Specification

> **Document Version:** 1.0.0  
> **Format Standard:** BDD (Given-When-Then)  
> **Target Audience:** Claude Code Autonomous Implementation Agent

---

## 1. Specialist Discovery & Directory

### AC-001: Star-Rating-Free Specialist Directory
```gherkin
GIVEN an authenticated or guest patient on HD-PATIENT-FIND-SPECIALIST (view-3)
WHEN the patient views the doctor list
THEN each doctor card displays the physician's verified name, BMDC number, clinical experience, hospital affiliation, and consultation fees
AND zero star rating icons (⭐/★) or vanity review counts are displayed
AND doctor cards can be filtered by modality ('all', 'video', 'chat') and specialty.
```

---

## 2. Multi-Prescription Upload (Max 5 Images)

### AC-002: Multi-Prescription Intake Cap Enforcement
```gherkin
GIVEN a patient on HD-PATIENT-BOOK-SLOT (view-4) with 2 prescription photos already attached
WHEN the patient selects 4 additional photos from their device
THEN the first 3 photos are accepted (bringing total to exactly 5 photos)
AND the 4th photo is omitted
AND a warning notification toast is displayed: "Maximum 5 prescription photos allowed. Extra photos were omitted."
AND the thumbnail gallery displays 5 items with page indicators "Page 1 of 5" through "Page 5 of 5".
```

### AC-003: Multi-Page Prescription Viewer
```gherkin
GIVEN a patient or doctor viewing attached prescriptions in the Waiting Room (view-10) or Clinical Workstation
WHEN the user taps any prescription thumbnail
THEN the full-screen modal #labReportModal opens
AND the active document is rendered in high resolution
AND the pagination bar displays "Page X of Y" with functional Previous and Next buttons.
```

---

## 3. Slot Scheduling, Booking & Escrow Hold

### AC-004: Atomic Slot Reservation & Double-Booking Prevention
```gherkin
GIVEN two patients simultaneously attempting to book the identical 10:00 AM slot for Dr. Sabrina Akter
WHEN Patient A and Patient B both submit confirmation
THEN the database transaction locks the slot for Patient A and transitions it to BOOKED
AND Patient A receives a 201 Created confirmation with appointment ID
AND Patient B's transaction fails with 409 Conflict (SLOT_NOT_AVAILABLE)
AND Patient B is prompted with: "This appointment slot was just booked by another patient. Please select an alternative slot."
```

### AC-005: Escrow Holding State
```gherkin
GIVEN a successful consultation booking of ৳ 800
WHEN the payment gateway verifies the transaction
THEN a transaction record is created with gross_amount = ৳ 800, platform_fee = ৳ 160 (20%), net_amount = ৳ 640
AND the transaction escrow_status is set to ESCROW_HELD
AND the doctor's withdrawable wallet balance is NOT incremented until the consultation completes.
```

---

## 4. Live Telehealth, Telemetry & 24h Chat

### AC-006: Premature Call Termination Detection
```gherkin
GIVEN an active WebRTC teleconsultation session
WHEN the connection disconnects after only 15 seconds (callSeconds < 30)
THEN the session is marked with status PREMATURE_TERMINATION
AND the telemetry record stores call_duration_seconds = 15 and premature_end = true
AND the escrow payment remains locked in ESCROW_HELD rather than auto-settling to the doctor.
```

### AC-007: 24-Hour Clinical Chat Auto-Lock
```gherkin
GIVEN a completed video consultation session
WHEN 24 hours have elapsed since consultation termination
THEN the chat conversation status transitions to is_locked = true
AND the chat input box in view-6 / doc-view-4 is disabled with placeholder "Consultation chat concluded"
AND subsequent message submissions return 403 Forbidden (CHAT_SESSION_EXPIRED).
```

---

## 5. Doctor Earnings & Transparent 20% Fee Debarment

### AC-008: 3-Tier Doctor Earnings Display
```gherkin
GIVEN a physician opening their Wallet & Earnings screen (doc-view-3)
WHEN the earnings card renders
THEN Row 1 displays Total Earnings (Gross) = ৳ 35,562.50
AND Row 2 displays HeloDoc Platform Charge (20% Withheld) = - ৳ 7,112.50
AND Row 3 displays Final Earning (Net Take-Home) = ৳ 28,450.00
AND the card explicitly displays the mandatory note: "Total calculation is based including the platform charge 20%."
AND every consultation in the itemized ledger reflects this 3-tier gross, 20% deduction, and net breakdown.
```

---

## 6. Clinical Grievance Redressal & Adjudication

### AC-009: Background Telemetry Binding in Grievance Submission
```gherkin
GIVEN a patient filing a dispute against a doctor on #patientGrievanceModal
WHEN the patient submits their statement
THEN the consultation's forensic telemetry (call duration, WebRTC ICE health, prescription status, payment ID) is silently bound to the dispute payload
AND raw technical diagnostics remain hidden from the patient UI (aria-hidden="true", display: none)
AND the grievance is queued in the Admin Portal docket with status PENDING_REVIEW.
```

### AC-010: Grievance Adjudication: Escrow Refund Disbursal
```gherkin
GIVEN an administrator reviewing a grievance where telemetry confirms call_duration = 0m 08s and prescription = Not Issued
WHEN the administrator clicks "Disburse Refund" (#btnAdjudicateRefund)
THEN the grievance status updates to REFUNDED
AND the board remedy updates to "Escrow Refund Disbursed (৳800)"
AND the escrow transaction status updates to REFUNDED_TO_PATIENT
AND an MFS refund API call is dispatched to the patient's bKash account.
```

### AC-011: Grievance Adjudication: BMDC Disciplinary Warning
```gherkin
GIVEN an administrator reviewing a physician conduct complaint
WHEN the administrator clicks "Issue Warning" (#btnAdjudicateWarning)
THEN the grievance status updates to WARNED
AND the board remedy updates to "BMDC Disciplinary Warning Logged"
AND an official reprimand record is permanently logged in the doctor's BMDC dossier.
```

---

## 7. Emergency Contact & Localization

### AC-012: Emergency Contact Placement & Dialing
```gherkin
GIVEN a patient on the Home screen (view-0)
WHEN inspecting the screen layout
THEN the Emergency Contact banner (#patientEmergencyHelplineStrip) is positioned directly above the Upcoming Consultation section
AND the banner is strictly labeled "Emergency Contact" without any "24/7" prefix
AND tapping "Call 16263" immediately launches the system dialer with tel:16263.
```

### AC-013: Bilingual Language Switcher (English & Swahili)
```gherkin
GIVEN a patient on Account settings (view-7)
WHEN the patient opens #languageModal and selects "Swahili (Kiswahili)"
THEN the active badge updates to "Swahili"
AND the application language preference is saved to local storage
AND a confirmation toast is displayed: "Language Updated: Application language switched to Swahili (Kiswahili)."
```

---

## 8. Offline Resilience

### AC-014: Offline Action Interception
```gherkin
GIVEN a patient or doctor without an active internet connection (offline)
WHEN the user attempts a destructive action (Confirm & Pay, Book Slot, Sign Prescription)
THEN the submission is blocked before dispatching network requests
AND the modal #offlineActionGuardModal is displayed
AND previously entered form data is preserved until connectivity resumes.
```
