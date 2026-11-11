# HelloDoctor.Asia — Milestone 8 Final Acceptance

Date: 2026-11-01  
Verdict: **PARTIAL — local engineering evidence is substantially complete; production readiness is not declared.**

This closes the remaining Workstream 4 verification without starting Milestone 9.
All tests used synthetic data and disposable infrastructure. Shared development
PostgreSQL and Redis remained running and were not modified.

## Completed

- Added and reviewed the one-VM deployment package in `deploy/single-vm/`:
  Rust backend, PostgreSQL 16, Redis, Admin Web, Doctor Web, Caddy, private
  data network, health checks, restart policies, persistent-volume declarations,
  and a combined resource budget.
- Added the disposable local stack override and ran all six services together.
- Added `ops/dr/verify_local_postgres_dr.ps1`, a verification harness only; it
  does not replace the guarded recovery helper or create a production scheduler.
- Preserved Claude’s backend fixes, benchmark evidence, synthetic-data tooling,
  and the independent recovery work.
- Resolved the CMS approval question: author self-approval is allowed for
  non-sensitive content; sensitive article/category content requires an
  independent Admin and is enforced by API and database rules.

## Runtime evidence

### Disposable six-service stack

The merged Compose configuration parsed successfully. Fresh named volumes and
loopback-only ports were used. All six containers reached healthy state:

| Service | Runtime result |
|---|---|
| PostgreSQL 16 | GREEN — healthy; migrations current |
| Redis | GREEN — healthy |
| Rust backend | GREEN — `/healthz` reported matching binary/database migration `20261202000005` |
| Admin Web | GREEN — login page returned HTTP 200 |
| Doctor Web | GREEN — root page returned HTTP 200 |
| Caddy | GREEN — proxy health check passed |

PostgreSQL and Redis had no host bindings in the disposable stack. Caddy alone
was bound to loopback test ports. A synthetic PostgreSQL row survived a
PostgreSQL/Redis restart, and all services became healthy again. Restart-induced
PostgreSQL connection termination messages were expected; no unexplained
backend, Redis, web, or Caddy fatal/error lines were found.

The disposable HTTP route verified page and health routing. The development
stack’s Admin login challenge and OTP verification were exercised, but an
automatic browser Secure-cookie replay through the local HTTP-only test route
was not accepted as production cookie evidence. A local Caddy `tls internal`
attempt was blocked by the Windows Docker environment’s certificate-install
failure. Existing Admin Web tests and earlier live authentication evidence cover
the production BFF/session behavior; a real HTTPS browser/device smoke test
remains required. Status: **PARTIAL**.

### Resource and latency evidence

The approved production Compose limits are all on the same `cpuset 0-3` and
sum to 5,952 MiB: PostgreSQL 3,072 MiB, Redis 384 MiB, backend 1,536 MiB,
Admin Web 448 MiB, Doctor Web 384 MiB, and Caddy 128 MiB. This leaves about
2.05 GiB of the 8 GiB VM for Ubuntu, Docker, cache, and backup activity. CPU
shares are relative weights, not dedicated CPU allocations.

The disposable focused stack sampled these approximate peaks while serving
health/page traffic: backend 43 MiB, PostgreSQL 39 MiB, Redis 4.5 MiB, Admin
Web 59 MiB, Doctor Web 53 MiB, and Caddy 17 MiB. No OOM or restart occurred in
the sample window. Page latency samples were:

- Admin login page: p50 36 ms, p95 41 ms, p99 46 ms; 30/30 HTTP 200.
- Doctor root page: p50 59 ms, p95 61 ms, p99 62 ms; 30/30 HTTP 200.
- Direct backend health checks returned HTTP 200 at steady state. Repeated
  high-frequency probes also encountered the configured admission/rate-limit
  boundary, so those probes are not reported as capacity latency evidence.

These are local Docker measurements on a 12-vCPU/20-GiB development host with
container limits, not measurements of the approved Azure 4-vCPU/8-GiB VM. They
do not certify full-load p50/p95/p99, PostgreSQL pool pressure, or Azure host
overhead. Status: **PARTIAL**.

## Backup and disaster recovery

The repository’s existing `ops/dr/hellodoctor_dr.py` remains a guarded,
provider-neutral inventory/restore helper. It intentionally does not implement
scheduled backup, WAL retention, Azure credentials, or production cutover.

Fresh disposable PostgreSQL evidence is recorded in
`docs/recovery-evidence/m8-final-local-dr/local-postgres-dr.json`:

- Physical `pg_basebackup`: **GREEN**.
- `pg_verifybackup` manifest validation: **GREEN**.
- WAL archiving: **GREEN** — six archived WAL segments observed.
- Named restore-point PITR into a fresh target and promotion: **GREEN**.
- Integrity: **GREEN** — pre-boundary marker present and post-boundary marker
  absent after recovery.
- DR helper safety tests: **GREEN** — 7 Python tests passed.
- Encrypted object backup/restore and wrong-key rejection: **GREEN** — 2 tests
  passed.
- Production backup retention/scheduling: **BLOCKED** — no repository-level
  scheduler or retention worker exists.
- Physical backup-at-rest encryption/key custody: **BLOCKED** until the
  separately approved off-host destination and key-management policy exist.
- Azure backup/storage integration: **BLOCKED** — no subscription or authorized
  credentials.

The local PITR drill proves PostgreSQL mechanics only. It does not prove an
off-host immutable copy, Azure Blob access, WAL shipping durability after VM
loss, retention enforcement, or a production restore drill.

## Regression results

- Backend serialized integration suite: **GREEN — 464 passed, 0 failed,
  4 ignored**, `cargo test --release --test api_tests -- --test-threads=1`,
  fresh PostgreSQL/Redis fixtures.
- Backend recovery object tests: **GREEN — 2 passed, 0 failed**.
- Admin Web complete command with owned Redis fixture: **GREEN — 98 passed,
  0 failed** in the TypeScript phase and **28 passed, 0 failed** in the TSX
  phase. The complete command is the package `npm test`, with
  `TEST_REDIS_URL` and `TEST_REDIS_FIXTURE_ID` set to the owned fixture.
- Doctor Web: **GREEN — 85 passed, 0 failed**.
- Admin and Doctor production builds: **GREEN**.
- Admin and Doctor Docker image builds from pinned Node base digest: **GREEN**.
- Previously accepted Flutter and Milestone 8 evidence remains historical and
  was not relabelled as a new run here.

The backend suite’s earlier incomplete Workstream 4 attempt is superseded by the
final 464/0/4 result above.

## External integrations

| Integration | Status | Evidence and next activation gate |
|---|---|---|
| Azure Blob clinical/article storage | **PARTIAL/BLOCKED** | Provider selection, encryption, container separation, authorization and Azurite contract are implemented in prior evidence. Cloud RBAC, managed identity, private endpoint, retention and live round-trip require an Azure subscription. |
| Email OTP | **NOT TESTED/BLOCKED** | Development OTP challenge was used; configure Azure Communication Services endpoint, access key, verified sender and staging mailbox, then exercise delivery and expiry. |
| FCM | **PARTIAL/BLOCKED** | Notification records and payload protection are tested; delivery requires Firebase credentials, mobile app configuration and a physical/emulated device. |
| Agora RTC | **PARTIAL/BLOCKED** | Backend token authorization/time-window tests pass; join, reconnect, audio/video and device lifecycle require two authorized devices and Agora credentials. |
| Payment gateway | **PARTIAL/BLOCKED** | HMAC webhook, idempotency, replay and ledger behavior pass with synthetic callbacks; no sandbox gateway adapter/credentials were exercised and no money was charged. |

No real patient message, financial charge, clinical record, or provider-side
operation was used.

## Sensitive Admin step-up finding

The requirement traceability matrix does not define a step-up workflow. Existing
security documents explicitly record sensitive Admin step-up reauthentication as
**STILL_OPEN**, and the deployment checklist says it does not declare general
production readiness. No speculative MFA/step-up redesign was added. This is a
**production-readiness blocker** for sensitive financial/security actions until
the product owner approves the policy and a separately tested implementation is
available. Ordinary session rotation, reuse detection, logout revocation and
authorization remain covered by existing evidence.

## Remaining blockers

1. Azure subscription and managed identity/private Blob configuration.
2. Off-host encrypted PostgreSQL backup retention, WAL shipping, and an approved
   restore drill.
3. Real Azure VM resource/latency measurements on the 4-vCPU/8-GiB host.
4. External TLS/DNS/firewall/secrets deployment and production Caddy certificate
   issuance.
5. Physical-device Agora, FCM and end-to-end Patient/Doctor acceptance.
6. Authorized email delivery and payment sandbox verification.
7. Approved sensitive Admin step-up policy and implementation.

## Acceptance verdict

Local engineering acceptance is **PARTIAL**: code packaging, local six-service
startup, migrations, persistence across restart, backend regressions, web builds,
and disposable PostgreSQL physical recovery are verified. The combined resource
model is configured within one shared 4-vCPU/8-GiB budget, but target-VM runtime
measurement is not available.

Overall Milestone 8 production readiness is **NOT DECLARED / PARTIAL**. The
remaining items are operational or provider/device gates rather than a reason to
restart engineering or begin Milestone 9. No stage, commit, push, public deploy,
shared-volume deletion, or shared-database reset was performed.

## 2026-11-01 addendum — two corrections from the remaining-gap assessment

See `docs/MILESTONE_8_REMAINING_GAP_ASSESSMENT.md` for the full evidence behind
both items below; this addendum only updates this document's own verdicts.

1. **Payment hold/expiry/reconciliation (relates to the "Payment gateway" row
   in the External integrations table above): the local-engineering portion
   is GREEN, not open.** On re-inspection, the 10-minute payment-hold
   deadline is server-side and immutable, the background sweep and lazy
   reclaim both expire abandoned holds and release the slot independent of
   the client, the payment-vs-expiry race is transactional and tested for
   exactly-one-winner semantics, and a late provider success callback after
   expiry is captured, flagged for reconciliation, audited, and surfaced to
   Admin Finance rather than silently dropped or silently rebooked. Full
   evidence and test references are in `docs/PAYMENT_PRODUCTION_READINESS.md`
   (2026-11-01 correction section). The "Payment gateway" row's PARTIAL/
   BLOCKED status is otherwise unchanged: no real gateway adapter, sandbox
   credentials, or live refund/payout execution exist, and an active
   worker that polls a live provider for an ambiguous transaction's status
   remains correctly blocked on that same missing provider integration.

2. **Sensitive Admin step-up finding: WAIVED by explicit product-owner
   decision, not implemented.** The product owner has explicitly accepted the
   risk recorded in this document's "Sensitive Admin step-up finding" section
   above and decided not to implement step-up re-authentication for sensitive
   Admin actions. Existing Admin authentication and authorization (role/ABAC
   gating on every sensitive endpoint, session refresh/rotation, refresh-token
   reuse detection, and logout revocation) remain intact and were
   independently re-verified against current code with no regression. TOTP is
   not being restored. Item 7 of "Remaining blockers" above is closed by this
   waiver, not by engineering work.

Neither correction changes the overall Milestone 8 verdict: local engineering
acceptance remains **PARTIAL** and production readiness remains **NOT
DECLARED**, for the separate reasons detailed in
`docs/MILESTONE_8_REMAINING_GAP_ASSESSMENT.md`.
