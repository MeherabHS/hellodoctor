# Distributed abuse and load shedding

## Inspection baseline — before implementation

The request path in `services/backend/src/api/router.rs` has a 15 MiB body
limit, 10 second handler timeout, transport/proxy validation, a Redis-backed
per-IP guard, then authentication and handlers. The guard named `global_guard`
only applies `ratelimit:global:{ip}` and IP jail: it is not a service budget.
Tower `load_shed()` has no database-capacity readiness source here.

Five separate PostgreSQL capability pools default to ten connections each;
acquisition waits up to five seconds. Each protected request validates its
session through the auth pool. Upload sanitization has four process-wide
permits but waits on their semaphore. Per-account upload concurrency is two.
Password hashing also has four permits with waiting acquisition. Thus worker
execution is bounded, but incoming work/waiters have no explicit common bound.
Existing clients coalesce some reads and back off polling with jitter after
failures. Those are client optimizations, never authoritative admission controls.

## Implementation contract

Admission must run before Redis, session SQL, request body extraction, and
handlers. It must acquire permits without queuing, enforce a local aggregate
token budget, and retain permits until the admitted handler returns/cancels.
Auth, uploads, payment, investigation/downloads, and ordinary traffic have
separate caps. Provider callbacks, consultation operations, and health/metrics
have reserved caps independent of the ordinary request cap. A public flood on
the callback path can still exhaust that path; signatures and edge controls
remain necessary. No identity headers supplied by test clients are trusted.

Service-wide Redis budgets supplement the local ceiling across replicas.
Each replica retains its own fail-fast concurrency ceiling during a Redis
outage. Redis failure closes ordinary request admission. Readiness is allowed
to report dependency trouble without first requiring Redis.

Pool size/idle availability is an early pressure signal, not a reservation or
a capacity guarantee. Requests may race that sample; bounded admission and
short acquisition deadlines provide the backstop. No DB pool increase is part
of this change. Rejections must be controlled 429/503 with Retry-After and
bounded metric labels; they must not each generate a database error-log write.

## Edge and origin deployment requirements

Expected path: Internet → edge CDN/WAF → Azure origin reverse proxy → Rust API.
Select protections actually available in the subscribed service. Do not depend
on a proprietary feature for backend correctness. Restrict the origin to the
chosen edge/proxy using Azure networking and/or an authenticated origin path,
after explicitly managing SSH, deployment, monitoring, and recovery access.
This task does not change host firewall or Azure resources.

The reverse proxy must cap connections, header size/count, keepalive/idle time,
body size, request-header time and body-read time. The application timeout
bounds handler body extraction but does not cap sockets waiting for HTTP
headers. Public auth must use HTTPS and existing exact trusted-proxy rules.
See [Cloudflare origin guidance](https://developers.cloudflare.com/fundamentals/security/protect-your-origin-server/).

## Implemented admission policy

`AppState` shares an `Arc<Admission>` across all router clones. For the existing
ten-connections-per-role default, ordinary incoming work has a 20-request ceiling;
its lanes are Normal 8, Auth 4, Upload 4, Payment 2, Expensive 2. Health/metrics 2,
Webhook 2 and Consultation 4 have separate reservations (28 maximum combined
handler requests). Expensive ordinary lanes together cannot consume the final
ordinary admission slot. With smaller pools the caps scale down; Auth leaves
one auth connection free when at least two exist. A one-connection deployment
cannot offer that connection isolation. A semaphore reservation does not reserve
a database connection: independent accepted workloads can still contend.

Local token buckets use a one-second burst and these sustained refill rates:

| Lane | Local tokens/second | Shared Redis requests/minute |
| --- | ---: | ---: |
| Ordinary aggregate | 100 | 6000 |
| Normal | 100 | 6000 |
| Auth | 12 | 240 |
| Upload | 8 | 120 |
| Payment | 10 | 300 |
| Expensive | 20 | 600 |
| Consultation reservation | 60 | 3600 |
| Webhook reservation | 20 | 1200 |
| Health/metrics reservation | 10 | readiness has no Redis dependency; metrics retains per-IP policy |

These fixed initial constants are deliberately documented in `admission.rs`;
they are conservative policy, not a hardware throughput certification. Redis
fixed windows allow a boundary burst; local token buckets and concurrency caps
still apply. Callback, consultation and health reservations do not consume the
ordinary service bucket. Critical Doctor chat-list/consultation-summary routes
use the consultation reservation; uploads/downloads are classified first so
they cannot consume it. All Admin routes currently use the Expensive lane;
there is no independent authenticated Admin incident-recovery reservation.

Admission samples role pool size/idle/closed state before auth/handler SQL. It
rejects when the relevant pool is fully allocated without idle capacity. Common
authed transaction acquisition has a 250ms deadline; readiness dependency checks
also have 250ms deadlines. Session and legacy direct-pool queries retain the
pool's existing acquisition deadline, but only a bounded set of admitted
requests can wait. Shared rate-limit calls use 250ms; outer service and IP/jail
checks have 300ms combined deadlines. These are early backstops, not proof of
zero database waiters or predictable latency under every mixed workload.

Sanitizers now reject size/MIME/magic mismatches before blocking dispatch and
use try-acquire with four workers and no semaphore wait queue. The worker owns
its permit until completion even when its caller is cancelled. Existing
per-account two-upload concurrency and transaction-after-sanitize behavior are
preserved. Argon2 also uses four try-acquired workers; see the auth model.

Metrics include `admission_inflight`, `admission_accepted_total`,
`admission_rejected_total` with fixed class/reason labels, `db_pool_size`,
`db_pool_idle`, `db_acquire_seconds`, `db_acquire_deadline_total`,
`argon2_active_workers`, `argon2_operations_total`,
`upload_sanitize_active_workers` and `upload_sanitize_operations_total`.
Unmatched HTTP routes use a fixed metric label rather than arbitrary attacker
paths. Rejections do not create one system-error database write per request.
A limiter outage still creates an existing-pipeline dependency incident, capped
at one report per 30 seconds per application state; subsequent rejects use counters.
CPU, memory and peak direct-pool waiters still require external observer
sampling; they are not fabricated from admission counts.

Admin password entry additionally requires a short-lived, exact-body HMAC BFF
assertion, consumed through Redis NX EX90 after the BFF verifies Turnstile.
Deployed startup rejects absent/invalid `WEB_AUTH_ASSERTION_SECRET` before
connecting to databases; standalone router construction still denies Admin login
when it is unconfigured. Existing web/native shared Doctor routes use service,
account/IP and worker controls; an untrusted client-type header cannot prove a
browser challenge was completed.

## Cache and retry audit

Existing web/native read coalescing is per client/process, not a distributed
database-query single-flight guarantee. No new cache grants authorization or
stores clinical data. The reviewed high-read Doctor/slot/notification handlers
continue to perform authoritative SQL for admitted work; this change does not
claim a server cache or stale-while-revalidate implementation. Global admission
bounds cache-miss/repeated-read SQL work. If subsequent profiling identifies a
shared public metadata hot spot, add a narrowly keyed nonauthoritative read
cache separately and measure its coalescing behavior.

Polling failure backoff and recovery jitter are separate: online/focus/resume
must not reset all clients into an immediate request at the same instant.
Recovery fixes and their authored test inventory are recorded in the results
document; dynamic recovery timing remains an acceptance gate until executed.

## Evidence policy

Configured ceilings are not measured capacity. New 30,000-client HTTP,
login/upload isolation, pool/CPU/memory, and recovery measurements must be
recorded separately in `ADVERSARIAL_RESILIENCE_RESULTS.md`. Missing execution
or telemetry is INCONCLUSIVE. No DDoS immunity is claimed.
