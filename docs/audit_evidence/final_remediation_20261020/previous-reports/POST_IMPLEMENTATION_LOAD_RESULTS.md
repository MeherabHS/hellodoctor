# Post Implementation Load Results — 20 October 2026

Updated from **executed local development verification**, 19–20 October 2026. This is not formal sandbox certification or production acceptance. The owner authorized local toolchains/disposable services and froze original application source, tests, assertions, configuration and the security skill. Only new fixture/evidence files and these reports were changed.

Branch `main`; commit `dca9b3c5fcdb6303d31abbb77d3a1fd4ac1f879f`; dirty working tree on entry. [Entry baseline](audit_evidence/post_implementation_runtime_20261019/baseline.json) and [final preservation check](audit_evidence/post_implementation_runtime_20261019/source-freeze-final.json): **539 original files checked, zero changes, skill unchanged**. Web and Flutter manifests independently checked 132 and 425 original files (overlapping inventories, not additive). Earlier reports remain in [the historical archive](audit_evidence/post_implementation_runtime_20261019/previous-reports/POST_IMPLEMENTATION_LOAD_RESULTS.md); earlier source corrections are not new fixes in this frozen campaign.

## Fixture and interpretation

Only new resources labelled `hellodoctor.acceptance.run=runtime-20261019` were used. PG16/Redis7 cached images had read-only roots, tmpfs data, no swap and finite CPU/RAM/PID budgets. The standalone SQL and OTP fixtures used `--network none`. The HTTP fixture used internal-only PG/Redis plus a trusted dual-network fixed-destination forwarder exposing **127.0.0.1:15432/16379 only**; this composite is not a network-none sandbox. Actual Rust binary listened on 127.0.0.1:18080 under a Windows Job Object: 512 MiB memory, 32 processes, 20% host CPU rate policy, 300s wall limit and kill-on-close. Synthetic identities, no production env files/provider endpoints, and clean allowlisted process environments were used.

Host identification: 12 logical CPUs / 23.92 GiB RAM; Docker VM reports 12 logical CPUs / approximately 11.64 GiB RAM. PG budget 1 CPU / 512 MiB; Redis 1 CPU / 128 MiB; forwarder 1 CPU / 128 MiB. These are fixture conditions, not target Azure production capacity. Short CPU samples are expressed relative to one logical core; 100% is one core. Short samples may exceed the job's longer-interval CPU quota. Process RSS is not host/VM/container total memory.

Attacker: unchanged bounded `attack-local.py`; Defender: fresh actual Rust backend; Observer: separate process metrics/SQL/Redis reads and unchanged `observe-local.py`. Logical source addresses were modeled through a fixture-owned trusted loopback ingress. No production header trust/bypass was added. At most 32 producer connections were active; 30,000 identities were not 30,000 concurrent connections. No real OTP, payment, storage or other provider call was requested.

## Staged authentication workload

All role calls used invalid synthetic credentials. Same-account stages hold the target constant across logical IPs; different-account stages generate `.invalid` addresses. Admin development-route pressure is **not** production BFF/Turnstile/HMAC acceptance. The sequence was a capacity-limited closed-loop test, not a measured open-loop offered-RPS schedule.

| Stage | Realm / targets | Attempts / workers | 401 | 429 | 503 | Transport errors | Completed attempts/s | HTTP responses/s | All-attempt p50/p95/p99 ms |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | patient / same | 100 / 4 | 5 | 95 | 0 | 0 | 54.24 | 54.24 | 1.236 / 34.613 / 1313.383 |
| 2 | patient / different | 1000 / 8 | 8 | 0 | 991 | 1 | 559.75 | 559.19 | 1.259 / 24.069 / 277.363 |
| 3 | patient / different | 30000 / 32 | 31 | 145 | 10074 | 19750 | 1265.40 | 432.34 | 7.029 / 14.215 / 365.773 |
| 4 | doctor / same | 100 / 4 | 5 | 95 | 0 | 0 | 84.39 | 84.39 | 1.294 / 35.386 / 700.933 |
| 5 | admin / same | 100 / 4 | 5 | 95 | 0 | 0 | 88.37 | 88.37 | 1.375 / 38.597 / 644.141 |

The producer's `responses_per_second` counts completed attempts including transport failures. It must not be described as successful HTTP throughput. All-attempt latency percentiles also include errors; HTTP-only percentiles are separately retained in the derived JSON. Cause of the transport failures was not established. No unexpected received HTTP 5xx other than controlled 503 was observed, but missing responses prevent a universal controlled-response claim.

| Stage | Auth admissions | Argon2 operations / sampled peak | Auth inflight sampled peak | Instrumented DB acquisitions delta | Redis commands delta | Ordinary profile 200 / sampled | Ordinary p95 ms | RSS peak MiB |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 19 | 5 / 4 | 4 | 4 | 739 | 2 / 2 | 46.478 | 109.87 |
| 2 | 8 | 8 / 4 | 4 | 4 | 413 | 2 / 2 | 165.977 | 115.09 |
| 3 | 87 | 50 / 4 | 4 | 43 | 4095 | 21 / 21 | 52.623 | 121.98 |
| 4 | 18 | 5 / 4 | 4 | 4 | 668 | 2 / 2 | 42.728 | 120.73 |
| 5 | 19 | 5 / 4 | 4 | 4 | 680 | 2 / 2 | 23.330 | 120.95 |

The 30,000 stage lasted 23.708s: 87 auth admissions, 50 hashes, sampled active hashes/inflight both four, 21/21 authenticated profile and 21/21 health probes 200, profile p95 52.623ms, peak RSS 121.98MiB. Financial transaction/event/journal rows stayed zero. One coalesced Redis-limiter incident row occurred; it is not concealed as a clean dependency run. Instrumented acquisition counts include observed ordinary traffic and do not count every direct pool query. Redis deltas include probes and observer operations. Sampled maxima do not prove absence of an unobserved transient peak.

CPU averages/peaks, HTTP-only latencies, raw statuses, per-request IDs, DB before/after and process samples are retained in [http-derived-summary.json](audit_evidence/post_implementation_runtime_20261019/http-derived-summary.json) and [http-20261019T181155-observer.json](audit_evidence/post_implementation_runtime_20261019/http-20261019T181155-observer.json). The producer JSONs retain request identifiers/latencies without credentials. The unchanged independent observer returned **INCONCLUSIVE**, not DEFENSE_PASS: sanitizer/DB waiter observations, a predeclared ordinary latency budget and comprehensive core-DB evidence were absent. Case name `patient` was explicitly mapped to its generic `distributed_login` classifier in a separate derived input without changing the original attack record.

## Dependency failure and database lock

The first fault run demonstrated fail-closed behavior but its Redis forwarder listener exited. Its subsequent recovery failure is a **fixture failure**, not a confirmed application defect. The first pool attempt was contaminated by that failure; a preliminary retry also failed readiness. All records remain. The fixture forwarder alone was recreated with one-second supervised listener restarts, maintaining its original resource/network constraints.

Repaired run [faults-20261019T182031-observer.json](audit_evidence/post_implementation_runtime_20261019/faults-20261019T182031-observer.json): 100 requests / eight workers while only the owned Redis container was paused; 100 HTTP503, one incident row/occurrence, readiness 503 in 260.796ms, then readiness 200 after unpause. Final authenticated profile 200 in 26.090ms; no economic rows. This is a scoped dependency-loss/recovery pass, not a full restart/failover test.

The lock attempt immediately after Redis recovery did not establish SQL waits and is not graded as pool saturation. Separate fresh run [pressure-20261019T182204-observer.json](audit_evidence/post_implementation_runtime_20261019/pressure-20261019T182204-observer.json) independently confirmed an ACCESS EXCLUSIVE lock on fixture `auth_sessions` before launching 12 reads (eight normal paths/four consultation-class paths). It observed five auth connections waiting on SQL locks; pool size five, idle zero at those samples, versus configured maximum ten. Five requests returned408 after approximately 10.007s; seven returned503 with Retry-After1 in256–260ms. The latter occurred with limiter-unavailable evidence and are not attributed to DB saturation. Later samples showed all five idle and recovered profile/health 200. The field named `new_request_during_pressure` was collected after serial observations had outlasted the lock; its200 cannot prove availability during saturation. Consultation-class calls used a Patient token and are not legitimate Doctor consultation availability tests.

**DB_POOL_PRESSURE_TEST=INCONCLUSIVE**: bounded locked queries and recovery were observed, but full pool exhaustion, DB acquire-deadline counts and actual waiter peaks were not established. No pool or admission setting was enlarged/changed.

## Scenario accounting

| Requested scenario | Actual evidence | Status |
| --- | --- | --- |
| Distributed auth / many accounts | Five real HTTP stages; all three roles at100 same-target; Patient30,000 different-target | Executed; scoped bounds/profile checks pass, full acceptance INCONCLUSIVE |
| Expensive endpoint isolation | Actual Argon2 flood + ordinary profile probes; unit lane-reservation tests | Partial; authorized upload/clinical mixed-load NOT_RUN |
| Upload saturation | Sanitizer/content unit tests run; no configured synthetic storage/authorized upload fixture | HTTP flood NOT_RUN; worker bound/cancellation not runtime-certified |
| Payment initiation saturation | Actual DB races run; provider checkout adapter absent | HTTP/provider acceptance NOT_RUN; no claim from rejection statuses |
| Webhook flood / replay | DB event replay tested; real provider verifier/adapter acceptance absent | HTTP signed-callback flood NOT_RUN |
| Redis loss |100 ? HTTP 503, coalesced incident, readiness/profile recovery after fixture repair | Scoped PASS |
| DB saturation | Real lock/waits/timeouts, pool not exhausted | INCONCLUSIVE |
| Recovery stampede | Seven coordinator tests/web and three/app, deterministic1,000-client jitter model | UNIT_TESTED PASS; full-stack1,000-client NOT_RUN |
| Mixed realistic traffic | Auth + one synthetic profile + health only | Requested richer consultation/chat/upload/payment mix NOT_RUN; no production mix invented |

Missing live metrics are **NOT_MEASURED**: open-loop offered_rps, comprehensive DB query count, DB acquire-deadline count, DB waiter peak, sanitizer-worker peak under flood, authenticated consultation success, valid-provider callback success and production edge behavior. No missing metric was set to zero. Argon2 cancellation while blocking work continues and sanitizer cancellation remain unexecuted focused cases.

## Safe reproduction

Use the exact frozen source baseline and cached toolchain/image versions. First inspect labels and select **new** unused fixture names/databases/loopback ports; never reuse the shared5432/6379 services. The original standalone SQL command is `./services/backend/scripts/resilience/Invoke-IsolatedSql.ps1`; the OTP wrapper command and raw-source hashes are in its JSON. SQL/Lua fixture deadlines/resources are retained with those records.

Restore the retained trusted fixture tools to `scratch/acceptance-runtime-20261019/` according to their relative paths, prepare the loopback/internal PG+Redis fixtures as recorded in [fixture-forwarder-commands.md](audit_evidence/post_implementation_runtime_20261019/fixture-forwarder-commands.md), and use supervised listener loops documented in the retained fault evidence. Verify both listeners plus independently observed parallel PG connections before starting the backend. Existing databases cause the new fault script's `createdb` to fail rather than resetting data. The scripts never target a public domain.

Build from the disposable backend copy with `cargo build --offline --locked --bin hellodoctor-backend`. Run `python -I -S scratch/acceptance-runtime-20261019/http-fixture/verify-http.py --binary scratch/acceptance-runtime-20261019/cargo-target/debug/hellodoctor-backend.exe --repo <absolute-root>` using the recorded allowlisted environment and bounded runner. Actual producer argument vectors are retained per stage; do not increase its32-worker/30,000-identity ceilings. Fault scripts are fixture-specific and must be reviewed for fresh database names before rerunning. Stop on memory/wall/5xx guards. Preserve producer and independent SQL/telemetry observations, then verify labels and remove only owned resources.

This test does not establish a safe Azure RPS capacity, immunity to DDoS, or a guarantee that a consultation/webhook survives full shared-pool exhaustion.

Cleanup: [fixture-cleanup.json](audit_evidence/post_implementation_runtime_20261019/fixture-cleanup.json) confirms only the three owned composite-fixture containers and two owned networks were removed; shared container IDs/start times are unchanged and no volumes were removed. Retained reproducibility scripts are under [fixture-tools](audit_evidence/post_implementation_runtime_20261019/fixture-tools). The supervised-forwarder correction and contaminated records are identified in [fixture-forwarder-repair.json](audit_evidence/post_implementation_runtime_20261019/fixture-forwarder-repair.json).
