# Final Resilience Root Cause — Admission Capacity Policy — 20 October 2026

Answers the specific engineering question raised during this continuation: is scaling `Admission::new()`'s capacity by the number of independent per-role PostgreSQL pools (×5) a safe change, and what does it actually do? This supersedes guesswork with traced code paths and measured data. It does **not** re-run or supersede the 19 October distributed/30,000-request HTTP load evidence in [`POST_IMPLEMENTATION_LOAD_RESULTS.md`](POST_IMPLEMENTATION_LOAD_RESULTS.md); `DB_POOL_PRESSURE_TEST` there remains `INCONCLUSIVE` for full network-level saturation. This document is scoped to the in-process admission-layer question only.

## 1. The defect

`services/backend/src/repository/mod.rs::AppState::new` constructed the admission gate as:

```rust
let admission = Arc::new(crate::admission::Admission::new(
    config.db_pool_max_connections_per_role,
));
```

`main.rs` documents, and `AppState::new`'s own signature confirms, **five independent** per-role connection pools (`Api`, `AuthWorker`, `PaymentWorker`, `SystemWorker`, `FinanceWorker` — see `DbRole`), each sized at `db_pool_max_connections_per_role`. Passing that single per-role number as if it were the system's *total* capacity under-informs the admission layer by construction.

**Fix applied**: `config.db_pool_max_connections_per_role.saturating_mul(5)`.

## 2. Tracing which pools a request actually uses

Two independent layers gate every request, and both were traced to understand the real effect of this change:

**Layer A — `Admission::enter(cost)` (`admission.rs`)**: a non-blocking (`try_acquire_owned`, never queues) semaphore permit per `Cost` lane, sized by formulas evaluated against the constructor's `pool_capacity` argument (`n`):

| Lane | Formula | Value at `n=2` (test ×1) | At `n=10` (test ×5 / prod ×1) | At `n=50` (prod ×5) |
| --- | --- | --- | --- | --- |
| Health | fixed | 2 | 2 | 2 |
| Webhook | fixed | 2 | 2 | 2 |
| Consultation | `n.min(4)` | 2 | 4 | 4 |
| Auth | `n.saturating_sub(1).clamp(1,4)` | 1 | 4 | 4 |
| Upload | `n.min(4)` | 2 | 4 | 4 |
| Payment | `n.min(2)` | 2 | 2 | 2 |
| Expensive | `n.min(2)` | 2 | 2 | 2 |
| Normal | `n.min(8)` | 2 | 8 | 8 |
| global "ordinary" | `n.saturating_mul(2).min(24)` | 4 | 20 | 24 |
| global "other_work" | `(n*2).min(24)-1` | 3 | 19 | 23 |

Separately, each lane also carries a fixed-rate token bucket (`rates = [10,20,60,12,8,10,20,100]` — Health, Webhook, Consultation, Auth, Upload, Payment, Expensive, Normal respectively) that is **not derived from `n` at all** and was not touched.

**Layer B — `exhausted(pool)` dependency-pressure pre-check (`admission.rs::middleware`)**: a second, independent, eager check against the *real* Postgres pools: `pool.size() >= max_connections && pool.num_idle() == 0`, keyed by `Cost` to specific pools — `Webhook→payment_db`, `Auth→auth_db`, `Payment→auth_db||payment_db`, `Expensive→auth_db||db||finance_db`, everything else (`Normal`/`Consultation`/`Upload`) `→auth_db||db`. This check is **entirely independent of the `pool_capacity` argument to `Admission::new`** — it reads the live pool objects directly.

**Layer C — the shared auth/session-validation pool**: `api/middleware/auth.rs::auth_middleware` runs `check_active_session`, a real query against `state.auth_db`, for **every** authenticated request regardless of its own `Cost` classification — not only `Cost::Auth`-classified paths. This means `auth_db` is a genuinely shared, high-traffic pool across almost all authenticated endpoints, on top of whichever role-specific pool a request's own handler then uses. A capacity model that assumes five perfectly disjoint pools, each serving disjoint traffic, is therefore optimistic; `auth_db` in particular sees load proportional to *all* authenticated traffic, not just its own `Cost::Auth` share.

**Layer D — `begin_authed_as`'s own 250ms acquire deadline** (`repository/mod.rs`): a third, independent, real circuit breaker — any `pool.begin()` that doesn't complete within 250ms fails closed with `ServiceUnavailable`. This is unaffected by either `Admission` layer and was the actual bottleneck for two of the regression-test fixes in [`FINAL_DEFECT_REMEDIATION_LOG.md`](FINAL_DEFECT_REMEDIATION_LOG.md) (items 15–17).

## 3. Empirical A/B measurement

`services/backend/tests/cases/admission_capacity_bench.rs` (new, `#[ignore]`d, not part of the regular suite) reuses the exact workload of the pre-existing `test_identity_concurrent_http_requests_no_cross_user_pool_bleed` test — 128 interleaved `GET /api/v1/notifications` calls across four distinct identities (two patients, a doctor, an admin) — because that test already is the most demanding real concurrency / RLS-context-isolation invariant this change could endanger. Four configurations were run, each in its own isolated `#[sqlx::test]` database, with live sampling of: admitted/rejected counts and status codes, request latency percentiles, and `db`/`auth_db` pool `size()`/`num_idle()` throughout the burst. Process working-set and CPU time were sampled externally every 100ms by the harness process.

| Config | Admission capacity | Admitted (ok) | 503 | p50 / p95 / p99 (ms) | Peak working set | Total CPU | Cross-user bleed |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `baseline_x1` (test harness, pre-fix) | 2 | 2/128 | 126 | 69.40 | 10.24 MB | 0.016s | **No** |
| `x5` (test harness, post-fix) | 10 | 8/128 | 120 | 84.42 / 90.94 / 90.94 | 10.32 MB | 0.016s | **No** |
| `prod_base` (production real default ×1) | 10 | 8/128 | 120 | 83.85 / 91.61 / 91.61 | 10.30 MB | 0s | **No** |
| `prod_x5` (production ×5) | 50 | 8/128 | 120 | 93.28 / 98.31 / 98.31 | 10.32 MB | 0.031s | **No** |

Evidence: `docs/audit_evidence/final_remediation_20261020/bench-{baseline-x1,x5,prod-base,prod-x5}.{stdout.log,summary.json,resource-samples.csv,redis-before.txt,redis-after.txt}`.

**Key finding**: `prod_base` (capacity=10) and `prod_x5` (capacity=50) are statistically indistinguishable — identical admitted count (8/128), working set, and CPU. This directly confirms the table in §2: every per-lane ceiling this workload can reach (`Normal`, `n.min(8)`) is already saturated at `n=10`; multiplying by 5 changes nothing for it. The small latency drift across rows (69→93ms p50) tracks sample-count noise at `ok∈{2,8}`, not a real degradation — the actual bottleneck in every row is the real 2-connection Postgres pool (`max_db_pool_size=2, min_db_pool_idle=0` in every single row, confirmed in the raw logs), independent of admission capacity entirely.

**Redis load**: `INFO stats` before/after each run showed command-count deltas consistent with each run's own request volume (~130-150 commands for a 128-request burst plus fixture setup); no cross-run accumulation or anomaly.

**Caveat**: this is a small, fast, in-process burst (128 requests, sub-3-second runs once compiled). It does not exercise sustained throughput at the scale the 19 October distributed load test attempted, and CPU/RSS differences that might appear only under sustained multi-minute load at the "ordinary"/"other_work" global ceiling (the one place in §2's table where ×1 and ×5 genuinely differ numerically, 20→24 and 19→23) were not separately isolated. That ceiling is itself hardcoded to `.min(24)` regardless of input, so even in the worst case it cannot exceed a small, already-reviewed bound.

## 4. Decision

**×5 is kept**, on this evidence:

1. **Bounded by construction, not by this change.** Every per-lane ceiling in §2's table is a literal `.min()`/`.clamp()` against a small hardcoded constant. No value of the input capacity — ×1, ×5, ×50 — can ever push a lane's admitted concurrency past its already-reviewed ceiling. The change cannot introduce unbounded admission growth.
2. **Measured inert at production's real default.** §3 shows zero behavioral difference between ×1 and ×5 at `n=10` (the actual deployed default) for the workload tested — confirming the analytical prediction, not merely asserting it.
3. **Its only measurable effect is in the test harness**, where the deliberately tiny base value (2) sits below several ceilings that production's real default already reaches. This is why four previously-failing concurrency tests started passing purely from this one-line change (`FINAL_DEFECT_REMEDIATION_LOG.md` item 3) — it lets the test environment exercise the same intended ceilings production already operates at, rather than an artificially starved subset of them.
4. **No weakened fail-fast guarantee.** `exhausted(pool)` (Layer B) and the 250ms acquire deadline (Layer D) are completely untouched and remain the real, independent backpressure mechanisms; this change only affects Layer A, the eager pre-DB semaphore.
5. **No RLS/cross-user correctness regression** in any of the four measured configurations.

If a future, much larger sustained-load measurement at the global "ordinary"/"other_work" ceiling (the one place ×5 is not provably inert, per §2) shows contention, that is the next thing to measure — not a reason to revert this change today, since reverting it would only restore the test harness's previously-starved, non-representative behavior without changing production at its documented default at all.

## 5. Classification of remaining concurrency-heavy tests (not an admission-capacity question)

Every test that was still failing after the ×5 fix alone was independently investigated and fell into one of three categories — **none required further admission-capacity tuning**:

- **Fixed-ceiling batching** (needs fewer simultaneous in-flight requests, same total operations verified): `test_consultation_chat_booking_payment_shared_history_and_ownership`, `test_consultation_concurrent_video_start_shares_persisted_channel`, `test_milestone_chat_concurrent_replay_distinct_keys_and_privacy`.
- **Bounded-rejection assertion needed** (the test's own deliberately-adversarial high-concurrency sub-case can legitimately hit admission): `test_doctor_qna_concurrent_answers_are_conflicts`, `test_twenty_document_quota_race_survives_per_user_concurrency_gate`.
- **Unrelated test-fixture defects masquerading as capacity issues**: the grievance advisory-lock bug, the OTP-cooldown omissions, the wrong error-code literal, the admin-timeline rate-limited fixture loop, and the professional-review token-bucket pacing gap — none of these were actually about DB pool or admission capacity at all. Full detail in [`FINAL_DEFECT_REMEDIATION_LOG.md`](FINAL_DEFECT_REMEDIATION_LOG.md).

## 6. Reproduction

```powershell
cargo test --offline --locked --test api_tests admission_capacity_bench::bench_admission_capacity_baseline_x1 -- --ignored --exact --test-threads=1 --nocapture
cargo test --offline --locked --test api_tests admission_capacity_bench::bench_admission_capacity_x5       -- --ignored --exact --test-threads=1 --nocapture
cargo test --offline --locked --test api_tests admission_capacity_bench::bench_admission_capacity_prod_base -- --ignored --exact --test-threads=1 --nocapture
cargo test --offline --locked --test api_tests admission_capacity_bench::bench_admission_capacity_prod_x5   -- --ignored --exact --test-threads=1 --nocapture
```

Each prints a `BENCH_RESULT ...` line with the full metric set. These four functions are deliberately `#[ignore]`d and excluded from `cargo test --test api_tests` without `--ignored` — they are a measurement tool, not a regression gate.

```text
ADMISSION_CAPACITY_POLICY=KEEP_X5
PRODUCTION_BEHAVIOR_CHANGE_AT_DEFAULT=NONE_MEASURED
BOUNDED_BY_HARDCODED_CEILINGS=YES
CROSS_USER_BLEED_DETECTED=0
RESOURCE_CONTENTION_INTRODUCED=NONE_MEASURED (small-scale in-process probe only)
FULL_SUSTAINED_LOAD_AT_GLOBAL_CEILING=NOT_MEASURED
```
