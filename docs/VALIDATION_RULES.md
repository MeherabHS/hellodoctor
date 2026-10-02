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
| **Phone Number (BD)** | Auth / Profile | Regex `^(?:\+8801\|01)[3-9]\d{8}$` | Normalized to E.164: `^\+8801[3-9]\d{8}$` | `INVALID_PHONE_NUMBER` |
| **Phone Number (KE)** | Auth / Profile | Regex `^(?:\+254\|0)[17]\d{8}$` | Normalized to E.164: `^\+254[17]\d{8}$` | `INVALID_PHONE_NUMBER` |
| **Password** | Registration / Reset | Min 12 chars, 1 uppercase, 1 digit, 1 symbol | Min 12 chars, checked against Pwned Passwords list | `WEAK_PASSWORD` |
| **Medical License Number** | Doctor Profile | String starting with "BMDC #" or "BMDC A-" | Regex `^BMDC\s*(?:#\|A-)\d{4,6}$` + DB uniqueness | `INVALID_BMDC_NUMBER` |
| **Prescription Photos** | Pre-Consult Intake | Max 5 files total; files > 10 MB rejected | Array length $1 \le N \le 5$, file size $\le 10,485,760$ bytes, MIME $\in \{\text{image/jpeg}, \text{image/png}, \text{application/pdf}\}$ | `MAX_PRESCRIPTION_IMAGES_EXCEEDED` |
| **Consultation Slot** | Booking | Must be future date within 14 days | `start_time > now() AND start_time < now() + INTERVAL '14 days'`, slot status == `AVAILABLE` (Row-locked) | `SLOT_NOT_AVAILABLE` |
| **Consultation Fee** | Pricing / Billing | Positive decimal, min configurable amount, max configurable amount,000 | `fee >= min_fee AND fee <= max_fee`, strictly rounded to 2 decimals | `INVALID_FEE_AMOUNT` |
| **Platform Charge** | Financial Settlement | Computed: `fee * 0.20` | Server-enforced: `withheld = round(gross * 0.20, 2)`, `net = gross - withheld` | `FEE_CALCULATION_MISMATCH` |
| **Grievance Statement** | Dispute Filing | Min 10 chars, max 2000 chars | String length between 10 and 2000 chars, scrubbed of HTML tags | `STATEMENT_TOO_SHORT` |
| **Integrity Verification Hash** | Prescription Sign | Doctor biometric / PIN confirmation | HMAC-SHA256 integrity verification hash computed with server-managed key | `INVALID_DIGITAL_SIGNATURE` |
| **Chat Message** | 24h Chat Desk | Min 1 char, max 1000 chars; blocked if expired | `now() < conversation.expires_at`, text length $1 \le L \le 1000$ | `CHAT_WINDOW_EXPIRED` |

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
