# HelloDoctor - Current Engineering Progress

**Status reconciled:** 2026-10-08. This is the authoritative current status; dated records below are history. No overall percentage is claimed because implemented code, automated verification, device acceptance and production deployment are different gates.

## Latest verified baseline

| Component | Result | Evidence / scope |
| --- | --- | --- |
| Rust formatting | PASS | `cargo fmt --check` |
| Rust lint | PASS | `cargo clippy --offline --all-targets -- -D warnings` |
| Rust library | **50/50 PASS** | `cargo test --offline --lib` |
| Backend integration | **139/139 PASS** | Full `api_tests`, serialized with `--test-threads=1`; zero failures |
| Dart formatting / Flutter analysis | PASS | Format clean; `flutter analyze --no-pub` clean |
| Flutter tests | **98/98 PASS** | `flutter test --no-pub`; includes 17 Q&A layout/state/golden tests |
| Doctor Web | **44/44 PASS; typecheck/build PASS** | Last verified in the security/Articles pass; not rerun for the later backend/Q&A correction |
| Admin Web | **13/13 PASS; typecheck/build PASS** | Last verified in the security/Articles pass; not rerun for the later backend/Q&A correction |
| Migration agreement | **VERIFIED: 20261026000001** | Source = running binary = persistent development DB at the latest verification; all 34 applied UP checksums match |
| Historical migration integrity | PASS | 62 saved historical migration files unchanged |
| Latest live development checks | PASS | Synthetic login, private-profile isolation, canonical trace linkage, denied trace access, session/request audit metadata |
| Physical-device verification of latest Q&A fix | **PENDING_NO_DEVICE** | No Android device connected; installed phone APK was not replaced |

These are the latest recorded checks, not a new test run for this documentation update. Counts from earlier dated sections are historical. Automated regression and widget goldens do not constitute a penetration test or physical-device acceptance.

Primary evidence: [ownership/Q&A results](audit_evidence/user-isolation-qna.json), [security/Articles results](audit_evidence/public-exposure-security.json), [ownership/Q&A report](USER_ISOLATION_AND_QNA.md).

## Current implementation status

| Area | Current status | Remaining boundary |
| --- | --- | --- |
| Canonical identity / ownership | **USER_LEVEL_DB_ISOLATION_VERIFIED** | `users.id` is the security identity; unique FK-backed Patient/Doctor mappings, role/profile consistency, composite ownership FKs and pooled SET LOCAL context tested |
| Database isolation / traceability | **VERIFIED** | All 40 tables inventoried; 35 FORCE RLS tables and five documented pre-auth/infrastructure exceptions; five capability roles have no BYPASSRLS or table ownership |
| Controlled Compliance trace | **IMPLEMENTED / AUDITED** | COMPLIANCE and PLATFORM_ADMIN only; stable internal IDs, required reason, session/request audit, fail-closed audit persistence; no unrestricted Admin DB access or formal legal export workflow |
| JWT/session lifecycle | **IMPLEMENTED / REGRESSION GREEN** | Immediate access-token invalidation on current-family logout; other sessions preserved; atomic refresh rotation/reuse revocation and single-use Patient OTP tested |
| Protected API / cache boundary | **IMPLEMENTED / REGRESSION GREEN** | JWT, ABAC, ownership and RLS retained; subject/authorization/query-scoped idempotency cannot replay private data ahead of downstream checks |
| Public-exposure application guards | **IMPLEMENTED; DEPLOYMENT PARTIAL** | Explicit environment, production seed/dev-OTP prohibition, trusted HTTPS ingress checks, protected production metrics, blocked production simulation, Redis failure = 503 rather than fail-open |
| Kenya professional review | **KENYA_PROFESSIONAL_VERIFICATION_VERIFIED** | KMPDC/KE; explicit PENDING/VERIFIED/REJECTED; pending-only queue; atomic verify/reject with one race winner; boolean compatibility derived; internal manual review, no external registry integration |
| Scheduling / cross-portal availability | **IMPLEMENTED / VERIFIED** | Doctor owns normal slot management; authorized Admin emergency operations; patient availability API excludes unavailable slots; visible online portals refresh every 15 seconds plus request latency |
| Scheduling navigation / reported entries | **COMPLETE** | Dedicated Doctor Web Schedule & Slots; two specifically authorized unbooked entries cancelled and audited; unrelated three proposed historical corrections remain unapplied |
| Slot approval | **NOT_IMPLEMENTED_BY_PRODUCT_DESIGN** | Explicit user policy: no approval workflow; auditing does not require approval |
| Doctor Articles | **IMPLEMENTED** | Structured Doctor Web + Flutter authoring and direct publication by verified Doctors; no Admin review; published patient feed is API-only |
| Queue / notification domain | **BACKEND IMPLEMENTED** | Upcoming-first queue, owned inbox APIs, deduplicated reminders/daily summaries and lifecycle notifications; dedicated Flutter notification bell/inbox UI remains unimplemented; real push provider configuration required |
| Startup incident reporting | **IMPLEMENTED / VERIFIED** | Pre-initialization incident journal, authenticated delivery and Admin reference/build correlation; updated device build still needs installation/acceptance |
| Doctor Flutter Open Health Q&A | **DOCTOR_QNA_CODE_FIXED_REAL_DEVICE_PENDING** | Dedicated flow layout, flexible long content, keyboard-safe editor, answered list, authoritative 409 recovery and eight goldens verified; actual phone rendering still pending |
| Doctor Web / Doctor Flutter core workflows | **IMPLEMENTED; ACCEPTANCE PARTIAL** | Existing queue, chat, Smart Rx, schedule, wallet, profile/settings and Articles; user previously confirmed Doctor app acceptance, which does not verify the latest Q&A build |
| Patient client / complete patient journey | **NOT_IMPLEMENTED / NOT_VERIFIED** | No patient-facing Flutter client exists; synthetic backend journey passes, but patient UI and device journey are not complete |
| Private clinical-document storage | **IMPLEMENTED; DEPLOYMENT ACCEPTANCE OPEN** | Private storage configuration and successful deployed upload/download acceptance remain required; earlier live development uploads were blocked by missing storage configuration |
| Seven endpoint coverage gaps | **CLOSED / REGRESSION GREEN** | System-error simulation, compliance alerts, audit simulation, pending verification queue, aggregate telemetry, metrics and finance month-filter tests are in the current API suite |
| Upload compression / field validation | **IMPLEMENTED / REGRESSION GREEN** | Centralized 400 KiB batch ceiling, server-side field limits, email/phone validation and password rules; no longer deferred |
| Newtype identity refactor | **COMPLETE** | Distinct UserId / PatientProfileId / DoctorProfileId types; schema and wire UUID representation preserved |
| Machine-generated OpenAPI | **DEFERRED / NOT_IMPLEMENTED** | Handwritten API/client contracts remain; no utoipa/schemars retrofit or dependency installation |
| Current Azure hosting proposal | **PROPOSED / NOT_DEPLOYED_OR_LOAD_VERIFIED** | User will handle hosting; no infrastructure changes or capacity guarantee recorded |

## Current milestone coverage

| Milestone | Current status |
| --- | --- |
| SPEC / M0-M1: architecture and security model | Implemented foundation; current product decisions override historical spec assumptions |
| M2: canonical database / ownership | Implemented and regression/live verified through migration 20261026000001 |
| M3: API / authentication | Implemented and regression verified; sensitive Admin step-up remains open |
| M4: Flutter / Agora setup | Doctor Flutter workspace and RTC client exist; live media/device acceptance remains open |
| M5: specialist discovery | Backend directory exists; patient client acceptance incomplete |
| M6: patient home / helpline | Patient client not implemented |
| M7: booking / payment hold | Backend atomic booking and synthetic payment journey tested; real provider acceptance open |
| M8: prescription/intake uploads | Backend and Doctor workflows implemented; complete patient UI acceptance open |
| M9: waiting room / device readiness | Doctor queue exists; full patient waiting-room/device journey not verified |
| M10: RTC / telemetry | Backend tokens/telemetry and Doctor RTC client exist; real-device/provider verification open |
| M11: 24-hour clinical chat | Backend/Doctor workflows implemented; full patient client journey open |
| M12: prescription / health vault | Backend/Doctor prescription workflows implemented; patient health-vault UI and lab-report model remain gaps |
| M13: Doctor wallet / settlement | Internal ledger and Doctor views implemented/tested; live disbursement/provider readiness separate |
| M14: grievances / arbitration | Backend/Admin workflow and synthetic tests exist; live refund/provider readiness separate |
| M15: Admin / dossiers | Implemented; controlled audited trace added; step-up remains open for sensitive operations |
| M16: English/Swahili | Full bilingual product acceptance not verified |
| M17: offline guards | Doctor safe cached-read/error behavior exists; complete patient/offline acceptance open |
| M-SECURITY: application security | Latest defined regressions green; external production gates and step-up remain open |
| M18: final production acceptance | OPEN; not equivalent to unit/integration tests passing |

## Remaining work and release gates

1. **Sensitive Admin step-up authentication remains open** for financial/security operations. Existing authorization is not recent reauthentication.
2. **Latest Q&A real-device render/acceptance remains pending.** Connect an existing Android device and verify the corrected build against `prototype/index.html` before claiming the fix visually accepted. Widget goldens are separate evidence.
3. **Patient client and complete patient journey remain unfinished.** Patient APIs and synthetic backend tests do not replace patient UI/device/provider acceptance.
4. **External deployment/provider acceptance remains open:** approved TLS termination and production configuration, production OTP/SMS delivery, private clinical-document storage, real push delivery, live Agora/media acceptance, and payment/refund/payout provider validation. Purchasing Azure services does not integrate them automatically.
5. **Operational release evidence remains open:** off-host backups and restore drill, deployed monitoring/alerts, production secrets/key management, retention and the payment-production readiness checklist. Current Azure load/soak testing has not been performed; earlier local capacity results predate the latest session/RLS changes.
6. **Scoped product/quality gaps remain:** dedicated Flutter notification inbox/bell UI, lab-report endpoint/model, generated OpenAPI, and formal legal export workflow. These are recorded gaps, not authorization to reopen them in this documentation pass.

Overall release status: **PUBLIC_EXPOSURE_SECURITY_GATES_PARTIAL**. Controlled synthetic-data development/testing is supported; unrestricted clinical production, real-money operation and an unsupervised 1,200-user pilot are not declared ready. No evidence-based overall completion percentage is assigned.

See [public-exposure controls](PUBLIC_EXPOSURE_HARDENING.md), [deployment checklist](PRODUCTION_DEPLOYMENT.md), [Articles policy](DOCTOR_ARTICLES.md), [ownership inventory](USER_OWNERSHIP_INVENTORY.md), and [scheduling evidence](SCHEDULE_SYNC_FOLLOWUP.md).

## Current scope and resume rules

- This update changes documentation only. No code changes, deployments, service starts, package installs or host configuration changes are included.
- The user will handle the proposed Azure hosting setup. Hosting is not marked deployed, integrated or performance-verified.
- Do not re-run completed Articles, Kenya review, ownership-isolation or security implementation solely because an archived entry says "not started" or "resume here".
- Do not add slot approvals or Article Admin review. Preserve current five-role DB architecture, ABAC, RLS, session boundaries and Kenya-only professional review.
- Apply new SQLx migrations for future schema changes. Never edit applied migration history or manually modify `_sqlx_migrations`; B-01 source/binary/database guards remain required.
- No further implementation is automatically queued by this status reconciliation. Future work requires the user's next instruction; record pending acceptance and release gates accurately.

## Historical implementation log

Everything below this boundary records earlier passes. Old test counts, migration versions, agent rosters, environment descriptions, interrupted states and resume instructions are historical, not the current task list. The status tables above supersede them. Historical audit findings are retained rather than rewritten as if later fixes had already existed.

## Pre-Patient capacity benchmark + grievance-number concurrency fix — 2026-10-28

Ran a manually-driven open-loop RPS ramp (25→500+ RPS, 11 stages, 6 min each,
≈67 min total) against an isolated `docker-compose.capacity.yml` stack
(`hellodoctor-capacity-*`, never touching the real dev containers), reusing
the existing `examples/capacity_test.rs` harness and
`scripts/seed_capacity_dataset.sql` dataset (1150 patients, 40 doctors, 10
admins). Two Docker CPU models were tried: a CFS-quota ("burst") profile,
which collapsed catastrophically above ~400 RPS (21%+ true system error
rate, p50 climbing into multi-second territory — later understood to be a
Docker throttling artifact, not a real application limit), and a
`cpuset`-pinned/`cpu_shares`-based ("stable", non-burstable) profile, which
handled the entire 25–500+ RPS range cleanly (true system error rate
0.00–0.04% throughout, p50/p95 staying in single-to-low-double-digit
milliseconds, peak CPU never exceeding ~1.3 of the 4 pinned cores). No real
capacity ceiling was found under the stable profile within this range.
Raw evidence: `docs/audit_evidence/pre_patient_benchmark_4x8/`.

**Real bug found by the benchmark, not by inspection**: `POST /grievances`
threw genuine HTTP 500s (duplicate-key violations on
`grievance_reports_grievance_number_key`) starting at 50 RPS, growing with
load (96 total across the full stable-profile ramp). Root cause: the
grievance number was built from the first 4 hex characters of a fresh UUID
(16 bits of entropy) per calendar day — a probabilistic scheme that collided
under genuine concurrency. Fixed via migration `20261028000001`: a Postgres
sequence (`grievance_number_seq`) backs the column's own `DEFAULT`
(`GRV-YYYYMMDD-00000123`-shaped, zero-padded), atomic under concurrency by
construction, with gaps accepted as the normal cost of human-facing
reference numbers (not solved by widening the random suffix, which stays
probabilistic). `grievance_repo::insert` no longer accepts a caller-supplied
number; it returns the one the database generated via `RETURNING`.

Verified three ways: (1) 500 concurrent `pgbench` clients directly against
Postgres — 0 failures, 1200/1200 distinct numbers; (2) a new permanent
regression test, `test_grievance_number_unique_under_concurrent_submission`
(40 concurrent `GrievanceService::submit_grievance` calls, asserts all
succeed with distinct numbers); (3) the full suite — **fmt clean, clippy
clean, 50/50 lib, 143/143 integration** (142 prior + the 1 new test) — run
against both the capacity stack and the real dev backend, with
`verify_backend_schema.ps1` confirming source == binary == database ==
`20261028000001` on the real dev backend afterward.

No other error type appeared anywhere in the backend logs across the full
67-minute, 11-stage run — every other endpoint (`GET /doctors`,
`/doctors/:id/slots`, admin telemetry, wallet summary, appointment booking,
chat, rtc-token, telemetry, webhook confirm) stayed at 0% system errors
throughout.

**Caveats, stated plainly**: this is a local Docker/WSL2 benchmark on desktop
hardware (AMD Ryzen 5 5600), not a real Azure B4ls_v2/D-series VM — CPU
microarchitecture, virtualized networking/storage, disk IOPS, and
multi-tenant noisy-neighbor effects all differ from the real target
environment. Treat throughput/latency numbers as directional engineering
signal (where the architecture bends, which component saturates first,
correctness under load), not as a production capacity guarantee. That
requires the real Azure staging run the original benchmark spec itself
defers to.

## Doctor registration legal/consent gate — verified — 2026-10-27

Added a mandatory Legal & Professional Agreements step to Doctor
registration, per [ADR-011](ADR/ADR-011-legal-consent-ledger.md). New tables
`legal_documents` (server-owned registry, current/historical versions,
hash recomputed by a Postgres trigger, never client-supplied) and
`legal_acceptances` (immutable, append-only; no role is ever granted
`UPDATE`/`DELETE`) via migration `20261027000001_legal_acceptances`.
`POST /auth/doctor/register` now requires `accepted_documents: [{type,
version}]` covering every currently required document (`DOCTOR_TERMS`,
`PRIVACY_NOTICE`, `PROFESSIONAL_VERIFICATION_DECLARATION`,
`TELEMEDICINE_PRACTITIONER_POLICY`), with `MARKETING_COMMUNICATIONS`
optional; both a handler-level check and a DB trigger independently reject a
missing or non-current document, so a direct API call cannot bypass it.
Acceptance rows are written inside the existing registration transaction, so
the account and its acknowledgements land atomically. New public
`GET /legal/documents`, plus protected `GET /doctor/legal/acceptances` and
`GET /admin/legal/acceptances/{user_id}` (new `abac::Action::ViewLegalAcceptances`,
Compliance/PlatformAdmin/ClinicalAdmin).

Flutter: registration's existing single form now navigates to a new
`LegalConsentPage` (`features/auth/legal_consent_page.dart`) on "Continue"
instead of submitting directly — four required checkboxes (unchecked by
default) with a "View" reader for each document's live-fetched text, an
"Optional" marketing checkbox, and a "Submit application" button disabled
until all required boxes are checked. The account is only actually created
from this screen.

Seeded v1.0 document text is a reasonable draft grounded in Kenya ODPC
guidance (cross-border/Azure processing disclosure, KMPDC verification
authorization, telemedicine confidentiality) — explicitly **not
lawyer-reviewed**; must be replaced with legal/compliance-approved copy
before production launch.

Out of scope this pass (flagged, not silently dropped): forcing re-consent
at next login when a document version is bumped (the data model supports
detecting staleness; no login-time gate/UI is built yet), and a dedicated
marketing-consent revoke endpoint.

Verification: **cargo fmt clean, cargo clippy --all-targets -D warnings
clean, 142/142 backend integration tests PASS** (`cargo test --test
api_tests -- --test-threads=1`, includes 3 new tests covering the 10 named
scenarios from the spec plus the existing registration test updated for the
new required field). **Flutter: analyze clean, 98/98 tests PASS**. Schema
verified via `verify_backend_schema.ps1`: source == binary == database ==
`20261027000001`, confirmed against the actual running dev backend (restarted
after the migration landed, per this project's own migration-discipline
rule). Live-checked `GET /legal/documents` against the running dev backend
and confirmed it serves the seeded documents. Physical-device walkthrough of
the new consent screen itself is pending the user's own test pass.

## Kenya professional registration - verified - 2026-10-08

Professional registration now uses KMPDC/KE and explicit PENDING / VERIFIED /
REJECTED states. The Admin queue filters PENDING in SQL. ClinicalAdmin/PlatformAdmin
review commits state, metadata, audit and notification together; concurrent
verify/reject yields one success and one 409. The compatibility boolean is derived.
Account suspension, duty, MFA, payout readiness and sessions remain separate.
Doctor Web, Flutter and Admin use the new contract.

Migration 20261024000001 was applied through normal startup. Source, binary and
database agree; all 62 pre-existing SQL files retain their hashes. Exactly two
proven synthetic seed Doctor profiles and four related seed phone fields were
corrected. Unknown records and scheduling/finance data were left alone. Two new
clearly synthetic live-review Doctors remain off duty and have no slots.

Final sequential Rust checks: **fmt/clippy PASS, 50/50 library, 113/113 API**.
Flutter: **analysis clean, 74/74 tests**. Doctor Web: **typecheck/source guard clean,
33/33 tests, build PASS**. Admin: **typecheck clean, 4/4 tests, build PASS**.
The live walkthrough passed **14 checks, zero browser exceptions**, covering both
terminal review paths, refreshed queue exclusion and KMPDC/status display. The user
confirms the Doctor app is verified. No software or dependencies were installed.

This is internal manual registration review, with no claim of official registry
confirmation or current annual practice licensure. Details and requested status
fields: [Kenya verification report](KENYA_PROFESSIONAL_VERIFICATION.md).

## Dedicated Doctor scheduling navigation - verified - 2026-10-08

Added Schedule & Slots immediately before Appointments in the Doctor Web sidebar.
It opens availability and the Add Schedule Slot action directly, has its own active
state, and supports desktop/narrow-screen navigation and browser history. No
appointment/date shortcut is needed to reach slot creation. TypeScript,
source-safety/format, 32 web tests and production build pass. The isolated browser
walkthrough now passes 13 assertions with zero browser exceptions and cleans up its
owned database/processes/Redis data. Doctor Web restarted on port 3001.
Details: [scheduling follow-up](SCHEDULE_SYNC_FOLLOWUP.md).

## Scheduling follow-up - verified - 2026-10-08

Admin and Doctor Web now use the same EAT scheduling clock and expose schedule
freshness/errors. Admin schedule polling/mutations are independent of other
dashboard reads; Doctor appointments poll alongside availability. Two reported
unrecognized availability entries were confirmed unbooked and cancelled with audit
snapshots after the user's explicit choice. Their historical creators are unknown.
Verification: 32 Doctor Web tests, 2 Admin time-zone tests, both production builds,
and 11 isolated browser assertions pass with zero browser exceptions. The final
read-only browser check confirms both actual entries are CANCELLED in both portals.
All browser
mutation fixtures used an owned throwaway database, which was removed afterward.
Details: [scheduling follow-up](SCHEDULE_SYNC_FOLLOWUP.md).

## Startup reporting and schedule synchronization - verified - 2026-10-08

Startup failures are journaled before API/session initialization and delivered
after Doctor authentication, with reference/build visible to Admin. Admin blocks
now use BLOCKED; Doctor Web and mobile refresh schedules and display blocked rows.
Slot creation and transitions are audited. Four historical fixture scripts now
require explicit opt-in; three confirmed old fixture slots were identified without
changing shared schedule data. Three legacy cancellation-to-block corrections are
prepared pending the user's choice.

Verification: fmt/clippy clean, **50/50 library**, **106/106 serialized API**,
**72/72 mobile**, **32/32 Doctor Web**; Flutter analysis, both web builds and the
native ARM64 APK build pass. Read-only browser checks confirm both portals display
the same existing blocked slot and observe Doctor polling. Updated phone APK is
required; no physical phone was connected for verification.
Details: [startup reporting and slot sync report](STARTUP_REPORTING_AND_SLOT_SYNC.md).

## Authentication boundary hardening — verified — 2026-10-08

Completed in order: authenticated/authorized idempotency replay, atomic refresh
lineage with family reuse revocation, and single-use patient OTP concurrency.
Explicit environment/seed guards and expired JWT artifact cleanup are included.
Verification: fmt/clippy clean, **50/50 library**, **103/103 serialized API tests**,
five built-executable startup refusal checks, and source/binary/database schema
agreement at `20261023000001`. Legacy rotated refresh families require login again.
Details and exact tests: [backend security report](../services/backend/SECURITY_HARDENING.md).
Production verdict remains controlled synthetic-data testing only; unrestricted
clinical and real-money readiness is not declared.

## Doctor Engagement extension: Queue + notification domain — DONE — 2026-10-07 (later)

Full detail: `docs/CHECKPOINT.md`'s matching entry (pointer only here). Part A (Queue
upcoming-first ordering) and Parts B-G (full notification domain: schema, in-app inbox, DB-backed
daily-summary/T-15-reminder scheduler, push adapter boundary, registration/earning-credit
triggers) of the "HELLODOCTOR — DOCTOR ENGAGEMENT & CONTENT EXTENSION" spec are done and
independently re-verified: `cargo fmt`/clippy clean, `cargo test --lib` 43/43, full
`cargo test --test api_tests -- --test-threads=1` **62/62** (up from 53, zero regressions),
migration schema-verified (`SOURCE = BINARY = DATABASE`, `HELLODOCTOR_BACKEND_READY`). Two genuine
PostgreSQL RLS gotchas were found and fixed along the way (both documented in code comments and
the checkpoint entry): `ON CONFLICT DO NOTHING` and plain `UPDATE` both require implicit `SELECT`
visibility to find candidate rows, which an owner-scoped `SELECT` policy silently defeats for any
backend-authored write made on another user's behalf. `PUSH_PROVIDER_CONFIGURATION_REQUIRED` —
no real FCM/APNs integration exists or was faked.

Backend field validation + stricter email/phone + password strength (8+ chars, upper/lower/digit/
special) is also done, including an orchestrator follow-up fix (a stale 12-char password check
that was silently overriding the new 8-char policy — removed).

**Status reconciliation (2026-10-08):** Articles backend, Doctor Web/Flutter authoring and patient-feed APIs are implemented without Admin review. The newtype-ID refactor and backend validation are complete. Dedicated Flutter notification bell/inbox UI, real push delivery and generated OpenAPI remain open. No patient-facing Flutter app exists. The old deferred backlog is superseded.

## Multi-agent pass: Finance UI, Flutter parity/cleanup, upload compression, input-safety — 2026-10-07

Full detail: `docs/CHECKPOINT.md`'s matching entry (pointer only here). **Historical interrupted state; later validation and engagement work supersedes this old deferred list. Refer to the current status table before resuming.**

Done & independently re-verified by the orchestrator (not just trusted from agent self-reports):
admin-web Finance UI (refund-request resolution screen + a real month-filter bug fix); a second
Doctor Flutter prototype-parity pass (5 of 6 items — the 6th correctly identified as blocked on a
genuinely missing backend lab-report feature, not faked); a Flutter cleanup pass (controller-leak
fix, dead-widget removal, one new regression test); a mobile + admin-web "input safety" pass
(double-submit/busy-guard audit — found and fixed several real missing guards on both platforms —
plus an `Idempotency-Key` network-retry-safety mechanism wired into the Flutter API client, plus
field length caps using canonical numbers 150/1000/2000/300 chars). Evidence across all of the
above: Flutter `flutter analyze` 0 issues / `flutter test` 50/50 / `dart format` clean; admin-web
`tsc --noEmit`/`npm run build` clean.

**New feature, iterated three times with the user — done, independently re-verified, closed
out**: default server-side prescription/intake image compression (`upload_security_service.rs`),
settled at **400 KB total per upload batch** (divided across however many images are in it, e.g.
80 KB/image at 5 images), as a **guaranteed ceiling that is never exceeded and never rejects the
upload** — proven against genuine worst-case random-noise fixtures, not just realistic
document-photo ones. `cargo fmt`/clippy clean, `cargo test --lib` 29/29, full
`cargo test --test api_tests -- --test-threads=1` re-confirmed green. See the checkpoint entry's
§4 for the full algorithm and empirical numbers.

**Status reconciliation (2026-10-08):** Server-side length limits, sanitization, stricter email/phone validation, upcoming-first Queue, notification backend and direct Doctor Articles are implemented. Notification Flutter UI and real push delivery remain open; patient article feed exists as an API only. This entry is no longer a resume queue.

## Admin panel error-log/crash-report wiring + real stack-trace capture — 2026-10-07

Full detail: `docs/CHECKPOINT.md`'s matching entry (pointer only here, not duplicated). Summary:
admin-web's "App Error & Incident Logs" page was reading the wrong backend table (`/admin/logs`,
the audit trail) with fully client-side-fake resolve/simulate actions; rewired to the dedicated
`/admin/system-errors` endpoints (real `trace_id`/`subsystem`/`component`/`error_code` codename
fields, real acknowledge/resolve, a new dev-only `simulate` endpoint that writes a genuine row).
Separately, the `stack_trace` ("code snippet") column had zero writers anywhere, a known gap
flagged since the original System Error Service pass (§8.1) — fixed via `#[track_caller]`
constructors (`AppError::internal`/`AppError::service_unavailable`) and a manual, trackable
`From<sqlx::Error>` impl, with ~40 call sites migrated across 10 files. Backend:
`cargo fmt`/clippy clean, `cargo test --lib` 23/23 (4 new location-capture tests),
`cargo test --test api_tests -- --test-threads=1` 53/53 (unchanged, zero regressions). Admin-web:
`tsc --noEmit`/`npm run build` clean. Live-verified end to end against the real dev server
(simulate → list → acknowledge → resolve, all real HTTP calls against real Postgres).

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

## Historical Doctor visual replication checkpoint - 2026-10-06 (pass 2: Chat/Smart Rx/Schedule/Wallet/Q&A)

Queue was accepted from a real-device screenshot this session (minor typography/overflow fixes applied and now frozen per instruction). The remaining five primary Doctor screens — Consultation Chats, Smart Rx Scanner, Consultation Schedule, Doctor Earnings & Wallet, and Open Health Q&A — were fully rebuilt this pass by reading the actual `prototype/index.html` markup/CSS for each (not from memory), cross-checked against the real Rust backend contracts, and verified with `flutter analyze` (clean), `dart format --set-exit-if-changed` (clean), and `flutter test` (40/40, including new responsive tests at 360/390/412px and business-rule-wording tests). Full detail, per-screen discrepancy notes, and the three backend-data gaps intentionally left unfilled (chat preview/category, Rx lab-reports/vitals, Q&A answered-list) are in `docs/DOCTOR_FLUTTER_REPLICATION_REPORT.md`.

A genuine, minimal, additive backend defect was found and fixed in this pass: `list_own_appointments`, `list_own_chats`, `list_own_slots`, and `wallet_ledger` only ever returned `patient_id` (a UUID), never a patient name — every screen needs this to show a doctor who a row belongs to. Fixed via a `patient_profiles` join (RLS-safety verified first), with a new regression test; full backend suite re-run clean at 42/42 after each change, live dev server restarted to serve the fix.

**Current acceptance scope (2026-10-08):** The user subsequently confirmed the Doctor app. The latest Q&A correction has passed structural/widget/golden checks but still needs physical-device inspection; the old blanket app verdict below is historical.

**The rendered-screenshot-comparison gate below is still not satisfied for these five screens** — this environment has no Android SDK/emulator (confirmed: no adb devices), so only code-level/structural verification was possible. The app remains `DOCTOR_FLUTTER_NOT_VERIFIED` until a human (or an environment with device access) actually compares each screen's rendered output against the prototype, the same way Queue was confirmed.

The Doctor Flutter app must replicate the canonical source file `prototype/index.html` in every visual and interaction detail. This is a hard rule for all future work: every screen or component change must be checked back and forth between the prototype and the rendered Flutter screen. The required loop is inspect HTML/CSS/JS, render Flutter, compare, correct discrepancies, and compare again. Code inspection, analyzer success, and widget tests do not replace rendered comparison evidence. The app remains `DOCTOR_FLUTTER_NOT_VERIFIED` until every required screen passes this gate.
## Retired 2026-10-04 milestone tracker

The old 37% dashboard, 14/35 test baseline and "do not start Flutter" resume plan are superseded by the current milestone/status tables at the top of this document. Earlier capacity results remain historical evidence, not current Azure capacity verification.

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


## 2026-10-07 — Doctor Web implementation checkpoint

Created one Doctor Web project at `apps/doctor-web`, using the canonical
`prototype/index.html` and current Rust contracts. Flutter application source
is unchanged; Flutter analyze and all 50 tests pass. No host software, browser,
global package, Windows service or infrastructure was installed; MinIO remains
absent. The user approved only the loopback preview on 127.0.0.1:3001.

Current Web production build, typecheck, local format/safety checks and 8 Node
tests pass. Installed Edge was used for real MFA/session/core API checks and 55
canonical visual references across five viewport widths. Visual parity is not
accepted: 11 screen roots and 20 explicit subflows still require complete
rendered comparison and final control acceptance. Current verdict is
DOCTOR_WEB_PARTIAL; this is not completion of the requested exact replication.

A live reproducible shared defect required a narrow Rust service change:
complete-no-rx previously returned success while the appointment remained
CONFIRMED. It now runs the existing authoritative completion function in the
no-Rx transaction, preserving DTOs and existing clinical-document authorization.
No schema migration or financial redesign was introduced. Format/clippy and
29 Rust library tests pass. The second full API run passed 52/53, exposing an
old early-timing historical fixture; that fixture's corrected targeted test
passes. All 53 distinct API tests pass across runs; a final single full green
run is not claimed.

Backend canonical startup restored readiness and verified source, binary and
database migration versions all 20261019000001. The latest production Web
browser test verifies authoritative no-Rx completion, unchanged earnings on
retry, statement dialog and slot-generator Escape dismissal, with no browser
exceptions. Full visual/session/prototype coverage remains unfinished.

See DOCTOR_WEB_PROTOTYPE_INVENTORY.md, DOCTOR_WEB_BACKEND_MAPPING.md,
DOCTOR_WEB_VISUAL_PARITY_REPORT.md and DOCTOR_WEB_FINAL_ACCEPTANCE.md for
current evidence and concrete remaining work.
# Doctor Web login recovery — 2026-10-07

The support-reference sign-in error reproduced as backend HTTP 500 caused by
API tests changing shared PostgreSQL role passwords. The test harness now
reuses current development credentials. Installed Edge verified real UI
password login, MFA, reload session restoration and logout; all auth requests
returned 200, foreign-origin login returned 403, and no JavaScript exceptions
occurred. Localhost now redirects correctly to 127.0.0.1:3001. Web tests (10),
typecheck, lint, formatting checks and production build pass. See
docs/audit_evidence/doctor_web/LOGIN_RECOVERY.md for validation limits.
Overall Doctor Web acceptance remains PARTIAL. No host installation or
Doctor Flutter source change was made for login recovery.


## Doctor Web presentation correction - 2026-10-07

Corrected build `vBxYbGtSpLPl4rQ_7Fvhi` is running on approved loopback 3001.
Fullscreen viewport shell, sole canonical Web sidebar, no mobile bottom tabs,
desktop Rx/chat/availability/earnings and Web EMR panel are implemented. Existing
APIs/session handling retained. All five canonical Web roots are accounted for.
10 Web tests, typecheck/lint/format/build PASS; 45 Edge screen/viewport checks,
UI login/MFA/reload/logout and 14 populated workflow checks PASS, zero JS
exceptions. Original/normalized prototype references are kept separately.
VISUAL_PARITY remains PARTIAL, with data/provider and exhaustive dialog limits
explicit in DOCTOR_WEB_CORRECTION_REPORT.md. Overall DOCTOR_WEB_PARTIAL.
Baseline hashes prove Flutter unchanged. No backend edits were made by this pass;
three separately changed notification/test files are recorded and preserved.
No software,
browser, service or Docker infrastructure installed. Stop correction scope here.


## User-approved functional follow-up - 2026-10-07

Latest result: [DOCTOR_WEB_FUNCTIONAL_FOLLOWUP.md](DOCTOR_WEB_FUNCTIONAL_FOLLOWUP.md).
Slot-dialog field clicks no longer dismiss it; real single/batch slot publishing,
UTC+3 conversion, overlap handling and reload persistence are verified. Publishing
opens the schedule on the chosen date. Dashboard Resume Call, unsupported lab
controls, Doctor patient registration and Smart Rx vitals are removed as requested.
Doctor tools provides separate workflow links; calendar and notification bell
have distinct actions. Existing notifications are wired through the HttpOnly BFF.
Canonical startup resolved stale running-backend 404s; source/binary/database
migration versions all equal 20261020000001. No Rust/Flutter source was edited.

11 Web unit tests and typecheck/lint/format/build PASS. Final Edge walkthrough:
7 slot checks + 15 notification/workflow checks PASS; zero JS exceptions. Real
UI login/MFA/reload/logout and two-Doctor notification ownership checks pass.
22 is the declared follow-up regression count, not full historic app acceptance.
Overall full-project visual parity remains PARTIAL; see the linked report's limits.
The article backend is absent and the user explicitly narrowed this follow-up
to existing notifications. No articles API/workflow was invented.

3 local prototype visual assets reused, 0 external downloads, SVG inspection PASS,
0 hotlinked/unlicensed third-party additions and no software/browser/global package/
Docker infrastructure installation. THIRD_PARTY_UI_ASSETS.md records details.

## 2026-10-07 ? Dashboard / account / settings verified checkpoint

This targeted correction is complete before the dedicated Patient Chats routing follow-up. Build `lGgKwZC70Jn19ywa_XOwV`: production build PASS; typecheck, source lint, format checks and 11 unit tests PASS; 13 browser/API checks PASS with no JavaScript exceptions or unexpected 5xx. Installed Edge only. Evidence: `audit_evidence/doctor_web/dashboard-settings/results.json` and adjacent screenshots.

Dashboard: four supported summaries, one upcoming/active list preserving server order; duplicate sidebar queue and modality metrics removed. Instant Rx removed. Unsupported recent dispatch aggregation omitted (BACKEND_DATA_NOT_AVAILABLE). No developer-unavailable cards or emoji placeholders remain on Dashboard.

Account: compact keyboard-accessible popover, View profile opens the read-only Professional section, Settings is a full desktop page with Profile/Professional/Availability/Payout/Security sections. Display-name PATCH and server-confirmed duty tested then restored on the synthetic Doctor. Protected professional fields rejected by server on attempted mass assignment; only display name editable. Payout masking/empty state preserved; actual provider binding remains deferred. Wallet is a separate existing domain, not a profile action strip.

Prescription: camera removal/file picker/recoverable failed-page states separately PASS (7 browser checks). Current actual backend returns 409 RESOURCE_CONFLICT because private storage is unconfigured. STORAGE_CONFIGURATION_REQUIRED; successful server persistence has not passed.

Mobile and Rust source untouched by this pass; no installation or host/infrastructure change. Original whole-app acceptance remains DOCTOR_WEB_PARTIAL; this checkpoint is scoped to the user-authorized UX changes.

## 2026-10-07 ? Final targeted UX verification

Preview: http://127.0.0.1:3001; final build `NSx9VY1Mkxp3QUUfkUpDg`. Dashboard/account/settings completed first; dedicated Patient Chats followed. Production build, typecheck, lint, format and 11 unit tests PASS. Installed-Edge walkthroughs PASS: 7 prescription + 13 Dashboard/settings + 10 Patient Chats = 30 checks, all on this build, no JavaScript exceptions or unexpected 5xx.

Patient Chats: primary sidebar destination `/doctor/chats`; fluid two-pane desktop; correct selected navigation; back/forward/reload and cold login/MFA pass. In-call Chat remains contextual inside Telehealth, sharing the same thread implementation. Two-Doctor list/read/send authorization and real backend expiry pass. Unsupported category filters removed; unread-count/preview APIs are not supplied and were not fabricated. Dashboard has four supported summaries and one queue; Settings has five coherent full-page sections. All requested rendered states reviewed.

Prescription camera removed. Local file picker/previews/retry/remove work; real POST returns 409 RESOURCE_CONFLICT because document storage is not configured. `STORAGE_CONFIGURATION_REQUIRED`; successful persistence remains unverified. No MinIO/provider/storage replacement configured.

Mobile/backend source hashes unchanged; no installation/system/Docker/browser change. Complete scoped matrices, endpoint mapping, screenshots, reversible fixture-testing scope and host/asset integrity: [DOCTOR_WEB_UX_CORRECTION_REPORT.md](DOCTOR_WEB_UX_CORRECTION_REPORT.md). Whole-app acceptance remains DOCTOR_WEB_PARTIAL, not a whole-prototype parity claim. Stop this correction scope.


## 2026-10-07 ? Prescription compression and progress

Completed targeted Web Rx auto-compression to 400 KB per page and actual upload progress beside Send Prescription. Verified JPEG/PNG/WebP input, corrupt/source-size errors, prepared previews, frozen retry payloads, partial-state safety and session cancellation. Final build `6MUCqdkhsIXrtOLwiipGX`, 18 unit tests and 15 focused browser checks pass; narrow visual supplement passes. Storage persistence remains blocked by current private-storage configuration; no mobile/Rust/host changes.

Detailed contract, evidence and host integrity: [DOCTOR_WEB_RX_UPLOAD_REPORT.md](DOCTOR_WEB_RX_UPLOAD_REPORT.md).


## 2026-10-07 ? Schedule correction verified

User-approved Schedule correction VERIFIED on build `vROyHPJLjR3IY3OKge4Ib`: real own-slot PATCH AVAILABLE?BLOCKED, separate cancellation, atomic booking race protection, correct counts/filter/semantic icons and EAT dates. Source=binary=database migration `20261021000001`. Rust format/Clippy PASS, 43 library + 75 API tests PASS (13 new blocking cases); Web 21 units + 12 real browser checks PASS; Flutter analysis and 51 tests PASS. Minimal mobile compatibility exception omits BLOCKED from the legacy booked-patient view. No installation or new infrastructure. Web client diagnostics still log locally rather than feed Admin; this question was answered, not broadened into a telemetry implementation. Full matrix/evidence: [DOCTOR_WEB_SCHEDULE_CORRECTION.md](DOCTOR_WEB_SCHEDULE_CORRECTION.md).


## 2026-10-07 - System telemetry verification

FULL_SYSTEM_TELEMETRY_REQUIREMENT = NOT_MET. Real PostgreSQL incident service and Admin APIs verified; 11 unique focused Rust tests pass. Live Admin MFA/login/error feed/command telemetry and Prometheus metrics pass. Web browser diagnostics only log locally and create no Admin database incident; Flutter global handlers do not ingest crashes. Notification scheduler failures bypass the central incident sink. Missing measurements are replaced by Connected/24 ms defaults; Admin export is a toast, and the feed stops at 200 records. Running Admin login page returns HTTP 500 with missing compiled-module markers, so browser acceptance is blocked. No production code/mobile/backend changes or installations in this verification pass. Evidence and exact gaps: [SYSTEM_TELEMETRY_VERIFICATION.md](SYSTEM_TELEMETRY_VERIFICATION.md).
