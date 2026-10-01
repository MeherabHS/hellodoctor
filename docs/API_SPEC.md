# Phase 7 — REST & WebSocket API Specification

> **Document Version:** 1.0.0  
> **Backend Framework:** Rust `axum` 0.7+  
> **Serialization Engine:** `serde_json`  
> **Target Audience:** Claude Code Autonomous Implementation Agent

---

## 1. Global API Standards

### 1.1 Base URL & Content Negotiation
- **Base Route:** `/api/v1`
- **Request Headers:**
  - `Content-Type: application/json` (or `multipart/form-data` for file uploads)
  - `Authorization: Bearer <JWT_ACCESS_TOKEN>` (for authenticated routes)
  - `X-Request-Id: <UUIDv4>` (Traceability header auto-injected by proxy or client)
  - `Idempotency-Key: <UUIDv4>` (Required for financial and booking mutations)

### 1.2 Standard Success Response Envelope
```json
{
  "success": true,
  "data": {},
  "meta": {
    "timestamp": "2026-10-01T05:30:00Z",
    "request_id": "c1f7b8a2-..."
  }
}
```

### 1.3 Standard Error Response Envelope
```json
{
  "success": false,
  "error": {
    "code": "VALIDATION_FAILED",
    "message": "Invalid consultation slot or overlapping schedule.",
    "details": [
      { "field": "slot_id", "issue": "Slot is already booked." }
    ]
  },
  "meta": {
    "timestamp": "2026-10-01T05:30:00Z",
    "request_id": "c1f7b8a2-..."
  }
}
```

---

## 2. Authentication & Profile Endpoints

### `POST /api/v1/auth/register-otp`
- **Purpose:** Initiates registration by sending an SMS verification OTP.
- **Auth:** Public.
- **Request Body:**
  ```json
  { "phone_number": "+8801712345678", "role": "PATIENT" }
  ```
- **Response:** `200 OK`
  ```json
  { "success": true, "data": { "session_token": "otp_sess_94812", "expires_in": 120 } }
  ```

### `POST /api/v1/auth/verify-otp-and-login`
- **Purpose:** Verifies OTP and returns JWT access and refresh tokens.
- **Auth:** Public.
- **Request Body:**
  ```json
  { "session_token": "otp_sess_94812", "otp_code": "584920" }
  ```
- **Response:** `200 OK`
  ```json
  {
    "success": true,
    "data": {
      "access_token": "eyJhbGciOi...",
      "refresh_token": "eyJhbGciOi...",
      "user": {
        "id": "u-9481",
        "phone_number": "+8801712345678",
        "role": "PATIENT",
        "preferred_language": "en"
      }
    }
  }
  ```

---

## 3. Doctor Catalog & Discovery Endpoints

### `GET /api/v1/doctors`
- **Purpose:** Filtered directory of BMDC-verified specialist doctors.
- **Auth:** Public / Optional Patient Token.
- **Query Parameters:**
  - `modality`: `all` | `video` | `chat` (Default: `all`)
  - `specialty`: String (e.g., `Internal Medicine`, `Cardiology`)
  - `search`: String (Doctor name or hospital)
  - `page`: Integer (Default: 1)
  - `limit`: Integer (Default: 20)
- **Response:** `200 OK`
  ```json
  {
    "success": true,
    "data": [
      {
        "id": "doc-sabrina",
        "name": "Dr. Sabrina Akter",
        "bmdc": "BMDC #45821",
        "specialty": "Internal Medicine",
        "experience_years": 12,
        "current_hospital": "Dhaka Medical College Hospital",
        "qualifications": ["MBBS", "FCPS", "MD"],
        "fee_video": 800.00,
        "fee_chat": 500.00,
        "is_on_duty": true
      }
    ],
    "meta": { "total_count": 48, "page": 1, "limit": 20 }
  }
  ```

---

## 4. Slot Scheduling, Booking & Multi-Rx Upload

### `GET /api/v1/doctors/{id}/slots`
- **Purpose:** Retrieves available 15-minute consultation slots for a specific date.
- **Query Parameters:** `date` (YYYY-MM-DD, e.g., `2026-10-01`).
- **Response:** `200 OK`
  ```json
  {
    "success": true,
    "data": {
      "date": "2026-10-01",
      "slots": [
        { "id": "slot-01", "start_time": "2026-10-01T10:00:00Z", "end_time": "2026-10-01T10:15:00Z", "status": "AVAILABLE" },
        { "id": "slot-02", "start_time": "2026-10-01T10:15:00Z", "end_time": "2026-10-01T10:30:00Z", "status": "AVAILABLE" }
      ]
    }
  }
  ```

### `POST /api/v1/appointments/book`
- **Purpose:** Atomically reserves slot and initiates MFS escrow authorization.
- **Auth:** `PATIENT`.
- **Headers:** `Idempotency-Key: <UUID>`
- **Request Body:**
  ```json
  {
    "doctor_id": "doc-sabrina",
    "slot_id": "slot-01",
    "modality": "VIDEO",
    "gateway": "BKASH",
    "chief_complaint": "High fever, joint pain, suspected dengue"
  }
  ```
- **Response:** `201 Created`
  ```json
  {
    "success": true,
    "data": {
      "appointment_id": "apt-94812",
      "appointment_number": "APT-20261001-9481",
      "status": "CONFIRMED",
      "escrow_transaction_id": "TXN-BK-94812",
      "amount": 800.00
    }
  }
  ```

### `POST /api/v1/appointments/{id}/prescriptions/upload`
- **Purpose:** Uploads 1 to 5 physical prescription or lab report images for pre-consultation intake.
- **Auth:** `PATIENT`.
- **Content-Type:** `multipart/form-data`
- **Form Fields:**
  - `files[]`: Up to 5 binary image or PDF attachments (max 10 MB per file).
- **Validation:** If current uploaded count + new files > 5, returns `422 Unprocessable Entity` with code `MAX_PRESCRIPTION_IMAGES_EXCEEDED`.
- **Response:** `201 Created`
  ```json
  {
    "success": true,
    "data": {
      "appointment_id": "apt-94812",
      "uploaded_count": 3,
      "documents": [
        { "id": "doc-01", "page_number": 1, "url": "https://storage.../rx1.jpg", "size_bytes": 1245000 },
        { "id": "doc-02", "page_number": 2, "url": "https://storage.../rx2.jpg", "size_bytes": 1450000 },
        { "id": "doc-03", "page_number": 3, "url": "https://storage.../rx3.jpg", "size_bytes": 980000 }
      ]
    }
  }
  ```

---

## 5. Telehealth Video (Agora RTC) & Telemetry Capture

### `POST /api/v1/telehealth/agora-token`
- **Purpose:** Mints an authorized, short-lived Agora Dynamic RTC Token for entering a video consultation room.
- **Auth:** `PATIENT` or `DOCTOR` (Caller must be participant in the appointment).
- **Request Body:**
  ```json
  {
    "appointment_id": "apt-94812"
  }
  ```
- **Response:** `200 OK`
  ```json
  {
    "success": true,
    "data": {
      "channel_name": "apt-94812",
      "token": "007eJxTYGCoM2y+5tW2Z1n380vP4...",
      "uid": 94812,
      "app_id": "a1b2c3d4e5f6...",
      "expires_in": 3600
    }
  }
  ```

### `POST /api/v1/consultations/{appointment_id}/telemetry`
- **Purpose:** Submits auto-captured technical telemetry upon consultation termination (derived from Agora's `onRtcStats`).
- **Auth:** System / Doctor Client / Patient Client.
- **Request Body:**
  ```json
  {
    "call_duration_seconds": 15,
    "ice_connection_state": "COMPLETED",
    "packet_loss_percent": 0.0,
    "round_trip_time_ms": 48,
    "premature_end": true,
    "prescription_issued": false
  }
  ```
- **Response:** `200 OK`
  ```json
  { "success": true, "data": { "telemetry_id": "telem-94812", "recorded": true } }
  ```

---

## 6. Doctor Wallet & Transparent 20% Fee Debarment

### `GET /api/v1/doctor/wallet/summary`
- **Purpose:** Retrieves doctor's earnings strictly formatted with 20% fee debarment.
- **Auth:** `DOCTOR`.
- **Response:** `200 OK`
  ```json
  {
    "success": true,
    "data": {
      "doctor_name": "Dr. Sabrina Akter",
      "bmdc": "BMDC #45821",
      "total_gross": 35562.50,
      "platform_charge_percent": 20.0,
      "withheld_fee": 7112.50,
      "final_net": 28450.00,
      "basis_note": "Total calculation is based including the platform charge 20%.",
      "recent_transactions": [
        {
          "consult_id": "CONS-9481",
          "patient_name": "Rafiq Ahmed",
          "service": "Video Consultation (10 min)",
          "gross": 800.00,
          "withheld_20_percent": 160.00,
          "net": 640.00,
          "trx_id": "BK-MER-7718290",
          "status": "Settled"
        }
      ]
    }
  }
  ```

---

## 7. Clinical Grievance Redressal & Adjudication

### `POST /api/v1/grievances`
- **Purpose:** Lodges a formal clinical or technical dispute. Auto-binds session telemetry.
- **Auth:** `PATIENT`.
- **Request Body:**
  ```json
  {
    "consultation_id": "CONS-9481",
    "target": "DOCTOR",
    "category": "Rushed Consultation / Ended Abruptly",
    "claim_summary": "Doctor stayed only 2 minutes and abruptly disconnected call without prescribing medicine."
  }
  ```
- **Response:** `201 Created`
  ```json
  {
    "success": true,
    "data": {
      "grievance_id": "GRV-20261001-73",
      "status": "PENDING_REVIEW",
      "board_remedy": "Under Governance Review",
      "message": "Report submitted. Medical Administration is reviewing your claim."
    }
  }
  ```

### `POST /api/v1/admin/grievances/{id}/refund`
- **Purpose:** Admin Board disburses escrow refund to patient MFS account.
- **Auth:** `ADMIN`.
- **Response:** `200 OK`
  ```json
  {
    "success": true,
    "data": {
      "grievance_id": "GRV-20261001-73",
      "status": "REFUNDED",
      "board_remedy": "Escrow Refund Disbursed (৳800)",
      "refund_trx_id": "BK-REF-876862"
    }
  }
  ```

### `POST /api/v1/admin/grievances/{id}/warn`
- **Purpose:** Admin Board logs a formal BMDC disciplinary warning into doctor's dossier.
- **Auth:** `ADMIN`.
- **Response:** `200 OK`
  ```json
  {
    "success": true,
    "data": {
      "grievance_id": "GRV-20261001-73",
      "status": "WARNED",
      "board_remedy": "BMDC Disciplinary Warning Logged",
      "doctor_bmdc": "BMDC #45821"
    }
  }
  ```

---

## 8. Admin Oversight & Dossier Endpoints

### `GET /api/v1/admin/doctors/{id}/dossier`
- **Purpose:** Comprehensive physician dossier including verified phone and residential address.
- **Auth:** `ADMIN`.
- **Response:** `200 OK`
  ```json
  {
    "success": true,
    "data": {
      "id": "doc-anika",
      "name": "Dr. Anika Rahman",
      "bmdc": "BMDC #52891",
      "phone": "+880 1711-884920",
      "residence": "Dhanmondi, Dhaka (House 42, Road 7A)",
      "email": "dr.anika.dmch@helodoc.com",
      "hospital": "Dhaka Medical College Hospital",
      "lifetime_consultations": 342,
      "disciplinary_warnings": []
    }
  }
  ```
