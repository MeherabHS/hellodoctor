# Milestone 8 Phase 5 — Comprehensive Efficiency Optimization Investigation

**Date:** 2026-11-02
**Scope:** Continuation of the six-service performance investigation in
[`MILESTONE_8_SIX_SERVICE_PERFORMANCE_REMEDIATION.md`](MILESTONE_8_SIX_SERVICE_PERFORMANCE_REMEDIATION.md)
Phases 1-4. Product-owner direction for this pass: do not conclude a larger
VM is necessary until avoidable SQL, RLS, business-logic, caching, and
API-request overhead has been systematically evaluated; keep the 4-vCPU/8-GB
six-service architecture; investigate before changing hardware.

## Starting hypothesis (from the Phase 4 report) vs. what the evidence showed

Phase 4 flagged `fn_doctor_treats_patient` and `doctor_history_context_allowed`
as the next-priority investigation targets. Live `EXPLAIN (ANALYZE, BUFFERS)`
profiling of `GET /api/v1/doctor/chats` (the same endpoint Phase 3 fixed)
under the real `hellodoctor_api` role and real RLS context — first against the
live `hellodoctor-m8-final-local` benchmark database, then in an isolated
`pg_dump`/restore clone (`hellodoctor_phase5_clone`, never applied against the
live benchmark DB or the shared dev database) for safe before/after
measurement — found this was **not** where the remaining cost was:

- `fn_doctor_treats_patient` (evaluated via `patient_profiles_access`, deferred
  to only the already-paginated ~20 result rows by the Phase 3 fix): **~11ms
  total** for a 20-row page. Confirmed cheap, not the bottleneck.
- `doctor_history_context_allowed` (called directly once per paginated row in
  `list_own_appointments`): same pattern, same conclusion.

The real remaining cost, found by profiling rather than assumed, was two
unrelated issues in `list_own_chats`:

## Finding 1 — missing index on `chat_conversations.doctor_id`

`chat_conversations` (80,377 rows in the seeded benchmark dataset) had **no
index at all** on `doctor_id` (or `patient_id`) — only a primary key and a
unique constraint on `appointment_id`. Every `GET /api/v1/doctor/chats` call
did a full `Seq Scan`, reading and filter-evaluating 78,351 unrelated rows to
find one doctor's 2,026 conversations.

**Fix:** `migrations/20261204000002_chat_conversations_doctor_index.up.sql` —
a plain `CREATE INDEX idx_chat_conversations_doctor ON chat_conversations(doctor_id)`,
following this project's existing precedent
(`20261202000005_t15_reminder_scan_indexes`) of a non-`CONCURRENTLY` index for
a modest-sized table. No index was added on `patient_id`: no existing query
filters `chat_conversations` directly by `patient_id` (the Patient app's own
listing is driven from the already-indexed `appointments.patient_id`), so a
reflexive "index every FK" index would not be a measured benefit — the same
caution this project's own prior migration explicitly states.

Measured in the isolated clone: **1,849ms → 1,672ms** (~10%). Modest on its
own — the seq scan's cost was mostly in row/filter touches, not pure I/O,
since PostgreSQL's `AND` short-circuits the expensive RLS predicate once
`doctor_id` fails to match.

## Finding 2 (the dominant cost) — `chat_messages_access` RLS re-evaluated once per raw message row

`list_own_chats` computes `MAX(m.sent_at)` over `chat_messages`, joined
**before** the `LIMIT`, to sort conversations by recency. `chat_messages` has
`FORCE ROW LEVEL SECURITY`; its `chat_messages_access` policy is a correlated
`EXISTS` back into `chat_conversations`. Because this runs once per **raw
message row**, not once per conversation, the live plan showed **3,591
separate executions** of that subquery for one doctor's 2,026 conversations
(average ~1.8 messages/conversation) — confirmed via the plan's
`loops=3591` on the `chat_conversations_pkey` subplan, not assumed.

Measured in the isolated clone by substituting `c.created_at` for the
aggregate (a floor test — what if `chat_messages` were never touched at all):
**1,672ms → 744ms**, i.e. this one re-check pattern accounted for **~930ms,
roughly 55-60% of total query time** — more than fully explaining the
degradation the original Phase 4 benchmark saw from a handful of extra
Admin/Doctor workers.

**Fix:** `migrations/20261204000003_chat_conversation_last_message_denorm.up.sql`
denormalizes the latest message timestamp onto
`chat_conversations.last_message_at`, maintained by a plain (`SECURITY
INVOKER`, not `SECURITY DEFINER`) `AFTER INSERT` trigger on `chat_messages`.
The list/sort query now never touches `chat_messages` at all; opening an
actual chat thread is unaffected and still reads `chat_messages` directly.
Combined with deferring the `appointments` join (needed only for display
fields, not sorting) to the same already-paginated rows — the identical
pattern the Phase 3 fix already used for `patient_profiles` — the full
restructured query measured **369ms**, a **5.0x reduction** from the 1,849ms
baseline.

### Security review of the new write path

`hellodoctor_api` previously had **read-only** (`SELECT`) access to
`chat_conversations`; all writes belonged to `hellodoctor_system_worker`. This
migration grants exactly `UPDATE(last_message_at)` — one derived-cache column,
not a clinically or financially meaningful field — following this project's
existing narrow-column-grant precedent (`GRANT UPDATE(is_read) ON
chat_messages TO hellodoctor_api`). The trigger function is deliberately
**not** `SECURITY DEFINER`, so it runs with the same FORCE RLS enforcement as
any other write the inserting role performs; it can only ever touch the one
conversation (`NEW.conversation_id`) that `chat_sender_guard` /
`chat_messages_insert`'s own `WITH CHECK` clauses already proved the sender is
an authorized party of.

Live-verified, not just asserted, against the real `hellodoctor_api` role in
the isolated clone:

- **Positive case:** an authenticated doctor sending a message to their own
  conversation → trigger fires, `last_message_at` updates correctly.
- **Negative case (cross-doctor isolation):** a different, real, authenticated
  doctor attempting `UPDATE chat_conversations SET last_message_at=... WHERE
  id=<not their conversation>` → `UPDATE 0`, no rows affected, final value
  unchanged from the authorized write. FORCE RLS holds exactly as for any
  other column.

No RLS policy, authorization function, or role grant beyond this one narrow
column was touched. `fn_doctor_treats_patient` and
`doctor_history_context_allowed`'s own logic are untouched by this entire
investigation — they were measured and confirmed not to need it.

## Net result

| Stage | Execution time (isolated clone, real RLS, 2,026-conversation doctor) |
|---|---|
| Baseline (before this pass) | 1,849ms |
| + missing index fix | 1,672ms |
| + `last_message_at` denormalization + deferred `appointments` join | **369ms (5.0x faster than baseline)** |

## Verification performed

- Row-set equality: confirmed in the isolated clone for two different
  doctors (2,026 and 2,019 conversations respectively) at different
  pagination offsets (`LIMIT 20 OFFSET 0` and `LIMIT 25 OFFSET 40`) — the old
  query (recomputing `MAX(sent_at)` fresh) and the new restructured query
  returned byte-for-byte identical row sets (`EXCEPT` both directions = 0
  rows) in every case.
- Denormalized-column correctness: a table-wide comparison of
  `chat_conversations.last_message_at` against a fresh
  `MAX(sent_at) GROUP BY conversation_id` recomputation across all 80,377
  rows found **0 mismatches**, confirming both the one-time backfill and the
  ongoing trigger are exact, not approximate.
- Cross-doctor isolation on the new write grant: confirmed live — an
  authorized doctor's message insert correctly updated their own
  conversation's `last_message_at`; a different, real, authenticated
  doctor's direct `UPDATE` attempt against that conversation affected **0
  rows** (FORCE RLS held), with the conversation's value left exactly as the
  authorized write set it.
- Backend regression suite: `cargo check --all-targets` clean;
  `cargo clippy --lib --tests -- -D warnings` clean (the pre-existing,
  unrelated `cargo clippy --all-targets` failures in
  `examples/web_workload.rs` predate this session and were not touched);
  `cargo test --lib` **135/135 passed**; full serialized
  `cargo test --test api_tests -- --test-threads=1` against the disposable
  `TEST_REDIS_URL=redis://127.0.0.1:16380/14`,
  `TEST_REDIS_FIXTURE_ID=final-remediation-phase5` fixture — **476 passed, 0
  failed, 5 ignored, finished in 1578.89s (~26.3 min)**.
- `cargo fmt --all -- --check` reports pre-existing, unrelated diffs in
  several files this session did not touch (`examples/capacity_test.rs`,
  `examples/clinical_doc_test.rs`, `examples/rtc_token_scenario.rs`,
  `examples/web_workload.rs`, `src/repository/doctor_repo.rs`,
  `src/services/notification_transport.rs`, `tests/api_tests.rs`,
  `tests/cases/admin_query_g20_investigation.rs`) — these predate this
  session's changes (consistent with the repository's existing large
  uncommitted-work state) and were left alone per scope discipline;
  `services/backend/src/api/v1/doctor_handlers.rs` (the only Rust file this
  pass edited) is not in that diff list.

## Live deployment to `hellodoctor-m8-final-local`

After the regression suite passed clean, the backend Docker image was
rebuilt from this source (`docker build -t hellodoctor-backend:m8-final-local
services/backend`) and the running backend container was replaced with one
from the new image, on the same two Docker networks with the same
`backend` alias, the same env (extracted from the live container via `docker
inspect` and reused verbatim — the override stack's own generated secrets
file had since been cleaned up from disk and was not reconstructed or
guessed), and the same `m8-final-local-journal` volume mount. The old
container was stopped and renamed (not deleted) as a rollback, not removed.
The host port publish (`127.0.0.1:58280`) could not be re-bound due to an
unrelated Windows/WSL2 port-exclusion permission error (`bind: An attempt was
made to access a socket in a way forbidden by its access permissions`) —
verification was done via `docker exec` instead, which does not depend on
that host port; Admin-Web/Doctor-Web reach the backend via the internal
`backend:8080` network alias regardless, unaffected by this.

`GET /healthz` confirmed `"status":"ready"`,
`"binary_latest_migration":"20261204000003"`,
`"database_latest_migration":"20261204000003"`, `"migrations":"current"` —
per `CLAUDE.md` §4 migration discipline, this is the live confirmation that
the running binary and database agree, not just that migration files exist
on disk. `EXPLAIN (ANALYZE, BUFFERS)` re-run directly against the live
`hellodoctor` database (not the clone) for the same test doctor measured
**400ms** (clone: 369ms; within normal variance) — a **5.06x** improvement
over the original live baseline of 2,026ms.

The restart's own idempotent dev-seed check (`SEED_DEV_DATA=true`,
`seed::seed_dev_data`) logged "Development seed data inserted" but is
guarded per-row by `find_by_id(...).is_none()` existence checks (confirmed by
reading `seed.rs`) and only ever touches the fixed-ID canonical demo
accounts, never the benchmark's `scripts/seed_capacity_dataset*.sql` rows —
confirmed a true no-op by comparing row counts before and after across
`chat_conversations` (80,377), `appointments` (96,042), `patient_profiles`
(10,001), `doctor_profiles` (42), and `chat_messages` (149,187): unchanged.

## Explicitly not done this pass

- Task 2 (reconcile the 6 vs. 29-33 reported DB connection figures from
  Phase 4) — not investigated.
- `GET /api/v1/doctor/appointments` and any Admin/Patient-side equivalent
  were not checked for the same MAX-aggregate-before-LIMIT pattern (a search
  confirmed `MAX(sent_at)`/`last_message_at` has exactly one call site in the
  backend — `list_own_chats` — so this specific pattern does not recur
  elsewhere, but a broader efficiency sweep across other endpoints per the
  original Phase 5 plan's Tasks 2-4 was not performed).
- No live six-service comparison run (the controlled, repeatable benchmark
  harness used in Phase 4) was executed against this fix; the numbers above
  are live single-query measurements, not a full concurrent-load re-run —
  recommended as the actual confirmation that this closes (or how much it
  closes) the Phase 4 true-system-error-rate/booking-success gap.
- No full end-to-end HTTP round trip (real doctor login + `GET
  /api/v1/doctor/chats` over HTTP) was run against the live deployment; the
  live verification was a direct `EXPLAIN ANALYZE` of the exact SQL text now
  in `doctor_handlers.rs`, combined with the full HTTP-level integration
  suite passing against the identical code before deployment.
- No git staging/commit/push was performed. Two migration files
  (`20261204000002`, `20261204000003`) and one Rust source file
  (`doctor_handlers.rs`) are uncommitted, consistent with this repository's
  existing large uncommitted-work state.
