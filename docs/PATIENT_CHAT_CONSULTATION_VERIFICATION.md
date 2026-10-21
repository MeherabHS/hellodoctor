# Patient consultation implementation and verification

Date: 2026-10-14. Scope: Patient booking/reopen/payment-state UI, Doctor Flutter and Web consultation actions, shared clinical history, authorization and duplicate safety. Existing repository/BLoC/Dio structure, financial state machine, database schema and queue ordering are retained.

## Authoritative contract

- `POST /appointments/book` requires an available Doctor slot for **both CHAT and VIDEO**. The server selects the configured modality price, locks the slot, and creates one `PENDING_PAYMENT` appointment and `INITIATED` transaction. Nonpositive configured prices mean that modality is unavailable.
- Payment is confirmed through the existing signed, timestamped, provider-matched webhook. Success transitions the transaction to `PAYMENT_HELD`, appointment to `CONFIRMED`, and slot to `BOOKED`. Neither client confirmation nor opening chat creates a payment success.
- The existing payment saga creates one consultation session and one conversation per appointment. Its 24-hour messaging window begins on payment activation. Existing VIDEO bookings include follow-up messaging; this contract has been preserved. A separate paid CHAT booking has its own appointment/conversation.
- `GET /appointments/:id/conversation` resolves the existing authorized conversation. Repeated opens do not create rows or charges. `GET/POST /chat/:id/messages` supply the same history to all clients.
- Patient lists use `/patients/me/appointments`, scoped to self or an authorized dependent. Doctor lists use `/doctor/appointments` and `/doctor/chats`. Patient display names come from the joined Patient profile, never a submitted `patient_name` field or a client-generated label.
- Row-level security and server authorization remain authoritative. Unrelated Patient/Doctor appointment, conversation and message requests return the existing non-identifying 404. Cancelled/refunded/unactivated consultations cannot send; expiry and explicit locking remain enforced on the server. Authorized read-only history is retained.
- RTC tokens require a VIDEO booking, an authorized participant, an allowed appointment status, and the existing server time gate. Concurrent starts resolve the same stored channel, including when the session was initially absent.

## Client changes

Patient Home and Services chat shortcuts now open a profile-scoped Active/Past consultation list, with a real booking entry. The directory can open with Chat selected. Existing discovery/profile modality choices lead to paid-chat details and the slot picker, rather than a future-feature notice.

After booking, the Patient sees the server's payment status, authoritative booked amount, an intake-record action and a consultation entry. Pending/failed/unknown checks never unlock access. Confirmed state permits resolving the existing chat or opening the video waiting room. Duplicate taps during booking and repeated submission after success are blocked. Past/cancelled conversations open read-only where server policy permits. Expired conversations move to Past using server expiry metadata.

Doctor Flutter Queue and Schedule show the authorized Patient name and modality-specific primary action. Queue and Chats now load all pages through the existing endpoints, preserving server ordering; a booking beyond the first 25/100 records remains accessible. Bounded offline caches retain the first page rather than being overwritten by the final historical page. Chat resolves the conversation directly, independent of inbox pagination. Chat-only threads do not display a video-call control. Expired/cancelled threads show history rather than an active start action. Open Doctor threads now refresh messages periodically, preserve failed text and close the composer after a server denial.

Doctor Web Overview, Appointments, Queue, Schedule and Patients use the same modality/state action helper. Patient Chats opens the same conversation and history, including direct resolution when the selected appointment is outside the first inbox page. Chat-only threads omit video actions. Existing reminder navigation uses the booking modality.

Patient chat expiry layout wraps deliberately; all four width checks retain readable text. Paid-chat copy no longer promises permanent retention or a guaranteed prescription. Patient localized presentation strings were audited to remove emoji characters. No synthetic success, payment, answer or parallel chat history was introduced.

## Test integrity

The pre-edit source snapshot includes pre-existing dirty files; comparing only `git diff` would incorrectly attribute earlier Patient work to this task. The snapshot is in ignored backend build output, with hashes in [test inventory](audit_evidence/patient_consultation/test_inventory_before.json). The historical `pre-patient-security-validated` tag remains unchanged at `eadf92f36c07ee77e100cc70736b8530b009b2d1`.

See [machine-readable assertion audit](audit_evidence/patient_consultation/assertion_preservation.json) and [exact task-local test diff](audit_evidence/patient_consultation/test_changes_from_task_snapshot.patch).

Existing test files changed:

| File | Changes and preserved evidence |
| --- | --- |
| `services/backend/tests/api_tests.rs` | Added three consultation cases. Repaired two stale legacy-auth setups: current consent supplied to concurrent first-time activation, and a well-formed negative OTP supplied after an already-denied Doctor-phone OTP request. Existing exact status, concurrency, session/profile, uniqueness and rollback assertions remain. Added explicit request-denial and no-Patient-profile checks. |
| `apps/mobile/test/doctor_widgets_test.dart` | Two responsive cases now provide real `CONFIRMED`/`VIDEO` booking metadata. Four exact visible-label expectations were updated for this requested UI contract; all widths, exact widget counts, overflow and Patient-linkage checks remain. Added opposite-modality absence checks. |
| `apps/mobile/test/notification_center_page_test.dart` | Fixed the clock at the original golden's 14:45. The two previously failing goldens differed only in the wall-clock refresh time. No golden image or pixel assertion was changed. |
| `services/backend/src/services/consultation_service.rs` | Production code changed; existing embedded unit-test assertions remain unchanged. |

The four intentional expectation updates are:

| Test | Old assertion target | New assertion target | Unchanged assertion |
| --- | --- | --- | --- |
| queue action toolbar stays on one line at common Android widths | `Call Patient` | `Start video consultation` | `findsOneWidget`, same width matrix |
| same queue case | `Chat` | `Follow-up messages` | `findsOneWidget`, same width matrix |
| same queue case | `Write Rx` | `Write prescription` | `findsOneWidget`, same width matrix |
| schedule shows human-readable time and real patient linkage, no overflow | `Call Patient` | `Start video consultation` | `findsOneWidget`, same widths, Patient identity and overflow checks |

New test files: Patient `consultation_booking_test.dart`, `consultation_flow_test.dart`; Doctor Flutter `consultation_actions_test.dart`, `consultation_pagination_test.dart`; Doctor Web `consultation-actions.test.ts`. No existing test was deleted or skipped. No security/business/concurrency assertion was weakened. The audit distinguishes these four documented UI expectation replacements from deleted assertions.

## Verification results

| Suite | Before | Final | Notes |
| --- | --- | --- | --- |
| Rust library | 53 inventoried | 53 passed | 0 failed/ignored/filtered |
| Backend integration | 211 inventoried | 214 passed | Corrected full serialized run: 0 failed/ignored/filtered; final output recovered on 2026-10-15 |
| Doctor Flutter | 172: 170 passed, 2 time-dependent golden failures | 180 passed | Exit 0 re-observed on 2026-10-15 after the Android launch fix; original goldens retained |
| Patient Flutter | 171 passed | 181 passed | 0 failed/skipped |
| Doctor Web | 44 passed | 47 passed | Typecheck/lint/build pass; 0 failed/skipped |
| Admin Web | 13 passed | 13 passed | Typecheck/build pass; lint exits 0 with 9 existing warnings |

All gates retain real process exit codes. Rust integration execution stays serialized as requested; no request-concurrency assertion was reduced. An earlier full-suite process used the pre-repair auth fixture and its failure is retained in the log. It is not presented as a passing run.

## Evidence

Live browser/API verification uses existing synthetic accounts, real password/development OTP authentication and signed **development** MPESA callbacks. It exercises pending-to-confirmed payment, correct Patient identity and separate modalities, Patient send, Web receive/reply, Doctor Flutter receive/reply and Patient Flutter reopen/history. See [live results](audit_evidence/patient_consultation/live_results.json). Test sessions stay in memory or short-lived ignored build output; they are not published in this report.

- [Doctor Web booking visibility](audit_evidence/patient_consultation/doctor_web_queue.png)
- [Doctor Web shared conversation](audit_evidence/patient_consultation/doctor_web_shared_chat.png)
- [Doctor Flutter shared conversation](audit_evidence/patient_consultation/doctor_flutter_shared_chat.png)
- [Patient Flutter shared conversation](audit_evidence/patient_consultation/patient_flutter_shared_chat.png)
- [Paid-chat details at 320dp](audit_evidence/patient_consultation/paid_chat_details_320.png)
- [Chat slot selection at 320dp](audit_evidence/patient_consultation/chat_slot_booking_320.png)
- [Pending payment at 320dp](audit_evidence/patient_consultation/chat_payment_pending_320.png)
- [Confirmed payment at 320dp](audit_evidence/patient_consultation/chat_payment_confirmed_320.png)

The full mocked-transport UI path is also exercised at 320/360/390/412dp, including selecting Chat, choosing a slot, submitting the exact CHAT/MPESA DTO, retaining pending lock, receiving authoritative confirmation, resolving a conversation and sending a message. Backend tests independently exercise the real handlers/database/payment and authorization; mocked UI fixtures are not represented as backend evidence.

Backend concurrency evidence uses 10 simultaneous booking requests and 8 simultaneous webhook retries, with exact appointment/transaction/session/conversation/event counts. Eight concurrent RTC starts from both participants assert one stored channel/session. Patient and Doctor ownership tests assert exact 404 and no identifying payloads; cancelled/expired sends assert exact 403.

`adb devices -l` reports no connected device. Flutter captures are rendered widget evidence; Web captures come from existing headless Edge. Neither is physical-device acceptance. No emulator was launched or host software installed.

## Remaining external limits

- **Live M-Pesa/provider verification is unconfigured.** Booking returns a deliberately unusable provider URL in development; production payment confirmation fails closed. The client never launches the `.invalid` link or invents payment success. Signed development callbacks prove the existing financial transition, not real money/provider acceptance.
- **Agora media integration is deferred in the existing clients/backend.** Video identity, modality, server time gating and shared-session behavior are tested. This report does not claim real video/audio transmission or production Agora-token compatibility.
- Existing notification types support consultation reminders, not a new immediate booking/message push event. This work preserves that domain and privacy-safe notification architecture. Open threads receive updates by polling; closed-process message push is not claimed.
- Azure email delivery is not configured locally. Device test accounts use the explicitly enabled development OTP mechanism.

## Independent statuses

| Status | Result |
| --- | --- |
| PATIENT_CHAT_BOOKING_IMPLEMENTED | IMPLEMENTED |
| PATIENT_CHAT_PAYMENT_ACTIVATION_VERIFIED | VERIFIED_SIGNED_DEVELOPMENT_CALLBACK_ONLY |
| DOCTOR_FLUTTER_PATIENT_CHAT_VISIBLE | VERIFIED_LIVE_BACKEND_RENDER |
| DOCTOR_WEB_PATIENT_CHAT_VISIBLE | VERIFIED_REAL_BROWSER |
| DOCTOR_PATIENT_NAME_MAPPING_VERIFIED | VERIFIED |
| DOCTOR_START_CHAT_CONSULTATION_VERIFIED | VERIFIED_EXISTING_CONVERSATION |
| PATIENT_DOCTOR_SHARED_CHAT_HISTORY_VERIFIED | VERIFIED_ALL_THREE_CLIENTS |
| CHAT_DUPLICATE_ACTION_SAFE | VERIFIED_CONCURRENT_REQUESTS |
| CHAT_BOLA_IDOR_VERIFIED | VERIFIED_REAL_HANDLER_AND_DATABASE |
| VIDEO_PATIENT_NAME_MAPPING_VERIFIED | VERIFIED |
| VIDEO_START_ACTION_VERIFIED | VERIFIED_MODALITY_AND_SERVER_GATE; LIVE_MEDIA_PENDING |
| PATIENT_CHAT_PHYSICAL_DEVICE_VERIFIED | PENDING_NO_CONNECTED_DEVICE |

Integrity: `EXISTING_TESTS_DELETED=0`, `EXISTING_ASSERTIONS_DELETED=0`, `TEST_ASSERTIONS_WEAKENED=0`, `TESTS_SKIPPED_TO_FORCE_GREEN=0`, `TEST_RUNNER_FAILURES_SWALLOWED=0`. Four exact presentation-label expectations were deliberately updated and listed above; no unrelated assertion was changed.

## Device test artifacts

[Patient ARM64 APK](../releases/hellodoctor-patient-arm64-consultations-20261014.apk): version 1.0.1+3, 20,889,612 bytes (19.9 MiB), SHA-256 `e9d80e2ceacdaf4804ca49f32a994497f36cb22cb782bc6a169ff6d40ac85f3b`. It targets `http://192.168.0.105:8080/api/v1` on the same local network with the backend running, development OTP explicitly enabled. These are development test artifacts, not production release approval. See the [test guide and credentials](../releases/PATIENT_CONSULTATION_TESTING_20261014.md).

The Doctor native build initially failed because WorkManager 0.5.2 references removed Flutter Android embedding APIs. It was updated to the already-cached 0.6.0 used by Patient, with the existing scheduling/notification APIs retained. A direct `fake_async` development dependency also records the package already used by existing Doctor tests. No host software was installed. Physical background-task/notification acceptance remains pending.
