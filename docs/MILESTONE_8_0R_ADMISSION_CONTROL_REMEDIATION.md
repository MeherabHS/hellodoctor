# Milestone 8.0R — Admission-Control Correctness and Benchmark Remediation

Date: 2026-10-31. Scope: `services/backend/src/admission.rs` and its one
downstream dependent file (`api/middleware/auth.rs` read, not modified).
Explicitly authorized as a narrowly-scoped correctness fix, not the general
8C optimization program. No repository reorganization, no Azure changes, no
real patient data, no migration edits, no destructive Docker operations, no
Git staging/commit/push. Remediations A–D and E1–E4 and the authorized Admin
OTP decision are untouched.

**Verdict: GREEN.** The excessive-rejection problem is corrected under
sustainable legitimate concurrent traffic; independent accounts now function
correctly together; every existing anti-abuse control and global overload
protection remains effective (verified, not assumed); the full in-scope
regression suite passes; before/after load measurements directly support the
conclusion.

---

## 1. Reproduced issue

Milestone 8.0's Configuration A RPS sweep (`docker-compose.configA.yml`, the
1,200-account seeded dataset) measured, at 200 offered RPS:

- `rate_limited`: 46.2% of all requests.
- `TRUE_SYSTEM_ERROR_RATE`: 0.057%.
- p50 latency flat at 3-5ms across every tested rate (50-250 RPS).
- `GET /doctors` specifically: 3,550 of 6,681 requests (53%) rejected, with
  *successful* throughput plateauing at essentially the same ~3,550-3,572
  per 60s window regardless of whether 150, 200, or 250 RPS was offered.

This combination — flat low latency, near-zero real errors, a hard
throughput ceiling independent of offered load — is the signature of a
policy ceiling, not hardware or database saturation.

## 2. Exact root cause

Traced to `src/admission.rs`'s `middleware()`/`Admission`, not assumed from
the 8-slot `Normal` lane alone, per the explicit caution to verify before
attributing cause:

1. **Primary, confirmed cause**: two **redundant, fully account-agnostic**
   global ceilings, both evaluated for *every* non-reserved (Normal / Auth /
   Upload / Payment / Expensive) request **combined**, regardless of which
   account or how many distinct accounts were calling:
   - A local in-process token bucket, `service_tokens: Mutex<Bucket>`,
     hardcoded `Bucket::new(100)` — 100/s, never derived from
     `pool_capacity`.
   - A duplicate Redis-backed copy of the same ceiling,
     `ratelimit:service:all`, `6000` per `60s` = 100/s.
   - **Evidence, not inference**: summing successful non-reserved-class
     throughput across all endpoints at the 250-RPS stage gives ≈99 req/s —
     matching the hardcoded 100/s ceiling almost exactly. The `classify()`
     function's own doc comment ("never User-Agent, caller role, or IP")
     confirms this was never account-scoped.
2. **Not the dominant cause, but also undersized**: the `Normal` lane's
   concurrency semaphore (`n.min(8)`) and the shared `ordinary`/`other_work`
   semaphores (`min(n×2, 24)`), which capped out at small fixed numbers
   regardless of how large the real combined five-pool capacity (`n`) was.
   Flat p50 latency at up to 250 offered RPS shows these had headroom to
   spare — Little's Law (concurrency ≈ rate × latency ≈ 250 × 0.005s ≈ 1.25)
   is far below even the old 8-slot cap — but they were still an
   unbenchmarked placeholder worth correcting given the evidence now exists.
3. **Already correctly scoped, confirmed unaffected**: `auth_middleware`
   (`api/middleware/auth.rs:34-56`) already applies a **per-authenticated-
   account** budget post-authentication
   (`ratelimit:authenticated:{user_id}:normal`, 120/60s = 2/s). Real
   per-account traffic in the benchmark ran at ≈0.1/s (far under 2/s) — this
   layer was never the active constraint, and is untouched.
4. **Previously uncovered**: nothing bounded an **anonymous or
   invalid-token** request shaped like the `Normal` class per-source before
   it reached `auth_middleware`'s post-auth 401 — `admission::middleware`
   runs *before* authentication. One anonymous flood could consume the old
   shared global budget alongside real accounts with no per-source
   attribution at all.

Dependency-pressure overload protection (`exhausted()`, DB pool
exhaustion → 503) and every per-account/per-IP anti-abuse control
(`check_booking_limit`, `check_otp_request_limit`, `check_login_attempt_limit`,
`ip_jail`) are separate code paths, untouched, and confirmed still effective
(§6).

## 3. Before / after admission policy

| Mechanism | Before | After |
|---|---|---|
| Cross-class global rate ceiling (local) | `service_tokens` bucket, hardcoded 100/s, shared by every non-reserved class combined | **Removed** |
| Cross-class global rate ceiling (Redis) | `ratelimit:service:all`, 6000/60s, shared by every non-reserved class combined | **Removed** |
| Per-principal budget for `Normal` | None | **New**: `ratelimit:principal:normal:{acct:<user_id>｜anon:<ip>}`, 1800/60s — generous for any real account (looser than `auth_middleware`'s own 2/s, which still binds first for authenticated callers), closes the anonymous-flood gap |
| `Normal` lane concurrency | `n.min(8)` | `n.min(64)` |
| `Normal` local rate (per-process backstop) | 100/s | 400/s |
| `Normal` per-class Redis budget | 6000/60s (100/s) | 30000/60s (500/s) |
| `ordinary`/`other_work` global concurrency | `min(n×2, 24)` / `…−1` | `min(n×2, 128)` / `…−1` |
| Auth/Upload/Payment/Expensive lanes, rates, Redis budgets | Unchanged | **Unchanged** (untouched — already small, already protective, not implicated by evidence) |
| Per-account budget (`auth_middleware`) | 120/60s for Normal | **Unchanged** |
| Per-account/per-IP anti-abuse (login, OTP, booking, `ip_jail`) | — | **Unchanged** |
| Dependency-pressure 503 (DB pool exhaustion) | — | **Unchanged** |
| Rejection-reason observability | One generic reason, `"budget"`, for every semaphore/token rejection | `enter()` now returns `(StatusCode, &'static str)`: `ordinary_concurrency` / `other_work_concurrency` / `lane_concurrency` / `lane_rate_limit`; `middleware()` adds `principal_budget` / `shared_class_budget` / `dependency_pressure` / `limiter_unavailable` — each a distinct `admission_rejected_total{reason=...}` label |
| 429 vs 503 mapping | Policy throttling → 429, capacity/dependency overload → 503 | **Unchanged** (already correct; preserved exactly) |

## 4. Changes implemented

| File | Change |
|---|---|
| `services/backend/src/admission.rs` | Removed `service_tokens` field and both its check sites; widened `Normal` lane/`ordinary`/`other_work` sizing formulas; `enter()` returns a `(StatusCode, reason)` tuple; added `principal()` (stateless JWT decode, IP fallback) and the new per-principal Redis check for `Cost::Normal`; removed `ratelimit:service:all`; raised `Normal`'s per-class Redis budget; updated/added unit tests |
| `services/backend/tests/cases/admission_control_fairness.rs` | New: 7 integration tests (§5) |
| `services/backend/tests/api_tests.rs` | Registered the new test module |
| `services/backend/tests/cases/admission_capacity_bench.rs` | No code change (its own capacity-bench logic is unaffected); comment numbers left as-is since the file is `#[ignore]`d and not part of the regular suite |

No other file was modified. No migration was touched (this is pure
application-layer rate-limit/admission logic, nothing persisted in the
schema).

## 5. Security invariants preserved (verified, not assumed)

- **No auth/RLS bypass**: `wider_admission_ceilings_do_not_weaken_authorization_or_rls` — an unauthenticated caller still gets 401 from `auth_middleware` regardless of admission capacity; a Patient token against a Doctor-only route still gets 403 from ABAC.
- **Individual abuse controls intact**: `same_account_abnormally_frequent_requests_still_rate_limited` — the *same* account past `auth_middleware`'s existing 120/60s budget is still throttled (429) with the fix in place.
- **Anonymous floods bounded, without punishing real accounts**: `anonymous_flood_is_bounded_and_does_not_starve_a_real_account` — a single anonymous source past its own per-source budget is throttled, while a distinct, legitimate, simultaneously-active authenticated account is unaffected.
- **Multiple accounts behind one IP don't throttle each other**: `patient_and_doctor_sharing_one_ip_both_succeed` — fairness is keyed by account identity, not by NAT/IP, for authenticated callers.
- **No blanket exemption invented**: the new per-principal budget (1800/60s) is a bound, not a bypass; `Normal`'s widened concurrency/rate ceilings are still bounded numbers (64, 400/s, 500/s), never unlimited; Auth/Upload/Payment/Expensive are completely untouched.
- Payment-class protection specifically confirmed still active: `POST /appointments/book` successful throughput stayed capped at exactly 300 per 60s window (5/s, its unchanged Redis budget) in *both* the before and after sweeps (§7) — proof this remediation did not accidentally loosen a sensitive class while fixing `Normal`.

## 6. Account-fairness and global-capacity regression tests

`services/backend/tests/cases/admission_control_fairness.rs`, run against
the real HTTP/DB/Redis stack (not mocks), with a production-realistic
admission concurrency capacity (`Admission::new(50)`, matching
`DB_POOL_MAX_CONNECTIONS_PER_ROLE=10 × 5` pools) rather than
`test_config()`'s deliberately tiny CI default, which would otherwise
confound the rate/fairness dimension under test with the separate,
already-covered-elsewhere concurrency-capacity dimension:

| Test | Proves |
|---|---|
| `one_patient_ordinary_browsing_always_succeeds` | Baseline: one account, ordinary use, always succeeds |
| `many_independent_patients_browse_concurrently_without_rejection` | 40 distinct Patients, concurrent — 40/40 succeed |
| `many_independent_doctors_access_dashboards_concurrently` | 40 distinct Doctors, concurrent — 40/40 succeed |
| `patient_and_doctor_sharing_one_ip_both_succeed` | Two different accounts, one IP — both succeed |
| `same_account_abnormally_frequent_requests_still_rate_limited` | One account over budget — still 429 |
| `anonymous_flood_is_bounded_and_does_not_starve_a_real_account` | Anonymous flood throttled; unrelated real account unaffected |
| `wider_admission_ceilings_do_not_weaken_authorization_or_rls` | No auth/ABAC bypass |

Mapping to Milestone 8.0R Phase 4's 15-scenario list: scenarios 1-8 and 15
are the table above. 9 (booking conflicts/idempotency) is pre-existing
coverage in `schedule_exclusivity.rs`/`booking_concurrency_test.rs`,
unaffected (Payment lanes/budgets untouched). 10 (Consultation/Webhook
reserved lanes under load) and 11 (DB capacity exhaustion) are pre-existing
coverage in `admission.rs`'s own `expensive_flood_does_not_take_ordinary_slots`
unit test and `pool_pressure.rs`, neither touched by this change. 12/13
(bounded waiting, safe rejection, recovery) follow structurally from
`try_acquire_owned` (admit-or-reject immediately, never a queue) — unchanged
by this fix. 14 (no bypass) is scenario 15 plus the full suite in §8.

Each test asserts the actual resulting status/business outcome (200/401/403/429),
not merely "a response was returned" — e.g. the fairness tests assert the
*count* of 200s among concurrent distinct-account requests, not just that
the request completed.

## 7. Before / after load metrics — Configuration A, identical sweep

Same deterministic 1,200-account dataset, same weighted request mix, same
`capacity_test.rs` harness, same `docker-compose.configA.yml` topology
(4 vCPU/8 GiB app pool + separately-resourced 2 vCPU/4 GiB Postgres pool),
same 50/100/150/200/250 RPS × 60s stages. Backend rebuilt from the fixed
source between runs; nothing else in the stack changed.

| RPS | `rate_limited` before | `rate_limited` after | `GET /doctors` succ/total before | `GET /doctors` succ/total after | Real error rate before | after |
|---|---|---|---|---|---|---|
| 50  | 0.000%  | 0.061% | 1707/1707 (100%) | 1734/1734 (100%) | 0.0000% | 0.0307% |
| 100 | 3.317%  | 3.335% | 3302/3302 (100%) | 3352/3352 (100%) | 0.0000% | 0.0159% |
| 150 | 29.792% | 4.431% | 3549/5029 (70.6%) | 4950/4950 (**100%**) | 0.0323% | 0.0538% |
| 200 | 46.183% | 5.285% | 3550/6681 (53.1%) | 6638/6638 (**100%**) | 0.0569% | 0.1382% |
| 250 | 55.576% | 6.078% | 3572/8266 (43.2%) | 8245/8246 (**~100%**) | 0.0131% | 0.3987% |

**The remaining post-fix `rate_limited`% is not residual bug — it is the
correctly-functioning, untouched Payment budget.** At 250 RPS after the fix,
`POST /appointments/book` alone accounts for 890 of the 918 total rejected
requests (890/15,301 ≈ 5.8% of all traffic, against an overall reported
`rate_limited` of 6.078%) — its successful throughput stayed pinned at
exactly 300/60s (5/s) in *both* runs, proving this specific protection was
never part of the bug and remains exactly as protective as before.

**Throughput**: `GET /doctors` successful throughput at the 250-RPS stage
went from ≈59/s (hard-capped, unable to rise with offered load) to ≈137/s
(scaling with offered load, no longer capped) — a genuine ~2.3× increase in
real served traffic for this endpoint at the same offered rate, achieved
entirely by removing an artificial ceiling, not by weakening any check.

**First hints of real (not policy) saturation now visible**: real error
rate rose from 0.013% to 0.399% at 250 RPS, and `POST chat message` p99
latency rose from 171ms to 500ms at the same stage — small, but the first
*genuine* signal now exposed once the artificial ceiling stopped masking it.
The true hardware/DB saturation point was **not reached** within this
50-250 RPS range; finding it is in scope for the full Milestone 8.0 sweep
(which extends further), not this focused 8.0R verification.

Full logs:
`docs/performance-evidence/milestone-8-0/configA/sweep.log` (before),
`docs/performance-evidence/milestone-8-0/configA/sweep_after_8_0R.log` (after).

**Resource-sampling limitation, disclosed rather than glossed over**: the
container CPU-utilization sampler (`scripts/sample_container_resources.ps1`,
built on `docker stats --no-stream`) returned implausible near-zero CPU%
for all three containers throughout the after-fix sweep even while actively
serving ~250 RPS — consistent with a known limitation of single-snapshot
`docker stats --no-stream` calls, which need two time-separated internal
readings to compute a valid CPU delta. **CPU utilization during this sweep
is therefore NOT_MEASURED**, not "confirmed low" — memory figures from the
same tool look stable and plausible and are reported for completeness, but
the CPU column should not be trusted. This must be fixed (e.g. a
continuously-streamed `docker stats` reader, or reading
`cpuacct.usage`/cgroup counters directly) before the full Milestone 8.0
profiling phase relies on this script.

## 8. Full regression results

| Suite | Command | Result |
|---|---|---|
| Format | `cargo fmt --check` | Clean |
| Lint | `cargo clippy --all-targets -- -D warnings` | Clean |
| Rust library | `cargo test --lib` | **129 passed**, 0 failed (includes updated `admission::tests` reflecting the new sizing) |
| Rust integration | `cargo test --test api_tests -- --test-threads=1` (disposable Redis fixture) | **460 passed**, 0 failed, 4 ignored (453 baseline + 7 new `admission_control_fairness` tests) |
| Dependency audit | `cargo audit --deny warnings --ignore RUSTSEC-2023-0071` | Clean, exit 0 |
| Admission-control tests | `admission::tests::*` (5), `admission_control_fairness::*` (7) | All pass |
| Per-account anti-abuse | `security_acceptance.rs`, `same_account_abnormally_frequent_requests_still_rate_limited` | All pass, unaffected |
| Booking concurrency | `schedule_exclusivity.rs`, `booking_concurrency_test.rs` | Pass, unaffected (part of the 460) |
| Consultation/webhook reservation | `admission.rs::expensive_flood_does_not_take_ordinary_slots` | Pass, unaffected |
| Flutter/Next.js | Not re-run | Not needed — the HTTP contract (status codes, `error.code` values, response body shape) is byte-for-byte unchanged; only internal metric reason labels changed. Verified by inspection of `reject()`'s `code` mapping, which no new reason string alters. |

Every command above exited 0. No existing test was weakened, skipped, or
deleted.

## 9. Remaining limitations

- The widened concurrency/rate numbers (`Normal` lane 64, `ordinary`/`other_work`
  128, local rate 400/s, Redis class budget 500/s, per-principal 1800/60s)
  are evidence-informed but not exhaustively tuned against a true saturation
  point — because that point was not reached in this sweep (§7). Revisiting
  these numbers once the full Milestone 8.0 sweep finds the real ceiling is
  reasonable future work, not required for this remediation's own claim.
- The resource-sampling tool's CPU% column is unreliable (§7) — CPU
  utilization during both sweeps is NOT_MEASURED, not confirmed-low, though
  the latency/error-rate evidence independently supports "no hardware
  saturation in this range."
- This remediation covers the `Normal` cost class specifically (the one the
  evidence implicated). Auth/Upload/Payment/Expensive were deliberately left
  untouched as out of evidence-supported scope, per the mission's own
  instruction not to weaken those paths' protections.

## 10. Classification: GREEN

All seven Strict-GREEN criteria are met: the excessive-rejection problem is
corrected under sustainable legitimate concurrent traffic (§7); independent
accounts function correctly together (§6); anti-abuse protections remain
effective (§5, §6); global overload protection remains effective (dependency-
pressure 503 untouched, concurrency ceilings still bounded, not unlimited);
complete in-scope functional dependencies are verified (§6, §8); required
regression tests pass (§8); and measured before/after results directly
support the conclusion (§7), with no fabricated or rate-limited-counted-as-
successful throughput claims.
