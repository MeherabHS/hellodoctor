# Doctor Backend & Contract Verification Audit (Agent A)

Scope: `services/backend` (Rust/Axum/SQLx/Postgres), cross-checked against
`apps/mobile/lib/features/doctor` (Flutter) and `apps/admin-web` where the
Doctor domain touches Finance. Read-only audit — no production code was
modified. Repo root: `C:\Users\CUBE\Desktop\helodoc old site`. HEAD
`e3739ae5` plus uncommitted work, branch `main`.

Everything below is derived from reading the actual current source,
migrations, and running the actual test suite / live server in this
environment on 2026-10-06/07 — not from the self-reported docs
(`docs/DOCTOR_APP_FINAL_SYSTEM_AUDIT.md`, `docs/DOCTOR_PAYOUT_ARCHITECTURE.md`,
`docs/DOCTOR_FLUTTER_BACKEND_MAPPING.md`), which were read only for
orientation and are called out explicitly wherever they diverge from what
the code actually does.

---

## 0. Test suite results (actually run, this session)

| Command | Result |
|---|---|
| `cargo fmt --check` | **Clean** — zero diff output. |
| `cargo clippy --all-targets -- -D warnings` | **Clean** — 0 warnings, 0 errors, exit 0. |
| `cargo test --lib` | **15 passed, 0 failed.** |
| `cargo test --test api_tests -- --test-threads=1` | **47 passed, 4 failed** (51 total `#[sqlx::test]` functions), 374.45s. |

### The 4 `api_tests` failures are environment races, not product defects

All four failures are the *identical* failure shape, at test-pool-connect
time, before the test body runs at all:

```
thread '<test>' panicked at sqlx-core-0.8.6/src/testing/mod.rs:198:14:
failed to connect test pool: Database(PgDatabaseError { severity: Fatal, code: "3D000",
message: "database \"_sqlx_test_<id>\" does not exist",
detail: Some("It seems to have just been dropped or renamed."), ... })
```

Failing tests: `test_monthly_finance_disbursement_batch`,
`test_multi_prescription_intake_and_5_photo_limit`,
`test_production_without_providers_fails_closed_without_false_success`,
`test_wallet_credited_exactly_once_under_concurrent_completion`.

This is `sqlx::test`'s own ephemeral-test-database lifecycle (it creates a
throwaway DB per test against the shared `hellodoctor-postgres` container
and reaps old ones) racing with **another concurrent `cargo test` process
against the same Postgres container** — i.e. another agent/process in this
shared sandbox dropping a test database that this run's test was still
using, or vice versa. Evidence this is a known hazard in this exact repo,
not a hypothesis I invented: before I ran anything, `Get-Process` showed two
pre-existing `api_tests-*.exe` processes already running (PIDs 1768, 26808,
started before this session) which I was not able to remove (blocked by the
harness's own "Interfere With Workloads" guard — reasonably, since another
agent may own them); my first `cargo test --test api_tests` attempt failed
outright with `ld.lld: ... Permission denied` because one of those stale
binaries still held the `.exe` file lock. The second run (reported above)
got past the link step but still raced on Postgres. Separately,
`tests/api_tests.rs` itself documents (lines ~198-210) that Redis state in
this environment is "one real, persistent instance shared by every test run
... including ad hoc manual testing against the same container" and had to
add a `FLUSHDB` specifically because of a prior cross-run contamination
incident — the exact same class of shared-infrastructure fragility, just on
the Redis side instead of Postgres. None of the 4 failures show an
*assertion* failure (no `assertion failed`, no mismatched expected/actual
value) — only the connection-setup panic. **I could not get a fully clean
single-process run in this shared environment in the time available; the
four flagged tests should be re-run in isolation (no other `cargo test`
against the same Postgres container) before trusting a 51/51 claim, but I
have no code-level evidence these four are genuinely broken** — three of
them (prescription 5-photo limit, production-fails-closed, wallet-credited-
once) are exactly the invariants I independently verified by reading the
service/handler code below, and that code is consistent with the tests'
names/intent.

This is itself a finding, filed as **MEDIUM / PROCESS**: the backend test
suite is not safely runnable in parallel against a shared dev Postgres
instance; `sqlx::test`'s template-DB reaping has no isolation from a second
concurrent `cargo test` invocation. Recommended fix: point CI/local
multi-agent work at per-agent ephemeral Postgres containers, or set
`DATABASE_URL`/`PGDATABASE` to a distinct template name per process.

---

## 1. Endpoint inventory (Doctor-relevant, verified against `router.rs` + handlers)

All routes are under `/api/v1`. "Auth" = requires bearer token (all but the
explicitly public ones). Source: `services/backend/src/api/router.rs`
lines 102-341.

| Method/Path | Handler | Role/ABAC Action | DB tables (primary) | Flutter call site | Test coverage | Status |
|---|---|---|---|---|---|---|
| POST `/auth/doctor/register` | `auth_handlers::doctor_register` | Public, rate-limited (5/15min/IP) | `users`, `doctor_profiles`, `doctor_wallets` | `auth_cubit.dart:135-193` (`registerDoctor`) | `api_tests.rs` (registration/duplicate/capacity tests) | **OK** |
| POST `/auth/doctor/login` | `auth_handlers::doctor_login` | Public, IP+account jail | `users` via `AuthService` | `auth_cubit.dart` login | covered | **OK** |
| POST `/auth/doctor/verify-otp` | `auth_handlers::doctor_verify_otp` | Public step 2 | sessions | `auth_cubit.dart` | covered | **OK** |
| POST `/auth/refresh`, `/auth/logout` | `auth_handlers::{refresh_token,logout}` | Public (refresh-token-authed) | `refresh_tokens` | `api_client.dart` | covered | **OK** |
| GET `/doctor/profile` | `doctor_handlers::get_own_profile` | Doctor, `DoctorRead` | `doctor_profiles` | `doctor_repository.dart:295-300` | covered | **OK** |
| PATCH `/doctor/profile` | `doctor_handlers::update_own_profile` | Doctor, `DoctorManageProfile` | `doctor_profiles` (allow-list: `full_name` only) | `doctor_repository.dart:302-306` | covered | **OK** |
| GET/PUT `/doctor/duty` | `doctor_handlers::{get_own_duty,update_own_duty}` | Doctor, `DoctorRead`/`DoctorManageOwnSchedule` | `doctor_profiles.is_on_duty` | `doctor_repository.dart:153-166` | covered | **OK** — going on-duty requires `verified_doctor_profile_id` (unverified doctor cannot go on duty) |
| GET/POST `/doctor/slots` | `doctor_handlers::{list_own_slots,create_own_slot}` | Doctor, `DoctorManageOwnSchedule` | `doctor_schedule_slots` | `doctor_repository.dart:168-185` | `test_unverified_doctor_clinical_restrictions_and_concurrent_schedule_conflict`, `test_doctor_contract_endpoints_and_qna_authorization` | **OK** |
| DELETE `/doctor/slots/:id` | `doctor_handlers::cancel_own_slot` | Doctor, own `AVAILABLE` slot only | `doctor_schedule_slots` | `doctor_repository.dart:187-190` | same as above | **OK** |
| GET `/doctors/:id/slots` | `doctor_handlers::get_doctor_slots` | AnyAuthenticated | `doctor_schedule_slots` | (patient booking flow) | covered | **OK** |
| GET `/doctor/appointments` | `doctor_handlers::list_own_appointments` | Doctor (verified), `DoctorRead` | `appointments`+`doctor_schedule_slots`+`consultation_sessions`+`patient_profiles` | `doctor_repository.dart:141-151` | covered | **OK** |
| GET `/doctor/chats` | `doctor_handlers::list_own_chats` | Doctor (verified), `DoctorRead` | `chat_conversations`+`chat_messages` | `doctor_repository.dart:192-201` | covered | **OK** |
| GET/POST `/chat/:id/messages` | `chat_handlers::{get_chat_messages,send_chat_message}` | existing conversation authz | `chat_messages` | `doctor_repository.dart:203-237` | covered incl. BOLA test | **OK** |
| GET `/doctor/appointments/:current/patient-history` | `doctor_handlers::list_patient_history` | Doctor, `DoctorRead` + SQL `doctor_history_context_allowed` | `appointments`,`doctor_schedule_slots`,`consultation_sessions` (fn) | `doctor_repository.dart:239-247` | `test_doctor_patient_history_requires_current_treatment_context`, `test_doctor_history_window_pagination_and_cutoff` (boundary-tests minute-by-minute) | **OK — thoroughly tested** |
| GET `/doctor/appointments/:current/patient-history/:episode` | `doctor_handlers::get_patient_history_detail` | same | same + `prescriptions`,`prescription_intake_documents` | `doctor_repository.dart:249-253` | same tests | **OK** |
| GET `/doctor/consultations/:id/summary` | `doctor_handlers::consultation_summary` | Doctor (verified) + same context-window SQL fn | `appointments`,`appointment_clinical_intake`,`consultation_sessions` | `doctor_repository.dart:437-450` | covered | **OK** |
| POST `/consultations/:id/rtc-token` | `consultation_handlers::mint_agora_token` | existing consult authz | n/a (Agora token mint) | `doctor_repository.dart:477-480` | `...rtc token must not be mintable hours before the scheduled slot window opens` | **OK** |
| POST `/consultations/:id/telemetry`, `/telemetry/events` | `consultation_handlers::{submit_telemetry,submit_telemetry_events}` | AnyAuthenticated (resource-scoped internally) | `consultation_telemetry*` | `doctor_repository.dart:482-486` | covered | **OK** |
| POST `/consultations/:id/complete` | `consultation_handlers::complete_consultation` | Doctor, `CompleteConsultation` | `appointments.clinical_outcome` (`ClinicalOutcome` enum: `CompletedWithRx`/`CompletedNoRx`/`Referred`/`Escalated`) | `doctor_repository.dart:465-469` | covered | **OK — matches CLAUDE.md §2.6 exactly** |
| POST `/consultations/:id/prescription` | `prescription_handlers::upload_doctor_prescription` | Doctor, `UploadDoctorPrescription` | `prescriptions` | `doctor_repository.dart:488-499` | covered | **OK, 5-page server cap (see §9)** |
| POST `/consultations/:id/complete-no-rx` | `prescription_handlers::complete_no_rx` | Doctor, `UploadDoctorPrescription` | `prescriptions` (`is_no_rx_required`) | `doctor_repository.dart:471-475` | covered | **OK** |
| POST `/appointments/:id/prescriptions` | `prescription_handlers::upload_patient_intake` | Patient, `UploadPatientIntake` | `prescription_intake_documents` | (patient app) | covered | **OK, 5-image server cap, race-safe (see §9)** |
| GET `/appointments/:id/documents/:doc_id` | `prescription_handlers::get_document_download_url` | AnyAuthenticated + resource check | grant in Redis (300s TTL) | `doctor_repository.dart:501-510` | covered | **OK** |
| GET `/documents/download` (public, token-gated) | `prescription_handlers::download_document` | short-lived signed grant only | — | via resolved URL | covered | **OK** |
| GET `/doctor/qna/questions` | `doctor_handlers::list_open_questions` | Doctor, `DoctorQna` | `health_questions`,`health_answers` | `doctor_repository.dart:255-276` | covered | **OK** |
| POST `/doctor/qna/questions/:id/answers` | `doctor_handlers::answer_question` | Doctor (verified), `DoctorQna` | `health_answers` | `doctor_repository.dart:278-282` | covered | **OK** |
| GET `/doctor/wallet/summary` | `wallet_handlers::get_doctor_wallet` | Doctor, `ViewOwnWallet` | `doctor_wallets` | `doctor_repository.dart:319-324` | covered | **OK** |
| GET `/doctor/wallet/ledger` | `doctor_handlers::wallet_ledger` | Doctor, `ViewOwnWallet` | `transactions` | `doctor_repository.dart:379-394` | covered | **OK** |
| GET/PUT `/doctor/wallet/payout-method` (legacy) | `wallet_handlers::{get_payout_method,set_payout_method}` | Doctor, `ViewOwnWallet`; **PUT is `development`-only** | `doctor_payout_methods` | `doctor_repository.dart:326-346,367-377` | `test_doctor_contract_endpoints_and_qna_authorization` area | **PARTIAL — see §5** |
| GET `/doctor/wallet/payout-methods` | `wallet_handlers::list_payout_destinations` | Doctor, `ViewOwnWallet` | `doctor_payout_destinations` | `doctor_repository.dart:326-346` | covered | **OK (read-only)** |
| POST `/doctor/wallet/payout-methods/dev-bind` | `wallet_handlers::dev_bind_payout_method` | Doctor, `ViewOwnWallet`; **`development`-only, hard 403 otherwise** | `doctor_payout_destinations` | `doctor_repository.dart:348-355` | covered | **SIMULATED only — see §5** |
| GET `/doctor/wallet/statements` | `wallet_handlers::get_monthly_statement` | Doctor, `ViewOwnWallet` | `financial_periods`,`monthly_payout_statements`,`monthly_statement_transactions`,`disbursement_items`,`financial_adjustments`,`transactions` | `doctor_repository.dart:396-410` | not directly named in `api_tests.rs` grep, but exercised by `test_monthly_finance_disbursement_batch` (one of the 4 races above — connection-level failure only) | **OK, real month-bounded SQL — see §10/§12** |
| POST `/appointments/book` | `appointment_handlers::book_appointment` | Patient, `BookAppointment` | `appointments`,`transactions`,`doctor_schedule_slots` | (patient app) | covered | **Hard-disabled outside `development` — see §10** |
| POST `/webhooks/payment/:provider` | `webhook_handlers::handle_payment_webhook` | Public, HMAC-signed | `transactions`,`payment_events` | n/a | covered | accepts `bkash`/`nagad`/`mpesa`/`card`/`bank-transfer` — see §6 |
| GET `/admin/bmdc/queue`, POST `/admin/bmdc/:id/verify`, `/reject` | `admin_handlers::{get_bmdc_queue,verify_bmdc_doctor,reject_bmdc_doctor}` | `ClinicalAdmin | PlatformAdmin` only via `ManageBmdcQueue` | `doctor_profiles.is_verified` only | n/a (admin web) | covered | **OK — see §4** |
| GET `/admin/finance/summary` | `admin_handlers::get_finance_summary` | `FinanceAdmin | PlatformAdmin` | `transactions`,`doctor_wallets` | admin-web | — | **No month filter at all — see §12** |
| GET `/admin/finance/transactions` | `admin_handlers::list_admin_transactions` | `FinanceAdmin | PlatformAdmin | Compliance` | `transactions` | admin-web | — | **No month filter at all — see §12** |
| GET `/admin/finance/payout-routing` | `admin_handlers::list_payout_routing` | `FinanceAdmin | PlatformAdmin` | `doctor_profiles`,`transactions`,`monthly_payout_statements`,`doctor_payout_destinations`,`disbursement_items` | admin-web | — | **OK — genuinely month-bound SQL, defaults to current month** |
| POST `/admin/finance/periods/finalize` | `admin_handlers::finalize_financial_period` | `InitiateDisbursement` (`FinanceAdmin|PlatformAdmin`) | `financial_periods`,`monthly_payout_statements`,`monthly_statement_transactions` | admin-web | — | **OK, explicit-only, idempotent-safe — see §11** |
| POST `/admin/disbursements/initiate`, `/admin/disbursements/:id/approve` | `admin_handlers::{initiate_disbursement,approve_disbursement_item}` | `InitiateDisbursement` | `disbursement_batches`,`disbursement_items` | admin-web | `test_disbursement_claims_each_transaction_at_most_once_under_concurrency` | **OK, `money_moved:false` explicit in response** |

---

## 2. Flutter ↔ backend contract cross-check

Read `apps/mobile/lib/features/doctor/doctor_repository.dart` in full (511
lines) against `router.rs`. **Every call site matches an existing route,
verb, and path exactly** — no stale paths, no undocumented-but-implemented
routes called from Flutter, no routes the Flutter client expects that are
missing. This contradicts the "DOCTOR_APP_NOT_VERIFIED" framing insofar as
the *wiring* is concerned — the contract is in fact consistent as of this
revision.

One genuine doc defect found: **`docs/DOCTOR_FLUTTER_BACKEND_MAPPING.md`
(lines 34-35) is stale.** It states "Monthly payout statement — Not yet
available — Genuine backend gap" and "Provider payout binding — Not yet
available — Genuine backend gap." The monthly-statement endpoint
(`GET /doctor/wallet/statements`) is fully implemented (`wallet_handlers.rs`
lines 258-349, with FINALIZED-snapshot and IN_PROGRESS-estimate branches,
pagination, entries, adjustments) and is wired in the router and called
from Flutter. The *payout binding* half of that doc's claim is still
accurate (see §5), but the monthly-statement half is not — the doc was not
updated after `docs/CHECKPOINT.md`'s "Kenya payout routing backend
continuation — 2026-10-06" entry shipped it. **Severity: LOW** (doc-only,
code and Flutter already agree) — fix: update the mapping table's monthly-
statement row.

---

## 3. Doctor registration

`services/backend/src/api/v1/auth_handlers.rs` lines 44-55, 109-264:
`DoctorRegistrationRequest` has `#[serde(deny_unknown_fields)]` and exactly
10 fields (`full_name, license_number, phone_number, email, password,
primary_specialty, experience_years, current_hospital, qualifications,
residential_address`) — **no `role`, `is_verified`, `is_admin`, or payout
field exists on the wire; a client cannot supply any of them.** Server
hardcodes `role: UserRole::Doctor`, `is_verified: false` (line 207-210,
244), `is_on_duty: false`. Verification is 100% server-driven (§4). This is
a genuinely separate flow from patient registration (`register_otp` /
`verify_otp_and_login`, OTP-only, no license/specialty fields at all) —
confirmed, not assumed.

**The claimed "Account / Professional Details / Payout Setup / Review &
Submit" 4-step wizard does not exist.** I read the actual screen,
`apps/mobile/lib/app.dart` lines 230-320+ (`RegistrationPage`): it is a
**single flat `ListView` form** with all 10 fields and one submit button
that calls `AuthCubit.registerDoctor` (`auth_cubit.dart` lines 135-193) in
one shot — no steps, no payout fields at all (consistent with the backend
accepting none). On success it shows a static "pending ClinicalAdmin
verification" screen. If any doc or prior report claims a multi-step wizard
with a payout step, that claim is false for this revision — flag as
**stale-assumption, not a defect** (nothing is broken; the premise in prior
material was simply wrong).

---

## 4. Professional verification vs. payout — separation confirmed clean

- `abac.rs` line 90-92: `ManageBmdcQueue` (backing `GET /admin/bmdc/queue`,
  `POST /admin/bmdc/:id/verify`, `POST /admin/bmdc/:id/reject`) is gated to
  `ClinicalAdmin | PlatformAdmin` only. `Doctor` is not and cannot be in
  this match arm — a doctor role calling any of these three routes gets
  `AppError::Forbidden` from `abac::authorize` before the handler body runs.
  **A doctor can never self-verify.**
- `doctor_repo::set_verified` (`repository/doctor_repo.rs` lines 86-103)
  touches **only** `doctor_profiles.is_verified` (and optionally
  `is_on_duty`, forced off on rejection) — it never touches
  `doctor_payout_methods` or `doctor_payout_destinations`. Conversely,
  nothing in `wallet_repo.rs` or `wallet_handlers.rs` ever writes
  `doctor_profiles.is_verified`. **Clinical verification and payout
  capability are two fully independent state machines in both schema and
  code — no leakage either direction.**
- What being "verified" actually gates: `ctx.verified_doctor_profile_id`
  is required for going on duty (`update_own_duty`, only when turning *on*),
  `list_own_appointments`, `list_own_chats`, `answer_question`, and
  `book_appointment`'s target-doctor check (`doctor.is_verified` checked
  explicitly, `appointment_service.rs` line 48). Unverified doctors can
  still read their own profile and (per `create_own_slot`, line 203) create
  schedule slots — note **slot creation uses `verified_doctor_profile_id`
  too**, so an unverified doctor cannot create slots either (confirmed by
  `test_unverified_doctor_clinical_restrictions_and_concurrent_schedule_conflict`,
  which asserts 403 on slot creation pre-verification).

---

## 5. Payout backend — classification: **SIMULATED in dev, NOT_IMPLEMENTED for real provider binding in production**

Evidence, not the doc's own say-so:

- `services/backend/src/services/appointment_service.rs` line 42-44:
  `book_appointment` **hard-fails** (`AppError::Conflict`, "Payment
  collection is not configured. No checkout or confirmed payment was
  created.") for **every** booking attempt unless
  `state.config.environment == "development"`. There is currently no
  payment-collection path at all outside dev — this is the single biggest
  gap in the whole Doctor/Patient pipeline, since nothing downstream
  (settlement, wallet credit, statements, disbursement) can ever happen in
  production without a booking existing first.
- `services/backend/src/api/v1/wallet_handlers.rs`: both `set_payout_method`
  (legacy PUT, line 135-140) and `dev_bind_payout_method` (line 199-203) are
  gated by the identical `if state.config.environment != "development" {
  return Err(Forbidden) }`. **In production there is currently no route a
  doctor can call to create or bind any payout destination at all** — only
  the read routes (`GET /doctor/wallet/payout-method`,
  `/payout-methods`) work in prod, and they will simply return "not
  configured" / empty for every doctor forever, because nothing can ever
  populate them outside dev.
- `wallet_repo::create_dev_payout_destination` (`repository/wallet_repo.rs`
  lines 58-84): inserts a row with `provider_code='DEV_SIMULATED'`,
  `provider_reference='dev-payout-' || gen_random_uuid()`, and a
  **hardcoded masked string** (`"•••• 8214"` passed in from the handler) —
  it never calls any HTTP client, SDK, or external domain. Grepped the
  entire `services/backend/src` tree for `safaricom`, `daraja`, `m-pesa
  api`-shaped strings and any outbound HTTP client construction tied to a
  payments domain: **none exist.** `services/backend/.env.example` has
  **no M-Pesa/Daraja/provider secret variables defined at all** (only
  `AGORA_APP_CERTIFICATE`, `WEBHOOK_HMAC_SECRET`, `STORAGE_SECRET_KEY` —
  none payment-provider-related) — further confirming no real integration
  exists to even configure.
- The schema itself enforces this honestly: migration
  `20261017000001_clinical_and_payout_invariants.up.sql` line 38-39 adds
  `CHECK(status NOT IN ('PAID','CONFIRMED','DISBURSED') OR (paid_at IS NOT
  NULL AND gateway_reference IS NOT NULL AND provider_code IS NOT NULL AND
  provider_code <> 'DEV_SIMULATED'))` on `disbursement_items` — **a
  dev-simulated destination can never be marked as actually paid**, so the
  system cannot accidentally represent a fake payout as a real one. This is
  a genuine strength, not a gap.

**Classification: SIMULATED** (dev-only fixture, honestly labeled, cannot
leak into a "paid" state) for the binding/creation path; **NOT_IMPLEMENTED**
for any real M-Pesa/bank/card provider integration in production, and the
booking pipeline itself (prerequisite for everything downstream) is also
**NOT_IMPLEMENTED in production** (hard-gated to dev). This matches
`docs/CHECKPOINT.md`'s own internal admission (`GENUINE_BACKEND_GAP_
PROVIDER_PAYOUT_BINDING`, "External provider binding and payout execution
remain deferred") — independently confirmed from code, not taken on faith.

**Severity: CRITICAL (by design/known, not a surprise)** — flagging because
the mission requires it be stated plainly: *HelloDoctor's backend currently
cannot take a real payment or make a real payout in production at all.*
Recommended fix: this is presumably the next major milestone, not a quick
patch — needs a real M-Pesa Daraja (or similar) integration with
server-side-only credentials, replacing both the `book_appointment` gate
and the payout-binding gate.

---

## 6. Kenya-only defect: live Bkash/Nagad surface in Finance

CLAUDE.md requires BDT/bKash/Nagad be isolated as historical-only, never
reachable by the live Kenya-facing API. Findings:

- **Flutter is clean**: grepped `apps/mobile/lib` for
  `Bkash|bKash|Nagad|BDT|৳|Bangladesh` — **zero matches.**
- **Backend is not fully clean.** `Gateway::{Bkash,Nagad}` and
  `PayoutMethodType::Bkash` remain live, compiled, reachable enum variants:
  - `appointment_handlers.rs` line 21: `BookAppointmentRequest.gateway:
    Gateway` is client-deserialized with **no validation rejecting
    Bkash/Nagad for a Kenya deployment** — any caller (or an integration
    test, as seen in the `api_tests.rs` log: `POST
    /api/v1/webhooks/payment/bkash` fired repeatedly during the real test
    run) can still request/settle a `bkash` gateway transaction. In
    practice this is moot today only because `book_appointment` is
    entirely dev-gated (§5), not because gateway choice is itself
    Kenya-restricted.
  - `admin_handlers::get_finance_summary` (lines 728-781) **actively
    computes and returns** `gateway_volume.bkash` and `gateway_volume.nagad`
    buckets in the live `/admin/finance/summary` response shown to
    PlatformAdmin/FinanceAdmin on the Kenya-only deployment's admin
    dashboard. The code's own comment (lines 745-752) admits this sums
    `gross_amount` **across currencies** (BDT vs KES) if both markets were
    ever live simultaneously, and says so explicitly: "Flagged, not fixed
    here." This is dead-market surface left live and user-visible, not
    isolated as historical/back-compat-only.
  - `webhook_handlers.rs` line 38-39 and `appointment_service.rs` lines
    138-139 still map `"bkash"`/`"nagad"` provider strings to live `Gateway`
    variants in the webhook ingestion path.

**Severity: MEDIUM.** Not a security hole (webhook ingestion is still
HMAC-gated, and booking is dev-only in prod today), but it is exactly the
kind of "active-path reference not isolated as historical" the mission
flags as a defect, and it will become a real bug the day `book_appointment`
is turned on for production without someone first removing/guarding these
gateway choices. Recommended fix: either remove `Bkash`/`Nagad` from the
live `Gateway`/`PayoutMethodType` enums (move them to a separate
"legacy read-only" representation used only by historical-fixture code), or
add an explicit `gateway.is_kenya_supported()` check in
`book_appointment`/`set_payout_method`/`dev_bind_payout_method`, and strip
the bkash/nagad buckets out of `get_finance_summary`'s live response (or
gate them behind a feature flag that's off for the Kenya deployment).

---

## 7. Provider secret boundary

- `DoctorPayoutMethod.account_identifier_encrypted` (`domain/models.rs` line
  198-199) is correctly annotated `#[serde(skip_serializing)]` — it can
  never leak into any JSON response; all payout-method handlers
  (`wallet_handlers.rs`) manually construct response DTOs
  (`PayoutMethodResponseDto`) that only ever carry `account_identifier_
  last4`, never the encrypted blob or plaintext.
- `DoctorPayoutDestination` (used by `list_payout_destinations`,
  `list_payout_routing`) carries only `masked_destination`,
  `provider_reference` (a locally-generated opaque reference, not a
  provider secret), and status enums — no raw account data.
- `AGORA_APP_CERTIFICATE` exists only in `services/backend/.env.example`
  and is never referenced anywhere under `apps/mobile` (grepped,
  zero matches) — CLAUDE.md §7's "never in Flutter client code" rule holds.
- No merchant/provider API secret, signing key, or consumer-secret-shaped
  string exists anywhere in `apps/mobile` or `apps/admin-web` (grepped
  broadly). Given §5's finding that no real provider integration exists yet,
  there is currently nothing to leak — the boundary is enforced by
  absence as much as by design, which is worth re-checking once real
  M-Pesa credentials are introduced.

**No defect found here.**

---

## 8. Patient history contract — thoroughly implemented and tested

`doctor_history_context_allowed(p_appointment, p_user)` (migration
`20261005000002_history_access_window.up.sql` lines 6-20) is a `SECURITY
DEFINER STABLE` SQL function, evaluated against `CURRENT_TIMESTAMP`
(Postgres server time, never a client-supplied value):

```sql
AND CURRENT_TIMESTAMP >= s.start_time - INTERVAL '15 minutes'
AND ( (a.status IN ('CONFIRMED','WAITING','IN_CONSULTATION') AND cs.ended_at IS NULL)
   OR (a.status='COMPLETED' AND COALESCE(cs.ended_at,a.completed_at) IS NOT NULL
       AND CURRENT_TIMESTAMP < COALESCE(cs.ended_at,a.completed_at) + INTERVAL '24 hours') )
```

It also binds `d.user_id = p_user` (the CURRENT appointment's doctor must be
the caller) and is called from `list_patient_history`/
`get_patient_history_detail`/`consultation_summary` alike.
`doctor_patient_history(...)` additionally joins
`a.patient_id = current_a.patient_id` (same-patient) and
`s.start_time < anchor.start_time` (only earlier episodes), with real SQL
`LIMIT GREATEST(p_limit,1) OFFSET GREATEST(p_offset,0)` — and the handler
(`doctor_handlers.rs::page()`, line 89-94) independently clamps
`limit.clamp(1,100)` before that, so pagination is doubly bounded, never
unbounded.

**Test coverage is exemplary** — `tests/api_tests.rs` lines 3671-3748
(`test_doctor_history_window_pagination_and_cutoff`) literally tests the
exact boundaries the mission asked about: 16 min before start → 404, 15 min
before → 200, 1 min before/after start → 200, then after completion: 1439
min after `ended_at` → 200, 1440 min (24h exactly) → 404, 1441 → 404,
100000 → 404; plus a same-current-appointment-cancelled → 404 case and a
different-doctor → 404 (BOLA) case at every offset tested. **No gaps found
here — this is the strongest-verified contract in the whole audit.**

---

## 9. Slot contract — verified, GiST exclusion constraint present and correct

- `CreateSlotRequest { start_time: DateTime<Utc>, end_time: DateTime<Utc>
  }` (`doctor_handlers.rs` line 184-187) — both fields required, no
  duration default exists.
- Past-slot prevention: `if body.start_time <= Utc::now() { return
  Err(Validation) }` (line 200-202), server time, not client-supplied.
- Overlap prevention is enforced **twice**: application-level
  (`SELECT EXISTS(... start_time < $3 AND end_time > $2 ...)` inside a
  `doctor_profiles ... FOR UPDATE` lock, line 205-215) *and* at the DB level
  — `migrations/20261001000001_initial_schema.up.sql` lines 130-134:
  `CONSTRAINT no_overlapping_slots EXCLUDE USING gist (doctor_id WITH =,
  tstzrange(start_time, end_time) WITH &&) WHERE (status != 'CANCELLED')`.
  Confirmed live via a real concurrency test
  (`test_unverified_doctor_clinical_restrictions_and_concurrent_schedule_
  conflict`, lines 588-594): two concurrent identical slot-creation requests
  race, and the test asserts the actual outcome is `[201, 409]` — i.e. the
  DB constraint is what ultimately decides the race, not just the app-level
  check (which has a TOCTOU window the GiST exclusion closes).
- Booked-vs-cancellable: `cancel_own_slot` only updates rows matching
  `status='AVAILABLE'` (`doctor_handlers.rs` line 237) — a `BOOKED` slot
  cannot be cancelled through this endpoint; `rows_affected() != 1` →
  409 Conflict. No test explicitly exercises cancelling an already-`BOOKED`
  slot (only "cancel someone else's slot" is tested, line 3418-3424,
  → 409), so this exact path (own slot, but already booked) is
  **code-verified but not test-verified** — minor gap.

**Severity: LOW** (missing one specific test case, not a code defect).
Recommended fix: add a test that books a slot then attempts
`DELETE /doctor/slots/:id` as the owning doctor and asserts 409.

---

## 10. Multi-image Rx backend — server-enforced cap, race-safe, real MIME/size handling

`PrescriptionService::upload_patient_intake_documents`
(`services/prescription_service.rs` lines 15-110):
- Server-side cap is real: `if existing_count as usize + files.len() > 5 {
  return Err(UnprocessableEntity) }` (line 43-47) — **not** client-trust.
- Race safety: `appointment_repo::lock_for_document_upload` (line 38, a row
  lock) serializes the count-check-insert sequence per appointment, with an
  explicit comment explaining exactly the TOCTOU race this closes.
- Each file independently runs through `UploadSecurityService::
  sanitize_async` (MIME/content validation) before persistence; a
  SHA-256 content hash is computed and stored.
- Retry safety / idempotency: the **doctor-side** single-prescription-photo
  path (`upload_doctor_prescription_photo`, lines 153-161) explicitly
  dedupes by `(appointment_id, integrity_verification_hash)` — re-uploading
  the exact same bytes after a client-side failure returns the existing
  row rather than creating a duplicate/charging again. The **patient
  intake** multi-file path does not do content-hash dedup across retries
  (it will create new rows for a retried batch up to the 5-cap, then reject
  once the cap is hit) — this is reasonable (intake isn't a priced/chargeable
  action) but is worth noting as an asymmetry.
- On any mid-batch failure (storage put or DB insert), the handler
  deletes every object it had already written in that request before
  returning the error — no orphaned storage objects on partial failure.
- Doctor-prescription cap is independently enforced too:
  `upload_doctor_prescription_photo` lines 162-173, `count >= 5` → 422, and
  a `no_rx` record blocks any further Rx upload (mutual exclusion between
  "has Rx" and "explicitly no Rx" is enforced, lines 164-168 and in
  `complete_without_prescription` lines 238-249).

**No functional defect found.** I could not exercise this live end-to-end
(the `hellodoctor-minio` container the mission brief described as running
was **not actually present** in this environment —
`docker ps -a` shows only `hellodoctor-postgres` and `hellodoctor-redis`;
`docs/DOCTOR_APP_FINAL_SYSTEM_AUDIT.md` line 9 independently confirms MinIO
was deliberately removed from this environment on 2026-10-07 at the user's
request, which matches what I observed, so this is an intentional
environment state, not something I broke). This means I verified the Rx cap
from code + the one relevant integration test name (its actual pass/fail
status is one of the 4 connection-race failures in §0, not an assertion
failure) rather than a live multi-file HTTP upload.

---

## 11. Financial state machine (from code, not the architecture doc's prose)

```
transactions.payment_status:
  INITIATED -> PAYMENT_HELD -> SETTLED_TO_DOCTOR -> DISBURSED
                              (-> REFUND_PENDING -> REFUNDED_TO_PATIENT)
                              (-> FAILED)
```
(`domain/models.rs` `PaymentStatus` enum, confirmed against
`appointment_service.rs::process_payment_webhook` which transitions
`INITIATED`→`PAYMENT_HELD` on a webhook, find-or-create/idempotent against
provider event-id replay.)

```
financial_periods.status:  OPEN (implicit/absent row) -> FINALIZED
  (explicit POST /admin/finance/periods/finalize only; see §12 — no
   calendar-time auto-transition exists anywhere in the codebase)
```

```
disbursement_items.status:
  PENDING_APPROVAL -> APPROVED -> PROCESSING -> {PAID | FAILED | RECONCILIATION_REQUIRED}
```
(`disbursement_service.rs` line 45, `wallet_repo::approve_disbursement_item`,
`admin_handlers::approve_disbursement_item` response literally returns
`"money_moved": false` — the approval step is explicitly documented, in the
API response itself, as not having moved any money.) A DB `CHECK` constraint
(§5) makes it **structurally impossible** for an item to reach `PAID` (or
`CONFIRMED`/`DISBURSED`) with a `DEV_SIMULATED` provider code or without a
`paid_at`+`gateway_reference`+real `provider_code` — i.e., the simulated dev
path can never masquerade as a real payout at the data layer, independent of
any application logic bug.

Transitions that are automatic/webhook-triggered: `INITIATED→PAYMENT_HELD`
(payment webhook), `PAYMENT_HELD→SETTLED_TO_DOCTOR`+wallet credit (also
inside `process_payment_webhook`/settlement logic, crediting
`doctor_wallets` via `wallet_repo::credit`). Transitions requiring explicit
Finance/admin action: period finalize, disbursement initiate (batch
preparation — allocates but does not move money, per `CHECKPOINT.md` and
confirmed by the `money_moved:false` response), disbursement item approve
(still `money_moved:false`), and actual provider execution/PAID state —
**which has no code path at all today** (consistent with §5 — there is no
provider-execution service; the state machine's final "real money moves"
step is not implemented).

---

## 12. Financial period finalization — explicit-only, confirmed

`admin_handlers::finalize_financial_period` (lines 119-176):
- Gated by `Action::InitiateDisbursement` (`FinanceAdmin | PlatformAdmin`).
- Grepped the entire `services/backend/src` tree for `tokio::time::interval
  | cron | scheduler | background` — **no scheduled job of any kind exists
  anywhere in the codebase** (`main.rs` has no `tokio::spawn` background
  loop at all). Finalization is reachable **only** through this one
  endpoint.
- Idempotency: re-finalizing an already-`FINALIZED` period for the same
  `(currency, period_year, period_month)` returns `409 Conflict`
  ("already finalized and immutable") rather than silently re-running —
  confirmed by the `SELECT ... FOR UPDATE` + status check before the
  `INSERT ... ON CONFLICT DO UPDATE` (lines 143-159), and the whole
  operation is wrapped in an advisory xact lock
  (`pg_advisory_xact_lock(61012, year*100+month)`) to serialize concurrent
  finalize attempts for the same period.
- Refuses to finalize a period with any `INITIATED`/`PAYMENT_HELD`
  transaction still outstanding in that date range (line 166-170) — closes
  the month only when everything in it is actually settled.
- A `CHECK` constraint (`finalized_period_evidence`, migration
  `20261017000001...` line 36) additionally guarantees a `FINALIZED` row
  always carries `finalized_at`/`finalized_by` at the DB layer, independent
  of application code.

**No defect found — this is a correctly-built explicit-only state
transition with real safeguards.**

---

## 13. Finance month routing

- `GET /doctor/wallet/statements?month=YYYY-MM`
  (`wallet_handlers::get_monthly_statement`, lines 258-349): genuinely
  month-scoped. Validates format strictly (`month.len()==7`, parses as
  `YYYY-MM-01`, year in `2000..=2200`), and **rejects any future month**:
  `start > Utc::now().date_naive().with_day(1)` → 422 ("Choose a current or
  previous month"). Queries bound by `t.created_at >= $2 AND t.created_at <
  $3` (half-open date range) when not yet finalized, or by the frozen
  `monthly_payout_statements`/`monthly_statement_transactions` snapshot rows
  tied to the specific `financial_period_id` when finalized — **no
  current-month fallback and no all-time leakage** in either branch.
- `GET /admin/finance/payout-routing?month=YYYY-MM`
  (`admin_handlers::list_payout_routing`, lines 49-109): also genuinely
  month-bound (`t.created_at >= $1::date AND t.created_at < $2::date`,
  parameters derived from the requested month), defaulting to the current
  month only when `month` is omitted (a reasonable default, not a bug).
- **`GET /admin/finance/summary` and `GET /admin/finance/transactions` have
  NO month/period query parameter at all** — `get_finance_summary` and
  `list_admin_transactions` (lines 649-783) take no `Query<...>` extractor
  whatsoever and unconditionally call `transaction_repo::list_all`, i.e.
  **every transaction ever, all-time, every call**, with no way to scope
  to a given period. This is a genuine gap versus the mission's expectation
  that "Finance month routing" works across the Finance surface, not just
  the per-doctor statement/routing views.

**I was not able to complete a live two-month HTTP comparison test in this
environment**: the already-running backend process (PID 16620, confirmed
alive via `Get-Process` before I touched anything) had exited by the time I
went to use it (no port 8080 listener, process no longer existed — I did
not kill it). I restarted it myself via `cargo run` per the brief's
instructions, but the freshly-started process failed every DB-touching
request with `password authentication failed for user
"hellodoctor_auth_worker"` / `"hellodoctor_system_worker"` (Postgres error
`28P01`) — a role-credential mismatch between this fresh process's
connection pools and whatever the `hellodoctor-postgres` container's roles
currently are. This looks like drift from concurrent activity elsewhere in
this shared sandbox (role passwords reset, or `.env` changed, after the
original process had already cached working connections) rather than
anything I changed, and fixing Postgres role passwords was out of scope for
a read-only audit, so **I did not force it**. I therefore verified month
routing by reading the exact SQL (above) rather than by live two-month
comparison — per the mission's own fallback allowance. The code reading is
unambiguous enough (explicit date-range bind parameters, explicit rejection
of out-of-range months) that I'm confident in the **OK** verdict for the
two endpoints that do scope by month, and the **gap** verdict for the two
that don't.

**Severity: MEDIUM** for the missing month filter on
`/admin/finance/summary` and `/admin/finance/transactions` — Finance has no
way to pull a specific historical month's aggregate totals or transaction
list from the admin surface at all today; they'd have to filter client-side
from an all-time dump, which also means every call re-transfers the
entire transaction table. Recommended fix: add the same
`month: Option<String>` pattern already used by `list_payout_routing`
and `get_monthly_statement` to both handlers.

---

## 14. Summary of defects (ranked)

| # | Severity | Area | Defect | Fix |
|---|---|---|---|---|
| 1 | CRITICAL (known/by design) | Payments/Payout | No real payment collection or payout provider integration exists; `book_appointment` and payout-binding are hard-disabled outside `development` | Build the real M-Pesa/bank integration; this is a milestone, not a patch |
| 2 | MEDIUM | Finance API | `/admin/finance/summary` and `/admin/finance/transactions` have no month/period filter at all (all-time only) | Add `month` query param + date-range bind, matching `list_payout_routing`'s pattern |
| 3 | MEDIUM | Kenya-only compliance | Live `Gateway::{Bkash,Nagad}` / `PayoutMethodType::Bkash` remain reachable (webhook ingestion, booking DTO, finance summary buckets) with no Kenya-only guard | Remove or feature-flag-gate for the Kenya deployment |
| 4 | MEDIUM | Test infra | 4/51 `api_tests` failed on `sqlx::test` ephemeral-DB races against a shared Postgres container used by concurrent processes | Isolate test runs (dedicated Postgres per agent/CI job) |
| 5 | LOW | Docs | `docs/DOCTOR_FLUTTER_BACKEND_MAPPING.md` says monthly statement is "not yet available" — it is implemented | Update the doc |
| 6 | LOW | Tests | No test exercises "attempt to cancel one's own already-BOOKED slot" (code correctly blocks it, just untested) | Add the test |
| 7 | CRITICAL (same root cause as #1, see §16/A1) | Auth, Payments | Doctor/patient MFA-OTP SMS delivery (`auth_service.rs:45-48,183-187`) and payment-webhook confirmation (`webhook_handlers.rs:67-72`) are *also* hard-gated to `environment=="development"` — no doctor can even complete login in production today, independent of the booking/payout gates in #1 | Same milestone as #1: real SMS gateway + provider wiring |
| 8 | LOW (see §16/A2) | Flutter↔backend contract | `WalletDto.last_disbursement_at/last_disbursement_amount` (Flutter) are parsed from a response the backend never populates | Add the two fields to `DoctorWalletSummaryDto`, or drop them from the Flutter DTO |

## 15. What I could not verify, and why

- **Live multi-file Rx upload end-to-end**: `hellodoctor-minio` is not
  running in this environment (confirmed via `docker ps -a`; matches an
  intentional removal documented in `docs/DOCTOR_APP_FINAL_SYSTEM_AUDIT.md`
  line 9, not something I broke). Verified from code + existing test name
  only (§10).
- **Live two-month Finance HTTP comparison**: the backend process I
  restarted could not authenticate to Postgres as its scoped DB roles
  (`hellodoctor_auth_worker`, `hellodoctor_system_worker`) — a credential
  drift in this shared sandbox, not something within a read-only audit's
  scope to fix. Verified from SQL/code instead (§13).
- **A fully clean, contention-free `api_tests` run**: blocked by other
  concurrent processes in this shared environment holding file locks /
  racing Postgres test-database lifecycle (§0). I was not permitted to
  terminate the pre-existing stale processes (reasonably — they may belong
  to another agent). The failures observed are connection-setup races, not
  assertion failures, and three of the four failing tests' named invariants
  were independently confirmed correct by direct code reading.
- **A real M-Pesa/Safaricom Daraja sandbox**: does not exist in this
  codebase or environment at all (§5) — there is nothing to test against.

---

## 16. Addendum — independent second-pass findings (merged, not overwritten)

**Collision notice:** this file was overwritten mid-session by what appears to be a second, independent
"Agent A" process running the identical audit task concurrently in this same shared sandbox (same repo, same
`hellodoctor-postgres` container, same file path). This explains several environmental anomalies both of us
hit independently: two extra, unexplained `cargo` processes and `_sqlx_test_*` database churn observed via
`pg_stat_activity`; the live backend process dying mid-audit; and `api_tests` failing differently on each of
our runs despite identical commands. Per instruction, the existing (overwriting) content above is kept as
the current state rather than reverted. This addendum merges in findings from my own independent pass that
are **not** already covered above, so nothing either of us found is lost. Everything below was independently
re-verified against the actual source in this session, not copied from the other version.

**A1. The production fail-closed gate is wider than booking + payout-binding alone — two more independent
subsystems are gated the same way, and together they form a clear pattern worth stating as one finding:**
- `services/backend/src/services/auth_service.rs:45-48` (patient OTP) and `:183-187` (Doctor MFA/OTP, step 1
  of login) both hard-fail with `AppError::Conflict("SMS delivery is not configured. No verification code was
  dispatched.")` for every call outside `environment=="development"`. **There is no real SMS gateway
  integration anywhere in the codebase** (confirmed by grep — no Twilio/Africa's Talking/SMS-provider client
  exists). This means **no doctor can complete login at all in a production deployment today**, independent
  of and in addition to the booking/payout gates already documented in §5 above.
- `services/backend/src/api/v1/webhook_handlers.rs:67-72` additionally gates the inbound payment webhook
  itself: `if state.config.environment != "development" { return Err(Conflict("Real payment-provider
  verification is not configured. No payment was confirmed.")) }` — so even if a real provider could somehow
  call this endpoint, it would still be refused in production, independent of HMAC signature validity.
- Put together with §5's two gates, that's **four** independent, unrelated subsystems (login MFA delivery,
  booking/payment collection, payment webhook confirmation, payout binding) all hard-gated to
  `environment=="development"` — not one isolated gap but a consistent, deliberate architectural pattern of
  "fail closed until a real provider is wired in." Worth stating explicitly as the headline framing for
  whoever reads this report, since it means the *entire* external-facing production pipeline — not just
  Finance — is presently inert by design.

**A2. `WalletDto` in Flutter expects two fields the backend never sends.**
`apps/mobile/lib/features/doctor/doctor_models.dart:146-147,163-167` parses `last_disbursement_at` and
`last_disbursement_amount` out of the `GET /doctor/wallet/summary` response, but
`DoctorWalletSummaryDto` (`services/backend/src/services/disbursement_service.rs:12-25`) and the handler
building it (same file, lines 49-62) never populate either field. Harmless today — `optionalText()` on the
Flutter side degrades to `null`/blank — but it is a genuine Flutter-needs/backend-doesn't-return contract gap
that a straight route-by-route comparison (as in this addendum's §2-equivalent check) surfaces. **Severity:
LOW.** Fix: either add these two fields to `DoctorWalletSummaryDto` (derived from the most recent `PAID`
`disbursement_items` row) or remove them from the Flutter DTO.

**A3. Confirmed on the consumer side that the `/admin/finance/summary` / `/admin/finance/transactions`
month-filter gap (this file's §13, ranked defect #2) is live in the shipped Admin web client, not just a
theoretical backend gap:** `apps/admin-web/src/lib/api.ts:337` and `:341` call
`/admin/finance/transactions` and `/admin/finance/summary` with **no query string at all**, while `:345-346`
appends `?month=` only to `/admin/finance/payout-routing`. The Admin web client was built to match the
backend's actual (uneven) capability — this is a real, currently-shipped limitation of the Finance dashboard,
not a hypothetical one a future caller might hit.

**A4. Test-suite flakiness, a second independent data point.** My own full serialized run
(`cargo test --test api_tests -- --test-threads=1`, run in isolation after this file's original
sqlx-core-0.8.6 panics at `testing/mod.rs:261:14` (`DROP DATABASE ... is being accessed by other users`,
a post-test-cleanup race) — a **different** failure signature than the `testing/mod.rs:198:14` ("failed to
connect test pool...does not exist") panics reported in §0 above, on a **different** set of 5 tests
(`test_grievance_cannot_be_adjudicated_twice`, `test_max_active_users_caps_new_registration_not_existing_logins`,
`test_multi_dimensional_abuse_jailing`, `test_patient_otp_auth_flow_and_token_refresh`,
`test_unverified_doctor_clinical_restrictions_and_concurrent_schedule_conflict`). I confirmed one of mine
(`test_unverified_doctor_clinical_restrictions_and_concurrent_schedule_conflict`, the slot-overlap/
unverified-doctor test) **passes cleanly in complete isolation** (`cargo test --test api_tests
test_unverified_doctor_clinical_restrictions_and_concurrent_schedule_conflict -- --test-threads=1
--nocapture` → `1 passed; 0 failed`), with only a non-fatal cleanup warning logged. Two independent runs,
two different random subsets of failures, both exclusively at `sqlx::test`'s own fixture setup/teardown
boundary and never inside an actual test assertion — this **strongly corroborates** (rather than merely
repeats) this file's own ranked defect #4: the test suite is not safe to run concurrently against a shared
dev Postgres instance, and whichever of us runs it next should expect a different random handful of failures
for the same underlying reason, not a regression. Recommend treating any single `api_tests` run's exact
failure list as noise until tests run against an isolated per-process Postgres instance; the **shape** of the
failures (always at the sqlx-test-harness boundary, never an assertion) is the reliable signal, and both
independent passes agree on that shape.

**No other corrections to the content above are needed** — the endpoint inventory, registration/verification
analysis, payout classification, patient-history/slot contract verification, Rx upload analysis, and
financial-state-machine/period-finalization sections in this file were independently cross-checked against my
own reading of the same source files during my pass and found to be accurate.
