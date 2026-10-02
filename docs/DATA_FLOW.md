# Phase 29 — End-to-End System Data Flow Architecture

> **Document Version:** 1.0.0  
> **Flow Paradigm:** User Action ──> Flutter BLoC ──> Dio Client ──> Rust Axum Router ──> Auth Middleware ──> Validation ──> Domain Service ──> SQLx Repository ──> PostgreSQL ──> Response ──> BLoC State ──> UI Update  
> **Target Audience:** Claude Code Autonomous Implementation Agent

---

## 1. Flow 1: Slot Booking & Payment Hold

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
    participant MFS as Telco MFS Gateway

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
    Repo->>DB: SELECT status FROM doctor_schedule_slots WHERE id = $1 FOR UPDATE SKIP LOCKED
    DB-->>Repo: Slot status == 'AVAILABLE'
    Svc->>Repo: update_slot_status(slot_id, 'LOCKED_IN_PAYMENT')
    Repo->>DB: UPDATE doctor_schedule_slots SET status = 'LOCKED_IN_PAYMENT'
    Svc->>Repo: create_appointment(status='PENDING_PAYMENT')
    Repo->>DB: INSERT INTO appointments ... RETURNING id
    Svc->>DB: COMMIT TRANSACTION
    Svc-->>Axum: Return payment URL
    Axum-->>Dio: 201 Created { payment_url }
    Dio-->>Patient: Navigate to MFS Gateway
    Patient->>MFS: Complete Payment
    MFS->>Axum: POST Webhook (Payment Success)
    Axum->>DB: BEGIN TRANSACTION
    Axum->>DB: UPDATE appointments SET status='CONFIRMED'
    Axum->>DB: UPDATE doctor_schedule_slots SET status = 'BOOKED'
    Axum->>DB: INSERT INTO transactions (..., payment_status = 'PAYMENT_HELD')
    Axum->>DB: COMMIT TRANSACTION
    Axum-->>MFS: 200 OK
```

---

## 2. Flow 2: Multi-Prescription Intake Upload (1–5 Photos)

```mermaid
sequenceDiagram
    autonumber
    actor Patient as Patient
    participant Bloc as PrescriptionIntakeBloc
    participant Axum as Axum Multipart Handler
    participant SecPipe as Security Pipeline
    participant AV as Malware Scanner
    participant S3 as S3/MinIO Object Storage
    participant DB as PostgreSQL

    Patient->>Bloc: Select 3 Prescription Photos
    Bloc->>Bloc: Check current count (0) + new (3) <= 5 -> Accepted
    Bloc->>Axum: POST /api/v1/appointments/{id}/prescriptions (multipart)
    Axum->>Axum: Validate file count (3 <= 5) and size limit (<= 10MB each)
    loop For each file
        Axum->>SecPipe: magic-byte validation (verify MIME)
        SecPipe->>SecPipe: temporary quarantine storage
        SecPipe->>AV: malware scan
        AV-->>SecPipe: clean
        alt IF image
            SecPipe->>SecPipe: decode, strip EXIF, re-encode to safe format (JPEG/PNG)
        else IF PDF
            SecPipe->>SecPipe: structural validation, sanitize (strip JS/macros)
        end
        SecPipe->>SecPipe: calculate SHA-256 content_hash
        SecPipe->>S3: move to private final object storage with server-generated object_key
        S3-->>SecPipe: Return storage success
        Axum->>DB: INSERT INTO prescription_intake_documents (object_bucket, object_key, content_hash, mime_type, file_size_bytes)
    end
    Axum-->>Bloc: 201 Created { uploaded_count: 3, documents: [{document_id, page_number}] }
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
    participant CompSvc as Consultation Completion Service
    participant SettlementSvc as SettlementService
    participant DB as PostgreSQL

    Patient->>Axum: POST /api/v1/consultations/{appointment_id}/rtc-token
    Doctor->>Axum: POST /api/v1/consultations/{appointment_id}/rtc-token
    Axum-->>Patient: 200 OK (Agora Token, Channel: rtc_7baf3072_96bd_4fae_a735_e8d2c1f94b3a, UID: 94812)
    Axum-->>Doctor: 200 OK (Agora Token, Channel: rtc_7baf3072_96bd_4fae_a735_e8d2c1f94b3a, UID: 10421)
    Patient->>Agora: joinChannel(token, "rtc_7baf3072_96bd_4fae_a735_e8d2c1f94b3a", 94812)
    Doctor->>Agora: joinChannel(token, "rtc_7baf3072_96bd_4fae_a735_e8d2c1f94b3a", 10421)
    Note over Patient,Doctor,Agora: Adaptive 720p/1080p Video via Agora SD-RTN
    Note over Patient: Agora RtcStats tracks callSeconds, packetLoss, RTT
    Doctor->>Agora: leaveChannel()
    Note over Doctor: Writes prescription on paper & takes photo
    Doctor->>Axum: POST /api/v1/consultations/{appointment_id}/prescription (multipart photo)
    Axum->>SecPipe: magic-byte validation, malware scan, EXIF stripping
    SecPipe->>S3: move to private final object storage with rx_image_object_key
    SecPipe->>SecPipe: calculate SHA-256 integrity_verification_hash
    Axum->>DB: INSERT INTO prescriptions (rx_image_object_key, integrity_verification_hash...)
    Axum-->>Doctor: 201 Created (Photo delivered to Patient Health Vault)
    
    Patient->>TelemetrySvc: POST /api/v1/consultations/{appointment_id}/telemetry (from onRtcStats)
    Note over TelemetrySvc: Payload: call_duration = 642s, packet_loss = 0.4%, premature_end = false
    TelemetrySvc->>DB: record technical evidence only (INSERT INTO consultation_telemetry)
    
    Note over CompSvc: Triggered on Session End / Consultation Completion
    CompSvc->>CompSvc: verify appointment state = IN_CONSULTATION
    CompSvc->>CompSvc: verify server-side timestamps (minimum session duration)
    CompSvc->>CompSvc: verify clinical_outcome is set
    CompSvc->>CompSvc: verify no blocking grievance/dispute
    CompSvc->>SettlementSvc: trigger_settlement()
    SettlementSvc->>DB: UPDATE transactions SET payment_status = 'SETTLED_TO_DOCTOR'
    Note over SettlementSvc: SETTLED_TO_DOCTOR means earmarked for doctor, pending monthly batch payout
    SettlementSvc->>DB: INSERT INTO payment_events (event_type = 'SETTLED')
    DB-->>Patient: 200 OK -> Navigate to View-11 Post-Consultation
```

---

## 4. Flow 4: Clinical Grievance Adjudication & Payment Refund

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
    SettlementSvc->>DB: UPDATE transactions SET payment_status = 'REFUNDED_TO_PATIENT'
    SettlementSvc->>DB: UPDATE grievance_reports SET status = 'REFUNDED', board_remedy = 'Payment Refund Disbursed (৳800)'
    SettlementSvc-->>Admin: 200 OK (Dispute Closed, Refund Disbursed)
```

---

## 5. Flow 5: Auth Session Lifecycle

```mermaid
sequenceDiagram
    autonumber
    actor Client as Patient Mobile App
    participant Axum as Rust Backend API
    participant AuthSvc as AuthService
    participant DB as PostgreSQL

    Client->>Axum: POST /api/v1/auth/login
    Axum->>AuthSvc: verify_credentials()
    AuthSvc->>AuthSvc: generate opaque 64-byte random refresh_token
    AuthSvc->>AuthSvc: compute refresh_token_hash = SHA-256(refresh_token)
    AuthSvc->>AuthSvc: generate token_family_id = new UUID
    AuthSvc->>DB: INSERT INTO auth_sessions (user_id, token_family_id, refresh_token_hash, device_fingerprint, ip_hash, user_agent, expires_at)
    AuthSvc-->>Axum: (access_token (Ed25519), refresh_token)
    Axum-->>Client: 200 OK
    Note over Client: Access Token expires in 15 mins
    Client->>Axum: POST /api/v1/auth/refresh (refresh_token)
    Axum->>AuthSvc: rotate_refresh_token(refresh_token)
    
    alt IF presented token matches a rotated_at IS NOT NULL session (Reuse Detection)
        AuthSvc->>DB: REVOKE ALL sessions with same token_family_id
        AuthSvc-->>Axum: Force re-authentication
        Axum-->>Client: 401 Unauthorized
    else Valid Token
        AuthSvc->>DB: UPDATE auth_sessions SET rotated_at = now() WHERE id = old_session_id
        AuthSvc->>AuthSvc: generate new_refresh_token & compute hash
        AuthSvc->>DB: INSERT INTO auth_sessions (same token_family_id, new refresh_token_hash...)
        AuthSvc-->>Axum: (new_access_token, new_refresh_token)
        Axum-->>Client: 200 OK
    end
```
