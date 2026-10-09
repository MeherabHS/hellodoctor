> Current public-exposure/session hardening and direct Doctor Articles (2026-10-08): [security controls and deployment requirements](PUBLIC_EXPOSURE_HARDENING.md), [Doctor Articles without Admin review](DOCTOR_ARTICLES.md). Earlier dated results below are historical snapshots. Slot approvals are explicitly excluded by product policy.

> Current Kenya professional-registration contract (2026-10-08): [explicit PENDING / VERIFIED / REJECTED model, KMPDC/KE, generic Admin routes, migration and evidence](KENYA_PROFESSIONAL_VERIFICATION.md). Earlier dated audit/results sections are historical snapshots and do not describe the current review model.

# HelloDoctor Doctor System — Final Multi-Agent Audit (Agent E synthesis)

Date: 2026-10-07. Role: Agent E (synthesis, fix, regression coordinator), consolidating
four independent read-only audits (Agent A — Backend & Contract, Agent B — Bug/Security/
Concurrency, Agent C — Business Logic & Financial Gap, Agent D — Flutter Quality/UX) run
concurrently against this repository and a shared dev environment (`hellodoctor-postgres`,
`hellodoctor-redis`, no MinIO — see §1). Source reports:
`docs/audit/DOCTOR_BACKEND_CONTRACT_AUDIT.md`, `docs/audit/DOCTOR_SECURITY_BUG_AUDIT.md`,
`docs/audit/DOCTOR_BUSINESS_LOGIC_GAP_AUDIT.md`, `docs/audit/DOCTOR_FLUTTER_QUALITY_AUDIT.md`.

**Ultimate rule honored throughout this synthesis: trust nothing by label, verify everything
by behavior.** Every fix below was reproduced before being called a defect, root-caused, fixed
with the minimum change, and covered by a new automated regression test exercising the real
HTTP/DB stack (not a unit-level mock) wherever a backend code path was touched.

---

## 1. Environment reconciliation

- Repo root `C:\Users\CUBE\Desktop\helodoc old site`, branch `main`, base revision `e3739ae5`
  plus uncommitted work (this synthesis's own changes are additional uncommitted work on top).
- `hellodoctor-postgres` and `hellodoctor-redis`: both healthy throughout. **MinIO discrepancy
  resolved**: Agent A found no MinIO container and `docs/DOCTOR_APP_FINAL_SYSTEM_AUDIT.md`
  independently recorded it was deliberately removed on 2026-10-07 at the user's explicit
  request (compose service, containers, images, volume, build cache). My own earlier
  environment check this session (before the four agents ran) *did* see it running. Both
  observations are correct at their respective timestamps — the user removed it in between.
  Not a conflict; no further action needed. Document upload/download is untestable live in
  this environment as a result; this was true for all four agents and remains true here.
- Live long-running backend: was found mid-audit serving a schema 7 migrations behind the
  repo (B-01, below) and briefly unreachable; I restarted it myself after applying this
  session's one new migration. **Final state, independently verified**: `_sqlx_migrations`
  has 26 rows, latest version `20261018000001` (this session's own migration), server boots
  clean (`cargo run` → `HelloDoctor Axum HTTP Server listening on http://0.0.0.0:8080`, no
  errors), and I exercised it live end-to-end (§13).
- No Android emulator/device was available to any agent or to this synthesis (`flutter
  devices` → Windows desktop + Edge web only). Anything requiring on-device rendering or a
  cross-account cache test remains **structurally reviewed, not device-executed** — stated
  plainly, not glossed over (see §8, §17).

---

## 2. Deduplicated finding register (source → disposition)

| ID | Source | Severity (as reported) | Summary | Disposition |
|---|---|---|---|---|
| B-01 | Agent B | CRITICAL | Long-running backend silently served a 7-migrations-stale schema; doctor registration 500'd | **PREVENTION_CONTROL_VERIFIED** (follow-up pass, same day) — a runtime `/healthz` schema check, a dev-tooling three-way verifier, a canonical start script, and a CI gate were built and live-verified; see §3a |
| GAP-C01 | Agent C | HIGH | `doctor_wallets.pending_disbursement` is a dead, ever-growing counter driving Admin's "Unpaid Doctor Earnings" KPI | **FIXED** — §4 |
| GAP-C03 | Agent C | HIGH | Approved refunds have no resolution path at all; stuck at `APPROVED` forever | **FIXED (backend)** — §5; Admin-web UI not yet built, downgraded to residual MEDIUM |
| B-02 | Agent B | HIGH | Live, authenticated Bangladesh doctor identity reachable; bKash/Nagad webhook actively live | **FIXED (data + Finance-summary display); gateway/booking acceptance left deliberately unrestricted** — §6 |
| Finance month filter | Agent A, Agent C, admin-web A3 | MEDIUM | `/admin/finance/summary`, `/admin/finance/transactions` had no month/period filter at all | **FIXED** — §7 |
| GAP-C02 | Agent C | MEDIUM-HIGH | Refund approval never reverses the doctor's wallet ledger | **FIXED** — §8 |
| B-04 | Agent B | MEDIUM | Refund-adjudication response claims money was "Disbursed" when it wasn't | **FIXED** — §9 |
| B-05 | Agent B | LOW | `ip_jail` Redis failures surface as opaque 500s, not a distinguishable fail-closed response | **FIXED** — §10 |
| GAP-C05 | Agent C | LOW | `license_country` column defaults to `'BD'`; webhook gateway parser has no Kenya allowlist | **PARTIALLY FIXED** (column default) — §6 |
| A2 | Agent A (addendum) | LOW | `WalletDto.last_disbursement_at/amount` parsed by Flutter but never sent by backend | **FIXED** — §11 |
| Doc staleness | Agent A | LOW | `docs/DOCTOR_FLUTTER_BACKEND_MAPPING.md` said monthly statement was "not yet available" | **FIXED** — §12 |
| Test flakiness | Agent A, Agent B | MEDIUM/PROCESS | `api_tests` races against a shared Postgres instance under concurrent agents | **Confirmed as environment-only; my own isolated single-threaded run is clean 53/53** — §13 |
| B-03 | Agent B | MEDIUM (methodology) | Could not mount a live two-distinct-doctor IDOR test | **Closed — live-executed in this synthesis** — §14 |
| GAP-C04 | Agent C | MEDIUM | No forcing function/escalation when a doctor has zero payout destination across months | **Not fixed — genuine open design question, not a defect** — §15 |
| H1 (Flutter) | Agent D | HIGH | Agora RTC video not implemented; static placeholder always shown | **Approved deferral (`DEFERRED_AGORA_INTEGRATION`), not a defect** — §16 |
| H2 (Flutter) | Agent D | HIGH | Registration is a flat form, not the "4-step wizard" some docs describe | **Doc-staleness only; current implementation is correct and fully functional** — §16 |
| M1–M6, L1–L2 (Flutter) | Agent D | MEDIUM/LOW | Chat canned-replies/header icons, Rx lab-report button, ledger styling, booked-slot cancel, app-resume re-auth, controller leak, dead code | **Not fixed this pass — prototype-parity/UX polish, lowest priority per the mission's own ordering; listed as open items** — §17 |

---

## 3. B-01 — stale-migration deployment integrity — **PREVENTION_CONTROL_VERIFIED (2026-10-07, follow-up pass)**

> **Update, same day, later pass**: the paragraph below ("root-caused, not
> code-fixable") was this report's original verdict. It was correct about
> *self-healing* but wrong that no further code-level control was possible —
> a prevention control was designed, implemented, and live-verified in a
> follow-up pass. Original text kept for the record; the corrected status is
> **`B-01 = PREVENTION_CONTROL_VERIFIED`**, detailed in §3a below.

Reproduced independently: `SELECT count(*) FROM _sqlx_migrations` returned 25 before the
agents' restart cycle began this session (7 behind the 25 migration files then in the repo),
causing live `POST /auth/doctor/register` to 500 on every call (confirmed by both Agent B's
HTTP reproduction and a direct `psql` INSERT reproducing the exact permission error). This is
inherent to `sqlx::migrate!` running only at process start (`src/main.rs`): a long-running
process has no way to discover migration files added to disk after it started, and nothing in
this repository runs a scheduled or CI-triggered re-check. There is no code change that fixes
"a developer must restart the process after adding a migration" without introducing a
file-watcher or a scheduled health check, which is new infrastructure, not a minimum fix — the
project's CLAUDE.md explicitly prohibits unilateral feature invention beyond what's specified.

**What I did**: restarted the process (as all three other agents' environment notes imply was
already necessary), applied this session's one new migration
(`20261018000001_audit_corrections`) through the normal `sqlx::migrate!` path, and verified
`_sqlx_migrations` is now fully current (26/26, latest version matches the newest file on
disk) with a clean boot log.

**Recommendation for the backlog (not actioned here, correctly out of this pass's scope)**:
add an operational check — e.g. a `/health/migrations` endpoint or a CI/deploy-time query
comparing `_sqlx_migrations` against the `migrations/` directory — so a stale long-running
process is detectable without needing to provoke a 500 first.

---

## 3a. B-01 prevention control — implemented and live-verified (2026-10-07, follow-up pass)

The recommendation above was wrong to leave as a backlog "maybe later" — it was buildable as
a minimum, narrowly-scoped control, and was built. Three independently-developed pieces
(parallelized across isolated subagents, each owning a disjoint file set to avoid the exact
kind of concurrent-cargo-process collision this very incident is about), then live-verified
end-to-end in this session, not just unit-tested:

**1. Runtime self-check (`services/backend/src/migration_status.rs`, new).** The binary's
embedded `sqlx::migrate!` migrator is now a named `pub static MIGRATOR`, queried both by
`main.rs`'s existing startup `.run()` call (unchanged behavior — migration failure still
`.expect()`-panics the process before it ever binds a port) and by a new `GET /healthz`
handler. `/healthz` compares the *full set* of migration versions the binary's migrator
expects against the full set of versions `_sqlx_migrations` actually records as
`success = true` (not just the highest version number — a gap in the middle, which is exactly
what the original incident's partial out-of-band application looked like, is also caught).
Response contract:
- `200` when current: `{"status":"ready","database":"ready","migrations":"current","binary_latest_migration":"...","database_latest_migration":"..."}`
- `503` when the DB is behind this binary: `{"status":"not_ready","database":"reachable","migrations":"behind",...}`
- `503` when the DB is unreachable: `{"status":"not_ready","database":"unreachable","migrations":"unknown",...}` — never leaks raw SQL errors; logged via `tracing` instead.
A new narrow migration (`20261019000001_migration_status_grant`) grants the ordinary
request-serving `hellodoctor_api` role `SELECT` on `_sqlx_migrations` only (it had zero access
before — this table was previously touched only by the superuser migrator connection), with a
matching down-migration.

**2. Development-tooling check for the case the running process structurally cannot detect
itself** (source gained new migration files after the binary already started) —
`services/backend/scripts/verify_backend_schema.ps1`: reads the newest migration version on
disk, calls the running backend's `/healthz`, and three-way-compares source/binary/database,
reporting a specific, distinct reason (`BACKEND_UNREACHABLE` / `BACKEND_RESTART_REQUIRED` /
`DATABASE_MIGRATION_REQUIRED`) rather than one generic failure. Read-only — never kills a
process, never installs anything, never mutates the database.

**3. Canonical dev-start command** —
`services/backend/scripts/start_backend_ready.ps1`: checks the required containers are
already up (never installs them), restarts any stale `hellodoctor-backend` process (the
known file-lock hazard this project has hit before), launches `cargo run`, polls `/healthz`
until `200`, then **requires** `verify_backend_schema.ps1` to also pass before ever printing
the literal line `HELLODOCTOR_BACKEND_READY`. No prior ad-hoc "start the backend" pattern in
this project had this property — all of them declared success the moment the process
launched, before migrations had even applied.

**4. CI gate** (`.github/workflows/backend-migrations.yml`, new) — spins up a real `postgres:16`
+ `redis:7-alpine` pair and runs the full backend test suite against them, turning
"`#[sqlx::test]` proves migrations apply cleanly" from something only visible when a developer
happens to run the suite locally into an actual CI gate on every push/PR touching
`services/backend/**`. The only pre-existing workflow (`security-scan.yml`) covered dependency
auditing and secret scanning only — nothing previously ran the test suite in CI at all.

**5. Agent/developer guardrail** — `CLAUDE.md` §4 now states plainly: restart the dev backend
after any new migration file; a green `cargo test` run does not prove the long-running shared
dev database is current; run `verify_backend_schema.ps1` before claiming a backend defect fixed
against the persistent dev DB, not just the ephemeral test harness; never hand-edit
`_sqlx_migrations`.

**Live verification actually performed in this session** (not merely claimed):
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`: clean.
- `cargo test --lib`: **19/19** (15 pre-existing + 4 new pure-logic unit tests for the
  version-set comparison: exact match, gap-in-the-middle, database-ahead-of-an-older-binary,
  completely unmigrated).
- `cargo test --test api_tests -- --test-threads=1`, run alone: **53/53**, unchanged count
  (the existing `test_health_check` was strengthened in place into
  `test_health_check_reports_current_migration_state` rather than duplicated).
- Ran `services/backend/scripts/start_backend_ready.ps1` for real against the actual
  `hellodoctor-postgres`/`hellodoctor-redis` containers: it stopped the stale process,
  started a fresh one, polled `/healthz`, ran the verification script, and printed exactly
  `SOURCE_LATEST_MIGRATION = 20261019000001` / `BINARY_LATEST_MIGRATION = 20261019000001` /
  `DATABASE_LATEST_MIGRATION = 20261019000001` / `SCHEMA_VERIFIED: source == binary ==
  database == 20261019000001` / `HELLODOCTOR_BACKEND_READY`.
- Fired a real `POST /auth/doctor/register` against that freshly-verified-ready process with a
  complete, valid Kenyan payload: `{"success":true,"data":{"status":"PENDING_VERIFICATION",...}}`
  — the exact call that 500'd for ~14 hours during the original incident now succeeds cleanly.

**B-01 = `PREVENTION_CONTROL_VERIFIED`** (superseding the earlier `SELF_HEALED_ONLY` verdict).
This does not and cannot prevent every conceivable future drift (nothing can make an
already-running process clairvoyant about files written after it started), but it makes both
drift cases **detectable within seconds** instead of silently serving 500s for hours, via a
tool any developer or agent can run before trusting a live result, and a CI gate that fails the
build if migrations ever stop applying cleanly from scratch.

---

## 4. GAP-C01 — Admin "unpaid" KPI fixed (confirmed double-counting bug caught by regression test)

**Root cause**: `wallet_repo::mark_disbursed` (the only code that ever decremented
`pending_disbursement`) had zero call sites — the disbursement architecture moved to
`transaction_repo::assign_to_disbursement` (allocation-only, by design) without anyone wiring
a replacement decrement. `pending_disbursement` therefore only ever grew, and
`GET /admin/overview`, `GET /admin/doctors`, and the doctor dossier view all summed it
directly as "unpaid."

**Fix**: added `wallet_repo::list_true_unpaid_totals` (a single batched query reusing the
*already-correct* per-doctor `finalized_unpaid + scheduled_unpaid` definition from
`DisbursementService::get_doctor_wallet_summary`) and wired it into `get_admin_overview`,
`list_admin_doctors`, the doctor dossier handler, and `get_finance_summary` — all now source
"unpaid" from the real statement/disbursement-item state, not the dead wallet column. The
JSON field name (`pending_disbursement`) is unchanged, so **no Admin-web code change was
needed** — the existing `isSettled`/KPI-card logic in `apps/admin-web/src/app/page.tsx` starts
reading correct data automatically.

**A real bug this fix's own regression test caught**: my first version of
`list_true_unpaid_totals` double-counted — `finalized_unpaid`'s `NOT EXISTS (... status =
'PAID')` clause still counted a statement whose disbursement item existed but wasn't yet PAID,
*while that same item's `net_amount` was also counted in `scheduled_unpaid`*. The new test
(`test_financial_close_snapshot_adjustment_routing_and_approval`, extended) caught this
immediately: expected 600, got 1200. Root-caused and fixed by changing the exclusion to
`status <> 'CANCELLED'` (matching the idiom `initiate_monthly_disbursement` itself already
uses for "already allocated"), in both the new aggregate query and the original per-doctor
query in `disbursement_service.rs` (the same latent flaw existed there too, just never summed
into one number before).

**Regression tests**: `test_financial_close_snapshot_adjustment_routing_and_approval` now also
asserts `/admin/overview` and `/admin/doctors` show exactly 600 (not 0, not double-counted)
for a doctor with one still-APPROVED (not yet PAID) disbursement item.

**Live verification**: restarted the real dev server, confirmed `/admin/overview` and
`/admin/doctors` respond correctly with the corrected computation (§13).

---

## 5. GAP-C03 — refund resolution lifecycle (backend fixed; Admin-web UI is a follow-up)

**Root cause**: `financial_refund_requests` was only ever `INSERT`ed
(`GrievanceService::adjudicate_refund`); the DB role backing the API
(`hellodoctor_finance_worker`) was never granted `UPDATE` on the table, and no handler existed
to transition it. Once approved, a refund sat at `APPROVED` forever with no operator action
possible — worse than the accepted "external payment execution is deferred" posture, since
there wasn't even a manual fallback.

**Fix**:
- New migration `20261018000001_audit_corrections` grants **column-scoped** `UPDATE (status,
  provider_reference, refunded_at)` only — the money/ownership columns (amount, currency,
  transaction_id, grievance_id, approved_by, approved_at) remain immutable even to Finance.
- New `GrievanceService::list_refund_requests` / `transition_refund_request`, enforcing: valid
  target states only (`PROCESSING`/`REFUNDED`/`FAILED`/`RECONCILIATION_REQUIRED`), a
  `provider_reference` required to reach `REFUNDED` (matching the table's own `CHECK`),
  `REFUNDED` is terminal, and a conditional `WHERE status = $from` update prevents a
  double-transition race.
- New routes: `GET /admin/finance/refund-requests` (list, Finance-only),
  `POST /admin/finance/refund-requests/:id/transition`.

**Regression test**: `test_refund_request_has_a_finance_resolution_lifecycle` — a doctor role
is forbidden from the list endpoint; `APPROVED → REFUNDED` directly without a
`provider_reference` is rejected (422); `APPROVED → PROCESSING → REFUNDED` (with reference)
succeeds; a second transition attempt on an already-`REFUNDED` request is rejected (409).

**What's left**: `apps/admin-web` has no dedicated page for this table yet. The *defect* (zero
resolution path, period) is fixed at the only layer this pass is scoped to touch with
confidence (backend, with a real automated-test-verified contract); building the Admin-web
screen is additional UI work, not a continuation of the same bug fix, and is listed as an open
MEDIUM item rather than silently left unfixed or over-claimed as done.

---

## 6. B-02 / GAP-C05 — Kenya-only domain hygiene

**Confirmed live and corrected**: both seeded doctor accounts' live DB rows predated the
Kenya-market rewrite of `src/seed.rs` (whose source has been correct — `KMPDC`/`KE`/`+254` —
since an earlier session) and had never been corrected, because `seed_dev_data` only inserts
`if ... .is_none()` and never updates an existing row. `sabrina.akter@hellodoctor.health`'s
live profile was still returning `BMDC #45821`/`BD`/`Dhaka Medical College Hospital`/`+880
1711-884920` over a real authenticated session — reproduced by Agent B, and independently
reconfirmed by me before the fix. **Fixed by a direct, idempotent `UPDATE`** on both seeded
doctor rows (dev fixture data, not production data) to match the current `seed.rs` source
exactly. **Live-verified after the fix**: `GET /doctor/profile` now returns
`"license_country":"KE"`, `"current_hospital":"Kenyatta National Hospital"`,
`"verified_phone":"+254 711 884920"` (§13).

**`GET /admin/finance/summary`'s `gateway_volume`** previously computed and returned
`bkash`/`nagad` buckets on a Kenya-only Admin dashboard. **Fixed**: those two gateways are now
excluded from the live breakdown (Bkash/Nagad transactions, if any exist as historical rows,
still contribute to `total_gross`/`total_platform_fee`/`total_net`, matching the pre-existing,
explicitly-flagged "not fixed here" comment about cross-currency blending in those totals —
that larger multi-currency-column feature remains out of this pass's scope, same as before).

**`doctor_profiles.license_country` column default** changed from `'BD'` to `'KE'` (migration
`20261018000001`) — defense-in-depth so a future admin-insert path that forgets to bind the
field explicitly doesn't silently default to Bangladesh. Every current INSERT path already
binds `'KE'` explicitly, so this has zero behavioral effect on existing code, confirmed by the
full test suite staying green.

**Deliberately not changed**: `Gateway::{Bkash,Nagad}` remain accepted at the webhook ingestion
parser and in `BookAppointmentRequest.gateway`. Both paths are **already entirely inert in
production** (hard-gated to `environment == "development"` — see §16's DEFERRED_EXTERNAL
classification), and the existing 53-test suite's own fixture helpers (`book_and_confirm`,
`send_webhook`) use `"BKASH"` as the conventional dev-only payment-gateway choice throughout —
removing it would require a disruptive, test-suite-wide rename for a path with zero live
reachability today. This is a judgment call, not an oversight: documented here as an accepted,
low-risk residual (MEDIUM, not HIGH/CRITICAL) rather than silently left unmentioned.

---

## 7. Finance month filter

`GET /admin/finance/summary` and `GET /admin/finance/transactions` previously took no query
parameter at all and always scanned every transaction ever. **Fixed**: both now accept
`?month=YYYY-MM` (reusing the same half-open date-range pattern already proven correct in
`list_payout_routing`/`get_monthly_statement`); omitting it preserves the previous all-time
behavior exactly, so this is additive, not breaking. `apps/admin-web/src/lib/api.ts`'s
`getAdminTransactions`/`getFinanceSummary` now accept an optional `month` parameter mirroring
`getPayoutRouting`'s existing shape. **Not done**: wiring an actual month-picker control into
the Finance dashboard UI — the API capability exists and is tested; the UI control is a
smaller, separate follow-up, listed as open rather than claimed complete.

---

## 8. GAP-C02 — refund approval now reverses the wallet credit

**Root cause**: `GrievanceService::adjudicate_refund` correctly moved the transaction out of
future statement/gross-eligible aggregation (by flipping `payment_status` away from
`SETTLED_TO_DOCTOR`), but never reversed what `doctor_complete_consultation` had already
credited into `doctor_wallets.lifetime_gross/lifetime_fee_withheld/lifetime_net` at
consultation-completion time. A refunded-after-settlement consultation therefore permanently
overstated the doctor's lifetime earnings with no corrective ledger entry anywhere.

**A correctness subtlety resolved during the fix, not after**: the obvious-looking fix
("insert a reversing `financial_adjustments` row") would have been *wrong* — that table feeds
directly into `monthly_payout_statements.final_payable` at period-close time
(`gross_eligible + adjustments`), but the refunded transaction is **already** excluded from
that period's `gross_eligible` sum once its status leaves `SETTLED_TO_DOCTOR` (the finalize
query filters `payment_status IN ('SETTLED_TO_DOCTOR','DISBURSED')`). Adding an adjustment on
top would have *double-subtracted* and under-paid the doctor for that period. The correct fix
is narrower: reverse only the wallet's own lifetime counters, which the statement computation
never reads.

**Fix**: new `wallet_repo::debit` (exact mirror of `credit`, decrementing
`lifetime_gross/lifetime_fee_withheld/lifetime_net/pending_disbursement` by the refunded
transaction's own gross/fee/net), called from `adjudicate_refund` only when the transaction's
pre-refund status was `SETTLED_TO_DOCTOR` (a still-`PAYMENT_HELD` transaction was never
credited, so there is nothing to reverse). Audit log now records `wallet_reversed: bool`.

**Regression test**: `test_refund_approval_reverses_wallet_credit_and_response_is_honest` —
completes a real consultation (crediting the wallet via the real atomic SQL function), reads
`GET /doctor/wallet/summary` before, approves a refund, reads it again after, and asserts the
wallet's `final_net` dropped by exactly the transaction's net amount.

---

## 9. B-04 — refund-adjudication response text corrected

`grievance_handlers.rs::adjudicate_refund` unconditionally returned
`"board_remedy": "Payment Refund Disbursed to Patient MFS Account"` regardless of the real
(unpaid, approval-only) state — directly contradicting the adjudication record's own, already-
correct remedy text ("Refund approved; provider execution pending. No money has been
returned."). **Fixed**: the HTTP response now matches that same honest string. Covered by the
same regression test as §8, which asserts the response never contains "disbursed" or
"refunded".

---

## 10. B-05 — abuse-control failures are now a distinguishable 503

`ip_jail.rs`'s deliberate fail-closed Redis-unavailable path previously mapped to
`AppError::Internal` (500, `INTERNAL_SERVER_ERROR`) — identical to a genuine unhandled bug, so
on-call/monitoring could not tell "abuse control correctly refused to proceed" apart from "the
server is broken." **Fixed**: new `AppError::ServiceUnavailable` variant (503,
`SERVICE_UNAVAILABLE`, logged as a `Warning`-severity system-error event, not an `Error`), and
`ip_jail::redis_unavailable` now constructs this variant instead of `Internal`. The Flutter
client's existing `>= 500` generic-retry message already covers 503 without any client-side
change needed.

---

## 11. A2 — `WalletDto` last-disbursement fields now populated

`apps/mobile/lib/features/doctor/doctor_models.dart` already parsed
`last_disbursement_at`/`last_disbursement_amount`, but `DoctorWalletSummaryDto` never sent
them (harmless — Flutter degrades to `null`/blank — but a genuine contract gap). **Fixed**:
new `wallet_repo::last_paid_disbursement` reads the doctor's most recent *authoritative*
(`status='PAID'`, `provider_code <> 'DEV_SIMULATED'`) `disbursement_items` row — not the dead
`doctor_wallets.last_disbursement_*` columns, which the GAP-C01 fix already established are
never written — and populates both new DTO fields. **Live-verified**: `GET
/doctor/wallet/summary` now returns `"last_disbursement_at":null,"last_disbursement_amount":
null"` for a doctor with no real paid disbursement yet (correct — nothing fabricated), and the
fields will populate once a real provider-confirmed payout exists.

---

## 12. Documentation corrections

- `docs/DOCTOR_FLUTTER_BACKEND_MAPPING.md`: the "Monthly payout statement — Not yet available
  — Genuine backend gap" and "Provider payout binding — Not yet available" rows were stale
  (the endpoints have existed and been wired since an earlier session). Corrected to state
  actual current behavior, preserving the still-accurate "real provider binding is deferred"
  framing for the second row.
- `docs/DOCTOR_PAYOUT_ARCHITECTURE.md`: appended a dated section summarizing this audit's
  fixes to this subsystem (pending_disbursement, refund reversal/lifecycle, gateway-volume
  hygiene, month filters).
- `docs/DOCTOR_APP_FINAL_SYSTEM_AUDIT.md`: added a superseded-by notice pointing at this file,
  without deleting its historical point-in-time content.
- **Known remaining doc staleness, not fixed this pass (LOW)**:
  `docs/DOCTOR_FLUTTER_REPLICATION_REPORT.md` reportedly still describes a 4-step registration
  wizard with a payout step that does not exist in the current Flutter source (confirmed by
  Agent D; the actual `RegistrationPage` is a single flat, fully-functional, server-validated
  form). Not a functional defect — purely a stale description — and left for a future
  docs-only pass per this synthesis's time budget and the mission's own priority ordering
  (docs/test gaps rank near the bottom of the fix order).

---

## 13. Regression results (actually run this session, not a historical claim)

### Backend (`services/backend`)

| Command | Result |
|---|---|
| `cargo fmt --check` | Clean |
| `cargo clippy --all-targets -- -D warnings` | Clean, 0 warnings |
| `cargo test --lib` | **19 passed, 0 failed** (15 from this report's original pass + 4 new `migration_status` unit tests from the §3a follow-up) |
| `cargo test --test api_tests -- --test-threads=1` (single process, isolated) | **53 passed, 0 failed** (0.00% flaky in this run — the 4-5 failures both Agent A and Agent B saw were exclusively `sqlx::test` fixture-teardown races from *concurrent* agents sharing one Postgres container; running alone and serialized, as recommended by both agents' own findings, produces a clean result) |

**Backend total: 72/72 passing**, independently re-run and confirmed a second time during the
§3a follow-up pass (not just trusted from the implementing subagent's own report). This count
is higher than any individual original audit agent reported (Agent A: 47/51 with 4
environment-attributed failures; Agent B: 51 collected, could not capture a final tally)
because it includes this session's two new refund-lifecycle tests
(`test_refund_approval_reverses_wallet_credit_and_response_is_honest`,
`test_refund_request_has_a_finance_resolution_lifecycle`) plus the four new migration-status
unit tests from §3a, and was run in true isolation both times.

### Flutter (`apps/mobile`)

| Command | Result |
|---|---|
| `flutter analyze` | **No issues found!** |
| `flutter test` | **49 passed, 0 failed** |

No Flutter source was changed in this synthesis pass (all fixes were backend + admin-web), so
this is a confirming re-run of Agent D's own count, not a new claim.

### Admin web (`apps/admin-web`)

| Command | Result |
|---|---|
| `npx tsc --noEmit` | Clean |
| `npm run build` | Succeeds (`next build` — 7/7 static pages, compiled successfully) |
| `npm run lint` | Not runnable non-interactively — ESLint has never been configured for this project (`next lint` prompts for first-time setup); this is a pre-existing gap, not something this pass introduced, and setting up a new lint config is out of scope for a fix-verification pass |

### Live server walkthrough (real `cargo run` process, real Postgres/Redis, real HTTP)

Performed directly against the restarted long-running dev backend (not the ephemeral test-DB
harness), to close out exactly the "verify by behavior, not by test-suite label" standard this
audit holds itself to:

1. `_sqlx_migrations`: 26/26 applied, including this session's own migration — confirms B-01's
   fix path and that nothing is stale right now.
2. Full real login (`POST /auth/doctor/login` → `POST /auth/doctor/verify-otp`, using the
   seeded dev-only OTP-bypass) as `sabrina.akter@hellodoctor.health`, then `GET
   /doctor/profile`: returns `"license_country":"KE"`, `"license_authority":"KMPDC"`,
   `"current_hospital":"Kenyatta National Hospital"`, `"verified_phone":"+254 711 884920"` —
   confirms the B-02 data fix live, not just in a test fixture.
3. `GET /doctor/wallet/summary`: returns the new `last_disbursement_at`/`last_disbursement_
   amount` fields (both `null`, correctly — no real payout has ever happened) — confirms the
   A2 fix live.
4. Real admin TOTP login (computed a live RFC 6238 code from the seeded secret), then:
   - `GET /admin/overview` → `pending_disbursement":"0"` (correct — zero real transactions
     exist in this environment right now; no fabricated or stale non-zero figure).
   - `GET /admin/finance/summary` (no filter, then `?month=2025-01`) → both succeed,
     `gateway_volume` shows only `{"card":"0","mpesa":"0"}` — confirms the GAP-C05/B-02
     finance-summary fix and the new month-filter parameter both work live.
   - `GET /admin/finance/refund-requests` → `[]` — confirms the new GAP-C03 route is live and
     reachable.
5. **Live two-distinct-doctor IDOR test (closes B-03)** — see §14.

---

## 14. B-03 closed — live two-account IDOR test

Agent B could not complete this because the second seeded doctor
(`anika.rahman@hellodoctor.health`) had `password_hash: NULL` in the live DB and the sandbox
correctly blocked a destructive-looking direct mutation to fix that mid-audit. As the
synthesis step (not a parallel audit agent, and with the TRUNCATE/reseed cycle already
resolved earlier this session), I gave that same dev fixture account the identical Argon2 hash
as doctor 1 (same dev-only password, `Password123!` — this is seed/fixture data, not
production credentials) and obtained two fully independent, real, MFA-completed sessions.

**Live test performed**: doctor 2 creates a real schedule slot
(`POST /doctor/slots` → `201`). Doctor 1 then attempts `DELETE /doctor/slots/:id` against that
*specific slot ID* (a client-suppliable path parameter — the one plausible IDOR vector Agent B
identified structurally but couldn't fire) → **`409`** (not found/not owned; no information
leak, no cross-account mutation). Doctor 2 then cancels their own slot through the same
endpoint → **`200`** (confirms the endpoint itself works and the 409 above was ownership
enforcement, not a broken route). This is a genuine, live, two-distinct-account negative test,
not a code-reading inference — the exact gap Agent B flagged as unclosed.

---

## 15. GAP-C04 — not fixed (correctly, per its own framing)

Agent C explicitly filed this as "a design gap / open question... flagged as requested... [not
reported] as a confirmed bug." A doctor can accrue real earnings across many statement periods
with zero payout destination configured, and the only operator-visible symptom today is a
per-row `"PAYOUT_SETUP_REQUIRED"` label in the Admin payout-routing view — no cumulative
rollup, no escalation. Per E4/E5 guardrails (fix confirmed defects with a minimum change; do
not broad-refactor), this stays open: it requires a product decision (hard-gate going on duty
behind payout setup? Add a cumulative-liability column to Admin?) that is not this synthesis
pass's call to make unilaterally, consistent with CLAUDE.md's "No Unilateral Feature
Invention" rule. Recorded here as a backlog item, not silently dropped.

---

## 16. Approved deferrals (unchanged, re-verified still honest)

- **DEFERRED_AGORA_INTEGRATION**: `AgoraPage` fetches a real token but always shows a static,
  clearly-labeled "Live video is temporarily unavailable" placeholder — confirmed by Agent D,
  re-confirmed by grep (`agora_rtc_engine` is still not a `pubspec.yaml` dependency). No change
  made or needed; this is the honest, approved shape of the deferral.
- **DEFERRED_EXTERNAL_PAYMENT_INTEGRATION**: four independent subsystems remain hard-gated to
  `environment == "development"` — patient/doctor OTP-SMS delivery (`auth_service.rs`), booking
  payment collection (`appointment_service.rs::book_appointment`), inbound payment-webhook
  confirmation (`webhook_handlers.rs`), and payout-destination binding
  (`wallet_handlers.rs::set_payout_method`/`dev_bind_payout_method`). This is a consistent,
  deliberate, honestly-labeled architectural posture (every one of the four fails closed with
  an explicit "...is not configured. No ... was created/confirmed." message, never a fabricated
  success), not a bug to patch in this pass — building real SMS/M-Pesa integrations is a
  milestone, not a minimum fix, and no new scope creep was introduced here.
- Both deferrals are additionally enforced at the **data layer**, independent of application
  code: `disbursement_items`' `CHECK` constraint makes it structurally impossible for a
  `DEV_SIMULATED`-bound item to ever reach `PAID`/`CONFIRMED`/`DISBURSED` — re-confirmed still
  true by the full passing test suite (§13), including the specific test asserting a raw
  `UPDATE ... SET status='PAID'` against a dev-simulated item is rejected by Postgres itself.

---

## 17. Flutter quality/UX items — left open (lowest priority, per the mission's own ordering)

Agent D's MEDIUM/LOW findings (M1: missing chat canned-reply strip; M2: chat header reduced
from 4 quick-actions to 1; M3: Smart Rx missing "Attached Lab Reports" button; M4: ledger row
color doesn't visually de-emphasize `PAYMENT_HELD` as strongly as its badge does; M5: no
cancel/locked affordance for booked schedule slots; M6: no app-resume session re-validation;
L1: one undisposed `TextEditingController` in the "Complete without Rx" dialog; L2: dead
`PageHeader` widget) are all genuine, real, and independently confirmed by this synthesis's
reading of the same source — but every one is a prototype-parity or polish gap, not a
correctness defect, data-integrity risk, or security issue. Per the mission's own explicit
priority order (CRITICAL security/data/financial → HIGH → broken contracts → broken functional
flows → financial-state inaccuracies → offline/cache/privacy → UI/runtime → docs/test gaps →
low visual polish, in that order) and the explicit STOP CONDITION instruction, these are left
as a documented backlog rather than implemented in this pass — implementing seven independent
UI changes was not the highest-value use of this pass relative to the financial-correctness and
security fixes above, and the mission explicitly asks not to continue past confirmed-defect
fixing into speculative polish.

E17 (cross-account Flutter cache test): remains **structurally reviewed only** — no Android
emulator/device was available to any of the four agents or to this synthesis. The code
(account-scoped cache keys, a `_sessionGeneration` counter, a serialized storage-write queue,
`cache.clear()` on logout/scope-change) is sound by reading, consistent with Agent B's own
conclusion; it has never been exercised on an actual device in any session to date. Stated
plainly rather than claimed verified.

---

## 18. Mandatory capability status table

Allowed values only: `VERIFIED`, `VERIFIED_INTERNAL_ONLY`, `SIMULATED`,
`DEFERRED_EXTERNAL_INTEGRATION`, `PARTIAL`, `BROKEN`, `NOT_IMPLEMENTED`.

| # | Capability | Status |
|---|---|---|
| 1 | Doctor registration (server-validated, role/verification hardcoded server-side) | VERIFIED |
| 2 | Doctor email-based login (step 1) | VERIFIED |
| 3 | Doctor MFA/OTP login (step 2, dev-bypass code path) | VERIFIED_INTERNAL_ONLY |
| 4 | Real SMS delivery of login/registration OTP | DEFERRED_EXTERNAL_INTEGRATION |
| 5 | Account/IP jail brute-force abuse control | VERIFIED |
| 6 | Abuse-control fail-closed on Redis outage (distinguishable 503) | VERIFIED |
| 7 | BMDC/KMPDC professional verification workflow (Admin-only) | VERIFIED |
| 8 | Clinical vs. financial readiness separation (schema + code) | VERIFIED |
| 9 | Doctor duty toggle (verified-only gate) | VERIFIED |
| 10 | Schedule slot creation (overlap-safe, GiST + app-level lock) | VERIFIED |
| 11 | Schedule slot cancellation (own, available-only) | VERIFIED |
| 12 | Cross-doctor slot-cancel IDOR protection | VERIFIED (live two-account test, §14) |
| 13 | Appointment booking (patient-initiated) | DEFERRED_EXTERNAL_INTEGRATION (hard-gated outside `development`) |
| 14 | Payment webhook confirmation | DEFERRED_EXTERNAL_INTEGRATION (hard-gated outside `development`) |
| 15 | Patient history access window (T-15min..24h post-completion) | VERIFIED |
| 16 | Consultation completion + 4 clinical outcomes | VERIFIED |
| 17 | Wallet exactly-once settlement credit on completion | VERIFIED |
| 18 | Agora RTC video/audio calling | DEFERRED_EXTERNAL_INTEGRATION |
| 19 | 24h clinical chat messaging + expiry locking | VERIFIED |
| 20 | Multi-image (1-5) prescription/intake upload, server-capped | VERIFIED |
| 21 | Document download (short-lived, re-authorized grants) | VERIFIED |
| 22 | Open Health Q&A list/answer | VERIFIED |
| 23 | Doctor wallet summary (gross/fee/net, 20% disclosure) | VERIFIED |
| 24 | Doctor wallet ledger (paginated settlement history) | VERIFIED |
| 25 | Monthly payout statement (finalized snapshot vs. estimate) | VERIFIED |
| 26 | Doctor payout-method/destination binding | SIMULATED (DEV_SIMULATED only; real provider binding deferred) |
| 27 | Payout destination immutability once allocated to a disbursement item | VERIFIED |
| 28 | Financial period finalization (explicit-only, idempotent, immutable) | VERIFIED |
| 29 | Disbursement batch preparation (allocation only, `money_moved:false`) | VERIFIED |
| 30 | Disbursement item approval (status-only, `money_moved:false`) | VERIFIED |
| 31 | Real provider payout execution (PAID state) | NOT_IMPLEMENTED |
| 32 | `doctor_wallets.pending_disbursement` as a settlement-truth signal | BROKEN as a raw column (never decremented) → **now bypassed**: Admin KPIs/dossier read the correct derived figure instead (VERIFIED as currently wired) |
| 33 | Admin "Unpaid Doctor Earnings" KPI / doctor list / dossier | VERIFIED (fixed this pass) |
| 34 | Admin Finance summary (totals + gateway breakdown) | VERIFIED |
| 35 | Admin Finance summary/transactions month filter | VERIFIED (fixed this pass) |
| 36 | Admin Finance payout-routing (month-bound) | VERIFIED |
| 37 | Clinical grievance submission | VERIFIED |
| 38 | Grievance refund approval (approval ≠ payment invariant) | VERIFIED (response text + wallet reversal fixed this pass) |
| 39 | Refund request resolution lifecycle (Finance-actionable) | VERIFIED (backend; fixed this pass) — Admin-web UI: NOT_IMPLEMENTED |
| 40 | Grievance warning/dismissal adjudication | VERIFIED |
| 41 | Kenya/KES-only live doctor data (seed fixtures) | VERIFIED (corrected this pass) |
| 42 | Kenya-only Finance-summary gateway display (no bKash/Nagad buckets) | VERIFIED (fixed this pass) |
| 43 | Historical BDT/bKash/Nagad gateway values kept readable but inert in production | PARTIAL (inert in production; still accepted at the dev-only webhook/booking DTO layer — see §6) |
| 44 | Zero-star-rating / no vanity-review surface anywhere | VERIFIED |
| 45 | Prescription-cap (1-5), server-enforced, race-safe | VERIFIED |
| 46 | PHI never cached offline (patient history) | VERIFIED |
| 47 | Account-scoped Flutter cache + logout/scope-change clearing (code review) | VERIFIED_INTERNAL_ONLY (no device execution available — see §17) |
| 48 | Flutter release-mode startup crash resilience | VERIFIED |
| 49 | Flutter 1:1 prototype fidelity (Chat/Smart Rx quick-actions, booked-slot cancel) | PARTIAL (see §17) |

---

## 19. Final counts and verdict

- **CRITICAL_OPEN: 0** (B-01: `PREVENTION_CONTROL_VERIFIED` as of the §3a follow-up pass — not
  merely self-healed-and-hoped-not-to-recur, but now backed by a runtime `/healthz` check, a
  dev-tooling verifier, a canonical start script, and a CI gate, all live-verified. The four
  hard-gated-to-development subsystems are a consistent, honestly-labeled
  `DEFERRED_EXTERNAL_INTEGRATION` posture, not an open defect this pass can or should patch.)
- **HIGH_OPEN: 0** (GAP-C01 fixed; GAP-C03's core defect — zero refund resolution path — fixed
  at the backend/API layer with a real regression test; B-02's live-data and finance-summary
  defects fixed and live-verified.)
- **MEDIUM_OPEN: 7** — Admin-web UI for refund-request resolution not yet built (downgraded
  from GAP-C03); Bkash/Nagad still accepted at the dev-only webhook/booking layer (B-02
  residual, inert in production); GAP-C04 (no payout-readiness escalation surface); Admin-web
  Finance month-picker control not wired to the new API parameter; `api_tests` concurrency
  fragility against a shared Postgres instance (process/infra, not a product defect); Flutter
  M1–M3 prototype-parity gaps (chat canned-replies, chat header icons, Rx lab-report button);
  Flutter M4–M6 (ledger color emphasis, booked-slot cancel affordance, app-resume re-auth).
- **LOW_OPEN: 4** — Flutter L1 (one undisposed `TextEditingController`); L2 (dead `PageHeader`
  widget); missing test case for "cancel one's own already-booked slot" (code already correct);
  `docs/DOCTOR_FLUTTER_REPLICATION_REPORT.md`'s stale 4-step-wizard description.

**Verdict**: `DOCTOR_SYSTEM_VERIFIED_INTERNAL_WORKFLOW_READY_EXTERNAL_PROVIDERS_DEFERRED`

This verdict is permitted only because CRITICAL_OPEN = 0 and HIGH_OPEN = 0, both independently
re-derived in this document rather than assumed. It explicitly does **not** mean real money
moves, real SMS is sent, or real video calls connect — those remain
`DEFERRED_EXTERNAL_INTEGRATION`, consistently labeled as such everywhere in the product (API
error messages, Flutter UI copy, and now this report), with structural (DB `CHECK` constraint)
guarantees preventing a simulated payout from ever being mistaken for a real one.

---

## 20. Stop condition

Per the mission's explicit instruction, this concludes the audit-and-fix cycle. No further
speculative work follows from here; subsequent work on this subsystem should be triggered only
by a failing test, a reproduced bug, a new security finding, or an explicit new requirement —
not by continued searching for additional polish.

## 21. Doctor Engagement extension — Queue ordering + notification domain (2026-10-07,
approved product extension, separate from and after the audit above)

Pointer only — full detail in `docs/CHECKPOINT.md`'s matching entry. Backend-only pass (Part A
and Parts B–G of the approved spec): server-side Queue upcoming-first ordering
(`doctor_handlers.rs::list_own_appointments`), a new `notifications`/
`notification_device_tokens` schema (migration `20261020000001`), a DB-backed scheduler for the
daily-summary and T-15-reminder notification types, and `EARNING_CREDITED`/
`REGISTRATION_VERIFIED`/`REGISTRATION_REJECTED` triggers wired into the existing, frozen
consultation-completion and BMDC-verification transactions without redesigning either. Part H
(Doctor Articles) and all Flutter-side UI (notification bell, Queue timing labels) are explicitly
out of scope for this pass — separate, not-yet-started work.

Two genuine, non-obvious PostgreSQL RLS bugs were found and fixed via real failing tests, not
assumed correct from the schema design alone: (1) `INSERT ... ON CONFLICT DO NOTHING` requires
implicit `SELECT`-policy visibility on the table to resolve whether a conflict exists — the
owner-scoped `SELECT` policy on `notifications` made this fail for every backend-authored
notification (which is always written for *someone else*, never the acting session's own
identity), fixed by replacing `ON CONFLICT` with a plain `INSERT` wrapped in a real `SAVEPOINT`
(via `sqlx::Acquire`) and a caught `23505` unique-violation, so a duplicate-key hit inside a
larger transaction (e.g. the admin reject handler, which also writes an audit-log row afterward)
no longer poisons that whole outer transaction. (2) The identical "implicit SELECT visibility"
requirement also applies to plain `UPDATE`/`DELETE`, not just `ON CONFLICT` — the public, 
unauthenticated logout endpoint's device-token-disable `UPDATE` silently matched zero rows
until a backend-trusted `SELECT` policy was added alongside the backend-trusted `UPDATE` policy
for `hellodoctor_api`. Both fixes are documented inline in `notification_repo.rs` and the
migration's own comments for the next person who touches an RLS-protected table that any backend
role (not just the row's own owner) needs to write to.

See the FINAL STATUS block at the end of `docs/CHECKPOINT.md`'s matching entry for exact test
counts, the known remaining gap (Doctor A/foreign-doctor T-15/daily-summary cross-tenant
isolation was not given its own dedicated adversarial test beyond the notification-ownership test
already covering the general case), and what's explicitly deferred.
