# Patient payment UX — 2026-10-16

Unpaid consultations now have one primary **Proceed to payment** action. Consultation entry is absent before server confirmation. Refresh status and Attach health records are secondary; the reference is the existing human-readable appointment number. Development configuration paragraphs and Patient cancellation controls are absent. The existing binding rule prohibiting Patient cancellation is preserved.

## Backend and state truth

The existing booking transaction already creates an INITIATED transaction/payment session. There is no live collection adapter or valid checkout URL; the old booking response contains a `.invalid` placeholder that the new UI never opens.

`POST /appointments/:id/payment` checks Patient ABAC, booking rate limits, RLS/guardian ownership, PENDING_PAYMENT state, database slot time, and the existing payment session. It creates no appointment, transaction, provider attempt or charge. With the current unavailable provider it returns a controlled 409: **Payment service is currently unavailable. Please try again later.** This replaces engineering configuration copy with an honest user-facing result.

The Flutter repository calls that backend action. Its checkout-opening path accepts only a backend-returned HTTPS URI and rejects `.invalid` hosts; it never constructs provider URLs. A real adapter must still be implemented/configured before redirect verification can pass. No live M-Pesa or other external payment acceptance is claimed.

| Actual state | UI |
| --- | --- |
| PENDING_PAYMENT / INITIATED | Payment required; Proceed to payment |
| PENDING_PAYMENT / FAILED | Payment failed; retry via Proceed to payment |
| Payment request in flight | Starting payment; button disabled |
| CONFIRMED / WAITING / IN_CONSULTATION / COMPLETED | Consultation action; payment CTA removed |
| CANCELLED / REFUNDED / NO_SHOW / EXPIRED / PREMATURE_TERMINATION / DISPUTED | No payment action |

There is no distinct provider-processing state in the current payment enum and no configured provider attempt to represent. The UI does not manufacture one. Clicking payment never changes confirmation locally.

Four genuine concurrent payment requests exposed a nested database-pool acquisition deadlock. The handler now releases its first read transaction before guardian resolution. All four requests retain exact 409 expectations, with exactly one original appointment/transaction and unchanged PENDING_PAYMENT/INITIATED state. No concurrency count, timeout or assertion was relaxed.

## Checks and rendered evidence

Unpaid, failed and confirmed regressions preserve price/state assertions. New tests cover one primary CTA, absent consultation entry, no development paragraph, request-in-flight protection, provider-unavailable/no-fake-success, and 320/360/390/412dp layouts. Backend tests cover unrelated Patient 404, Doctor 403, concurrent requests and exact financial postconditions.

The actual Flutter widget rendering was inspected at narrow widths with the bundled font. [320dp pending screen](audit_evidence/patient_consultation/chat_payment_pending_320.png). This uses synthetic test responses and is explicitly **not** a phone screenshot or provider acceptance test.

## Shared quality gates

| Suite | Before | Final observed result |
| --- | --- | --- |
| Rust library | 53 | 53 passed, 0 failed, 0 ignored, 0 filtered |
| Backend integration | 215 | 221 passed, 0 failed, 0 ignored, 0 filtered |
| Doctor Flutter | 190 | 197 passed, 0 failed, 0 skipped |
| Patient Flutter | 185 | 195 passed, 0 failed, 0 skipped |
| Doctor Web | 47 | 50 passed, 0 failed, 0 skipped |
| Admin Web | 13 | 13 passed, 0 failed, 0 skipped |

Rust formatting/Clippy and both Flutter format/analyze gates pass. Doctor Web typecheck/lint/format/tests/build and Admin typecheck/lint/tests/build pass. Release APKs are built for development LAN testing with explicit development OTP; device installation is deferred by the user. The full serialized backend run completed with exit 0; final output reports all 221 tests passed.

Logs, test inventory, exact test diff, live checks and release hashes are in [the evidence directory](audit_evidence/existing_features_device_e2e/).

## Test-integrity audit

The baseline froze 66 existing test files. Four existing files changed:

- `services/backend/tests/api_tests.rs`: six tests appended; all original content and assertions preserved.
- `apps/mobile/test/doctor_widgets_test.dart`: two existing cases explicitly select All Scheduled after the requested default changed. Their fixtures and assertions remain unchanged. New tests separately enforce the Upcoming default.
- `apps/mobile_patient/test/consultation_booking_test.dart`: its INITIATED/FAILED/HELD cases now inspect the correct primary action. The old `button.onPressed != null == PAYMENT_HELD` assertion for a disabled Open consultation becomes an enabled Proceed/Open action assertion, plus exact absence of Open consultation before payment and exact absence of Proceed after confirmation. The pending-copy expectation changes from Awaiting payment confirmation to Payment required. Failed/confirmed state, price and overflow assertions remain.
- `apps/mobile_patient/test/consultation_flow_test.dart`: pending-copy expectation changes identically. The disabled Open consultation check becomes **Open consultation absent + Proceed to payment enabled**, preserving and strengthening the paid-access invariant. Check payment status navigation becomes Refresh status. The slot-label lookup uses Nairobi time instead of host-local time, as explicitly requested. Booking modality/slot/gateway and confirmed-chat/message assertions remain unchanged.

These are intentional UI-contract updates, not changes to expected financial/security behavior. No existing test case, security/business/concurrency assertion or race participant was removed. [Exact diff](audit_evidence/existing_features_device_e2e/correction_test_diff.txt), [audit inventory](audit_evidence/existing_features_device_e2e/correction_test_integrity.json).

`EXISTING_TESTS_DELETED=0; EXISTING_ASSERTIONS_DELETED=0; TEST_ASSERTIONS_WEAKENED=0; TESTS_SKIPPED_TO_FORCE_GREEN=0; TEST_FAILURES_SWALLOWED=0`.

## Status

- PATIENT_PROCEED_TO_PAYMENT_UX_VERIFIED: automated + inspected widget rendering.
- PATIENT_PAYMENT_INITIATION_BACKEND_WIRED: verified real handler and ownership/concurrency regressions.
- PAYMENT_PROVIDER_REDIRECT_READY: **NOT_VERIFIED — collection adapter is absent**; client URI-opening path is prepared.
- NO_FAKE_PAYMENT_SUCCESS_VERIFIED: exact backend and Flutter assertions.
- PATIENT_PAYMENT_PHYSICAL_DEVICE_VERIFIED: **DEFERRED_BY_USER**.
- VIDEO_AGORA_IMPLEMENTATION: **DEFERRED**.
