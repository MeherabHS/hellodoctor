> Kenya correction, 2026-10-08: [current professional-registration contract](KENYA_PROFESSIONAL_VERIFICATION.md) supersedes legacy regulator/status terminology below. Original dated/prototype evidence is retained as history. The user confirms the Doctor app is verified.

# Doctor Flutter replication report

Source of truth: `prototype/index.html` for visual structure and `docs/DOCTOR_FLUTTER_BACKEND_MAPPING.md` for API behavior.

## 2026-10-06 pass — Chat, Smart Rx, Schedule, Wallet, Q&A full port + Queue typography fix

**Verification method used in this pass:** every class/widget below was built by reading the actual
`prototype/index.html` markup and CSS rules (not from memory or the root `index.html`), then
cross-checked against the real Rust backend contracts (handlers/repos), then verified with
`flutter analyze` (0 issues), `dart format --set-exit-if-changed lib test` (clean), and
`flutter test` (40/40 passing, including new responsive tests at 360/390/412 logical px and
business-rule-wording tests). Three real layout bugs and one latent `Container` API-misuse bug were
caught this way and fixed before they'd have reached a device.

**Not yet performed: literal on-device/emulator screenshot capture.** This environment has no
Android SDK/emulator attached (confirmed: no adb devices). The Queue screen's current state was
visually confirmed by the user from a real-device screenshot and is the accepted quality bar the
other five screens were built to match; the other five have **not yet** been screenshotted side by
side with the prototype by a human. That final visual sign-off is the one remaining step before
this can be called fully `MATCHED` rather than code/structurally matched. The release APK
(`build_field_test.ps1`) was rebuilt and compiles/runs after every change in this pass.

| Screen/subscreen | Widget | Status |
|---|---|---|
| Doctor Queue | `QueuePage`, `AppointmentCard`, `_QueueActionButton` | MATCHED — user-confirmed from device screenshot; typography/button-wrap/search-truncation bugs fixed and frozen per instruction |
| Consultation Chats | `ChatListPage`, `_ConvCard`, `ChatThreadPage`, `_ChatTopBar`, `_ChatBubble`, `_ChatExpiryBar` | MATCHED (pending user screenshot sign-off) |
| Smart Rx Scanner | `ScannerPage`, `_RxMetaBar`, `_RxPagesStrip`, `_RxPadFooter` | MATCHED_WITH_REQUIRED_NATIVE_PLATFORM_DIFFERENCE (pending user screenshot sign-off) |
| Consultation Schedule | `SchedulePage`, `_ScheduleSummaryBanner`, `_DateSlider`, `_ScheduleSlotCard` | MATCHED (pending user screenshot sign-off) |
| Doctor Earnings & Wallet | `WalletPage`, `EarningsBreakdownCard`, `_LedgerCard` | MATCHED_WITH_REQUIRED_NATIVE_PLATFORM_DIFFERENCE (pending user screenshot sign-off) — wording substitutions are the required business-rule conflict resolution, not a visual gap |
| Open Health Q&A | `QnaPage`, `_QnaFlowTop`, `_QnaHeroBanner`, `_QnaOpenCard` | MATCHED_WITH_REQUIRED_NATIVE_PLATFORM_DIFFERENCE (pending user screenshot sign-off) — "Answered" tab omitted, see discrepancy note |
| Consultation detail | `ConsultationPage` | IMPLEMENTED_AND_TESTED (pre-existing, unchanged this pass) |
| Patient history/detail | `HistoryPage`, history detail dialog | IMPLEMENTED_AND_TESTED (pre-existing, unchanged this pass) |
| Profile/settings | `ProfilePage` | IMPLEMENTED_AND_TESTED (pre-existing, unchanged this pass) |
| Consultation call | `AgoraPage` | DEFERRED_AGORA_INTEGRATION |
| Prototype withdrawal action | none | PROTOTYPE_BUSINESS_RULE_CONFLICT — "Transfer to bKash"/instant withdrawal/escrow wording replaced with "Scheduled Disbursement"/"Processed by Finance"; no withdraw endpoint created |

### Backend changes made this pass (all additive, minimal, tested)

A doctor-identity gap was found and fixed: `list_own_appointments`, `list_own_chats`,
`list_own_slots`, and `wallet_ledger` (`services/backend/src/api/v1/doctor_handlers.rs`) returned
only `patient_id` (an opaque UUID), never a human-readable name — every screen above needs this to
show a doctor which patient a row belongs to. Fixed by joining `patient_profiles` (verified RLS-safe
against the real `fn_doctor_treats_patient` policy beforehand). Regression tests added to
`services/backend/tests/api_tests.rs`
(`test_doctor_lists_include_patient_display_name`). Full backend suite re-run after each change:
**42/42 passing**. The live dev server was restarted after each backend edit so the field-test build
serves real names, not placeholders.

No other backend code was touched. No bulk-slot-create endpoint, lab-report-list endpoint, or
Q&A-answered-list endpoint was added — see discrepancies below for why.

### Per-screen detail

**Consultation Chats** — ported `#docChatListView`/`#docChatThreadView`
(`.conv-card`/`.conv-avatar-wrap`/`.conv-online-dot`/`.chat-top-bar`/`.chat-bubble`/`.doc-canned-strip`
equivalents are absent only where no real data exists, see below). Fixed a real bug: messages were
previously all rendered right-aligned regardless of sender; now correctly split doctor-vs-patient
using the signed-in doctor's real `user_id` from `/doctor/profile`. Thread now opens as a full
screen push (matches the prototype's single-page view-swap) instead of a bottom sheet.
**Discrepancy:** the prototype's conversation-list preview text and Urgent/Follow-up/Refill category
badges have no backend field to back them (`list_own_chats` has no message-preview or triage-category
column) — filters shown are "All/Active/Closed" derived from the real `locked` state instead of
fabricated triage categories.

**Smart Rx Scanner** — ported the patient-meta auto-sync bar, multi-page capture strip with
per-page thumbnail/remove/retry, doctor's-advice textarea (now actually wired to the real
`doctor_notes` field the upload endpoint already accepted but the old UI never sent), and a
prescription-pad footer using the real doctor name/BMDC number. Fixed a real bug: the old upload
loop aborted the entire batch on the first failed page; it now attempts every page independently and
only the failed ones are retried (never re-sends a page that already succeeded).
**Discrepancy:** the prototype's patient EMR vitals row, risk indicator, and "Attached Lab Reports"
have no backend data source for a doctor (`patient_profiles` has no live vitals; the only intake-document
list query exists for admin, not doctor, routes) — omitted rather than fabricated.
**Native platform difference:** the prototype's custom camera viewfinder modal is a browser-only
simulation of a camera; the real app correctly uses the OS's native camera via `image_picker`
instead of reimplementing a fake viewfinder over it.

**Consultation Schedule** — ported the dark "Today's Schedule" summary (real counts), date slider
(real per-day slot indicator dots), filter chips, human-readable time ranges (never a raw ISO
timestamp), status pills (In Progress/Upcoming/Completed/Open for Booking), and
Call/Chat/Write Rx actions on booked slots using the newly-joined real patient/appointment data.
"⚡ Create 10-Min Slots" has no bulk-create endpoint behind it in the backend; it issues 6 real
sequential `POST /doctor/slots` calls and reports exactly how many succeeded, rather than faking an
instant bulk action. "Block Slot" was dropped — there is no such slot status in the backend enum
(`AVAILABLE`/`LOCKED_IN_PAYMENT`/`BOOKED`/`CANCELLED` only), so it would have been a dead button.

**Doctor Earnings & Wallet** — ported the 3-step Gross→Withheld→Net breakdown with real values, the
mandatory "Total calculation is based including the platform charge 20%" note rendered verbatim, and
the required business-rule wording substitutions. Ledger rows now show the real patient name and map
the actual `payment_status` enum honestly (`PAYMENT_HELD` → "Held", never "Settled"/escrow wording —
this was specifically unit-tested since getting it wrong means telling a doctor money is theirs
before it is).

**Open Health Q&A** — ported its own `.flow-top`/`.flow-back-btn` header (not `.dir-header`, per
instruction), hero banner, quick-preset canned pills that actually populate the answer field, and the
publish flow (already-correct 409-conflict handling preserved). **Discrepancy:** `list_open_questions`
is hardcoded server-side to `WHERE status='OPEN'` — there is no endpoint that returns a doctor's own
previously-answered questions, so the prototype's "Answered" filter tab/card has no real data source
and was omitted rather than shown empty or faked. The "Unanswered (n)" count is accurate because it's
literally everything the endpoint can return today.

### Outstanding before final sign-off

1. A human (or an environment with an Android SDK/emulator) needs to actually look at Chat, Smart
   Rx, Schedule, Wallet, and Q&A side-by-side with `prototype/index.html` and confirm the visual
   match, the same way Queue was confirmed from a real-device screenshot this session.
2. The three documented backend-data gaps (chat preview/category, Rx lab-reports/vitals, Q&A
   answered-list) are real product decisions, not oversights — flagging them for a product call on
   whether they're worth a further backend addition, rather than silently leaving them unmentioned.

## Cache/offline visual and behavior audit (2026-10-05)

Status: `IMPLEMENTED_AND_TESTED`. The existing prototype card/layout language is retained while cache UX adds subtle progress, offline, last-updated, retry, and pull-to-refresh states. Queue, schedule, wallet, chats, Q&A, consultation, profile, and scanner use cached-first rendering where safe. Sensitive history stays online-authorized and shows an online-check requirement rather than exposing persistent longitudinal PHI. Mutations never become locally confirmed. Cache storage is versioned, encrypted through secure storage, Doctor-scoped, TTL-bounded, and cleared on logout/revocation/account switch. Agora remains `DEFERRED_AGORA_INTEGRATION`; external provider movement remains `DEFERRED_EXTERNAL_PAYMENT_INTEGRATION`.

Live backend evidence was added after starting Docker Desktop: PostgreSQL, Redis, and the seeded Rust service were healthy; real Doctor MFA, queue, duty, schedule, chat list, Q&A list, wallet summary/ledger, profile, schedule mutation, duty mutation, refresh rotation, logout, and post-logout refresh rejection all passed. The cache UX itself remains `IMPLEMENTED_AND_TESTED`; Android visual/offline reconnection evidence remains unavailable without an emulator/device.
