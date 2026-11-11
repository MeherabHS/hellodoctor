# Milestone 8.0 — Database Performance Remediation: chat_messages RLS Fix
# (Before / After, Verified)

Date: 2026-10-31. Scope: the narrowly-authorized fix for the
`chat_conversations`/`chat_messages` RLS bottleneck identified in
`docs/MILESTONE_8_0_PERFORMANCE_CHARACTERIZATION.md` §3. No other
performance work, no security-audit restart, no repository restructuring.
One new migration pair; no previously-applied migration was modified or
renamed; no Git staging/commit/push; all existing databases/volumes
preserved (Configuration A and B were restarted/rebuilt in place, never
recreated from scratch or volume-wiped).

**Correction applied: a query-shape fix, not new indexes.** Per the
technical caution received before starting, indexes were investigated
first and proven — empirically, via `EXPLAIN (ANALYZE, BUFFERS)` against
the real `hellodoctor_api` role and real data, not assumed — insufficient:
the offending predicate wraps `patient_id` inside a function call
(`fn_patient_profile_owned_by_caller(patient_id, caller)`), which a plain
btree index cannot accelerate. The actual fix needed no new index: one RLS
policy (`chat_messages_insert`) used an **uncorrelated** subquery shape
forcing a full-table scan, where its own sibling policies
(`chat_sender_guard`, `chat_messages_access`) already used a **correlated**
`EXISTS` shape that lets the existing primary-key index do the work. The
fix rewrites `chat_messages_insert` to match its siblings' already-correct
shape — logically identical semantics, proven in §1, not a weakening.

---

## 1. Step 1 — Established the exact query plan before changing anything

Ran against Configuration A's real database, as the real restricted
`hellodoctor_api` role (via `SET ROLE`), with the real session GUCs
(`app.current_user_id`, `app.current_role`) the application sets, using a
real seeded conversation (not synthetic/fabricated), wrapped in
`BEGIN; ... ROLLBACK;` so nothing was mutated:

| Query shape | Plan | Execution time |
|---|---|---|
| `chat_messages_insert`'s original `conversation_id IN (SELECT id FROM chat_conversations WHERE <ownership-or-doctor via function call> AND ...)` | **Seq Scan on chat_conversations**, 22,910 rows scanned, 22,895 removed by filter | **476-480ms** |
| `chat_sender_guard`'s `EXISTS (SELECT 1 FROM chat_conversations c WHERE c.id = conversation_id AND ...)` (unchanged, already correct) | **Index Scan using chat_conversations_pkey** | 0.379ms |
| The real `INSERT INTO chat_messages ...` (full RLS stack, rolled back) | SubPlan 4 = the same Seq Scan, dominating total cost | 487.5ms execution |

**On the index question specifically**: both `chat_conversations.doctor_id
IN (SELECT ...)` sub-branches already resolve as hashed, hashed-set
membership tests (not bottlenecks); the bottleneck is squarely the
`fn_patient_profile_owned_by_caller(patient_id, caller)` function-call
predicate, which cannot be made sargable by indexing `patient_id` alone —
confirmed by inspecting the actual filter condition in the `Seq Scan` plan
node, not inferred. A plain index on `patient_id`/`doctor_id` was not
applied, because the evidence shows it would not have changed this plan.

**Proposed fix, verified BEFORE being written into a migration**: the same
predicate, restructured as a correlated `EXISTS` (identical to the sibling
policies' existing shape), measured via the same `EXPLAIN (ANALYZE,
BUFFERS)` method: **1.106ms**, using `Index Scan using
chat_conversations_pkey` — a ~430x reduction, with the ownership function
now evaluated exactly once (on the one matched row) instead of once per
scanned row.

A note on measurement accuracy, per the caution received: `pg_stat_statements`
reports **execution time**, not CPU time directly. The conclusion that this
query was the dominant *CPU* cost rests on the combination of (a) its 48.9%
share of total statement execution time, (b) PostgreSQL CPU utilization
independently measured via cgroup counters climbing to 90-99% during the
exact window this query was dominant, and (c) `pg_waiting_locks` holding at
0 throughout (ruling out lock-wait time inflating the execution-time figure
without corresponding CPU use). The three together are what support "CPU
saturation caused by this query," not the execution-time figure alone.

## 2. Step 2 — Smallest effective correction, implemented as a migration

`migrations/20261202000003_chat_messages_insert_rls_performance.{up,down}.sql`:
drops and recreates `chat_messages_insert` with the correlated-`EXISTS`
shape. No index added (proven unnecessary, §1). No other policy touched.
No previously-applied migration file modified.

**Forward/down/forward round-trip tested** on a disposable, throwaway
Postgres 16 container (never the shared dev or benchmark databases):
`sqlx migrate run` (all 64 migrations including the new one apply cleanly)
→ `sqlx migrate revert` (confirmed the policy reverts to the exact
byte-identical prior shape) → `sqlx migrate run` again (confirmed the fixed
shape is restored). Container discarded after.

## 3. Step 3 — Security and functionality verification

**Focused suite** (cross-account denial, chat-sender spoofing, BOLA for
both Patient and Doctor, SQL injection via the conversation-path parameter,
booking/payment/chat shared-history ownership, consultation identity, chat
expiry/locking, chat concurrent-replay idempotency), run against the
disposable `TEST_REDIS_URL`/`TEST_REDIS_FIXTURE_ID` fixture with the new
migration applied:

```
test_identity_wrong_doctor_writes_and_chat_sender_spoofing_denied ... ok
test_bola_doctor_cannot_access_another_doctors_chat ... ok
test_24h_clinical_chat_messaging_and_expiration_locking ... ok
test_chat_message_send_is_rate_limited ... ok
test_bola_patient_cannot_post_into_another_patients_chat ... ok
test_sql_injection_chat_conversation_path_param_and_message_body ... ok
test_consultation_chat_booking_payment_shared_history_and_ownership ... ok
test_consultation_video_retains_identity_and_separate_chat_booking ... ok
test_chat_booking_rejects_started_slots_without_financial_effect ... ok
test_milestone_chat_concurrent_replay_distinct_keys_and_privacy ... ok
test_milestone_chat_expiry_wall_clock_and_historical_reads ... ok

test result: ok. 11 passed; 0 failed
```
All exercised, with the new policy shape live: a Patient/Doctor cannot read
or write another account's chat; cross-account (BOLA) attempts correctly
get 403; sender-identity spoofing is denied; SQL injection payloads in the
path parameter are rejected (400) before ever reaching the policy; a locked
or expired conversation still rejects new messages; a Doctor's historical
read access to a completed consultation's chat survives
professional-verification revocation (unaffected policy); idempotent
concurrent sends still behave correctly.

**Full backend regression suite**, same migration state:
```
cargo test --test api_tests -- --test-threads=1
test result: ok. 464 passed; 0 failed; 4 ignored (1,493.79s)
```
Identical to the pre-fix baseline (464 passed, 0 failed, 4 ignored) —
**zero regressions**. `cargo fmt --check` and
`cargo clippy --all-targets -- -D warnings` both clean (no Rust source was
touched; only migration SQL).

## 4. Step 4 — Configuration A, before vs. after (same REALISTIC_RAMP profile: 1,200 sessions, 120s ramp, 300s duration)

Both runs: Configuration A rebuilt from current source immediately before
(confirmed genuine recompiles, not cache hits), container recreated for a
zero-residue metrics baseline, `pg_stat_statements_reset()` immediately
before each run.

| Metric | Before | After | Change |
|---|---|---|---|
| **TRUE_SYSTEM_ERROR_RATE** | 5.07% | **0.10%** | ~50x reduction |
| 5xx per 10s window, steady-state | 25 → 118 (growing) | 0-2 (flat) | growth trend eliminated |
| **PostgreSQL CPU, steady-state** | 90-99% (saturated) | **15-18%** (headroom) | ~6x lower |
| Backend CPU, steady-state | 7-8% | 8-9% | unchanged (never the bottleneck) |
| `chat_messages` INSERT, mean latency | 916.77ms | **0.61ms** | ~1,500x |
| `fn_patient_profile_owned_by_caller`-driven scan cost | 43.1% of all Postgres execution time (5.7M calls) | not in top 10 (effectively eliminated) | — |
| Consultation-class `lane_concurrency` rejections | 1,227 of 2,746 (44.7%) | **0 of 2,769 (0%)** | eliminated — confirms this was a downstream symptom of the latency bug, not an independent sizing problem (per the explicit instruction not to touch this limit unless proven necessary: **it was not necessary**) |
| `POST chat message` success rate | 55.2% (660/1,196) | **100%** (1,209/1,209) | fixed |
| `POST chat message` **p50 / p95 / p99** | 506-515ms / 1,027-1,272ms / 1,212-1,400ms | **11ms / 18ms / 48ms** | ~45x / ~60x / ~27x |
| Overall **p50 / p95 / p99** (time-series, steady-state) | 6-7ms / 50-77ms / 650-1,293ms | **5-6ms / 19-24ms / 26-55ms** | p99 tail ~15-25x tighter |
| `POST /appointments/book` **p50 / p95 / p99** | 25ms / 57ms / 86ms | 15ms / 28ms / 62ms | improved further (less DB contention platform-wide) |
| `END_TO_END_BOOKING_SUCCESS` | 97.0-97.8% | **98.8%** | improved |
| Payment-class rejections | 0% (already fixed, prior report) | 0% | unchanged, confirms no interaction |
| Backend RAM | ~25MB, stable | ~24MB, stable | unchanged |
| PostgreSQL RAM | growing to ~650MB | growing to ~630MB | unchanged (not a memory issue either way) |

Every number above is from a clean, zero-residue, freshly-reset
measurement window specific to its own run — not blended with any other
run's history.

## 5. Scope discipline confirmed

- **Consultation admission limits were not touched.** The post-fix
  measurement proves they did not need to be: 0% rejection at higher
  Consultation-class throughput (2,769 vs. 2,746 accepted-plus-rejected)
  than the pre-fix run. Exactly the outcome the "do not change Consultation
  admission limits unless the post-fix measurements prove another
  adjustment is necessary" instruction anticipated — the measurements prove
  the opposite.
- **A due-diligence check** (read-only; no changes) confirmed the same
  uncorrelated-subquery anti-pattern does not recur elsewhere:
  `chat_messages_insert` was the **only** policy, among every policy
  referencing `fn_patient_profile_owned_by_caller`, using the vulnerable
  shape. The fix is complete, not partial.
- **Configuration B** was rebuilt and migrated identically (confirmed
  `database_latest_migration=20261202000003` on both stacks) so it is not
  running stale code, but a full repeat 1,200-session sweep was not run
  against it in this pass — Step 4 named Configuration A specifically, and
  the root-cause mechanism (a role/data-shape-dependent query plan, not a
  topology-dependent one) applies identically regardless of CPU allocation;
  this is a reasoned inference stated as such, not re-measured, to respect
  the requested scope.
- No schema/migration change beyond the one authorized policy rewrite. No
  other RLS policy, table, or index was modified. No Git staging, commit,
  or push. No existing database, volume, or container was destroyed (only
  restarted/rebuilt in place).

## 6. Status

This specific, narrowly-scoped remediation is **complete and verified**:
root-caused with live evidence before any change, fixed with the smallest
effective correction (a query-shape rewrite, no new index), verified secure
and functionally correct (focused + full regression suites, both clean),
and verified effective with a controlled before/after measurement on
Configuration A showing the predicted improvement across every requested
dimension. Stopping here for review, as requested.

**Not claimed**: that Milestone 8.0 overall is GREEN. This fix removed the
single largest identified bottleneck in Configuration A under the
1,200-session REALISTIC_RAMP profile; whether the system's current combined
performance meets the full set of Milestone 8.0 acceptance criteria across
Configuration B, longer soaks, and the planned 4-vCPU/8-GiB production
shape remains to be separately assessed in the continuation work.
