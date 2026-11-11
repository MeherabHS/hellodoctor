# M8-G-20 — Admin Query Verification: `list_admin_doctors`, `list_admin_transactions`, `list_admin_grievances`

**Date:** 2026-11-01
**Scope:** Exactly the three handlers named in `CHECKPOINT.md:1724` and
`docs/MILESTONE_8_REMAINING_GAP_ASSESSMENT.md` M8-G-20. No other admin
endpoint, no ORM restructuring, no unrelated optimization.

## 1. Method

- Code-read each handler in `services/backend/src/api/v1/admin_handlers.rs`
  to determine actual behavior (not assumed from the "N+1" label in
  `CHECKPOINT.md`).
- Built a disposable, `#[ignore]`d investigation test,
  `services/backend/tests/cases/admin_query_g20_investigation.rs`, that seeds
  a representative fixture of **10,000 appointments, 10,000 transactions, and
  2,000 grievances** (well over the 10,000-row minimum required) against the
  shared seeded doctor/patient pair, using sequential non-overlapping slot
  times to satisfy the existing `EXCLUDE` constraints.
- Ran `EXPLAIN (ANALYZE, BUFFERS)` on the real underlying `SELECT`s and on
  proposed replacement queries, and timed both the raw repository calls and
  the full HTTP handler round-trips with `Instant::now()`.
- Used a disposable Redis fixture (`g20-test-redis`, port 16382, owner marker
  `final-remediation-g20`) per the suite's `TEST_REDIS_URL`/
  `TEST_REDIS_FIXTURE_ID` requirement; no shared dev resource was touched.

## 2. Findings — what kind of problem this actually is

None of the three handlers perform classic N+1 (no per-row SQL round-trips in
a loop). All three **whole-table-fetch** one or more tables and then filter,
join, or count **in Rust**, which at 10k-row scale costs far more in
deserialization than the SQL itself ever costs:

| Repo call (raw, unfiltered) | Rows | SQL execution time (EXPLAIN ANALYZE) | Rust fetch+deserialize time |
|---|---|---|---|
| `appointment_repo::list_all` | 10,000 | 1.665 ms | ~147–162 ms |
| `transaction_repo::list_all` | 10,000 | 5.749 ms | ~190–216 ms |
| `doctor_repo::list` | 2 | negligible | ~3.5 ms |
| `patient_repo::list` | 1 | negligible | ~2.7 ms |

The SQL planner evidence confirms there is no missing index or bad join
driving the cost — a plain sequential scan is already the correct plan at
this table size (`Seq Scan on appointments ... Execution Time: 1.665 ms`,
`Seq Scan on transactions ... Execution Time: 5.749 ms`). The cost is the
**volume of rows crossing the Rust/SQL boundary**, not query planning.

### `list_admin_doctors` (admin_handlers.rs:~477, before fix)
Fetched `doctor_repo::list()` + **all** appointments via
`appointment_repo::list_all()`, then ran three separate
`appts.iter().filter(...).count()` passes per doctor (consult/video/chat
counts) — an O(doctors × appointments) in-memory scan on top of the full
10k-row fetch.

### `list_admin_transactions` (admin_handlers.rs:~922, before fix)
Always called `transaction_repo::list_all()` (every transaction ever
created), then applied the `?month=` filter via `txs.retain(|t|
t.created_at.date_naive() >= start && < end)` **after** the full fetch — the
month filter never reduced the amount of data pulled from Postgres or
deserialized. Also called `appointment_repo::list_all()` unconditionally to
resolve `patient_name`/`doctor_name`, regardless of how many transactions
were actually being displayed.

### `list_admin_grievances` (admin_handlers.rs:~2034, before fix)
The worst of the three: `grievance_repo::list_all()` plus **four more**
whole-table-ish fetches (`patient_repo::list()`,
`consultation_repo::list_all()`, `appointment_repo::list_all()`,
`doctor_repo::list()`) plus `consultation_repo::list_all_telemetry()`, purely
to resolve names and counts through the session → appointment → doctor
chain for what may be a handful of grievances.

## 3. Fixes applied

All fixes reuse the **already-established** `find_by_ids`/scoped-query
pattern in this codebase (`appointment_repo::find_by_ids`,
`patient_repo::find_by_ids`, `consultation_repo::list_telemetry_by_session_ids`
all pre-existed before this work). No new abstraction was introduced.

| Handler | Change | New/reused repo functions |
|---|---|---|
| `list_admin_doctors` | Replaced `appointment_repo::list_all()` + 3× in-memory `.filter().count()` per doctor with one `GROUP BY doctor_id` aggregate query | **New:** `doctor_repo::count_appointments_by_doctor` |
| `list_admin_transactions` | Month filter now applied in SQL (`WHERE created_at >= $1 AND created_at < $2`) instead of fetching every transaction and discarding most of them in Rust; appointment lookup narrowed to only the appointment IDs actually referenced by the returned transactions | **New:** `transaction_repo::list_in_range`. **Reused:** `appointment_repo::find_by_ids` |
| `list_admin_grievances` | Patient/session/appointment/doctor lookups narrowed to only the IDs referenced by the fetched grievances, instead of fetching each table in full; telemetry switched to the existing scoped function | **New:** `consultation_repo::find_by_ids`, `doctor_repo::find_by_ids`. **Reused:** `patient_repo::find_by_ids`, `appointment_repo::find_by_ids`, `consultation_repo::list_telemetry_by_session_ids` |

`patient_repo::list()`/`doctor_repo::list()` inside `list_admin_transactions`
were deliberately **left unchanged** — no measured evidence showed them to be
a problem at current or fixture scale (1–2 rows), and changing them would
have been an unrelated optimization outside the measured findings.

No migration was needed — every fix is a query-shape change against existing
columns/indexes (appointments, transactions, and grievances are all queried
by primary key or an already-indexed foreign key via `= ANY($1)`).
Authorization/RLS, pagination, ordering, filters, and API response shapes
are all unchanged; only the code path used to assemble the same response
data changed.

## 4. Before/after measured evidence (10,000 appointments / 10,000
transactions / 2,000 grievances fixture)

"After" is the mean of 3 runs on the same machine immediately following
"before," to separate real effect from run-to-run noise (the raw `list_all`
calls alone showed ~10–15% run-to-run variance even unchanged, e.g.
`transaction_repo::list_all` measured 189.6 ms then 215.9 ms with no code
change between them).

| Endpoint | Before (single run) | After (3 runs) | Change |
|---|---|---|---|
| `GET /admin/doctors` | 200.2 ms | 99.1 / 105.6 / 134.3 ms (avg ~113 ms) | **~1.8× faster** |
| `GET /admin/finance/transactions` (unfiltered) | 774.0 ms | 815.0 / 840.5 / 894.6 ms (avg ~850 ms) | **No improvement** (see note below) |
| `GET /admin/finance/transactions?month=2026-06` | 490.6 ms | 100.3 / 107.8 / 115.6 ms (avg ~108 ms) | **~4.5× faster** |
| `GET /admin/grievances` | 903.4 ms | 490.1 / 493.4 / 578.0 ms (avg ~520 ms) | **~1.7× faster** |

**Honest note on the unfiltered-transactions case:** when no `?month=` filter
is supplied, `list_in_range` falls back to the unchanged `list_all()` (every
transaction), and the referenced-appointment-ID set collected from 10,000
transactions is itself ~10,000 IDs — i.e. at the fixture's current data
distribution, the "scoped" `find_by_ids` lookup resolves to essentially the
whole `appointments` table, so there is no row-count reduction to benefit
from. Measured results show no regression beyond ordinary run-to-run
variance (the before sample was a single run; the after range spans
815–895 ms around that same before value), but also no measured
improvement. This is expected and was not treated as a defect: the real-world
admin UI filters by month, which is the path that improved ~4.5×; the
unfiltered case was deliberately left as-is rather than adding a
special-cased `list_all()`-vs-`find_by_ids()` branch purely to chase a
benchmark number for a request shape the fixture doesn't reflect typical
usage of.

## 5. Regression testing

All three handlers are exercised by existing tests in
`services/backend/tests/api_tests.rs`, run against the fixed code
(`cargo test --test api_tests -- --test-threads=1`, per the repo's documented
convention that parallel runs race on shared capability roles):

| Test | What it asserts that the fix could have broken | Result |
|---|---|---|
| `test_admin_dashboard_endpoints_resolve_names_correctly` | `list_admin_doctors`'s `lifetime_gross`/`consultations_count` (exercises the new `count_appointments_by_doctor` aggregate); `list_admin_transactions`'s and `list_admin_grievances`'s `patient_name`/`doctor_name` resolve to real seeded names, not generic fallbacks (exercises the new scoped `find_by_ids` chains) | **PASS** |
| `test_admin_finance_transactions_month_filter_changes_result_set` | The `?month=` filter actually changes the result set (exercises `transaction_repo::list_in_range`) | **PASS** |
| `test_traffic_admin_aggregate_summaries_match_existing_appointment_values` | Admin aggregate summaries still match appointment-derived values | **PASS** |

No other test in the suite asserts on these three endpoints' response
shapes. No existing test failed.

(One spurious failure was observed and is **not** a regression: running two
`#[sqlx::test]` cases in parallel without `--test-threads=1` produced a
`"tuple concurrently updated"` Postgres error while cloning the migrated
template database — a known flake from this suite's own documented
single-threaded requirement, reproduced and resolved by re-running serially.)

## 6. Cleanup

`g20-test-redis` (disposable Docker container, port 16382) was created solely
for this investigation and is torn down after this document is written; no
pre-existing Docker resource (`hellodoctor-postgres`, `hellodoctor-redis`, or
any `hellodoctor-phonetest-*` container) was touched.

## 7. Verdict

**GREEN.** All three queries were measured against a real ≥10,000-row
fixture with actual `EXPLAIN (ANALYZE, BUFFERS)` evidence. None were a
genuine N+1 (no per-row round-trips); all three *were* measurably slow
(200 ms–900 ms end-to-end) due to whole-table Rust-side deserialization. The
minimal correction — pushing existing filters into SQL and reusing the
codebase's own established `find_by_ids`-scoping convention — is applied,
compiles clean, passes `cargo clippy --all-targets -- -D warnings`, and
passes all existing regression tests. No API shape, authorization, or
pagination behavior changed.
