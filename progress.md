# HelloDoctor / AfyaMkononi — Work Log

This file is a durable record of completed work across the session's tasks.
For the *exact* point to resume from right now, read `checkpoint.md` instead —
this file is context/history, not a to-do list.

## Task 1 — Patient APK "offline" bug investigation (COMPLETE)
Investigation-only (no code changes), delivered an 8-point root-cause report.
No further action pending from this task.

## Task 2 — 6-issue E2E remediation (COMPLETE)
Fixed: Patient slot booking UI, expired-slot bookability (found NOT reproducible,
verified via existing test), doctor profile photo not showing for patients,
redundant Patient home search bar removed, admin self-approval + FCM campaign
push failure, 24h→12h AM/PM time display (Doctor Web only; deliberately did NOT
switch Admin Web's `time()` to device-local timezone — preserved the existing
fixed Africa/Nairobi wall-clock invariant, flagged as a judgment call).
Tests run and green. No merge/push/deploy was performed (per constraints).

## Task 3 — Doctor Web real Agora video consultation + Issue 7 (spinner) (COMPLETE)
Replaced placeholder/deferred Telehealth with real appointment-driven Agora RTC
consultation (`agora-rtc-sdk-ng@4.24.9`), reusing existing backend/mobile
contracts exactly — no duplicate architecture, no custom WebRTC/SFU.
Also fixed Doctor Web's malformed/stretched loading spinner (root-caused, fixed
proportions, added prefers-reduced-motion support, regression tests added).
Tests run and green. No merge/push/deploy was performed.

## Task 4 — FINANCIAL_MODULE_MODE=deferred|enabled (IN PROGRESS — active task)
See `checkpoint.md` for the precise resume point. Summary of what exists so far:

### Backend (services/backend) — DONE
- `src/config.rs`: `FinancialModuleMode` enum (`Deferred`/`Enabled`), parsed
  fail-closed from `FINANCIAL_MODULE_MODE` env var (defaults to `enabled` if
  unset, panics on garbage input), `AppConfig.financial_module_mode` field,
  `financial_module_deferred()` helper.
- `src/domain/models.rs`: `PaymentPolicy` enum (`Required`/`NotRequired`),
  `Appointment.payment_policy` field.
- Migrations (with down-migrations):
  - `20261210000001_financial_module_deferment` — new `payment_policy_enum`
    type + `appointments.payment_policy` column, backfilled `REQUIRED`.
  - `20261210000002_defer_completion_payment_gate` — rewrites
    `doctor_complete_consultation()` SQL function with a
    `v_payment_required` branch; down.sql restores the exact prior body.
- `src/repository/appointment_repo.rs`: `payment_policy` in all appointment
  SELECTs/INSERTs; new `insert_confirmed_no_payment()`.
- `src/services/appointment_service.rs`: `book_appointment`'s `gateway` param
  is now `Option<Gateway>`; new `book_appointment_deferred()` path — doctor
  verification, slot lock, confirms appointment with `payment_policy:
  NotRequired` and **zero** transaction/wallet/payment rows, fires normal
  booking-confirmed notification inside the same transaction.
- `src/api/v1/appointment_handlers.rs`: gateway resolution now tolerates
  `None`, `payment_policy` included in booking response JSON.
- `src/api/v1/config_handlers.rs` (NEW): `GET /config/booking-policy` —
  authenticated-any-user endpoint reporting `{"payment_required": bool}`,
  the single source of truth clients must read (never decide locally).
- Route registered in `src/api/router.rs`; `patient_handlers.rs` and
  `doctor_handlers.rs` appointment-listing SQL/JSON now include
  `payment_policy`.
- Tests: `tests/cases/financial_module_deferment.rs` (NEW, 7 tests, all
  passing) covering deferred booking (no gateway, no transaction row),
  double-booking still rejected, full Agora-join→complete lifecycle with zero
  wallet-balance change (including idempotent re-completion = 200 OK, not
  409), enabled-mode still requires gateway/payment_method_id (422), the
  booking-policy endpoint in both modes, and a deferred appointment being
  structurally unreachable by payment webhooks (404 — no transaction row or
  `payment_session_id` ever exists for a webhook to match against).
- Fixed 6 pre-existing `Appointment{}` literals + `AppConfig{}`
  `test_config()`/`synthetic_config()` in `tests/api_tests.rs` that broke
  from the new required struct fields.
- **Full integration suite: CONFIRMED GREEN.** `540 passed; 0 failed; 5
  ignored` in 1687.77s (~28 min). (A first attempt was killed by an external
  580s timeout wrapper — exit 143/SIGTERM — which was not a real test
  failure; the second, unwrapped run completed cleanly.)

### Patient Flutter (apps/mobile_patient) — DONE
- `booking_models.dart`: `BookingResultDto.paymentPolicy` (+`paymentRequired`
  getter), new `BookingPolicyDto`, `AppointmentDetailDto.paymentPolicy`
  (+`paymentRequired` getter).
- `booking_repository.dart`: `book()`'s `paymentMethodId` now nullable/omitted
  when not required; new `bookingPolicy()` calling `GET /config/booking-policy`.
- `booking_cubit.dart`: `paymentRequired`/`loadingPolicy` state (safe default
  `true`), `loadBookingPolicy()`, `canProceed` relaxed when payment not
  required, `submit()` passes `null` payment method when deferred.
- `patient_booking_page.dart`: conditionally renders fee/payment-method UI vs.
  a "No payment required" card; submit button label adapts.
- `app_en.arb`/`app_sw.arb`: added `bookingConfirmNoPayment` /
  `bookingNoPaymentRequiredNotice` (English + Swahili), ran `flutter gen-l10n`.
- `home_models.dart`: `AppointmentSummaryDto.paymentPolicy` (+getter).
- `consultation_status_page.dart`: fee display shows "No payment required"
  when `!a.paymentRequired` instead of a fee amount.
- Deliberately NOT touched: `paid_chat_booking_modal.dart` (informational
  pre-screen only — the real gate is `PatientBookingPage`), `waiting_room_page.dart`
  (already status-based, compatible with a CONFIRMED-from-start appointment).
- Test fixtures fixed for the new required `paymentPolicy` field:
  `booking_cubit_test.dart`, `consultation_booking_test.dart`,
  `pre_e2e_availability_payment_test.dart`, `waiting_room_cubit_test.dart`
  (all 5 sites in this last file fixed last).
- **`flutter analyze`: 0 issues. `flutter test`: 375/375 passed.**

### Doctor Flutter (apps/mobile) — DONE
- `doctor_models.dart`: added `BookingPolicyDto` (mirrors Patient's).
- `doctor_repository.dart`: added `bookingPolicy()` → `GET /config/booking-policy`,
  fails safe (rethrows; caller defaults to `true`).
- `doctor_cubit.dart`: `DoctorState.paymentRequired` (default `true`, the safe
  default), `copyWith` support, new `loadBookingPolicy()` — swallows errors,
  never surfaces them, keeps the safe default on failure.
- `app.dart`: `loadBookingPolicy()` wired into the existing startup
  `postFrameCallback` block alongside `loadAll()`/notifications.
- `doctor_pages.dart`: extracted a new standalone `DeferredWalletNotice`
  widget (placed immediately above `WalletPage`) — "Financial module not yet
  active" / "Payment collection is not yet active for this launch..." copy.
  `WalletPage.build()` now returns `DeferredWalletNotice` instead of the real
  earnings UI when `!state.paymentRequired`. Extracted as its own widget
  specifically so it's testable without pulling in `WalletPage`'s real
  network-calling `initState` (matches this file's own established test
  convention of testing inner cards directly rather than the full page).
- Tests added: `doctor_cubit_test.dart` (new `group('loadBookingPolicy
  (FINANCIAL_MODULE_MODE)')`, 4 tests: default-true, deferred-response,
  enabled-response, failed-fetch-keeps-safe-default-no-error), and
  `doctor_widgets_test.dart` (1 new widget test pumping `DeferredWalletNotice`
  directly, asserting no earnings/KES text leaks through).
- **`flutter analyze`: 0 issues. `flutter test`: 312/312 passed.**

### Doctor Web (apps/doctor-web) — DONE
- `src/lib/contracts.ts`: new `paymentRequiredFromPolicy(data)` pure helper
  (fails safe to `true` unless the server explicitly says `payment_required:
  false`); added `['GET', 'config/booking-policy']` to the BFF's
  `safeBackendPath` allowlist (GET-only).
- `src/components/Wallet.tsx`: new exported `DeferredFinancialNotice`
  component (same copy as Doctor Flutter's, for cross-app consistency).
- `src/components/DoctorApplication.tsx`: new `bookingPolicy` resource
  (`useResource<{payment_required:boolean}>(enabled?'config/booking-policy':null)`),
  `paymentRequired` derived via the new helper. Gated: the `wallet` page view,
  the `statement` and `payout` modal dialogs, the sidebar "Earnings & Wallet"
  extra-nav button (hidden outright when deferred, mirroring the Flutter
  approach of hiding rather than disabling), and a new `paymentRequired` prop
  passed to `Dashboard` and `Settings`.
- `src/components/Desktop.tsx`: `Dashboard` takes `paymentRequired` (default
  `true`); its wallet `useResource` call is skipped entirely when deferred,
  and the "Eligible Earnings" KPI card shows "Financial module not yet
  active" (no "View Wallet" button) instead of real amounts.
- `src/components/Settings.tsx`: `Settings` takes `paymentRequired`; the
  Payout section renders `DeferredFinancialNotice` instead of
  `<PayoutSettings/>`, and hides the "View Wallet" button, when deferred.
- Tests added to `tests/contracts.test.ts`: BFF allowlist admits
  `config/booking-policy` as GET only (not POST/PATCH); `paymentRequiredFromPolicy`
  fail-safe behavior (undefined/`{}`/`{payment_required:true}` → `true`;
  only explicit `{payment_required:false}` → `false`).
- **`npm run lint`: clean. `npx tsc --noEmit`: clean. `npm test`: 96/96
  passed** (94 pre-existing + 2 new).

### Admin Web (apps/admin-web) — DONE
- No BFF allowlist change needed: `src/app/api/proxy/[...path]/route.ts`
  forwards any path for an authenticated admin session (the backend's own
  ABAC does the authorization) — unlike Doctor Web, there's no client-side
  path allowlist here to update.
- `src/lib/api.ts`: new `BookingPolicy` interface + `getBookingPolicy()` →
  `GET /config/booking-policy`.
- `src/lib/bookingPolicy.ts` (NEW): pure `paymentRequiredFromPolicy()` helper,
  identical fail-safe rule to Doctor Web's (defaults to `true`; only an
  explicit `payment_required: false` flips it) — kept in its own
  dependency-free module so it's directly unit-testable, same reasoning as
  Doctor Web's `contracts.ts` helper.
- `src/lib/useBackendSync.ts`: added `onBookingPolicyUpdate` to
  `BackendSyncCallbacks`; added an **unconditional** polling job (same
  reasoning as the existing system-errors/verification-queue jobs — this
  gates nav/content on multiple pages, so it must stay current regardless of
  which page is open).
- `src/app/page.tsx`: new `paymentRequired` state (default `true`), wired to
  `useBackendSync`'s new callback; new local `DeferredFinancialNotice`
  component (same copy as the other three apps). Gated:
  - The `'doctors'` page's financial sub-section — "Master Payment Ledger →"
    / "Finalize Month" / "Prepare Batch" buttons and the 4 KPI cards (Gross
    Telehealth Revenue, 24h Chat Revenue, Platform Fee 20%, Unpaid Doctor
    Earnings) + the "platform charge 20%" note — replaced with
    `DeferredFinancialNotice` when deferred. The Physician Directory table
    itself (and its clinical columns: specialty, visit/chat counts,
    verification status) stays fully visible always; only its
    payout/amount/payout-method/status columns switch to a **distinct**
    "Deferred" label (deliberately NOT reusing the existing "Unavailable"
    string, which already means "no data yet" elsewhere in that same table —
    reusing it would make deferred mode look like a broken integration,
    violating the spec's "clearly distinguishes deferred mode from
    payment-provider failure" requirement).
  - The doctor-dossier modal (opened by clicking a directory row): its two
    financial KPI tiles (Total Gross Revenue, Net Payouts Disbursed) show
    "Deferred" instead of real amounts; the platform-charge note and the
    "Recent Financial Wallet Disbursements" list are replaced with a short
    deferred-mode note; the pending-disbursement-approval banner is now also
    explicitly gated (it was already structurally unreachable in deferred
    mode since no disbursement rows are ever created, but the explicit guard
    was added anyway for defense-in-depth/clarity).
  - The separate `'finance'` page ("Omnichannel Payment & Escrow Master
    Ledger") — its "Reconcile MFS Webhooks"/"Export Ledger CSV" buttons and
    its entire KPI-grid-through-ledger-table body are replaced with
    `DeferredFinancialNotice` when deferred. The page header
    (title/subtitle) and the "Doctor Payouts Hub →" nav-back button stay, so
    an admin who navigates there still sees *why* it's empty rather than a
    blank page.
  - **Judgment call, flagged explicitly**: unlike Doctor Web (which hides its
    sidebar "Earnings & Wallet" nav button outright when deferred), Admin
    Web's "Settlements & Payouts" sidebar nav item is deliberately left
    visible in both modes. Reasoning: this is the main admin sidebar (not a
    doctor-facing nav), and an admin needs to be able to navigate there to
    *confirm* the module is deferred (via `DeferredFinancialNotice`) rather
    than wondering why an entire section silently vanished from their
    console. Same category of judgment call as Task 2's AM/PM-vs-timezone
    decision — reviewers should sign off on this specific choice before
    deploy.
- Tests added: `tests/booking-policy.test.ts` (1 test, table-driven:
  undefined/`null`/`{}`/`{payment_required:true}` → `true`;
  `{payment_required:false}` → `false`).
- A pre-existing Redis-fixture test (`tests/refresh.test.ts`) requires its
  own disposable Redis fixture pointed at **logical DB 14** specifically
  (distinct from the backend's DB 0) plus a matching
  `hd:test-fixture:owner` marker key in that DB — this is a pre-existing
  project requirement unrelated to this task's changes, just not something
  previously exercised in this session; documented here in case a future
  session hits the same "TEST_REDIS_URL must name an explicitly provisioned
  disposable Redis fixture" failure again.
- **`npx tsc --noEmit`: clean. `npm run lint`: clean (1 pre-existing,
  unrelated warning in `CampaignManager.tsx`, not touched by this task).
  `npm test`: 127/127 passed** (99 `.test.ts` + 28 `.test.tsx`, including the
  1 new test — up from 126 pre-existing, 0 failed, 0 regressions).

### Still fully pending (not started)
All app/service-level implementation work for Task 4 is now complete and
green (Backend, Patient Flutter, Doctor Flutter, Doctor Web, Admin Web). The
disposable test Redis fixture container has been stopped and removed — no
further cleanup needed. What's left:
- The final comprehensive 9-point deliverables report

## Out-of-band fix — `cargo audit` CI failure (RUSTSEC-2026-0459)

User pasted a CI `cargo audit` failure: `libsqlite3-sys 0.30.1` flagged for
CVE-2026-119375 (RUSTSEC-2026-0459, medium 6.8), fix suggested >=0.39.0.

Investigated before touching anything: `cargo tree -i libsqlite3-sys` (and
again with `--all-features`) returns zero matches — it's not part of the
actual compiled dependency graph. It reaches `Cargo.lock` only because it's
`sqlx-sqlite`'s dependency, and `sqlx-sqlite` is itself gated behind sqlx's
optional `sqlite` feature, which this backend's `Cargo.toml` does **not**
enable (`default-features = false`, only `postgres` among the enabled
features — this is a PostgreSQL-only backend per CLAUDE.md). No SQLite code
is ever compiled into the binary; zero actual exploitable surface.

Confirmed `cargo update -p libsqlite3-sys --precise 0.39.0` genuinely cannot
work regardless: `sqlx-sqlite 0.8.6` itself pins `libsqlite3-sys = "^0.30.1"`,
which caps below 0.39.0 — there is no version of `libsqlite3-sys` we could
lock to that would both satisfy that pin and be patched, short of waiting for
upstream `sqlx-sqlite` to bump its own requirement (which we don't need
anyway, since the feature is off).

Tried a bare `cargo update` first to see whether a fresh re-resolve would
naturally prune the orphaned entry — it bumped 67 unrelated packages (all
safe minor/patch semver bumps) but did **not** remove `libsqlite3-sys` (it's
listed in `sqlx`'s own `Cargo.lock` package entry regardless of feature
activation — a Cargo.lock quirk, not a real reachability signal). Reverted
this bulk update entirely (restored `Cargo.lock` from a backup) since it was
an unrelated, unrequested 67-package diff that didn't even fix the reported
issue — out of scope for a single CVE fix.

**Actual fix applied**: `.github/workflows/security-scan.yml` already had an
established, precise precedent for exactly this situation —
`RUSTSEC-2023-0071` (rsa) is ignored there with a dated, verified, linked
comment explaining it's unreachable via sqlx's unenabled `mysql` feature.
Added `--ignore RUSTSEC-2026-0459` to the same `cargo audit` invocation,
with a comment in the same style/rigor, dated 2026-11-11, explaining the
exact reachability check performed and when to re-check (if sqlx's `sqlite`
feature is ever enabled, or once a `sqlx-sqlite` release ships with a
patched `libsqlite3-sys`).

**Verified the fix actually works**: ran `cargo audit --deny warnings
--ignore RUSTSEC-2023-0071 --ignore RUSTSEC-2026-0459` locally against the
committed (unmodified) `Cargo.lock` — exit code 0, no vulnerabilities
reported. `Cargo.lock`/`Cargo.toml` are untouched (confirmed via `git
status` — zero diff); only `.github/workflows/security-scan.yml` changed
(additively, on top of its pre-existing uncommitted edit from earlier in
this session). (source-level impact
  assessment, architecture/schema changes, minimal-implementation summary,
  regression tests+results, files changed, required rebuilds per
  app/service, production configuration instructions for setting
  `FINANCIAL_MODULE_MODE=deferred` on Azure, rollback procedure, residual
  risks) — per the explicit constraint to NOT merge/push/deploy/change Azure
  env vars until this has been reviewed, and per the user's stated release
  gate: Production backend → Free/policy-waived booking → Confirmed
  appointment → Real Agora consultation → Clinical completion → Notifications
  and records → only then restore `ENVIRONMENT=production` with
  `FINANCIAL_MODULE_MODE=deferred`.


## Task 5 — Patient Flutter UI replication (IN PROGRESS, 2026-11-12 to 2026-11-13)

Scope: `apps/mobile_patient` only. Branch `feature/patient-ui-replication`, uncommitted. No repository, cubit, model, network, backend or schema file was changed for presentation work. `flutter analyze` clean; `flutter test` 462 of 462 (407 before this task).

Authorities, by product-owner instruction: the reference collage `docs/design/afyamkononi-patient-ui-reference.jpeg`, the v2.1 brief (root docx), and individual designs supplied during the task. `CLAUDE.md` was edited to record each decision.

### 5.1 Discovery (complete)

Five documents in `docs/patient-ui/`: discovery report, screen inventory, component map, emulator workflow, redesign plan. Key findings: 36 screens; old tabs were Home, Services, Blogs, Account; Peer Navigator, Reminders, Facilities, videos and audio had no client or backend code; every endpoint the app calls exists.

### 5.2 Built

| Area | What changed |
|---|---|
| Theme | `PatientTokens` extended with the reference palette; primary green now `#009A73` |
| Shell | Tabs Home, Services, Messages, Profile; Messages built on first visit; module screens show the bottom bar |
| Home | Brand header, greeting, search pill (opens Services), hero, 3 × 3 module grid, upcoming-consultation card when one exists, offline state |
| Modules | One list drives Home and Services. Screens for education hub, support chat (over the existing Q&A), peer navigator (layout only), reminders, health records hub, facilities (layout only), consultation landing, privacy, more |
| Reminders | New local feature: encrypted on-device storage per account, Android local notifications, once/daily/weekly/monthly, cancelled on sign-out and re-armed on sign-in. No backend |
| Messages, Profile | Redesigned from the design language |
| Auth | Log in, create account, forgot password; header artwork plus native card; scrolls; independent of screen size |
| Notifications, Find a Doctor | Headers and empty states to the supplied designs |
| Consultation | Confirmed, waiting room and completion screens restyled on the existing pages; join gating, payment and polling logic untouched |
| Q&A | Topic list is now HIV Testing, PrEP, ART, Sexual Health, Stigma & Mental Health, Other |

New files: `core/widgets/afya_widgets.dart`, `core/widgets/afya_bottom_nav.dart`, `features/auth/auth_scene.dart`, `features/patient/modules/*`, `features/patient/reminders/*`, `features/patient/consultation/consultation_scene.dart`, `features/patient/home/upcoming_consultation_section.dart`, `assets/brand`, `assets/illustrations`, `assets/auth`.

Deleted: `features/patient/home/emergency_contact_banner.dart` (the banner left Home by product-owner decision; the number constant is still used on the consultation landing).

Android manifest: `RECEIVE_BOOT_COMPLETED` and the two `flutter_local_notifications` receivers, for reminders.

Tests added: `auth_scene_adaptive_test.dart` (31), `reminders_test.dart` (16), `post_consultation_layout_test.dart` (8). Tests edited: tab labels in `patient_navigation_safe_area_test.dart`; auth button and label copy in `auth_navigation_test.dart`; localization delegates added to `patient_chat_list_deferred_fee_test.dart`, `consultation_booking_test.dart`, `pre_e2e_availability_payment_test.dart`.

### 5.3 Deliberate differences from the designs

- Card 3 on Home is "Book Consultation", not "Book Testing, PrEP or ART".
- "Start Video Call" is "Book a Video Consultation" and opens doctor discovery; calls stay slot-booked and join-window gated.
- No sample facilities, reminders, records, consultation summaries or follow-up plans are shown; those need data the app does not have.
- Registration keeps required email, date of birth and gender; forgot password is email-code only.
- The GHI staff dashboard and multi-channel panels are not in the Patient app.

### 5.4 Open

- Sign-out defect (see `checkpoint.md`).
- Screens not yet redesigned, emulator captures still missing, and outstanding decisions: listed in `checkpoint.md` and in `docs/patient-ui/PATIENT_UI_REPLICATION_STATUS.md`.

### 5.5 Backend work the redesign implies (assessment, nothing built)

Sizes are rough judgements from reading the backend, not estimates from a plan.

| Item | What exists | What is missing | Size |
|---|---|---|---|
| Book Consultation, waiting room, call, completion | Everything | Nothing | None |
| Health Records (prescriptions, uploads) | Everything | Nothing | None |
| 15-minute consultation reminder | `CONSULTATION_REMINDER_15M` | Nothing | None |
| Support chat topics | Free-text category on questions | Optional server-side category filter | Small |
| Education topic filter | Article categories and tags in the schema | A category or tag parameter on the patient feed, a patient-readable category list, seeded topics | Small |
| Messages inbox previews and unread badges | `last_message_at` on conversations; a doctor-only chat list | A patient conversations endpoint with last message and unread count | Small |
| Help & Support, emergency number config | Legal documents endpoint | Content or config endpoints | Small |
| Consultation summary and follow-up plan on the completion screen | Clinical outcome only | A patient-visible summary and plan written by the doctor: schema, doctor-side entry, patient read | Medium (touches Doctor app or Doctor Web) |
| Facilities directory | Nothing | Table and migration, Admin CRUD and Admin Web screens, patient list and detail endpoints, map-link validation | Medium |
| Videos and audio | Nothing (article media accepts cover images only) | Content model, storage or links, Admin authoring, patient endpoints | Medium |
| Reminder sync across devices | Nothing (reminders are local) | Optional; table and endpoints if wanted | Small to medium, optional |
| Peer Navigator | Nothing | New role, registration branch, Admin approval, patient-facing directory, chat-request state machine, conversations not tied to an appointment, call authorisation for peer sessions, access rules and tests; also Doctor Flutter and Admin Web changes | Large |

Summary: the app as it now stands needs **no backend work to ship**. The small items are each roughly a day or two. Facilities and video content are each roughly a week of backend plus Admin Web. Peer Navigator is the one heavy piece, on the order of several weeks across backend, Doctor app and Admin Web.


## Task 6 — Doctor Flutter UI replication and Peer Navigator discovery (IN PROGRESS, 2026-11-13)

Scope: `apps/mobile` only; uncommitted. Target is the Doctor reference board in `docs/design/` reproduced pixel for pixel (product-owner instruction; recorded in `CLAUDE.md` rule 2). Discovery, plan, Peer Navigator gap analysis and per-screen status are in `docs/doctor-ui/`; read `DOCTOR_UI_REPLICATION_STATUS.md` first.

- Discovery complete: 35 screens; the mobile consultation-completion screen is unreachable; no backend for doctor notes, patient list or record, or structured prescriptions; no Peer Navigator support anywhere.
- Design preview: `flutter run -t lib/dev/design_preview_main.dart` opens the signed-in screens with labelled sample data, no login. Debug only.
- Batch A built: login, two-step registration, account-under-review and other approval states, splash; new theme colours. Password-reset and legal-agreement screens have the new theme but not yet a reworked layout.
- `flutter analyze` clean; `flutter test` 324 passed, 10 golden-image tests failing as they did before the work (platform font drift; goldens will need regenerating on CI).


## Task 7 — Patient post-rebuild gap analysis (DIAGNOSTIC COMPLETE, 2026-11-13)

Report: `docs/patient-ui/PATIENT_POST_REBUILD_GAP_ANALYSIS.md`. No product code, backend, configuration or migration was changed.

- Baseline: HEAD `3c09124` + uncommitted redesign; Flutter 3.47.6 / Dart 3.13.5; `flutter analyze` 0 issues; `flutter test` 462/462; debug APK built and run on `Pixel_9` (API 37) against the local backend.
- Runtime verified: registration, email code, session restore, Home, Services, Messages, Profile, nine module screens, notification inbox, data rights, English and Swahili, text scale 1.3, 360 dp.
- Not testable locally: every consultation flow (no doctors, slots or appointments in the local database), push delivery (FCM transport is a no-op locally), Peer (no backend).
- Defects found: intermittent sign-out after sign-in (P0, root cause unverified); Notification settings never loads; password reset blocked outside development with a misleading message; Home card ignores server join data; chat booking missing from Book Consultation; Profile chips overflow at 1.3 (open since baseline); plus visual and copy items. Counts P0 3, P1 9, P2 19.
- Fixture run (approved): one synthetic doctor, three slots, a video and a chat booking. Verified: directory, profile, EAT slots, deferred booking, confirmation, waiting room, permissions, RTC token (shared channel), server completion, PHI-free notifications. New defects: deferred chat bookings never get a conversation (P0); no API sets doctor fees; completion screen/history unreachable after the fact; consultation copy contradictions. Final counts P0 4, P1 12, P2 25.
- Next: fix P0-1 (sign-out) and P0-4 (deferred chat activation), then a two-client video test.
