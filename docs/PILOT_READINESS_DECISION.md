# Pilot Readiness Decision

> Produced per `docs/CHECKPOINT.md` §3 item 8, after items 1–7 (test suite rework, multi-dimensional rate limiting, webhook dedup, admin-portal hardening, N+1 cleanup, load testing — `docs/CAPACITY_EVIDENCE_REPORT.md`) were completed and evidenced, not merely compiled/tested.
> **Decision date:** 2026-10-04.

## Decision: **NOT_READY_FOR_UNSUPERVISED_1200_USER_PILOT**

**CONDITIONALLY_READY_FOR_A_SMALL_SUPERVISED_PILOT** (tens, not hundreds, of real users, with engineering actively monitoring) — see conditions below.

## Why not a flat "ready"

This is not a judgment that the security/correctness work is weak — the opposite is true (see §1 below). It is purely that **no evidence exists yet at anything close to 1,200-user scale or on production-representative infrastructure**, and the user's own master directive set that evidentiary bar explicitly. Per `docs/CAPACITY_EVIDENCE_REPORT.md` §6, the load test that was run:

- Tested 50 concurrent synthetic users, not 1,200.
- Ran on one Windows development laptop over loopback — no network latency, no TLS cost, no real geographic distance to Bangladesh/Kenya users, no production-sized dataset, no real payment-gateway round trip.
- Ran for 45 seconds, not a soak test.

Scaling a laptop's loopback numbers up linearly to "1,200 users in production" would be a real, dangerous overclaim, independent of what the numbers themselves say. The honest conclusion is that the *capacity* question remains open, even though the *correctness and security* questions have strong evidence behind them.

## 1. What the evidence does support

- The core security architecture (five-role least-privilege DB split, router-level default-deny JWT auth, ABAC, Row-Level Security including the `fn_doctor_treats_patient` circular-policy fix, multi-dimensional account+IP abuse jailing, webhook HMAC verification + replay dedup) is implemented, live-verified against real Postgres+Redis, and covered by a 21/21-passing integration suite that specifically targets cross-role/cross-tenant attack shapes (BOLA, privilege escalation, token tampering, replay).
- Under the load test that was run, this security stack added **no observable failure mode** — every real failure traced to a narrow, now-fixed application bug (the appointment-number collision) or a load-test harness bug, never to the auth/RLS/rate-limiting layers themselves.
- `MAX_ACTIVE_USERS` exists as a configurable admission gate and is tested (`test_max_active_users_caps_new_registration_not_existing_logins`), so there is a real mechanism to cap registration *when* capacity needs to be protected — it just hasn't been load-tested at its actual 1,200 default.

## 2. What would need to happen before this becomes an unqualified "ready"

In rough priority order:

1. **A real-scale load test**, not a 50-user laptop run: provision on the order of 1,200 real patient accounts (accepting the longer OTP-provisioning wall-clock time, or building a bulk-seed path for test accounts specifically), and run sustained load — minutes to hours, not 45 seconds — against infrastructure that at least approximates the intended production shape (a real cloud VM or container, not a developer laptop; ideally a managed Postgres instance with its own connection/IOPS characteristics).
2. **Production-representative data volume** in the test database — months of appointments/prescriptions/chat history, not a few hundred rows — so Postgres query plans and cache-hit behavior reflect reality rather than everything fitting trivially in memory.
3. **Real network conditions**, or at least a deliberate latency-injection test, given the target markets (Bangladesh, Kenya) have meaningfully different and more variable mobile network conditions than a local loopback test can show.
4. **A real payment-gateway sandbox test** (bKash/Nagad/M-Pesa), not just the generic signed-webhook stub, since gateway latency/timeout/retry behavior is completely untested so far.
5. **The two deferrals already on record** in `docs/CHECKPOINT.md` §3 item 5 (admin step-up re-authentication for sensitive actions; real token revocation on logout) should land before any pilot that includes real refunds/disbursements, even a small supervised one — those are financial-control gaps, not performance gaps, but they belong in the same "before trusting this with real users and real money" bucket.
6. **cargo-audit / dependency and secret scanning in CI** (checkpoint item 10) — not yet set up; cheap to add and should exist before any pilot, supervised or not.

## 3. What a small, supervised pilot could reasonably do now

Given the strength of the security evidence specifically, a **small, actively-monitored pilot (tens of real users, not hundreds, with an engineer watching logs/metrics in real time and able to intervene)** is a reasonable next step *if the user wants one before items 1–4 above are done* — it would itself generate real production-shaped evidence to close the gap identified here. This is the user's call to make, not a default recommendation to proceed; the honest default per the evidence in hand is to treat items 1–4 as prerequisites for anything larger.

## 4. Explicit non-negotiable carried forward

Per the user's standing instruction (carried in `docs/CHECKPOINT.md` §3): none of the work that gets this to a "ready" state may be satisfied by weakening JWT verification, ABAC, RLS, or the Redis abuse checks on the request path. Every finding and fix in this pass (see `docs/CAPACITY_EVIDENCE_REPORT.md` §3) honored that — the global per-IP rate limit was made *configurable* (not removed) specifically so a legitimate single-origin test tool could be measured without permanently weakening the production default, and the per-account booking limit was left untouched; the load-test harness was changed to behave more realistically instead.
