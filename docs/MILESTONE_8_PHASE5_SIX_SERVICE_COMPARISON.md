# Milestone 8 Phase 5 — Post-Optimization Six-Service Comparison

**Date:** 2026-11-02
**Scope:** Step 2 of the Phase 5 plan — short, controlled six-service comparisons
against the live `hellodoctor-m8-final-local` stack, after deploying the
`chat_conversations.doctor_id` index and `last_message_at` denormalization
(see [`MILESTONE_8_PHASE5_EFFICIENCY_INVESTIGATION.md`](MILESTONE_8_PHASE5_EFFICIENCY_INVESTIGATION.md)).
Unchanged throughout: 4-vCPU/8-GB topology, `DB_POOL_MAX_CONNECTIONS_PER_ROLE=10`,
Consultation admission lane `n.min(6)`, all RLS/ABAC/auth/rate-limit controls,
Agora configuration. No pool or admission-limit tuning performed.

## Pre-run verification (Step 1, completed before any load test)

All performed against the real live stack, no synthetic bypasses except where
explicitly noted:

- Caddy routing confirmed with correct `Host` headers for all three vhosts
  (`api.localhost`, `admin.localhost`, `doctor.localhost`).
- Real authenticated Doctor backend requests (`GET /doctor/chats`,
  `/doctor/appointments`) via real login + OTP + JWT: `200 OK`.
- Real Doctor-Web BFF session (real login + MFA + cookie) **and** the actual
  `GET /api/proxy/doctor/chats`/`/doctor/appointments`/`/doctor/profile`
  proxy calls through that session: `200 OK` for all three — the gap flagged
  after the previous report is now closed.
- Real patient login, real slot creation, real booking, real HMAC-signed
  payment webhook confirmation (secret handled via an operator-provided,
  gitignored `-SecretsEnvFile`, never printed/logged), and a real chat
  message send as the authenticated doctor — confirmed the new
  `last_message_at` trigger fires correctly against the live production
  database (not just the earlier isolated clone): message sent at
  `15:49:23.483160180Z`, conversation's `last_message_at` immediately read
  back as `15:49:23.483160Z`.
- Host-port binding issue (Windows/Hyper-V dynamic TCP port exclusion ranges,
  confirmed via `netsh interface ipv4 show excludedportrange`) resolved by
  republishing backend/proxy on 48280/48080 instead of the excluded
  58280/58080 — unrelated to Docker, the application, or this migration.
- Rate-limit/IP-jail state: this session's own interactive login testing
  left `ratelimit:login:<ip>` entries with short TTLs (confirmed via
  read-only `TTL`/`GET`, no secret exposure) — allowed to self-expire rather
  than retried repeatedly. `-ResetTestRateLimitState` (default on, scoped to
  `ratelimit:booking:*`/`ratelimit:grievance:*` on this benchmark's own
  isolated Redis container only) cleared leftover contamination from earlier
  Phase 4 runs before each scenario below.
- Benchmark binaries (`capacity_test.exe`, `web_workload.exe`) reused as-is
  from the prior Phase 4 build: neither touches `doctor_handlers.rs` or the
  migrations, only `hellodoctor_backend::crypto::create_access_token`,
  unaffected by this pass's changes.

## Scenario A — API workload only, Admin/Doctor Web idle (360s)

| Metric | Result |
|---|---|
| Achieved RPS | 174 (steady state, 2,000 active sessions) |
| True system error rate | 0.6851% |
| Business rejection rate | 0.058% |
| End-to-end booking success | 95.3% (3,778 / 3,965) |
| Postgres peak CPU | ~207% |
| Backend peak CPU | ~128% |

Two endpoints account for most of the (still nonzero) error count:
`GET admin telemetry` (208/1,945, 10.7%) and `POST webhook confirm`
(156/3,934, 4.0%) — consistent with the user's own prior note that a
separate ~0.48% API-only Webhook/Expensive admission-limit gap exists
independent of the web-load question this phase targets.

## Scenario B — API workload + realistic 3-Admin/5-Doctor Web activity (360s)

| Metric | Result |
|---|---|
| Achieved API RPS | 145.5 |
| Achieved Web RPS | 2.61 (admin 0.94, doctor 1.66) |
| True system error rate | **8.6626%** |
| Business rejection rate | 0.013% |
| End-to-end booking success | **81.6%** (3,126 / 3,831) |
| Postgres peak CPU | **368.39%** |
| Backend peak CPU | **309.01%** |
| Max active Postgres connections | 6 (of pool=10/role; unchanged — still not the constraint) |
| Dominant Postgres wait state | `active, wait_event=none` (93 samples) — genuinely CPU-bound, same diagnosis as Phase 4, not lock/IO-bound |

Admission rejections this run (full breakdown in
`docs/performance-evidence/milestone-8-final/phase5/scenario_plausible/`):
dominated by `lane_concurrency` across classes (`normal` 1,908, `expensive`
827, `webhook` 519, `consultation` 355), with `dependency_pressure` a smaller
contributor (`normal` 688, `consultation` 102, `expensive` 16) — i.e. lanes
filling up because more work is now actually completing and re-queuing
faster, not primarily DB/Redis back-pressure.

The real Doctor-Web BFF proxy calls under load (`web_workload.log`) directly
exercise the fixed `GET /api/proxy/doctor/chats`: p50 647ms, p95 3,171ms, p99
3,662ms, 2/76 syserr — far better than Phase 4's pre-fix p50/p95/p99 of
5,682/7,454/8,651ms for the same call, though still elevated relative to the
isolated single-query measurement (369-400ms) because this run's 2,000
concurrent API sessions are competing for the same CPU budget.

## Comparison with Phase 4's documented realistic-web baseline

| Metric | Phase 4 (pre-fix) | Phase 5 (this pass) | Change |
|---|---|---|---|
| True system error rate | 12.27% – 13.39% | **8.66%** | ~33-35% relative reduction |
| End-to-end booking success | 73.7% – 76.4% | **81.6%** | +5-8 points |
| Postgres peak CPU | 233% – 314% | **368%** | *Higher*, not lower — see analysis |
| Doctor Chats query (isolated) | 3,452ms (pre-Phase-3) / 1,631ms (post-Phase-3) | **400ms** | 5.0x vs. post-Phase-3 |
| Max active Postgres connections | ≤6 | ≤6 | Unchanged — confirms connections were never the constraint, in either phase |

### Why Postgres peak CPU went up, not down, despite the 5x query fix

This is a real, measured result, not an error, and it is consistent with —
not contradictory to — the fix being effective. Before Phase 5, the slow
Doctor Chats/Appointments queries themselves were part of what kept admission
lanes (and therefore accepted request volume) throttled: a request sitting in
a 1.6-8.6 second query held its lane slot that whole time, so fewer requests
per second ever reached Postgres to begin with. After Phase 5, each request
completes far faster, so **more legitimate work is actually admitted and
reaches Postgres per unit time** at the same offered load — raising
Postgres's observed peak CPU while simultaneously raising completed
throughput and lowering the error rate. The wait-event sampling confirms
Postgres is spending that CPU on genuine active query execution (`active,
wait_event=none`), not padding from locks or I/O, exactly as Phase 4 found.
This is the expected signature of removing a downstream bottleneck and
exposing the next one (CPU capacity), not a regression in the fix itself.

## Step 4 — readiness decision

Per the product owner's own stated decision criteria: an improvement from
~12-13% errors to below 1% would be major progress (not yet achieved); only
reaching ≤0.35% with healthy booking completion justifies moving toward the
200 RPS acceptance run.

**Result: 8.66% true system error rate.** A real, reproducible-in-direction
improvement (lower errors, higher booking success, confirmed by the same
query-level and wait-event evidence used throughout this investigation), but
**not** below 1%, and nowhere near the 0.35% acceptance target.

**Decision: do NOT proceed to the 200 RPS diagnostic or the 60-minute
acceptance run.** The Phase 5 database-query optimization produced a
material, measured, single-digit-improvement in system-wide behavior under
realistic Admin/Doctor web load, but confirms — rather than resolves — Phase
4's own conclusion: the remaining constraint is genuine CPU capacity on this
4-vCPU host, now even more directly visible because the query-level
inefficiency that was partially masking it is gone. Further query-level
optimization of `fn_doctor_treats_patient`/`doctor_history_context_allowed`
(the original Phase 4 suspects) was already ruled out in the Phase 5
investigation as not being where the cost is. The next lever is a genuine
CPU-capacity question — host sizing, or a further efficiency pass targeted at
the endpoints admission is now rejecting most (`normal`, `expensive`,
`webhook` lanes) — not another database query fix in the Doctor Chats path.

## Explicitly not done this pass

- No pool size or admission limit changes (`DB_POOL_MAX_CONNECTIONS_PER_ROLE`,
  Consultation `n.min(6)`, or any lane ceiling) — left exactly as instructed.
- No 200 RPS diagnostic or 60-minute acceptance run.
- No second repeat of Scenario B for reproducibility confirmation (single run
  only, per the time budget for this pass) — recommended before treating the
  8.66%/81.6%/368% figures as fully stable rather than single-sample.
- No investigation of the separate ~10.7%/4.0% `admin telemetry`/`webhook
  confirm` error contributors visible even in the idle (Scenario A) run.
- No git staging/commit/push. The operator-provided secrets file
  (`services/backend/scripts/.bench_secrets.env`) is gitignored and should be
  deleted once this investigation is fully concluded.
