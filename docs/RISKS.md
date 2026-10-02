# Phase 26 — System Risk Assessment & Technical Mitigations

> **Document Version:** 1.0.0  
> **Classification Standard:** Likelihood × Impact (LOW, MEDIUM, HIGH, CRITICAL)  
> **Target Audience:** Claude Code Autonomous Implementation Agent

---

## 1. Risk Matrix Overview

| Risk Identifier | Category | Severity | Likelihood | Technical Mitigation Strategy |
|---|---|---|---|---|
| **RSK-001: Concurrent Slot Double-Booking** | Database / Concurrency | **CRITICAL** | High | Pessimistic row-level locks (`SELECT ... FOR UPDATE`) in PostgreSQL transactions; GiST exclusion constraint preventing overlapping time ranges. |
| **RSK-002: MFS Webhook Timeout / Double Debit** | Payments / Escrow | **CRITICAL** | High | Mandatory `Idempotency-Key` header; IPN webhook reconciliation background job (`reconcileAdminMfsWebhooks`). |
| **RSK-003: Exposure of Protected Health Information (PHI)** | Security / Privacy | **CRITICAL** | Low | Encrypted storage at rest (AES-256); automated log scrubbing in Rust tracing layer; pre-signed URLs with 15-minute TTL. |
| **RSK-004: Premature Agora RTC Call Drops (<30s)** | Telehealth / Network | **HIGH** | Medium | Agora SDK telemetry layer captures `call_duration_seconds`; automated flag `premature_end = true` holds escrow funds pending review. |
| **RSK-005: Agora Vendor Dependency & Service Disruption** | Vendor Risk | **HIGH** | Medium | Agora SDK version pinning, fallback audio-only mode, vendor DPA, monitoring Agora status page, maintaining ability to swap to alternative provider. |
| **RSK-006: Prescription Photo Upload Overload** | Storage / Memory | **MEDIUM** | Medium | Strict client & server cap: max 5 files total, max 10 MB per file; excess files rejected with explicit user notification. |
| **RSK-007: SMS OTP Phishing / Telco SIM Swap** | Authentication | **MEDIUM** | Low | Rate limit: max 3 OTP requests per 10 minutes; 120-second TTL; constant-time hash comparison in Redis/PostgreSQL. |
| **RSK-008: Stale Offline State Write Drift** | Mobile Offline | **MEDIUM** | Medium | Destructive actions (booking, payments, Rx signing) blocked via `#offlineActionGuardModal` when `connectivity_plus` reports offline. |
| **RSK-009: Background Push Notification Drops** | Mobile Reliability | **LOW** | Medium | High-priority data pushes via FCM/APNs paired with in-app polling fallback when the app transitions to foreground. |
| **RSK-010: Database Connection Pool Exhaustion** | Backend Scale | **MEDIUM** | Low | Tuning `sqlx::PgPoolOptions` with `max_connections(50)` and 5-second acquire timeouts; load shedding via Tower middleware. |
| **RSK-011: BOLA/IDOR Authorization Bypass** | Security | **CRITICAL** | Medium | ABAC enforcement on every endpoint (role + ownership + relationship + context), integration tests for cross-user access attempts. |
| **RSK-012: PHI Leakage via Push Notifications** | Security / Privacy | **HIGH** | Low | Strict policy prohibiting PHI in push payloads, generic notification text only, sensitive data fetched after authenticated app open. |
| **RSK-013: Medical File Upload Attack (Malware/Steganography)** | Security | **HIGH** | Medium | Magic-byte validation, malware scanning, quarantine, image re-encoding, PDF sanitization. |
| **RSK-014: Pre-signed URL Persistence** | Security | **HIGH** | Low | Persist object keys not URLs, generate pre-signed URLs on-demand after authorization. |

---

## 2. Deep-Dive Mitigations

### 2.1 Mitigation: Concurrent Slot Double-Booking (RSK-001)
- **Problem:** Two patients click "Confirm & Pay" for the identical 15-minute slot at the same millisecond.
- **Mitigation:**
  1. The Rust endpoint initiates an ACID transaction: `let mut tx = pool.begin().await?`.
  2. The slot is fetched with an exclusive row lock:
     ```sql
     SELECT id, status FROM doctor_schedule_slots WHERE id = $1 FOR UPDATE;
     ```
  3. The first transaction marks the status as `BOOKED` and commits.
  4. The second transaction unblocks, detects `status != 'AVAILABLE'`, immediately rolls back, and returns `409 Conflict` (`SLOT_NOT_AVAILABLE`).
  5. The Flutter UI refreshes the slot grid and alerts the user: *"This slot was just booked by another patient. Please select another time."*
  6. Enforced by GiST exclusion constraint preventing overlapping time ranges.

### 2.2 Mitigation: MFS Webhook Timeout & Escrow Protection (RSK-002)
- **Problem:** Patient's mobile wallet is debited by bKash/Nagad, but telco network drops before the IPN webhook reaches the server.
- **Mitigation:**
  1. All booking requests require a unique client-generated UUID in the `Idempotency-Key` header.
  2. The transaction is placed in `ESCROW_HELD`. Funds are never settled directly to the doctor until the consultation completes.
  3. A background reconciliation worker (`reconcileAdminMfsWebhooks`) polls the MFS provider API every 60 seconds to match orphan debits.
  4. If an orphan debit is confirmed without an active session, the Admin Portal flags the incident in `adminPage_logs` and provides a one-click refund trigger.

### 2.3 Mitigation: Premature Call Drop & Escrow Disputes (RSK-004)
- **Problem:** Doctor enters video call, stays for 10 seconds, disconnects, and patient is left without care while money is debited.
- **Mitigation:**
  1. The Agora onRtcStats callback tracks connected call seconds (`callSeconds`).
  2. Upon disconnect, if `callSeconds < 30`, the backend marks `premature_end: true`.
  3. Auto-settlement to the doctor's wallet is blocked.
  4. When the patient opens `#patientGrievanceModal`, the 10-second telemetry record is pre-bound.
  5. The Admin Grievance Board sees call duration: `0m 10s` and prescription: `Not Issued`, allowing one-click refund disbursal.
