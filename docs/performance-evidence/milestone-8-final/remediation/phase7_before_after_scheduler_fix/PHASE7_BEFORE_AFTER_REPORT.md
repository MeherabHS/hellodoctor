# Phase 7 — Before/After Scheduler-Fix Causal Breakdown (2026-11-03)

Scope: a single matched-pair six-service benchmark (`run_six_service_web_comparison.ps1
-Scenario plausible -DurationSecs 360`, 2,000 concurrent sessions, REALISTIC_RAMP, the
existing ~10k-patient/42-doctor synthetic dataset, unchanged) against the live
`hellodoctor-m8-final-local` stack — BEFORE = the backend image already running
(pre-scheduler-fix, but post admin-telemetry-index/chat-denorm Phase 5/6 fixes),
AFTER = the already-built `hellodoctor-backend:m8-final-local` scheduler-fix image,
freshly deployed for this run. No code was changed in this pass. This adds a new
read-only "causal watcher" (`scripts/pg_causal_watcher.ps1`) alongside the existing
`sample_live_instrumentation.ps1`: per-tick active-query snapshots with wait event +
age, `pg_stat_database` temp-file tracking, and ranked `pg_stat_statements` snapshots
every 30s, reset to zero at the start of each run. `track_io_timing` was turned on
(reversible, diagnostic-only, no query-shape change).

**Operational note**: the redeploy itself briefly produced two backend containers
running simultaneously (Compose's rename-on-recreate collided with a stale container
left from an earlier manual redeploy). Caught and corrected immediately — removed the
stale container, renamed the new one to its canonical name, confirmed it was the sole
instance, healthy, migrations current, before the AFTER run started.

## Top-line business/API metrics

| Metric | BEFORE (pre-fix) | AFTER (scheduler-fix) | Delta |
| --- | --- | --- | --- |
| True system error rate | **9.42%** | **15.90%** | +6.5 pts (worse) |
| Business rejection rate | 0.044% | 0.019% | better |
| Auth-reject rate | 3.22% | 2.93% | ~flat |
| Achieved sustained RPS (steady state) | ~170 | ~168 | ~flat |
| Booking success (booked + payment confirmed) | 79.4% (3132/3946) | 71.6% (2682/3746) | -7.8 pts (worse) |
| Admin Web syserr share (8 endpoints) | spread 0–8/41 per endpoint | spread 0–8/44 per endpoint | ~flat, slightly worse |
| Doctor Web syserr share (8 endpoints) | spread 0–5/76 per endpoint | spread 0–6/75 per endpoint | ~flat, slightly worse |

**Headline paradox**: the AFTER run's resource usage was measurably *lower*, not higher:

| Resource | BEFORE | AFTER |
| --- | --- | --- |
| Postgres CPU — peak / avg (of 4-core budget) | 347.0% / 160.1% | 254.7% / 88.3% |
| Backend CPU — peak / avg | 247.7% / 111.2% | 171.1% / 69.9% |
| Postgres `IO` / `LWLock` wait-sample counts | 10 / 7 | 3 / 2 |

So the system used **less** CPU and had **fewer** I/O/lock wait samples in the AFTER
run, yet produced a **worse** error rate and **worse** booking success. This rules out
"the scheduler fix made the database do more work" — it demonstrably didn't.

## Why the error rate went up despite lower CPU: admission-control shedding, not DB cost

Diffing the backend's own `/metrics` admission counters (pre/post each run) gives a
near-exact match to the measured error-rate gap:

| Rejection reason (summed across all classes) | BEFORE | AFTER |
| --- | --- | --- |
| `lane_concurrency` (fixed per-lane concurrency ceiling hit) | 3,809 | 6,685 |
| `dependency_pressure` (dynamic backpressure heuristic) | 1,135 | 1,593 |
| `limiter_unavailable` / `account_budget` | 30 | 6 |
| **Total rejected** | **4,977** | **8,284** |
| Total accepted | 49,380 | 45,923 |
| Implied rejection rate | ~9.2% | ~15.3% |

This tracks the measured `TRUE_SYSTEM_ERROR_RATE` almost exactly in both runs — the
increase is **admission-layer load-shedding**, not slower queries. Since CPU was
simultaneously *lower*, the most coherent reading is that requests spent more wall-clock
time **waiting** (not CPU-busy) in the AFTER run — e.g. connection-pool acquisition,
internal queueing, or cold-cache effects from the backend having just been restarted —
which fills the fixed-size `lane_concurrency` slots faster and trips the
`dependency_pressure` heuristic more often, without ever showing up as higher CPU%.
`normal`-class `lane_concurrency` rejections roughly doubled (2,139 → 4,422), the single
largest contributor to the gap.

**This should not yet be read as "the scheduler fix regressed the system."** Candidate
explanations, none confirmed:
1. **Single-run variance.** The prior Phase 4 investigation measured 11.65–13.39% true
   error rate across three repeated runs of this *exact* scenario (3-admin/5-doctor
   workers) — a ~1.7-point band on its own. A single BEFORE/AFTER pair this far apart
   (9.42% vs 15.90%) is suggestive but not proof; this was one run each, not repeated.
2. **Cold-start / sequential-run bias.** The AFTER backend was a container recreated
   seconds before its run started (fresh connection pools, fresh query-plan cache,
   fresh in-process admission-pressure state), while BEFORE ran against a backend that
   had been warm for ~21 minutes. Postgres itself was never restarted between runs
   (same continuously-running container), so its buffer cache carried over — but that
   also means the AFTER run inherited more WAL/bloat/temp-file pressure accumulated
   from the BEFORE run (see temp-file growth below), a known sequential-ordering
   confound.
3. A genuine behavioral change from the scheduler-fix code was not ruled in or out —
   the fix's own target queries (`INSERT INTO notifications`, the notification
   SELECTs) are confirmed cheap in both runs (next section), so if the fix is
   responsible it is not through added query cost on the request path.

**Recommendation**: do not treat this single pair as decisive either way. Re-run both
configurations 2–3 times each, ideally alternating order (AFTER-then-BEFORE as well as
BEFORE-then-AFTER) to cancel sequential bias, before concluding the scheduler fix
helped, hurt, or was neutral for this specific six-service scenario.

## What's actually causing the PostgreSQL load — identical in both runs

Ranking `pg_stat_statements` by cumulative `total_exec_time` at the end of each
6-minute window (reset to zero at each run's start) shows the **same statements in the
same order, at similar magnitudes**, in both BEFORE and AFTER. The scheduler fix is
confirmed **not** to touch any of these — they are pre-existing, independent of this
pass's change:

| Rank | Query (shape) | BEFORE calls / total ms / mean ms | AFTER calls / total ms / mean ms | Classification |
| --- | --- | --- | --- | --- |
| 1 | `WITH encounters AS (SELECT a.id,... )` — consultation-history CTE (backs Admin Patients / Doctor history views) | 71 / 168,852 / **2,378** | 67 / 136,356 / **2,035** | **Complexing** — by far the most expensive per-call statement in the system |
| 2 | `SELECT EXISTS(...patient_profiles... OR fn_patient_pr...)` — patient RLS/ownership predicate | **913,038** / 134,475 / 0.15 | **881,582** / 109,816 / 0.12 | **Burdened by volume** — cheap per call, dominant purely from ~900K calls/6min (~2,500/sec) |
| 3 | `WITH base AS (SELECT c.id,... FROM chat_conversations c...)` — doctor chat list | 76 / 48,417 / 637 | 71 / 38,691 / 545 | Complexing, moderate volume |
| 4 | `SELECT id, transaction_number,... FROM transactions...` | 78 / 30,394 / 390 | 72 / 25,857 / 359 | **Memory load** — see temp-spill section below |
| 5 | Admin finance aggregate (`COUNT(*) FILTER... SUM(...) FILTER... EXISTS(...)`) | 40 / 21,707 / 543 | 35 / 15,762 / 450 | Complexing |
| 6 | `SELECT d.id FROM doctor_profiles WHERE user_id = NULLIF(current_setting(...))::uuid` — doctor identity lookup | **855,418** / 17,944 / 0.02 | **832,459** / 14,495 / 0.02 | Burdened by volume |
| 7 | `SELECT EXISTS(...patient_profiles WHERE id=... AND user_id=...)` — patient identity lookup | **858,679** / 16,445 / 0.02 | **835,481** / 13,266 / 0.02 | Burdened by volume |
| 8 | `doctor_schedule_slots WHERE doctor_id=... AND status=...` | 8,796 / 15,279 / 1.74 | 8,186 / 12,673 / 1.55 | Moderate |
| 9 | `appointment_notification_recipients($1)` | 3,104 / 13,979 / 4.50 | 2,663 / 10,521 / 3.95 | Moderate |
| 10–15 | appointments/consultation_trace_events/directory/booking-check queries | similar magnitude in both | similar magnitude in both | Moderate |

**Takeaway**: items #2, #6, #7 are the same shape of problem the Nov 2 doctor-side RLS
fix (`fn_current_doctor_profile_id()`) already solved for `doctor_profiles` — but a
**patient-side equivalent is still missing**. `fn_patient_pr...` (likely
`fn_patient_profile_*`/an ownership-check function) and the raw `patient_profiles`
identity `EXISTS` check are each evaluated 800K–900K+ times per 6-minute window,
exactly mirroring the pre-fix doctor pattern. This is the single most promising **next**
optimization target, worth more cumulative time (151K–134K ms combined across #2/#6/#7
in the BEFORE run alone) than the admin-telemetry index fix saved.

Item #1 (the `encounters` CTE) is the most expensive **per call** statement in the
entire system by a wide margin (2.0–2.4 seconds/call) despite low call volume — this
is "complexing," not "burdened," and is the direct cause of Admin Patients'/Doctor
history's multi-second p95/p99 tails seen in both web-workload logs
(`admin/patients` p50 2.3–2.8s, p95 5.2–7.6s).

## Memory load: a real, reproducible ~2GB-per-6-minutes temp-file spill

`pg_stat_database.temp_bytes` for the `hellodoctor` database, sampled every ~3s:

| Run | temp_files start → end | temp_bytes start → end | Spilled this run |
| --- | --- | --- | --- |
| BEFORE | 0 → 117 | 0 → 2,115,174,400 (2.10 GB) | **+2.10 GB** |
| AFTER | 117 → 228 (continues accumulating, same Postgres container) | 2,115,174,400 → 4,191,936,512 | **+2.08 GB** |

Essentially identical spill volume in both runs — this is a stable, reproducible
pattern, not noise. It is directly traceable to query #4 above
(`SELECT id, transaction_number,... FROM transactions...`), whose
`temp_blks_written` alone accounts for ~177K–185K blocks (~1.4–1.5 GB) per run — a
sort or hash operation spilling to disk because the result set (`rows` column shows
8.0–8.5 million rows returned for just ~75 calls, i.e. ~108K rows/call) exceeds
`work_mem`. This is genuine unnecessary memory/I/O pressure: a query that should almost
certainly be scoped with a narrower filter or a covering index rather than sorting
millions of rows per call. A close second contender is query #14 in the full data
(`SELECT DISTINCT ON (appointment_id) appointment_id, payment_status... FROM
transactions ORDER BY...`), which returns a similarly large row count without
temp-block evidence in its own stat row but shares the same `transactions`-wide-scan
shape — worth checking whether it's the same logical call site under a different
prepared-statement form.

## Confirmed: the scheduler fix's own target queries are cheap and non-dominant here

`INSERT INTO notifications (...)` (two prepared-statement variants): 5,326 + 1,771
calls, 1,499.95 ms + 749.93 ms total — under 2.3 seconds combined, versus the top
query's 136,000+ ms. `SELECT * FROM notifications WHERE user_id=...`: 69 calls, 851.88
ms total. **This benchmark does not exercise the pathology the scheduler fix actually
addressed** (the 16,902-row re-render-forever bug required an accumulated real-time
backlog of already-notified historical rows that a fresh 6-minute synthetic run starting
from `pg_stat_statements_reset()` does not reproduce). This run is valid evidence that
the fix adds no new expensive hot-path query, but it cannot confirm or deny the fix's
real-world benefit — that requires either a long-duration run or a seeded backlog
scenario, consistent with the originally-reported 580x isolated measurement rather than
a six-service comparison.

## What this does and doesn't tell us

**Confirmed, reproducible (same in both runs)**:
- The `encounters` CTE is the single most expensive per-call statement system-wide.
- Patient-side RLS/ownership identity checks are called 800K–900K+ times per 6 minutes
  — the same shape of problem already fixed for doctors, not yet fixed for patients.
- A `transactions`-table query spills ~2 GB of temp files to disk per 6-minute window,
  a genuine unnecessary-memory-pressure finding, not just a slow query.
- The scheduler fix's own queries are cheap; this benchmark cannot validate its real
  intended benefit (backlog accumulation over real time).

**Not yet established** (needs repeat runs before acting on it):
- Whether the scheduler-fix backend is actually *worse* for this scenario, or whether
  the observed error-rate/booking-success regression is single-run variance and/or
  cold-start/sequential-run bias from the redeploy. The admission-counter diff shows
  the gap is real and attributable to load-shedding, not measurement noise in the
  error-rate number itself — but *why* the shedding increased (genuine latency
  regression vs. cold-start vs. ordering bias) is not yet isolated.

## Recommended next step (not yet done — explicitly holding per instruction)

Before any further fixes: re-run this exact pair 2–3 times each, with run order
alternated, to separate genuine scheduler-fix effect from cold-start/sequential bias.
Then, independent of that outcome, the two standing, evidence-backed, next-fix
candidates are (a) a `fn_current_patient_profile_id()`-style STABLE wrapper mirroring
the Nov 2 doctor-side RLS fix, and (b) investigating/narrowing the `transactions` query
behind the ~2GB/6min temp-file spill. Neither has been touched in this pass.

## Evidence

All raw files under this report's directory:
`before/` and `after/` each contain `capacity_test.log`, `web_workload.log`,
`docker_stats.csv`, `pg_wait_events.csv`, `pg_connection_summary.csv` (existing
instrumentation), plus the new `pg_active_queries.csv`, `pg_db_stats.csv`, and
`pg_stat_statements_snapshot_*.csv` series (this pass's causal watcher), and
`pre_test_counters.txt`/`post_test_counters.txt` (admission metrics).
