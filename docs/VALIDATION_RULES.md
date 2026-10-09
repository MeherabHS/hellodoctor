# Phase 15 — Client & Server Validation Rules Specification

> **Document Version:** 1.0.0  
> **Core Principle:** Defense-in-Depth — Client validation provides immediate UX feedback; Server validation strictly enforces security and business integrity.  
> **Target Audience:** Claude Code Autonomous Implementation Agent

---

## 1. Validation Architecture

```
[User Input in Flutter]
       │
       ▼
1. CLIENT VALIDATION (FormValidators in Dart)
   • Immediate UI error messages below inputs
   • Disables submit buttons when invalid
   • Never trusted by server
       │
       ▼ (HTTPS REST / JSON)
2. SERVER VALIDATION (Rust `validator` crate)
   • Strict regex, bounds, and enum checks
   • Database foreign key & uniqueness verification
   • Cryptographic signature verification
```

---

## 2. Validation Rules Matrix

| Field / Action | Scope | Client Rule (Flutter) | Server Rule (Rust) | Error Code |
|---|---|---|---|---|
| **Phone Number (KE)** | Auth / Profile | Regex `^(?:\+254\|0)[17]\d{8}$` | Normalized to E.164: `^\+254[17]\d{8}$` | `INVALID_PHONE_NUMBER` |
| **Password** | Registration / Reset | Min 12 chars, 1 uppercase, 1 digit, 1 symbol | Min 12 chars, checked against Pwned Passwords list | `WEAK_PASSWORD` |
| **KMPDC Registration Number** | Doctor Profile | Required submitted registration number | Generic `license_number`, trimmed, minimum 3 characters, DB uniqueness; no fabricated regulator regex or external lookup | `VALIDATION_ERROR` |
| **Prescription Photos** | Pre-Consult Intake | Max 20 files per consultation; files > 10 MB rejected | Array length $1 \le N \le 20$, file size $\le 10,485,760$ bytes, MIME $\in \{\text{image/jpeg}, \text{image/png}, \text{application/pdf}\}$ | `MAX_PRESCRIPTION_IMAGES_EXCEEDED` |
| **Consultation Slot** | Booking | Must be future date within 14 days | `start_time > now() AND start_time < now() + INTERVAL '14 days'`, slot status == `AVAILABLE` (Row-locked) | `SLOT_NOT_AVAILABLE` |
| **Consultation Fee** | Pricing / Billing | Positive decimal, min configurable amount, max configurable amount,000 | `fee >= min_fee AND fee <= max_fee`, strictly rounded to 2 decimals | `INVALID_FEE_AMOUNT` |
| **Platform Charge** | Financial Settlement | Computed: `fee * 0.20` | Server-enforced: `withheld = round(gross * 0.20, 2)`, `net = gross - withheld` | `FEE_CALCULATION_MISMATCH` |
| **Grievance Statement** | Dispute Filing | Min 10 chars, max 2000 chars | String length between 10 and 2000 chars, scrubbed of HTML tags | `STATEMENT_TOO_SHORT` |
| **Prescription-Photo Integrity** | Prescription Photo Upload | Doctor confirms the captured/uploaded photo before submission | HMAC-SHA256 integrity verification hash computed with a server-managed key | `INTEGRITY_VERIFICATION_FAILED` |
| **Chat Message** | 24h Chat Desk | Min 1 char, max 1000 chars; blocked if expired | `now() < conversation.expires_at`, text length $1 \le L \le 1000$ | `CHAT_WINDOW_EXPIRED` |

---

### 2.2 Backend free-text field validation, strict email/phone, and password strength (2026-10-07)

The matrix above predates this pass and is partly aspirational (e.g. the Pwned-Passwords check and
KMPDC-format license regex were never implemented — this is a Kenya deployment with manual KMPDC registration review; the
chat-message figure of 1000 chars below is now superseded by 2000, see below). This section
documents what is actually implemented and enforced, both server-side (authoritative) and
client-side (Flutter/admin-web, done in an earlier pass this session).

**Shared helper**: `services/backend/src/domain/validation.rs::validate_text_field(value,
field_name, max_len)` trims whitespace, strips control characters other than `\n`/`\r`/`\t`, and
rejects anything over `max_len` *characters* (not bytes). This is a resource-abuse/DB-integrity
measure, not an XSS mitigation — neither the Flutter app nor the admin-web/React UI renders raw
HTML anywhere, so there is no stored-XSS rendering vector to close here.

**Canonical length limits applied across every free-text field found in a full sweep of the
handler layer** (matching the client-side limits already applied this session):
- Short labels (categories, admin notes/actions, simulate custom notes, provider references):
  **150-300 chars** depending on field (see source for the exact per-field number).
- Clinical/administrative free text (grievance claim summary, audit notes, "reason for no Rx",
  chief complaint, resolution notes): **1000 chars**.
- Chat messages: **2000 chars** (supersedes the older 1000-char figure in the matrix above).
- Fields that already had a bespoke, sensible limit before this pass (doctor registration's
  `full_name`/120, `primary_specialty`/80, `current_hospital`/200, `residential_address`/500,
  `qualifications`/150 each; doctor profile `full_name`/150; Q&A `answer_text`/4000; Smart Rx
  `doctor_notes`/4000) were left as-is rather than homogenized — and `POST /admin/doctors`
  (admin-created doctors) was found to accept `full_name`/`primary_specialty`/
  `residential_address` with **zero** length validation at all (a real gap, now fixed to match
  the self-service registration path's own numbers, keeping the two doctor-creation paths
  consistent with each other).

**Stricter email validation**: `validate_email` now additionally requires a genuine TLD shape —
the label after the last `.` must be alphabetic-only and at least 2 characters. Previously a
degenerate domain like `a@b.c` passed.

**Phone number digit-only enforcement**: `validate_phone_number` (the dual KE validator) only
checked prefix + total length, not that the remaining characters were actually digits — e.g.
`+8801abcdefgh` (right prefix, right length) previously passed. Fixed to require every character
after an optional leading `+` to be an ASCII digit, matching the stricter behavior
`normalize_kenya_phone_number` already had.

**Doctor password strength** (`validate_password_strength`, new): applies to the Doctor
self-service registration password only (`POST /auth/doctor/register`) — patient login is
OTP-based with no password at all, and admin accounts are seeded/managed, not self-service, so
neither is in scope. Requires at least 8 characters (the existing, stricter 12-character minimum
on this same endpoint is unchanged and already satisfies this), at least one uppercase letter, one
lowercase letter, one digit, and one special character from a broad punctuation/symbol class
(`!@#$%^&*()_+-=[]{};:,.<>/?\|~`` '"`). No maximum length is imposed — capping a password's
maximum length is a known anti-pattern this codebase does not otherwise follow. The rejection
message names exactly which class(es) are missing.

### 2.3 Doctor registration legal/consent gate (2026-10-27)

`POST /auth/doctor/register` additionally requires a non-empty
`accepted_documents: [{type, version}]` array. Two layers:
- **Syntax** (`domain::validation::validate_accepted_documents`): every `type`
  is one of the five known document-type strings, no duplicate type, version
  non-empty and ≤20 chars.
- **Currency** (inline in `auth_handlers::doctor_register`, DB-backed — same
  style as the existing duplicate-license/phone/email checks): every
  CURRENTLY required document (`DOCTOR_TERMS`, `PRIVACY_NOTICE`,
  `PROFESSIONAL_VERIFICATION_DECLARATION`, `TELEMEDICINE_PRACTITIONER_POLICY`)
  must be present with its current version, and no submitted document may
  reference a non-current version. `MARKETING_COMMUNICATIONS` is optional and
  does not block registration either way.

Both checks fail with `VALIDATION_ERROR` (422). A `BEFORE INSERT` trigger on
`legal_acceptances` re-validates currency independently at the database layer
and overwrites `document_hash` from the server's own `legal_documents` row,
so neither check can be bypassed by calling the API directly. See
[ADR-011](ADR/ADR-011-legal-consent-ledger.md).

### 2.1 Default server-side image compression (Prescription Photos)

The 10 MB figure above is the hard *upload rejection* limit, not the *stored* size. Every
prescription/intake image (doctor Smart Rx scans and patient pre-consultation intake photos)
additionally passes through `UploadSecurityService::validate_and_sanitize`
(`services/backend/src/services/upload_security_service.rs`), which now compresses by default
before storage — new behavior, not present when the 10 MB limit above was first specified.

**The target is a FIXED PER-IMAGE ceiling, never a shared/divided batch budget.** Every
prescription/intake image — regardless of how many images are submitted together in the same
request — is compressed toward its own independent **40 KB** (`MAX_PRESCRIPTION_IMAGE_BYTES =
40 * 1024 = 40,960` bytes) ceiling. A patient's pre-consultation intake request can contain
1-20 photos per consultation; a 20-photo submission must have EACH photo independently land
at/under 40 KB, for a resulting maximum aggregate of 20 × 40 KB = 800 KB per consultation (never
divided, e.g. a 20-image submission does NOT get a shared 800 KB pool split 20 ways — each image
gets the full 40 KB). The doctor's Smart Rx upload submits one photo per API call and is held to
the same 40 KB per-image ceiling; the doctor's own page-count cap (5 pages per appointment) is a
separate, unrelated limit. See docs/PRESCRIPTION_UPLOAD_40KB_20_IMAGE_VERIFICATION.md for the
full before/after contract and test evidence.

**The 40 KB per-image budget is a guarantee, not a best-effort target.** The system auto-compresses
whatever is uploaded to fit the budget; the photo is stored regardless of how hard it is to
compress — there is no rejection on compression-difficulty grounds. (The separate 10 MB *input*
rejection limit above is unrelated and unaffected: that is about refusing absurdly huge raw
uploads before compression even starts, not about how compressible the content turns out to be.)

- Images wider or taller than **2000px** are first downscaled (aspect-ratio preserved) to fit
  within that bound — a generous first pass that cuts a typical modern phone-camera photo's pixel
  count (and therefore size) by 80-90% before any further compression is even attempted.
- JPEGs are then re-encoded toward the fixed 40 KB target, iteratively stepping encoder quality
  down from 80. If the target still isn't hit, the image is progressively downscaled further and
  the quality search repeats — this continues, going lower on both quality and resolution, until
  the target is met.
- PNGs go through the same downscale-and-retry loop; since PNG is lossless (no quality knob),
  resolution is the only lever, so PNG alone may need to downscale further than JPEG to hit the
  same target.
- The only floors are bottom-of-the-barrel technical minimums (JPEG quality down to 1; resolution
  down to 8px on the long edge) that exist solely to avoid asking the encoder for a degenerate or
  invalid result — not legibility protections. A pathological image is allowed to come out looking
  rough in an extreme worst case; it must never come out over its budget. In practice these floors
  are essentially never reached: JPEG's quantization crushes even genuinely incompressible
  per-pixel noise down to well under the budget once quality drops low enough, and PNG's downscale
  loop converges well before 8px for the same reason. See
  docs/PRESCRIPTION_UPLOAD_40KB_20_IMAGE_VERIFICATION.md for measured sizes under the 40 KB
  contract, including the deliberately worst-case synthetic-noise fixture.
- The original format is never converted (PNG stays PNG, JPEG stays JPEG) — only re-compressed —
  so the stored MIME type always matches the originally declared one.

---

## 3. Server-Side Implementation (Rust `validator`)

```rust
use validator::Validate;

#[derive(Debug, Validate, serde::Deserialize)]
pub struct BookAppointmentRequest {
    pub doctor_id: uuid::Uuid,
    pub slot_id: uuid::Uuid,
    
    #[validate(length(min = 10, max = 500, message = "Chief complaint must be between 10 and 500 characters."))]
    pub chief_complaint: Option<String>,
    
    #[validate(custom = "validate_payment_gateway")]
    pub gateway: String,
}

fn validate_payment_gateway(gateway: &str) -> Result<(), validator::ValidationError> {
    match gateway {
        "BKASH" | "NAGAD" | "CARD" | "MPESA" => Ok(()),
        _ => Err(validator::ValidationError::new("unsupported_payment_gateway")),
    }
}
```

## 4. File Upload Security Rules
- Magic-byte validation (server verifies content matches MIME)
- Image re-encoding (strip EXIF, re-encode to safe format)
- PDF structural validation
- No executable/SVG/HTML content
- Per-user storage quotas
- Server-generated object names
