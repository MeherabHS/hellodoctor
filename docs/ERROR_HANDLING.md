# Phase 16 — Standardized Error Handling & Diagnostics Architecture

> **Document Version:** 1.0.0  
> **Security Mandate:** Never expose internal database stack traces, SQL errors, or sensitive infrastructure paths to client applications.  
> **Target Audience:** Claude Code Autonomous Implementation Agent

---

## 1. Error Classification & User vs. Log Separation

```
[System Event / Exception]
       │
       ▼
Is this an internal technical error?
       ├── YES ──> Log the technical stack trace plus allow-listed sanitized diagnostic context to `system_error_logs` with unique `trace_id`
       │           Return sanitized generic message to user with `trace_id` for support.
       │
       └── NO  ──> Map to strongly typed domain error (`VALIDATION_ERROR`, `SLOT_NOT_AVAILABLE`)
                   Return localized, actionable guidance to user in Flutter UI.
```

---

## 2. Standard Error Code Registry

| Error Code | HTTP Status | User-Facing Message (En / Sw) | Actionable Recovery for User |
|---|---|---|---|
| `VALIDATION_FAILED` | `422 Unprocessable` | "Please check the entered information." / "Tafadhali kagua maelezo uliyoweka." | Highlight faulty input field with error text. |
| `MAX_PRESCRIPTION_IMAGES_EXCEEDED` | `422 Unprocessable` | "Maximum 5 prescription photos allowed." / "Upeo wa picha 5 za cheti unaruhusiwa." | Truncate excess files, show toast alert. |
| `SLOT_NOT_AVAILABLE` | `409 Conflict` | "This appointment slot was just booked by another patient." / "Nafasi hii imehifadhiwa na mgonjwa mwingine." | Refresh slot grid, prompt user to select alternative slot. |
| `UNAUTHORIZED` | `401 Unauthorized` | "Session expired. Please log in again." / "Kipindi kimeisha. Tafadhali ingia tena." | Flush expired token, route user to Login screen. |
| `FORBIDDEN` | `403 Forbidden` | "You do not have permission to perform this clinical action." | Display access denied dialog. |
| `CHAT_SESSION_EXPIRED` | `403 Forbidden` | "This 24-hour consultation chat has concluded." / "Mazungumzo haya ya saa 24 yamemalizika." | Lock chat box into read-only state. |
| `RESOURCE_NOT_FOUND` | `404 Not Found` | "Requested doctor or record could not be found." | Render empty state illustration. |
| `GATEWAY_TIMEOUT` | `504 Gateway Timeout` | "We could not confirm the payment status. Please do not pay again while we verify the transaction." | Payment Timeout Recovery Protocol:<br>1. Backend sets transaction to PAYMENT_STATUS_UNKNOWN<br>2. Backend immediately initiates provider status poll/reconciliation<br>3. Possible outcomes:<br>   a. Provider confirms SUCCESS → transition to PAYMENT_HELD, confirm appointment<br>   b. Provider confirms FAILED → transition to FAILED, release slot, allow new payment attempt<br>   c. Provider returns PENDING → continue polling at 15-second intervals (max 5 attempts)<br>   d. Provider unreachable → hold PAYMENT_STATUS_UNKNOWN, alert on-call engineer<br>4. Patient UI shows: 'Verifying your payment status... Please do not close the app.'<br>5. DO NOT re-enable the payment button until provider status is definitively resolved. |
| `INTERNAL_ERROR` | `500 Server Error` | "A temporary server issue occurred. Reference: {trace_id}" | Provide "Contact Support" button with pre-filled trace ID. |

---

## 3. Subsystem Incident Ingestion (Admin Diagnostics)

When unexpected infrastructure failures occur (e.g., MFS IPN webhook failure or Agora signaling crash), the system captures the incident into `system_error_logs`:

```json
{
  "trace_id": "TRC-94812-BKASH",
  "subsystem": "MFS_BKASH_GATEWAY",
  "component": "BkashWebhookHandler",
  "severity": "CRITICAL",
  "action_attempted": "CONFIRM_PAYMENT_HOLD",
  "error_message": "IPN Webhook signature validation failed: Connection reset by peer.",
  "stack_trace": "services/backend/src/services/payment_service.rs:184\nservices/backend/src/api/v1/payment_handlers.rs:92",
  "impacted_user_id": "u-c1f7b8a2-9481-4b72-9132-841920842011",
  "is_resolved": false,
  "logged_at": "2026-10-01T05:32:15Z"
}
```
Administrators can inspect these incidents in `adminPage_logs`. Failure simulation is available only in development and staging environments. It is stripped from production builds.

## 4. Logging Rules
Never dump entire request objects, JWTs, authorization headers, prescription content, uploaded filenames, or PHI. Use allow-list approach for logged fields.

*Note: System error logs are NOT an adequate security/medical audit log. Reference the new audit_events table for healthcare audit trail.*
