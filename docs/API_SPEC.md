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
  - `Idempotency-Key: <UUIDv4>` (Required for financial mutations, booking mutations, AND file upload mutations.)

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
  { "phone_number": "+254700000002", "role": "PATIENT" }
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
        "phone_number": "+254700000002",
        "role": "PATIENT",
        "preferred_language": "en"
      }
    }
  }
  ```

### `POST /api/v1/auth/doctor/login`
- **Purpose:** Initiates doctor login. Registered email address + password. Professional registration is reviewed separately. (Step 1 of MFA).
- **Auth:** Public.
- **Request Body:**
  ```json
  { "email": "synthetic.doctor@example.test", "password": "SecurePassword123!" }
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

Admin TOTP is backed by the encrypted `mfa_credentials` model in `DATABASE_SCHEMA.md`. Enrollment, recovery, reset, and sensitive admin actions follow the lifecycle in `AUTH_SECURITY.md`.

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
        "name": "Dr. Synthetic Doctor One",
        "license_number": "SYNTH-KE-0001",
        "specialty": "Internal Medicine",
        "experience_years": 12,
        "current_hospital": "Synthetic Nairobi Clinic",
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
      "payment_session_id": "7baf3072-96bd-4fae-a735-e8d2c1f94b3a",
      "payment_redirect_url": "https://gateway.bkash.com/pay/provider-session-token",
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
- **Headers:** `Idempotency-Key: <UUID>` (Required to prevent duplicate uploads on mobile network retries.)
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
- **Headers:** `Idempotency-Key: <UUID>` (Required to prevent duplicate uploads on mobile network retries.)
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
      "doctor_name": "Dr. Synthetic Doctor One",
      "license_number": "SYNTH-KE-0001",
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
      "doctor_license": "SYNTH-KE-0001"
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
      "name": "Dr. Synthetic Doctor Two",
      "license_number": "SYNTH-KE-0002",
      "phone": "+254700000101",
      "residence": "Synthetic Nairobi, Kenya",
      "email": "dr.anika.dmch@helodoc.com",
      "hospital": "Synthetic Nairobi Clinic",
      "lifetime_consultations": 342,
      "disciplinary_warnings": []
    }
  }
  ```


## Current Kenya professional-registration API (2026-10-08)

Manual HelloDoctor review of submitted KMPDC registration; not an external registry
or annual practice-licence integration. Existing `license_number` is the submitted
registration number. New public/Admin-created Doctors are PENDING, off duty,
`license_authority=KMPDC`, `license_country=KE`. Public registration rejects
unknown/privileged fields rather than accepting client verification state.

All routes require JWT authentication except public registration/login:

| Route | Allowed reviewers | Contract |
| --- | --- | --- |
| `GET /api/v1/admin/doctors/pending-verification` | ClinicalAdmin, PlatformAdmin | Only PENDING profiles; VERIFIED and REJECTED excluded |
| `POST /api/v1/admin/doctors/{doctor_id}/verify` | Same | Optional note; PENDING → VERIFIED |
| `POST /api/v1/admin/doctors/{doctor_id}/reject` | Same | Optional note; PENDING → REJECTED |

Mutation response: `{ "success": true, "data": { "doctor_id": "...",
"professional_verification_status": "VERIFIED", "status": "VERIFIED",
"is_verified": true } }`. Rejection returns REJECTED and false. Concurrent/repeated
terminal review returns 409 for the loser. Audit, metadata and notification share
the status transaction. The own profile includes the explicit status, review note,
review timestamp and reviewer; public directory records omit internal review metadata.
The overview count is `pending_professional_verification`. No old-country route alias
remains. See [schema/backfill and client mappings](KENYA_PROFESSIONAL_VERIFICATION.md).

## Doctor Articles ? current direct-publication contract

[Doctor Articles](DOCTOR_ARTICLES.md) defines the implemented `DRAFT / PUBLISHED / ARCHIVED` lifecycle, structured fields, APIs, optimistic versions, forced ownership/publication RLS and migration `20261025000001`. Doctors publish directly; no Admin review state or queue exists. Professional verification remains its separate existing state machine. [Session hardening](PUBLIC_EXPOSURE_HARDENING.md) requires an active JWT `sid` family on protected reads/writes.

## Legal & Professional Agreements consent gate (2026-10-27)

`POST /api/v1/auth/doctor/register` now additionally requires a non-empty
`accepted_documents` array, each entry `{ "type": "<DOCUMENT_TYPE>", "version": "<string>" }`.
The server independently re-checks this against its own current-documents
registry — the client supplies only a type and version, never a hash — and
rejects (422) if any CURRENTLY required document is missing or any submitted
document is not the current version. Required types: `DOCTOR_TERMS`,
`PRIVACY_NOTICE`, `PROFESSIONAL_VERIFICATION_DECLARATION`,
`TELEMEDICINE_PRACTITIONER_POLICY`. Optional: `MARKETING_COMMUNICATIONS`.
Acceptance rows are written in the same transaction as the `users`/
`doctor_profiles`/`doctor_wallets` inserts, so the account and its
acknowledgements land atomically or not at all. See
[ADR-011](ADR/ADR-011-legal-consent-ledger.md).

| Route | Auth | Contract |
| --- | --- | --- |
| `GET /api/v1/legal/documents` | Public | Returns all current documents: `[{ "document_type", "version", "title", "body", "required" }]`. Backs the mobile Legal & Consent screen — body text is always fetched from here, never duplicated as static strings client-side. |
| `POST /api/v1/auth/patient/register` | Public | Patient registration step 1: `{email, phone_number, password}`. Creates a short-lived email OTP challenge; account creation waits for OTP and current consent. |
| `POST /api/v1/auth/patient/login` | Public | Patient login step 1: `{email, password}`. Verifies the password and sends a short-lived Azure email OTP challenge. |
| `POST /api/v1/auth/patient/verify-otp` | Public | Patient MFA step 2: `{challenge_token, otp_code, accepted_documents?}`. Registration requires the current Patient data-sharing consent; login does not. |
| `POST /api/v1/auth/verify-otp-and-login` | Public | For a new Patient, accepts `accepted_documents: [{"type":"PATIENT_DATA_SHARING_CONSENT","version":"1.0"}]`; the current server version is required and is recorded atomically with account/profile creation. |
| `GET /api/v1/patients/legal/acceptances` | Patient (own) | Returns the Patient's immutable consent-evidence history. |
| `GET /api/v1/doctor/legal/acceptances` | Doctor (own) | That Doctor's own immutable acceptance history. |
| `GET /api/v1/admin/legal/acceptances/{user_id}` | Compliance, PlatformAdmin, ClinicalAdmin | Evidence view of a specific user's acceptance history (`abac::Action::ViewLegalAcceptances`). |

No role is ever granted `UPDATE`/`DELETE` on `legal_acceptances` — a changed
document version produces a new row, never a mutation of an old one. Forcing
re-consent at next login when a document version is bumped, and a dedicated
marketing-consent revoke endpoint, are both out of scope for this pass (the
data model supports detecting staleness; no login-time gate/UI is built yet).


### Patient Q&A owner-read correction (2026-10-14)

`GET /api/v1/patients/me/qna/questions?include_own_open=true` additively includes the authenticated Patient's own and guardian-dependent OPEN questions alongside the public ANSWERED feed. Omitting the parameter preserves the answered-only result. Each row includes `status` (`OPEN`/`ANSWERED`) and `answering_doctor_verified`; OPEN answer/doctor fields are null. Author IDs remain inaccessible and are never serialized. POST returns the server-created OPEN DTO with HTTP 201. CHAT consultation availability continues to use the existing positive-fee directory contract and requires slot selection/payment.

### Consultation client correction (2026-10-14)

No new booking, payment or messaging endpoint is introduced. Existing response projections add:

- `GET /appointments/:id`: nullable transaction `payment_status`, under the same authenticated role/ownership context as the appointment.
- `GET /patients/me/appointments`: nullable `chat_expires_at` and `chat_locked`, scoped to the requested authorized self/dependent profile; existing `consultation_fee`, Doctor identity and modality remain authoritative.
- `GET /doctor/appointments`: nullable `conversation_id`, `chat_expires_at` and `chat_locked`; Patient name and modality are still joined from authorized backend records.
- `GET /doctor/chats`: `modality` and `appointment_status` for modality-specific entry and read-only rendering.
- `GET /appointments/:id/conversation`: closed appointment states present `is_locked=true` without deleting authorized history. Resolution remains read-only and never creates a paid consultation.

`POST /chat/:id/messages` rejects unactivated/cancelled/refunded/no-show appointments as well as existing locked/expired conversations. RTC authorization rejects CHAT modality before minting. Opening the same paid booking repeatedly, or concurrent VIDEO authorization when the session was initially absent, resolves the one stored appointment-scoped conversation/session. Nonpositive configured modality fees return 409 from booking. Existing successful payment transitions and VIDEO follow-up messaging are preserved. See [verification contract and evidence](PATIENT_CHAT_CONSULTATION_VERIFICATION.md).


## 2026-10-16 pre-E2E additive contracts

- Doctor appointments return database-calculated `queue_actionable`. `view=upcoming` filters to IN_CONSULTATION/current/future work, including an authorized still-active Chat window. The complete endpoint remains available for history. Default client surfaces use the flag and server ordering, not client clocks. Own-slot rows include `bookable`; Patient public slots require AVAILABLE and a future full UTC start timestamp. Booking revalidates using the wall clock after the row lock.
- `POST /appointments/:id/payment`: Patient-owned existing payment attempt only; role/RLS/guardian/state/start-time checks and booking rate limit. With the current absent collection adapter it returns controlled 409 and performs no economic mutation. Clients must never open the old `.invalid` booking URL or infer confirmation locally.
- `POST /chat/:conversation_id/attachments`: verified Doctor, authorized active clinical context, JPEG/PNG `{mime_type, base64_content}` and required `Idempotency-Key`. Existing normalization guarantees at most 40 KiB per stored image. Same scoped key/image is concurrency-safe. Returns the authoritative chat message and safe attachment metadata, without private storage keys.
- `GET /chat/:conversation_id/messages/:message_id/attachment`: authenticated conversation/document authorization on every view, including Doctor clinical-window check; returns `{mime_type, base64_content}` with no-store/private/nosniff headers. No public or bearer document URL is issued. Existing encrypted DocumentStorageService remains the only byte-storage path; unconfigured live storage fails honestly.

No database migration or new business enum was introduced. Video/Agora media remains deferred.
