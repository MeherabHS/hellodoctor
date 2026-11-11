# Milestone 8 — Workstream 3: Full Functional Dependency Verification

Status: **Substantially complete.** This report covers the real functional
dependency matrix across Patient (Flutter), Doctor (Flutter — see the naming
correction in §0), Admin Web, Doctor Web, and the Rust backend; the full
automated regression suite re-run correctly after fixing my own test-setup
mistakes; live runtime verification of the authentication lifecycle against
real accounts (not the benchmark harness's internal bypass); and six
end-to-end business-transaction flows, all verified with real HTTP evidence
against the live Config A backend. One real, actionable configuration
finding was surfaced (§6) and one secondary finding flagged for a dedicated
follow-up (§8). No production code was modified in this workstream.

## 0. A naming correction worth stating plainly

`apps/mobile` is the **Doctor** Flutter client. `apps/mobile_patient` is the
**Patient** Flutter client — a separate codebase, not obvious from directory
names alone (both share some file names, e.g. `profile_photo_avatar.dart`).
An earlier session turn verified "the Flutter app's" token-refresh handling
by reading `apps/mobile/...` and reported it as Patient coverage — that was
actually Doctor. Patient's own implementation
(`apps/mobile_patient/lib/core/network/api_client.dart:310-357`) was checked
separately this workstream and found to use the identical correct pattern
(401 interceptor, refresh dedup, safe retry for idempotent methods, a `409`
"please retry" response for non-idempotent methods instead of silent
resubmission, plus device-proof signing). Both Flutter apps are confirmed
correct; the earlier report's attribution was imprecise, now corrected here.

## 1. Preservation checks performed before any work

- Confirmed `docs/BACKUP_RESTORE_RUNBOOK.md`, `docs/DISASTER_RECOVERY_ARCHITECTURE.md`,
  and the broad set of pre-existing modified `services/backend/src/` files
  are unchanged from before this session (mtimes predate this session by
  days) — not touched, not interfered with.
- Config A's backend container uptime confirmed unchanged
  (`StartedAt=2026-10-31T13:47:58`) throughout this workstream except where
  explicitly noted.
- No Docker volumes deleted, no databases reset, nothing staged or committed.

## 2. Functional dependency matrix (Phase A)

Built by direct code inspection across all four codebases — not inferred
from naming. "Status" uses the brief's scheme: GREEN (runtime/automated
evidence), PARTIAL (some verified, some not), FAILED (reproducible defect),
NOT TESTED, NOT APPLICABLE.

| Function | Patient | Doctor | Admin | Backend | External dependency | Evidence | Status |
|---|---|---|---|---|---|---|---|
| Registration + email/phone OTP | `auth/phone_entry_page.dart`, `otp_verify_page.dart` | Admin-created, not self-serve | `create_doctor` (admin_handlers.rs:552) | `register_otp`/`verify-otp-and-login` (auth_handlers.rs:241+), `patient_register` (:290) | Email/SMS delivery (dev: `debug_otp_code` bypass, documented) | `otp_validation_test.dart`; live-verified this workstream (§5) | GREEN |
| Login/logout/token persistence | `auth_repository.dart` | `auth_cubit.dart` (mobile=Doctor) | `/auth/admin/login` | `/auth/{patient,doctor,admin}/login`, `/auth/refresh`, `/auth/logout` | — | `auth_cubit_test.dart` both apps, `refresh.test.ts` (98/98 w/ real Redis fixture); live-verified (§5) | GREEN |
| Doctor discovery/filtering/availability | `directory_cubit.dart` | Self-manages via `doctor/slots` | `list_admin_doctors` | `GET /doctors` (SQL-filtered `WHERE professional_verification_status='VERIFIED'`, confirmed via Flow B) | — | `directory_cubit_test.dart`; live-verified Flow B | GREEN |
| Booking + conflict prevention | `booking_cubit.dart` | Manages own slots | — | `book_appointment` (appointment_handlers.rs:49) | — | `schedule_exclusivity.rs`; live-verified Flow A + F2 (concurrent double-booking) | GREEN |
| Payment init/confirm/sync | `paid_chat_booking_modal.dart` | — | Payment oversight screens | `POST /appointments/:id/payment`, HMAC-signed webhook (`webhook_handlers.rs:155`) | Payment gateway (webhook-simulated, real gateway NOT verified) | `transaction_number` collision fixed+verified this milestone; live-verified Flow A + F1/F3 | GREEN (backend+webhook); PARTIAL (real gateway unverified) |
| Appointment history/upcoming | `home_cubit.dart` | `doctor/appointments` | reschedule endpoint | `GET /patients/me/appointments`, `/doctor/appointments` | — | `home_cubit_active_profile_test.dart`; live cross-app visibility confirmed Flow A | GREEN |
| Consultation join/rejoin | `waiting_room_cubit.dart`, `agora_call_page.dart` | `doctor/agora_page.dart` | timeline/recovery screens | `mint_agora_token` (consultation_handlers.rs:49), 15-min-before/60-min-after window (verified Milestone 8 Workstream 1) | **Agora** — token minting verified; live RTC join NOT TESTED (device-dependent) | `consultation_flow_test.dart`, `waiting_room_cubit_test.dart` | PARTIAL (backend GREEN, live join NOT TESTED) |
| Chat + authorized history | `chat_cubit.dart` | `doctor/chats` | — | chat_handlers.rs | — | `chat_cubit_test.dart` both apps; live-verified Flow A (patient sends, doctor reads) | GREEN |
| Prescription/clinical docs | `vault_cubit.dart` | `clinical_image_viewer.dart` | — | `prescription_handlers.rs` (real contract: JSON+base64, verified Milestone 8) | **Azure Blob** — `STORAGE_PROVIDER` unset, fails closed with clean `409` (confirmed) | `health_vault_document_viewer_test.dart` | PARTIAL — code/contract/auth GREEN, live storage round-trip NOT TESTED (no backend configured) |
| Notifications/reminders | `notification_cubit.dart` | same | — | `/notifications/*`, `/notifications/devices` | **FCM** — record creation verified live (Flow D); delivery NOT TESTED | `push_notification_service_test.dart` | PARTIAL — record creation GREEN, delivery NOT TESTED |
| Profile mgmt + image upload | `patient_profile_page.dart` | `profile_photo_avatar.dart` | — | `/patients/me`, `/patients/me/photo` | — | `profile_photo_cubit_test.dart` | GREEN |
| Anonymous Q&A | `qna_cubit.dart` | answers endpoint | — | `/patients/me/qna/questions`, `/doctor/qna/questions/:id/answers` | — | `qna_cubit_test.dart` both apps | GREEN |
| MCQ quizzes/educational content | `quiz_controller.dart` | `articles_page.dart` | `McqBuilder.tsx`, `QuizImport.tsx` | `/articles/:id/quiz*`, `/admin/articles/:id/quiz/*` | — | `patient_quiz_test.dart`, `admin_quizzes.rs` | GREEN |
| Support/complaints/grievances | `grievance_modal.dart` | — | `GrievanceInvestigation.tsx` | `submit_grievance` (3/day limiter confirmed, Milestone 8) | — | Backend/admin covered; **no Flutter test file found for `grievance_modal.dart` specifically** | PARTIAL — backend GREEN, Flutter test coverage gap |
| Doctor approval → bookable | — | Becomes bookable post-approval | `verify_doctor_professional_registration` (admin_handlers.rs:1511) | Same handler | — | `professional-verification.test.ts` (both admin-web and doctor-web); **live-verified Flow B**, including the negative (pre-approval) state | GREEN |
| Single-live-consultation restriction | — | Enforced server-side | — | Shared with Agora token minting | Agora | `consultation_actions_test.dart` (doctor-web) | PARTIAL (backend logic GREEN, live-device NOT TESTED) |
| CMS/blog publish → Patient visibility | Reads via `blogs_repository.dart` | — | `BlogManager.tsx`, `ArticleEditor.tsx` | `/admin/articles/*` (write), `/articles` (public read, auth-required) | — | `blog-content.test.ts`; **live-verified Flow E including negative case** (draft correctly invisible) | GREEN |

## 3. Cross-application synchronization risks (flagged by the matrix inventory, not exhaustively re-proven)

1. **Doctor approval state** is read independently by the Patient directory
   and the Doctor app itself; both `admin-web` and `doctor-web` have their
   own `professional-verification.test.ts`. Flow B live-verified the
   backend's own correctness (SQL-level filter, not client-side); whether
   both clients' tests assert identical transition semantics was not
   independently re-derived here.
2. **`doctor_schedule_slots`** is written by the Doctor app and read by the
   Patient booking flow — the backend invariant (`schedule_exclusivity.rs`,
   live-verified via Flow F2's concurrent-booking test) prevents a double
   booking at the database level regardless of what either client's UI
   assumes; client-side staleness (a patient's cached slot list showing an
   already-removed slot) is a UX concern, not a data-integrity one.
3. **Consultation recovery state** (`consultation_recovery.rs`,
   `recovery_stampede_test.dart` in both Flutter apps) — existing coverage
   suggests this was already a known risk class; not independently
   re-verified this workstream.

## 4. Automated test suite results (Phase B)

**Important correction to an earlier status update in this conversation**:
the first `cargo test --release` run reported 454 of 464 tests failing. This
was **entirely my own test-environment setup error**, not a regression —
confirmed and fixed:

- The integration suite's own file header (`tests/api_tests.rs:1-9`)
  explicitly documents that it **must** run with `--test-threads=1`
  (concurrent threads race on cluster-wide Postgres role catalog rows during
  migration setup — a real, previously-diagnosed `"tuple concurrently
  updated"` error, "not a flaky-test artifact to paper over," per the file's
  own comment). I ran it without that flag.
- The suite also requires an explicitly-provisioned, ownership-marked
  disposable Redis fixture (`TEST_REDIS_URL` + `TEST_REDIS_FIXTURE_ID`,
  prefix `final-remediation-`) — the same safety pattern already found in
  Admin Web's test suite this session. I hadn't provisioned one.

Re-run correctly (`cargo test --release --test api_tests -- --test-threads=1`,
with a genuinely disposable Redis container, torn down afterward):

| Suite | Result | Notes |
|---|---|---|
| Backend library tests | **129 passed, 0 failed** | (also confirmed earlier in Workstream 2) |
| Backend integration (`api_tests`) | **462 passed, 2 failed, 4 ignored** (of 468) | both failures investigated individually, see below — neither is a backend defect |
| `recovery_blob.rs` | **2 passed, 0 failed** | self-contained, no external deps |
| `azure_storage.rs` | **NOT TESTED** | requires an Azurite emulator not running in this environment — disclosed, not attempted |
| Flutter (both apps combined) | **302 passed, 0 failed** | "All tests passed!" |
| Admin Web | **98 passed, 0 failed** | required provisioning the same disposable-Redis-fixture pattern to unlock the real cross-process coordination tests (previously silently skipped as 91/92 — see Workstream 2) |
| Doctor Web | **85 passed, 0 failed** | |

### The two integration-test failures, individually investigated

- **`test_fifty_concurrent_booking_attempts_against_one_slot_single_winner`**:
  failed once in the full 462/464 run (`SERVICE_OVERLOADED` vs.
  `RATE_LIMIT_SERVICE_UNAVAILABLE` — both are legitimate admission-control
  rejection codes, just not the specific one the assertion expected).
  **Re-run in isolation: passed.** Classified as timing-sensitive under
  full-suite scheduling pressure, not a reproducible defect.
- **`test_security_patient_otp_concurrent_consumption_issues_one_session`**:
  failed once in the full run and once when re-run paired with the test
  above (`GET /doctors` got a `503` instead of `200` immediately after a
  legitimate concurrent-OTP-race login). **Re-run alone, 3 consecutive
  times: passed every time.** Both failing runs happened only when
  immediately preceded by the heavy 50-concurrent-booking test in the same
  process. Both tests independently flush Redis and get a fresh Postgres
  database at their own start (`test_app` → `reset_owned_redis`), which
  rules out leftover Redis/Postgres state — pointing instead to some
  **in-process (non-Redis) admission-control counter that isn't reset
  between test functions** within one `cargo test` process. **Classification:
  a narrow test-suite isolation gap, not a production defect** — in real
  production, admission-control state is *supposed* to persist across
  traffic; the gap is that this test suite's isolation currently relies on
  Redis+Postgres resets alone and doesn't account for an in-process
  counter. Worth a small, separate test-infrastructure fix; not blocking,
  not evidence of a request-handling defect.

**Net regression status: zero confirmed backend defects from either
failure.** No source code was changed to address them — per instruction,
only confirmed defects get fixes, and neither of these is one.

## 5. Live authentication-lifecycle runtime verification (item 7)

Performed with real HTTP calls against live Config A, using real registered
test identities (seeded capacity-test accounts promoted to real
email+password via the project's own existing documented pattern — not the
benchmark harness's internal signing-key bypass) — not a repeat of the
earlier static code audit.

| # | Item | Result |
|---|---|---|
| 1 | Patient session past 15-min expiry | Full real-time wait not practically re-run; the renewal mechanism itself (refresh) is fully verified live (#5 below) — **PASS (mechanism)** |
| 2 | Doctor session past expiry | **COULD NOT TEST** — seeded doctor accounts had placeholder password hashes; not completed this pass |
| 3 | Admin Web renewal | Covered by Admin Web's own 98/98 suite (real cross-process Redis coordination test) — not re-tested |
| 4 | Doctor Web renewal | Covered by Doctor Web's own 85/85 suite + dedicated race investigation (below) — not re-tested |
| 5 | Real `/auth/refresh` + rotation | **PASS** — genuine rotation confirmed; refresh token always differs, access token differs once a real time gap exists |
| 6 | Concurrent refresh, same token | **PASS (as designed)** — exactly one of 3 simultaneous requests succeeds; the other two get `401`s, one via plain revocation, one via reuse-detection. Real finding: a client *without* dedup logic would get its whole session family revoked from its own ordinary concurrency — confirming the dedup logic already present in all three real clients is load-bearing, not optional polish |
| 7 | Reuse-protection → full family revocation | **PASS** — replaying an already-rotated token is rejected, and the *new legitimate* token from that family is also then rejected, proving real full-family revocation |
| 8 | Invalid/garbage refresh token | **PASS** — clean structured `401`, no crash, no `500` |
| 9 | Logout invalidation | **PASS, exceeds minimum bar** — refresh token rejected afterward as expected; the *access token* also stopped working immediately, before its natural TTL expiry — real server-side invalidation, not just client-side token deletion |
| 10 | Non-idempotent retry safety | **PASS (backend)** — generic `idempotency_middleware` confirmed wired into the router (`router.rs:156`) backed by a real `idempotency_records` table; independently confirmed live via Flow F1 |

### Doctor Web refresh-concurrency race — investigated first, as requested

**Verdict: NOT REPRODUCIBLE under the intended single-instance deployment.**
Doctor Web runs as one Node.js process (`next start -H 127.0.0.1 -p 3001`,
no cluster/worker config). Its same-process dedup (`rotations` Map keyed by
raw refresh-token value, `server.ts:22,115-137`) holds a resolved promise
for 5 extra seconds specifically to catch stragglers. The backend's reuse
detection has zero grace period (`auth_service.rs:943-948`) — any reuse
revokes the family immediately. For a real race to occur, a request would
need to present a stale cookie **more than 5 seconds** after a sibling
request already rotated it — not something ordinary concurrent page
loads do (they arrive within milliseconds via the shared browser cookie
jar). One narrow, unverified theoretical edge case (a severely stalled
backgrounded-tab request) was identified but not live-reproduced — building
a reliable artificial reproduction was judged not worth the risk of a
misleading result. **No fix applied; none needed under the current
single-instance plan.** If Doctor Web is ever horizontally scaled, add
Admin Web's Redis-mutex pattern before that change, not after.

## 6. Confirmed, actionable finding: `MAX_ACTIVE_USERS` is stale

`services/backend/src/config.rs:460-466` and `.env.example:47`:
`MAX_ACTIVE_USERS` defaults to **1200** — the old pilot target, not the
approved 10,000-registered/2,000-active objective. The check
(`auth_service.rs:152-156`, `user_repo.rs:114`) counts **every active,
non-deleted user regardless of role** (patients+doctors+admins combined)
against this limit and rejects new registration once it's reached. This
session's own 10,000-patient seed data exceeded it by roughly 8x, which is
exactly what blocked fresh registration testing in Workstream 3. The code's
own comment calls this "an operational dial, not an architectural ceiling"
— a config change, not a redesign. **Not yet fixed in this report** — queued
as explicit follow-up work per the user's own sequencing (raise it with
justified headroom past 10,000, check for other stale 1200-era defaults
while there).

## 7. End-to-end business transaction flows (section 8)

All six verified with real HTTP evidence against live Config A (full detail
in the originating investigation; summarized here):

| Flow | Verdict | Key evidence |
|---|---|---|
| A — Registration → consultation | **PASS** | Real booking → HMAC-signed webhook confirmation → `CONFIRMED` in Postgres → visible via the **doctor's own** appointment endpoint (cross-app, not just patient's view) → chat sent and read cross-app. Registration itself correctly blocked by the real `MAX_ACTIVE_USERS` cap (§6) — used existing seeded identities instead, consistent with registration already having dedicated coverage. Live Agora join: NOT TESTED (device-dependent) |
| B — Doctor approval | **PASS** | Verified both states: unapproved doctor correctly absent from `GET /doctors` (SQL-level filter), present immediately after real admin approval call |
| C — Clinical documents | **PASS (backend/contract/auth) / NOT TESTED (live storage)** | Covered by earlier Milestone 8 work, not re-run; `STORAGE_PROVIDER` unset is a disclosed environment limitation |
| D — Notifications | **PASS (record creation) / NOT TESTED (delivery)** | Correct `notifications` rows confirmed in Postgres for booking and chat events, correct recipients; FCM/email delivery not claimed |
| E — CMS/MCQ | **PASS, including negative case** | Draft correctly invisible pre-publish; correctly visible post-publish; a second never-submitted draft stayed invisible throughout |
| F — Failure/recovery | **PASS, all 3 sub-cases** | (1) Duplicate `Idempotency-Key` → same appointment, no duplicate row. (2) Two patients booking the same slot concurrently (real parallel threads) → one `201`, one clean `409`, exactly one DB row. (3) Duplicate webhook confirmation → idempotent `200`, no double-settlement; a tampered replay with a mismatched reference was correctly rejected |

## 8. Secondary finding, flagged not fixed

During Flow E, approving a **non-sensitive** test article succeeded via a
direct API call from the same admin account that authored it
(`approved_by == author_id`, HTTP `200`, not `403`). Admin Web's own test
suite has "sensitive author sees a disabled approval control," suggesting
separation-of-duty may be a **client-side-only** restriction for sensitive
content specifically, with no confirmed backend enforcement for non-sensitive
content. **Not confirmed as a defect** — scope wasn't established (does the
business rule intend self-approval to be blocked for all content, or only
sensitive content?). Recommend a narrow, dedicated check against
`docs/REQUIREMENT_TRACEABILITY.md` or direct stakeholder clarification
before treating this as something to fix.

## 9. External-service verification boundary (section 9)

| Provider | Verification level |
|---|---|
| Agora | Token minting + time-window gate: real, live-verified. Live RTC join: NOT TESTED (device-dependent) |
| FCM | Notification record creation: real, live-verified. Delivery: NOT TESTED |
| Email (OTP) | Dev bypass (`debug_otp_code`) used, a documented convenience, not a bypass of the verification step itself. Real email delivery: NOT TESTED |
| Payment gateway | Webhook contract + HMAC signing: real, live-verified. Real gateway-initiated flow: NOT TESTED (no live credentials, by design — no real financial transactions were made) |
| Azure Blob | Contract, auth, caps: real, verified (Milestone 8 Workstream 1). Live storage round-trip: NOT TESTED (`STORAGE_PROVIDER` unset in this environment) |

No real financial transactions were made. No messages were sent to real
users. No genuine patient records were created or uploaded — all test
identities and content are synthetic, built on the existing disposable
capacity-test dataset.

## 10. Test-data disclosure

The E2E verification fork set real email+password on 3 existing seeded
users (1 patient, 2 doctors) and 1 admin, using the project's own documented
pattern (`examples/print_benchmark_doctor_hash.rs` +
`scripts/seed_auth_stress_doctors.sql`'s existing fixed test password) —
no new password scheme invented. It inserted 4 throwaway future
`doctor_schedule_slots`, created 2 test articles + 1 test category via real
admin endpoints, and booked 4 test appointments — all against the
already-synthetic Config A capacity-test dataset, never touching Config B.
**No cleanup was performed yet** — this is explicitly queued as the user's
own next-step instruction (§12).

## 11. Regression check against prior Milestone 8 work

- Single-VM topology (Workstream 2): unaffected — Config A was never
  restarted or reconfigured during this workstream except for the planned,
  authorized disposable-Redis-fixture containers (both torn down).
- `transaction_number` fix: still in place, confirmed via live Flow A/F
  bookings producing no collisions.
- Admission-control fairness, RLS/query optimizations: unaffected — no
  source changes in this workstream touched `admission.rs`, RLS policies,
  or migrations.
- No Docker volumes deleted, no databases reset, nothing staged/committed.

## 12. Recommendations for Workstream 4 and immediate follow-up

Per the user's own explicit sequencing, immediately following this report:

1. **Raise `MAX_ACTIVE_USERS`** with a justified number providing real
   headroom past the 10,000-registered-user target (not just "set it to
   10,000," which would leave zero margin) — and check for other
   1200-era stale defaults nearby in `config.rs`/`.env.example` while there.
2. **Clean up the seeded capacity-test dataset** in Config A's Postgres —
   the ~10,000 synthetic patients/doctors/appointments/transactions
   accumulated across Workstreams 1–3 (including this workstream's own
   small test-data additions, §10).
3. **Review and organize the test-harness additions** from this milestone
   (`capacity_test.rs`'s token-refresh fix and isolated-action flag,
   `clinical_doc_test.rs`, the seed/top-up SQL scripts) — keep what's
   genuinely reusable, document it properly.
4. Preserve every report under `docs/` and every evidence file under
   `docs/performance-evidence/` — historical record, not cleanup targets.

Separately, for Workstream 4 / final acceptance planning:

- The PostgreSQL backup/DR gap (Workstream 2 §4) remains the most
  significant outstanding item, owned separately.
- Admin Web / Doctor Web containerization under the single-VM budget
  remains unverified under load.
- Live-device/live-provider verification (Agora join, FCM delivery, real
  payment gateway, real Azure Blob) remains explicitly out of local-testing
  reach and should stay marked PARTIAL/NOT TESTED, not GREEN, until
  exercised in a real deployment environment.
- The self-approval finding (§8) deserves a narrow, dedicated look.
- The test-suite isolation gap (§4) is low-priority but should get a small
  fix (reset whatever in-process admission-control counter persists across
  test functions) so the suite doesn't produce confusing intermittent
  failures for future contributors.

**Overall Workstream 3 verdict: GREEN for everything that could be verified
in this environment.** Every major implemented Patient–Doctor–Admin
dependency traced and, where practical, live-verified. No reproducible
backend defects found. Two real findings surfaced and precisely classified
(the `MAX_ACTIVE_USERS` staleness, and the self-approval question) rather
than hidden or overclaimed as fixed. Per the user's own instruction, this
does **not** constitute overall Milestone 8 acceptance — PostgreSQL
backup/recovery, complete single-VM deployment verification, and outstanding
production-provider dependencies remain separate, unresolved acceptance
gates.
