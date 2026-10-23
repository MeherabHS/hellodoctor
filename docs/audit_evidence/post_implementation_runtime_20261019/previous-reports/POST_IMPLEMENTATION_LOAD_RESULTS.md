# Post-implementation load results — 19 October 2026

**No HTTP/load scenario was executed. All scenarios below are BLOCKED_ENVIRONMENT, not DEFENSE_PASS.** No production/shared service, external provider, real OTP recipient or connected device was targeted.

The current Docker daemon is unreachable; the campaign was not authorized to start it or provision another sandbox. See [isolation evidence](audit_evidence/post_implementation_20261019/isolation-probe.json) and [safe continuation procedure](POST_IMPLEMENTATION_REGRESSION_RESULTS.md#safe-continuation-procedure). Host loopback alone is not isolation.

## Scenario coverage and missing prerequisites

| Scenario | Planned synthetic workload / observation | Existing harness support | Current outcome |
| --- | --- | --- | --- |
| A: distributed authentication | Up to 30,000 logical addresses targeting one synthetic account; Patient/Doctor/Admin separately; independent ordinary/consultation reads | Bounded producer, unique logical forwarded IPs, response/latency records. Requires fixture-owned trusted ingress; Admin requires BFF challenge/assertion fixture | BLOCKED_ENVIRONMENT |
| B: many-account authentication | Distinct `.invalid` email targets; observe actual auth budgets, DB acquisitions and hashing rather than only responses | `--accounts different` on email `/login` cases | BLOCKED_ENVIRONMENT |
| C: expensive endpoint isolation | Synthetic investigation/read/processing work alongside separate authenticated ordinary and consultation probes | Single-case producer exists; concurrent probe/orchestration and independent resource collection still needed | BLOCKED_ENVIRONMENT |
| D: uploads | Small valid/invalid authorized fixture uploads; multiple accounts and one-account races; inspect sanitizer peak, per-account peak, quota, temporary cleanup and DB effects | Producer currently JSON-only; no multipart builder or cleanup observer | BLOCKED_ENVIRONMENT; upload harness support incomplete |
| E: payment initiation | Same/different/no keys against one fresh appointment; observe current obligation and provider state | Key variation supported; checkout adapter is absent | BLOCKED_ENVIRONMENT; provider recovery INCONCLUSIVE_PROVIDER_NOT_INTEGRATED |
| F: invalid webhooks | Invalid synthetic callbacks while probing ordinary/consultation traffic; signed fixture events separately | Producer can send payload/header fixtures, but generates no provider signatures. Production callback handler currently refuses absent real verification | BLOCKED_ENVIRONMENT |
| G: Redis dependency loss | Interrupt only disposable fixture Redis; observe local bounds, fail-closed admissions, readiness and coalesced incidents | Requires fixture-owned fault control and independent metrics; no such execution occurred | BLOCKED_ENVIRONMENT |
| H: DB pool pressure | Hold fixture-owned connections; observe pool idle/acquisition deadlines/rejection latency and readiness | Pool metrics exist; holder/sampler/waiter observation absent from current producer | BLOCKED_ENVIRONMENT |
| I: recovery stampede | Deterministic 1,000-client unit distribution, timer coalescing/cancellation/supersession/backoff/manual refresh; full-stack recovery separately | Seven authored tests per web app, three per Flutter app; no test bodies executed | BLOCKED_ENVIRONMENT |
| J: mixed workload | Explicit synthetic blend listed below, in bounded stages, with independently sampled legitimate outcomes | Existing single-case producer lacks mixed-workload scheduler and resource/fault orchestrator | BLOCKED_ENVIRONMENT; orchestration incomplete |

These distinctions prevent “the harness exists” from being reported as a completed load test.

## Measurements

For **each** scenario A–J, every field below is `NOT_MEASURED`. The machine-readable [load-scenarios.json](audit_evidence/post_implementation_20261019/load-scenarios.json) contains each field for every scenario, with null numeric values and explicit measurement status.

```text
offered_rps
achieved_rps
admitted_rps
rejected_rps
http_429
http_503
http_5xx_unexpected
p50_ms
p95_ms
p99_ms
cpu_peak
rss_peak
redis_operation_count
db_pool_size
db_pool_idle
db_acquire_deadline_count
db_waiter_peak_if_observable
argon2_active_workers_peak
sanitizer_active_workers_peak
consultation_success_rate
ordinary_success_rate
webhook_validation_results
```

No legitimate traffic survival rate, worker peak, DB/Redis health result or capacity threshold was measured. The previous 400 RPS benchmark is not used as evidence. The historical 6.615-second SQL fixture elapsed time is not HTTP latency, offered RPS or a backend capacity test.

## What the producer and observer actually do

`services/backend/scripts/resilience/attack-local.py` accepts explicit synthetic manifests and literal HTTP loopback origins only, at most 30,000 requests/identities and 32 worker connections. Outstanding futures are bounded. Each connection has a two-second socket timeout, request bodies at most 128 KiB, and bounded response reading. It records status/latency/request IDs without token/body values. The output verdict is `OBSERVER_REQUIRED`.

The producer is closed-loop, so “responses_per_second” is achieved completion rate, not independently scheduled offered load or server admission. Thirty thousand logical identities means thirty thousand request identities over time, not thirty thousand concurrent sockets or real interfaces. Arbitrary forwarded identities are not trusted in production. A disposable ingress must be the exact approved proxy peer before the model tests account/global controls rather than remeasuring one loopback IP limit. Production identity handling must not be modified for tests.

Missing producer capabilities: fixed offered-RPS scheduling, mixed workload orchestration, independent legitimate probes, CPU/RSS and Redis-operation collection, DB waiter sampling, outage/response-loss injection, multipart upload construction, device proof generation and provider signing. No new load framework or infrastructure was introduced in this campaign to hide those gaps.

`observe-local.py` independently grades supplied committed DB/metric snapshots for a limited set of cases. Missing peaks/provider state/ordinary latency yields INCONCLUSIVE. Snapshot matching is not authentication of the collector; fixture ownership, collection commands and source identity must be recorded independently. The observer has no built-in implementation for every new A–J acceptance condition. Do not accept an attacker-generated verdict or a set of 503s as proof of economic idempotency.

## Source-backed limits to measure

At the default ten connections per role: ordinary aggregate concurrency 20; Normal 8, Auth 4, Upload 4, Payment 2, Expensive 2; separate Consultation 4, Webhook 2 and Health 2. Combined handler admission ceiling 28. Each process owns its local ceiling; Redis budgets are shared. These are configured values.

Four Argon2 workers and four sanitizer workers use no-wait permits held until blocking completion. Per-account concurrent uploads are two. Common authed transactions have a 250ms acquisition deadline; some direct-pool operations retain the pool's five-second acquisition deadline. Sampling pool availability does not reserve a connection. Health readiness now has separate DB and Redis checks with 250ms bounds each; it must not be used for aggressive restart liveness during dependency outages.

Invalid callbacks can consume both webhook permits and the shared callback budget before signatures are checked. Their dedicated lane isolates ordinary admission, not an authenticated provider's guaranteed access. A real provider can still be denied during an invalid flood. Edge controls, bounded validation, provider retries and authoritative reconciliation are independent acceptance requirements.

All Admin routes currently share Expensive admission. There is no dedicated Admin recovery lane and no server-side query single-flight. These are documented limitations, not evidence of an actual consultation outage. The corrected finite method labels need a runtime metric-cardinality check; the added pure test does not measure RSS under traffic.

## Safe reproduction after an isolated stack is supplied

Run these commands **inside the prepared OS-isolated fixture namespace**, from `services/backend/scripts/resilience`. They are not instructions to point a host process at a shared local backend. The fixture owner must provide the backend/BFF/PG/Redis, approved logical-IP ingress, synthetic accounts/providers, independent observer snapshots and all resource limits first.

Example non-secret manifest for one invalid-password case:

```json
{"synthetic_fixture":true,"base_url":"http://127.0.0.1:18080","cases":{"distributed_login":{"path":"/api/v1/auth/patient/login","body":{"email":"synthetic@example.invalid","password":"fixture-invalid-password"}}}}
```

Staged existing producer commands, each with a fresh exclusive output filename:

```sh
python -I -S attack-local.py --manifest fixture.json --case distributed_login --count 100 --workers 4 --output attack-stage1.json
python -I -S attack-local.py --manifest fixture.json --case distributed_login --count 1000 --workers 8 --output attack-stage2.json
python -I -S attack-local.py --manifest fixture.json --case distributed_login --count 30000 --workers 32 --output attack-stage3.json
python -I -S attack-local.py --manifest fixture.json --case distributed_login --count 30000 --workers 32 --accounts different --output attack-many-account.json
python -I -S observe-local.py --before before.json --after after.json --attack attack-stage3.json --output observer-stage3.json
```

These vary closed-loop concurrency/count, not offered RPS. Stop on the fixture resource envelope or unexpected failure; do not increase pools or limits merely to pass. Use fresh synthetic obligations for payment key variants (`--keys same`, `different`, `none`). A refused checkout is provider-unavailable, never a provider-idempotency pass. Admin cannot be tested by silently bypassing required BFF proof.

The exact existing SQL-only command, from repository root, is:

```powershell
./services/backend/scripts/resilience/Invoke-IsolatedSql.ps1
```

It requires the already-running permitted daemon and cached `postgres:16`; `--pull never`, no network/published port, read-only source/root, clean environment and explicit resource/wall limits are implemented by that runner. Do not start Docker Desktop to make this command work under the current authorization. This command does not run HTTP scenarios, SQLx test binaries, auth workers or Android code.

## Synthetic mixed-workload design, not a production model

If the fixture gains a bounded coordinator, use an initial **chosen synthetic** mix of 40% ordinary authenticated reads, 20% consultation summaries, 10% Doctor chat-list reads, 10% invalid-password authentication, 5% payment initiation, 5% small authorized uploads and 10% recovery-triggered reads. The purpose is to exercise normal/reserved/auth/payment/upload lanes together; there is no evidence that these percentages describe production usage.

Use actual routes from `api/router.rs`, such as `/api/v1/patients/me/appointments`, `/api/v1/doctor/appointments`, `/api/v1/doctor/chats`, `/api/v1/appointments/<fixture-id>/consultation` and the corresponding payment/upload routes. Use synthetic identities authorized for each object. The existing single-case producer cannot express this full mix by itself. Record actual fixture resource profile, duration, offered schedule and role/lane distribution before any future run.

## Pass criteria to define before runtime

Hard invariants: no admission semaphore waiting queue; worker peaks at most four; per-account uploads at most two; no duplicate economic effect per tested identity; controlled denials with safe errors; invalid callbacks never create economic effects; no per-rejection DB error-write explosion. Verify actual blocking-worker lifetime during cancellation.

Latency, accepted RPS, ordinary/consultation success targets and acceptable measured DB waiter count must be selected for the actual fixture/hardware before grading; none was selected from unmeasured capacity. DB availability cannot be inferred from admission permits. Missing telemetry remains NOT_MEASURED/INCONCLUSIVE even when all hard counters appear bounded.
