# Phase 17 — Comprehensive Quality Assurance & Testing Strategy

> **Document Version:** 1.0.1  
> **Testing Pyramid:** Unit Tests (70%), Integration Tests (20%), End-to-End System Tests (10%)  
> **Target Audience:** Claude Code Autonomous Implementation Agent

---

## 1. Flutter Mobile Test Suite

### 1.1 Unit Tests (Dart `test` & `bloc_test`)
- **BLoC State Transitions:**
  - `PrescriptionIntakeBloc`: Test that adding 6 files caps at 5 and emits a warning message.
  - `AppointmentBookingBloc`: Test slot selection, gateway choice, and validation failures.
  - `DoctorWalletBloc`: Test 3-tier math calculation ($G \to C \to N$).
- **Mappers & Validators:**
  - Phone number regex verification for Bangladesh (`+880`) and Kenya (`+254`).
  - Fee formatters and currency symbol localization (`৳` vs. `KES`).

### 1.2 Widget Tests (Flutter `flutter_test`)
- **Emergency Helpline Strip:** Verify that `#patientEmergencyHelplineStrip` renders directly above Upcoming Appointments and strictly omits any *"24/7"* prefix.
- **Doctor Card:** Verify that doctor credentials and BMDC numbers are displayed and that **zero star rating icons (`⭐`, `★`)** exist in the widget tree.
- **Doctor Earnings Card:** Verify that rows for Gross, 20% Platform Charge, and Net Final Earning are rendered alongside the mandatory basis note: *"Total calculation is based including the platform charge 20%."*

### 1.3 Integration Tests (`integration_test/`)
- **Full Booking & Waiting Room Loop:**
  1. Launch app -> Home (`view-0`).
  2. Tap "Doctor Consultation" -> Select Dr. Sabrina Akter.
  3. Select tomorrow's 10:00 AM slot.
  4. Attach 3 prescription photos -> Verify thumbnails.
  5. Select bKash -> Confirm booking.
  6. Assert navigation into Waiting Room (`view-10`) with countdown timer active.
  7. Verify Agora RTC connection health indicator activates upon doctor entry.

---

## 2. Rust Backend Test Suite

### 2.1 Unit Tests (`cargo test`)
- **Financial Arithmetic:**
  ```rust
  #[test]
  fn test_doctor_earnings_debarment_math() {
      let gross = BigDecimal::from_str("35562.50").unwrap();
      let platform_rate = BigDecimal::from_str("0.20").unwrap();
      let withheld = (&gross * &platform_rate).round(2);
      let net = &gross - &withheld;

      assert_eq!(withheld, BigDecimal::from_str("7112.50").unwrap());
      assert_eq!(net, BigDecimal::from_str("28450.00").unwrap());
  }
  ```
- **Validation Rules:** Test BMDC number validation, slot time boundary enforcement, and photo upload limits.

### 2.2 Integration Tests (SQLx + PostgreSQL Testcontainers)
- **Concurrent Slot Double-Booking Prevention:**
  - Launch 10 concurrent async tasks attempting to book the identical slot ID.
  - Assert that exactly **1 task succeeds (`201 Created`)** and **9 tasks fail with `409 Conflict` (`SLOT_NOT_AVAILABLE`)**.
  - Verify that database row remains in `BOOKED` state without corrupting the payment ledger.
- **Grievance Refund Payment Disbursal:**
  - Seed transaction in `PAYMENT_HELD`.
  - Execute `POST /api/v1/admin/grievances/{id}/refund`.
  - Assert transaction updates to `REFUNDED` and patient wallet is credited.
- **Payment Adversarial Tests:**
  - Double-submit payments.
  - Webhook replay attacks.
  - Timeout handling logic during payment gateway verification.
- **Idempotency Persistence Tests:**
  - Verify `Idempotency-Key` headers correctly prevent duplicate transactions and return identical saved responses without executing business logic twice.

---

## 3. Security Testing

- **BOLA/IDOR Tests:** Verify cross-user access attempts (e.g., Doctor A attempting to access Patient B's records) return 403 Forbidden.
- **Auth Session Reuse Detection:** Verify that using an invalidated refresh token triggers family revocation.
- **Agora Token Authorization:** Ensure RTC tokens are strictly bound to specific appointments and only issued to verified participants.
- **File Upload Adversarial Tests:** Validate malicious MIME types, oversized payloads, and executable uploads are safely rejected or quarantined.
- **PHI Leakage Tests:** Scan push notifications, error responses, and system logs to confirm they are sanitized of protected health information.

---

## 4. Coverage Targets & Continuous Integration

- **Financial & Payment-Hold Engine:** 100% statement and branch coverage required.
- **Grievance & Telemetry Capture:** 95%+ coverage required.
- **Clinical Services & Prescription-Photo Integrity Pipeline:** 90%+ coverage required.
- **Client Presentation & UI Blocs:** 85%+ coverage required.
