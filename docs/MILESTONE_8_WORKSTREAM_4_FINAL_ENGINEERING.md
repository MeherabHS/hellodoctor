# Milestone 8 Workstream 4 — Final Engineering Refinement

Date: 2026-11-01  
Overall verdict: **PARTIAL**

This workstream closed the remaining CMS-policy investigation and packaged the
approved one-VM deployment model. It does not certify a live Azure deployment,
PostgreSQL backup/restore, physical devices, or external providers.

## Scope completed

- Inspected the uncommitted worktree, active branch, containers, deployment
  files, required Milestone 8 reports, `PROGRESS.md`, `CHECKPOINT.md`, and the
  requirement traceability matrix.
- Preserved the existing Claude performance, authentication, RLS, booking,
  payment, migration, and test-fixture work. No reset, stash, stage, commit,
  push, volume deletion, database cleanup, or shared-service restart was done.
- Added production Next standalone output and minimal container build contexts
  for both web applications.
- Added a complete, separate single-VM Compose configuration and Caddy reverse
  proxy under `deploy/single-vm/`.
- Captured baseline hashes/status/container evidence under
  `docs/performance-evidence/milestone-8-workstream-4/`.

## Files created or modified

Modified:

- `apps/admin-web/next.config.js`
- `apps/doctor-web/next.config.js`

Created:

- `apps/admin-web/Dockerfile`, `apps/admin-web/.dockerignore`
- `apps/doctor-web/Dockerfile`, `apps/doctor-web/.dockerignore`
- `deploy/single-vm/backend.Dockerfile`
- `deploy/single-vm/Caddyfile`
- `deploy/single-vm/compose.yml`
- `docs/MILESTONE_8_WORKSTREAM_4_FINAL_ENGINEERING.md`
- `docs/performance-evidence/milestone-8-workstream-4/*`

No backend source, migration, authentication rule, payment rule, rate-limit
threshold, backup tool, or benchmark harness was changed.

## CMS approval policy finding

The approved CMS rule is conditional: independent approval is required when the
article or its category is sensitive. The traceability and CMS design documents
explicitly describe non-sensitive platform material as eligible for the normal
workflow. The backend enforces the same rule in two layers:

- `admin_article_repo.rs` exposes `can_approve=false` and
  `INDEPENDENT_REVIEW_REQUIRED` for a sensitive author.
- `admin_article_handlers.rs` rejects a sensitive author’s direct approval with
  `403`, leaving the article pending and its version unchanged.
- The database check constraint and transition trigger reject sensitive
  self-approval as well.

The observed direct approval of a non-sensitive article by its author therefore
does **not** violate the approved requirement. No production correction was
made. Existing focused tests cover sensitive author denial, independent reviewer
approval, non-sensitive author approval, publication visibility, audit events,
RLS, optimistic versioning, and concurrent transition behavior. Status: **GREEN**.

## Benchmark and test tooling organization

`examples/capacity_test.rs` remains test-only and is not included by the backend
runtime image. Its token maintenance is deliberately a load-generator shortcut:
it re-mints access JWTs with the existing synthetic signing key before the real
15-minute TTL; it does not emulate refresh-token rotation or reuse detection.
This limitation remains disclosed. `CAPACITY_ISOLATED_ACTION` is an explicit
test-only switch. `SESSION_START` samples are bookkeeping markers, not HTTP
requests, so active-session counts must not be reported as concurrent request
counts.

The harness documents `MAX_BOOKING_ATTEMPTS_PER_PATIENT=8`. At the established
think-time and booking probability this can produce a plateau near 21 minutes;
that is a harness safety cap, not a capacity limit. The 1,200-session and
2,000-session commands, 10,000-patient scale-up SQL, slot top-up SQL, clinical
document harness, and telemetry scripts remain preserved. Synthetic seed data
and credentials are excluded by repository and Docker ignore rules and are not
copied into runtime images. No redundant test code was demonstrably safe to
remove, so no harness redesign or deletion was performed. Status: **GREEN** for
organization; benchmark execution remains historical evidence.

## Complete single-VM deployment architecture

`deploy/single-vm/compose.yml` defines one Azure VM with all services on the
same `cpuset 0-3`:

| Service | Memory limit | CPU shares | Network role |
|---|---:|---:|---|
| PostgreSQL 16 | 3,072 MiB | 3,000 | internal data network |
| Redis | 384 MiB | 500 | internal data network |
| Rust backend + in-process workers | 1,536 MiB | 6,000 | app + internal data |
| Admin Web | 448 MiB | 750 | app + data for refresh coordination |
| Doctor Web | 384 MiB | 750 | app |
| Caddy reverse proxy | 128 MiB | 500 | public edge + app |
| **Container limit subtotal** | **5,952 MiB** | — | — |

The subtotal leaves about 2.05 GiB of the 8 GiB VM for Ubuntu, Docker,
filesystem/page cache, and scheduled backup activity. CPU shares are relative
weights; they are not separate CPU allocations. PostgreSQL and Redis have no
published host ports. The backend is reachable only from the Compose networks.
Persistent PostgreSQL, Redis, Caddy, and erasure-journal paths are explicit
external/pre-provisioned volumes; the manifest never runs `down -v`.

The Caddyfile terminates HTTPS, overwrites forwarded metadata, injects the
server-only web ingress assertion, and routes API/Admin/Doctor hostnames to the
internal services. The web applications validate the assertion and forward only
validated HTTPS/client-IP metadata to Rust. Existing HttpOnly, Secure,
SameSite, CSP, origin, backend authorization, and web-auth assertion controls
remain authoritative.

The web Dockerfiles use Next.js `output: 'standalone'`, copy only the traced
runtime, static assets and public files, run as the unprivileged `node` user,
and exclude environments, tests, credentials, evidence, and local state. The
backend image remains the existing reviewed image; the added backend wrapper
only adds a health check.

Static validation: both web production builds passed, and both Docker builds
passed using the pinned Node base digest; the Next standalone output was emitted
for both applications. The Compose file was reviewed against the pinned
service/volume/network structure, but a full Compose interpolation check was not
performed because its private environment file is intentionally absent. A full
production Compose startup
was **NOT TESTED** because the approved production images, private environment,
TLS certificates, Azure VM, external volumes, and backup configuration are not
available and starting such a stack would require shared-service authorization.

## Combined CPU and RAM evidence

The existing Milestone 8 evidence verified the backend/PostgreSQL/Redis
configuration and the 2,000-session sustained benchmark. This workstream adds
the missing configuration and build artifacts but did not claim a full-system
runtime measurement. During focused verification, the shared development
PostgreSQL and Redis stayed healthy; disposable test PostgreSQL used roughly
67 MiB of its 1.5 GiB test limit and disposable Redis roughly 6 MiB of 128 MiB.
Those are fixture observations, not production capacity evidence.

Full six-service idle/active CPU, RAM, p50/p95/p99 latency, OOM/restart, pool,
Redis-key, and reverse-proxy evidence is **NOT TESTED** on the target VM. The
complete VM cannot be labelled GREEN from backend/database measurements alone.

## Test suites executed

- Admin Web: **28 passed, 0 failed** in the current `npm test` glob.
- Doctor Web: **85 passed, 0 failed**.
- Admin Web production build: **PASS**.
- Doctor Web production build: **PASS**.
- Backend serialized API suite was started against a disposable PostgreSQL and
  Redis fixture with `--test-threads=1` and owned fixture markers. It exercised
  many cases successfully but was stopped before the full suite completed to
  avoid leaving the disposable fixture running; its partial log is preserved as
  `backend-api-tests.log`, so no final pass count is claimed here.
- Prior accepted evidence remains: backend library 129/129, serialized API
  462 passed / 2 isolated timing or process-state failures / 4 ignored,
  Flutter 302/302, Admin Web 98/98, Doctor Web 85/85. Those historical counts
  are not relabelled as this workstream’s execution.

No device test or real provider test was run. Agora RTC join, FCM delivery,
email delivery, live payment gateway transactions, and Azure Blob round-trip
remain **NOT TESTED/BLOCKED** by missing authorized environments. No real money
or patient data was used.

## Existing Claude changes and backup work

Claude’s admission fairness, query/RLS, booking/payment concurrency,
transaction-number, JWT lifecycle, memory, Redis TTL, benchmark, client
dependency, and test-isolation work remains intact. The independent PostgreSQL
backup/recovery work remains in its existing files (`docs/BACKUP_RESTORE_RUNBOOK.md`,
`docs/DISASTER_RECOVERY_*`, `ops/dr/`, and related backend code). This workstream
does not overwrite or duplicate it. The Compose design provides persistent
database storage and a durable erasure journal path, but backup scheduling,
off-host retention, restore drills, and WAL verification remain that separate
acceptance gate.

## Production-readiness blockers and verdict

- PostgreSQL backup-and-restore evidence: **BLOCKED** / separate work.
- Full six-service runtime resource and latency evidence on one 4-vCPU/8-GiB
  Azure VM: **NOT TESTED**.
- External TLS, DNS, firewall, managed secrets, Azure Blob/RBAC, FCM, email,
  Agora device join, and live payment provider: **BLOCKED/NOT TESTED**.
- Physical-device verification: **NOT TESTED**.
- Sensitive Admin step-up authentication remains an existing documented gap.

Workstream 4 is **PARTIAL**: the CMS finding is resolved by documentation and
existing server-side enforcement, benchmark tooling is organized and preserved,
and the complete deployment configuration is ready for operator review. The
overall Milestone 8 production-readiness verdict must remain **PARTIAL**, not
GREEN, until backup/restore, target-VM resource measurements, and the separately
owned provider/device gates have actual evidence.
