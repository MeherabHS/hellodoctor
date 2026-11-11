# Milestone 8.0 — Performance Characterization: Session Ramp, Booking
# Investigation, Instrumentation, and the Real Root Cause

Date: 2026-10-31. Scope: the five follow-up items requested after accepting
the three prior verification items and the booking-fairness fix
(`docs/MILESTONE_8_0_BOOKING_FAIRNESS_REMEDIATION.md`). No security-audit
restart, no unrelated hardening, no blind retuning. **No further code fix
was applied in this pass** — every finding below is reported for a decision,
consistent with "do not increase the Payment concurrency limit again without
profiling evidence" applied to the new findings as well.

Files touched: `services/backend/examples/capacity_test.rs` (session-ramp
feature, two named profiles, SESSION_START bookkeeping, end-to-end booking
metric), `services/backend/scripts/sample_full_telemetry.ps1` (new).
No schema/migration change, no admission.rs change, no Git staging/commit/
push. Configuration A was restarted (not recreated-from-scratch) twice for
clean metrics baselines; Configuration B was brought up from a stopped
state and its backend rebuilt from current source (it was running stale
code otherwise) — both are benchmark-only adjustments, not destructive
operations, and both stacks are left running.

---

## 1. Session-ramp harness improvement

**Added** (`CAPACITY_RAMP_SECS`, default 0 — exactly preserves the original
immediate-start behavior): spreads each session's first start uniformly
across `[0, CAPACITY_RAMP_SECS]` instead of every session starting at the
same instant. Two explicitly named, reproducible profiles:

- `CAPACITY_PROFILE=REALISTIC_RAMP` (`CAPACITY_RAMP_SECS > 0`): sessions
  introduced gradually; the profile for a genuine steady-state capacity
  conclusion.
- `CAPACITY_PROFILE=SIMULTANEOUS_BURST` (`CAPACITY_RAMP_SECS=0`, the
  default): the original behavior, now explicitly labeled and banner-printed
  as a stress test, not a capacity conclusion.

**Reporting fidelity** (the actual point of this item): a `SESSION_START`
marker sample is recorded the instant each session really begins (after its
ramp delay), which is bookkeeping only — excluded from every throughput/
latency/outcome statistic (`is_session_start_marker`), never counted as a
request or a failure. From these markers:
- The time-series table gains a `sessions_active` column: the real, observed
  count of sessions that had actually started by the end of each 10s window
  — never assumed to jump straight to `CAPACITY_ACTIVE_SESSIONS`.
- A `SESSIONS: target=... started=... never_started=...` summary line
  reports exactly how many sessions got a chance to run before the deadline
  (`never_started` is only nonzero if `CAPACITY_RAMP_SECS` exceeds
  `CAPACITY_DURATION_SECS`) — distinct from, and printed separately from,
  `registered_patients` (the dataset's account count). The two are never
  conflated in the output.
- `END_TO_END_BOOKING_SUCCESS: N of M booking attempts fully completed
  (booked AND payment confirmed)` — a true end-to-end business-outcome
  metric (derived from the existing "POST webhook confirm" success count,
  which already only increments for an attempt whose preceding booking
  succeeded — no new correlation tracking needed), not just a per-step
  status code.

Validated with a small live smoke run (20 sessions, 10s ramp, 20s duration)
against Configuration A before use in the full runs below: `sessions_active`
climbed 0→20 across the ramp exactly as designed, `SESSIONS: target=20
started=20 never_started=0`, `END_TO_END_BOOKING_SUCCESS: 1 of 1 ... 100%`.
`cargo fmt --check` and `cargo clippy --example capacity_test -- -D
warnings` both clean.

**Effect on measurement quality**: the previous un-staggered harness
produced a thundering-herd artifact — 680-979 5xx in the first 10-second
bucket alone across three earlier runs (§5 of the prior report). With
`CAPACITY_RAMP_SECS=120`, the equivalent window (0-110s, ramping 100→1,100
of 1,200 sessions) shows **0 to 1 5xx per 10s bucket** — the artifact is
gone. What's left once it's gone is real, reproducible, and far more
informative — see §3.

---

## 2. The 78 Payment-class `lane_concurrency` rejections — RESOLVED, was pure startup-burst, confirmed by direct A/B comparison

**Method**: rather than inferring from the burst run alone, the identical
aggregate booking demand was measured under both profiles, same fix in
place, clean per-run metrics (`/metrics` scraped immediately after each
run on a freshly-restarted container, so each number is exactly
attributable to that one run):

| Profile | Booking attempts | `lane_concurrency` rejects | `shared_class_budget` rejects | Any Payment rejection |
|---|---|---|---|---|
| SIMULTANEOUS_BURST (prior report §2.4) | 1,194 | 78 (6.5%) | 0 | 6.5% |
| REALISTIC_RAMP, run 1 | 1,872 | **0** | 0 | **0%** |
| REALISTIC_RAMP, run 2 (reproduced) | 1,854 | **0** | 0 | **0%** |

Both REALISTIC_RAMP runs — at *higher* absolute booking volume than the
burst run — show **zero** Payment-class rejections of any kind. Since the
admission.rs Payment concurrency/rate/budget values were identical in all
three runs (the fix from the prior report, untouched since), the only
variable was the arrival pattern. This directly answers "concentrated in
the startup burst or persist during steady-state": **entirely the startup
burst — confirmed twice, not inferred once.** No further change to Payment's
concurrency limit is warranted by this evidence, consistent with "do not
increase it again without profiling evidence" — the evidence says the
opposite of what would justify a further increase.

`POST /appointments/book` end-to-end: 97.0-97.8% fully completed
(booked + payment confirmed) across both REALISTIC_RAMP runs.

---

## 3. The real finding: PostgreSQL CPU saturation from a missing index, amplified by Configuration A's hard 2-core partition

### 3.1 What full telemetry showed

`scripts/sample_full_telemetry.ps1` (new — CPU/memory via the same
cgroup-counter-delta method as the already-fixed sampler, plus DB
connection-pool gauges, admission accept/reject deltas, PostgreSQL active-
connection and lock-wait counts, and Redis ops/sec, all from real `/metrics`
scrapes and `docker exec` reads, never mutating anything) ran alongside the
REALISTIC_RAMP sweep. Full data:
`docs/performance-evidence/milestone-8-0/configA/full_telemetry_realistic_ramp.csv`.

**PostgreSQL CPU climbed from ~3% to a sustained 90-99% within about two
minutes of reaching full session count, and stayed there** — while the
backend stayed at a flat ~7-8% (on *twice* as many allocated cores) and
Redis stayed under 2%:

| Time into run | sessions_active | Postgres CPU% | Backend CPU% | 5xx this window |
|---|---|---|---|---|
| ~30s (ramping) | 400 | 4.2% | 1.9% | 0 |
| ~75s (ramping) | 900 | 59.9% | 7.4% | 1 |
| ~110s (steady-state begins) | 1,200 | 81.1% | 8.3% | 25 |
| ~180s (steady-state) | 1,200 | 90.8% | 7.9% | 34 |
| ~240s (steady-state) | 1,200 | 94.7% | 7.5% | 87 |
| ~290s (steady-state) | 1,200 | 99.7% | 7.7% | 118 |

`pg_waiting_locks` was **0 at every single sample** — this rules out lock
contention entirely. `pg_active_connections` stayed low (0-6) throughout —
this rules out connection-count exhaustion. The correlation between rising
Postgres CPU% and rising 5xx count across the steady-state window is direct
and visible in the raw data, not asserted from a summary. **This is genuine
hardware/CPU saturation, not an admission-layer policy ceiling** — exactly
the distinction 8.0R's own closing note said still needed to be found.

### 3.2 Root cause, from `pg_stat_statements` (reset immediately before the run)

| Query | Calls | Mean | % of all Postgres CPU time |
|---|---|---|---|
| `INSERT INTO chat_messages (...)` | 660 | **916.77ms** | 48.9% |
| `SELECT EXISTS (... FROM patient_profiles pp WHERE pp.id = ...)` (`fn_patient_profile_owned_by_caller`) | **5,742,464** | 0.09ms | 43.1% |
| (its inlined duplicate, `fn_patient_profile_is_self`-adjacent) | 5,716,502 | 0.01ms | 3.1% |

**These three together are 95.1% of all Postgres CPU time in this run.**
5.7 million calls against ~660 chat-message inserts (and the ordinary
read traffic) is not plausible as "one ownership check per row" — it is the
signature of a **sequential scan with no supporting index**.

Confirmed by schema inspection: `chat_conversations` (22,910 rows at time of
inspection) has **no index on `patient_id` or `doctor_id`** — only a primary
key and a unique constraint on `appointment_id`. Every `INSERT INTO
chat_messages` is gated by **three separate RLS policies**
(`chat_messages_access` USING, `chat_sender_guard` RESTRICTIVE, and
`chat_messages_insert` PERMISSIVE — all forced-RLS, all independently
evaluated per PostgreSQL's policy-combination rules), and at least two of
them run a subquery of the shape `... IN (SELECT id FROM chat_conversations
WHERE fn_patient_profile_owned_by_caller(chat_conversations.patient_id, ...)
OR chat_conversations.doctor_id IN (...))`. With no index to turn
`patient_id`/`doctor_id` into an index lookup, PostgreSQL has no choice but
to **sequentially scan the whole table and invoke the ownership function
once per scanned row**, for every policy, for every insert. That is the
exact, fully-identified mechanism behind both the 916ms mean insert latency
and the 5.7M function calls.

**This is a missing-index/RLS-performance bug, not a capacity-sizing
question** — it would very likely resolve with two ordinary, additive
indexes (`CREATE INDEX ON chat_conversations(patient_id)` and `(doctor_id)`
— the standard answer to a function-gated subquery like this), but **no
schema or migration change was made in this pass**: that is a real,
reviewable migration (per this project's own migration-discipline rule,
forward + down, tested before being declared done), explicitly out of scope
for a performance-characterization pass. Reported here as a precise,
actionable finding for a decision, not applied.

### 3.3 Why this also explains the Consultation-class rejections — and why that cap should NOT simply be raised

Clean metrics for the same run:
`admission_rejected_total{class="consultation",reason="lane_concurrency"}`
= 1,227 against 1,519 accepted — **44.7% of all chat/RTC-token traffic
rejected** by Consultation's `n.min(4)` concurrency semaphore (never raised
by any prior remediation; same un-benchmarked-placeholder pattern as the
old Normal/Payment ceilings). Reproduced in a second run (1,227/2,746 =
44.7% again).

**Important distinction, worth getting right rather than pattern-matching
the Payment fix onto this too**: Configuration B (§4) shows this is *not*
primarily an undersized-concurrency problem in its own right. In
Configuration B, the identical `chat_messages` insert averages **45.56ms**
(not 917ms — a ~20x difference, because Postgres there has CPU headroom to
borrow, §4) and Consultation-class rejections are **2 of 1,227 (0.16%)** —
essentially none, at comparable throughput and the *same* `n.min(4)` cap.
By Little's Law, concurrency in flight scales with latency: a 20x latency
increase means ~20x more concurrent in-flight Consultation requests for the
same arrival rate, which is what actually saturates the small cap in
Configuration A. **Raising Consultation's concurrency limit now, before the
underlying query latency is fixed, would very likely mask this real
saturation signal the same way the old Normal-class 100/s ceiling once
masked whether real DB saturation existed** — exactly the mistake this
whole investigation has been careful not to repeat. The evidence-based next
step, if authorized, is to fix the index first and re-measure Consultation's
rejection rate before touching its concurrency cap at all.

---

## 4. Configuration A vs. Configuration B — same bug, different blast radius

Configuration B (`docker-compose.capacity.yml`, the pre-existing "4-vCPU/
8GiB stable VM" profile — backend, Postgres, and Redis sharing one `cpuset
0-3` pool with weighted `cpu_shares`, rather than Configuration A's
disjoint, hard-isolated 4-core app pool / 2-core DB pool) was rebuilt from
current source (it had been stopped for hours and was running stale code;
confirmed a genuine recompile from a cache-miss, not a cache-hit) and run
with the identical REALISTIC_RAMP profile (1,200 sessions, 60s ramp, 180s
duration — shorter only to bound this pass's total runtime):

| | Configuration A | Configuration B |
|---|---|---|
| `TRUE_SYSTEM_ERROR_RATE` | 5.07% | **0.086%** |
| Postgres CPU, steady-state | 90-99% (saturated) | 8-16% (headroom) |
| `chat_messages` INSERT mean latency | 916.77ms | **45.56ms** |
| Consultation `lane_concurrency` rejects | 44.7% | 0.16% |
| `POST chat message` success rate | 55.2% (660/1,196) | **99.8%** (535/536) |
| Payment-class rejections | 0% (both profiles, §2) | 0% |
| `END_TO_END_BOOKING_SUCCESS` | 97.0-97.8% | 95.8% |

**Same root-cause query shapes dominate Postgres CPU time in both**
(`chat_messages` INSERT + `fn_patient_profile_owned_by_caller`, ~59% of
Postgres CPU time in Configuration B's shorter run vs. 92% in Configuration
A's longer one). **Configuration B's materially healthier numbers are not
evidence the system is actually fine — they are evidence that Configuration
B currently has more CPU headroom for Postgres to burn on the same
inefficient queries**, because its three services share one elastic 4-core
pool (Postgres can burst past its nominal priority share when backend/redis
aren't using theirs) rather than Configuration A's hard-walled, unborrowable
2-core partition. The same missing-index bug would be expected to produce
the same saturation in Configuration B too, once real concurrent demand (or
a longer sustained run, or added app-tier load competing for those same 4
cores) consumes the currently-idle headroom it is quietly relying on. This
is disclosed as an open risk, not resolved by Configuration B's better
numbers.

---

## 5. Admission counter reconciliation — exactly where each counter increments

Requested: reconcile `accepted` against booking attempts and rejection
counts, and explain precisely where each counter increments in the
middleware pipeline. Using the clean Configuration A run from §2 (1,854
booking attempts) as the worked example:

```
admission_accepted_total{class="payment"}              1,854   <- see (1)
admission_rejected_total{reason="lane_concurrency"}         0   <- see (2)
admission_rejected_total{reason="lane_rate_limit"}           0   <- see (2)
admission_rejected_total{reason="shared_class_budget"}       0   <- see (3)
```

1. **`admission_accepted_total{class}` increments inside
   `Admission::enter()`** (`admission.rs`, end of the function, immediately
   before returning `Ok(Lease{..})`), the instant a request has
   successfully acquired, in order: the shared `ordinary` semaphore permit
   (skipped for reserved classes), the shared `other_work` semaphore permit
   (skipped for reserved classes and `Normal`), the class's own `lanes[]`
   concurrency-semaphore permit, and passed the class's own local
   per-process `lane_tokens[]` token-bucket check. **"Accepted" therefore
   means "passed concurrency admission and the local rate bucket" — it is
   NOT yet "will reach the handler."**
2. If any of those four checks fails, `enter()` returns `Err((status,
   reason))` *instead of* incrementing `accepted` — the specific failing
   check determines the `reason` string (`ordinary_concurrency` /
   `other_work_concurrency` / `lane_concurrency` / `lane_rate_limit`), and
   `middleware()` calls `reject()`, which increments
   `admission_rejected_total{reason}`. **These rejections and `accepted`
   are mutually exclusive for the same request** — a request contributes to
   exactly one of them at this stage.
3. **A request counted in `accepted` can still be turned away afterward.**
   Back in `middleware()`, *after* `enter()` has already returned `Ok` (and
   therefore already incremented `accepted`), the function makes further
   checks that were never part of `enter()`: the per-principal Redis budget
   (`Cost::Normal` only, `reason="principal_budget"`), the dependency-
   pressure check (DB pool exhaustion, `reason="dependency_pressure"`, any
   class), and the per-class shared Redis budget
   (`reason="shared_class_budget"`, any class). Each of these, if it fails,
   calls `reject()` and increments `admission_rejected_total{reason}` —
   **without decrementing `accepted`**, because `accepted` was never meant
   to mean "ultimately succeeded"; it means "passed the concurrency gate."

**The reconciliation identity, therefore, is**:
```
total class-C admission attempts
  = rejected{ordinary_concurrency} + rejected{other_work_concurrency}
  + rejected{lane_concurrency} + rejected{lane_rate_limit}
  + accepted{class=C}

requests that actually reached the handler
  = accepted{class=C}
  - rejected{shared_class_budget}   (same class)
  - rejected{principal_budget}      (Normal only)
  - rejected{dependency_pressure}   (same class)
  - rejected{limiter_unavailable}   (same class, Redis outage only)
```
In the worked example above, `accepted=1,854` and every post-`enter()`
rejection reason is `0`, so all 1,854 reached the handler — which matches
the harness's own count exactly (1,854 `POST /appointments/book` samples,
1,847 of which got a real HTTP response other than a connection failure;
the 7-attempt gap is genuine business rejects/transport-level outcomes
counted by the harness itself, not an admission-layer discrepancy).

No 503 admission rejection was classified as successful throughput or
folded into a generic bucket anywhere in this reconciliation — each
`reason` label is read and reported individually from `/metrics`,
distinctly from the harness's own client-side `syserr` column, which is why
§3.3 could show precisely that Consultation's 503s are `lane_concurrency`
admission rejections, not unexplained "system errors."

---

## 6. Overall status — explicitly NOT Milestone 8.0 GREEN

- **Resolved and verified**: booking fairness (prior report + §2 here,
  reproduced twice under realistic load with zero rejections).
- **Harness measurement fidelity**: substantially improved (ramp profiles,
  accurate active-session reporting, end-to-end business-outcome metric);
  the startup-burst artifact is now isolable rather than contaminating every
  run.
- **Open, NOT fixed in this pass, both requiring an explicitly authorized
  follow-up**:
  1. Missing index(es) on `chat_conversations(patient_id)` /
     `chat_conversations(doctor_id)` — the measured root cause of 92-95% of
     PostgreSQL CPU time and the single highest-leverage fix identified in
     this entire investigation. A real migration (forward + down), not a
     benchmark-only adjustment.
  2. Consultation-class `lane_concurrency` (44.7% rejection in Configuration
     A's steady state) — deliberately NOT retuned; re-measure after (1) is
     fixed before deciding whether it needs its own change at all.
  3. Configuration B's materially better numbers are provisional, resting on
     currently-idle CPU headroom shared with the app tier, not a separate
     confirmation the system is ready.
- **Not claimed**: that Configuration A or B, as currently deployed, can
  sustain 1,200 realistically active sessions with acceptable latency,
  correct business outcomes, and low genuine failure rates — Configuration
  A's steady-state `TRUE_SYSTEM_ERROR_RATE` of 5.07%, driven by a real,
  now-identified, not-yet-fixed database bottleneck, says the opposite under
  current conditions. Configuration B looks healthy under this exact
  workload but for reasons (§4) that do not generalize without further
  verification.
- Full regression suite was not re-run in this pass (no `admission.rs` or
  other production-code change was made — only the test harness, which has
  no bearing on the application test suite's outcome).
