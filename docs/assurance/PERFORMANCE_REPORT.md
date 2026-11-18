# Performance and capacity report

Baseline: commit `eef9ce7` (formerly `6f02b84`) on the 4 vCPU / 8 GB six-service topology. Every number below is
regenerable from `docs/assurance/evidence/<scenario>/` with
`services/backend/scripts/assurance/summarize_assurance.py`; see [BASELINE.md](BASELINE.md) for
exactly what was measured and how it differs from production.

## 1. Summary

| Question | Answer (4 vCPU / 8 GB, production admission settings, payments deferred) |
|---|---|
| Sustainable mixed traffic | **250 requests/s** with p95 23 ms and 0 server errors (≈ 1,250-5,000 people actively using the app at once) |
| What stops it at 300 RPS | Admission-control **budgets**, not hardware: backend CPU p95 33 %, Postgres 28 %, DB pool 20 % |
| Hardware limit (budgets ×10, diagnostic build) | 350 RPS within targets, ~675 successful RPS peak; then the API database pool (10 connections) and CPU |
| Overload behaviour | Controlled: excess gets 429/503 **with Retry-After on 100 %**; latency stays flat; immediate recovery after a 5× spike |
| 65-minute soak (2,200 users + Admin and Doctor dashboards) | 663,204 requests, **0 server errors**, p95 15 ms, 2,200/2,200 bookings; 1.03 % controlled rejections (just over the 1 % target) |
| Weak points found | **Login** throughput (≈ 2 sign-ins/s platform-wide, and 5 per 15 min per IP), **open chats** (~600-650 at the old 10 s poll), **uploads** (2/s platform-wide), Admin dashboard concurrency (2) |
| Defects fixed during this assessment | 502s when a rejected request had a body (21 → 0 in the ramp, 11 → 0 for uploads); 503 log flood; both verified by before/after runs |
| Limits tuned after the runs (§5) | Sign-in, per-IP sign-in, chat, upload and admin limits raised on 2026-11-18. **Not yet re-measured**: every number in §4 is from the earlier limits |

Overall: the system is **stable and correct under overload**: no crashes, OOM kills, data errors or
uncontrolled failures were seen in any run. It is **conservatively budgeted**, so the published
capacity is set by deliberate limits rather than by the hardware; several of those limits (login,
chat, upload, admin) are tighter than real use is likely to need and are listed in §5 as tuning
recommendations.


## 2. Method

### Service levels used to judge a load step

| Measure | Target |
|---|---|
| p95 latency (server-side, through Caddy) | ≤ 1,000 ms |
| Server errors: 5xx other than 503, plus requests with no response | ≤ 0.1 % |
| Controlled rejections: 429 and 503 from rate limits and admission control | ≤ 1 % |

The highest step meeting all three is the **maximum sustainable rate**. A 503 or 429 is a
*controlled* rejection (the client is told to retry, with `Retry-After`); it is counted apart from
genuine failures so overload behaviour can be judged on its own.

### Traffic

The load generator (`services/backend/examples/capacity_test.rs`) drives the real API through Caddy
with real signed JWTs, real session checks, role checks and row-level security. Two drive modes:

- **Open loop** (ramp, spike, upload): requests arrive at a set rate whether or not earlier ones
  have finished, as real users do. Before measurement starts, 3,000 patients each book an appointment
  and open a Peer Navigator conversation (unmeasured), so consultation traffic acts
  on real appointments. New bookings during the run come from the other 7,000 patients.
- **Sessions** (soak, chat): each simulated user is one long-lived session that acts, waits a
  random 5-20 s "think time", and acts again.

Mix per action (open loop): 10 % Peer Navigator traffic; the other 90 % split as 33 % doctor
directory, 20 % doctor slots, 10 % doctor-patient chat (75 % polls, 25 % sends, as the app polls
every 10 s), 8 % video token, 8 % new booking (payments deferred: no gateway step, as in production), 7 % doctor wallet,
5 % consultation telemetry, 4 % admin telemetry, 3 % token refresh (falls back to a directory read),
2 % grievance. Peer traffic: directory 20 %, peer slots 10 %, my requests 5 %, chat list 10 %,
chat polls 30 %, chat sends 13 %, the peer's own inbox and chat list 12 %.

Every response body is read in full, as an app would.

### Known differences from real users

| Gap | Effect |
|---|---|
| The mix is an estimate; no production usage data exists yet. 8 % bookings is booking-heavy | Pessimistic for the booking path |
| Login is measured separately (scenario 6); the mixed traffic uses pre-issued tokens | Mixed results exclude password hashing |
| Video tokens return 403: test appointments are tomorrow, so the join window is closed | The final Agora signing step is skipped; it is cheap |
| One machine, no TLS, no mobile network | Server-side latency only; real phones add network time |
| Requests per second are not users. An active app user makes a request every ~5-20 s | 250 RPS ≈ 1,250-5,000 people actively using the app at once |

## 3. Scenarios

| # | Scenario | Shape |
|---|---|---|
| 1 | Ramp to failure | Open loop 50 → 800 RPS, ten 2-minute steps |
| 2 | Diagnostic ramp, budgets ×10 | Same ramp on a build with admission budgets raised tenfold, to find the hardware limit (not a shippable build) |
| 3 | Spike | 150 RPS → peak for 60 s → 150 RPS, then recovery |
| 4 | Soak | 60 minutes of sessions at about 70 % of capacity, plus Admin and Doctor web dashboards |
| 5 | Open chats | Sessions that only poll and send chat, rising until the consultation budget binds |
| 6 | Login | Real patient OTP, doctor password + MFA, refresh, logout: (a) all from one client address, (b) from many |
| 7 | Uploads | Prescription-photo uploads at rising rates (3 images of ~35 KB each) over 150 RPS of normal traffic |

## 4. Results

All runs: canonical dataset v2 restored and verified first; payments deferred, as production runs.
Server-side numbers through Caddy. "5xx" excludes 503 (load shedding), which is a controlled
rejection.

### 4.1 Ramp to failure (`evidence/ramp`)

| Offered RPS | Successful RPS | p95 ms | p99 ms | 429/503 | 5xx | Meets targets |
|---:|---:|---:|---:|---:|---:|---|
| 50 | 46 | 19 | 34 | 0 | 0 | yes |
| 100 | 93 | 19 | 23 | 0 | 0 | yes |
| 150 | 139 | 20 | 25 | 0 | 0 | yes |
| 200 | 186 | 22 | 26 | 0 | 0 | yes |
| **250** | **232** | **23** | **28** | 2 | 0 | **yes** |
| 300 | 274 | 24 | 30 | 1.8 % | 0 | no (rejections) |
| 400 | 344 | 22 | 31 | 8.9 % | 1 | no |
| 500 | 411 | 20 | 35 | 13.6 % | 1 | no |
| 650 | 470 | 24 | 42 | 24.5 % | 10 | no |
| 800 | 467 | 19 | 37 | 39.0 % | 9 | no |

Successful RPS levels off at about 470: the admission budgets (consultation 60/s, normal and
payment lanes) shed the rest. Latency does not degrade under overload. At the top step the backend
used CPU p95 33 %, Postgres 28 %, peak memory backend 87 MiB and Postgres 1.37 GiB; no OOM kills
or restarts. "Successful RPS" excludes intentional business refusals such as the video token's 403
outside the join window.

### 4.2 Diagnostic ramp, budgets ×10 (`evidence/ramp_diag_x10_payments_enabled`)

Not a shippable build: every admission budget raised tenfold, to find the hardware limit. (It ran
before the switch to deferred payments, so bookings included the payment webhook; this makes it
slightly pessimistic.)

| Offered RPS | Successful RPS | p95 ms | 429/503 | Meets targets |
|---:|---:|---:|---:|---|
| 250 | 250 | 17 | 0.0 % | yes |
| **350** | **347** | **20** | 0.7 % | **yes** |
| 450 | 445 | 26 | 1.3 % | no |
| 650 | 612 | 45 | 5.7 % | no |
| 900 | 675 | 76 | 25.0 % | no |

Here the API database pool (10 connections) reached 100 % and `dependency_pressure` shedding
started; Postgres CPU p95 42 %. With budgets as shipped, the 250 RPS figure is a policy choice with
about 40 % hardware headroom behind it.

### 4.3 Spike (`evidence/spike`)

150 RPS → **750 RPS for 60 s** → 150 RPS. During the burst 470 RPS succeeded, 34 % were shed with
Retry-After, p95 22 ms. The first step after the burst was back to normal (p95 21 ms, 2 rejections).

### 4.4 Soak, 65 minutes (`evidence/soak`)

2,200 concurrent users (sessions with 5-20 s think time, about 170 RPS) plus the Admin and Doctor web
dashboards. 663,204 requests: **0 server errors**, p50 6 ms, p95 15 ms, p99 82 ms; 2,200 of 2,200
bookings completed. Controlled rejections 1.03 %: per-account limits on grievance submissions
(the harness files grievances faster than a person would), consultation in-flight limit (6) and the
Admin expensive-lane concurrency (2), which also produced 246 errors in the Admin web workload.
Backend memory rose about 0.5 MiB/min (peak 260 MiB of 1,536 MiB): not a leak at this length, but
worth a longer soak (§5).

### 4.5 Open chats (`evidence/chat`)

Sessions that only chat (poll every 10 s, as the apps did on the baseline commit), rising to 2,000.
The consultation lane budget (60 requests/s) is the ceiling: **about 600-650 open chats**; beyond
that requests are shed (61 % at 2,000 open chats). No server errors from the backend (0.03 % at the
proxy, all the 502 race fixed in §6).

**Effect of the 2026-11-17 chat change (not measured, computed):** the apps now poll every 3 s for
2 minutes after a message, 10 s for the next 10 minutes and 30 s when idle. An active chat costs
3.3× the old load and an idle one a third of it. 60 requests/s therefore supports about **180
chats exchanging messages at the same moment**, or about 1,800 idle open chats, or a mix (e.g. 100
active + 750 idle). If live consultations are expected to overlap more than that, raise the
consultation budget (§5).

### 4.6 Login (`evidence/login_*`)

Real flows: patient password + OTP, doctor password + MFA, refresh, logout.

| Run | Result |
|---|---|
| (a) All from one IP address | 91 % refused: the per-IP limit is 5 sign-in attempts per 15 minutes |
| (b) Many IPs, 10 / 25 / 50 attempts per second | 90 % / 97 % / 98 % refused by the **auth class budget (240/min platform-wide)**: about 2 completed sign-ins per second. The budget also covers token refresh, so refreshes alone reach it at about 3,600 active users |

No password-hashing bottleneck appeared (Argon2id runs on its own worker pool); the limits are
policy. Accepted sign-ins completed with p95 ≤ 32 ms. Both limits are the main **High** finding (§5).

### 4.7 Uploads (`evidence/uploads`)

150 RPS of mixed traffic plus prescription uploads (3 images of ~38 KB each) rising 0.5 → 4 per
second. Up to 2/s: all accepted, p95 128 ms per upload. Above 2/s the upload budget (120/min
platform-wide) refuses the excess with 429 + Retry-After. CPU stayed low (backend p95 20 %): the
limit is policy.

## 5. Recommendations and what was done

Items 1-5 were applied on 2026-11-18 (`services/backend/src/admission.rs`, `src/rate_limit.rs`).
They were chosen from the measured headroom (backend CPU p95 ≤ 35 %, Postgres ≤ 30 % at the
budgeted limit; §4.2 showed the hardware sustaining 350 RPS) but **have not been benchmarked**:
re-run the ramp, chat, login and upload scenarios before quoting new capacity figures.

| # | Finding | Severity | Done (2026-11-18) |
|---|---|---|---|
| 1 | Auth budget 240/min platform-wide, shared with token refresh | High | Refresh and logout moved to their own `session` class (100/s local, 6,000/min platform-wide). Sign-in budget 240 → 1,200/min (about 10 sign-ins/s); local auth rate 12 → 20/s; Argon2 concurrency unchanged |
| 2 | Per-IP sign-in limit 5 / 15 min, counting successes | High | Raised to 300 / 15 min as a volume backstop for carrier-shared IPs. Brute-force protection unchanged in `ip_jail`: account locks after 5 failures from any IP; an IP failing against 20 accounts is jailed 30 min |
| 3 | Consultation budget 60/s caps open chats | Medium | 60 → 200/s local and 3,600 → 12,000/min platform-wide; in-flight 6 → 12. About 600 chats exchanging messages at once at the 3 s active poll |
| 4 | Upload budget 2/s platform-wide | Medium | 120 → 360/min (6/s). Per-account upload limits and the 4-slot image sanitiser unchanged |
| 5 | Admin expensive-lane concurrency 2 | Low | 2 → 4 |
| 6 | Backend memory +0.5 MiB/min over 65 min | Watch | Open: run a 6-8 h soak on the production VM before launch |
| 7 | Real devices and real network | Watch | Open: measure from phones in Kenya after deployment |
| 8 | Limits in items 1-5 not yet measured | Watch | Open: re-run the scenarios in §3 on the new build |

## 6. Defects found and fixed

### 6.1 502 instead of 429/503 when a request with a body was rejected

**Cause.** Admission control rejects before reading the request body. Closing the connection with
the body unread made Caddy return 502 to the client: either it reused the connection for the next
request, or (for a ~115 KB prescription upload) it was still sending the body when the socket closed.

**Fix** (`services/backend/src/admission.rs`, `settle_unread_rejections`). After an early rejection
the backend reads and discards the unread body when it is small (≤ 2 MB declared) and arrives within
2 s; otherwise it answers with `Connection: close`. A first attempt that only sent
`Connection: close` fixed small requests but not uploads, and was replaced.

| Run (before → final fix) | 5xx | Other change |
|---|---|---|
| Ramp 50 → 800 RPS | 21 → **0** (first fix) | Same capacity; p95 +2-9 ms above 400 RPS |
| Spike 750 RPS | 1 → **0** (first fix) | Burst p95 22 → 31 ms |
| Uploads up to 4/s | 11 → 13 (first fix) → **0** (final) | Upload p95 128 → 106 ms |
| Spike 750 RPS, final fix | 1 → **0** | Burst p95 34 ms; recovery immediate |

### 6.2 503 load shedding logged at ERROR

Each shed request wrote an ERROR line, flooding the log during overload and hiding real faults. Shed
503s are now logged at DEBUG (they are counted in `admission_rejected_total`); every other 5xx is
still logged at ERROR.


## 7. Reproducing these results

```powershell
# 1. Build images from the baseline commit (x86-64 host)
docker build --platform linux/amd64 -t hellodoctor-backend:perf-6f02b84 services/backend
docker build --platform linux/amd64 -t hellodoctor-admin-web:perf-6f02b84 apps/admin-web
docker build --platform linux/amd64 -t hellodoctor-doctor-web:perf-6f02b84 apps/doctor-web

# 2. Start the stack (deploy/single-vm/.env.perf-baseline holds synthetic local secrets)
cd deploy/single-vm
docker compose --env-file .env.perf-baseline -f compose.yml -f compose.perf-baseline.yml up -d
docker compose --env-file .env.perf-baseline -f compose.yml -f compose.perf-baseline.yml --profile loadgen build loadgen

# 3. Build the canonical dataset once (steps in evidence/baseline/CANONICAL_BASELINE_MANIFEST.json)

# 4. Run a scenario; it restores and verifies the dataset first
services/backend/scripts/assurance/Invoke-AssuranceScenario.ps1 -Scenario ramp -SamplerSecs 1500 `
  -LoadEnv @{ CAPACITY_RPS_STEPS='50,100,150,200,250,300,400,500,650,800'; CAPACITY_STEP_SECS='120';
              CAPACITY_PREBOOK='3000'; CAPACITY_PEER_SHARE='0.10' }

# 5. Summarise
python -I services/backend/scripts/assurance/summarize_assurance.py docs/assurance/evidence/ramp
```
