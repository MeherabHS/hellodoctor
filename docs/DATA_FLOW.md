# Phase 29 — End-to-End System Data Flow Architecture

> **Document Version:** 1.0.0  
> **Flow Paradigm:** User Action ──> Flutter BLoC ──> Dio Client ──> Rust Axum Router ──> Auth Middleware ──> Validation ──> Domain Service ──> SQLx Repository ──> PostgreSQL ──> Response ──> BLoC State ──> UI Update  
> **Target Audience:** Claude Code Autonomous Implementation Agent

---

## 1. Flow 1: Slot Booking & Escrow Hold

```mermaid
sequenceDiagram
    autonumber
    actor Patient as Patient (Flutter App)
    participant Bloc as AppointmentBookingBloc
    participant Dio as Dio HTTP Client
    participant Axum as Axum Router (/api/v1/appointments/book)
    participant Auth as Auth Middleware (JWT Verifier)
    participant Val as Validator (Request DTO)
    participant Svc as AppointmentService
    participant Repo as AppointmentRepository
    participant DB as PostgreSQL (ACID Transaction)

    Patient->>Bloc: Tap "Confirm & Pay ৳ 800"
    Bloc->>Bloc: Emit BookingSubmittingState
    Bloc->>Dio: POST /appointments/book (with Idempotency-Key)
    Dio->>Axum: HTTPS POST /api/v1/appointments/book
    Axum->>Auth: Validate Bearer Token
    Auth-->>Axum: Token Valid (Patient Role, user_id)
    Axum->>Val: Validate slot_id, doctor_id, gateway
    Val-->>Axum: DTO Validated
    Axum->>Svc: book_appointment(patient_id, doctor_id, slot_id, gateway)
    Svc->>DB: BEGIN TRANSACTION
    Svc->>Repo: lock_slot_for_update(slot_id)
    Repo->>DB: SELECT status FROM doctor_schedule_slots WHERE id = $1 FOR UPDATE
    DB-->>Repo: Slot status == 'AVAILABLE'
    Svc->>Repo: update_slot_status(slot_id, 'BOOKED')
    Repo->>DB: UPDATE doctor_schedule_slots SET status = 'BOOKED'
    Svc->>Repo: create_appointment(...)
    Repo->>DB: INSERT INTO appointments ... RETURNING id
    Svc->>Repo: create_escrow_transaction(fee = ৳ 800, platform_fee = ৳ 160, net = ৳ 640)
    Repo->>DB: INSERT INTO transactions (..., escrow_status = 'ESCROW_HELD')
    Svc->>DB: COMMIT TRANSACTION
    Svc-->>Axum: Return AppointmentBookingResult
    Axum-->>Dio: 201 Created { appointment_id, escrow_status: "ESCROW_HELD" }
    Dio-->>Bloc: OnSuccess(bookingResult)
    Bloc->>Bloc: Emit BookingSuccessState
    Bloc->>Patient: Navigate to Waiting Room (view-10)
```

---

## 2. Flow 2: Multi-Prescription Intake Upload (1–5 Photos)

```mermaid
sequenceDiagram
    autonumber
    actor Patient as Patient
    participant Bloc as PrescriptionIntakeBloc
    participant S3 as S3/MinIO Object Storage
    participant Axum as Axum Multipart Handler
    participant DB as PostgreSQL

    Patient->>Bloc: Select 3 Prescription Photos
    Bloc->>Bloc: Check current count (0) + new (3) <= 5 -> Accepted
    Bloc->>Axum: POST /appointments/{id}/prescriptions/upload (multipart)
    Axum->>Axum: Validate file count (3 <= 5) and sizes (<= 10MB each)
    loop For each file
        Axum->>S3: Stream bytes to S3 bucket (enc-aes256)
        S3-->>Axum: Return storage_key & eTag
        Axum->>DB: INSERT INTO prescription_intake_documents (page_number, file_url, size_bytes)
    end
    Axum-->>Bloc: 201 Created { uploaded_count: 3, documents: [...] }
    Bloc->>Bloc: Emit PrescriptionsUploadedState
    Bloc->>Patient: Render Thumbnail Gallery & Waiting Room Strip
```

---

## 3. Flow 3: Telehealth Session & Telemetry Capture (Agora RTC)

```mermaid
sequenceDiagram
    autonumber
    actor Doctor as Doctor Workstation
    actor Patient as Patient Mobile App
    participant Axum as Rust Backend API
    participant Agora as Agora SD-RTN Global Network
    participant TelemetrySvc as TelemetryService
    participant DB as PostgreSQL

    Patient->>Axum: POST /api/v1/telehealth/agora-token (appointment_id)
    Doctor->>Axum: POST /api/v1/telehealth/agora-token (appointment_id)
    Axum-->>Patient: 200 OK (Agora Token, Channel: apt-94812, UID: 94812)
    Axum-->>Doctor: 200 OK (Agora Token, Channel: apt-94812, UID: 10421)
    Patient->>Agora: joinChannel(token, "apt-94812", 94812)
    Doctor->>Agora: joinChannel(token, "apt-94812", 10421)
    Note over Patient,Doctor,Agora: Adaptive 720p/1080p Video via Agora SD-RTN
    Note over Patient: Agora RtcStats tracks callSeconds, packetLoss, RTT
    Doctor->>Agora: leaveChannel() & Sign Prescription
    Patient->>TelemetrySvc: POST /consultations/{id}/telemetry (from onRtcStats)
    Note over TelemetrySvc: Payload: call_duration = 642s, packet_loss = 0.4%, premature_end = false
    TelemetrySvc->>DB: INSERT INTO consultation_telemetry (...)
    TelemetrySvc->>DB: UPDATE transactions SET escrow_status = 'SETTLED_TO_DOCTOR'
    DB-->>Patient: 200 OK -> Navigate to View-11 Post-Consultation
```

---

## 4. Flow 4: Clinical Grievance Adjudication & Escrow Refund

```mermaid
sequenceDiagram
    autonumber
    actor Patient as Patient
    actor Admin as Medical Governance Admin
    participant GrievanceSvc as GrievanceService
    participant SettlementSvc as SettlementService
    participant MFS as Telco MFS Gateway (bKash)
    participant DB as PostgreSQL

    Patient->>GrievanceSvc: POST /api/v1/grievances (Target: DOCTOR, Category: Rushed Call)
    GrievanceSvc->>DB: INSERT INTO grievance_reports (status = 'PENDING_REVIEW')
    Note over GrievanceSvc: Auto-binds session telemetry (duration = 0m 08s)
    Admin->>GrievanceSvc: GET /api/v1/admin/grievances/{id}
    GrievanceSvc-->>Admin: Render Claim + Telemetry Evidence (0m 08s < 30s threshold)
    Admin->>SettlementSvc: POST /api/v1/admin/grievances/{id}/refund
    SettlementSvc->>MFS: Dispatch Refund API (৳ 800 to Patient Wallet)
    MFS-->>SettlementSvc: Refund Success (TxID: BK-REF-876862)
    SettlementSvc->>DB: UPDATE transactions SET escrow_status = 'REFUNDED_TO_PATIENT'
    SettlementSvc->>DB: UPDATE grievance_reports SET status = 'REFUNDED', board_remedy = 'Escrow Refund Disbursed (৳800)'
    SettlementSvc-->>Admin: 200 OK (Dispute Closed, Refund Disbursed)
```
