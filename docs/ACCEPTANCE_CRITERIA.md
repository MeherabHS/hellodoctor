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

## 3. Slot Scheduling, Booking & Payment Hold

### AC-004: Atomic Slot Reservation & Double-Booking Prevention
```gherkin
GIVEN two patients simultaneously select the same available time slot
WHEN both booking requests arrive at the server concurrently
THEN exactly one patient atomically acquires the slot via SELECT FOR UPDATE SKIP LOCKED
AND the winning patient's slot transitions to LOCKED_IN_PAYMENT
AND the winning patient's appointment is created with status PENDING_PAYMENT
AND the losing patient receives HTTP 409 with error code SLOT_NOT_AVAILABLE
AND upon successful payment webhook, the slot transitions to BOOKED and appointment to CONFIRMED.
```

### AC-005: Payment Holding State
```gherkin
GIVEN a successful consultation booking of ৳ 800
WHEN the payment session is created before redirect
THEN a transaction record exists with payment_status = INITIATED, gross_amount = ৳ 800, platform_fee_amount = ৳ 160 (20%), and net_amount = ৳ 640
AND its opaque payment_session_id is persisted before it is returned to the client
AND when the verified payment webhook succeeds, payment_status transitions to PAYMENT_HELD
AND the doctor's accrued net earnings are NOT incremented until the consultation completes.
```

---

## 4. Live Telehealth, Telemetry & 24h Chat

### AC-006: Premature Call Termination Detection
```gherkin
GIVEN an active Agora RTC teleconsultation session
WHEN the connection disconnects after only 15 seconds (callSeconds < 30)
THEN the session is marked with status PREMATURE_TERMINATION
AND the telemetry record stores call_duration_seconds = 15 and premature_end = true
AND the payment remains in PAYMENT_HELD rather than auto-settling to the doctor.
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
THEN the consultation's forensic telemetry (call duration, Agora RTC connection health, prescription status, payment ID) is silently bound to the dispute payload
AND raw technical diagnostics remain hidden from the patient UI (aria-hidden="true", display: none)
AND the grievance is queued in the Admin Portal docket with status PENDING_REVIEW.
```

### AC-010: Grievance Adjudication: Payment Refund Disbursal
```gherkin
GIVEN an administrator reviewing a grievance where telemetry confirms call_duration = 0m 08s and prescription = Not Issued
WHEN the administrator clicks "Disburse Refund" (#btnAdjudicateRefund)
THEN the grievance status updates to REFUNDED
AND the board remedy updates to "Payment Refund Disbursed (৳800)"
AND the transaction payment_status updates to REFUNDED_TO_PATIENT
AND an MFS refund API call is dispatched to the patient's bKash account.
```

### AC-011: Grievance Adjudication: Internal Platform Compliance Warning
```gherkin
GIVEN an administrator reviewing a physician conduct complaint
WHEN the administrator clicks "Issue Warning" (#btnAdjudicateWarning)
THEN the grievance status updates to WARNED
AND the board remedy updates to "Internal Platform Compliance Warning Logged"
AND a compliance warning is logged in the doctor's platform dossier.
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
WHEN the user attempts a destructive action (Confirm & Pay, Book Slot, Upload Prescription Photo)
THEN the submission is blocked before dispatching network requests
AND the modal #offlineActionGuardModal is displayed
AND previously entered form data is preserved until connectivity resumes.
```

---

## 9. Security, Payments & Session Management

### AC-015: Two-Stage Payment Booking
```gherkin
GIVEN a patient confirming an appointment booking with bKash payment
WHEN the booking request is submitted
THEN the appointment is created with status PENDING_PAYMENT
AND the slot transitions to LOCKED_IN_PAYMENT
AND a payment session is initiated with the MFS provider
AND an INITIATED transaction and its payment_session_id are persisted before the redirect URL is returned
AND the appointment transitions to CONFIRMED only after the payment webhook confirms successful debit.
```

### AC-016: Auth Session Refresh Token Rotation
```gherkin
GIVEN an authenticated user with an expired access token and a valid refresh token
WHEN the client presents the refresh token to POST /api/v1/auth/refresh
THEN a new access token and new refresh token are issued
AND the previous refresh token is invalidated
AND presenting the old refresh token again triggers session family revocation.
```

### AC-017: ABAC Authorization Enforcement
```gherkin
GIVEN a doctor authenticated with valid JWT credentials
WHEN the doctor attempts to access appointment records belonging to a different doctor's patient
THEN the request is denied with 403 Forbidden
AND the unauthorized access attempt is logged to audit_events.
```

### AC-018: File Upload Security
```gherkin
GIVEN a patient uploading a prescription image
WHEN the file is received by the server
THEN the server validates magic bytes match the declared MIME type
AND the image is re-encoded to strip EXIF metadata
AND the file is quarantined until malware scan completes
AND the response returns a document_id, NOT a raw storage URL.
```

### AC-019: Push Notification PHI Protection
```gherkin
GIVEN a patient with an upcoming consultation in 30 minutes
WHEN the server dispatches a push notification reminder
THEN the notification payload contains only a generic message like 'You have an upcoming HelloDoctor consultation'
AND the notification does NOT contain the doctor's name, diagnosis, prescription details, or any PHI.
```

### AC-020: Consultation Completion Without Prescription
```gherkin
GIVEN a doctor in an active consultation where no medication is indicated
WHEN the doctor selects clinical outcome 'No Prescription Required' and ends the session
THEN the consultation transitions to COMPLETED with clinical_outcome = COMPLETED_NO_RX
AND payment_status transitions from PAYMENT_HELD to SETTLED_TO_DOCTOR
AND the patient is NOT blocked from proceeding.
```
