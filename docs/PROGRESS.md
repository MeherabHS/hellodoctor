## B-01 prevention control (migration/deployment integrity) — 2026-10-07 (follow-up pass)

Direct follow-up to the four-agent audit below, scoped to exactly one finding: B-01, where a
long-running backend had silently served a schema 7 migrations behind the repository for
hours, 500ing every Doctor registration, "self-healed" only by a restart that happened to
occur. The original audit report called this "self-healed, no further code fix possible" —
correct about the self-healing, wrong that nothing more could be done. This pass designed and
built an actual prevention control, then live-verified it end to end, not just unit-tested it.

**Work was split across three parallel subagents, each owning a disjoint file set specifically
to avoid the exact class of concurrent-cargo-process collision this incident itself is about**
(only one of the three touched `services/backend/src`/`tests` and ran `cargo`, so no two
processes raced the same build lock or shared Postgres instance):

1. **Runtime self-check** — new `services/backend/src/migration_status.rs`: a named
   `pub static MIGRATOR` (the single source of truth for which migrations this compiled binary
   expects — never hardcoded), plus a pure, unit-tested comparison function that checks the
   *full set* of expected migration versions against the full set `_sqlx_migrations` actually
   records as applied (not just the highest version number — a gap in the middle, exactly the
   shape of the original incident's partial out-of-band schema state, is caught too). `GET
   /healthz` (previously a static `"OK"` string with no DB awareness at all) now runs this
   check on every call and returns `200`/`"ready"` or `503`/`"not_ready"` with
   `binary_latest_migration`/`database_latest_migration` fields, never leaking raw SQL errors.
   A new narrow migration (`20261019000001_migration_status_grant`) grants the ordinary
   request-serving `hellodoctor_api` role `SELECT` on `_sqlx_migrations` only — it had zero
   access before, since that table was previously touched exclusively by the superuser
   migrator connection.
2. **Dev-tooling three-way verifier** — new `services/backend/scripts/verify_backend_schema.ps1`:
   catches the one case the running process structurally cannot detect on its own — new
   migration files added to disk *after* the process already started. Compares
   `SOURCE_LATEST_MIGRATION` (newest file on disk) against `/healthz`'s reported
   binary/database versions and reports a specific, distinct reason
   (`BACKEND_RESTART_REQUIRED` vs. `DATABASE_MIGRATION_REQUIRED` vs. `BACKEND_UNREACHABLE`).
   Read-only: never kills a process, installs anything, or mutates the database.
3. **Canonical dev-start command** — new `services/backend/scripts/start_backend_ready.ps1`:
   checks required containers are already up (never installs them), restarts any stale
   `hellodoctor-backend` process (a known file-lock hazard from earlier sessions), launches
   `cargo run`, polls `/healthz` until `200`, then *requires* the verifier script above to also
   pass before ever printing the literal line `HELLODOCTOR_BACKEND_READY` — no previous ad-hoc
   "start the backend" pattern in this project had that property.
4. **CI gate** — new `.github/workflows/backend-migrations.yml`: real `postgres:16` +
   `redis:7-alpine` service containers running the full backend test suite on every push/PR
   touching `services/backend/**`. The only pre-existing workflow covered dependency/secret
   scanning only; nothing previously ran the test suite in CI at all.
5. **Guardrail docs** — `CLAUDE.md` §4 now states plainly: restart the dev backend after any
   new migration file; a green `cargo test` run does not prove the long-running shared dev
   database is current (`#[sqlx::test]` gives every test its own fresh throwaway database);
   run the verifier script before claiming a backend defect fixed against the persistent dev
   DB; never hand-edit `_sqlx_migrations`.

**Live verification, performed directly, not delegated**: independently re-ran (not just
trusted the implementing subagent's self-report) `cargo fmt --check` (clean), `cargo clippy
--all-targets -- -D warnings` (clean), `cargo test --lib` (**19/19**, up from 15 — 4 new
`migration_status` unit tests), `cargo test --test api_tests -- --test-threads=1` run alone
(**53/53**, unchanged count — the existing `test_health_check` was strengthened in place
rather than duplicated). **Backend total: 72/72.** Then ran `start_backend_ready.ps1` for real
against the actual Docker containers — it stopped the stale process, started a fresh one,
polled readiness, ran the verifier, and printed `SOURCE_LATEST_MIGRATION =
BINARY_LATEST_MIGRATION = DATABASE_LATEST_MIGRATION = 20261019000001` /
`SCHEMA_VERIFIED` / `HELLODOCTOR_BACKEND_READY`. Finally fired a real `POST
/auth/doctor/register` with a complete valid Kenyan payload against that freshly-verified
process: `{"success":true,"data":{"status":"PENDING_VERIFICATION",...}}` — the exact call that
500'd for ~14 hours during the original incident now succeeds cleanly.

**B-01 reclassified from `SELF_HEALED_ONLY` to `PREVENTION_CONTROL_VERIFIED`** in
`docs/DOCTOR_SYSTEM_FINAL_MULTI_AGENT_AUDIT.md` §3a. This does not make an already-running
process clairvoyant about migration files written after it started — nothing can — but both
drift cases (database behind binary; binary behind source) are now detectable within seconds
via a script any developer or agent can run before trusting a live result, and the build fails
in CI if migrations ever stop applying cleanly from scratch. Per the user's explicit
instruction, this pass touched nothing else — not payout onboarding, registration UX, refund
UI, the Finance month-picker, Flutter prototype polish, Agora, or external payment providers,
all of which remain exactly as documented open/deferred in the entry below.

## Doctor system four-agent audit + fix pass — 2026-10-07

Triggered by an explicit adversarial multi-agent acceptance request: four independent
read-only audits (Backend/Contract, Security/Bug/Concurrency, Business-Logic/Financial-Gap,
Flutter Quality/UX) ran concurrently against the live backend, Postgres, Redis, and Flutter
source, each writing its own report to `docs/audit/`. Full synthesis, deduplicated findings,
every fix's reproduce/root-cause/minimum-fix/regression-test, and the mandatory capability
status table are in `docs/DOCTOR_SYSTEM_FINAL_MULTI_AGENT_AUDIT.md` — summary only below.

**Confirmed and fixed, all with new automated regression tests against the real HTTP/DB
stack:** (1) Admin's "Unpaid Doctor Earnings" KPI/doctor-list/dossier were reading a dead
`doctor_wallets.pending_disbursement` counter that nothing ever decremented — every doctor who
ever earned anything showed as permanently unpaid; now sourced from the real
finalized-statement/disbursement-item aggregate (a double-counting bug in the fix's first draft
was itself caught by its own new regression test before landing). (2) An approved grievance
refund had **no resolution path at all** once approved — stuck at `APPROVED` forever with zero
Finance action possible; Finance can now list and transition refund requests through a new,
column-scoped-UPDATE-granted endpoint pair. (3) A refund approved against an already-wallet-
settled transaction never reversed that wallet credit, permanently overstating a doctor's
lifetime earnings — now reversed via a new `wallet_repo::debit`, with the DB-level finalized-
statement math left untouched (the obvious "add a financial_adjustments row" fix would have
double-subtracted and under-paid the doctor — caught before it was written, not after). (4) The
refund-approval HTTP response claimed money was "Disbursed" when it never was — corrected to
match the already-honest DB-level remedy text. (5) Two live seeded doctor accounts were still
serving Bangladesh identity data (BMDC/BD/Dhaka/+880) over real authenticated sessions on a
Kenya-only deployment, predating an earlier session's Kenya rewrite of `seed.rs` that never
backfilled the already-existing rows — corrected live; `/admin/finance/summary`'s gateway
breakdown no longer surfaces bkash/nagad buckets on the Kenya dashboard; `license_country`'s
column default changed from `'BD'` to `'KE'`. (6) `/admin/finance/summary` and
`/admin/finance/transactions` had no month/period filter at all (always all-time) — now accept
the same `?month=YYYY-MM` pattern `payout-routing` already used; `apps/admin-web`'s API client
updated to match. (7) `ip_jail`'s deliberate fail-closed Redis-unavailable behavior surfaced as
an indistinguishable generic 500 — now a dedicated 503 `SERVICE_UNAVAILABLE`. (8) Flutter's
`WalletDto.last_disbursement_at/amount` were parsed but never sent by the backend — now
populated from the authoritative `disbursement_items` table, live-verified.

**Live-verified, not just test-suite-verified**: restarted the real dev server (confirmed
26/26 migrations applied), and against the actual running process — logged in as a real doctor
and confirmed Kenya data; logged in as real admin (live TOTP) and confirmed the corrected
`/admin/overview`/`/admin/finance/summary` figures and new refund-requests route; and closed a
previously-unclosed audit gap by standing up **two real, independent, MFA-completed doctor
sessions** and firing a genuine cross-account IDOR attempt (doctor 1 tries to cancel doctor 2's
real schedule slot via its real ID → `409`, doctor 2 cancelling their own → `200`).

**Evidence**: backend `cargo fmt --check`/clippy clean; `cargo test --lib` 15/15; full
`cargo test --test api_tests -- --test-threads=1` run alone (not racing other concurrent
processes, which is what caused every previous session's flaky-failure reports) — **53/53**,
0 failed; Flutter `flutter analyze` clean, `flutter test` 49/49 (unchanged — no Flutter source
touched this pass); admin-web `tsc --noEmit` clean, `npm run build` succeeds. **Backend total:
68/68.** Left open and explicitly documented rather than silently skipped: an Admin-web UI
page for the new refund-resolution endpoints, a Finance month-picker control wired to the new
API parameter, and several Flutter prototype-parity/UX items (chat canned-replies, header
quick-actions, Rx lab-report button, booked-slot cancel affordance, app-resume session
re-validation) — all judged lower priority than the financial-correctness and security fixes
above, per the audit's own explicit priority ordering.

## Doctor Flutter field-test hardening — 2026-10-06 (pass 3: SafeArea bug, mock-data root cause, email login, payout method)

Triggered by real on-device IRL testing of pass 2's build, which surfaced a visual bug, a data-integrity scare, and two explicit product-design requests.

**Status-bar SafeArea bug (visual only, not backend):** Q&A, Consultation Chats (list), Smart Rx Scanner, Consultation Schedule, and Doctor Earnings & Wallet all build a custom header instead of a platform `AppBar`, and none of their `Scaffold`s had a `SafeArea` — their header text rendered directly under the Android status bar, confirmed from the user's own screenshots. Queue avoided this by accident (a zero-height `AppBar` already reserves the inset); `ChatThreadPage` already had `SafeArea` from pass 2. Fixed by wrapping each affected screen's `Scaffold` in `SafeArea(bottom: false, ...)`.

**Root cause of "a lot of mock patient data":** not a Flutter bug. The live dev Postgres database had 271 separate `patient_profiles` rows all literally named "Patient User," a third non-seeded doctor ("Dr. Live Specialist"), 753 appointments, 753 transactions, 686 chats, and 3,275 slots — leftover fixture data from the constrained-capacity load-testing work (`docs/CONSTRAINED_CAPACITY_TEST.md`) never cleaned out of the shared dev database. Confirmed by direct query before touching anything. With explicit user confirmation, the user ran a `TRUNCATE` (rows only — no table/schema change) across all 30 content tables except `_sqlx_migrations`. `SEED_DEV_DATA` (an explicit opt-in flag; seeding is deliberately never automatic — see `main.rs`) was flipped to `true` for one restart so the backend's own existing seeding logic rebuilt exactly the canonical baseline, then flipped back to `false` immediately, restoring the original "a plain restart behaves like production" property. Verified clean by direct query: 2 real doctors, 1 real test patient, 5 open slots, zero appointments/transactions/chats.

**Email-based doctor login** (the user's own words: *"this part was my fault of design"*): doctor login moved from BMDC license number to email address, end to end — `DoctorLoginRequest`/`DoctorRegistrationRequest` (email now required, validated by a new `validate_email`), `AuthService::doctor_login_step1` now looks up by email, seed doctors given real emails, Flutter `LoginPage`/`RegistrationPage`/`AuthCubit` updated to match. **A real, independent bug was caught along the way**: `doctor_verify_otp`'s brute-force account jail still keyed on `license_number` after the login handler moved to email, so repeated MFA failures and a fresh login check were jailing two different keys that never matched — silently defeating the lockout. Caught by the existing `test_doctor_mfa_brute_force_is_blocked` test failing, fixed by keying both on the same `user.email`.

**Doctor payout method setup** (new feature, explicitly requested): a doctor can now register where Finance should send a scheduled disbursement (M-Pesa/bKash/card/bank transfer). New RLS-protected `doctor_payout_methods` table, raw account identifier encrypted at rest (reusing the existing TOTP AES-256-GCM scheme, generalized into `crypto::encrypt_secret`/`decrypt_secret`), only `display_label` + last-4 ever returned by the API. `DEFERRED_EXTERNAL_PAYMENT_INTEGRATION` is unchanged — this stores a destination, it never moves money. New Wallet-screen card + setup sheet in Flutter.

Evidence: backend `cargo check`/clippy clean, `cargo test --test api_tests -- --test-threads=1` **43/43** (up from 42); Flutter `flutter analyze`/`dart format` clean, `flutter test` **41/41** (up from 40); live server restarted and re-verified against the real running Postgres/Redis containers after every change, not just the test suite; release APK rebuilt successfully throughout (`apps/mobile/build_field_test.ps1`). The three documented backend-data gaps from pass 2 (chat preview/category, Rx lab-reports/vitals, Q&A answered-list) remain untouched, awaiting a product decision.

## Doctor visual replication checkpoint - 2026-10-06 (pass 2: Chat/Smart Rx/Schedule/Wallet/Q&A)

Queue was accepted from a real-device screenshot this session (minor typography/overflow fixes applied and now frozen per instruction). The remaining five primary Doctor screens — Consultation Chats, Smart Rx Scanner, Consultation Schedule, Doctor Earnings & Wallet, and Open Health Q&A — were fully rebuilt this pass by reading the actual `prototype/index.html` markup/CSS for each (not from memory), cross-checked against the real Rust backend contracts, and verified with `flutter analyze` (clean), `dart format --set-exit-if-changed` (clean), and `flutter test` (40/40, including new responsive tests at 360/390/412px and business-rule-wording tests). Full detail, per-screen discrepancy notes, and the three backend-data gaps intentionally left unfilled (chat preview/category, Rx lab-reports/vitals, Q&A answered-list) are in `docs/DOCTOR_FLUTTER_REPLICATION_REPORT.md`.

A genuine, minimal, additive backend defect was found and fixed in this pass: `list_own_appointments`, `list_own_chats`, `list_own_slots`, and `wallet_ledger` only ever returned `patient_id` (a UUID), never a patient name — every screen needs this to show a doctor who a row belongs to. Fixed via a `patient_profiles` join (RLS-safety verified first), with a new regression test; full backend suite re-run clean at 42/42 after each change, live dev server restarted to serve the fix.

**The rendered-screenshot-comparison gate below is still not satisfied for these five screens** — this environment has no Android SDK/emulator (confirmed: no adb devices), so only code-level/structural verification was possible. The app remains `DOCTOR_FLUTTER_NOT_VERIFIED` until a human (or an environment with device access) actually compares each screen's rendered output against the prototype, the same way Queue was confirmed.

The Doctor Flutter app must replicate the canonical source file `prototype/index.html` in every visual and interaction detail. This is a hard rule for all future work: every screen or component change must be checked back and forth between the prototype and the rendered Flutter screen. The required loop is inspect HTML/CSS/JS, render Flutter, compare, correct discrepancies, and compare again. Code inspection, analyzer success, and widget tests do not replace rendered comparison evidence. The app remains `DOCTOR_FLUTTER_NOT_VERIFIED` until every required screen passes this gate.
# HelloDoctor â€” Engineering Progress & Milestone Tracker

> **Document Version:** 1.2.0
> **Status:** M2 (Database v2) and M3 (Auth v2) implemented and verified against a real PostgreSQL instance. M-SECURITY (backend hardening: default-deny auth, ABAC, five-role RLS, rate limiting/IP jail, webhook signatures, N+1 cleanup, load testing) is implemented and verified end-to-end against a live Postgres 16 + Redis instance. Two load-testing passes exist: an unconstrained-hardware pass (`docs/CAPACITY_EVIDENCE_REPORT.md`, `LOCAL_BACKEND_VALIDATION_PASSED`) and a resource-constrained pass (`docs/CONSTRAINED_CAPACITY_TEST.md`) that first found Postgres CPU-saturated at a 2.5-core profile (`LOCAL_CONSTRAINED_CAPACITY_TARGET_FAILED`, ~20â€“30 RPS healthy ceiling), then â€” per explicit instruction â€” reran once with CPU rebalanced toward Postgres within a genuine 2.0-vCPU budget, found and fixed a PostgreSQL JIT-compilation overhead bug along the way, and reached **`LOCAL_CONSTRAINED_CAPACITY_TARGET_PASSED` up to ~200 RPS** (a 7â€“10x improvement), confirming the original failure was substantially a resource-allocation/configuration problem, not an application-design flaw. Zero data corruption and no security control weakened at any stage of either pass. See `docs/PILOT_READINESS_DECISION.md` for the still-standing `NOT_READY_FOR_UNSUPERVISED_1200_USER_PILOT` decision (unchanged â€” the constrained-capacity result is encouraging but still a local-laptop simulation, not a real production VPS run). M4+ (Flutter mobile, Agora client UI, localization, offline guards) remain pending and explicitly not started per user sequencing ("introduce all of these before we touch flutter").
> **Target Execution Agent:** Claude Code Autonomous Engineer
> **Last Updated:** 2026-10-04

---

## 1. Executive Status Dashboard

- **Current Milestone:** M-SECURITY â€” Backend hardening core work and load testing complete; two financial-control deferrals and CI scanning remain before M4 (see Â§3)
- **Overall Completion:** `~37%`
- **Last Completed Task:** Implemented the full backend security architecture requested by the user, frozen as: (1) five least-privilege PostgreSQL capability roles (`hellodoctor_api`, `_auth_worker`, `_payment_worker`, `_system_worker`, `_finance_worker`) instead of one generic app role, with no passwords ever committed to a migration; (2) a default-deny-by-construction Axum auth path (every protected handler takes `AuthContext`, which now gets its claims from a pre-verified source rather than re-parsing the JWT per call site â€” see CHECKPOINT.md for the exact mechanism); (3) ABAC (`abac.rs`) on every protected route; (4) Row-Level Security across all PHI/financial tables, keyed on transaction-local `app.current_user_id`/`app.current_role`; (5) Redis-backed rate limiting + fail2ban-style IP jail; (6) real HMAC-SHA256 payment webhook signature verification plus an event-timestamp freshness check. All of this was run against a real local Postgres+Redis (not just `cargo check`) â€” full patient OTP login, doctor-directory read, and cross-role 403 enforcement were exercised live. That live run caught and fixed two bugs static review could not have found: an RLS circular-policy infinite-recursion bug between `patient_profiles`/`appointments`, and a critical pre-existing JWT verification bug (`jsonwebtoken`'s `from_ed_der` does no DER parsing â€” it needs the raw 32-byte Ed25519 key, not the 44-byte SPKI-DER key `openssl pkey -pubout` produces â€” so every bearer token would have failed verification in production).
- **Current Task:** Constrained-VPS capacity test complete, including the rebalanced rerun (`docs/CONSTRAINED_CAPACITY_TEST.md` Â§12) â€” built from scratch this pass: a real Docker image of the backend (it had only ever run as a bare `.exe`), a resource-capped container stack with limits verified via `docker inspect`, a deterministic ~1,200-user/13,440-appointment synthetic dataset (`scripts/seed_capacity_dataset.sql`), and a new open-loop load generator (`examples/capacity_test.rs`). First pass (Profile A, 2.5 cores): `LOCAL_CONSTRAINED_CAPACITY_TARGET_FAILED`, Postgres CPU-pinned, ~20â€“30 RPS ceiling. Per explicit instruction, reran once with CPU moved from the idle backend/Redis to the saturated Postgres, within a genuine 2.0-vCPU budget this time (all three containers pinned to the same 2 physical cores for real shared-core contention, not just independent quotas) â€” result: **`LOCAL_CONSTRAINED_CAPACITY_TARGET_PASSED` up to ~200 RPS**, a 7â€“10x improvement, with the bottleneck confirmed to have genuinely moved (to the now-smaller backend CPU share at 250 RPS, not Postgres). Two real bugs found and fixed along the way: `get_command_telemetry`'s N+1-at-scale (whole-table fetches that didn't scale past the tiny original dev dataset â€” p50 1,406ms â†’ 317ms) and a PostgreSQL JIT-compilation overhead bug on RLS-protected queries (an `OR`'d correlated-subquery policy inflates the planner's cost estimate past Postgres's JIT threshold even though the expensive branch is never executed â€” p50 265ms â†’ 4ms with `jit=off`, zero change to RLS enforcement or results). Zero data corruption at any stage, no security control weakened. Capacity-tuning work is now frozen, as instructed.
- **Last Completed Task (this pass):** Â§8.2's remaining two-thirds of the observability/accountability follow-on â€” consultation telemetry event timeline (`consultation_telemetry_events`, ordered per-perspective PATIENT/DOCTOR/SERVER events, reconnect-window reconstruction, admin incident-reconstruction endpoint correlating grievances and â€” best-effort â€” system errors) and the admin audit trail (`before_data`/`after_data`/`changed_fields` snapshots on `audit_events`, wired into 8 admin mutations, up from 2; global audit-history read access narrowed from six admin roles to one designated "Super Admin" authority, SECURITY_ADMIN). Full detail: `docs/CHECKPOINT.md` Â§8.2. A real bug was found and fixed live (not just in tests): the new table's `BIGSERIAL` column needed an explicit `GRANT USAGE, SELECT ON SEQUENCE ...` â€” table `INSERT` alone does not cover it, confirmed via a genuine `permission denied for sequence` error on first run.
- **Blocked Tasks:** None.
- **Known Issues:** see `docs/CHECKPOINT.md`, `docs/CAPACITY_EVIDENCE_REPORT.md`, `docs/CONSTRAINED_CAPACITY_TEST.md`, and `docs/PILOT_READINESS_DECISION.md` for the full, current list â€” this section is intentionally not duplicated here to avoid the docs drifting out of sync.
- **Tests Passing:** the latest full baseline is clean: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --lib` (**14/14**), and `cargo test --test api_tests -- --test-threads=1` (**35/35**). The two new migrations (`20261004000002`, `20261004000003`) were also live-verified against the real persistent dev Postgres container, not just ephemeral test databases. `.github/workflows/security-scan.yml` (cargo-audit + TruffleHog secret scanning) is done. Remaining before production hardening sign-off are the explicitly listed payment, step-up, provider, deployment, and operational controls; logout refresh-token revocation is no longer deferred.
- **Active Architecture Decisions:** ADR-001 through ADR-010 approved. The five-role DB capability split and router-level default-deny auth (this session's work) are a direct, explicit user decision overriding an earlier single-role simplification that was never executed â€” see CHECKPOINT.md Â§0 for the full decision record.

---

## 2. Milestone Execution Checklist

| Milestone ID | Description | Status | Verification Check |
|---|---|---|---|
| **SPEC** | Complete 30-Phase Architecture & Engineering Specs | **COMPLETE** | Cross-document stale-term and endpoint consistency checks pass |
| **M0-M1** | Architecture Remediation & Security Model | **COMPLETE** | Security, payment, schema, MFA, RLS, and privacy decisions synchronized |
| **M2** | Canonical State Model & Database v2 | **COMPLETE** | `sqlx migrate run` applies cleanly (6 reversible migrations, each with a `.down.sql`); 13 per-aggregate repository modules replace the in-memory `AppState` |
| **M3** | API v2 & Auth (Ed25519) | **COMPLETE** | Real Argon2id passwords, hashed+rate-limited OTP, encrypted TOTP, Ed25519 JWT with `permissions[]` claim; `cargo test` auth suites pass |
| **M4** | Agora ADR & Implementation Setup | PENDING | Backend RTC-token endpoint exists (`agora_token_service.rs`); Flutter mobile workspace (`apps/mobile/`) not yet created â€” `cargo check` passes, `flutter analyze` N/A |
| **M5** | Specialist Discovery & Star-Rating Elimination | PENDING | Zero star rating widget test passes |
| **M6** | Emergency Contact Helpline & Patient Home | PENDING | Banner position & dialer test passes |
| **M7** | Atomic Slot Booking & Payment Hold | PENDING | Concurrency double-booking and payment-reconciliation tests pass |
| **M8** | Multi-Prescription Upload Engine (Max 5 Images) | PENDING | Cap & dropzone widget tests pass |
| **M9** | Virtual Waiting Room & Device Readiness | PENDING | Pre-flight mock tests pass |
| **M10** | Agora RTC Telehealth & Telemetry Auto-Capture | PENDING | Agora token tests pass |
| **M11** | 24-Hour Clinical Chat Desk | PENDING | 24h lock & WebSocket broadcast tests pass |
| **M12** | Handwritten Prescription Photos & Health Vault | PENDING | Upload-pipeline and cryptographic integrity tests pass |
| **M13** | Doctor Wallet & 20% Debarred Settlement | PENDING | 3-tier arithmetic unit test passes |
| **M14** | Clinical Grievance Redressal & Board Arbitration | PENDING | Dispute & payment refund tests pass |
| **M15** | Central Admin Consoles & Doctor Dossiers | PENDING | Dossier & error log tests pass |
| **M16** | Bilingual Localization (English/Swahili) | PENDING | Language switcher & offline tests pass |
| **M17** | Offline Guards | PENDING | Network guard tests pass |
| **M-SECURITY**| Security Gates & Verification | **CORE COMPLETE, PILOT NOT YET CLEARED** | Default-deny auth + ABAC + 5-role RLS + rate limiting/IP jail + webhook HMAC/provider binding + N+1 cleanup + consultation telemetry/admin audit trail all implemented and live-verified against real Postgres+Redis; the latest baseline is 14/14 library tests and 35/35 serialized integration tests; cargo-audit/secret-scanning CI wired up and clean; load testing remains frozen at approximately 150â€“200 RPS. The explicit decision remains `NOT_READY_FOR_UNSUPERVISED_1200_USER_PILOT` because provider/deployment controls are not complete. Server-side refresh-token logout revocation is implemented; admin step-up re-authentication remains open. |
| **M18** | Final Quality Assurance & Production Verification | PENDING | 100% full regression pass |

---

## 3. Immediate Next Actions for Claude Code

**Do not start Flutter (M4+) work yet** â€” the user has explicitly sequenced full backend hardening (M-SECURITY) before any mobile work begins. CHECKPOINT.md Â§3 items 1â€“10 are all done (System Error Service + consultation telemetry + admin audit trail, the constrained-VPS capacity test, and cargo-audit/secret-scanning CI all completed 2026-10-04). One item remains before M-SECURITY can be called fully signed off:

1. Implement admin step-up re-authentication for sensitive actions (refund, disbursement initiation, MFA reset, and privileged security changes). Server-side refresh-token-family revocation on logout is complete and regression-tested; it is no longer a deferral.

Once that's done (or the user explicitly decides to proceed without it):

3. M4: scaffold the Flutter mobile workspace (`apps/mobile/`) per `docs/FLUTTER_ARCHITECTURE.md` and wire it to the now-real, now-hardened, now-load-tested backend.
4. Add a doctor/admin password-change and TOTP-enrollment flow (today these are only seeded for the fixed demo accounts).
5. Continue M5+ per the roadmap once M4 lands.
6. Update this `docs/PROGRESS.md` document after completing each milestone.
## Final gap-closure pass (2026-10-04)

- Verified the existing MFA, IDOR, upload, wallet/disbursement race, grievance,
  telemetry, audit, and M-Pesa fixes against the current code/test suite.
- Added a payment webhook provider-to-transaction gateway binding check.
- Added server-side refresh-token-family revocation through
  `POST /api/v1/auth/logout`.
- Recorded remaining provider/deployment-dependent controls without claiming
  them complete: payment UNKNOWN reconciliation, expiry worker, currency
  separation, provider refunds/payouts, step-up auth, Argon2 isolation, AV,
  key rotation, retention, backup/restore, and operational alerting.
- Evidence is tracked in `docs/FINAL_SECURITY_GAP_REVIEW.md` and
  `docs/PAYMENT_PRODUCTION_READINESS.md`.

## Final regression and security-control changes (2026-10-05)

### Changes made

1. **Server-side logout/session revocation**

   `POST /api/v1/auth/logout` now hashes the presented refresh token and
   revokes its refresh-token family in PostgreSQL. Browser cookie clearing is
   still performed by the client boundary, but refresh-token invalidation is
   now enforced server-side. Replaying the old refresh token after logout
   fails, while unrelated sessions remain independent.

   **Reasoning:** clearing a cookie is not authentication-state revocation. A
   stolen or replayed refresh token would otherwise remain usable until its
   natural expiry. Family revocation closes that persistence window without
   changing the existing JWT architecture.

2. **Webhook provider-to-transaction binding**

   Payment webhook processing now rejects a signed callback when its gateway
   does not match the transaction's originally selected gateway. The existing
   HMAC, timestamp, replay, amount/state, and idempotency controls remain in
   place.

   **Reasoning:** a valid signature proves possession of a provider secret, but
   does not prove that the callback belongs to this transaction. Binding the
   provider identity prevents a correctly signed event from the wrong gateway
   from mutating payment state.

3. **Regression verification**

   The complete baseline passed:

   - `cargo fmt --check`
   - `cargo clippy --all-targets -- -D warnings`
   - `cargo test --lib` â€” **14/14 passed**
   - `cargo test --test api_tests -- --test-threads=1` â€” **35/35 passed**
   - admin portal `npm run build` â€” **passed**

   The integration suite verified MFA/session behavior, authorization, RLS,
   BOLA/IDOR, webhook replay and provider mismatch, wallet and disbursement
   exactly-once behavior, grievance terminal states, document caps, Agora
   authorization windows, audit, telemetry, and system errors.

   **Reasoning:** these changes affect security and financial state transitions,
   so targeted tests alone were insufficient. The serialized full suite was
   required to detect regressions across shared PostgreSQL roles and Redis.

4. **Test-fixture and scope decisions**

   Integration tests intentionally use `--test-threads=1` because the fixture
   shares cluster-wide PostgreSQL capability roles and Redis state. This is a
   fixture limitation, not an application concurrency limitation. Clean
   temporary databases are created by the `#[sqlx::test]` cases; persistent
   migration verification remains recorded in this document.

   Capacity ladders, long soaks, 1,200-user seeding, and profiling were not
   rerun. The previously established approximately 150â€“200 RPS evidence is
   frozen as directed.

### Current status after this pass

- **Verified complete:** logout refresh-family revocation, webhook provider
  binding, and the complete regression baseline.
- **Still open:** admin step-up re-authentication, payment UNKNOWN-state
  reconciliation, abandoned-payment expiry/slot release, currency separation,
  provider refund/payout execution, Argon2 async isolation, malware scanning,
  production deployment/backup/monitoring, and external provider sandbox
  validation.
- No provider integration or real-money readiness is claimed by this pass.

## Doctor prototype backend contract closure (2026-10-05)

This is the narrow contract-completion pass for the approved Doctor prototype
(Capabilities 1â€“10). The existing Rust/Axum, PostgreSQL, Redis, AuthContext,
RBAC/ABAC, RLS/FORCE RLS, payment, consultation, Agora, audit, telemetry,
grievance, and disbursement architecture was preserved.

### Changes, reasoning, and locations

| Capability | Status | Implementation and reason |
|---|---|---|
| Doctor queue | FIXED_AND_TESTED | `services/backend/src/api/v1/doctor_handlers.rs::list_own_appointments`, routed in `services/backend/src/api/router.rs`, derives queue data from existing appointments/slots and SQL-filters by authenticated doctor; no queue state machine or client `doctor_id`. |
| Duty/availability | FIXED_AND_TESTED | `get_own_profile`/`update_own_duty` use existing `doctor_profiles.is_on_duty`; explicit AuthContext identity prevents cross-doctor mutation. |
| Schedule/slots | FIXED_AND_TESTED | `list_own_slots`, `create_own_slot`, and `cancel_own_slot` reuse appointment slots, validate time/overlap, and conditionally protect booked/locked slots. |
| Appointment list/history | FIXED_AND_TESTED | Paginated SQL-scoped `list_own_appointments` supports existing status filters without whole-table fetches or N+1 queries. |
| Longitudinal patient history | FIXED_AND_TESTED | Migration `services/backend/migrations/20261005000001_doctor_contract_qna.up.sql` adds narrow DB functions. Context is current appointment â†’ authenticated doctor â†’ same patient; history is read-only, paginated, metadata-first, and audited. |
| Chat list | FIXED_AND_TESTED | `list_own_chats`/`GET /doctor/chats` compose existing chat tables, scope by doctor, minimize data, and paginate; HTTP chat remains unchanged. |
| Open Health Q&A | FIXED_AND_TESTED | The migration adds minimum `health_questions`/`health_answers` schema, grants, indexes, and RLS. Doctor list/answer handlers are paginated and AuthContext-bound; Q&A remains separate from private history. |
| Profile/settings | FIXED_AND_TESTED | `DoctorProfileUpdate` is an explicit DTO at `PATCH /doctor/profile`; sensitive role, verification, wallet, security, and payment fields are not mass-assignable. |
| Consultation summary | FIXED_AND_TESTED | `/doctor/consultations/{appointment_id}/summary` composes existing appointment/intake/session data and requires assigned-doctor authorization; no second consultation model. |
| Wallet ledger | FIXED_AND_TESTED | `/doctor/wallet/ledger` is a paginated, read-only, doctor-scoped view of existing settlement transactions; financial truth remains backend-owned. |

### Security/schema decisions

- `services/backend/src/abac.rs` adds `DoctorRead`,
  `DoctorManageOwnSchedule`, `DoctorManageProfile`, and `DoctorQna` while
  retaining default-deny routing, RBAC/ABAC, and RLS.
- The history functions are `SECURITY DEFINER` only as a narrow DB query
  boundary. They explicitly validate doctor ownership, current appointment
  state, same-patient linkage, and requested historical episode; they do not
  expose finance, audit, telemetry, or grievance data.
- `PATIENT_HISTORY_VIEWED` and `PATIENT_HISTORY_EPISODE_VIEWED` audit events
  record access without copying clinical content into the audit log.
- The prototype withdrawal control is intentionally
  `PROTOTYPE_BUSINESS_RULE_CONFLICT`; no withdrawal/cash-out/payout-request
  endpoint was added because Finance Admin owns scheduled disbursement.
- The frozen route/DTO/auth/RLS contract is recorded in
  `docs/DOCTOR_FLUTTER_BACKEND_MAPPING.md`.

### Verification

- Added `test_doctor_contract_endpoints_and_qna_authorization` and
  `test_doctor_patient_history_requires_current_treatment_context` in
  `services/backend/tests/api_tests.rs`.
- `cargo fmt --check` â€” PASS; `cargo clippy --all-targets -- -D warnings` â€”
  PASS; `cargo test --lib` â€” **14/14 PASS**; serialized
  `cargo test --test api_tests -- --test-threads=1` â€” **37/37 PASS**.
- Serial integration is intentional because shared PostgreSQL capability roles
  and Redis fixtures make parallel tests interfere; this is a fixture limit,
  not an application throughput claim.
- Capacity ladders/soaks were not rerun; existing ~150â€“200 RPS evidence stays
  frozen. Payment-provider, deployment, AV, and real-money gaps remain in the
  separate production-readiness documents.

The Doctor backend contract is now frozen for Flutter consumption.

## Targeted Doctor contract correction pass (2026-10-05)

The post-closure review found six concrete correctness/contract defects. This
pass made only the approved corrections.

1. **History pagination authorization â€” FIXED_AND_TESTED.**
   `list_patient_history` now calls the DB authorization boundary before any
   history query, regardless of `offset` or `limit`. Foreign appointments now
   return the same denial at offset 0, 1, and 100. The new history window
   migration also enforces T-15 opening, server UTC time, current-treatment
   validity, and `ended_at/completed_at + 24h` expiry. Historical SQL is cut off
   at the current appointment's scheduled start, preventing later unrelated
   consultations from appearing. Files: `doctor_handlers.rs`,
   `20261005000002_history_access_window.up.sql`, `api_tests.rs`.
2. **Concurrent Q&A answers â€” FIXED_AND_TESTED.** `answer_question` uses
   `ON CONFLICT (question_id) DO NOTHING`; only one contender succeeds and all
   other concurrent contenders deterministically receive 409. The database
   unique constraint remains the final safeguard. File: `doctor_handlers.rs`;
   concurrency test covers 2 and 20 contenders.
3. **Doctor modality filter â€” FIXED_AND_TESTED.** `GET /doctors` now validates
   the existing VIDEO/CHAT enum values and performs parameterized SQL filtering
   using existing consultation fees. Unsupported values return validation
   error; the ignored-parameter pattern is removed. File: `doctor_handlers.rs`.
4. **Duty DTO symmetry â€” FIXED_AND_TESTED.** `GET` and `PUT /doctor/duty` now
   return the same narrow `DutyStatus` DTO and never expose profile fields.
   Files: `doctor_handlers.rs`, `router.rs`.
5. **Verified credential integrity â€” FIXED_AND_TESTED.** Doctor self-service
   profile DTO now accepts only approved non-credential `full_name`; unknown
   fields are rejected. `current_hospital`, `qualifications`, verification,
   role, wallet, and security fields remain ClinicalAdmin-controlled. Admin
   doctor creation validates bounded credential payloads. Files:
   `doctor_handlers.rs`, `admin_handlers.rs`.
6. **Completion timestamp â€” FIXED_AND_TESTED.** Consultation completion now
   sets `consultation_sessions.ended_at` once with server time, making the
   24-hour window stable across retries. Existing appointments without a
   session use the authoritative completion timestamp as a compatibility
   fallback.

Notification delivery was inspected and is not present in the repository. It
is recorded as `DOCTOR_NOTIFICATION_DELIVERY_CAPABILITY_REQUIRED`; notification
delivery is not used for authorization. Patient Q&A creation is recorded as
`PATIENT_QNA_SUBMISSION_REQUIRED_FOR_PATIENT_PHASE`, not optional and not a
product-decision blocker.

Final verification after the correction pass: `cargo fmt --check` PASS,
`cargo clippy --all-targets -- -D warnings` PASS, `cargo test --lib` **14/14**,
and serialized `cargo test --test api_tests -- --test-threads=1` **40/40**.
No capacity ladder, soak, seed, or JIT profiling was rerun.

## Doctor Flutter implementation started (2026-10-05)

Created apps/mobile because no Flutter project existed. Added centralized Dio client, secure token storage, refresh/logout handling, Doctor repository/Cubit, six prototype destinations, consultation/history, Q&A and chat flows. At the time of this initial entry, Flutter execution was pending SDK discovery; the later completion audit below records the actual SDK checks.

## Doctor Flutter verification pass (2026-10-05)

Replaced partial runtime widgets with typed, server-backed Doctor pages and contextual flows. Added camera/gallery prescription upload, Agora token/join/telemetry handling, profile/settings with verified fields read-only, chat thread send, history/detail, Q&A conflict UX, wallet ledger, and 11 recorded tests plus three additional auth/duty tests. Static formatting/analyzer/tests passed before the final session/action guards were added; the Flutter SDK then became unavailable, so no post-change result is claimed. After Docker recovered, the seeded Rust backend was exercised directly for login/MFA, invalid credentials, queue/duty/slots/chats/Q&A/wallet/profile, logout, refresh revocation, and rate limiting. Android runtime and Flutter-against-backend verification remain blocked; status is NOT_READY.
## Doctor Flutter completion audit (2026-10-05)

Implemented and tested at source level: authentication/session storage and revocation handling, queue/duty, schedule, consultation summary/history/detail/completion, chat send, Open Health Q&A answer/conflict handling, camera/gallery/file Smart Rx upload, authorized document opening, read-only wallet/ledger, profile/settings, and mapped loading/empty/retry/error states.

Deferred exactly as allowed: `DEFERRED_AGORA_INTEGRATION` and `DEFERRED_EXTERNAL_PAYMENT_INTEGRATION`. Patient Q&A creation remains `PATIENT_QNA_SUBMISSION_REQUIRED_FOR_PATIENT_PHASE`; prototype withdrawal remains `PROTOTYPE_BUSINESS_RULE_CONFLICT`.

This earlier environment note is superseded by the completion audit below, which records the bundled SDK checks.
## Doctor Flutter completion audit (2026-10-05)

Implemented and tested at source level: authentication/session storage and revocation handling, queue/duty, schedule, consultation summary/history/detail/completion, chat send, Open Health Q&A answer/detail/conflict handling, camera/gallery/file Smart Rx upload, authorized document opening, read-only wallet/ledger, profile/settings, and mapped loading/empty/retry/error states.

Verification: `dart format --set-exit-if-changed .` PASS; `flutter analyze` PASS with zero errors; `flutter test` PASS (27 tests); `flutter run -d edge` PASS. Android APK/device validation is blocked by the missing Android SDK/device. Live Docker/Rust backend evidence is recorded in the correction below.

Deferred exactly as allowed: `DEFERRED_AGORA_INTEGRATION` and `DEFERRED_EXTERNAL_PAYMENT_INTEGRATION`. Patient Q&A creation remains `PATIENT_QNA_SUBMISSION_REQUIRED_FOR_PATIENT_PHASE`; prototype withdrawal remains `PROTOTYPE_BUSINESS_RULE_CONFLICT`.

Final verdict remains `DOCTOR_FLUTTER_NOT_READY` until mobile runtime and real backend exercise are completed.

## Doctor Flutter cache/offline/loading pass (2026-10-05)

`IMPLEMENTED_AND_TESTED`: introduced the single secure, versioned, bounded `DoctorCacheStore` and centralized connectivity service. Queue, schedule, chats, Q&A, wallet/ledger, duty, profile, and consultation summaries use stale-while-revalidate with cached-first rendering, subtle refresh progress, offline/last-updated banners, retry, and pull-to-refresh. Corrupt/old/expired entries fail closed and logout, revocation, or account scope changes clear protected cache.

Offline data is read-only. Mutations require the Rust backend and cannot report success while offline. Longitudinal patient history never becomes offline authorization; document signed URLs and binaries are not persisted. Chat drafts remain `NOT SENT`, Q&A answers remain unsubmitted, and completion/duty/schedule/profile/upload actions are blocked until online.

Verification: `dart format --set-exit-if-changed .` PASS, `flutter analyze` PASS, `flutter test` PASS (27 tests). No real emulator/device offline scenario or real-backend Flutter request is marked `IMPLEMENTED_REAL_BACKEND_VERIFIED` because required services and Android runtime are unavailable. Deferred statuses remain `DEFERRED_AGORA_INTEGRATION` and `DEFERRED_EXTERNAL_PAYMENT_INTEGRATION`; final verdict remains `DOCTOR_FLUTTER_NOT_READY`.

## Doctor Flutter live backend verification correction (2026-10-05)

Docker Desktop was started and the backend stack is now running: PostgreSQL 16 and Redis 7 are healthy, and the Rust service is serving port 8080 with seeded Doctor data. Real requests verified login/MFA, queue, duty GET/PUT, schedule list/create/cancel, chats, Q&A list, wallet summary/ledger, profile, refresh rotation, logout, and post-logout refresh revocation. Flutter Edge also launched against `http://127.0.0.1:8080/api/v1`. Those exercised contracts are `IMPLEMENTED_REAL_BACKEND_VERIFIED`. The only remaining environment validation gap is the physical Android/emulator offline reconnect exercise; no Android runtime is installed. Agora and external payments remain exactly `DEFERRED_AGORA_INTEGRATION` and `DEFERRED_EXTERNAL_PAYMENT_INTEGRATION`.

The actual Flutter `ApiClient` + `DoctorRepository` were additionally run in an ephemeral live smoke test against the same backend; login/MFA, reads, refresh, logout, and session clearing passed. The temporary harness was removed after the run.
## 2026-10-06 — Doctor registration contract

`POST /api/v1/auth/doctor/register` is implemented and covered by a permanent
integration test. It creates a pending, unverified Doctor using the existing
schema and Argon2 credential path; role, verification, admin, wallet, and
payment fields are server-controlled. Existing patient registration and Doctor
login contracts remain unchanged.
## Session checkpoint — 2026-10-06

Status: `IMPLEMENTED_AND_TESTED` for the approved Doctor registration contract
extension; the exact HTML-to-Flutter visual port remains in progress.

Completed:

- Doctor registration endpoint: `POST /api/v1/auth/doctor/register`.
- Pending/unverified Doctor creation with existing Argon2 hashing.
- Unknown privileged registration fields rejected.
- Duplicate identity checks and Redis registration rate limiting.
- Flutter registration form and pending-verification state.
- Flutter bottom navigation corrected to Queue, Chat, Scan Rx, Schedule, Wallet.
- Queue, chat-list, multi-page Rx selection/upload model, and date/start/end slot selection advanced toward the canonical prototype.

Evidence:

- Backend: 14/14 library tests and 41/41 API integration tests passed.
- Flutter: `flutter analyze` passed and 29/29 tests passed.
- Device runtime/screenshot comparison remains pending because this machine exposes Windows and Edge only.

Next session resumes at the Queue screenshot gate, then proceeds screen by
screen through Chat, Smart Rx, Schedule, Wallet, and Q&A.

## Queue exact-port pass — 2026-10-06

Reworked `apps/mobile/lib/features/doctor/doctor_pages.dart::QueuePage` to
follow the canonical `prototype/index.html` Queue hierarchy: Doctor profile and
duty bar, dark Open Patient Q&A banner, scheduled-consultations heading and
refresh control, search field, All/Upcoming/Completed/Free Q&A filter chips,
and rich queue cards with complaint panels plus Call Patient, Chat, and Write
Rx actions. Appointment content remains backend-derived; no prototype patient
records were added. Queue widget expectations were updated to assert the new
canonical labels.

Validation: `flutter analyze` PASS (0 issues); `flutter test` PASS (29/29).
The project has no configured web target in this checkout and no Android
emulator or device is available, so the required rendered screenshot comparison
is still open. Queue status is `IMPLEMENTED_SOURCE_PORT_PENDING_VISUAL_GATE`.

## Wallet payout safety continuation � 2026-10-06

Removed the Flutter raw payout destination form. The wallet now explains that provider authorization is required and does not treat a typed wallet/card identifier as verified. The legacy backend setter remains explicitly classified as `GENUINE_BACKEND_GAP_PROVIDER_PAYOUT_BINDING`; the current backend also lacks a currency-separated server-calculated monthly statement contract, classified as `GENUINE_BACKEND_GAP_MONTHLY_PAYOUT_STATEMENT`. Details are in `docs/DOCTOR_PAYOUT_ARCHITECTURE.md`.

Flutter validation after this pass: `flutter analyze` PASS; `flutter test` PASS (41/41).

## 2026-10-06 - Kenya payout routing backend

Added the Kenya-only (KES) payout destination model, masked/provider-reference storage, primary eligible destination resolution, destination snapshots on disbursements, server-calculated monthly statements, and Finance doctor-specific payout routing. Development binding is server-generated and explicitly simulated. Merchant/provider authorization and execution remain DEFERRED_EXTERNAL_PAYMENT_INTEGRATION.

## 2026-10-06 - Finance state machine and Kenya-only routing correction

The internal finance workflow now distinguishes `PENDING_APPROVAL`, `APPROVED`, `PROCESSING`, `PAID`, `FAILED`, and reconciliation-required outcomes. Batch preparation allocates settled transactions to a disbursement item without changing their payment status, reducing the Doctor wallet, or marking a batch complete. Finance approval is a separate audited transition and explicitly reports `money_moved: false`. The external provider boundary remains `DEFERRED_EXTERNAL_PAYMENT_INTEGRATION`; no local UUID is presented as a provider transaction reference.

A persisted KES financial period and monthly statement snapshot provide controlled month close. Current/open months are estimated; only Finance finalization creates a final statement snapshot. Finance routing is month-filtered and returns Doctor-specific KES payable, adjustment, statement, disbursement, payout-method and masked-destination fields. Payout destinations are snapshotted on prepared items, so a later primary-method change cannot redirect a locked obligation. The allocation index was corrected so one item can aggregate many transactions. Kenyan bank transfer has a distinct gateway value; historical Bangladesh gateway values remain only for backward-compatible old records and are not active Kenya options.

Validation after this pass: `cargo fmt --all`, `cargo check --lib`, `cargo test --lib` (14/14), the disbursement regression subset (2/2), and `npm run build` for the Admin portal. The full serialized integration baseline previously passed 43/43 before the final additive gateway/query changes; the current targeted regression remains green. Flutter CLI is not installed in this environment, so a new APK/analyze run is not claimed here.

Final regression correction: after preserving the legacy BKASH setter solely for backward-compatible historical API tests (active Kenya onboarding still rejects it), the complete serialized integration suite is green again: 43/43. `cargo clippy --all-targets -- -D warnings` also passes.

Mobile verification after the wallet month-selector/status changes: Flutter analyze passed, Flutter tests passed (41 tests), and the ARM64 release build passed with `--dart-define=DEV_PAYOUT_BINDING=true`. Artifact: `apps/mobile/build/app/outputs/flutter-apk/app-arm64-v8a-release.apk` (18.0 MB). The development binding is explicitly simulated and cannot claim a real provider payout.

The legacy `PUT /doctor/wallet/payout-method` compatibility endpoint is development-only and disabled outside development; Kenya production onboarding must use provider-bound payout destinations. Historical BKASH records remain readable solely to avoid breaking old data/tests and are not offered by the active Kenya UI.


## User-directed cleanup and verification ? 2026-10-07

Removed newly added local MinIO compose/configuration defaults, containers, images, object volume and associated Docker build cache. Uninstalled newly added Android emulator and deleted incomplete system-image downloads. PostgreSQL/Redis and their data, existing Android build tools and APKs remain. Development document operations explicitly report storage unavailable until external private S3 is configured; tests use an identified in-memory object-store fixture only.

User approved development-only MFA bypass for now; no real SMS delivery is claimed. Flutter formatting/analyze passed and all 49 tests passed; Rust library 15 passed. Final checks: Rust formatting/clippy passed; API full run 50 passed plus one temporary-database setup failure, with the failed test passing on an isolated rerun (51 distinct API tests passed across runs). Sixteen real local backend HTTP checks passed, including KES/current and previous monthly statements, login/dev MFA, refresh and logout. Backend restarted and left running. Requested cleanup/verification work is complete and stopped; Android runtime remains unverified. Android runtime/visual acceptance is not claimed: no device is attached. The full acceptance audit is stopped at the user's requested cleanup/verification boundary; see DOCTOR_APP_FINAL_SYSTEM_AUDIT.md.
### 2026-10-19 — Economic idempotency, distributed admission and session hardening

Status: IMPLEMENTATION_DELIVERED / ACCEPTANCE_INCONCLUSIVE. Added appointment-level
payment uniqueness/economic identities, bounded origin/auth/upload work, server-verified
web challenges with Admin BFF assertions, Admin SHA256 TOTP, Android session proof and
recovery jitter. The isolated PostgreSQL fixture passed six 100-operation phases
(maximum eight concurrent SQL clients), state-regression checks and economic migration
down/up. Full Rust/Next/Flutter/Android regression and 30,000-client HTTP evidence remain
unverified because suitable isolated application toolchains are unavailable. Real
payment-provider integration and production edge/origin configuration remain required.
Do not mark launch-ready. See [detailed report](SECURITY_MILESTONE_IMPLEMENTATION_REPORT.md)
and [assertions/evidence](ADVERSARIAL_RESILIENCE_RESULTS.md).

### 2026-10-20 — Backend regression gate closed; admission-capacity defect found, fixed and measured

Status: BACKEND_REGRESSION_COMPLETE / OVERALL_NOT_ACCEPTED (unchanged). Completed the
backend half of the stabilization campaign referenced in
`docs/audit_evidence/final_remediation_20261020/`: both single-test `exit_code=101`
failures (`security_acceptance::`, `pool_pressure::`) traced to a stale scratch-evidence
copy of already-fixed test files, not live defects. Found and fixed one genuine
production defect — `Admission::new()` in `services/backend/src/repository/mod.rs` was
sized from one capability-role DB pool instead of the combined capacity across the five
independent per-role pools the application actually runs — then built a dedicated
`#[ignore]`d A/B benchmark (`tests/cases/admission_capacity_bench.rs`) to measure, not
merely assert, that the fix is inert at production's real default configuration and
bounded-safe regardless of input (every per-lane admission ceiling is a hardcoded
constant independent of the capacity argument once reached). Investigated and fixed the
remaining 19 concurrency-sensitive test failures individually; none required further
admission/pool-capacity tuning — root causes included a shared advisory-lock key
serializing supposedly-concurrent grievance submissions, three tests missing the
already-established OTP-delivery-cooldown-expiry pattern, a wrong expected error-code
literal, a fixed non-capacity-derived admission rate shield, and tests whose own
concurrency level exceeded an architecturally-fixed lane ceiling (now batched to that
ceiling without reducing total operations verified).

Full backend `api_tests` suite: 275 passed, 0 failed, 4 ignored (two independent
consecutive runs). `cargo clippy --all-targets -- -D warnings`: 0 diagnostics.
`cargo fmt --all -- --check`: clean. `cargo test --lib`: 76 passed, 0 failed. Zero
assertions weakened, zero tests deleted/skipped, one production source file corrected.
Web/Flutter/Android/distributed-load evidence not touched this round; overall verdict
remains `NOT_ACCEPTED` per
[`FINAL_AUTOMATED_ACCEPTANCE_MATRIX.md`](FINAL_AUTOMATED_ACCEPTANCE_MATRIX.md). Full
per-defect detail: [`FINAL_DEFECT_REMEDIATION_LOG.md`](FINAL_DEFECT_REMEDIATION_LOG.md).
Admission-capacity architecture trace and measurement:
[`FINAL_RESILIENCE_ROOT_CAUSE.md`](FINAL_RESILIENCE_ROOT_CAUSE.md).

### 2026-10-23 — FCM push-notification milestone verified; backend regression gate genuinely closed

Resumed from a large pile of uncommitted work rather than restarting: Stage A
(scheduling/booking, payment-hold expiry/cleanup), Stage B4 (consultation presence
recovery), and a substantial Firebase Cloud Messaging pass were all already present but
uncommitted and never verified end to end. This session's scope was the FCM milestone —
secure backend delivery plus authenticated device-token lifecycle — per explicit
instruction to inspect first, implement only the first coherent milestone, verify, and
stop. Found the milestone's implementation already functionally complete: `FcmTransport`
(HTTP v1, OAuth2 JWT service-account auth, retry/backoff, dead-token disabling) in
`notification_transport.rs`; `notification_scheduler::run_push_delivery_pass` claiming
and delivering due notifications; `POST /notifications/devices` and
`/notifications/devices/revoke` handlers; both Flutter apps' `PushNotificationService`
(firebase_messaging) wired into `app.dart` on sign-in with token registration on
obtain/refresh. Verification, not implementation, was the real work.

**Found and fixed, all previously undetected:**
1. Backend did not compile at all going in — two `appointment_repo.rs` functions lost
   their `Result` return value to a stray trailing semicolon after `.await?;`, and
   `prescription_service.rs` used a transaction after it was already consumed by
   `.commit()`. None of these three are FCM-related; all are now fixed with the function
   bodies returning correctly.
2. `/notifications/devices/revoke` and the entire FCM delivery scheduler's
   claim/send/retry/permanent-failure state machine had zero test coverage. Added
   `services/backend/tests/cases/push_notification_delivery.rs`
   (`test_revoke_device_endpoint_is_owner_scoped`,
   `test_push_delivery_pass_sent_permanent_and_retry_paths`, the latter using a
   scripted/fake `NotificationTransport` so no real network call to FCM is ever made).
3. `20261113000001_payment_hold_cleanup` silently dropped `NEW_CHAT_MESSAGE` — the actual
   notification type `chat_service.rs` and both Flutter apps' `local_notification_service.dart`
   have always used — from the `notifications_type_check` constraint, replacing it with an
   unused `CHAT_MESSAGE` placeholder. **Every chat message send had been returning 500 since
   that migration landed**, invisible until this session's full regression run actually
   exercised chat. Fixed forward-only in `20261118000001_restore_new_chat_message_notification_type`.
4. `hellodoctor_payment_worker` was never granted `SELECT` on `doctor_profiles`/
   `patient_profiles` or `EXECUTE` on the `doctor_own_encounter_allowed` SECURITY DEFINER
   function — both needed transitively by RLS policies the new hold-expiry-cleanup path
   (`expire_locked_hold`, used by both the background sweep and the reclaim-on-booking
   path) touches. The new "purge clinical records on expiry" feature failed closed with a
   raw `permission denied` error. Fixed forward-only in
   `20261118000002_grant_payment_worker_profile_reads`.
5. `patient_profiles` never got the `doctor_profiles`-equivalent unconditional
   `hellodoctor_system_worker` read policy (`doctor_worker_read` exists; no
   `patient_profiles` counterpart did). The notification scheduler's bare-pool queries
   (`&state.system_db`, no RLS session variables ever set) silently saw zero patient
   rows — Patients were never actually receiving `BOOKING_HOLD_CREATED`/
   `BOOKING_HOLD_EXPIRED` notifications, with no error at all, just an empty result set.
   Fixed forward-only in `20261118000003_patient_profiles_system_worker_read`.
6. Four SQL string-literal bugs in `notification_repo.rs`: multi-line backslash-continued
   query strings missing the trailing space before the backslash, merging adjacent SQL
   tokens (`...notifications\` + `WHERE...` → `notificationsWHERE`; same pattern in
   `claim_due_notifications` and all three `mark_delivery_*` functions). Caught only
   because this session's new test was the first thing to ever execute these queries.
7. Documented a real `sqlx::migrate!` gotcha directly in `migration_status.rs`: the
   compile-time embedded `MIGRATOR` does not reliably re-scan `./migrations` just because
   a new file was added to disk — the file containing the macro invocation must itself be
   content-changed to force re-expansion. Cost three `VersionMissing` test failures before
   diagnosed; now documented in place so it isn't rediscovered the hard way again.
8. A background research agent, asked only to watch a background test run and report
   back, instead launched its own concurrent `cargo test` run against the same shared dev
   Postgres cluster without being asked, corrupting sqlx's test-database bookkeeping and
   producing a false 300+/316-failure signal before the pattern was caught and the agent
   stopped. Recorded in this assistant's cross-session memory as a new instance of the
   fork-scope-creep pattern (the agent exceeded even a "just wait" instruction).

**Final verified baseline:** `cargo fmt --all -- --check` clean; `cargo clippy
--all-targets -- -D warnings` clean, zero warnings; `cargo test --lib` 84/84; full
serialized `cargo test --test api_tests -- --test-threads=1` **312/312 passed, 0 failed,
4 ignored** (ignored = explicit A/B capacity benchmarks), run against a disposable,
explicitly-owned test Redis fixture (`TEST_REDIS_URL=redis://127.0.0.1:16380/<db>`,
`TEST_REDIS_FIXTURE_ID`), never the shared dev Redis on 6379. Doctor Flutter
(`apps/mobile`): `flutter analyze` clean, 274/274 tests pass. Patient Flutter
(`apps/mobile_patient`): `flutter analyze` clean, 277/277 tests pass. Neither Flutter
app's source was modified this pass — both reverified only, confirming the
already-completed FCM client wiring still holds.

**Explicitly still open, not started this pass:** bilingual (English/Swahili)
notification template content — `notification_templates`/`notification_type_registry`
exist with seeded type metadata but zero localized copy; every current notification body
is hardcoded English only, with no locale selection anywhere in the creation path.
Quiet-hours and per-type daily frequency-limit enforcement — the columns exist on
`notification_type_registry`; nothing in `notification_scheduler.rs`/`notification_service.rs`
reads them yet. Holiday/birthday/welcome/health-education/admin-campaign notification
triggers are not implemented (only their type-registry rows exist). Android
`google-services.json`/`GoogleService-Info.plist` and the Google Services Gradle plugin
are not wired into either app's native build (expected — these are deployment secrets per
`docs/FCM_SETUP.md` — but the Gradle plugin application itself, guarded for when a
config file is supplied, is also not present yet). The persistent/shared dev Postgres
database is confirmed several migrations behind current (`appointment_expiry_cleanup_jobs`
does not exist there) — restart the dev backend and run
`scripts/verify_backend_schema.ps1` before trusting any live check against it, per
CLAUDE.md §4 migration discipline. The repository-wide git cleanup (94 changed/untracked
files, several scratch/audit-evidence directories) has not been started.
## Backend regression remediation — 2026-10-24

The remaining eight Milestones 4–5 regression failures were resolved without changing
production safeguards. Five registration fixtures were updated from the obsolete legal
document version `1.0` to the current `1.0-afyamkononi` only in successful-registration
paths; existing stale-version rejection coverage remains. The grievance-number test now
runs all 40 submissions in bounded four-way batches, matching the deliberately small
two-connection test pool while retaining successful-persistence and uniqueness assertions.
The AfyaMkononi down-migration was corrected to clear rebrand current flags before restoring
the prior current documents.

Fresh isolated Postgres 16/Redis 7 fixtures were used on ports 55432/16479; shared
development services on 5432/6379 were left untouched. Fresh migration application,
rebrand rollback/reapply, the two migration regressions, the focused registration/grievance
filters, and notification-focused filters passed. The full serialized backend suite passed
**349 tests, 0 failed, 4 ignored**. Final Rust checks passed: `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, `cargo check --all-targets`, and
`cargo test --lib` (**84 passed**). Existing Flutter verification remains Doctor
267/267 with clean analysis and Patient 295/295 with clean analysis; no new Flutter run was
needed for this backend-only remediation. No Milestone 6 work began.

Files changed by this remediation: `services/backend/tests/api_tests.rs`,
`services/backend/migrations/20261124000002_rebrand_afyamkononi.down.sql`,
`docs/PROGRESS.md`, and `docs/CHECKPOINT.md`. The worktree remains intentionally dirty and
Git history was not modified. Native Firebase credentials/device delivery remain external
configuration limitations. The complete backend regression gate is **GREEN**.

## Milestone 6D — Admin Blog/CMS — 2026-10-25

Focused read-only inspection preceded implementation. Added a separate Admin article
schema with FORCE RLS, bilingual categories/content, exact-version approval,
independent review for sensitive metadata, guarded publication/scheduling and audit
events. Doctor articles and their policies/publication behavior remain intact. The
existing Patient article endpoints now combine published Doctor and Admin content;
the existing Flutter Blogs tab supports Editorial identity, EN/SW fallback, safe
Markdown and authenticated covers. Admin Blog Manager adds modular authoring,
local smartphone preview, secure covers and review/publication controls. Publishing
does not send notifications or create campaigns. No MCQs or advanced Patient browsing
were added. Existing Milestone 6C code and legitimate uncommitted work were preserved.

Verified: Rust format, Clippy with warnings denied and compilation; **87 library
tests**, **14 focused CMS integration tests**, full serialized backend **379 passed,
0 failed, 4 existing benchmarks ignored** (383 total cases; native exit 0). Admin:
**76 tests**, typecheck, production build and ESLint with 0 errors/1 existing Campaign
Manager warning. Patient: clean analyze and **308 tests**. Installed-Edge checks:
3 static responsive layouts and 10 interactive checks with synthetic transport,
including local preview isolation, payloads, covers and integrated small-screen layout.

Acceptance gate is **PARTIAL**, not live cross-app acceptance: the authenticated
Admin BFF → same persisted record → running Patient app walkthrough and real external
object-store round trip remain unverified. Existing Next.js security advisories also
remain; the first build stopped on its advisory, and the retry passed. Shared databases
were not reset/migrated and no deployment or Git history action occurred. Full exact
file list, endpoints, migrations, security details and evidence:
[Milestone 6D report](MILESTONE_6D_REPORT.md).

### Milestone 6D stabilization — 2026-10-25

Applied independent-approval UX and readable CMS errors without weakening backend
review rules. Failed cover uploads retain the saved text draft and cannot show
upload success. Patched Next/eslint-config-next to **16.4.3** and transitive Zod to
**4.6.7**; audit now has no critical/high entries and three moderate Tailwind/PostCSS
entries pending separate review. Admin **83 tests**, typecheck/lint/build pass;
Rust gates/**87 library tests**, focused CMS **14**, Flutter analyze/**308 tests**,
synthetic browser **11**, layouts **3**, live anonymous smoke **6** pass.
Initial full backend run had **378 passed, 1 database-acquisition timeout, 4 ignored**;
the cleanup case passes alone. The full repeat without competing builds passed the
14 CMS cases, then was stopped at the user's request. It is incomplete; gate PARTIAL.

**DEFERRED — AWAITING AZURE BLOB INTEGRATION.** Legacy storage configuration remains
intentionally empty. No S3 workaround, Azure implementation or new infrastructure.
Live seeded Admin sign-in returned 401; one live Admin and unavailable Patient
runtime also prevent full same-record cross-app acceptance. No account reset or
extra live Admin was created. Details and exact evidence:
[stabilization report](MILESTONE_6D_STABILIZATION_REPORT.md),
[planning-only Azure integration](AZURE_BLOB_INTEGRATION_PLAN.md).

### Milestone 6E — optional Admin article MCQs — 2026-10-25

Added optional inclusion, modular bilingual question authoring/reordering/deletion,
CSV/JSON validation/preview/atomic import and local interactive quiz simulation in
the existing article editor. Approved immutable quiz revisions are separate from
drafts, protecting published articles and previously approved quizzes. Sensitive
quiz review is independent and binds the exact quiz/article content versions.
Patient article responses contain only safe publication metadata; no answer keys,
attempts, scoring, Flutter quiz renderer or new navigation.

Verification: Rust format/Clippy/compile pass; library **91**, MCQ integration **9**,
existing CMS **14**, Admin **93** tests pass; Admin typecheck/lint/build pass
(0 lint errors, 1 existing Campaign warning); responsive layouts **3** pass.
No full backend rerun: preserve **378 passed, 1 timeout, 4 ignored** and the prior
interrupted repeat. Full regression remains a coordinated future gate.
Azure stays intentionally deferred; no dependencies, cloud configuration, production
deployment or Git history operations. See [6E report](MILESTONE_6E_REPORT.md).

### Milestone 6F — Patient interactive quizzes — 2026-10-25

Implementation present: restricted Patient question-read/answer-check functions and
APIs; optional quiz in the existing BlogDetailPage; local-only scoring and progress,
feedback, explanations, restart, EN/SW and stale-revision refresh. No attempts or
profiles persisted, no answer-selection analytics, no Admin rebuild or Azure work.
Flutter format/analyze and **326 tests** pass; Rust format/Clippy/compile and **91
library tests** pass. Focused Patient quiz **10**, Admin quiz **9**, CMS **14** pass.
One serialized full backend run: **398 passed, 0 failed, 4 existing benchmarks
ignored**, native exit 0, **1143.61 seconds**, within its 35-minute cap. The prior
timeout did not recur; its evidence and unchanged deadline/pool settings remain.
No overlap or full-suite retry. Automated gate GREEN; real device/live cross-app
verification remains unavailable. Azure and GitHub migration were not started.
Stop at 6F. Details: [6F report](MILESTONE_6F_REPORT.md).
