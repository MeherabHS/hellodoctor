# Milestone 8.0 — Three Verification Items (JWT admission identity, booking
# fairness, CPU measurement) and the booking-fairness fix

Date: 2026-10-31. Scope: exactly the three items a reviewer asked to close
out before final Milestone 8.0 production-capacity conclusions, against
`docs/MILESTONE_8_0R_ADMISSION_CONTROL_REMEDIATION.md`'s "GREEN within its
defined scope" result. No broad security-audit restart, no unrelated
refactoring. Files touched: `services/backend/src/admission.rs` (one
evidence-justified booking-fairness fix, §2), its unit test, one stale
comment in `tests/api_tests.rs`, four new JWT-identity tests in
`tests/cases/admission_control_fairness.rs`, and
`services/backend/scripts/sample_container_resources.ps1` (CPU sampler
rewrite). No migration, no Git staging/commit/push, no destructive Docker
operation (the Config-A stack was rebuilt/restarted in place, never
recreated-from-scratch/volumes-wiped).

---

## 1. JWT admission identity — VERIFIED CORRECT, no code change needed

**Claim to check**: does `admission.rs::principal()` verify the bearer
token's signature/claims, or merely parse them unverified — could a forged
token bypass admission or poison/ride another account's rate-limit bucket?

**Finding: already correct.** `principal()` calls
`crypto::verify_access_token()` — the exact same primitive
`auth_middleware` uses for real authentication, not a separate/weaker
parser. That function (`src/crypto.rs:437`) does full EdDSA signature
verification against the server's Ed25519 public key, plus issuer,
audience, and expiry (`validate_exp = true`) validation. A token that fails
any of those returns `Err`, and `principal()` maps that to `None` →
`anon:{ip}` — the same fallback as having no token at all. There was no
separate unverified-claims code path to find.

Why this closes every variant the reviewer asked about:
- **Forged signature**: an attacker without the server's private key cannot
  produce a token that verifies against its public key for an arbitrary
  `user_id`, regardless of what claims they write into the unsigned parts —
  `decode::<Claims>` rejects it before `principal()` ever reads `user_id`.
- **Expired**: `validate_exp = true` rejects it at decode time, independent
  of whether the signature itself is otherwise valid.
- **Malformed**: `decode()` fails to parse it at all.
- In every case, the request is **not treated as that account** for
  budgeting purposes — it falls to the IP bucket — and is still rejected
  with a normal 401 by `auth_middleware` immediately afterward, exactly as
  an absent token would be. There is no way to exhaust or "ride" a specific
  victim account's `ratelimit:principal:normal:acct:{id}` bucket without
  possessing a token that genuinely, cryptographically verifies as that
  account.

**New regression coverage** (`tests/cases/admission_control_fairness.rs`),
run against the real HTTP/DB/Redis stack:

| Test | Proves |
|---|---|
| `forged_signature_token_cannot_poison_or_use_victim_accounts_budget` | A victim's principal budget is pre-exhausted; a tampered-signature token claiming the victim's identity gets 401 (downstream auth), never the victim's own 429 — proving admission never attributed it to the victim |
| `expired_token_cannot_poison_or_use_victim_accounts_budget` | Same, for a genuinely-signed-but-expired token |
| `malformed_bearer_token_fails_closed_without_crashing_or_attribution` | A non-JWT bearer value fails closed with 401, no panic/5xx |
| `exhausting_one_valid_accounts_principal_budget_does_not_affect_another` | Two distinct *valid* accounts are independently keyed — exhausting one's `ratelimit:principal:normal:acct:*` budget never blocks the other |

All 11 tests in the module (7 existing + 4 new) pass:
`cargo test --test api_tests admission_control_fairness -- --test-threads=1`
→ **11 passed, 0 failed** (disposable `TEST_REDIS_URL`/`TEST_REDIS_FIXTURE_ID`
fixture, never the shared dev Redis). `cargo fmt --check` and
`cargo clippy --all-targets -- -D warnings` both clean.

---

## 2. Booking fairness — CONFIRMED a real product capacity constraint, fixed

**Claim to check**: does the Payment-class global booking budget (previously
300/60s = 5/s, shared by every account combined) cause unnecessary failures
for independent legitimate Patients once aggregate demand exceeds 5
bookings/s — without blindly retuning, and while preserving financial
integrity, anti-abuse, and booking-conflict protections.

### 2.1 Method: not blind — a live, clean-metrics measurement

The project's own documented 1,200-active-session scenario
(`capacity_test.rs --mode=sessions`, 5-20s think time — the literal example
in that file's own module doc comment) was run twice against **Configuration
A rebuilt fresh from current source** (confirmed: BuildKit cache-hit on the
`COPY src` layer, which under content-addressed caching means the image's
compiled binary is bit-identical to what current `src/` produces — not a
stale binary; the running container was additionally restarted between runs
for a zero-residue metrics baseline, since `/metrics` counters are
cumulative since process start and the container had been serving earlier
8.0R sweep traffic for hours):

- Run 1 (240s, against the long-lived container): `POST /appointments/book`
  1,943 attempts, 1,195 succeeded, 5 genuine business rejects, **492
  rate_limited**, 251 5xx.
- Run 2 (150s, against a **freshly restarted, zero-residue** container, so
  `/metrics` is exactly attributable to this run alone): 1,299 attempts, 828
  succeeded, 7 genuine business rejects, **223 rate_limited**, 241 5xx.

Both runs independently show the same shape: booking attempts arrived at
**~8.2-8.7 req/s aggregate** (1,299 requests / 150s = 8.66/s; 1,943/240s =
8.10/s) from **over a thousand independent Patient accounts**, each
registered once and, within either run's duration, never remotely close to
their own separate, untouched `check_booking_limit` (10 bookings/hour/
account — the harness's own synthetic per-patient attempt counter stays
under 8 attempts across the whole run, i.e. under one every 20-30s per
patient, nowhere near 10/hour).

### 2.2 Exact root cause, from clean server-side metrics (run 2)

`GET /metrics` immediately after the clean run, filtered to `class="payment"`:

| Reason | Count | HTTP outcome | % of 1,299 attempts |
|---|---|---|---|
| `lane_concurrency` (admission concurrency semaphore full) | 240 | 503 (misclassified by the test harness as "system error", not policy) | 18.5% |
| `shared_class_budget` (Redis 300/60s ceiling) | 219 | 429 rate_limited | 16.9% |
| `lane_rate_limit` (local per-process 10/s bucket) | 4 | 429 rate_limited | 0.3% |
| accepted (passed admission) | 1,055 | proceeds to handler | — |

**240 + 219 + 4 = 463 of 1,299 attempts (35.6%) were rejected by
account-agnostic admission-layer policy ceilings — not by
`check_booking_limit`, not by a real slot conflict (genuine business
rejects were only 7, 0.5%).** This is the same signature already
established for the Normal-class bug in 8.0R: a fully account-agnostic
global ceiling binding below realistic, legitimate, distinct-account
aggregate demand. Two distinct mechanisms were both implicated, with the
**concurrency semaphore the larger of the two** (240 > 219) — something the
mathematical-demand argument alone would have missed, which is exactly why
this was measured live rather than inferred:

1. **Payment's admission-lane concurrency ceiling was `n.min(2)`** — unlike
   every other non-trivial class here (Auth/Upload at `n.min(4)`, Normal now
   at `n.min(64)` after 8.0R), Payment's concurrency cap never scaled with
   `pool_capacity` at all; it was a flat 2, platform-wide, regardless of
   deployment size. With booking's own measured p95/p99 latency (401ms /
   1,164ms in this run — heavier than Normal's cheap reads, since booking
   does real writes), Little's Law (concurrency ≈ rate × latency ≈ 8.5/s ×
   0.15-0.4s ≈ 1.3-3.4) already sits at or above a cap of 2 even before
   counting burst variance — this was never going to hold up under the
   project's own documented 1,200-session target.
2. **The Redis `shared_class_budget` (300/60s = 5/s)** is, independent of
   (1), below the measured ~8.2-8.7/s real aggregate demand by roughly
   60-75%.

### 2.3 Fix (evidence-based, not a blind retune)

`services/backend/src/admission.rs`:

| Mechanism | Before | After | Justification |
|---|---|---|---|
| Payment admission-lane concurrency | `n.min(2)` | `n.min(8)` | Matches the already-accepted Upload/Consultation shape (bounded, scales with deployment size); Little's Law against measured latency comfortably fits under 8 |
| Payment local per-process rate | 10/s | 20/s | Matches Webhook's own local rate exactly — Payment and Webhook are sequential steps of the same booking+payment-confirmation flow; keeps the local backstop at/above the new Redis budget so it isn't reintroduced as a tighter, un-benchmarked bottleneck |
| Payment Redis `shared_class_budget` | 300/60s (5/s) | 1200/60s (20/s) | Matches Webhook's own existing, already-accepted budget exactly (same flow, same number) — ~2.3-2.4x the measured ~8.2-8.7/s real demand, real headroom without being unbounded |

**Explicitly untouched** (per the reviewer's instruction to preserve these):
- `check_booking_limit` (10 bookings/hour/account) — the actual per-account
  anti-abuse/anti-fraud control. Unchanged.
- Booking-conflict detection (slot exclusivity, `schedule_exclusivity.rs`) —
  unchanged, a separate DB-transactional mechanism, not admission-layer.
- Payment webhook HMAC signature verification, idempotency-key handling,
  financial amount/escrow logic — unchanged, none of this is in
  `admission.rs`.
- Every other cost class (Auth/Upload/Expensive/Normal/Webhook/
  Consultation) — unchanged.

Unit test `admission::tests::expensive_flood_does_not_take_ordinary_slots`
updated to assert each sensitive class's *own* expected cap (was a blanket
`<= 4`, which Payment's new, legitimately-larger `8` would have failed) —
Auth=4, Upload=4, Payment=8, Expensive=2 — so a future regression in any one
class can no longer hide behind the others' margin. One stale comment in
`tests/api_tests.rs` (claiming Payment was "permanently capped at 2") was
also corrected; the test's own behavior (2 concurrent idempotency-race
requests) was never dependent on that specific number and is unaffected.

### 2.4 Post-fix live re-verification (Configuration A rebuilt and recreated)

Configuration A's backend was rebuilt (a genuine recompile this time —
confirmed in the build log, `cargo build --release` actually ran rather than
cache-hitting) and the container recreated (fresh `Created`/`StartedAt`,
zero-residue metrics), then the identical 1,200-session/150s scenario was
run a third time:

| Metric (`POST /appointments/book`) | Before fix (clean run) | After fix |
|---|---|---|
| Total attempts | 1,299 | 1,194 |
| Succeeded | 828 | 1,110 |
| `rate_limited` | 223 (17.2%) | **4 (0.34%)** |
| `admission_rejected_total{reason="shared_class_budget"}` | 219 | **0** |
| `admission_rejected_total{reason="lane_concurrency"}` | 240 (18.5%) | **78 (6.5%)** |

The Redis budget and local-rate ceilings are no longer binding at all (both
zero); the concurrency semaphore's share of rejections fell by roughly 3x
(not all the way to zero — see caveat below), consistent with a 4x larger
cap under bursty arrival timing. **The targeted problem — independent,
legitimate Patients failing to book for no reason but a shared,
account-agnostic ceiling — is resolved**, without touching
`check_booking_limit`, booking-conflict detection, or payment integrity.

**Caveat, disclosed rather than smoothed over**: this run's *overall*
`TRUE_SYSTEM_ERROR_RATE` (8.64%) was higher than the pre-fix clean run's
(6.73%) — but nearly the entire difference (+290 total 5xx) traces to
*other*, Normal-class endpoints in the first 10-second time-series bucket
(979 5xx this run vs. 680 in the pre-fix run — a +299 difference, almost
exactly matching the overall delta), not to the Payment class this fix
touched (Payment's own 5xx count *fell*, 241 to 78). This 10-second window
is the harness's un-staggered startup: all 1,200 session tasks are spawned
and dispatch their first action at the same instant, producing an
artificial thundering-herd burst whose severity visibly varies run to run
(disclosed as a pre-existing, not-fixed-here harness characteristic — see
§5). Regression evidence (§2.5) independently confirms the fix itself
introduces no new failure mode.

### 2.5 Full regression suite, before and after the fix

| Suite | Before (pre-fix baseline, same session) | After (with the Payment fix) |
|---|---|---|
| `cargo fmt --check` | n/a (not re-run standalone) | Clean |
| `cargo clippy --all-targets -- -D warnings` | n/a | Clean |
| `cargo test --lib` | n/a | **129 passed**, 0 failed (includes the updated `admission::tests::expensive_flood_does_not_take_ordinary_slots` with its new per-class exact-cap assertions) |
| `cargo test --test api_tests -- --test-threads=1` | **464 passed**, 0 failed, 4 ignored (1,411.58s) | **464 passed**, 0 failed, 4 ignored (1,470.01s) |

Identical pass/fail/ignored counts before and after — **zero regressions**
from the admission.rs change, across the full in-scope suite including
booking-conflict (`schedule_exclusivity.rs`), per-account anti-abuse
(`security_acceptance.rs`), and the idempotency-race test whose stale
comment was corrected (§2.3).

### 2.6 Classification: real product capacity constraint, now fixed and verified

This was a genuine capacity-planning gap — the Payment class's ceilings were
sized as a conservative placeholder (same category 8.0R identified and
fixed for Normal), never benchmarked against the project's own already-
documented 1,200-session concurrent target, not a deliberate
fraud-prevention design choice. The actual fraud/anti-abuse boundary for
booking has always been, and remains, `check_booking_limit` — a *per-account*
control, which this change does not touch in any way.

**Re-verified end-to-end after the fix**: §2.4 above documents exactly this
— a third live sweep against a genuinely-rebuilt backend, confirming
`rate_limited` fell from 17.2% to 0.34% and `lane_concurrency` fell from
18.5% to 6.5%, with an unchanged 464/0/4 full-regression pass count (§2.5).
This claim is not outstanding.

---

## 3. CPU measurement — sampler fixed and validated idle + active, app vs. DB separated

**Claim to check**: `scripts/sample_container_resources.ps1`'s CPU% column
was disclosed in 8.0R §7/§9 as `NOT_AVAILABLE` (implausible near-zero for
all three Configuration-A containers throughout an actively-served ~250 RPS
sweep) — fix and validate against real OS/cgroup counters, idle and active,
with application and PostgreSQL CPU reported separately.

### 3.1 Root cause

The old script called `docker stats <container> --no-stream` once per
container per tick. Each such call is a fresh one-shot request to the Docker
Engine's stats API; under load or frequent invocation the engine's own
internal "current" vs. "previous" cgroup reads can land close enough in time
that the computed delta is ~0 — a known class of issue with one-shot
`docker stats`, not a problem with the containers' real CPU usage.

### 3.2 Fix

Rewrote the script to read the cumulative CPU-usage cgroup counter directly
off each container every tick (`cpu.stat`'s `usage_usec`, cgroup v2 —
confirmed present on all three Configuration-A containers; falls back to the
cgroup v1 `cpuacct.usage` path if absent) and compute CPU% itself from the
delta between consecutive ticks (seconds apart — never the one-shot
millisecond-scale ambiguity) divided by the real elapsed wall-clock time and
the container's own visible CPU count (read once from `cpu.max`'s quota/
period when a hard quota is set, otherwise from `nproc`, which correctly
reflects Configuration A's `cpuset:` pinning — verified 4 inside
backend/redis, 2 inside postgres). Memory is read the same direct way
(`memory.current`/`memory.max`). The script never execs a mutating command.
Per-container rows plus an `APPLICATION_TIER` (backend+redis) vs.
`POSTGRESQL_TIER` summary are both printed, satisfying "record application
and PostgreSQL CPU separately."

### 3.3 Validation — idle and active, cross-checked

| Condition | Container(s) | Result |
|---|---|---|
| Idle (genuinely idle long-lived dev `hellodoctor-postgres`/`hellodoctor-redis` containers, not under any test load) | postgres, redis | 0.21%, 0.18% avg — plausible near-idle |
| Active (Configuration A, during the live 1,200-session sweep, run 1) | confA backend/postgres/redis | backend 8.36%, **postgres 68.25%**, redis 1.88% |
| Active (Configuration A, during the live 1,200-session sweep, run 2, clean restart) | confA backend/postgres/redis | backend 6.48%, **postgres 35.04%**, redis 1.52% |

The old script returned near-zero for all three containers in an equivalent
actively-served scenario (8.0R §7); the rewritten script now shows real,
plausible, non-zero load that scales up under genuine traffic and down at
idle — the defining property the old script lacked. Postgres's materially
higher CPU share than the backend under this workload (even on half as many
cores) is itself a useful, previously-invisible-due-to-the-bug signal for
where the real capacity ceiling in this stack is likely to be as load
increases further — worth carrying into the rest of Milestone 8.0's capacity
conclusions, not asserted further here since that is outside this item's
scope.

---

## 5. Honest status — what is and is not verified as of this document

- **Items 1, 2, and 3 are all now fully verified**: item 1 with passing
  automated tests; item 2 with a live before/after measurement on a
  genuinely-rebuilt Configuration A plus an unchanged full-regression pass
  count (§2.4-2.5); item 3 with cross-checked idle/active validation on real
  containers, application and PostgreSQL CPU reported separately.
- **Known, disclosed, NOT fixed in this pass** (out of the three items'
  scope, and fixing it would be the kind of harness refactor the reviewer
  asked not to start unprompted): `capacity_test.rs`'s "sessions" mode
  spawns all `CAPACITY_ACTIVE_SESSIONS` session tasks and dispatches each
  one's first action simultaneously at t=0, with no startup stagger. This
  produces a real, visibly run-to-run-variable thundering-herd burst in the
  first ~10 seconds of every sweep (680-979 5xx observed across three runs
  in this pass, over an order of magnitude above the steady-state per-window
  rate) that is a test-harness artifact, not a property of organic user
  ramp-up, and inflates the headline `TRUE_SYSTEM_ERROR_RATE` figure in a
  way unrelated to whatever is actually being measured in that run. Anyone
  using this harness's aggregate error-rate numbers for a go/no-go capacity
  conclusion should exclude or separately report the first time-series
  bucket, or stagger session start times, before trusting that number at
  face value. Flagged per "ensure the 1,200-session scenario accurately
  reports... successful end-to-end business outcomes" — this is exactly the
  kind of measurement-fidelity gap that instruction was pointing at, even
  though remediating it was not itself one of the three requested items.
- Also observed, not investigated further here: Postgres's CPU share was
  consistently and substantially higher than the backend's in every
  Configuration-A sweep in this pass (35-68% on 2 cores vs. 6-8% on 4 cores
  for the backend) — now visible for the first time because of the item-3
  fix. This is a real signal about where Configuration A's next capacity
  ceiling likely sits as load increases further, worth carrying into the
  rest of Milestone 8.0's capacity conclusions.
- This document does not re-open or re-conclude the rest of Milestone 8.0
  (CPU/DB saturation point beyond the observation above, long soaks,
  Configuration B) — those remain separately in progress per the reviewer's
  own instruction not to restart broad scope.
