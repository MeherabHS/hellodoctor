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

### 1.2 Authorization & ABAC Note
All authenticated endpoints employ Attribute-Based Access Control (ABAC) in addition to Role-Based Access Control (RBAC). Beyond checking the user's role, the system verifies:
- Ownership of the resource (e.g., patient accessing their own records)
- Active doctor-patient relationship
- Context of an ongoing or completed appointment

### 1.3 Standard Success Response Envelope
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

### 1.4 Standard Error Response Envelope
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
- **Purpose:** Initiates registration by sending an SMS verification OTP for patients.
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
- **Purpose:** Verifies OTP and returns JWT access and opaque refresh tokens.
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
      "refresh_token": "a8f4c2e1b9d7...",
      "user": {
        "id": "u-9481",
        "phone_number": "+8801712345678",
        "role": "PATIENT",
        "preferred_language": "en"
      }
    }
  }
  ```

### `POST /api/v1/auth/doctor/login`
- **Purpose:** Initiates doctor login using Medical license credentials + password (Step 1 of MFA). Note: Keep UI labels as "BMDC Registration Number" for Bangladesh deployment. The /bmdc/ path is kept as a V1 Bangladesh-specific path.
- **Auth:** Public.
- **Request Body:**
  ```json
  { "license_number": "BMDC #45821", "password": "SecurePassword123!" }
  ```
- **Response:** `200 OK`
  ```json
  { "success": true, "data": { "session_token": "mfa_sess_123", "requires_otp": true } }
  ```

### `POST /api/v1/auth/doctor/verify-otp`
- **Purpose:** Verifies the second factor OTP for doctor login (Step 2 of MFA).
- **Auth:** Public.
- **Request Body:**
  ```json
  { "session_token": "mfa_sess_123", "otp_code": "837194" }
  ```
- **Response:** `200 OK`
  ```json
  {
    "success": true,
    "data": {
      "access_token": "eyJhbGciOi...",
      "refresh_token": "a8f4c2e1b9d7...",
      "user": { "id": "doc-sabrina", "role": "DOCTOR" }
    }
  }
  ```

### `POST /api/v1/auth/admin/login`
- **Purpose:** Administrator login using Email, Password, and TOTP in a single request.
- **Auth:** Public.
- **Request Body:**
  ```json
  { "email": "admin@helodoc.com", "password": "SecurePassword123!", "totp_code": "123456" }
  ```
- **Response:** `200 OK`
  ```json
  {
    "success": true,
    "data": {
      "access_token": "eyJhbGciOi...",
      "refresh_token": "a8f4c2e1b9d7...",
      "user": { "id": "adm-1", "role": "PLATFORM_ADMIN" }
    }
  }
  ```

### `POST /api/v1/auth/refresh`
- **Purpose:** Exchanges a valid opaque refresh token for a new access/refresh token pair. Employs token rotation and reuse detection.
- **Auth:** Public.
- **Request Body:**
  ```json
  { "refresh_token": "a8f4c2e1b9d7..." }
  ```
- **Response:** `200 OK`

### `POST /api/v1/auth/logout`
- **Purpose:** Revokes the current session's refresh token.
- **Auth:** Authenticated.
- **Request Body:**
  ```json
  { "refresh_token": "a8f4c2e1b9d7..." }
  ```
- **Response:** `200 OK`

### `POST /api/v1/auth/logout-all`
- **Purpose:** Revokes all active sessions for the user.
- **Auth:** Authenticated.
- **Response:** `200 OK`

---

## 3. Doctor Catalog & Discovery Endpoints

### `GET /api/v1/doctors`
- **Purpose:** Filtered directory of Medical license-verified specialist doctors.
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
        "license_number": "BMDC #45821",
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

### `POST /api/v1/appointments/book`
- **Purpose:** Atomically reserves a slot and initiates payment authorization. The slot transitions to `LOCKED_IN_PAYMENT`.
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
      "status": "PENDING_PAYMENT",
      "payment_session_id": "sess_bkash_9123",
      "payment_redirect_url": "https://gateway.bkash.com/pay/sess_bkash_9123",
      "amount": 800.00
    }
  }
  ```

### `POST /api/v1/webhooks/payment/{provider}`
- **Purpose:** Webhook endpoint for payment providers (e.g., BKASH) to confirm payment status. Transitions appointment from `PENDING_PAYMENT` to `CONFIRMED`.
- **Auth:** Provider-specific Signature / HMAC Verification.
- **Response:** `200 OK` (Provider specific acknowledgment).

### `POST /api/v1/appointments/{id}/prescriptions`
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
        { "id": "doc-01", "page_number": 1, "mime_type": "image/jpeg", "size_bytes": 1245000 },
        { "id": "doc-02", "page_number": 2, "mime_type": "image/jpeg", "size_bytes": 1450000 },
        { "id": "doc-03", "page_number": 3, "mime_type": "image/jpeg", "size_bytes": 980000 }
      ]
    }
  }
  ```

### `GET /api/v1/appointments/{id}/documents/{document_id}`
- **Purpose:** Returns a short-lived pre-signed download URL for a specific document, enforcing ABAC authorization checks.
- **Auth:** `PATIENT` or `DOCTOR`.
- **Response:** `200 OK`
  ```json
  {
    "success": true,
    "data": {
      "document_id": "doc-01",
      "download_url": "https://storage.helodoc.com/pre-signed/doc-01?expires=...",
      "expires_in": 300
    }
  }
  ```

---

## 5. Telehealth Video (Agora RTC) & Telemetry Capture

### `POST /api/v1/consultations/{appointment_id}/rtc-token`
- **Purpose:** Mints an authorized, short-lived Agora Dynamic RTC Token for entering a video consultation room. Note: Channel names are server-generated UUIDs. They must NOT be derived from appointment IDs, phone numbers, doctor IDs, or patient IDs.
- **Auth:** `PATIENT` or `DOCTOR` (Caller must be participant in the appointment).
- **Request Body:** Empty.
- **Response:** `200 OK`
  ```json
  {
    "success": true,
    "data": {
      "channel_name": "rtc_7baf3072_96bd_4fae_a735_e8d2c1f94b3a",
      "token": "007eJxTYGCoM2y+5tW2Z1n380vP4...",
      "uid": 94812,
      "app_id": "a1b2c3d4e5f6...",
      "expires_in": 3600
    }
  }
  ```

### `POST /api/v1/consultations/{appointment_id}/telemetry`
- **Purpose:** Submits auto-captured technical telemetry upon consultation termination (derived from Agora's `onRtcStats`). Telemetry is cross-validated server-side with Agora webhooks/callbacks to ensure integrity.
- **Auth:** System / Doctor Client / Patient Client.
- **Request Body:**
  ```json
  {
    "call_duration_seconds": 15,
    "connection_state": "COMPLETED",
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

### `POST /api/v1/consultations/{appointment_id}/prescription`
- **Purpose:** Uploads a photo of the handwritten prescription after consultation.
- **Auth:** `DOCTOR` (must be the assigned doctor for this appointment).
- **Content-Type:** `multipart/form-data`
- **Form Fields:**
  - `rx_image`: Single photo of handwritten prescription (JPEG/PNG, max 10MB)
  - `doctor_notes`: Optional text notes (max 500 chars)
- **Response:** `201 Created`
  ```json
  { "success": true, "data": { "rx_id": "rx-101", "document_id": "doc-505" } }
  ```

### `POST /api/v1/consultations/{appointment_id}/complete-no-rx`
- **Purpose:** Completes a consultation without a prescription.
- **Auth:** `DOCTOR`
- **Request Body:**
  ```json
  { "reason": "Follow-up consultation — lifestyle advice provided" }
  ```
- **Response:** `200 OK`
  ```json
  { "success": true, "data": { "status": "COMPLETED_NO_RX" } }
  ```

---

## 6. Doctor Wallet & Transparent 20% Fee Debarment

### `GET /api/v1/doctor/wallet/summary`
- **Purpose:** Retrieves doctor's earnings strictly formatted with 20% fee debarment. Doctor can only READ their earnings; monthly disbursement is handled by Finance Admin.
- **Response:** `200 OK`
  ```json
  {
    "success": true,
    "data": {
      "doctor_name": "Dr. Sabrina Akter",
      "license_number": "BMDC #45821",
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

### `POST /api/v1/admin/disbursements/initiate`
- **Purpose:** Initiates the monthly batch disbursement of funds to doctors.
- **Auth:** `FINANCE_ADMIN`
- **Request Body:**
  ```json
  { "period_start": "2026-09-01", "period_end": "2026-09-30" }
  ```
- **Response:** `201 Created`
  ```json
  {
    "success": true,
    "data": {
      "batch_id": "batch-1234",
      "total_doctors": 120,
      "total_amount": 450000.00
    }
  }
  ```

### `GET /api/v1/admin/disbursements/{batch_id}`
- **Purpose:** Retrieves batch details with per-doctor breakdown.
- **Auth:** `FINANCE_ADMIN`
- **Response:** `200 OK`

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
- **Purpose:** Admin Board disburses payment refund to patient MFS account.
- **Auth:** `FINANCE_ADMIN` or `PLATFORM_ADMIN`.
- **Response:** `200 OK`
  ```json
  {
    "success": true,
    "data": {
      "grievance_id": "GRV-20261001-73",
      "status": "REFUNDED",
      "board_remedy": "Payment Refund Disbursed (৳800)",
      "refund_trx_id": "BK-REF-876862"
    }
  }
  ```

### `POST /api/v1/admin/grievances/{id}/warn`
- **Purpose:** Admin Board logs a formal internal compliance warning into doctor's dossier.
- **Auth:** `CLINICAL_ADMIN` or `PLATFORM_ADMIN`.
- **Response:** `200 OK`
  ```json
  {
    "success": true,
    "data": {
      "grievance_id": "GRV-20261001-73",
      "status": "WARNED",
      "board_remedy": "Internal Platform Compliance Warning Logged",
      "doctor_license": "BMDC #45821"
    }
  }
  ```

---

## 8. Admin Oversight & Dossier Endpoints

### `GET /api/v1/admin/doctors/{id}/dossier`
- **Purpose:** Comprehensive physician dossier including verified phone and residential address.
- **Auth:** `PLATFORM_ADMIN` or `COMPLIANCE`.
- **Response:** `200 OK`
  ```json
  {
    "success": true,
    "data": {
      "id": "doc-anika",
      "name": "Dr. Anika Rahman",
      "license_number": "BMDC #52891",
      "phone": "+880 1711-884920",
      "residence": "Dhanmondi, Dhaka (House 42, Road 7A)",
      "email": "dr.anika.dmch@helodoc.com",
      "hospital": "Dhaka Medical College Hospital",
      "lifetime_consultations": 342,
      "disciplinary_warnings": []
    }
  }
  ```
