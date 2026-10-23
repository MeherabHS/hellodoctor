# Resilience producer remediation — 20 October 2026

This is a focused source correction and offline unit verification, not a new HTTP attack result. Original evidence remains in `../post_implementation_runtime_20261019/`. No application admission settings, database pools or security controls were changed by this work.

## Confirmed defects and corrections

**RES-001 — Transport evidence was discarded.** `attack-local.py` previously caught `OSError`/`HTTPException` and retained only `transport_error=true`. It could not distinguish connect refusal, reset, timeout, EOF or client socket resource exhaustion. The producer now retains exception class, numeric errno/Windows error, request phase and a bounded taxonomy. It deliberately excludes exception messages, response bodies, request bodies, authorization and token values. Request-relative start times support correlation with separately collected metrics. A reset alone is never labeled a server-resource failure.

**RES-002 — HTTP throughput was mislabeled.** `responses_per_second` counted all completed attempts, including errors. New explicit attempt, received-header and completed-HTTP counts/rates separate these meanings. Completed-HTTP percentiles exclude transport errors. The old field is retained for compatibility with an explicit legacy-semantics label. A status received before a body read fails is preserved and separately marked incomplete.

**RES-003 — Scheduler regression coverage was missing.** The same bounded replacement strategy was extracted into a testable helper; it still submits at most one outstanding future per worker. Deterministic tests exercise all 30,000 logical indices exactly once and prove the outstanding-future peak remains 32. Worker exceptions continue propagating. Original 30,000-request, 32-worker, two-second socket, 128 KiB request and 64 KiB response ceilings and literal-loopback/synthetic guards are preserved. No retries were added.

Affected source: `services/backend/scripts/resilience/attack-local.py`. New test file: `services/backend/scripts/resilience/test_attack_local.py`. Before/after hashes and patch are retained beside this note.

## Executed offline evidence

Command, repository working directory:

```text
C:/Users/CUBE/AppData/Local/Python/bin/python.exe -I -B services/backend/scripts/resilience/test_attack_local.py
```

The owner-authorized local runner clears its child environment, uses scratch home/temp and a 30-second deadline. HTTP connections are mocked; no sockets, backend, database, Redis or provider are contacted. This is ordinary local verification, not formal sandbox certification.

First execution: 20 named tests, 19 passed, one failed. The new classifier initially treated `RemoteDisconnected` as a generic reset because Python's HTTP exception also subclasses `ConnectionResetError`. The implementation was corrected to prefer the stronger HTTP EOF observation. The same strict test was retained. Second execution: **20 passed, zero failed, zero skipped**, exit 0, 0.3614671 seconds including runner overhead. See `resilience-unit.*` and `resilience-unit-corrected.*`. Counts must not be added twice.

## Historical 19,750 failures: what the records establish

`resilience-historical-analysis.json` independently reanalyzes retained JSON and includes source hashes:

- All 30,000 indices are unique. Received statuses: 31 × 401, 145 × 429, 10,074 × 503; 19,750 attempts have no status.
- The independent admission counter deltas are **87 accepted + 29,913 rejected for budget = 30,000** for Auth. The additional limiter-unavailable counter is a later rejection of an already-admitted request and must not be added as a new incoming request. Under the dedicated-fixture workload these counts reconcile collectively with the producer; they do not associate an individual failed socket with an admission outcome.
- **19,731 failures completed in under 100 ms**. Error latency: min 1.710 ms, p50 7.117 ms, p95 11.395 ms, p99 14.123 ms, max 2024.827 ms. Only 19 lasted at least 1900 ms. Therefore the records do not support describing all failures as two-second socket timeouts or as failures entirely before admission.
- Missing fields include exception type, errno, socket phase and per-attempt start time. Neither resets, EOF, resource exhaustion nor server culpability can be reconstructed from the Boolean and latency alone. No exact OS/network cause is asserted.
- The backend HTTP listener was directly bound on host loopback. The BusyBox forwarder was on the database/Redis dependency path, not an HTTP reverse proxy. Its separately demonstrated listener failure is relevant fixture context, but is not proof that it caused these 19,750 HTTP failures.
- Each attempt created a fresh TCP connection; no keepalive pool was used. Host socket churn, early HTTP connection termination, unread-body handling, scheduling pressure and dependency latency remain hypotheses needing instrumentation, not confirmed defects. No admission/pool limit was increased to obscure them.

`TRANSPORT_ERROR_ROOT_CAUSE=INCONCLUSIVE` with narrowed historical evidence; `SERVER_TRANSPORT_DEFECTS=INCONCLUSIVE`. Actual backend counters establish pressure control activity, not universal successful HTTP responses.

## Why new adversarial HTTP stages were not executed

The current owner instruction requires the security skill's full OS isolation for adversarial workloads. Read-only environment checks show Docker uses Linux x86_64; cached images are PostgreSQL 16 and Redis 7 only; the installed Rust target is Windows GNU only. No available cached Linux-compatible actual HelloDoctor backend/toolchain plus Python harness has been established for a no-external-network namespace. A Windows Job Object limits resources but does not provide the required network/filesystem isolation. Ordinary local execution authorization does not override this campaign's separate adversarial-isolation requirement.

No image was pulled, dependency installed, firewall changed, shared process contacted or new host attack run. Additional 100/1,000/5,000/10,000/30,000-request stages, authorized clinical-load probes, full pool exhaustion and upload/callback flood acceptance remain **BLOCKED_ENVIRONMENT** here. Historical results are not promoted to new passes.

The smallest prerequisite is an operator-provisioned cached offline Linux fixture toolchain or image capable of running the actual backend and Python producer alongside synthetic PG/Redis in an OS-isolated loopback namespace. Use read-only source/toolchain mounts, scratch-only writable storage, allowlisted empty child environments, explicit CPU/RAM/process/file/disk/wall limits and no external network. Do not expose Docker sockets or shared fixture endpoints to target-controlled code. Follow the skill's promotion requirements for formal artifacts.

Before each bounded stage, record producer concurrency separately from logical identities, independent authorized clinical/profile probes, stage start/end counters, socket/connection counts, database allocations/waiters, Redis latency, CPU/RSS, all exception categories and unexpected statuses. Predetermine latency, success-rate and resource thresholds for the actual fixture before load begins; stop at those resource ceilings. Preserve 429/503 as explicit shedding outcomes and transport failures as failures requiring attribution. Missing telemetry remains NOT_MEASURED. A producer unit-test pass does not establish Level D acceptance.
