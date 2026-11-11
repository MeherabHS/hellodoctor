# Milestone 8 — Memory Profile (Workstream 1)

Status: **Pass 2 — all isolated profiles that could be safely and
correctly measured are now complete; clinical file upload/download remains
genuinely unmeasured (no test coverage exists for it anywhere in the repo —
see §7), and a true multi-hour sustained window is Workstream 2's job, not
repeated here. See §8 for the explicit per-profile scorecard.**

All data below is measured, not estimated. Raw evidence:
`docs/performance-evidence/milestone-8-final/configA/` (`memory_profile_ramp.csv`,
`realistic_ramp_run.log`).

## 0. Pre-measurement corrections (required before any number below could be trusted)

1. **Configuration B (capacity stack) was running a stale compiled binary.**
   `sqlx::migrate!` embeds migrations into the binary at *compile* time, not
   read from disk at process start (see `src/migration_status.rs`). Config
   A's backend had been rebuilt today after the two newest migrations
   (`maintenance_job_volatile_timestamp_scans`, `t15_reminder_scan_indexes`)
   were added (93/93 applied), but Config B's backend image predated them
   (91/93 — confirmed via `/healthz`: `binary_latest_migration` stuck at
   `20261202000003`). A bare container restart does **not** fix this (the
   embedded migrator is fixed at compile time); the image had to be rebuilt.
   Rebuilt via `docker compose -f docker-compose.capacity.yml build backend`
   and recreated. Post-fix, both configs report identical
   `binary_latest_migration` = `database_latest_migration` = `20261202000005`,
   `migrations: "current"`. This was a genuine, confirmed discrepancy — the
   rebuild was justified, not routine, and no further rebuild is planned
   unless source/deps/migrations change again.
2. **No other rebuilds were performed.** Config A was not touched (no
   discrepancy found in it).

## 1. Configuration A resource-budget verification

Verified live against the running containers (`docker inspect`, in-container
`/sys/fs/cgroup/*`), not just the compose file text:

| Container | Live `cpuset.cpus.effective` | Live `memory.max` | Compose declares |
|---|---|---|---|
| `confa-backend` | `0-3` | 7,945,687,040 B (7.40 GiB) | `cpuset: 0-3`, `mem_limit: 7.4g` |
| `confa-redis` | `0-3` | 524,288,000 B (500 MiB) | `cpuset: 0-3`, `mem_limit: 500m` |
| `confa-postgres` | `4-5` | 4,294,967,296 B (exactly 4 GiB) | `cpuset: 4-5`, `mem_limit: 4g` |

**Application tier total: 4 vCPU (cores 0-3) / 7.9 GiB (backend 7.4 + redis
0.5), 0.1 GiB short of 8 GiB — the compose file's own comment reserves that
0.1 GiB headroom for Docker/OS inside the modeled VM; this is a documented
modeling assumption, not something independently verified against a real
Azure VM's own OS overhead.** PostgreSQL tier: 2 vCPU (cores 4-5) / exactly 4
GiB, disjoint cores from the app tier — confirmed these can never contend for
the same physical core.

**Backend, Redis, and all application workers genuinely share the single
4-vCPU/8-GiB budget**, as required: there is no separate worker binary or
container — background jobs (e.g. `dr_reconciliation`, confirmed actively
running) are in-process tokio tasks inside the same `hellodoctor-backend`
process already inside that budget.

**Admin Web and Doctor Web are NOT included** in Config A's resource budget
or its measured traffic — disclosed directly in `docker-compose.configA.yml`'s
own header comment (no Dockerfile exists for either yet). This is a standing
scope gap carried forward into every number in this report, not something
newly discovered or hidden here.

**CPU affinity vs. exclusivity — the precise answer:** `cpuset` is a real,
hard, *one-directional* exclusion enforced by the kernel scheduler: the app
pool physically cannot execute on cores 4-5, and Postgres physically cannot
execute on cores 0-3, so app-tier and DB-tier CPU can never steal cycles from
each other. It does **not** mean cores 0-3 are reserved exclusively *for*
Config A against everything else on the host. At the start of this
investigation, cores 0-3 were **also** claimed by the entire Config B
(capacity) stack (same cpuset) and by two long-running unpinned dev
containers (`hellodoctor-postgres`, `hellodoctor-redis`, cpuset
unrestricted) — confirmed live, with `hellodoctor-postgres` observed actively
consuming CPU at the time. **Config B was stopped for the duration of the
actual measurement run below and restarted immediately afterward** (verified
still current post-restart); the two unpinned dev containers were left
running (they are a shared, persistent dev resource not owned by this task)
and are disclosed as a residual, low-magnitude noise source rather than
eliminated.

**Load-generator isolation:** the benchmark driver (`examples/capacity_test.rs`)
is a host/container process with no isolation documented anywhere in the
existing harness. For this run it was built natively (`cargo build --release
--example capacity_test`) and launched with its Windows process affinity
explicitly pinned to cores 6-11 (mask `0xFC0`) — disjoint from both the app
pool (0-3) and DB pool (4-5) — so it cannot contend with either measured
tier. (The Dockerfile's `builder` stage does not `COPY examples/`, so an
in-container run of the example isn't currently possible without a
Dockerfile change; native-binary-plus-affinity was used instead to avoid an
unrequested Dockerfile edit.)

**One standing caveat that cannot be resolved by reconfiguring containers:**
this host is Docker Desktop on Windows via WSL2. `.wslconfig` assigns the
WSL2/Docker VM all 12 of the Windows host's logical processors, so there is
no smaller hidden allocation — but Hyper-V still time-slices those physical
cores against anything else running on Windows itself, invisibly to every
cgroup counter used here. Every number in this report is a local-dev-laptop
measurement, not a literal isolated Azure VM measurement; treat it as
directionally reliable evidence, not a production-equivalent figure.

## 2. What the "~24 MB backend memory" figure actually was

Confirmed by reading its source (`MILESTONE_8_0_DB_PERFORMANCE_REMEDIATION.md`):
it is literally one container's own cgroup RSS, reported as a single aside
line next to "PostgreSQL RAM ~630MB." It excludes Redis, Docker/OS overhead,
and the web tiers. It does **not** exclude a hidden worker process, because
none exists — but it was never a VM-wide figure, and no prior document
claims it was.

## 3. Measured memory — idle / active-ramp / peak / post-load (Config A)

Workload: `CAPACITY_MODE=sessions CAPACITY_PROFILE=REALISTIC_RAMP
CAPACITY_ACTIVE_SESSIONS=1200 CAPACITY_RAMP_SECS=120 CAPACITY_DURATION_SECS=300`
against `http://localhost:8280`, isolated load generator (cores 6-11), Config
B stopped for the run. Achieved 85.1 RPS average, 25,528 requests,
`TRUE_SYSTEM_ERROR_RATE=0.1058%`, booking success 98.5% — consistent with
(slightly better than) the previously confirmed 85–91 RPS / 0.13% / 98.3%
baseline, confirming the 8.0/8.0R fixes still hold on current code.

| Phase | backend mem | postgres mem | redis mem | backend CPU% | postgres CPU% | redis CPU% |
|---|---|---|---|---|---|---|
| **Idle** (pre-test) | 11.4–11.8 MB | 445.6–446.2 MB | 20.8–21.5 MB | ~0% | ~1.2% | ~0.5% |
| **Ramp start** (t+0–30s, ~100–300 sessions) | 13.9–17.2 MB | 465–484 MB | ~21 MB | 1.1–1.9% | 4.1–4.6% | ~0.8% |
| **Peak steady-state** (1,200 sessions active, ~t+280–300s) | 23.1–25.3 MB (max observed 25.3 MB) | 688–725 MB (climbing) | 20.7–25.4 MB | 7–9.7% | 15–21% | ~2% |
| **Immediately post-load** (+10–25s after traffic stopped) | 21.6–22.3 MB | 719–725 MB (not yet declining) | 20.9–21.3 MB | ~0.24% | ~2.1% | ~0.65% |

**Combined app-tier (backend+redis) memory at observed peak: ≈ 46 MB of the
7.9 GiB budget** — i.e., essentially no measurable pressure at this load
level (85 RPS / 1,200 sessions with 5–20s think time is a moderate,
realistic-traffic profile, not a stress test). **This headroom finding
should not be generalized to SIMULTANEOUS_BURST or a sustained multi-hour
run — neither has been measured yet (§6).**

### Per-service anon-vs-file breakdown (idle → immediately post-load)

| Container | anon (idle) | anon (post-load) | Δanon | file (idle) | file (post-load) | Δfile |
|---|---|---|---|---|---|---|
| backend | 8,536,064 B | 19,251,200 B | **+10.7 MB** | 53,248 B | 53,248 B | 0 |
| postgres | 30,076,928 B | 189,014,016 B | **+159 MB** | 399,790,080 B | 498,872,320 B | +99 MB |
| redis | 7,962,624 B | 7,913,472 B | ~0 | 10,616,832 B | 10,616,832 B | 0 |

**Backend:** +10.7 MB anonymous memory retained in the ~25s observed after
load stopped, with no further growth across the two post-load samples taken
(plateaued, not climbing). This is consistent with either (a) ordinary
allocator retention (Rust's default allocator, like glibc's, does not always
return freed heap pages to the OS immediately) or (b) genuinely retained
per-connection/session state. **Not classified as a leak** — a single
25-second post-load window cannot distinguish the two. Resolving this
requires a longer idle-recovery window and a repeated load-cycle test
(does the retained amount grow on each successive cycle, or stay flat?) —
deferred to a follow-up pass.

**Postgres:** +159 MB anon, correlated with connection-pool scale-up
observed in the same run (api pool 1→7, auth pool 0→6, payment pool 0→4 —
up to 17 concurrent backend processes vs. 1 at idle, each with its own
per-connection `work_mem`/sort overhead). This tracks plausibly with
connection count rather than looking like an unexplained leak, but was not
independently confirmed per-backend-process. The +99 MB file-backed growth
is page cache from the ramp's table/index activity — reclaimable under
memory pressure, not held hostage.

**Redis:** flat. `used_memory` grew 1.48 MB → 1.69 MB (+0.21 MB) across
+1,501 keys created during the run (1,124 → 2,625; rate-limit counters,
idempotency keys, session/admission state) — tiny per-key footprint, no
fragmentation concern (`mem_fragmentation_ratio` 6.82 → 5.95, both driven by
Redis's small absolute footprint rather than real fragmentation). RSS did not
move at all (9,834,496 B before and after) — whether those 1,501 keys carry
TTLs and will actually expire was not verified in this pass (see §6).

### Admission control / DB pool / Redis ops during the run

No admission rejections of any class during the entire 300s measurement
window (all `rejected_*` columns held at 0 throughout) — expected, since 85
RPS / 1,200 think-time-gated sessions is well under this stack's previously
established ceiling. `pg_waiting_locks` was 0 for all but three isolated
samples where it briefly read 1 — not a contention pattern. Redis
`connected_clients` grew 2 → 7 and stayed at 7 through the post-load samples
(pool connections, not leaked per-request connections).

## 4. Anomalies observed during this run (flagged, not investigated here — out of Workstream 1 scope)

Two endpoint-level results in `realistic_ramp_run.log` stood out and should
be picked up explicitly in Workstream 3 (functional dependency verification)
or Workstream 2.4 (error classification), not assumed to be defects here:

- **`POST rtc-token`: 932/932 requests auth-rejected (100%).** Either a
  genuine authorization-flow gap for the synthetic harness's session state
  (e.g. Agora token minting requires a consultation state the harness never
  establishes) or an expected/intentional business rule — not yet
  determined.
- **`POST grievance`: 380/485 (78%) rate-limited.** Could be an intentionally
  tight per-account grievance-submission rate limit, or a harness artifact
  (many sessions sharing identity/IP budgets). Not yet determined.

Neither affected `TRUE_SYSTEM_ERROR_RATE`, booking success, or the memory
figures above (both are classified as `authrej`/`ratelim`, not 5xx).

## 5. Tooling notes (for reproducing/extending this pass)

- `scripts/sample_full_telemetry.ps1` covers per-container CPU/mem, DB pool
  gauges, admission counters, basic Redis ops/clients — it does **not**
  capture Redis `used_memory`/fragmentation or the anon/file split; both were
  captured manually alongside it in this pass and should be folded into the
  script for the next one.
- Git Bash's MSYS path-conversion silently mangles absolute container paths
  passed to `docker run`/`docker exec` (e.g. turns `-w /app` into a Windows
  path and fails); `export MSYS_NO_PATHCONV=1` or a `//`-prefixed path is
  required. Cost real time in this pass (two failed load-test launches)
  before being caught.
- The backend `Dockerfile`'s `builder` stage never `COPY`s `examples/`, so
  `capacity_test` cannot be run inside a container built from it without
  modifying that Dockerfile (not done here, to avoid an unrequested
  production-image change). The native-binary + Windows `ProcessorAffinity`
  approach used here is a viable substitute but is host-OS-specific
  (PowerShell `[Diagnostics.Process]::ProcessorAffinity`), not something a
  Linux CI box could reuse as-is.

## 6. SIMULTANEOUS_BURST profile (1,200 sessions, zero ramp)

Same scale/duration as the REALISTIC_RAMP baseline, `CAPACITY_RAMP_SECS=0` —
all 1,200 sessions start in the same instant. Isolated the same way (cores
6-11, Config B stopped for the run). Raw evidence:
`simultaneous_burst_run.log`, `memory_profile_burst.csv`.

**Throughput/errors:** 105.5 RPS achieved, `TRUE_SYSTEM_ERROR_RATE=4.24%`
(vs. 0.11% under REALISTIC_RAMP) — concentrated almost entirely in the first
10-second bucket (979 of ~1,954 requests in that window; the time-series
table shows 5xx collapsing to single digits by the 20-30s bucket and staying
there for the rest of the 300s). End-to-end booking success 88.1% (vs. 98.5%
under the ramp profile) — lower, as expected for a genuine thundering-herd
stress profile, not a steady-state capacity conclusion (the harness's own
printed banner says exactly this).

**Queue pressure / rejected requests — real admission-control engagement,
the first observed in this campaign:** in the first telemetry tick
(~10s in), DB pools jumped instantly from idle to fully saturated (api
10/10, auth 10/10, payment 9/9) and admission control rejected
`rejected_normal_lane_concurrency_delta=216` and
`rejected_payment_lane_concurrency_delta=78` requests in that single window
— concrete evidence the admission control is actually shedding load under a
real burst, not merely configured and never exercised. Rejections dropped to
single digits/zero for the remaining ~290s once the pools stabilized at max
size.

**Peak memory:** backend jumped to 42.2 MB in the very first tick (vs. 25.3
MB peak under REALISTIC_RAMP), then settled to a 38-40 MB plateau for the
rest of the run — a noticeably sharper, higher initial spike than the
gradual ramp profile, consistent with 1,200 connections/sessions all
initializing at once rather than over 120s. Postgres climbed continuously
from 605 MB to 821 MB across the full 300s window and had not plateaued by
the end (unlike REALISTIC_RAMP, where it was still climbing too, but from a
lower starting point).

**Recovery:** CPU recovered to near-idle within ~10s of load stopping
(backend 1.28%→0.16%, postgres 4.27%→1.66%). Memory did **not** recover in
the same window — backend held at 38-39 MB for the ~30s observed post-load
(vs. ~21 MB pre-burst), roughly double what REALISTIC_RAMP alone had left
behind. This scales with the severity of the episode rather than looking
like unbounded drift — directly tested further in §6a below.

## 6a. Repeated-cycle test — does retained memory stabilize or accumulate? (resolves the open question from Pass 1)

Five distinct load episodes were run back-to-back on the **same, unrestarted**
Config A backend process in this session, in this order, with the backend
memory plateau observed after each:

| # | Episode | Backend mem after | Redis DBSIZE after |
|---|---|---|---|
| 1 | REALISTIC_RAMP, 1,200 sessions (Pass 1) | ~21.9 MB (+10.7 MB anon vs. idle) | 2,625 |
| 2 | SIMULTANEOUS_BURST, 1,200 sessions | 38-39 MB | 2,667 |
| 3 | 5-minute pure idle | flat 38.1-39.2 MB (no decline) | 2,261 (TTL expiry working) |
| 4 | Isolated booking/payment, 300 sessions | 38.6-40.6 MB | — |
| 5 | Isolated chat, 300 sessions (corrected run) | 38.2-39.8 MB | — |
| 6 | REALISTIC_RAMP again, full 1,200 sessions | **38.8-39.7 MB throughout — pre-load, peak, and post-load were all within the same ~1 MB band** | 3,611 |

**Conclusion: stabilizes, does not accumulate.** The large jump happened
once, from episode 1 (21.9 MB) to episode 2's burst (38-39 MB) — correlated
with DB connection pools reaching their maximum size (api 10/10, auth 10/10)
under the harder simultaneous-start profile. Every subsequent episode,
including a second full-scale 1,200-session REALISTIC_RAMP run, held at that
same ~38-40 MB ceiling instead of climbing further. The pools themselves
never shrank back down across 5 minutes of total idle (confirmed in Pass 1),
which is almost certainly *why* the floor is sticky — this looks like
connection-pool-proportional retention, not a leak. **This is not yet proof
against a slow multi-hour leak** (all 6 episodes together span under 25
minutes); Workstream 2's sustained run should keep sampling backend memory
specifically to confirm the ceiling holds over hours, not just over this
one session's ~25-minute span.

## 6b. Redis TTL effectiveness — directly verified, not inferred

Sampled TTLs on real rate-limit keys live in Redis (not assumed from code):
every sampled key had a genuine positive TTL (ranging 96s on a chat-window
key up to ~40,130s on a 24h grievance-limiter key) — **none were
persistent**, confirming expiry is actually configured, not just intended.
Across the 5-minute idle window in episode 3 above, DBSIZE fell from 2,667 →
2,261 (-406 keys) with zero traffic — direct, observed proof that expired
keys are actually being reclaimed, not just eligible for it. Growth and
shrinkage both behaved as expected: each load episode added roughly
1,300-1,500 keys (rate-limit counters, idempotency records, admission/session
state) and idle periods removed short-TTL ones. No evidence of unbounded
Redis key accumulation across this session's 5 episodes.

## 7. Isolated single-feature profiles

Reduced scale vs. the main baseline (300 sessions, 20s ramp, 90s measurement
— a diagnostic run, not a capacity claim), achieved via a small, additive
`CAPACITY_ISOLATED_ACTION` env-gated branch added to
`examples/capacity_test.rs`'s `dispatch_one_for_session` (default/unset
behavior — i.e. every previously recorded baseline — is byte-for-byte
unchanged; verified by re-reading the diff). No production code was touched.

**Booking/payment (isolated):** `isolated_booking_run.log`,
`memory_profile_isolated_booking.csv`. Backend memory 38.6-40.6 MB — within
the existing plateau, no new growth driver. 639/2,050 (31%) booking attempts
were rate-limited when booking is the *only* action every session performs
every 5-20s — a real admission-control lane-rate counter
(`rejected_payment_lane_rate_delta`) fired repeatedly (19-57 per 9s tick)
during this run, the clearest direct evidence yet of that specific limiter
engaging. **This is an expected artifact of isolating one action at full
per-session intensity, not a defect**: real traffic has booking as roughly
8% of the weighted mix (per the original `dispatch_one_for_session` table),
never 100%. End-to-end booking success dropped to 67.4% under this
artificial concentration vs. 98.5% in realistic mixed traffic.

**Chat/consultation (isolated):** first attempt (`isolated_chat_run.log`)
produced **zero** real chat data — all 2,079 samples were "GET /doctors"
fallbacks, because `chat_action` requires a conversation that only exists
after a successful `booking_action`, and an isolated chat-only dispatch
never calls booking. Caught, fixed (sessions now book once, then chat
repeatedly — `capacity_test.rs` diff comment documents why), and rerun
(`isolated_chat2_run.log`, `memory_profile_isolated_chat2.csv`): 751 real
"POST chat message" samples, **0 errors**, p50 9ms / p95 48ms / p99 73ms —
chat itself is fast and healthy. Booking-gating reproduced the same
lane-rate saturation seen in the booking-isolated run (53% of all requests
rate-limited, 10% end-to-end booking success) — consistent, not a new
finding. Backend memory held flat at 38.2-39.8 MB — chat traffic specifically
adds no measurable memory pressure beyond the existing plateau.

**Notification/background-worker profile:** no synthetic action was
generated for this — these jobs (`dr_reconciliation`, maintenance-job scans,
T-15 reminders) run on their own fixed schedule regardless of foreground
traffic, so synthetic load wouldn't change their footprint. The 5-minute
idle window in §6a/episode 3 already shows their natural signature: small,
periodic CPU upticks roughly every ~60-75s (0.08-0.10% baseline jumping to
0.7-0.8% briefly: samples at 12:54:39, 12:55:28, 12:56:32, 12:57:36),
consistent with the `dr_reconciliation` task's observed ~60s logging
interval. Memory impact of these ticks was not separately distinguishable
from the general plateau (too small relative to the ~38 MB floor to
isolate) — if finer-grained attribution is needed later, that requires
process-level profiling inside the container, not container-level cgroup
sampling.

**Clinical document upload/download:** **not measured, and not attempted.**
No example/benchmark tool anywhere in `services/backend/examples/` exercises
this flow — confirmed by direct search (`capacity_test.rs`, `load_test.rs`,
`booking_concurrency_test.rs` all lack any upload/multipart/prescription
action). Building one correctly — matching the real endpoint's multipart
contract, the 40 KB-per-image / 20-image-per-consultation caps, and the
encrypted-storage lifecycle — is real new engineering work, not a quick
harness flag, and rushing it risked producing a misleading measurement
(testing against a wrong contract) rather than no measurement. Flagged as
open work for a follow-up pass, not silently skipped.

## 8. Anomaly resolution (rtc-token / grievance) — both confirmed as harness artifacts, not backend defects

**`POST rtc-token`, 100% auth-rejected in every run so far:** the route and
handler are correct — `router.rs:480`'s `:appointment_id` really is an
appointment id (not an ID-type bug), and `ConsultationService::mint_agora_token`
(`consultation_service.rs:63-183`) creates the `consultation_sessions` row
itself on first call, so no missing "join" step exists either. The actual,
correct gate is a time window
(`AGORA_TOKEN_WINDOW_BEFORE_MINS=15`/`AGORA_TOKEN_WINDOW_AFTER_MINS=60`,
documented in `docs/AUTH_SECURITY.md §10.2`): a request outside
`[slot.start - 15min, slot.end + 60min]` is correctly `Forbidden`. Live DB
evidence: a booking made mid-test was for a seeded slot ~32 days in the
future (`scripts/seed_capacity_dataset.sql` spreads AVAILABLE slots across
many future days, consumed earliest-first by the harness but still far
outside any 15-minute window), and `rtc_token_action` requests a token
immediately after booking with no wait — something no real patient would
do. **Verdict: authorization is working as designed. Not touched, not
weakened.**

**`POST grievance`, 78-100% rate-limited depending on the run:**
`rate_limit.rs:219-221` enforces a documented 3-grievances-per-user-per-24h
fixed-window Redis counter (`docs/AUTH_SECURITY.md`'s rate-limit table,
"3 grievances / day / user → 429"), wired at
`grievance_handlers.rs:41`. `grievance_action` resubmits against the *same*
fixed historical `consultation_id` per patient repeatedly within a few
minutes of wall-clock time — far more than any real patient would attempt in
a day. **Verdict: the limit is correctly, and not overly aggressively,
protecting against exactly the repeated-submission pattern the harness
generates. Not touched, not loosened.**

## 9. Admin Web / Doctor Web — production topology and real footprint

**Documented intent:** `deploy/docker-compose.vm.example.yml` is the only
production-topology artifact in the repo, and it colocates all three
services — `backend`, `admin-web`, `doctor-web` — in one compose project,
implying one shared VM. No ADR or separate deployment doc contradicts this
(none exists). **However, that example compose file sets zero resource
limits on any service** — it is the only place colocation is declared, and
it provides no evidence that an 8 GiB envelope was ever actually sized to
include the web tiers. This is a real, confirmed gap, not an inference.

**Measured real footprint** (production `next start`, actual Node process
RSS after ~6s warm-up, no load applied, each then cleanly stopped):

| App | Idle RSS | Idle private memory |
|---|---|---|
| admin-web (Next.js 16.4.3) | 73.1 MB | 79.4 MB |
| doctor-web (Next.js 15.5.28) | 99.0 MB | 117.1 MB |

**Combined idle cost: ~172 MB**, currently entirely unaccounted for in
Config A's 7.9 GiB app-tier budget. Not measured: either app under concurrent
admin/doctor traffic, or their behavior inside an actual container image
(this was a host-process measurement, not containerized — no Dockerfile
exists for either app, per Pass 1 §1). **This is a decision item, not
something resolved by this report**: either (a) admin-web/doctor-web get
their own resource carve-out within the 8 GiB VM (even idle, ~172 MB is
small against 7.9 GiB headroom at current measured peak app-tier usage of
~46 MB — but neither app's *loaded* footprint has been measured), or (b)
they are deployed separately, in which case Config A's 4-vCPU/8-GiB budget
claim (backend+Redis only) is already accurate as scoped and no change is
needed. Recommend this be made an explicit decision in `docs/OPEN_QUESTIONS.md`
or an ADR rather than left implicit in an unlimited example compose file.

## 10. Explicit per-profile scorecard (supersedes Pass 1 §6)

- ✅ Warm idle (Pass 1 + extended 5-min window in Pass 2)
- ✅ Normal authenticated / mixed traffic (REALISTIC_RAMP ramp phase, x2)
- ✅ REALISTIC_RAMP, 1,200 sessions (run twice — see §6a)
- ✅ SIMULTANEOUS_BURST, 1,200 sessions, separately classified as stress
- ✅ Return toward idle after load (extended to 5 full minutes in Pass 2;
  memory plateaus, does not fully return to original cold baseline — now
  understood as connection-pool retention, not unexplained)
- ✅ Repeated-cycle leak check (§6a) — stabilizes across 5 episodes/~25 min;
  does not prove absence of a slow multi-hour leak (Workstream 2's job)
- ✅ Redis TTL-effectiveness / key-growth verification (§6b)
- ✅ Booking/payment-isolated profile
- ✅ Chat/consultation-isolated profile
- ✅ Notification/background-worker profile (observed via natural idle
  cadence, no synthetic action needed or possible)
- ✅ Admin Web / Doctor Web — topology decision gap identified, real idle
  footprint measured (~172 MB combined); loaded footprint not measured
- ❌ Clinical file upload/download profile — genuinely not measured, no
  harness exists for it anywhere in the repo (§7); flagged as follow-up work
- ⚠️ Cold start — still only approximated via a container restart, not a
  clean from-zero measurement
- ⚠️ Multi-hour sustained window — out of scope for Workstream 1 by design;
  carried into Workstream 2 per the original brief's sequencing

**Verdict: Workstream 1 memory profiling is now materially complete. The one
confirmed gap (clinical file upload/download) and the two items explicitly
deferred to Workstream 2 (true sustained duration, cold start) are the only
reasons this is not declared fully GREEN.** Two real, disclosed decision
items are now on record for follow-up: (1) whether Admin Web/Doctor Web
share the 8 GiB VM or deploy separately, and (2) whether DB connection pools
should be allowed to shrink on idle (currently they don't, which is the
direct cause of the ~38-40 MB sticky memory floor — not a defect, but a
tuning choice worth a deliberate decision rather than an accident). No code
was changed in the backend itself; the only source change was the additive,
default-off test-harness flag in `examples/capacity_test.rs`.
