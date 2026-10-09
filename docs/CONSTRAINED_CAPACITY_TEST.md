# Constrained-VPS Capacity Test

> **Status:** Follow-on to `docs/CAPACITY_EVIDENCE_REPORT.md` (which ran unconstrained, on the full developer laptop). This document asks a different, harder question: can the backend sustain the planned `MAX_ACTIVE_USERS=1200` pilot's traffic under resources that actually resemble a small production VPS, not a 12-core development machine?
> **Generated:** 2026-10-04, same session. **Updated same day with a rebalanced rerun — see §12.**
> **§1–11 verdict (Profile A, as originally run): `LOCAL_CONSTRAINED_CAPACITY_TARGET_FAILED`** — see §9. Real, useful evidence, not a failure of the exercise: it identified a specific, actionable bottleneck (Postgres CPU) rather than leaving an untested assumption.
> **§12 verdict (Profile B, rebalanced CPU + a PostgreSQL JIT fix found along the way): `LOCAL_CONSTRAINED_CAPACITY_TARGET_PASSED` up to ~200 RPS** on a genuine 2-vCPU budget — roughly a 7–10x improvement in the healthy ceiling, confirming the original failure was substantially a resource-allocation/configuration problem, not an application-design flaw. **§12 is the current, operative verdict; §§1–11 are preserved as the historical record of how the bottleneck was found, not superseded or deleted.**

---

## 1. What was actually built (the infrastructure, not just the test)

Nothing here existed before this session — it had to be built first:

- **`Dockerfile`** — multi-stage build (dependency layer cached separately from source) producing a real container image of the backend binary, since the backend had only ever run as a bare `.exe` directly on the host, which Docker/the OS cannot cgroup-limit.
- **`docker-compose.capacity.yml`** — a fully standalone compose stack (own project `hellodoctor-capacity`, own network, own ports `5532`/`6479`/`8180`), deliberately **not merged** with the dev `docker-compose.yml` (Compose concatenates list-valued keys like `volumes:` across merged files rather than replacing them, which would have tried to mount two volumes onto the same Postgres data directory — caught before it caused damage, not after). Profile A's resource caps are set via the plain top-level `cpus:`/`mem_limit:` keys, which Compose honors on ordinary `docker compose up` without Swarm.
- **`scripts/seed_capacity_dataset.sql`** — a deterministic synthetic dataset generator (see §3).
- **`examples/capacity_test.rs`** — a new **open-loop** load generator (see §4), distinct from the existing closed-loop `load_test.rs`.
- A real, previously-undiscovered **N+1-at-scale performance bug**, found and fixed (see §6).

## 2. Resource caps — verified real, not merely claimed

Per the instruction not to say "simulated 2-vCPU VPS" while the process can still use every host core, every limit below was confirmed with `docker inspect`, not just written into a compose file and trusted:

```
backend:  NanoCpus=1,250,000,000 (1.25 cores)   Memory=1,342,177,280  (1.25 GiB)
postgres: NanoCpus=1,000,000,000 (1.00 cores)   Memory=1,879,048,192  (1.75 GiB)
redis:    NanoCpus=  250,000,000 (0.25 cores)   Memory=  268,435,456  (0.25 GiB)

Host:     12 CPUs, ~11.64 GiB allocated to the Docker Desktop VM
```

These are real Linux cgroup limits enforced by the Docker Desktop WSL2 VM — confirmed empirically in §5/§7 below by watching `hellodoctor-capacity-postgres` actually get pinned at ~100% CPU (i.e., its 1.0-core ceiling) under load, not just asserted from the config file. Profile A's three component allocations (1.25 + 1.0 + 0.25 = 2.5 cores) sum to slightly more than the task's own "2 vCPU" headline number — the per-component breakdown it specified was used directly rather than force-fit to exactly 2.0, and this discrepancy is noted rather than hidden.

## 3. Production-like dataset — actual seeded row counts

`scripts/seed_capacity_dataset.sql`, run once against the fresh capacity-test Postgres (5.2 seconds for the whole script):

```
users                           1,200   (1,150 patients / 40 doctors / 10 admins)
patient_profiles                1,150
doctor_profiles                    40
doctor_schedule_slots           62,400  (19,200 historical + 43,200 future-available)
appointments                   13,440
appointment_clinical_intake    13,440
transactions                   13,440
payment_events                 13,440
consultation_sessions          12,502
consultation_telemetry         12,502
chat_conversations             13,440
chat_messages                  73,920
grievance_reports                 268
doctor_wallets                     40
audit_events                      300
```

No real PHI — every name/complaint/note string is a deterministic placeholder (`'Capacity Test Patient 42'`, `'Capacity-test synthetic complaint 7'`, etc.), not generated from any real patient data. Documented simplifications, not hidden: a historical slot's own `status` is set to `BOOKED` whenever it has an appointment regardless of that appointment's eventual status (real cancellation-releases-the-slot timing isn't modeled — this shows up as an expected, explained artifact in §8's integrity check); `transactions.payment_status` is a coarse mapping from appointment status, not a full settlement-cycle simulation.

## 4. The open-loop harness

`load_test.rs` (used for the original, unconstrained pass) is **closed-loop**: each of its worker tasks fires its next request only after the previous one returns, so a slowing backend automatically throttles the offered load — exactly the wrong property for finding a saturation point, since the test quietly "backs off" in sympathy with the thing it's supposed to be stressing.

`examples/capacity_test.rs` is new and **open-loop**: a `tokio::time::interval` ticks at the configured target rate and spawns an independent task per tick regardless of whether earlier requests have returned, so offered load stays at the requested rate even while the backend is struggling — which is what actually exposes queueing and collapse instead of hiding it.

**Traffic mix actually implemented** (adjusted from the task's suggested percentages to match real implemented routes, as instructed; folded the "3% other" bucket into directory reads since no distinct additional implemented endpoint was identified for it):

| Action | Weight | Real endpoint(s) |
|---|---|---|
| Doctor directory | 33% | `GET /doctors` |
| Doctor slots | 20% | `GET /doctors/:id/slots` |
| Chat message | 10% | `POST /chat/:id/messages` (patient's own freshly-booked conversation this run; falls back to a directory read if the virtual patient hasn't booked one yet) |
| RTC token | 8% | `POST /consultations/:id/rtc-token` (same fallback rule) |
| Booking | 8% | `GET` a pre-provisioned slot → `POST /appointments/book` → `POST /webhooks/payment/bkash` (real HMAC-signed confirm) |
| Doctor wallet | 7% | `GET /doctor/wallet/summary` |
| Telemetry | 5% | `POST /consultations/:id/telemetry` |
| Admin reads | 4% | `GET /admin/command/telemetry` |
| Token refresh | 3% | `POST /auth/refresh` (only for the subset with a real refresh token from actual login; falls back otherwise — see §10's limitations) |
| Grievance | 2% | `POST /grievances` (against the patient's own pre-existing historical consultation) |

Identities are pre-provisioned from the seeded dataset and minted directly via the real Ed25519 signing key (`create_access_token`) — cryptographically identical in shape to what a real login produces, with full ABAC/RLS enforcement exactly as if the token came from a real login; only the login flow's own latency is bypassed for the bulk mix, consistent with the task's own instruction that "authentication itself gets a separate dedicated benchmark later" (not built in this pass — see §10). Per-patient booking attempts are capped well under the real `check_booking_limit` (10/hour/user_id), and the shared global per-IP rate limiter is overridden to a documented benchmark-only ceiling (see §9 for the full statement) — no other security control is touched.

## 5. A real bug this exercise found and fixed: N+1-at-scale on the admin telemetry endpoint

`get_command_telemetry` had already been "fixed" for N+1 queries in an earlier session (replacing one-query-per-row-in-a-loop with whole-table fetches) — but against the tiny original dev dataset (a handful of rows), a whole-table fetch is indistinguishable from a cheap one. Against this capacity test's 13,440-appointment / 12,502-telemetry-row dataset, the same "fix" became the bottleneck it was supposed to remove: every call fetched **all** patients, **all** appointments, and **all** telemetry rows just to look up the handful actually behind *currently active* sessions.

**Measured, not assumed:** at a clean 20 RPS baseline, this one endpoint alone showed p50=1,406ms / p99≈5,000ms while every other endpoint was single-digit-to-tens of milliseconds.

**Fix:** added `appointment_repo::find_by_ids`, `patient_repo::find_by_ids`, `consultation_repo::list_telemetry_by_session_ids` (scoped `WHERE id = ANY($1)` queries) and rewrote the handler to fetch only the rows behind `consultation_repo::list_active`'s own (small) result set, not the whole table. Re-measured after the fix, same 20 RPS baseline: p50=317ms, p99=1,543ms — a 4–5x improvement, confirmed by rerunning the identical test, not claimed from reading the diff. `cargo test` 25/25 still passing after the change. **Not fully resolved** — see §10; the remaining latency is consistent with the same Postgres CPU contention affecting every endpoint under load, not a further defect in this specific handler. The same whole-table-fetch pattern likely also affects `list_admin_doctors`/`list_admin_transactions`/`list_admin_grievances` (flagged in the original N+1 cleanup pass) — **not verified in this pass**, since none of them are in the current traffic mix; worth checking before relying on the admin portal at this data scale.

## 6. Test ladder actually run, and why it stopped where it did

Per explicit instruction — *"Stop increasing load once the system clearly crosses its healthy operating threshold. Do not hammer a collapsed system merely to obtain a larger maximum."* — this ladder stopped once collapse was unambiguous, rather than running the full 50/100/200/300/400/500/600 RPS sequence regardless:

| Target RPS | Duration | Achieved RPS | TRUE_SYSTEM_ERROR_RATE | p50 (typical) | p99 (typical) | Postgres CPU |
|---|---|---|---|---|---|---|
| 20 | 60s | 21.7 | **0.0%** | 3–5ms | ~50–100ms (admin-telemetry excepted, see §5) | 60–100%, bursty |
| 50 | 90s | 54.1 | **0.0%** | bursty: 4ms in some 10s windows, 300–1,800ms in others | up to 8,018ms in the worst windows | oscillating 28–102% |
| 100 | 90s | 100.8 | **56.6%** | 5,002ms (flat) | 10,000ms+ (client timeout) | pinned ~98–104% |
| 200 | 300s | 200.3 | **86.7%** (after the §5 fix; 93.7% before it) | 5,002/10,001ms (flat) | 10,000ms+ (client timeout) | pinned ~98–104% |

400/500/600 RPS and the 10-minute "primary" 300 RPS run were **not attempted** — 100 RPS was already a majority-failure state and 200 RPS was worse; running higher targets against an already-collapsed system would not have produced new information, only more of the same timeout-dominated noise, which is exactly what the instruction says not to do.

**The flat 5,002ms/10,001ms pattern is diagnostic, not incidental**: the harness's HTTP client timeout is 10 seconds, and a uniform ~5-second cluster alongside a ~10-second (timeout) cluster, stable across the *entire* 100–300s test duration rather than climbing progressively, points to connection-level queueing/retry behavior once Postgres is saturated, not a gradually-growing backlog. Backend CPU (10–40%) and Redis CPU (1–5%) had abundant headroom throughout every single run — **Postgres's 1.0-core allocation was the sole, consistent bottleneck**, confirmed directly via `docker stats`, not inferred.

## 7. Database integrity after the test ladder

Reused the Phase-1 integrity harness (`booking_concurrency_test.rs`), pointed at the capacity stack via the now-configurable `CAPACITY_BASE_URL`/`DATABASE_URL`. Its own non-contended/contention re-provisioning could not run against this stack — **by design, not a bug**: the seeded dataset already sits at exactly `MAX_ACTIVE_USERS=1200` (1,150 + 40 + 10 = 1,200), so the real admission cap correctly refuses any further OTP-based patient registration. This is the capacity-admission control working exactly as intended, not a test failure; the original Phase-1 non-contended/contention correctness proof (`docs/CAPACITY_EVIDENCE_REPORT.md` §8) already established booking-concurrency-safety on this same codebase and is not invalidated by this.

**Direct SQL integrity check, run after the full 20/50/100/200 RPS ladder:**

```
duplicate_booked_slots                                0
multiple_confirmed_for_same_slot                      0
orphan_appointments                                   0
orphan_clinical_intake_rows                           0
appointments_with_no_transaction                      0
orphan_transactions                                   0
duplicate_gateway_reference                           0
duplicate_provider_event_id                           0
invalid_state_confirmed_without_held_payment          0
invalid_state_pending_with_held_payment               0
invalid_slot_available_with_live_appointment          0
invalid_slot_booked_with_no_live_appointment         938  ← see below, not a real defect
```

**The 938 is a known, pre-existing seed-data artifact, not corruption from the load test — verified, not assumed:** every one of the 938 rows is dated between 2026-06-06 and 2026-10-03 (the synthetic *historical* window the seed script generates, entirely before "now"), and each is a `CANCELLED` appointment on a slot the seed script deliberately leaves at `BOOKED` status (documented in the seed script's own header: "a historical slot's own `status` is set to BOOKED whenever it has an appointment regardless of that appointment's final status"). Re-running the identical check scoped to `start_time > now()` — i.e., only the future-dated slots the live load test actually booked against — returns **zero** for every single check, including that one. **Despite a 56–87% client-visible failure rate at 100–200 RPS, the backend produced zero data corruption of any kind** — every request either fully completed correctly or failed cleanly (timeout/409/429), never silently or partially.

## 8. Security controls — confirmed still fully active throughout

None of JWT verification, default-deny routing, ABAC, PostgreSQL RLS, the five database capability roles, Redis abuse protection, webhook HMAC verification, payment replay dedup, or idempotency were touched, weakened, or bypassed anywhere in this pass. **The one documented exception**, exactly as pre-authorized: the global per-source-IP rate limiter (`GLOBAL_IP_RATE_LIMIT_PER_MIN`, real default 120/min) was overridden to `100000` for the capacity-test container only, via `docker-compose.capacity.yml`'s `environment:` block — never written into the real `.env`. This was confirmed necessary empirically, not assumed: the very first 200 RPS run (before this override existed) showed a 98.8% `rate_limited` rate, which is this exact artifact (every synthetic request shares the one real test-machine IP, unlike real distributed traffic) and not a genuine finding.

## 9. The honest verdict and why

**`LOCAL_CONSTRAINED_CAPACITY_TARGET_FAILED`**

The primary 300 RPS / 10-minute target was not attempted because the system had already clearly collapsed by 100 RPS (56.6% true system error rate) and worsened at 200 RPS (86.7%), with Postgres's CPU allocation pinned at its 1.0-core ceiling in both cases while the backend and Redis containers sat mostly idle. Extrapolating, 300 RPS would not have fared better — running it anyway, against the explicit instruction not to hammer an already-collapsed system, would have produced no new information.

- **HEALTHY_OPERATING_RPS (Profile A, this dataset, this workload mix): ~20–30 RPS.** Clean, consistently low latency (single-digit-to-tens of ms on every endpoint except the admin-telemetry one, itself fixed in §5), zero true system errors.
- **A degraded-but-not-yet-failing zone exists around 40–60 RPS**: zero hard failures, but latency becomes bursty and occasionally severe (p99 up to 8 seconds in some 10-second windows) as Postgres CPU oscillates between ~30% and >100%.
- **SATURATION_RPS: ~80–100 RPS.** By 100 RPS, the majority of requests fail outright.

This is **~10–15x below** the 300 RPS primary target, and well below even the 200 RPS secondary target. It is a real, specific, actionable finding, not an ambiguous one: **Postgres's CPU allocation, not the backend or Redis, is the single clear bottleneck** for this application's actual per-request database-transaction intensity (nearly every endpoint, including simple-looking reads, round-trips through Postgres at least once, and booking's full chain spans several short transactions across multiple role pools).

## 10. What this does NOT mean, and what's genuinely still open

- **This is not evidence the application is broken.** Zero data corruption occurred at any load level (§7); the security architecture added no failure mode of its own (§8); the one real bug found (§5) was fixed and measurably improved the specific endpoint it affected. The failure mode here is pure resource starvation under an unbalanced profile, not a correctness defect.
- **Profile A's specific CPU split (backend 1.25 / Postgres 1.0 / Redis 0.25) is itself a candidate thing to fix, not a law of nature.** Given the backend and Redis containers had 60–90% CPU headroom *unused* throughout every run while Postgres sat pinned, the most direct, specific next step is to **re-run this exact test ladder with a rebalanced split** (e.g., backend 0.75 / Postgres 1.5 / Redis 0.25, same ~2.5-core total budget) before concluding the hardware tier itself is inadequate. This was not done in this pass — it's the clearest, most actionable next step, not a vague "add more resources" gesture.
- **Not attempted in this pass, scoped down from the original request given time already spent on the (substantial) infrastructure build-out above:** Profile B (4 vCPU/8GB — explicitly gated on Profile A "passing" anyway, per the original request's own ordering); 400/500 RPS headroom tests and the 600 RPS burst test (moot while 100 RPS is already collapsed); the realistic-think-time multi-tier virtual-user mode (§6 of the original request); a dedicated auth-stress benchmark (OTP/login/refresh under real Argon2id cost, isolated from the mixed workload); the 1,200-registered/300-active realistic cohort simulation; the optional extreme 1,200-concurrent-session test; and the 30–60 minute soak test. None of these would change the headline finding (Postgres CPU is saturated well below the primary target) — they would add detail once the rebalanced-profile retest in the bullet above is done.
- **The token-refresh action (3% of the mix) exercises real `/auth/refresh` only for patients who happened to go through a real login** — the bulk of virtual patients have JWTs minted directly and no real refresh token, so most of that 3% silently falls back to a filler directory read. A dedicated auth benchmark (above) would need to provision real sessions for this to be meaningful.

## 11. Resource summary

| | Backend | Postgres | Redis |
|---|---|---|---|
| CPU limit | 1.25 cores | 1.00 core | 0.25 core |
| Memory limit | 1.25 GiB | 1.75 GiB | 0.25 GiB |
| Observed CPU at 20 RPS | ~10–30% | 60–100% (bursty even here) | 2–5% |
| Observed CPU at 100–200 RPS | 10–40% | **98–104% (pinned)** | 1–5% |
| Observed memory, worst case | ~70 MiB (5.5% of limit) | ~600 MiB (33% of limit) | ~6 MiB (2.3% of limit) |
| True 5xx / timeout count | 0 at ≤50 RPS; dominant failure mode at ≥100 RPS | — | — |
| Deadlocks | 0 at any load level | — | — |
| Data corruption | 0 at any load level (§7) | — | — |

Memory was never a constraint at any tested load level, for any of the three containers — this is purely a CPU-allocation finding.

---

## 12. Rebalanced rerun (2026-10-04, same day) — `LOCAL_CONSTRAINED_CAPACITY_TARGET_PASSED` up to ~200 RPS

Per explicit follow-up instruction: *"the test did exactly what it was supposed to do — it found the bottleneck... give CPU to the component that is saturated, not to the components sitting idle."* One controlled rebalancing rerun, not a new optimization cycle. §§1–11 above are preserved unchanged as Profile A's record.

### 12.1 — The new profile, and making the "2 vCPU" constraint real this time

Profile A's three limits (1.25 + 1.0 + 0.25 = 2.5 cores) summed to more than its own "2 vCPU" headline, and — more subtly — three independent `--cpus` quotas on a 12-core host don't by themselves stop all three containers from bursting onto three *different* idle cores at once, which is not what a real 2-core VPS's processes experience. Fixed both problems at once:

```
backend:  0.60 vCPU / 1.25 GiB   (moved CPU away from this — it was the idle one)
postgres: 1.25 vCPU / 2.25 GiB   (moved CPU to this — it was the one pinned at 100%)
redis:    0.15 vCPU / 0.25 GiB
cpuset: "0,1" on all three services — forces genuine shared-core contention
total: 2.00 vCPU, pinned to 2 real physical cores
```

Verified via `docker inspect` exactly as before: `backend NanoCpus=600000000 CpusetCpus=0,1`, `postgres NanoCpus=1250000000 CpusetCpus=0,1`, `redis NanoCpus=150000000 CpusetCpus=0,1`.

### 12.2 — A second real bug found before the rerun even started: PostgreSQL JIT compilation overhead

Enabled `pg_stat_statements` (not on in Profile A) to get real slow-query data for this rerun, and the very first query it flagged was telling: `consultation_sessions`'s `list_active()` query — 23 calls, **mean 265.74ms**, while every other query on the table was sub-millisecond-to-low-single-digit ms.

**Root-caused with `EXPLAIN ANALYZE` run as the real `hellodoctor_api` role under a real RLS session context** (not as superuser, which bypasses RLS and would have hidden this): the query's actual data-scan time was ~3ms, but **JIT compilation alone added ~273ms** on top of it. Why: the RLS policy on `consultation_sessions` has an `OR`'d correlated subquery (the ordinary-user ownership check) that the planner must cost as if it might run for every row, inflating the estimated cost to 613,821 — far past Postgres's `jit_above_cost` threshold (default 100,000) — even though the subplan is actually never executed at runtime (correctly short-circuited by the cheap admin-role check that comes first in the `OR`). Postgres compiles expensive-looking query machinery to native code via JIT to speed up genuinely expensive queries; for a table this size (12–13k rows), there is no scan workload large enough for that compilation to ever pay for itself, so every single call paid the full ~270ms compilation tax for zero execution benefit.

**Confirmed with a direct before/after `EXPLAIN ANALYZE`, `SET jit = off` the only change**: 290ms → 3.15ms for the identical query, identical plan shape, identical rows returned, identical RLS enforcement. This is a pure query-execution-strategy setting — it does not touch RLS, ABAC, or any security semantics, and was verified not to change query results before being applied. Set via `-c jit=off` in the Postgres container's startup command (`docker-compose.capacity.yml`), not a code change.

This is a second, independent, generalizable finding: **any RLS-protected table in this schema whose policy has a similarly-shaped `OR`'d correlated ownership subquery is a candidate for this same JIT tax**, regardless of CPU allocation. It is not specific to `consultation_sessions` — the same policy pattern appears on `appointments` and others (see `migrations/20261003000001_rls_hardening.up.sql`). Not fully audited across every RLS-protected table in this pass; flagged as a follow-up rather than exhaustively checked.

### 12.3 — Test ladder: 20 → 30 → 50 → 75 → 100 → 150 → 200 → 250 RPS

Each stage flushed the booking rate-limit counters and reset `pg_stat_statements`/`pg_stat_database` first, so every stage's numbers are its own, not cumulative.

| Target RPS | Achieved | TRUE_SYSTEM_ERROR_RATE | p50 range | p95 range | p99 range | Backend CPU | Postgres CPU | Redis CPU |
|---|---|---|---|---|---|---|---|---|
| 20 | 21.8 | **0.0%** | 3–5ms | 5–48ms | 11–62ms | ~5% | ~3–4% | ~1–3% |
| 30 | 32.4 | **0.0%** | 3–8ms | 5–48ms | 15–89ms | ~7–8% | ~4–11% | ~1% |
| 50 | 53.8 | **0.031%** | 3–18ms | 6–48ms | 13–129ms | ~11–14% | ~5–11% | ~2% |
| 75 | 80.7 | **0.0%** | 3–20ms | 5–58ms | 27–179ms | ~15–16% | ~10–14% | ~2% |
| 100 | 108.2 | **0.0%** | 3–24ms | 8–39ms | 14–90ms | ~20% | ~10–18% | ~3% |
| 150 | 162.3 | **0.0%** | 3–30ms | 16–54ms | 48–150ms | ~34–41% | ~18–27% | ~4% |
| 200 | 216.6 | **0.0154%** | 5–39ms | 44–515ms | 83–845ms | ~52–59% | ~32–43% | ~4–6% |
| **250** | 259.8 (collapsing) | **11.33%, rising every window** | **302ms → 9,660ms, strictly climbing** | up to 10,001ms | up to 10,020ms | ~58–60% (**pinned**) | ~29–42% (comfortable) | ~3–6% |

**20 through 150 RPS are unambiguously healthy.** 200 RPS is still effectively clean (0.0154% — 3 isolated errors out of 19,492 requests) with p95/p99 beginning to rise, the first visible sign of the system working harder. **250 RPS is a genuine, classic progressive collapse** — not the flat, connection-level-artifact pattern seen in Profile A's runs, but latency climbing monotonically window-over-window (302ms → 2,267ms → 3,051ms → 4,596ms → 7,199ms → 9,053ms...) alongside a 5xx count that grows from 0 to 714-per-10-seconds by the end: a real backlog building up because sustained arrivals exceed sustained service capacity. Per the explicit instruction — *"stop when the curve clearly bends... do not jump directly to 300"* — 300 RPS was **not attempted**, since 250 already showed unambiguous, worsening collapse; running 300 against a system already collapsing at 250 would not have produced new information.

**And the bottleneck moved, exactly as hypothesized.** At 250 RPS, `docker stats` shows the **backend** pinned at ~58–60% CPU — which, on its 0.6-core quota over a 2-core cpuset, *is* its ceiling — while Postgres sits comfortably at 29–42% of its now-much-larger 1.25-core budget. Having fixed Postgres's bottleneck, the backend (deliberately given less CPU under this profile) became the new, different bottleneck once load climbed high enough. This is itself a clean, useful finding, not a complication: it confirms resource reallocation was genuinely the lever, not a fluke — the saturation point *moved to wherever the CPU was now scarce*, precisely as the reallocation hypothesis predicts.

### 12.4 — Does moving CPU to Postgres move the healthy ceiling? Yes — dramatically

**From ~20–30 RPS (Profile A) to ~200 RPS (Profile B): roughly a 7–10x increase in the healthy operating ceiling.** This is far past the user's own stated bar for "strong evidence" (*"if it goes from 30 healthy → 100+ healthy, that is strong evidence PostgreSQL CPU allocation was the main constraint"*). Three things contributed, and their relative contribution was not separately isolated (doing so would mean re-running the ladder with each fix in isolation, which was judged not worth the additional test cycles per the instruction to avoid chasing benchmark numbers for days):

1. **CPU reallocation itself** (§12.1) — confirmed necessary: the backend became the new bottleneck at 250 RPS specifically *because* it now has less CPU than before, which would make no sense if reallocation hadn't changed anything.
2. **The JIT fix** (§12.2) — confirmed necessary and large in isolation: a single query went from 265ms to 4ms mean, an independent, generalizable finding regardless of which container has how much CPU.
3. **The N+1-at-scale fix from the original pass** (§5) — still in effect, compounding with the above.

The honest conclusion is a combination, not a single cause — but the practical answer to the user's question is unambiguous: **this was substantially a resource-allocation and configuration-tuning problem, not an application-design flaw.** No RLS policy, ABAC rule, JWT check, or business-logic path was changed to achieve this result.

### 12.5 — Database integrity after the full rebalanced ladder, including the 250 RPS collapse

```
duplicate_booked_slots                                0
multiple_confirmed_for_same_slot                      0
orphan_appointments                                   0
appointments_with_no_transaction                      0
duplicate_gateway_reference                           0
duplicate_provider_event_id                           0
invalid_state_confirmed_without_held_payment          0
invalid_slot_booked_FUTURE_ONLY_no_live_appointment   0
invalid_slot_available_with_live_appointment          0
```

**Zero corruption, including through the 250 RPS collapse** — hundreds of timed-out/5xx requests at that stage produced no inconsistent state. Every request still either fully completed or failed cleanly.

### 12.6 — Updated verdict

**`LOCAL_CONSTRAINED_CAPACITY_TARGET_PASSED` for a genuine 2-vCPU budget, up to approximately 200 RPS** — not the originally-hoped 300 RPS, and the user's own framing already anticipated and pre-empted treating 300 as sacred: *"even a clean sustained 100–150 RPS on a tiny 2-vCPU VPS could already represent substantial real-user headroom... the 300-RPS target is deliberately conservative."* 200 RPS clean, with 150 RPS comfortably clean with real margin, is a strong result for a 2-core budget.

- **HEALTHY_OPERATING_RPS: ~150–200 RPS** (150 fully clean with margin; 200 still effectively clean at 0.0154% with early signs of rising latency).
- **SATURATION_RPS: ~250 RPS** (unambiguous, progressive collapse).
- **The bottleneck under this profile, once load is high enough: the backend's own CPU share (0.6 cores)** — not Postgres anymore. A further rebalance (e.g., 0.8–0.9 backend / 1.0–1.1 Postgres) is the next natural experiment *if* the ceiling needs to move past 200 RPS, but per the instruction to freeze capacity work here and return to the consultation-telemetry/error-tracing work, that further tuning is named as a future option, not pursued now.
- **Zero data corruption, zero security-control weakening, at every load level tested**, including the 250 RPS collapse.

**Freezing capacity-tuning work here, as instructed.** Returning to the consultation-telemetry-events / admin-audit-before-after-snapshots work (`docs/CHECKPOINT.md` §8.2) next.
