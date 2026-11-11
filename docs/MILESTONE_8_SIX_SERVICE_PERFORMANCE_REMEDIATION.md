# Milestone 8 — Six-Service Performance Remediation (Step 3)

Status: **narrow diagnostic remediation complete. NOT a 60-minute acceptance run. Do not read anything here as GREEN acceptance.**

Stack under test: `hellodoctor-m8-final-local` (backend, postgres, redis, admin-web,
doctor-web, Caddy proxy), 4 vCPU / 8 GB host, `deploy/single-vm/compose.yml` +
`deploy/single-vm/compose.local-verification.yml`.

## 1. Root-cause evidence recap (from prior Step 2 work)

The contaminated 60-minute run's valid Phase A segment (48.3 min, admin-web/doctor-web
genuinely under load) measured 117,310 total admission rejections, split almost
entirely between two mechanisms:

| Mechanism | Count | % | What it is |
|---|---|---|---|
| `lane_concurrency` | 63,830 | 54.4% | Per-class concurrency semaphore exhausted (instant fail) |
| `dependency_pressure` | 53,470 | 45.6% | A DB pool 100% checked out, zero idle connections |
| `limiter_unavailable` | 9 | 0.008% | Negligible |

Within `lane_concurrency`: Consultation (chat/rtc-token/telemetry, capped at
`n.min(4)` regardless of pool size) was the single largest source at 31,687;
Webhook (hardcoded `2`) contributed 8,132; Expensive (`n.min(2)`) contributed
5,486+1,574. `dependency_pressure` was concentrated in Normal-class reads
(43,650) because the DB pool (`n = DB_POOL_MAX_CONNECTIONS_PER_ROLE × 5` roles,
50 at the default of 10/role) was the real constraint there, not Normal's own
lane cap (`n.min(64)`, never binding at n=50).

**Important scope correction made during this stage:** Phase A's degradation was
driven by the *combination* of this API load **and** sustained admin-web/doctor-web
CPU contention on the same 4 cores (confirmed separately: Phase B, web traffic idle,
dropped true error rate from ~21.5% to ~0.39% at the same RPS). The
`web_page_workload.sh` script that generated that admin/doctor-web traffic was
ad hoc and was never persisted anywhere in the repository, so it could not be
reconstructed for this stage without introducing a brand-new, unvalidated
confound into what is supposed to be a narrowly-scoped experiment. **All tests in
this document ran with admin-web/doctor-web present and healthy but idle (API
load only).** They isolate the DB-pool/admission-lane mechanics in isolation; they
do **not** reproduce or re-measure the admin/doctor-web-driven degradation itself,
which remains characterized only by the prior Phase A/B figures.

## 2. Methodology note: a harness bug invalidated the first Stage A attempt

The load generator (`services/backend/examples/capacity_test.rs`) was pointed at
the stack through the Caddy proxy (`CAPACITY_BASE_URL=http://127.0.0.1:58080`)
without setting `CAPACITY_HOST_HEADER=api.localhost`. Caddy's Host-based vhost
routing does not match a bare `127.0.0.1:58080` Host header against its
`http://api.localhost { ... }` site block, and — confirmed directly with
`curl` — falls through to a blank `200 OK, Content-Length: 0` response that
never reaches the backend at all. The first Stage A run therefore recorded
100% "success" at ~1 ms latency with zero admission-metric movement because it
tested nothing; it is preserved for the record at
`docs/performance-evidence/milestone-8-final/remediation/pool12_consult4_INVALID_no_host_header/`
and excluded from all conclusions below. Every run reported here used
`CAPACITY_HOST_HEADER=api.localhost` and was spot-checked against real
booking/webhook/DB activity before being trusted.

All corrected runs: `CAPACITY_MODE=sessions CAPACITY_PROFILE=REALISTIC_RAMP
CAPACITY_RAMP_SECS=120 CAPACITY_THINK_MIN_SECS=5 CAPACITY_THINK_MAX_SECS=20
CAPACITY_DURATION_SECS=360` (120s ramp + 240s steady state), same 10,001
patient / 42 doctor / 11 admin dataset throughout, no reseeding. Short,
comparable runs per the Step 3 instruction — not another 60-minute benchmark.

## 3. Before/after configurations

| Stage | DB_POOL_MAX_CONNECTIONS_PER_ROLE | Pool capacity (×5 roles) | Consultation lane cap | Sessions | Evidence dir |
|---|---|---|---|---|---|
| Baseline | 10 | 50 | `n.min(4)` = 4 | 2000 | `remediation/baseline_pool10_consult4_clean/` |
| Stage A | 12 | 60 | `n.min(4)` = 4 | 2000 | `remediation/pool12_consult4/` |
| Stage B | 10 (A reverted) | 50 | `n.min(6)` = 6 | 2000 | `remediation/pool12_consult6/` (directory name kept per the instruction's naming; `ACTUAL_CONFIG.txt` inside records the real pool value) |
| 200 RPS exploratory | 10 | 50 | `n.min(6)` = 6 | 2400 | `remediation/stress_200rps/` |

Stage A's pool override was applied as an explicit `DB_POOL_MAX_CONNECTIONS_PER_ROLE: "12"` /
`"10"` line under `backend.environment` in `deploy/single-vm/compose.local-verification.yml`
— deliberately **not** touching `services/backend/.env`, which is a shared secrets
file also read by unrelated running stacks (phonetest, confa, the 3-week-old main
dev DB). This file is currently untracked in git (`??` in `git status`); the final
state left in place is `DB_POOL_MAX_CONNECTIONS_PER_ROLE: "10"` (Stage A reverted).

Stage B's change, kept, in `services/backend/src/admission.rs`:
```rust
// Milestone 8 Step 3 (docs/MILESTONE_8_SIX_SERVICE_PERFORMANCE_REMEDIATION.md):
// Consultation (chat/rtc-token/telemetry) was the single largest
// `lane_concurrency` rejection source in the Phase A six-service
// evidence (31,687 of 117,310 total admission rejections), capped at
// `n.min(4)` regardless of how large the DB pool actually was — the
// same "fixed ceiling independent of pool_capacity" shape already
// corrected once for Payment above. Raised to `n.min(6)` to test
// whether real headroom exists, following that exact precedent.
let limits = [
    2,
    2,
    n.min(6),   // was n.min(4)
    auth_limit,
    ...
];
```
The two existing unit tests that hardcoded the Consultation expectation were
updated to `n.min(6)`; `cargo test --lib admission` (5/5 tests) passes. The
backend Docker image (`hellodoctor-backend:m8-final-local`) was rebuilt from
this source and the container recreated before Stage B's test ran.

Webhook (hardcoded `2`) and Expensive (`n.min(2)`) were left untouched throughout,
per Stage C instructions.

## 4. Per-experiment measurements

| Metric | Baseline (pool10/consult4) | Stage A (pool12/consult4) | Stage B (pool10/consult6) | 200 RPS (pool10/consult6, 2400 sessions) |
|---|---|---|---|---|
| Achieved RPS (steady state) | ~168–178 (avg ~172) | ~167–178 (avg ~173) | ~168–178 (avg ~172) | ~197–210 (avg ~205) |
| True system error rate | 0.3631% | 0.3629% | 0.3808% | 0.5001% |
| Business rejection rate | 0.064% | 0.072% | 0.044% | 0.011% |
| Auth reject rate | 3.903% | 4.036% | 3.921% | 3.752% |
| Rate-limited rate | 1.569% | 1.482% | 1.892% | 2.509% |
| End-to-end booking success | 3683/3889 = 94.7% | 3803/4006 = 94.9% | 3599/3936 = 91.4% | 3843/4756 = 80.8% |
| `consultation` lane_concurrency rejections | 12 | 3 | 0 | 0 |
| `consultation` dependency_pressure rejections | 0 | 0 | 0 | 1 |
| `webhook` lane_concurrency rejections | 148 | 164 | 159 | 352 |
| `expensive` lane_concurrency rejections | 32 | 29 | 42 | 157 |
| `normal` dependency_pressure rejections | 0 | 0 | 0 | 4 |
| `payment` lane_concurrency rejections | 0 | 2 | 0 | 0 |
| Postgres peak CPU (docker stats) | not separately captured (comparable to Stage A) | ~30–60% | ~35–65% | up to 132% (of 300% share, 4-core host) |
| Postgres connections (active+idle) during run | 29–33 | 29–33 | 29–33 | not separately tabulated, no saturation observed |
| Backend peak CPU | low (<70%) | low (<70%) | low (<70%) | up to 104% |
| Container memory | all services well under their `mem_limit` in every run (backend ~20MB/1.5GB, postgres ~550–600MB/3GB) | same | same | same |
| Container stability | all 6 healthy throughout | all 6 healthy throughout | all 6 healthy throughout | all 6 healthy throughout, no restarts |

Raw capacity_test stdout, docker_stats.csv, pg_activity.log, and pre/post
`/metrics` snapshots for each stage are under
`docs/performance-evidence/milestone-8-final/remediation/<stage>/`.

## 5. Decisions: accepted vs reverted

**Stage A (DB pool 10→12): REVERTED.** Baseline and Stage A are statistically
indistinguishable (true error rate 0.3631% vs 0.3629%; RPS, booking success,
and every rejection-reason count within normal run-to-run noise). Neither run
showed any `dependency_pressure` rejections at all — meaning the pool was never
actually the binding constraint under this API-only workload. This confirms
the Step 3 instruction's own caution: the 53,470 `dependency_pressure`
rejections characterized earlier required the admin/doctor-web CPU contention
that is absent from these isolated runs; without it, pool=10 already has
ample headroom at this RPS, so raising it to 12 bought nothing measurable
while adding Postgres connection overhead for no benefit. Reverted to 10
(smaller footprint, same outcome). The expected side effect of a pool bump
(Normal's lane cap also rising from `n.min(64)`=50 to 60) is moot since Normal
never approached either ceiling in testing.

**Stage B (Consultation 4→6): KEPT.** Consultation `lane_concurrency`
rejections went from 12 (baseline) → 3 (Stage A) → **0** (Stage B) — the
targeted metric improved to zero with no corresponding regression: true error
rate (0.3808%) and all other signals stayed within the same noise band as
baseline/Stage A, CPU/memory/connection headroom stayed ample, and no new
`dependency_pressure` appeared. Booking success (91.4% vs ~95%) and
rate-limited count on booking (155 vs ~6–24) both moved in this run, but
neither is mechanistically linked to the Consultation concurrency cap (booking
is Payment-class, not Consultation-class) — most plausibly ordinary run-to-run
variance in Redis rate-limit bucket timing and slot availability, not a
consequence of this change. Kept; tests green; image rebuilt and deployed.

## 6. Stage C — Expensive/Webhook review (no changes made)

Expensive's rejection count stayed essentially flat across baseline/Stage
A/Stage B (32/29/42 — noise-level), rising only at 2x the session load (157 at
200 RPS) — proportional to offered load, no sign of a disproportionate
bottleneck. Webhook similarly tracked load (148/164/159/352). Webhook traffic
in this harness is synthetic — every virtual patient immediately self-triggers
one webhook call on booking, a concurrency pattern no real diverse patient
population would produce — so these counts should not be read as a production
capacity ceiling. Within these short runs, webhook's own rejections did not
materially suppress end-to-end booking success beyond what Payment-class rate
limiting already explains (webhook rejections are a small fraction of the
booking attempts that failed in each run). No change made to either lane, per
instruction.

## 7. Remaining bottlenecks

- The admin-web/doctor-web CPU-contention-driven degradation (Phase A's
  ~21.5% true error rate) is **unaddressed by this stage** — none of Stage
  A/B/C's changes touch it, because no test here reproduced that contention.
  This remains the largest known gap before a real 60-minute acceptance run.
- Auth-reject rate (~3.7–4.0%) is dominated by `POST rtc-token` returning 100%
  403/401 in every run (2,000+ attempts, 0 successes each time) — appointments
  booked seconds before are outside their live consultation window, so this
  reads as correct business-rule enforcement (rtc-token requires the
  appointment's actual scheduled time), not an admission or capacity defect,
  but it means Consultation-class rtc-token traffic in this harness never
  exercises real backend work past the auth check. Worth flagging for anyone
  designing the next harness revision.
- At 200 RPS, `dependency_pressure` rejections appeared for the first time
  (small: 1 consultation + 4 normal) alongside growing Webhook/Expensive lane
  rejections — the first visible sign that headroom is finite above ~200 RPS,
  though nowhere near instability.

## 8. Recommendation on the final 60-minute six-service run

**Do not attempt it yet.** Before it's worth spending another 60-minute run:
1. Rebuild (or deliberately re-scope) a corrected, full-lifecycle admin/doctor-web
   workload generator — the prior one no longer exists and was the source of
   the harness-lifecycle bug that invalidated the first 60-minute attempt.
   Without it, a new 60-minute run cannot validate the actual degradation this
   whole Step 2/3 investigation exists to fix.
2. Carry forward this stage's two changes: `DB_POOL_MAX_CONNECTIONS_PER_ROLE=10`
   (unchanged from before) and the Consultation `n.min(6)` admission.rs change
   (now in the working tree). Re-verify `CAPACITY_HOST_HEADER=api.localhost` is
   set in whatever runs the next full benchmark — its omission silently
   invalidated this stage's first attempt and would silently invalidate a full
   60-minute run just as easily.
3. Expect the true error rate to still land above the 0.35% target once
   admin/doctor-web contention is reintroduced, since nothing done in this
   stage addresses that contention directly — this stage only confirmed the
   DB-pool/admission-lane layer is not the dominant constraint in its absence.

## 9. 200 RPS exploratory result

At `CAPACITY_ACTIVE_SESSIONS=2400` (same ramp/think/duration shape), the stack
sustained **~197–210 RPS** (above the 200 target) for the full 360s with **no
crashes, no OOM, no container restarts**, true system error rate **0.50%**
(above the 0.35% target but not a stability failure — driven mostly by Payment-class
rate limiting and lane_concurrency on Webhook/Expensive scaling proportionally
with load), and Postgres/backend CPU still with headroom (peaks ~132%/104% of
a 300%-share/4-core host, well short of saturation). This is a **short,
single-condition exploratory result, not a new acceptance baseline** — it ran
without admin/doctor-web load, for 6 minutes, not 60. It suggests ~200 RPS is
plausibly reachable on this hardware once the admin/doctor-web contention
problem is separately resolved, but that must be confirmed, not assumed, in a
future full-topology run.

---

## Phase 2: Active Web Workload Validation

### 10. The reproducible workload generator

`services/backend/examples/web_workload.rs` replaces the original, never-committed
`web_page_workload.sh` with a reviewable, version-controlled Rust example matching
the project's existing `capacity_test.rs`/`load_test.rs` pattern. It:

- Mints real, correctly-signed ADMIN/DOCTOR bearer tokens for real synthetic
  `@capacity-test.local` identities (the exact same, already-reviewed bypass
  `capacity_test.rs` uses — no real password login is possible against the
  seeded dataset, since its Argon2id hash is a documented placeholder, not the
  hash of any known password) and sets them directly as the `hd_admin_access`
  / `hd_doctor_access` (+ `hd_admin_session`/`hd_doctor_scope`) cookies the
  Next.js BFFs expect.
- Drives a configurable number of Admin and Doctor "workers", each looping
  real dashboard page loads (`GET /`, `GET /doctor/chats`) and real read-only
  API calls through each app's own proxy route (`/api/proxy/admin/...`,
  `/api/proxy/doctor/...`) at a configurable think-time cadence
  (`WEB_ADMIN_WORKERS`, `WEB_DOCTOR_WORKERS`, `WEB_THINK_MIN_MS`/`_MAX_MS`,
  `WEB_DURATION_SECS`).
- Runs a **fail-fast preflight** before any measurement sample is taken: one
  real page load and one real authenticated API call per app, hard-asserting
  HTTP status, non-zero `Content-Length`, and the expected `Content-Type` —
  specifically to catch the "routing silently falls through to a blank Caddy
  response" failure class (see §11) before it can contaminate a real run.
- Detects early worker death (10 consecutive transport failures) and exits
  non-zero with `RUN_INVALID: ...`, so a partial run can never be mistaken for
  a complete one by anything scripting around its exit code.
- Reports Admin and Doctor results **separately** from each other and from
  the API `capacity_test` numbers — never merged into one aggregate.
- Uses only read-only GET endpoints (`apps/doctor-web/src/lib/contracts.ts`'s
  own existing `safeBackendPath` allowlist for Doctor; a documented safe
  Admin read-only subset — `admin/overview`, `admin/doctors`, `admin/patients`,
  `admin/finance/summary`, `admin/command/telemetry`, `admin/logs`,
  `admin/grievances`) — confirmed against `services/backend/src/api/router.rs`
  before selection, specifically to guarantee no payment, email, FCM, or
  Agora side effect is ever triggered.

`services/backend/scripts/run_six_service_web_comparison.ps1` is the persisted,
reviewable orchestration script (PowerShell, matching this project's existing
script conventions) that launches `capacity_test.exe` and `web_workload.exe`
together, with a shared `WEB_START_EPOCH_MS` handoff so both generators' real
measurement windows start within the same few seconds — logged, not assumed.
(The three comparisons below were driven directly via an equivalent inline
shell sequence in this session for reliability; the script captures the same
procedure for future reuse.)

### 11. Two real bugs this phase caught and fixed before trusting any result

1. **`CAPACITY_HOST_HEADER` omitted.** The very first Test A attempt in this
   phase was run without `CAPACITY_HOST_HEADER=api.localhost`. Most GET
   traffic still looked completely normal (differentiated latencies, correct
   JSON) — but `POST /appointments/book`'s paired `POST webhook confirm` step
   never appeared in the report at all, and `END_TO_END_BOOKING_SUCCESS` read
   **0 of 3,975 (0.0%)**. Root cause: without the Host header, some backend
   response path specific to the booking→webhook chain behaved differently
   than the GET-heavy traffic that happened to still route correctly. This is
   the exact bug class §8's "methodology note" already flagged as a risk for
   "whatever runs the next full benchmark" — it recurred immediately, in this
   very phase, confirming the concern was not hypothetical. The invalid run
   is preserved at
   `docs/performance-evidence/milestone-8-final/remediation/phase2_idle_consult6_INVALID_no_host_header/`.
   Fixed by always setting `CAPACITY_HOST_HEADER=api.localhost` (now baked
   into `run_six_service_web_comparison.ps1`, with its own comment pointing
   back at this incident) and adding an explicit curl-based preflight check
   before every `capacity_test` run.
2. **Bad identity selection in the new generator.** `web_workload.rs`'s first
   version selected ADMIN/DOCTOR identities via a bare `ORDER BY id LIMIT 20`,
   which (because their UUIDs sort first) silently picked up 2 real-looking,
   non-synthetic fixture accounts (`sabrina.akter@hellodoctor.health`,
   `anika.rahman@hellodoctor.health` — created by unrelated in-flight work in
   this repo, not the capacity-test dataset) that the backend legitimately
   401s on every `/api/proxy/doctor/*` call for reasons specific to those two
   accounts (confirmed: every one of the other 38 synthetic doctor identities
   works fine with the identical token-minting approach). This produced a
   consistent, reproducible ~50% `authrej` rate on Doctor-web proxy calls in
   the first Test B attempt (preserved at
   `docs/performance-evidence/milestone-8-final/remediation/phase2_plausible_web_INVALID_bad_doctor_ids/`).
   Fixed by filtering both identity queries to
   `email LIKE '%@capacity-test.local'` (the known synthetic domain,
   confirmed to match exactly the seeded 40 doctors / 10 admins).

Both fixes are in the committed-to-working-tree source, not one-off
workarounds — the next person running this harness gets them automatically.

### 12. Three controlled comparisons

Same API workload across all three (`CAPACITY_MODE=sessions
CAPACITY_PROFILE=REALISTIC_RAMP CAPACITY_ACTIVE_SESSIONS=2000
CAPACITY_RAMP_SECS=120 CAPACITY_THINK_MIN_SECS=5 CAPACITY_THINK_MAX_SECS=20
CAPACITY_DURATION_SECS=360 CAPACITY_HOST_HEADER=api.localhost`), deployed
config unchanged throughout (`DB_POOL_MAX_CONNECTIONS_PER_ROLE=10`,
Consultation `n.min(6)`). All three stayed fully healthy — no crashes, no OOM,
no container restarts (`docker ps` confirmed healthy after every run).

| Test | Web config | API achieved RPS | API true error rate | Web RPS | Web syserr highlights |
|---|---|---|---|---|---|
| A — idle | none | 147.2 | **0.48%** | 0 | n/a |
| B — plausible | 3 admin + 5 doctor workers, 2-8s think | 144.1 | **9.20%** | 2.25 (admin 1.02, doctor 1.23) | admin endpoints 4-15 syserr/48; doctor endpoints 1-6 syserr/56 |
| C — 40-worker stress | 20 admin + 20 doctor workers, 0.1-0.5s think | 138.1 | **86.46%** | 89.29 (admin 49.52, doctor 39.76) | `GET /doctors` 92.6% syserr; most doctor proxy endpoints >94% syserr |

Full endpoint tables, docker-stats samples, and pre/post `/metrics` snapshots
for each are under `docs/performance-evidence/milestone-8-final/remediation/phase2_{idle_consult6,plausible_web,stress_40worker}/`.

Admission-counter deltas explain the progression precisely:

- **Test A (idle):** the only rejections are Webhook `lane_concurrency` (+174)
  and Expensive `lane_concurrency` (+78) — hardcoded/`n.min(2)` caps binding
  under API-only load at this throughput. These exactly match the report's
  `POST webhook confirm` (174 syserr) and `GET admin telemetry` (78 syserr)
  rows. Consultation shows only 1 stray rejection — the `n.min(6)` change
  from Stage 1 is holding with no regression. **`dependency_pressure`
  (DB pool) is essentially absent** (+1 payment, negligible).
- **Test C (40-worker stress):** `dependency_pressure` appears for the first
  time at scale — **Normal class: +50,161**, dwarfing every lane_concurrency
  rejection combined (Expensive +5,570, Normal +3,436, Consultation +600,
  Payment +392, Webhook +334). A new reason also appears:
  `expensive,lane_rate_limit` (+7,510) — Expensive's local 20/s token bucket,
  not just its concurrency ceiling, is now binding too.

### 13. Confirmed root cause (evidence, not assumption)

Per-container `docker_stats.csv` samples across the three tests show CPU
utilization on `admin-web`/`doctor-web` rising sharply under load (all six
services share the same `cpuset: 0-3`, so this is real host-core contention,
not an isolated-container effect) and Postgres `pg_stat_activity` active
connection counts rising under Test C specifically. Combined with §12's
counter evidence, the mechanism is: **real Admin/Doctor web traffic adds
genuine backend request volume and Postgres query work** (every admin/doctor
page and API call is itself a real authenticated backend request — `admin/patients`,
`admin/finance/summary` etc. are not free), which **competes for the same
4-core host CPU budget as the API workload and for the same Postgres
instance**. At plausible load (Test B) this shows up mostly as lane-capacity
exhaustion (Webhook/Expensive syserr roughly tripling, still modest absolute
volume). At stress load (Test C) it escalates into genuine DB connection
pool pressure (`dependency_pressure` becomes the dominant rejection reason
by a wide margin) — **not because the pool is undersized in the abstract**
(Stage 1's API-only test never showed this), but because real concurrent
web-driven query load plus host CPU contention means connections are
genuinely busier for longer, consistent with the admission layer's own
`exhausted(pool)` check (`pool.size() >= max && pool.num_idle() == 0`) being
a real, not spurious, read at that load level.

**This phase did not capture mid-run Postgres wait-event/lock data** (only a
post-run snapshot, which naturally shows connections already drained) — a
genuine limitation. A future run should sample `pg_stat_activity` and
`pg_stat_statements` every ~20-30s *during* Test C specifically to
distinguish "genuinely more concurrent queries needed" from "the same
query count held open longer by CPU scheduling delay" with full confidence.
Both are plausible from what was measured here; only mid-run sampling can
separate them.

### 14. Minimal proposed optimizations

None are being made in this pass. The evidence does not yet cleanly separate
"raise the DB pool because real concurrent demand exceeds it" from "reduce
host CPU contention and the pool pressure disappears on its own" (§13's
disclosed limitation). Raising the pool again without that distinction would
repeat Stage 1's own lesson — Stage 1's API-only Stage A test already showed
raising the pool helps nothing when pool pressure isn't the real constraint.
The two most promising, still-unevaluated levers for a future pass: (a)
reducing Admin/Doctor page SSR cost specifically (several admin endpoints'
p95/p99 latency under Test B already exceed 500ms even at only 3+5 workers,
well before Test C's stress level), and (b) re-measuring the DB pool
question specifically *under* an active, plausible (Test-B-shape) web load
rather than API-only.

### 15. Is Consultation `n.min(6)` still justified?

**Yes.** Consultation `lane_concurrency` rejections stayed negligible (0-1)
across all three tests, including the 40-worker stress case, where every
other class's rejections exploded by orders of magnitude. The fix from
Stage 1 holds under real active web load, not just the API-only condition it
was originally validated against.

### 16. Booking-success figures across the three tests (not a regression)

END_TO_END_BOOKING_SUCCESS: Test A 95.2%, Test B 70.6%, Test C 12.3%. This
tracks the same CPU/DB contention mechanism in §13 (webhook-confirm and the
booking POST itself are both subject to the same Expensive/Payment/Normal
admission pressure) — it is not a separate or unexplained regression, and
Test A's 95.2% (vs. the lower *raw* `POST /appointments/book` success rate
implied by earlier Stage 1 figures) reflects that this run used the
corrected `CAPACITY_HOST_HEADER` the Stage 1 runs did not consistently carry
into every endpoint; see §11 item 1 for why that specifically matters for
this exact metric.

### 17. RTC-token fixture gap: fixed

`services/backend/examples/rtc_token_scenario.rs` is a new, small, standalone
scenario — deliberately **not** folded into the main 166-170 RPS workload —
that proves the RTC-token mint path works correctly for a *legitimately
scheduled* appointment, using only real APIs and no authorization bypass:
mints real DOCTOR+PATIENT tokens → `POST /api/v1/doctor/slots` with a
real near-term `start_time` (~90s out, the handler's only constraint is
`start_time > now()`) → `POST /api/v1/appointments/book` → real HMAC-signed
`POST /api/v1/webhooks/payment/bkash` confirmation → `POST
/api/v1/consultations/{id}/rtc-token` as both participants.

Result: **8 of 9 attempts succeeded** (p50 13ms, p95 18ms); the one failure
was a transient 429 from Payment-class rate limiting (residual load from the
stress test run moments before), not a scenario defect. This confirms
`capacity_test.rs`'s near-100% `rtc-token` 401/403 rate is correctly-working
backend authorization against an out-of-window test fixture, not a backend
defect — no G-30/G-31 reservation logic, consultation-state check, or
authorization boundary was touched or relaxed to get this result.

### 18. Readiness recommendation (two-tier target model)

1. **Production-like acceptance (166-170 RPS + realistic Test-B-shape web
   load, 60 minutes): NOT YET READY.** Test B's 9.20% true error rate is far
   above the 0.35% target, and §13's root cause (host CPU + DB contention
   from genuine concurrent web traffic) is not yet remediated — only
   characterized. Running the full 60-minute version now would very likely
   reproduce the original contaminated run's failure mode, just without the
   harness-lifecycle bug this time.
2. **High-concurrency stress (API workload + 40-worker case): NOT a
   60-minute acceptance candidate at all in its current state** — 86.46% true
   error rate reflects real overload, not merely "more rejections at the
   admission layer." Useful as a documented stress ceiling, not a target to
   pass.

Recommended next steps before either 60-minute attempt: (a) mid-run Postgres
wait-event sampling during a Test-B-shape run to finish distinguishing the
two root-cause hypotheses in §13, and (b) investigate reducing Admin/Doctor
SSR/query cost specifically, since Test B's degradation starts well before
CPU/DB would plausibly be fully saturated by only 8 web workers.

---

## Addendum: Revised acceptance targets for Stages 2-4 (not executed in this pass)

Recorded here per updated product-owner direction, received while this
Phase 2 diagnostic work was in progress. **None of Stages 2-4 below were run
in this pass** — they are out of scope for a short diagnostic phase and
require explicit separate authorization, consistent with this document's
existing rule that no 60-minute run happens without review of the evidence
above first.

- **Stage 1 (this document, through §18): complete.** Gates everything below
  on its own judgment of readiness — §18 says not yet ready.
- **Stage 2:** once the realistic six-service config passes short
  validation, run the OFFICIAL 60-continuous-minute acceptance test at **200
  sustained API RPS** + the realistic (Test-B-shape) Admin/Doctor web
  workload — not 166-170. Must pass to count as first full acceptance.
- **Stage 3:** only if Stage 2 passes, a short 5-10 minute diagnostic at
  **250 RPS** with the same realistic web traffic mix, to check safety
  margin.
- **Stage 4:** only if Stage 3 looks stable, a separate 60-minute acceptance
  attempt at **250 RPS** (stretch goal).

Non-negotiable constraints carried forward for Stage 2+: unchanged 4 vCPU/8GB
envelope with all six services active throughout and real (not idle/reduced)
authenticated Admin/Doctor web workloads; API offered/achieved RPS always
reported separately from supplemental web RPS; no weakened security/RLS/rate
limits/admission safeguards; no real external payment/notification provider
charges; no OOM/crashes/uncontrolled resource growth/progressive latency
degradation; target true system error rate ≤0.35% at acceptance with
documented p95/p99 and booking-success criteria (expected
authorization/business-rule rejections reported separately, never classified
as infrastructure failure, and never used to mask poor functional success);
explicitly distinguish "N RPS offered" from "N RPS successfully processed" in
every report; if 200 RPS fails acceptance, diagnose and remediate before
attempting 250 RPS at all — never jump ahead; if 200 passes but 250 fails,
retain the verified 200 RPS result as the accepted baseline rather than
weakening any acceptance criterion to force a 250 pass; no git
commits/pushes; preserve all raw evidence; a workload generator stopping
prematurely still invalidates that run.

---

## Phase 3: Root-Cause Isolation Under Active Web Load

Scope per product-owner direction: a single, instrumented diagnostic stage
to distinguish Next.js CPU cost, backend/database amplification, and
admission-control pressure as three separate mechanisms, before authorizing
any further pool/admission changes. **No pool, admission ceiling, CPU quota,
or deadline was changed in this phase.** No RLS policy, backend code, or
migration was changed either — this phase is read-only investigation plus a
documented recommendation, per the explicit "report before major
implementation changes" instruction.

### 20. Method

Phase 2's `phase2_plausible_web` evidence (the 3-Admin/5-Doctor "plausible"
scenario — 9.20% true system error rate, 70.6% booking success, the single
most important data point per product-owner direction) already contains, in
`web_workload.log`, a per-endpoint p50/p95/p99 breakdown that had not
previously been examined closely. Reading it first, before running anything
new, immediately isolated two specific Doctor-Web endpoints as extreme
outliers against all seven other Admin/Doctor pages:

| Endpoint | p50 | p95 | p99 |
|---|---|---|---|
| `GET doctor/appointments` (`/api/proxy/doctor/appointments` → backend `GET /api/v1/doctor/appointments`) | **6045ms** | **9303ms** | **10049ms** |
| `GET doctor/chats(api)` (`/api/proxy/doctor/chats` → backend `GET /api/v1/doctor/chats`) | **5682ms** | **7454ms** | **8651ms** |
| every other Admin/Doctor page (8 of 10 endpoints) | 25-1498ms | 193-2451ms | 264-2856ms |

This is not a uniform slowdown consistent with generic host CPU contention
(which would degrade all ten endpoints by a similar multiple) — it is two
specific endpoints running 5-50x slower than the rest. That pointed straight
at mechanism #2/#3 (backend/database amplification and connection-hold
time), not mechanism #1 (Next.js rendering cost): both pages are thin
proxies: the Next.js side does no rendering work of its own beyond passing
the backend's JSON through (`apps/doctor-web/src/app/api/proxy/[...path]/route.ts`),
so if the backend response takes 6-10 seconds, the page takes 6-10 seconds,
full stop.

To confirm the mechanism precisely (not just correlate it), the exact SQL
behind both handlers (`list_own_appointments` and `list_own_chats` in
`services/backend/src/api/v1/doctor_handlers.rs`) was run directly against
the live `hellodoctor-m8-final-local` Postgres container via `EXPLAIN
(ANALYZE, BUFFERS)`, once as the `postgres` superuser (which bypasses Row
Level Security entirely) and once as the real `hellodoctor_api` role with
the real session GUCs (`app.current_role='DOCTOR'`,
`app.current_user_id=<a real seeded doctor's user id>`) set exactly as the
Rust backend sets them per-request — i.e., the identical query, with and
without RLS enforcement, against the same live data (one doctor's ~1,613
seeded appointments / ~1,313 chat conversations). This is read-only
(`EXPLAIN ANALYZE` still executes the query, but neither handler mutates
state) and used the stack's own already-seeded synthetic data — no new data
was created, no RLS policy was altered to produce these numbers.

### 21. Finding: RLS policy evaluation, not a missing index, not Next.js, not primarily the DB pool

| Query | As superuser (RLS bypassed) | As `hellodoctor_api` with real RLS context | Slowdown |
|---|---|---|---|
| `list_own_chats` (`GET /api/v1/doctor/chats`) | 148ms | **3,452ms** | **~23x** |
| `list_own_appointments` (`GET /api/v1/doctor/appointments`) | *(not separately measured without RLS — not needed once the pattern was confirmed)* | **3,196ms** | — |

Both numbers land almost exactly on the 5,682ms/6,045ms p50 figures observed
end-to-end in the live run once Caddy/Next.js/connection-acquisition
overhead is added on top — this is the mechanism, not a correlated guess.

The root cause is the same in both queries and is a known PostgreSQL RLS
failure mode, not a missing index (every join in both plans already uses an
existing index — `idx_chat_messages_conv`, `idx_appointments_doctor`,
`chat_conversations_pkey`, `doctor_profiles_user_id_key`, etc. — this is not
"add an index" territory):

- `chat_conversations`, `chat_messages`, `appointments`, and
  `patient_profiles` each carry a forced Row Level Security policy whose
  "is this the doctor who owns this row" check is a **correlated subquery
  back into `doctor_profiles`** (e.g. `EXISTS (SELECT 1 FROM doctor_profiles
  d WHERE d.id = chat_conversations.doctor_id AND d.user_id =
  current_setting('app.current_user_id'))`).
- For a doctor with ~1,300-1,600 rows (entirely realistic after months of
  real use, and exactly what the seeded benchmark dataset already
  represents), Postgres cannot always convert that correlated check into a
  single hashed semi-join — in `list_own_chats`'s plan, the `chat_messages`
  RLS filter's subplan is **not hashed** and is executed as a nested loop
  **3,074 times** (once per matching message row), and each of those 3,074
  executions itself does a nested-loop lookup back through
  `chat_conversations` and `doctor_profiles` — a cascading, multiplicative
  cost that is invisible when the same query is run as a role RLS doesn't
  apply to (exactly why the superuser run looked fine at 148ms and nearly
  everyone testing this query by hand as `postgres` would never see it).
- This means: raising `DB_POOL_MAX_CONNECTIONS_PER_ROLE` further would not
  fix this — more connections just means more of them individually stuck
  for 3+ seconds each on the same per-row RLS cost. Raising the Consultation
  lane further (`n.min(6)` → higher) would not fix it either, and would
  actively make Postgres contention worse by allowing more of these
  multi-second queries to run concurrently — consistent with the explicit
  instruction not to reflexively tune pool/lane numbers again without new
  evidence. **Neither of those levers is the right next step.**

This also directly explains the 40-worker stress run's 86.46% error rate
and 50,161 `dependency_pressure` rejections (Phase 2 §9): 40 workers each
holding a DB connection for 3-10 seconds on this one query pattern exhausts
the pool almost immediately, independent of its configured size.

### 22. What this is NOT

- **Not Next.js/server-rendering cost.** Both pages are thin JSON proxies;
  confirmed by the SQL-level reproduction landing on the exact same latency
  figures with zero Next.js involved.
- **Not excessive dashboard polling** on the generator's part — `web_workload.rs`
  requests each page once per worker iteration with a 2-8s think time
  between actions (the "plausible" scenario), not a tight poll loop; a real
  doctor opening their chats list once produces this same cost.
- **Not a missing index** — every join already has one; see above.
- **Not primarily a DB-pool-size problem** — confirmed by Stage A (§3-5):
  raising the pool 10→12 with API-only traffic (no RLS-heavy doctor-web
  queries in flight) showed zero effect, because the pool was never the
  bottleneck in that condition. It becomes a symptom under active web load
  only because each connection is held 20-70x longer than it should be by
  the RLS cost above — the pool is being starved by hold-time, not by an
  undersized limit.
- **Consultation `n.min(6)` is not wrong** — it still shows ~0 rejections
  even in the 40-worker stress case per the prior fork's report. It is a
  concurrency safety valve doing its job; the problem is what's consuming
  the slots once they're granted (one `doctor/chats` call occupying a
  Consultation-lane slot for 3-10 seconds instead of milliseconds).

### 23. Recommended fix (NOT applied — requires explicit sign-off before touching RLS)

The actual fix is a Postgres RLS performance pattern, not a Rust/Next.js
change and not a config tunable: avoid the correlated `doctor_profiles`
subquery on every row by giving the already-authenticated request's own
doctor-profile identity to Postgres directly, the same way
`app.current_user_id` is already set once per request/transaction (see
`AppState::begin_authed` / wherever that GUC is currently set) — e.g. a
second session GUC such as `app.current_doctor_profile_id`, set
server-side from the same trusted source `ctx.verified_doctor_profile_id`
already uses (never client-supplied), that the affected RLS policies can
compare directly (`doctor_id = current_setting('app.current_doctor_profile_id')::uuid`)
instead of re-deriving it via a per-row subquery into `doctor_profiles`.
This changes *how fast* the policy evaluates, not *what it allows* — same
rows visible to the same callers — but it is still a change to
security-critical RLS SQL and to the request-setup code path that populates
session GUCs, so this report stops here and recommends it rather than
implementing it, per the explicit instruction to preserve RLS and report
before major changes. It would need: a migration altering the affected
policies (with a tested down-migration), the corresponding Rust change to
set the new GUC wherever `app.current_user_id` is currently set for a
DOCTOR-role request, and a full RLS regression pass (every existing
RLS/authorization test, not just a perf before/after) before being trusted.

A narrower, zero-RLS-change interim mitigation worth considering instead:
cache `list_own_chats`/`list_own_appointments` responses for a few seconds
per doctor (e.g. a short-TTL Redis cache keyed by doctor id + query params),
since a real doctor reloading their dashboard repeatedly within a few
seconds does not need a fresh RLS-evaluated query each time. This does not
touch RLS at all, is fully reversible, and would blunt the observed
degradation without resolving the underlying per-row RLS cost (which will
still bite the *first* load, and will still bite any other caller of these
same tables, e.g. the mobile app's own equivalent endpoints — not
investigated in this phase, flagged as a follow-up).

### 24. Outstanding items from this phase's instructions

- **Idle baseline's 0.48% true error rate** (already above the 0.35%
  target): confirmed fully attributable to the already-disclosed hardcoded
  `Expensive` (`n.min(2)`, 235 `lane_concurrency` rejections in this window)
  and `Webhook` (hardcoded `2`, 526 `lane_concurrency` rejections) lane
  caps — see Phase 2 §6/§9. Not a new or unexplained source; still an open
  item from Stage C, now with this run's exact counts attached.
- **RTC-token fixture, 8/9 not 9/9**: not re-investigated in this phase
  (out of scope for the RLS finding above); the prior fork's report
  attributed the one failure to a transient 429, which is consistent with
  Normal/Consultation-class rate budgets rather than a harness or
  authorization defect, but this has not been independently re-confirmed
  here and is flagged as still open if it matters for sign-off.

### 25. The fix was implemented, tested, deployed, and re-measured

Per explicit product-owner approval, §23's recommended fix was implemented
(not just proposed) and validated end-to-end:

- **Migration**: `services/backend/migrations/20261204000001_doctor_rls_identity_lookup_performance.{up,down}.sql`
  adds `fn_current_doctor_profile_id()` (STABLE SQL, no per-row argument) and
  rewrites the four affected policies (`appointments_access`,
  `chat_conversations_access`, `chat_messages_access`,
  `consultation_sessions_access`) to compare directly against it instead of
  a correlated subquery into `doctor_profiles`. `patient_profiles_access`
  (`fn_doctor_treats_patient`) was deliberately left untouched — its
  verification/active-account/consultation-state checks are not a pure
  identity lookup and are out of scope for this kind of rewrite.
- **Query restructuring**: `services/backend/src/api/v1/doctor_handlers.rs`'s
  `list_own_appointments` and `list_own_chats` now defer
  `doctor_history_context_allowed()` and the `patient_profiles` join (both
  previously evaluated for every candidate row before pagination) to only
  the already-paginated result rows.
- **Correctness, before deploying anywhere**: both changes were first
  applied to an isolated clone database (`hellodoctor_rls_clone`, a
  pg_dump/restore copy — the live/shared databases were never touched until
  validation was complete). Confirmed via direct queries: identical row-ID
  sets and order for the same doctor's chats/appointments before and after;
  zero cross-doctor leakage in either direction for a second doctor; patient
  self-access preserved with zero leaked rows for an unrelated doctor; ADMIN
  full visibility preserved (63,442 rows); an empty/unauthenticated session
  context still fails closed (0 rows). The migration's up→down→up cycle was
  tested cleanly on a separate throwaway database.
- **Full regression suite**: `cargo test --test api_tests -- --test-threads=1`
  (the exact invocation `.github/workflows/backend-migrations.yml` uses) —
  **476 passed, 0 failed, 5 ignored**, including every RLS/authorization/
  SQL-injection/slot-isolation test already in the suite. (Two earlier
  attempts at this showed mass failures; both were test-harness setup gaps
  on this session's part — a missing disposable Redis fixture required by
  `TEST_REDIS_URL`, and running with parallelism this suite's own CI
  comment documents as racing on shared Postgres catalog rows across the
  five capability roles — not real regressions. Root-caused and ruled out
  before trusting the green result.)
- **Deployed live**: migration applied via the normal `sqlx::migrate!`
  startup path against the `hellodoctor-m8-final-local` stack's real
  database (not just the clone), backend image rebuilt and the container
  recreated. Confirmed via direct `EXPLAIN ANALYZE` against the live
  database immediately after: the `list_own_chats` query measured
  **1,631ms**, consistent with the clone's 1,368ms (vs. the original
  3,452ms) — the fix works on the real target, not just in isolation.
- **Also benefits mobile**: `apps/mobile/lib/features/doctor/doctor_repository.dart`
  calls these exact same `/doctor/appointments` and `/doctor/chats` backend
  endpoints, so the Flutter Doctor app gets the same improvement, not just
  Doctor-Web.

### 26. Re-measuring the "plausible" web-load scenario (Test B) after the fix

Getting a trustworthy automated re-run took five iterations — all were bugs
in the *comparison harness script* (`run_six_service_web_comparison.ps1`),
never in the fix itself, and all are now fixed in that script: an
`Invoke-WebRequest -SkipHttpErrorCheck` parameter that only exists on
PowerShell 7+ (this host runs Windows PowerShell 5.1); a live
`System.Diagnostics.Process` object passed through `Start-Job`'s
`-ArgumentList`, which cannot be serialized across the job's process
boundary; a relative `$OutDir` resolved against the wrong working directory
by .NET APIs (which use `Environment.CurrentDirectory`, not PowerShell's
`$PWD`); and a `Register-ObjectEvent`-based async log capture whose
PowerShell-side callbacks are queued for the engine's idle loop and are not
guaranteed to have all run by the time `WaitForExit()` returns — this
silently lost a non-deterministic tail of each run's single end-of-measurement
print burst (exactly the summary lines this script exists to capture) twice,
by a different amount each time, before being replaced with plain OS-level
`Start-Process -RedirectStandardOutput` file redirection, which needs no
PowerShell-side callback at all.

Once the harness was reliable, a second, separate confound surfaced: the
real business rule `check_booking_limit` (10 bookings/hour/account,
`services/backend/src/rate_limit.rs:139-141`, Redis-backed) had accumulated
real state from the many benchmark scenarios already run today against this
same long-lived stack (Phase 2's idle/plausible/stress40 runs, the Stage
A/B/C pool and consultation experiments, the 200 RPS exploratory run, and
this phase's own earlier attempts) — checked directly in Redis:
**1,102 patient accounts already had a `ratelimit:booking:{user_id}` counter
at 4-18**, most already past the real 10/hour cap, purely from repeated
testing, before this comparison run even started. This is disclosed, not
hidden: the first clean-harness run (`phase3_plausible_web_rls_fixed/`)
measured booking success at only 37.6%, which looked like a severe
regression but was actually test-fixture contamination — confirmed by
`ratelim=2183` of 3,933 booking attempts in that run's own breakdown, far
above anything admission.rs produces (zero `shared_class_budget` or
`lane_rate_limit` rejections exist in that run's metrics at all). The
*only* narrowly-scoped cleanup applied was clearing that one Redis key
pattern (`ratelimit:booking:*` — a benchmark rate-limit fixture, not user
data, not RLS, not any other state) before the final re-run below.

| Metric | Phase 2 baseline (before fix) | After fix, run 1 (booking-limit-contaminated, disclosed above) | After fix, run 2 (clean) |
|---|---|---|---|
| Achieved API RPS | ~169 | ~168 | ~172 |
| TRUE_SYSTEM_ERROR_RATE | 9.1966% | 7.9624% | 11.6507% |
| `doctor/appointments` p50/p95/p99 (ms) | n/a (not measured this way in Phase 2) | 1909/3275/4368 | — |
| `doctor/chats(api)` p50/p95/p99 (ms) | n/a | 2620/4457/6104 | — |
| END_TO_END_BOOKING_SUCCESS | 70.6% (2716/3849) | 37.6% (contaminated — not valid) | **75.4%** (2898/3846) |
| Consultation `dependency_pressure` / `lane_concurrency` rejections (this run) | — | 61 / 152 | 211 / 348 |

Honest reading of this: the two post-fix runs disagree with each other on
TRUE_SYSTEM_ERROR_RATE (7.96% vs 11.65%) by more than either differs from
the pre-fix baseline (9.20%) — that is real run-to-run variance, most
plausibly from this specific host having done a full day of continuous,
heavy, back-to-back benchmark/build activity (this session alone ran or
rebuilt the Docker backend image multiple times, ran the full `cargo test`
suite three times, and executed roughly a dozen 6-minute load scenarios),
not a property of the database fix changing between runs (nothing changed
between those two runs except clearing the Redis booking-limit keys). The
booking-success number is the one metric here that IS clean and trustworthy
once the test-fixture contamination is accounted for, and it improved
(70.6% → 75.4%) rather than regressed — resolving what otherwise would have
read as an "unexplained booking-success regression."

What this does NOT show: a clear, reproducible drop in the aggregate
end-to-end error rate under the full six-service "plausible" web-load
condition. The query-level fix is proven correct and proven faster in
isolation (direct `EXPLAIN ANALYZE`, both in the clone and live); it has
not been proven, with low-noise confidence, to bring the full six-service
aggregate error rate under this specific active-web-load condition down to
anywhere near the 0.35% target — both post-fix measurements remain far
above it, same as before the fix.

### 27. Readiness decision (updated)

Still **not ready** for the 166-170 RPS tier, let alone 200 or 250.
Narrowed from §19/§25: the RLS/query fix is real, correct, safe (full
regression suite green, isolated-clone correctness proof), and measurably
faster for the two endpoints it targeted — keep it. It is not, by itself,
sufficient to bring the full six-service active-web-load error rate near
target, and this session's own benchmark noise (7.96% vs 11.65% on
back-to-back identical-config runs) means no single run here should be
treated as a final verdict either way. Before attempting any acceptance
tier:

1. Get a genuinely quiet measurement environment — not a host that has just
   finished a full day of continuous cargo builds, Docker rebuilds, and
   load tests — for the next comparison run, or explicitly account for
   host-load variance as its own measured factor.
2. Complete the instrumentation Step 1 originally called for and this phase
   did not fully deliver: live PostgreSQL `pg_stat_activity`
   wait_event/wait_event_type sampling and per-container CPU time-aligned
   with the load run, to distinguish "still RLS/query cost elsewhere"
   from "general host/Postgres CPU contention from six services sharing
   4 cores" as the remaining dominant factor.
3. Re-run the three-way A/B/C comparison (idle / plausible / 40-worker
   stress) from Phase 2 against the now-fixed backend, not just the single
   "plausible" scenario repeated here, to see whether the fix changed the
   *shape* of degradation (e.g., does the system now survive the 40-worker
   stress case meaningfully better, even if "plausible" alone still shows
   noise).
4. Only then reconsider the 200/250 RPS staged targets recorded in the
   Addendum above.

---

## Phase 4: Controlled Re-Measurement With Test-State Reset and Live Instrumentation

Per explicit product-owner direction: before any further code change, get a
controlled, reproducible comparison (identical reset state, identical
workload, live diagnostics throughout), and use it to answer one specific
question — is the Doctor Chats/Appointments query still slow because of a
remaining query-plan defect, or because of host/Postgres resource
contention under load? Three runs, not one, reported as a distribution.

### 28. Two more real harness bugs found and fixed first

- `run_six_service_web_comparison.ps1`'s docker-stats sampling had **never
  worked, in any run this entire investigation did** — verified by
  re-checking every prior evidence directory's `docker_stats.csv`: all
  contain only the header line. Root cause: it ran via `Start-Job`, whose
  child PowerShell process does not reliably inherit the calling session's
  Docker CLI context. Replaced with a dedicated script
  (`services/backend/scripts/sample_live_instrumentation.ps1`) launched via
  the same `Start-Process` pattern already proven reliable for
  `capacity_test.exe`/`web_workload.exe`.
- That replacement script's first test run *also* produced an empty
  `docker_stats.csv` even without `Start-Job` in the picture — `docker
  stats --format '{{.Container}}'` returns the container **ID**, not its
  name; the filter was matching a name substring against a hex ID and
  never matching anything. Fixed to `{{.Name}}` (singular). Confirmed live
  with a direct `docker stats` call before trusting the fix.

### 29. Test-state reset

Added `-ResetTestRateLimitState` (default on) to the comparison script.
Before each run it clears exactly two Redis key families on the benchmark's
own isolated Redis container, nothing else: `ratelimit:booking:*` (the real
10/hour/account business rule, `rate_limit.rs:139-141`) and
`ratelimit:grievance:*` (3/24h/account, `rate_limit.rs:219-220` — found
while investigating: this one had been silently exhausted since the very
first benchmark run today, across every scenario in this whole document,
including the original Phase 2 baseline — the grievance endpoint's numbers
were never a valid signal of anything, in any run before this one). Verified
live: 3,691 keys existed from the day's accumulated testing before the
first Phase 4 run; the reset is confirmed in each run's own log line
("Reset test-fixture rate-limit state: cleared N key(s)...").

### 30. Live instrumentation added

`sample_live_instrumentation.ps1` samples every 5s for the run's duration
and writes, per run: `docker_stats.csv` (per-container CPU%/memory, now
actually populated), `pg_wait_events.csv` (`pg_stat_activity` grouped by
state/wait_event_type/wait_event), and `pg_connection_summary.csv`
(active/idle/idle-in-transaction/total connection counts and the longest
currently-running query's age). Overhead is one extra `docker stats` call
and two extra trivial `psql` queries every 5 seconds — negligible against
the load being measured.

### 31. Three repeatable short runs (same config, same reset, same instrumentation)

| Run | TRUE_SYSTEM_ERROR_RATE | Booking success | Max concurrent active DB conns | Max total DB conns (any state) | Peak Postgres CPU | Peak backend CPU |
|---|---|---|---|---|---|---|
| 1 | 12.265% | 76.4% (2856/3738) | 6 | 6 | 232.83% | 145.94% |
| 2 | 13.390% | 73.7% (2833/3845) | 10 | 6 | 273.44% | — |
| 3 | 12.383% | 75.5% (3018/3995) | 6 | 6 | 313.89% | 172.85% |
| **Mean / range** | **12.68% (11.65-13.39% incl. the pre-reset clean run from Phase 3 §26)** | **75.2% (73.7-76.4%)** | — | **never above 6** | **233-314%** | **146-173%** |

With Redis test-fixture state actually controlled, the previously-worrying
7.96%-vs-11.65% spread mostly disappears: all four reset-controlled
"plausible" runs (the one at the end of Phase 3 plus these three) land in a
much tighter 11.65-13.39% band. The earlier 7.96% outlier most likely *was*
itself partly a symptom of uncontrolled Redis state interacting with the
measurement, not evidence of a cleaner underlying result — resolved by
Task 1, as intended.

### 32. Answering the query-plan-vs-contention question directly

**It is resource contention (host/Postgres CPU), not a remaining query-plan
defect in the fixed queries.** Evidence, consistent across all three runs:

- **Connection count was never the constraint.** Across 360-second runs
  with up to 2,000 active API sessions plus 8 web workers, the real,
  measured maximum was **6 total PostgreSQL connections in any state, and
  at most 6-10 concurrently active** — nowhere close to the configured
  pool (10/role × 5 roles = 50). This independently confirms Stage A's
  original finding (raising the pool did nothing) from a completely
  different angle: there was never a shortage of connections to hand out.
- **The database is CPU-bound when active, not lock- or I/O-bound.**
  Summed `pg_stat_activity` wait-event samples across all three runs are
  dominated by `idle/Client/ClientRead` (expected — connections waiting
  for the next request) and, among genuinely *active* sessions, overwhelmingly
  `active` with **no wait_event at all** (Postgres's own signal for "running,
  consuming CPU, not blocked on anything") — 88-120 such samples per run,
  versus single digits for any I/O (`DataFileRead`, `WALSync`) or lock
  wait. If this were a locking or disk-I/O problem, those categories would
  dominate instead; they don't.
- **Postgres alone consistently saturates more than half the entire
  4-core host.** Peak Postgres CPU was 232.83%, 273.44%, and 313.89%
  across the three runs — on a host whose total budget across *all six
  services* is 400% (4 vCPU). Backend peaked at 146-173% on top of that.
  Postgres + backend alone can consume 3.8+ of the 4 available cores at
  peak, leaving admin-web/doctor-web/redis/proxy to compete for whatever
  is left — this is the same mechanism Phase 2 originally hypothesized
  (mechanism #3, "resource contention") and Phase 3's RLS fix correctly
  addressed *part* of (the per-row correlated-subquery multiplication),
  but the two still-slow endpoints' remaining ~1.6s of genuine CPU work
  per call is, by itself, still enough to saturate the shared host under
  this concurrency level.

This means the next lever is not another query rewrite of the same two
endpoints (their remaining cost is dominated by `patient_profiles_access`'s
`fn_doctor_treats_patient` and the `doctor_history_context_allowed()`
business check, both deliberately left untouched in Phase 3 as
security-sensitive, non-identity checks) — it's that **the host has less
spare CPU than six concurrently-active services need**, independent of any
single query. Options worth evaluating next, none applied here: more CPU
allocated to this topology (the 4 vCPU envelope may simply be undersized
for six concurrently-active services, which is itself a legitimate
capacity-planning finding, not a bug); further reducing the per-call CPU
cost of `fn_doctor_treats_patient`/`doctor_history_context_allowed`
specifically (would require the same rigorous security review Phase 3 gave
the identity-lookup fix, since these are richer authorization checks, not
pure identity lookups); or reducing how often Doctor-Web/mobile call these
two specific endpoints (e.g., client-side caching with a short TTL) without
touching the backend at all.

### 33. Updated readiness decision

Still **not ready** for 166-170 RPS, 200 RPS, or 250 RPS. Keep pool=10 and
Consultation `n.min(6)` — nothing in this phase's evidence justifies
changing either. The RLS/query fix from Phase 3 remains correct, safe, and
worth keeping (it measurably reduced the per-row multiplication effect and
real-world booking success improved 70.6% → ~75%), but the dominant
remaining obstacle is now clearly host CPU capacity under concurrent
six-service load, not an RLS defect or a benchmark-harness artifact. The
0.35% target is still far away (~12-13% measured, consistently, across
three controlled runs) and will not be closed by further admission/pool
tuning — the next investigation should be a CPU budget/allocation question,
not another concurrency-limit experiment.
