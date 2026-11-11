# Phase 7 — Warm-Backend Repeat of the Corrected 200 RPS Benchmark (2026-11-04)

Scope: approved, single repeat of the exact `phase7_final_200_corrected` scenario
(`run_six_service_web_comparison.ps1 -Scenario plausible -DurationSecs 360`,
`-CapacityActiveSessions 2750 -CapacityTargetRps 200`, REALISTIC_RAMP, same
`hellodoctor-m8-final-local` stack, same seeded dataset), with one change: a
throwaway 150s warm-up pass of the same harness run first (discarded, evidence
under `.../scratchpad/m8_warmup_discard/`, not committed to this repo) to drive
Postgres/Redis connection pools and query-plan caches to steady state before the
real measured run. No code, config, migration, admission-limit, or security-control
change was made anywhere in this pass.

**Timing control confirmed**: `docker inspect` on `backend-1`/`postgres-1`/`redis-1`
showed identical container start timestamps immediately before and after the
warm-up pass and again immediately before the measured run — no container
restarted at any point in this pass. The prior `phase7_final_200_corrected` run,
by contrast, began its measurement window only **34 seconds** after `backend-1`'s
own container start (`backend-1` StartedAt `2026-11-03T17:45:52Z` vs. that run's
`MEASUREMENT_WINDOW_START_EPOCH_MS` = `2026-11-03T17:46:26Z`) — that run was a
near-true cold start, not a warm one. This run is the first genuinely warm
measurement of this exact scenario.

## Headline result: the hypothesis under test did not hold

| Metric | Cold run (`phase7_final_200_corrected`) | Warm run (this pass) | Delta |
| --- | --- | --- | --- |
| Full-run achieved response RPS | 194.106 | 193.811 | ~flat |
| Full-run successful-response RPS | 185.178 | 183.897 | ~flat |
| **TRUE_SYSTEM_ERROR_RATE** | **0.4050%** | **0.8313%** | **+0.426 pts, more than doubled** |
| Business rejection rate | 0.016% | 0.043% | worse |
| Auth-reject rate (rtc-token, expected) | 4.176% | 4.235% | ~flat |
| Rate-limited | 0.003% | 0.006% | ~flat |
| Booking success (booked + payment confirmed) | 2,361/2,371 = 99.6% | 2,365/2,380 = 99.4% | slightly worse |
| Backend CPU avg/peak (of host budget) | 61.7% / 97.3% | 57.4% / 118.2% | avg lower, peak higher |
| Postgres CPU avg/peak | 62.2% / 129.1% | 56.7% / 149.6% | avg lower, peak higher |
| Peak total Postgres connections | 45–46 | 40 | fewer, not more |

**Warming the backend before measuring made the error rate worse, not better, while
average CPU was simultaneously lower.** This directly contradicts the working
hypothesis from the prior analysis pass and from `phase7_before_after_scheduler_fix`
that cold-start was a primary driver of the 0.405% result. It instead reproduces,
almost exactly, the same paradox already documented in that earlier before/after
report: lower average CPU coinciding with *more* admission-layer shedding — now
seen twice, across two different variables (image/fix version there, warm-vs-cold
state here), which makes "random single-run variance in a shared, non-isolated
resource" (e.g. host-level contention outside Docker's own CPU accounting, or
a connection-pool-acquisition latency effect that the CPU% metric does not
capture) a more likely common cause than either specific explanation on its own.

## 1. Full-run vs. post-ramp steady-state RPS

| Window | Cold 5xx (sum) | Warm 5xx (sum) |
| --- | --- | --- |
| Ramp, 0–120s (12 buckets) | 35 | 81 |
| Steady state, 120–360s (24 buckets) | 248 | 499 |
| **Total** | **283** | **580** |

Steady-state achieved RPS itself was comparable in both runs (cold ~217–235 RPS,
warm ~218–236 RPS across the steady buckets) — the RPS the generator achieved is
not where the regression shows up. The regression is entirely in steady-state
*error volume* at a similar steady-state throughput: 248 → 499 5xx responses, a
~2.0x increase, while ramp-phase errors also roughly doubled proportionally
(35 → 81). Both runs show the same ~12–14%-ramp/86–88%-steady split of total
errors — ruling out "errors are concentrated in the ramp window" as the
explanation; the warm run is uniformly worse across the whole window, not just
worse early on.

## 2. True system errors — exact counts by cause

Diffing this run's own `pre_test_counters.txt`/`post_test_counters.txt`
(`admission_rejected_total`), all reasons, this run only:

| Class | Reason | Count (warm) | Count (cold) |
| --- | --- | --- | --- |
| normal | dependency_pressure | 202 | 104 |
| normal | lane_concurrency | 99 | 0 (reason did not appear at all) |
| expensive | lane_concurrency | 91 | 66 |
| consultation | lane_concurrency | 128 | 81 |
| consultation | dependency_pressure | 41 | 30 |
| webhook | lane_concurrency | 11 | 6 |
| expensive | dependency_pressure | 16 | 2 |
| payment | lane_concurrency | 2 | 4 |
| payment | dependency_pressure | 1 | 0 (reason did not appear at all) |
| expensive | account_budget | 1 | 6 |
| **Total admission-rejected** | | **592** | **299** |

**`normal`-class `lane_concurrency` is a materially new failure mode** — it never
fired once in the cold run (the class's rejections there were 100%
`dependency_pressure`) but accounts for 99 rejections here, the single largest
qualitative change between the two runs. No admission limit was changed between
runs — this is the same fixed `n.min(64)` normal-lane ceiling (per
`docs/CHECKPOINT.md`'s recorded config) under more sustained real concurrency,
not a reconfiguration.

Client-visible endpoint `syserr` (503/5xx as counted by each generator, summed
across `capacity_test.log`'s 10 endpoints + `web_workload.log`'s 16 Admin/Doctor
endpoints): **580 + 11 = 591** this run, vs. **283 + 10 = 293** in the cold run —
tracking the admission-counter totals closely in both cases (see §7).

No timeouts are separately broken out by the harness (requests either get a
response, a 5xx, or hang past the client's own request timeout, which the
generator buckets as the same `syserr`/`other` category) — this harness does not
distinguish a slow-5xx from a hard client-side timeout; that distinction would
require a HAR-level trace this pass did not add.

## 3. Ramp vs. steady-state errors (short windows)

Already shown in §1's table (10s-bucket breakdown preserved verbatim in both
runs' `capacity_test.log`). Both runs' worst single 10s buckets land in steady
state, not ramp: cold run's worst bucket is 160–170s (60 5xx); warm run's worst
are 280–290s (95 5xx) and 310–320s (66 5xx) — i.e. the warm run's worst moments
are *later* in the window than the cold run's, the opposite of what a
"lingering cold-start tail" theory would predict.

## 4. Per-class rejection counts and thresholds

See §2's table for exact counts. All three threshold *types* that exist in
`services/backend/src/admission.rs` fired in this run: `lane_concurrency` (a
fixed per-lane concurrency ceiling, e.g. Webhook's unchanged ceiling of 4,
Consultation's `n.min(6)`), `dependency_pressure` (checked via `exhausted()`:
true when a relevant `sqlx::PgPool` is at `max_connections` with zero idle
connections — a real pool-saturation signal, not a CPU heuristic), and
`account_budget` (per-account rate shaping, `expensive` class only, 1 this run
vs. 6 cold — the one reason that went *down*). No threshold was edited.

## 5. Booking / payment / consultation success

| | Cold | Warm |
| --- | --- | --- |
| Booking success | 2,361/2,371 = 99.6% | 2,365/2,380 = 99.4% |
| `POST webhook confirm` syserr | 6/2,367 = 0.25% | 11/2,376 = 0.46% |
| `POST chat message` syserr (consultation-adjacent) | 59/6,632 = 0.89% | 101/6,790 = 1.49% |

Consultation/payment integrity itself was not compromised in either run — no
double-charges, no booking-without-payment states were observed in either log —
but the warm run's error rate on these paths roughly doubled too, consistent with
the system-wide pattern, not isolated to one feature.

## 6. Resource headroom and latency

CPU/memory: see headline table. Postgres `pg_connection_summary.csv` peak total
connections were lower in the warm run (40 vs 45–46) — ruling out "more
concurrent DB connections" as the cause of more frequent pool exhaustion; this
points toward connections being held longer per request (slower individual
query/wait time) rather than more of them, consistent with `dependency_pressure`'s
actual definition (pool saturated, not a connection-count ceiling being raised).

Per-endpoint p95/p99 (ms), key paths, cold → warm:
- `GET /doctors`: p95 18→18, p99 54→150 (worse tail)
- `GET /doctors/:id/slots`: p95 21→21, p99 44→168 (worse tail)
- `POST chat message`: p95 41→41, p99 87→135 (worse tail)
- `GET admin/patients` (web_workload): p50 368→420, p95 498→565, p99 509→629
  (worse across the board — the single heaviest query in the system, per the
  prior before/after report's finding about the `encounters` CTE, got slower,
  not faster, after warming)

Every endpoint checked shows p99 flat-or-worse in the warm run; none improved
meaningfully. This is further evidence against "cold start" as the explanation —
a cache/connection-pool warm-up effect should improve tail latency, not degrade it.

## 7. The previously-unresolved 16-count admission-vs-syserr gap

Largely closed. The earlier analysis pass only summed `capacity_test.log`'s own
10-endpoint table (283 for the cold run) against the admission-counter total
(299), leaving an unexplained 16. Adding `web_workload.log`'s Admin+Doctor
per-endpoint `syserr` columns (10 more, cold run) narrows this to **293 vs. 299,
a residual of 6**. Repeating the same reconciliation for this run: `580 (capacity)
+ 11 (web_workload) = 591` vs. `592` admission-rejected — **a residual of just 1**.

The remaining small residuals (6 cold / 1 warm) are most plausibly the
unauthenticated Admin/Doctor-Web login preflight request(s) the harness fires
*before* the measurement window starts (confirmed in both logs: `PREFLIGHT PASSED`
precedes `MEASUREMENT_WINDOW_START_EPOCH_MS`) — these consume the `Auth`-class
admission lane and are captured by the `/metrics` pre/post diff (which spans
script start to script end, wider than the generators' own measurement window)
but are not represented in either generator's per-endpoint table, which only logs
requests inside the measurement window itself. This is a harness instrumentation
gap, not a reconciliation failure in the underlying system — not fully proven
without reading `web_workload.rs`'s preflight code path line-by-line, which this
pass did not do, so stated as "most plausible," not certain.

## 8. Verdict

| | Cold | Warm (this run) |
| --- | --- | --- |
| TRUE_SYSTEM_ERROR_RATE | 0.4050% | 0.8313% |
| vs. ≤0.35% acceptance bar | **Fails** | **Fails, by a wider margin** |

**Per the user's own decision table, this is the "Different failures between
runs" row, not the ">0.35%, same admission causes" row** — `normal`-class
`lane_concurrency` appearing as a materially new rejection reason, plus the
system-wide doubling of error rate alongside *lower* average CPU, means this
is not simply "the same cold-start problem, confirmed." **Recommendation:
investigate repeatability before modifying any code or admission threshold** —
run this same warm scenario 2 more times (as the earlier before/after report
already recommended and this session still has not done) to determine whether
0.83% or 0.41% (or neither) is the more representative number, before treating
either single run as ground truth. Given two single runs have now produced
0.41% and 0.83% — a 2x spread — a fixed admission change sized to "fix" either
number individually would be guessing, exactly what the user's instructions
correctly prohibit without more evidence.

**On "is startup-related reliability still an operational risk?"**: yes,
independent of this run's outcome. The 34-second gap between the cold run's
backend container start and its measurement window start was itself an
accidental, unplanned cold-start test, not a controlled one — a real production
restart/redeploy would face the same near-zero warm-up window unless an explicit
readiness/warm-up gate is added before traffic is routed to a freshly started
instance. This pass did not design or test such a gate; it only ran one
after-the-fact warm-up pass in a benchmark harness. That remains a real,
unaddressed operational gap regardless of which number (0.41% or 0.83%) turns
out to be the "true" steady-state rate.

## Evidence

This run: `capacity_test.log`, `web_workload.log` (+`.stderr`), `docker_stats.csv`,
`pg_wait_events.csv`, `pg_connection_summary.csv`, `pre_test_counters.txt`,
`post_test_counters.txt` — all in this directory. Prior run referenced throughout:
`docs/performance-evidence/milestone-8-final/remediation/phase7_final_200_corrected/`.
Throwaway warm-up pass evidence (not part of this repo, informational only):
local scratchpad directory, discarded per instruction.
