# Milestone 8 — Workstream 2: Sustained Reliability and Recovery

Status: **IN PROGRESS.** This report covers (1) a confirmed application
defect found and fixed, (2) the approved single-VM production architecture
change and its implementation in `docker-compose.configA.yml`, (3) a
15-minute benchmark under the corrected topology, and (4) a critical,
previously undocumented PostgreSQL durability gap this topology change
exposes. A true 60+ minute endurance window and the 10,000-user/2,000-session
scalability objective are tracked separately (§6, §7) — this is not yet a
complete Workstream 2 acceptance.

## 1. Confirmed defect: `transaction_number` UUID-truncation collision — FIXED

**Root cause**: `services/backend/src/services/appointment_service.rs:172`
generated `transaction_number` from only the first 8 hex characters of a
UUID (32 bits). At this session's cumulative volume (~9,446 payment-class
transactions when it was first observed), a collision on
`transactions_transaction_number_key` occurred exactly once, surfacing as an
unhandled `500 Internal Server Error` — the only non-admission-control 5xx
found across this entire Milestone 8 campaign.

**Fix applied**: use the full UUID (122 random bits) instead of truncating
it. `transaction_number` is `VARCHAR(50)`; `"TXN-" + 36-char UUID` = 40
chars, well within bounds. Collision probability is now cryptographically
negligible rather than realistically reachable at the 10,000-user/2,000-session
target (truncated-UUID collision risk scales quadratically with volume —
it would only have gotten worse).

**Verification**:
- `cargo build --release` — clean compile.
- `cargo test --release --lib` — **129 passed, 0 failed, 0 ignored.**
- Rebuilt Config A's backend image, recreated the stack, confirmed `/healthz`
  reports the new binary live (`binary_latest_migration` =
  `database_latest_migration` = `20261202000005`, `migrations: "current"`).
- Live confirmation under real traffic: the 15-minute benchmark in §3 ran
  6,082 booking attempts (hence 6,082 fresh UUID-based transaction numbers)
  with **zero** `transaction_number` collisions and **zero** `500`s of any
  kind (`curl /metrics | grep 'status="500"'` returns nothing post-fix).

No full `cargo test` integration suite (419+ tests, ~20 min) was run in this
pass given the time constraints on this cycle — the library-test regression
plus a live 15-minute production-traffic verification is the evidence
gathered so far. Recommend the full integration suite before final Milestone
8 acceptance sign-off.

## 2. Approved single-VM architecture — implemented

**Decision** (user-directed, effective this report): the entire platform —
Rust backend, PostgreSQL 16, Redis, in-process background workers, Admin
Web, Doctor Web, reverse proxy, and Ubuntu/Docker overhead — runs on **one**
Azure VM: 4 vCPU / 8 GiB. There is no separately allocated or managed
PostgreSQL instance. Configuration A is now the primary production
benchmark; Configuration B's prior results remain historical/diagnostic
evidence only, not a production-capacity model.

**What changed in `services/backend/docker-compose.configA.yml`** (full
reasoning in the file's own header comment):

| Service | Old (two-pool) | New (single-VM) |
|---|---|---|
| postgres `cpuset` | `4-5` (dedicated, isolated) | `0-3` (shared with backend+redis) |
| postgres `mem_limit` | 4 GiB | 4 GiB (unchanged absolute figure) |
| postgres `cpu_shares` | 0 (unset) | 3000 |
| backend `mem_limit` | 7.4 GiB | 2 GiB |
| backend `cpu_shares` | 9000 | 6000 |
| redis `mem_limit` | 500 MiB | 400 MiB |
| redis `cpu_shares` | 1000 | 500 |

8 GiB budget: backend 2.0 + postgres 4.0 + redis 0.4 = **6.4 GiB actually
enforced**, plus a documented 0.9 GiB reserve for the web tier (not yet
containerized — see §5) and 0.7 GiB for OS/Docker overhead = 8.0 GiB total.
The web-tier and OS reserves are budget math, not live cgroup limits on a
running container.

**Live-verified** via `docker inspect` immediately after recreation:
```
postgres: CpusetCpus=0-3 Memory=4294967296 CpuShares=3000
backend:  CpusetCpus=0-3 Memory=2147483648 CpuShares=6000
redis:    CpusetCpus=0-3 Memory=419430400  CpuShares=500
```
All three now genuinely share the same 4 physical cores — the isolation
that made Postgres CPU pressure structurally invisible to the app tier in
the old topology no longer exists, which is the entire point of this change
(it matches the real target deployment).

**Data preserved**: the named Docker volume (`hellodoctor_confa_pgdata`) was
not deleted — recreating the postgres container with new resource params
kept all data; `patient_profiles` count confirmed unchanged (1,150) after
recreation, and `/healthz` confirmed the schema was still current without
needing a fresh migration run.

## 3. 15-minute benchmark under the corrected topology

`CAPACITY_MODE=sessions CAPACITY_PROFILE=REALISTIC_RAMP
CAPACITY_ACTIVE_SESSIONS=1200 CAPACITY_RAMP_SECS=120 CAPACITY_DURATION_SECS=900`,
isolated load generator (cores 6-11, Config B stopped for the duration).
Raw evidence: `singlevm_15min_run.log`, `memory_profile_singlevm_15min.csv`.

### Throughput and errors — no meaningful regression

| Metric | Old two-pool baseline | New single-VM (this run) |
|---|---|---|
| Achieved RPS | 85–91 (earlier), up to ~105 (burst) | **96.7** |
| `TRUE_SYSTEM_ERROR_RATE` | 0.05–0.13% | **0.10%** |
| End-to-end booking success | 98.3–98.5% | **96.1%** |
| Real `500`s | 1 (the now-fixed collision) | **0** |

### Latency — a real, modest, honestly-measured degradation

| Percentile | Old two-pool baseline (steady state) | New single-VM (steady state) |
|---|---|---|
| p50 | 5–6 ms | **5–6 ms — unaffected** |
| p95 | 19–29 ms | **17–35 ms, mostly 19–27 ms — comparable, slightly wider spread** |
| p99 | 29–41 ms | **37–57 ms, with one spike window (640–650s) at 76 ms — a real ~20–40% increase in the tail** |

This is the expected, honest cost of the architecture decision: Postgres and
the backend now genuinely compete for the same 4 cores, and the tail
latency shows it. p50 throughput-under-load is unaffected; the tail is
where contention shows up first, exactly as CPU-sharing theory predicts.

### CPU contention — real, but not yet severe at this load level

Peak single-sample CPU observed: backend 8.86%, postgres 11.22%, redis
2.09% — **summed, only ~22% of the shared 4-core budget at peak**, nowhere
near saturation. This confirms items (1) genuinely answerable now that
wasn't possible before (backend and postgres CPU are visible on the exact
same cores and can be directly compared), but also means **this benchmark
does not yet prove the single-VM topology holds up under real pressure** —
1,200 sessions at ~97 RPS leaves enormous CPU headroom even shared. The
2,000-session test (§7) is the one that will actually stress this.

### Memory — no limit pressure, no OOM

| Container | Peak observed | New limit | Peak as % of limit |
|---|---|---|---|
| backend | 25.97 MB | 2 GiB | 1.3% |
| postgres | 529.8 MB | 4 GiB | 13.0% |
| redis | 15.68 MB | 400 MiB | 3.9% |

`OOMKilled=false` and `RestartCount=0` on all three, confirmed via
`docker inspect`. (Postgres's peak here, 530 MB, is notably lower than the
~800+ MB seen in earlier same-session-count runs — this is a **freshly
restarted process** from the rebuild, without the accumulated connection/
cache growth prior runs built up; not a contradiction of earlier findings,
just a different starting point.)

### An observation flagged, not fully explained

In the final ~2 minutes of the run, `admitted_webhook_delta` and
`admitted_payment_delta` both dropped to a sustained zero while `GET
/doctors`-class traffic continued — the same signature as the slot-exhaustion
pattern found in the earlier 60-minute run. However, this time **13,410
appointment slots remained available system-wide** when it happened (not
exhausted), which doesn't fully match a pure "ran out of slots" explanation.
Two live possibilities, not adjudicated in this pass: (a) a few specific
doctors'/days' slots that this run's bookings preferentially drew from were
exhausted while others remained, or (b) a business rule capping concurrent
active bookings per patient caused many of the 1,200 sessions to stop
successfully re-booking once each already held one. **Correcting the
previous report's confident "slot exhaustion" framing to "an unexplained
booking-admission plateau with two candidate causes, not yet distinguished"**
— this needs a dedicated look before the dataset/harness fix, not a repeat
of the same assumption.

## 4. Critical gap: PostgreSQL durability and backup — NOT YET REMEDIATED

This is the most significant finding of this workstream so far, and it is a
direct consequence of the architecture decision in §2.

**What the repo currently assumes** (`docs/BACKUP_RESTORE_RUNBOOK.md` §"Normal
protection" and `docs/DISASTER_RECOVERY_ARCHITECTURE.md`): both documents are
written entirely around **Azure Database for PostgreSQL Flexible Server** —
"Provision Flexible Server... Enable Azure Backup for the server... geo-redundant
backup selected at creation... retention 35 days." `DISASTER_RECOVERY_ARCHITECTURE.md`
explicitly frames "PostgreSQL on VM + pgBackRest" as the **fallback** path
only, used "if a regulatory or regional constraint... cannot use the managed
service," and its own comparison table lists self-hosted backup as
higher-complexity, operator-owned work: "pgBackRest repository, WAL
retention, corruption checks and failover are our responsibility."

**What the new architecture actually requires**: self-hosted Postgres on the
VM is no longer the fallback — it is the *only* path, by explicit decision.
The managed-service backup/PITR story this project's own DR documentation
relies on **does not exist** under this architecture.

**What actually exists today, confirmed by direct search**: nothing. No
`pg_dump`/`pg_basebackup` cron job, no `archive_command`/WAL-archiving
configuration, no pgBackRest setup, no off-VM backup destination anywhere in
`deploy/`, `services/backend/scripts/`, or any compose file. The current
Postgres container has a single Docker named volume and nothing else. **If
that VM or its volume is lost today, all clinical, financial, and account
data is lost with it.** This is a critical gap for a project explicitly
building a healthcare platform under CLAUDE.md's own stated privacy/safety
bar, and it did not exist as a documented risk until this architecture
decision made it load-bearing.

**Not remediated in this pass** — this needs its own deliberate design
(likely: scheduled `pg_dump` or WAL archiving to the already-approved Azure
Blob Storage, since that remains the external managed service in this
architecture; encryption at rest for the backup artifacts; a tested restore
procedure; disk capacity planning for WAL retention on a now-shared 4 vCPU/8
GiB box). Flagging this as a required follow-up, not attempting a rushed
implementation under the current time constraint.

## 5. Admin Web / Doctor Web / reverse proxy — still not containerized

Unchanged from the Workstream 1 finding: no Dockerfile exists for either
Next.js app, and no reverse proxy config exists in this repo. The 0.9 GiB
web-tier reserve in the new compose file's budget is money set aside, not an
enforced limit on a running container. Real measured idle footprint remains
~172 MB combined (admin-web 73.1 MB + doctor-web 99.0 MB,
`docs/MILESTONE_8_MEMORY_PROFILE.md` §9); loaded footprint is still
unmeasured. **Building and containerizing both apps plus a reverse proxy,
then re-verifying the 8 GiB budget against their real combined resource use
under load, remains required before Configuration A can be called a
complete, verified single-VM model.**

## 6. Sustained-duration gap — 15 minutes is not an endurance test

Per the original Milestone 8 brief's own requirement ("at least a 60-minute
sustained profile... do not treat a five-minute run alone as evidence
against long-term memory leaks"), this 15-minute run is explicitly
insufficient on its own for a sustained-reliability verdict. It is real,
useful evidence for the *topology change's* immediate effect (contention is
visible, throughput/error-rate hold, no OOM), but says nothing new about
multi-hour drift beyond what Workstream 1's repeated-cycle testing already
established. A genuine 60+ minute run under this new topology is still
owed, and is blocked on the same dataset slot-exhaustion/plateau question
from §3 being understood first — otherwise a long run just reproduces an
unexplained traffic drop-off rather than clean sustained-load evidence.

## 7. Scalability objective (10,000 users / 2,000 sessions) — not yet attempted

Explicitly sequenced after this 1,200-session result per the user's own
instruction. A 10,000-patient seed top-up script
(`services/backend/scripts/seed_capacity_dataset_scaleup_10k.sql`, additive —
does not touch the existing 1,150-patient historical dataset) has been
prepared but not yet run. A short (~4 minute) 2,000-session test against
this corrected single-VM topology is the next step, to be reported
separately.

## 8. Per-item verdict

| Item | Verdict | Why |
|---|---|---|
| `transaction_number` defect fix | **GREEN** | Root-caused, fixed, 129/129 lib tests pass, verified live with 0 new 500s/collisions under real traffic |
| Single-VM topology (backend+redis+postgres) | **GREEN** | Implemented, live-verified via `docker inspect`, real contention now measurable, no OOM, data preserved |
| 15-minute benchmark | **PARTIAL** | Real results obtained; throughput/p50 hold, p99 tail degrades modestly as expected; one traffic-pattern anomaly not yet explained |
| PostgreSQL durability/backup | **FAILED (newly disclosed, not yet remediated)** | No backup/WAL-archiving mechanism exists for the now-mandatory self-hosted Postgres; this is a genuine production blocker, not a benchmark artifact |
| Admin Web/Doctor Web/reverse proxy containerization | **PARTIAL (unchanged from Workstream 1)** | Idle footprint measured; not containerized; loaded footprint unknown |
| 60+ minute sustained endurance | **NOT ATTEMPTED under new topology** | Blocked on understanding §3's booking-admission plateau first |
| 10,000-user/2,000-session scalability objective | **NOT ATTEMPTED** | Seed script ready; next step |

**Overall Workstream 2 verdict (as of §1–§8): PARTIAL.** The architecture
change itself is soundly implemented and the bug fix is solid, but this is
not yet a complete sustained-reliability acceptance — most importantly, the
backup/DR gap in §4 should be treated as a release blocker independent of
any benchmark number, and the booking-admission plateau in §3 needs a real
explanation before a 60-minute run is worth running again.

## 9. ⚠️ §9 BELOW IS INVALID FOR SUSTAINED AUTHENTICATED CAPACITY

**Record this explicitly**: the 4-minute result in the original §9 below is a
valid *short-burst* resource-footprint data point, but any 40-minute
conclusion drawn from runs before §10 must be discarded. A real bug in the
test harness itself — not the backend — caused a massive, misleading 401/429
collapse after ~15 minutes in every run longer than that. Root cause, fix,
and proof are documented in full in §10. The diagnostic value of discovering
that bug (the exact admission-control mechanism it triggered) is preserved
there; only its use as evidence of backend capacity is retracted. The
corrected, trustworthy 40-minute result is in §10.

## 9. 10,000-user / 2,000-session scalability objective — resource-footprint result

**This is explicitly a ~4-minute test, not a sustained/endurance run — it
answers "what does the resource footprint look like at this scale," not
"does it hold up for hours."** Dataset was topped up additively (new
`scripts/seed_capacity_dataset_scaleup_10k.sql`, does not touch the original
1,150-patient historical set) to 10,000 total patients / 13,415 available
slots before this run. `CAPACITY_ACTIVE_SESSIONS=2000 CAPACITY_RAMP_SECS=30
CAPACITY_DURATION_SECS=210` (30s ramp + 210s steady state ≈ 4 min total),
same isolated single-VM topology and cores-6-11 load-generator isolation as
§3. Raw evidence: `singlevm_10k2k_4min_run.log`, `memory_profile_10k2k_4min.csv`.

### Throughput, latency, errors

| Metric | 1,200 sessions (§3) | 2,000 sessions (this run) | Ratio |
|---|---|---|---|
| Achieved RPS | 96.7 | **165.4** | 1.71x (sessions grew 1.67x — essentially **linear/proportional**) |
| p50 (steady state) | 5–6 ms | **5–6 ms — unchanged** | — |
| p95 (steady state) | 17–35 ms | **22–38 ms** | modestly wider |
| p99 (steady state) | 37–57 ms | **51–68 ms** | a further real increase, consistent with more concurrency on the same 4 shared cores |
| `TRUE_SYSTEM_ERROR_RATE` | 0.10% | **0.3455%** | ~3.4x — a real increase, but still low in absolute terms |
| End-to-end booking success | 96.1% | **92.1%** | lower, expected under higher concurrency contention for the same booking/payment admission lanes |

All error growth traces directly to admission-control shedding, not new
defects — confirmed via `/metrics`: `admission_rejected_total{class="webhook",
reason="lane_concurrency"}` grew by exactly 61 during this run (matching the
log's `POST webhook confirm` syserr count of 61 exactly), and
`{class="expensive", reason="lane_concurrency"}` grew by exactly 59
(matching `GET admin telemetry` syserr of 59 exactly). **Zero new `500`s** —
`curl /metrics | grep 'status="500"'` still returns nothing; the
`transaction_number` fix held under 2,573 fresh booking attempts in this run
alone (cumulative session total now well past 12,000 transactions since the
fix, zero collisions).

### Resource footprint — the key finding: memory scales far sub-linearly

| Resource | Peak @ 1,200 sessions | Peak @ 2,000 sessions | Ratio | vs. 1.67x session ratio |
|---|---|---|---|---|
| Backend CPU | 8.86% | 14.37% | 1.62x | ~proportional |
| Postgres CPU | 11.22% | 19.9% | 1.77x | slightly worse-than-proportional |
| Redis CPU | 2.09% | 3.07% | 1.47x | sub-linear |
| **Backend memory** | 25.97 MB | **27.09 MB** | **1.04x** | **far sub-linear** |
| **Postgres memory** | 529.8 MB | **614.3 MB** | **1.16x** | **far sub-linear** |
| **Redis memory** | 15.68 MB | **15.79 MB** | **1.01x** | **flat** |

CPU scales roughly in proportion to session count (expected — more
concurrent work), but **memory barely moved at all** despite 67% more
sessions. This is a strong, positive signal for the scalability objective:
the per-session memory cost at the Connection-pool/session-state layer is
small and not the limiting factor. DB connection pools did grow (api 8→10,
auth 6→7) but stayed well short of any configured maximum.

No OOM, no container restarts (`docker inspect` confirms `OOMKilled=false`,
`RestartCount=0` on all three). All peaks remain tiny fractions of the new
single-VM mem_limits (backend 27 MB / 2 GiB = 1.3%; postgres 614 MB / 4 GiB
= 15.4%; redis 15.8 MB / 400 MiB = 4.0%).

A brief admission drop-off appeared in the final ~30–45 seconds of this run
too — but unlike the mid-run plateau in §3, **11,002 slots remained
available afterward** (only 2,413 of the 13,415 consumed) and the drop
coincides exactly with the test's own 210s deadline, so this one is
confidently attributed to normal session wind-down (in-flight sessions
finishing their last action, none dispatching new ones as the deadline
hits), not the same unexplained plateau — worth stating explicitly since
the pattern looks superficially similar.

### Verdict

**PARTIAL, leaning positive.** At 2,000 concurrent sessions over a short
window, throughput scales linearly, memory scales sub-linearly (a genuinely
good sign for headroom), and CPU contention — while real and growing — has
not come close to saturating the shared 4-core budget (postgres's 19.9% peak
still leaves enormous room before 100%). Zero defects, zero OOM. **This
does not yet demonstrate the objective is met**, because: (1) 4 minutes
cannot speak to sustained behavior at this scale — the same pool-growth/
memory-plateau questions from Workstream 1 apply here and are unanswered at
2,000 sessions specifically; (2) the admission-control tuning (webhook/
expensive lane concurrency limits) was sized against the 1,200-session
baseline and is now visibly shedding more under 2,000 — whether that's
correctly calibrated headroom or needs retuning for the new target is not
yet evaluated; (3) the backup/DR gap in §4 and the web-tier containerization
gap in §5 apply equally at this scale and remain unresolved.

## 10. The real 40-minute, 2,000-session result (corrected benchmark)

### 10.1 What was actually wrong, and why it was the test tool, not the backend

The first 40-minute attempt showed `auth_reject=29.9%` and
`rate_limited=33.8%` appearing abruptly partway through. Root-caused
precisely, with file:line evidence, not assumed:

- `services/backend/src/crypto.rs:394`: the real access-token TTL is exactly
  15 minutes (`ACCESS_TOKEN_TTL_SECONDS: i64 = 15 * 60`) — correct,
  untouched, working as designed.
- `examples/capacity_test.rs` minted all ~2,000 patient tokens **once**, at
  test startup, into a plain immutable `Vec<String>`. `refresh_action`
  (the harness's own attempt at renewal) read a `patient_state.refresh_token`
  field that was **never set anywhere in the file** — always `None` — so it
  silently fell back to a filler read and never once called the real
  `/api/v1/auth/refresh` endpoint, for the entire history of this test tool.
- Consequently, ~15 minutes into any run longer than that, every session's
  token expired within the same narrow window, and the backend correctly
  started returning real `401`s.
- **The 429 surge was then independently proven**, not assumed, from
  `src/admission.rs:359-371`: the admission middleware's `principal()`
  function falls back to a key of `anon:<source-ip>` whenever JWT
  verification fails (expiry included). All ~2,000 sessions share one
  load-generator IP. That shared anonymous key has a hardcoded budget of
  **1,800 requests / 60 seconds** (`admission.rs:438-442`) — instantly and
  permanently exhausted once thousands of expired-token requests per second
  collapsed onto it, explaining why `rate_limited` (75,611) outnumbered
  `auth_reject` (26,043): most expired-token traffic was rejected by the
  exhausted shared bucket before ever reaching the point of returning a
  clean 401. `GLOBAL_IP_RATE_LIMIT_PER_MIN=100000` (set in
  `docker-compose.configA.yml` to neutralize a *different*, unrelated IP
  limiter for this benchmark) was ruled out — not remotely close to being
  hit by this volume.
- **Verdict on the backend: no defect.** Both behaviors — rejecting expired
  tokens, and treating a flood of auth-failing traffic from one IP as an
  abuse signal — are the backend working exactly as designed. The anonymous
  fairness bucket being small is intentional: real anonymous traffic from a
  single IP is not supposed to have much room.

### 10.2 The fix (test harness only — no production code touched)

`examples/capacity_test.rs`: patient, doctor, and admin tokens are now each
stored as `RwLock<(String, Instant)>` instead of a plain `String`, and every
call site reads through a `fresh_token()` helper that proactively re-mints
a token (via the same `create_access_token` signing-key shortcut the
harness already used for *initial* auth) once it's within 3 minutes of the
real 15-minute TTL, with a small per-identity jitter (0-119s, deterministic
from index) so ~2,000 identities minted together don't all re-check in the
same instant. Doctor and admin token pools were included too, since
`wallet summary` and `admin telemetry` traffic uses those small round-robin
pools and would otherwise hit the identical expiry problem independently.

**Deliberately not done**: replicating the real refresh-token rotation
protocol (hash-based verification, reuse-detection that revokes a whole
token family on replay, device-proof binding — see
`src/services/auth_service.rs:886-1112`). That's real security-sensitive
surface, and getting it subtly wrong under 2,000-way concurrency would be a
new failure mode in its own right. This harness still never exercises the
real `/api/v1/auth/refresh` endpoint under load — a disclosed, deliberate
gap, not a silent one.

### 10.3 Validation before the long rerun

Per instruction, a focused 200-session, ~20-minute run (crossing the real
15-minute TTL comfortably) was run first. Result: **zero 5xx across all 120
time-buckets**, `TRUE_SYSTEM_ERROR_RATE=0.0000%`. `auth_reject=6.395%` and
`rate_limited=1.804%` were present but **100% attributable to the two
already-understood, pre-existing business rules** (rtc-token's time-window
gate; grievance's 3/day limiter) — zero auth-rejection or rate-limiting on
every other endpoint, including in buckets well past the 15-minute mark.
Direct log spot-check (`docker logs ... --since 15m | grep "Status code: 401"`)
returned zero matches. The fix works. Full 40-minute rerun authorized and
executed.

### 10.4 The corrected 40-minute, 2,000-session result

`CAPACITY_ACTIVE_SESSIONS=2000 CAPACITY_RAMP_SECS=120 CAPACITY_DURATION_SECS=2400`,
10,000-patient dataset, single-shared-4-vCPU/8-GiB Config A topology
(unchanged from §2), isolated load generator. Raw evidence:
`sustained_2k_40min_v2_run.log`, `memory_profile_2k_40min_v2.csv`,
`worker_redis_state_2k_40min_v2.csv`.

**Headline**: `achieved_rps≈160`, `TRUE_SYSTEM_ERROR_RATE=0.3503%`,
`auth_reject=7.141%` (**27,870 of 27,914 — 99.8% — is rtc-token's
already-known time-window gate; every other endpoint shows zero**),
`rate_limited=1.765%` (**overwhelmingly grievance's known limiter, plus a
modest, expected amount of real booking lane-rate shedding under sustained
2,000-session pressure**). **Zero real `500`s** (`curl /metrics | grep
'status="500"'` empty). **Zero OOM, zero restarts** on all three containers.

All `syserr` (admission-control 503 shedding) is accounted for exactly
against `/metrics` deltas — e.g. webhook lane-concurrency grew by exactly
296 (matching the log's webhook-confirm syserr of 296 precisely), expensive
lane-concurrency grew by exactly 941 (matching admin-telemetry's syserr of
941 precisely). **`principal_budget` (the anonymous-bucket limiter behind
the whole original incident) did not increase by a single count this run** —
the cascade is completely, verifiably gone.

### 10.5 5-minute interval breakdown

| Elapsed | Backend CPU | Postgres CPU | Redis CPU | Backend mem | Postgres mem | Redis mem | Payment/webhook admitted (per ~16s tick) |
|---|---|---|---|---|---|---|---|
| 0 min (ramp start) | — | — | — | — | 613.7 MB (pre-load) | 10.4 MB | 0 |
| 5 min | 15.4% | 21.5% | 2.4% | 32.2 MB | 789.4 MB | 10.9 MB | ~180-230 |
| 10 min | 14.4% | 19.1% | 2.4% | 31.7 MB | 1,036.5 MB | 11.5 MB | ~190-220 |
| 15 min | 13.7% | 19.1% | 2.4% | 32.3 MB | 1,106.3 MB | 11.0 MB | ~150-165 |
| 20 min | 13.3% | 18.6% | 2.4% | 32.0 MB | 1,143.6 MB | 11.1 MB | ~95-115 (cap plateau beginning) |
| 25 min | 12.5% | 18.2% | 2.4% | 32.3 MB | 1,141.2 MB | 11.7 MB | ~45-65 |
| 30 min | 12.2% | 16.2% | 2.5% | 31.9 MB | 1,160.1 MB | 11.1 MB | ~20-35 |
| 35 min | 11.9% | 16.8% | 2.4% | 31.5 MB | 1,122.6 MB | 11.5 MB | ~5-15 |
| 40 min (load ending) | 0.1%* | 0.7%* | 0.4%* | 31.0 MB | 1,137.4 MB | 12.1 MB | 0 |

*drops to near-zero because the load generator finished at ~40 min; this is
expected test completion, not a backend event.

**Memory — the real sustained-duration answer**: backend and Redis were
flat the entire run (27-34 MB and 10-12 MB respectively, no trend at all).
**Postgres climbed steadily for the first ~20 minutes (614 MB → 1,144 MB),
then stabilized** — the remaining 20 minutes fluctuated within a tight
1,120-1,160 MB band with no further sustained upward trend. This is the
clean answer Workstream 1 couldn't fully give with shorter/contaminated
runs: Postgres memory growth under 2,000-session sustained load **plateaus,
it does not run away**, and it settled at 28.5% of its new 4 GiB budget —
comfortable headroom.

**CPU declined slightly over the run** (backend 15%→12%, Postgres 21%→16%)
— this tracks directly with the booking-attempts cap (§3) gradually
reducing booking/payment/webhook work as more of the 2,000 sessions
individually exhausted their 8-attempt limit over time; it is not a
performance regression signal.

**The booking-cap plateau (§3) is confirmed as the only other notable
pattern, and it is now smooth and gradual, not a cliff** — `admitted_payment`
/`admitted_webhook` per-tick counts decline steadily from the ~5-minute mark
(~200/tick) down to near-zero by ~35 minutes, consistent with sessions
independently and probabilistically reaching their 8-attempt cap at
staggered times rather than all at once. This is expected harness behavior,
fully explained, not a capacity concern — disclosed per instruction as the
decline in booking/payment traffic this causes over a long run.

**Redis**: DBSIZE grew to a peak of ~11,150 keys around the 14-minute mark,
then **declined on its own to 4,457 by the end** (TTL expiry outpacing
creation as booking-related key generation tapered with the cap plateau) —
further confirmation of healthy, bounded Redis behavior. `durable_erasure_pending_count`
stayed at exactly 0 for the entire 40 minutes — no backlog.

**Slots**: 64,176 remained at the end (started this run at 78,798) — no
exhaustion, plenty of margin remained.

### 10.6 Final verdict

**GREEN for the 2,000-session sustained-duration objective, as far as backend
performance and stability go.** Zero defects, zero OOM, zero restarts, error
rate fully and exactly accounted for by known, correctly-working business
rules and admission-control fairness limits, memory demonstrably stabilizes
rather than growing unbounded over 40 real minutes. This is the first
trustworthy sustained-duration evidence this milestone has produced at this
scale — everything before this section involving a run longer than ~15
minutes should be treated as superseded.

**Still explicitly PARTIAL, not a full production-readiness sign-off**,
because:
- The critical PostgreSQL backup/DR gap (§4) is unresolved and independent
  of this result — you've indicated this will be handled separately.
- Admin Web, Doctor Web, and a reverse proxy are still not containerized or
  load-tested (§5) — the "single shared 4 vCPU/8 GiB" claim is proven for
  backend+Redis+Postgres only so far.
- This harness still never exercises the real `/api/v1/auth/refresh`
  endpoint under load (§10.2) — a real client-facing code path with zero
  load-test coverage.
- ~~Whether the real Flutter app, Admin Web, and Doctor Web correctly renew
  their own sessions before expiring in actual production use~~ — **checked
  separately, resolved: all three are fine.** Flutter
  (`apps/mobile/lib/core/network/api_client.dart:313-475`) reactively
  catches `401`, deduplicates concurrent refreshes, correctly stores the
  rotated access+refresh tokens, and retries idempotent requests once
  (non-idempotent requests get a deliberate "please retry" `409` instead of
  a silent double-submit — a reasonable safety choice, not a gap). Admin Web
  and Doctor Web use a different, BFF/HttpOnly-cookie model (the browser
  never holds the raw access token at all); both correctly detect `401`,
  call the real refresh endpoint server-side, and retry once — Admin Web
  additionally has a cross-process Redis-backed mutex
  (`apps/admin-web/src/lib/refresh.ts:39-160`) so multiple server instances
  racing the same refresh cookie can't trigger the backend's reuse-detection
  and revoke a live session, a correctness detail Doctor Web's equivalent
  (`apps/doctor-web/src/lib/server.ts`) lacks but which only matters for a
  multi-instance deployment, not for whether a single real user stays
  logged in. None of the three has anything resembling the benchmark
  harness's bug. A real patient, doctor, or admin keeping their app open
  past 15 minutes will not be logged out or see failures from this cause.
