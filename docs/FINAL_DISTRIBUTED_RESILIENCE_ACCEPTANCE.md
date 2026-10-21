# HelloDoctor distributed resilience acceptance — 20 October 2026

## Historical transport run

The retained run `http-20261019T181155` offered 30,000 bounded closed-loop attempts through at most 32 producer workers. It recorded 31 HTTP 401 responses, 145 HTTP 429 responses, 10,074 HTTP 503 responses, and 19,750 transport failures. The retained transport rows contain only a boolean marker and structural request fields; they do not contain the OS exception category that the current `attack-local.py` now records.

Independent retained analysis found 19,731 of the 19,750 transport failures below 100 ms, p50 7.117 ms, p95 11.395 ms, p99 14.123 ms, and 19 at or above 1,900 ms. The observer recorded 29,913 auth budget rejections, one limiter-unavailable rejection, 87 auth admissions, 21 normal admissions, and 104 health admissions.

This evidence proves that the reported transport-error count was not successful throughput and that most failures were immediate. It does not prove whether the immediate failures originated in the producer’s connection pool, the fixture forwarder, server socket handling, or a local resource guard. The 19 long-latency rows are compatible with read/connect timeout behavior, but the retained artifact cannot distinguish them.

## Current decision

`TRANSPORT_ERROR_ROOT_CAUSE=INCONCLUSIVE`.

The most defensible current hypothesis is a producer/forwarder refusal or queue effect under the bounded auth budget, because nearly all missing responses arrived within one scheduling slice while the backend admitted very little auth work. This remains a hypothesis until a fresh stage records `transport_error_type`, `transport_phase`, OS errno/WinError, producer connection counts, and forwarder/backend socket state.

No admission ceiling was increased and no authorization or rate limiter was disabled. The current source includes the corrected admission capacity calculation; the four explicit A/B benchmarks passed and remain separately counted.

## Safe next run

The available current tree has the bounded classifier in `services/backend/scripts/resilience/attack-local.py` and the independent observer in `observe-local.py`, but no currently running disposable HTTP backend/forwarder fixture. A fresh run must first provision a newly labelled owned fixture and proceed through 100, 1,000, 5,000, 10,000, then 30,000 attempts only while the prior stage’s resource guard is healthy. It must retain exception categories and phases, HTTP-only latency, producer worker count, backend admissions, CPU/RSS, DB pool waiters, Redis health, and forwarder state.

Until that run exists, distributed load, authenticated consultation under pressure, upload flood pressure, and mixed workload recovery remain `INCONCLUSIVE` or `NOT_RUN`. The prior fixture failure records remain linked in [POST_IMPLEMENTATION_LOAD_RESULTS.md](POST_IMPLEMENTATION_LOAD_RESULTS.md) and were not overwritten.
