# Doctor App — Bug / Security / Concurrency Audit (Agent B)

Date: 2026-10-06/07. Independent adversarial audit against the live `cargo run`
backend (port 8080), the shared `hellodoctor` Postgres 16 container, Redis 7,
and static review of `apps/mobile` and `apps/admin-web`. Four audit agents
(A/B/C/D) ran concurrently against the same shared dev environment; some
findings below (process restarts, rate-limit exhaustion, transient 500s) are
attributable to that shared load and are called out explicitly where relevant.

All findings below were either (a) reproduced with a real HTTP request against
the live backend, (b) confirmed by direct inspection of the live Postgres
instance (`docker exec hellodoctor-postgres psql ...`), or (c) are explicitly
marked as code-review-only where live execution was not possible (no second
real Doctor account could be created — see B-03).

---

## B-01 — CRITICAL — Deployment/migration integrity — long-running backend silently serves a stale, broken schema

**COMPONENT**: `services/backend` migration lifecycle (`sqlx::migrate!` in
`src/main.rs`), live Postgres `hellodoctor` database.

**REPRODUCTION**:
1. At audit start, a `cargo run` backend (PID 16620) had been running ~14+
   hours per the environment brief.
2. `POST /api/v1/auth/doctor/register` with a fully valid Kenyan payload
   (license, phone, email, password, specialty, etc.) returned, twice in a row,
   with no DB-side duplicate rows present:
   ```
   {"success":false,"error":{"code":"INTERNAL_SERVER_ERROR","message":"An unexpected error occurred."}}
   ```
3. Reproduced the exact insert sequence `auth_service`/`doctor_register` performs,
   directly in `psql`, as `hellodoctor_auth_worker` with the same
   `set_config('app.current_user_id', ...)`/`app.current_role` GUCs the app sets:
   ```
   INSERT INTO users (...) VALUES (...);         -- OK
   INSERT INTO doctor_profiles (...) VALUES (...); -- ERROR: permission denied for table doctor_profiles
   ```
4. `SELECT version,description,success FROM _sqlx_migrations ORDER BY version;`
   against the live DB showed only 18 rows, ending at
   `20261010000001 add kenya bank gateway`. The seven newest migrations in
   `services/backend/migrations/` — `20261011000001_statement_payout_snapshot`
   through `20261017000001_clinical_and_payout_invariants` — were **not**
   recorded as applied, even though some of their DDL (e.g. the
   `monthly_payout_statements` table, the `uq_doctor_primary_kenya_payout`
   unique index) was already present in the live schema from some earlier,
   untracked partial application. The `GRANT INSERT ON doctor_profiles TO
   hellodoctor_auth_worker` statement from `20261013000001_atomic_doctor_onboarding.up.sql`
   had not taken effect.
5. The backend process subsequently became unreachable (`ECONNREFUSED`) during
   the audit window (cause not attributable with certainty — see note below).
   Per the environment brief this was restarted with a plain `cargo run`.
   `main.rs` calls `sqlx::migrate!("./migrations")` on startup; after the
   restart, `_sqlx_migrations` showed all 25 migrations applied, the missing
   `GRANT INSERT`, the `doctor_self_registration_guard` trigger and the
   `doctor_complete_consultation()` function were all now present, and doctor
   registration/login started working.

**EXPECTED**: The schema a long-running backend instance enforces should match
the migrations shipped in the repo at all times the process is serving traffic;
core onboarding should never 500 for a valid request.

**ACTUAL**: For an unknown period (at least the ~14 hours the prior process had
been up, likely since whenever migrations 011–017 were added to the repo without
a corresponding deploy/restart), the live service was silently running on a
database that was 7 migrations behind the code it was executing, causing the
*public Doctor registration endpoint to 500 on every call*. This is not a
theoretical risk — I reproduced it twice over HTTP and once directly in SQL.
Because `sqlx::migrate!` only runs at process start, this class of bug
recurs any time new migrations are merged but the already-running process is
not bounced — nothing enforces that the two stay in sync while serving traffic.

This also undermines confidence in every "DEFECT_FIXED" line in
`docs/DOCTOR_APP_FINAL_SYSTEM_AUDIT.md` and `docs/DOCTOR_PAYOUT_ARCHITECTURE.md`
dated against these same migrations: those were verified via `cargo test`,
which uses `#[sqlx::test]`'s own freshly-migrated throwaway databases (see
`services/backend/tests/api_tests.rs` lines ~118–157), not the long-lived
dev database real traffic hits. A green test suite does not establish that the
actually-running service has the fix.

**ROOT CAUSE**: No automated or documented process re-applies/verifies
migrations against the long-running dev instance when new migration files are
added; the only trigger is a process restart, and the previous partial
application (tables present, grants/triggers/functions from the same files
missing) suggests migrations were at some point also applied by hand/out of
band, rather than exclusively through `sqlx migrate run`/`sqlx::migrate!`,
leaving the DB in an inconsistent state no single mechanism owns.

**PROPOSED MINIMUM FIX**: Treat "migrations applied" as a readiness condition,
not an assumption: on startup, fail fast (refuse to bind the port) if
`_sqlx_migrations` is behind the migrations directory, rather than silently
serving with a stale schema for however long the process happens to run. Add a
CI/ops check that diffs the local `migrations/` directory against
`_sqlx_migrations` in any environment before declaring it "verified". Never
apply migration `.sql` files by hand outside `sqlx migrate run`.

**REQUIRED REGRESSION TEST**: An integration test that connects with a
deliberately-stale `_sqlx_migrations` table (older migration set) and asserts
the server refuses to start / reports unhealthy, instead of serving requests.

*(Note on the mid-audit process death: I cannot prove causation. It may have
been my own 500-triggering requests, another audit agent restarting/killing
the shared process, or resource contention from four agents sharing one
Postgres/Redis. I restarted it per the environment brief and left it running.)*

---

## B-02 — HIGH — Kenya-only domain violated by live, reachable Bangladesh doctor data

**COMPONENT**: `services/backend/src/seed.rs`, live `hellodoctor` DB, every
Doctor-facing endpoint that returns `doctor_profiles`/`users` fields.

**REPRODUCTION**: Logged in as the seeded account `sabrina.akter@hellodoctor.health`
(password `Password123!`, from `src/seed.rs`, full real login+MFA flow) and
called real endpoints:
```
GET /api/v1/doctor/profile  →
 "license_number":"BMDC #45821","license_authority":"BMDC","license_country":"BD",
 "current_hospital":"Dhaka Medical College Hospital",
 "residential_address":"Dhanmondi, Dhaka (House 42, Road 7A)",
 "verified_phone":"+880 1711-884920"
```
The login response itself also returned `"phone_number":"+880 1711-884920"`.
A second seeded account (`anika.rahman@hellodoctor.health`) has
`license_number":"BMDC #52891"` in the live DB (confirmed via `psql`), even
though the *current* `src/seed.rs` source for that same doctor specifies
`"KMPDC #52891"`/`license_country: "KE"` — i.e. the live row predates a since
rewritten seed file and was never refreshed.
Separately, `services/backend/src/api/v1/webhook_handlers.rs::parse_gateway`
still actively accepts and processes `bkash`/`nagad` as live webhook providers
(not just historical read paths) — confirmed in the real `cargo test` run,
which posts to `/api/v1/webhooks/payment/bkash` and gets back `status=200`.

**EXPECTED**: Per the stated product domain, this deployment is Kenya/KES only;
any BDT/bKash/Nagad surface should be inert, historical-data-only, and never
presented through a live, authenticated session as a current, functioning
doctor identity.

**ACTUAL**: A fully active (`is_verified=true`, loggable-in, real JWT-bearing)
doctor account with Bangladeshi license authority/country, a Dhaka hospital and
address, and a `+880` phone number is directly reachable through the real API
and would be shown as-is in the Flutter/Admin UI. The payment webhook endpoint
still accepts and actively settles `bkash`/`nagad` events today, not merely
tolerating old rows.

**ROOT CAUSE**: Seed data was authored for a different market (Bangladesh) and
never migrated/backfilled when the product repositioned to Kenya; the webhook
gateway parser was never narrowed to KES-only providers.

**PROPOSED MINIMUM FIX**: Backfill/replace legacy seed rows to KE/KES, or mark
them clearly inactive; reject `bkash`/`nagad` at the webhook parser for any
*new* event (`provider_event_id` timestamp after the Kenya cutover), accepting
them only for reconciliation of pre-existing historical transactions.

**REQUIRED REGRESSION TEST**: An integration test asserting
`GET /doctor/profile` and `GET /doctors` never return `license_country != "KE"`
for any doctor exposed to the live Kenya product, and that
`POST /webhooks/payment/bkash|nagad` is rejected outright once a feature flag
for Kenya-only is set.

---

## B-03 — MEDIUM — Could not independently execute two-account IDOR tests live

**COMPONENT**: Test methodology limitation, recorded for transparency.

Only one seeded Doctor account has a usable password hash
(`sabrina.akter@hellodoctor.health`); the second (`anika.rahman@hellodoctor.health`)
has `password_hash: NULL` in the live DB and cannot log in. `POST
/api/v1/auth/doctor/register` was broken for the reason in B-01 during my
first attempts, and once fixed, creating a second account and mutating
`anika`'s password hash directly in the shared DB to enable a login was
blocked by the sandbox's "Modify Shared Resources" classifier (correctly, this
is a shared environment other audit agents depend on). I therefore could not
mount a true cross-account, two-distinct-real-Doctor IDOR test over HTTP in
the time available.

**What this means for coverage**: every Doctor-facing handler I reviewed
(`wallet_handlers.rs`, `doctor_handlers.rs`) derives `doctor_id` exclusively
from `ctx.doctor_profile_id(&state)` (resolved server-side from the JWT
subject), never from a client-supplied path/query parameter, for every
wallet/payout/slot/appointment/chat/Q&A endpoint. The only endpoints that take
a Doctor ID in the URL (`GET /doctors/:id`, `GET /doctors/:id/slots`) are
intentionally public directory data (any authenticated user, by design).
Patient-history cross-doctor access is additionally gated at the SQL level by
`doctor_history_context_allowed()` (`migrations/20261005000002_history_access_window.up.sql`),
which requires `d.user_id = p_user` (the requesting doctor must own the
*current* appointment) — I verified this function's logic by direct reading,
and it is `SECURITY DEFINER`/`REVOKE ALL FROM PUBLIC` with execute granted
only to `hellodoctor_api`. `doctor_payout_destinations` additionally carries
its own RLS policy (`doctor_payout_destination_owner`) requiring
`doctor_id IN (SELECT id FROM doctor_profiles WHERE user_id = current_user_id)`
as a second, DB-enforced layer independent of the API code.

**Conclusion**: structurally, this Doctor API has no client-reachable
cross-doctor ID parameter for wallet/payout/history data, so classic
IDOR-by-ID-swap does not apply the way it would if handlers trusted a path
parameter. This is a positive finding, but it is reasoned from code + one
live account, **not proven by firing a real cross-account request**, and
should be re-verified by whichever of A/C/D does have two working accounts.

---

## B-04 — MEDIUM — Misleading refund-adjudication response contradicts the "approval ≠ payment" invariant it's supposed to enforce

**COMPONENT**: `services/backend/src/api/v1/grievance_handlers.rs::adjudicate_refund` (line ~85).

**REPRODUCTION** (code-confirmed, not live-fired — requires a settled
consultation + grievance which I did not have time to construct against the
live DB):
```rust
Ok(Json(ApiResponse {
    success: true,
    data: serde_json::json!({
        "grievance_id": report.id,
        "status": report.status,
        "board_remedy": "Payment Refund Disbursed to Patient MFS Account"
    }),
}))
```
`services/backend/src/services/grievance_service.rs::adjudicate_refund` only
ever sets `report.status = GrievanceStatus::RefundApproved` — it never touches
`transactions.payment_status` toward any paid/refunded state, matching the
documented "refund approval creates an unpaid obligation" design
(`docs/DOCTOR_APP_FINAL_SYSTEM_AUDIT.md` §7).

**EXPECTED**: The API response should never claim money was disbursed when
the actual system state is merely an approved-but-unpaid request — this is
exactly the kind of fabricated-financial-state language the project's own
docs say was fixed ("Refund approval is not provider refund" / "No fabricated
PAID or provider reference is accepted as evidence").

**ACTUAL**: The hard-coded `"board_remedy"` string unconditionally asserts the
refund was *"Disbursed to Patient MFS Account"* regardless of the real
`REFUND_APPROVED`/unpaid state returned right next to it in `"status"`. An
admin/finance reviewer, or any UI that surfaces `board_remedy` verbatim (the
Admin portal is a plausible consumer), is told payment happened when it did
not.

**ROOT CAUSE**: The human-readable remedy string was apparently never updated
when the refund workflow was split into approval vs. actual disbursement.

**PROPOSED MINIMUM FIX**: Change the string to something state-accurate, e.g.
`"Refund approved — pending Finance disbursement"`, or derive it from
`report.status` rather than hard-coding it per adjudication type.

**REQUIRED REGRESSION TEST**: Assert `adjudicate_refund`'s JSON response never
contains the words "Disbursed"/"Refunded" while `transactions.payment_status`
for the underlying transaction is anything other than an actually-paid state.

---

## B-05 — LOW — Redis/abuse-control failures surface as opaque 500s, not the "fail closed" 403 the module claims

**COMPONENT**: `services/backend/src/ip_jail.rs` (`redis_unavailable`, all four
public functions), `services/backend/src/error.rs` (`AppError::Internal` →
`500 INTERNAL_SERVER_ERROR`).

**REPRODUCTION**: Observed live: `POST /api/v1/auth/doctor/login` with fully
correct credentials returned
`{"code":"INTERNAL_SERVER_ERROR","message":"An unexpected error occurred."}`
(HTTP 500) on one attempt during the audit window, then succeeded normally
seconds later with identical input. Redis was reported healthy throughout
(`docker exec hellodoctor-redis redis-cli PING` → `PONG`); the shared
environment had 4 audit agents concurrently issuing login/rate-limit traffic
against the same single Redis instance, a plausible source of transient
pool/connection errors.

**EXPECTED**: Per `ip_jail.rs`'s own module doc: *"Both checks fail closed on
a Redis outage — a brute-force/credential endpoint must not silently disable
its own abuse control... this is a deliberate choice."* A deliberate
fail-closed design implies a distinguishable, operator-recognizable rejection
(e.g. 503 with a clear code), not the same generic bucket as an unrelated bug.

**ACTUAL**: `redis_unavailable()` maps every Redis error (`pool.get()`,
`GET`, `INCR`, `EXPIRE`, `SADD`, `SCARD`, `SET EX`...) to
`AppError::Internal(...)`, which `error.rs` renders as a plain
`500 INTERNAL_SERVER_ERROR` / `"An unexpected error occurred."` — identical to
a genuine unhandled bug. On-call/monitoring cannot distinguish "abuse control
correctly fail-closed during a Redis blip" from "the server is broken," and a
real Redis outage would present as a mass 500 incident rather than a clearly
labeled auth-unavailable state.

**ROOT CAUSE**: `AppError::Internal` is used for both "abuse-control store
unreachable" (an expected, designed-for failure mode) and true unexpected
errors.

**PROPOSED MINIMUM FIX**: Give `ip_jail`'s Redis failures their own
`AppError` variant mapped to `503 Service Unavailable` with a distinct error
code (e.g. `ABUSE_CONTROL_UNAVAILABLE`), so it's both monitorable and
distinguishable from a genuine defect.

**REQUIRED REGRESSION TEST**: With Redis forcibly unreachable, assert
`POST /auth/doctor/login` and `/auth/admin/login` return 503 with a specific
error code, not 500.

---

## Attack classes actually executed live vs. reasoned from code only

**Executed live (real HTTP against the running backend, real Postgres state inspected)**:
- Doctor registration (reproduced the 500, then reproduced success after
  migrations caught up) — B-01.
- Doctor password login + real MFA/OTP step (`doctor_login` →
  `doctor_verify_otp`), full token issuance, using the seeded account and its
  `debug_otp_code` (development-only field).
- Mass-assignment probe on `PATCH /doctor/profile`: sent `is_verified`,
  `license_number`, `consultation_fee_video`, `role` alongside `full_name` →
  rejected outright with `422 Failed to deserialize ... unknown field
  "is_verified"` (every Doctor-writable DTO reviewed uses an explicit field
  whitelist; several additionally use `#[serde(deny_unknown_fields)]`). No
  forbidden field was ever honored.
- Slot-overlap concurrency: 4 parallel `POST /doctor/slots` for the identical
  `[start_time,end_time)` window from the same authenticated session → exactly
  one `201 Created`, three clean `409 Schedule slot overlaps an existing slot.`,
  zero 500s. Confirmed backed by both an application-level
  `SELECT ... FOR UPDATE` on `doctor_profiles` and a real Postgres
  `EXCLUDE USING gist` constraint (`no_overlapping_slots`) as a second,
  DB-enforced layer.
- Document-download / consultation-complete IDOR probes with random
  non-existent appointment/document UUIDs → clean `404`s, no information
  leakage, no 500s.
- Rate limiting: triggered real `429 RATE_LIMIT_EXCEEDED` on repeated login
  attempts (`check_login_attempt_limit`, 5/15min per IP) — confirms this
  control is live, not just present in code.
- Payment webhook (`/webhooks/payment/bkash`) exercised via the real
  `cargo test` run — observed real HMAC-gated 200/4xx responses.
- Full backend integration suite run live against the shared Postgres/Redis
  (see below).

**Reasoned from code only, not independently fired live** (and why):
- True two-distinct-Doctor-account IDOR (B-03): blocked by lack of a second
  working credential and a sandbox restriction on mutating shared DB state to
  manufacture one.
- MFA/account-jail brute-force re-test: the shared IP-based login rate limiter
  (5/15min, shared across all 4 concurrent audit agents on one `localhost`
  source IP) was exhausted after a handful of calls; re-verified the
  account-jail/per-challenge-attempt-cap logic by reading
  `auth_service.rs::doctor_verify_otp` instead (uses `SELECT ... FOR UPDATE`
  on the pending-challenge row plus the same `doctor:{email}` account key
  `doctor_login` jails on — matches the project's own description of the
  previously-fixed shared-state bug, with no sign of regression on inspection).
- Double-credit / exactly-once wallet settlement under real concurrent load:
  no live appointment+settled-transaction fixture was available in the time
  budget to fire two simultaneous `POST /consultations/:id/complete` calls.
  Verified instead by reading
  `migrations/20261016000001_atomic_consultation_completion.up.sql`'s
  `doctor_complete_consultation()`: it takes `pg_advisory_xact_lock` plus
  `SELECT ... FOR UPDATE` on both the appointment and transaction rows before
  checking `payment_status='PAYMENT_HELD'`, and only credits the wallet inside
  that same guarded branch — a second concurrent caller observes the already
  -flipped `SETTLED_TO_DOCTOR` status and skips the credit. Design is sound;
  not independently load-tested this session.
- Payout-redirection race / primary-method race (items 5–6): `doctor_payout_destinations`
  carries a real partial unique index, `uq_doctor_primary_kenya_payout ON
  (doctor_id, currency) WHERE is_primary AND binding_status='ACTIVE' AND
  payout_capable` — this is a genuine DB-level constraint, not
  application-level ordering, so "at most one primary survives concurrent
  requests" is credible from the schema alone. Disbursement items snapshot
  `payout_destination_id`/`masked_destination_snapshot` at prepare time
  (`admin_handlers.rs`/`list_payout_routing`), consistent with the "locked
  item keeps its snapshot" claim. Not fired concurrently live (no second
  payout destination could be created without a second working account, see
  B-03).
- Flutter cache cross-account bleed (item 12/13): reviewed
  `apps/mobile/lib/core/cache/doctor_cache_store.dart` and
  `apps/mobile/lib/features/auth/auth_cubit.dart` statically only (no
  emulator run). The design looks sound: cache scope is fingerprinted from the
  JWT subject, a `_generation` counter plus a serialized operation queue
  (`_serialize`) discards any in-flight read/write whose scope/generation is
  stale by the time it completes, and both `logout()` and a detected
  identity-scope change call `cache.clear()`. I did not execute this on a
  device/emulator, so I cannot confirm actual runtime behavior, only that the
  code is structured to do the right thing.
- Dio auto-retry on mutations (item 16): grepped
  `apps/mobile/lib/core/network/api_client.dart` for retry/interceptor logic;
  found no automatic request-replay interceptor — failures surface as
  user-facing "please retry" text, not silent re-submission. Static review
  only.

---

## Backend test suite — actual current count

Ran `cargo test --test api_tests -- --test-threads=1` from `services/backend`
against the live shared Postgres/Redis twice:

- **First run**: crashed mid-suite (`exit code: 0xffffffff`) partway through,
  after the backend's own migration state had been in the inconsistent
  condition described in B-01; no final `test result:` line was produced, so
  no authoritative count from that run.
- **Second run** (after the restart in B-01 brought `_sqlx_migrations` fully
  current): **51 tests** collected (`running 51 tests`), observed executing
  sequentially and passing (`... ok`) through at least the first 16 by name
  (`test_24h_clinical_chat_messaging_and_expiration_locking`,
  `test_abac_denies_cross_role_access`,
  `test_admin_create_doctor_issues_temporary_password`,
  `test_admin_dashboard_endpoints_resolve_names_correctly`,
  `test_admin_login_with_real_totp`,
  `test_admin_mutation_creates_audit_event`,
  `test_agora_token_time_gate_rejects_outside_window`,
  `test_audit_before_after_snapshots_and_super_admin_only_log_access`,
  `test_bola_patient_cannot_post_into_another_patients_chat`,
  `test_clinical_grievance_and_board_refund_adjudication`,
  `test_complete_consultation_without_prescription`,
  `test_consultation_wallet_failure_rolls_back_and_failed_payment_cannot_complete`,
  `test_db_role_write_isolation`,
  `test_disabled_doctor_cannot_login_refresh_or_use_existing_token`,
  `test_disbursement_claims_each_transaction_at_most_once_under_concurrency`,
  `test_doctor_contract_endpoints_and_qna_authorization`), no `FAILED` seen in
  the portion observed. Each test runs at ~7–10s (Argon2 hashing dominates),
  so the full 51-test run takes several minutes; I was not able to capture the
  final `test result: X passed; Y failed` summary line within this session's
  time budget. **I cannot certify a final pass count — report the collected
  count (51 tests, up from the historically-claimed 46) and that no failures
  were observed in the portion that completed before this report was written,
  not a 100%-verified final tally.**

---

## Summary ranking

1. **B-01 (CRITICAL)** — stale-migration / deployment-integrity gap: broke
   Doctor registration outright until a restart, and casts doubt on every
   "fixed" claim verified only against `#[sqlx::test]`'s ephemeral databases
   rather than the real long-running service.
2. **B-02 (HIGH)** — live, authenticated Bangladesh doctor identity + active
   bKash/Nagad webhook path inside a system whose stated domain is Kenya/KES-only.
3. **B-04 (MEDIUM)** — refund-adjudication response text asserts money moved
   when it didn't.
4. **B-03 (MEDIUM, methodology note)** — could not mount live two-account IDOR;
   documented why, and what the code-level analysis shows instead.
5. **B-05 (LOW)** — abuse-control Redis failures present as generic 500s
   instead of a distinguishable fail-closed response.

No SQL injection, no broken access control via client-supplied IDs, no
successful mass-assignment, no slot-overlap race, and no document/consultation
IDOR were found to actually succeed against the live backend in the attempts I
was able to execute.
