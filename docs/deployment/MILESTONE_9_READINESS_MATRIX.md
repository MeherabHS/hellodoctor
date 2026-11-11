# Milestone 9 — Pre-Deployment Readiness Matrix

**Audit date:** 2026-11-07
**Status of this document:** Synthesis of four independent read-only audits. No application code, migrations, Docker configuration, or environment variables were modified to produce any of the four source audits or this matrix.
**Source documents:**
- `docs/deployment/DEV_DEBUG_AND_SEED_AUDIT.md` (Audit A)
- `docs/deployment/ARM64_CONTAINER_ADAPTER_AUDIT.md` (Audit B)
- `docs/deployment/POSTGRES_BACKUP_DR_AUDIT.md` (Audit C)
- `docs/deployment/AZURE_ACS_EMAIL_AUDIT.md` (Audit D)

Every classification below was independently spot-checked against the live source tree while synthesizing this matrix (not merely copied from the sub-audits) — see "Independent verification performed" at the end.

---

## 1. Critical deployment blockers (ranked)

### Blocker 1 — Real appointment booking and payment confirmation are impossible outside `ENVIRONMENT=development`
- **Evidence:** `services/backend/src/services/appointment_service.rs:46-48` and `services/backend/src/api/v1/webhook_handlers.rs:161-165` both unconditionally return an error (`409 Conflict`) whenever `state.config.environment != "development"`, with no feature flag and no override. The webhook guard runs **before** HMAC signature verification. Confirmed by direct read, not just audit citation.
- **What this means:** No patient can book and pay for a consultation, and no payment-provider webhook can ever confirm a payment, in test, staging, or production as the code stands today. This is the entire revenue-generating core of the product.
- **Why it exists:** There is no real payment-gateway (M-Pesa/Bkash/card) integration yet. The guard is a correct, deliberate fail-closed stand-in — it prevents fabricating a "confirmed" booking with no real payment behind it. It is **not a bypass to remove**; it is a missing integration.
- **Decision required from the user/product owner, not from code:** Either (a) a real or sandboxed payment provider must be integrated before any staging/production use that includes live booking, or (b) if Milestone 9's "internal pre-launch verification" does not need live booking yet, this limitation must be explicitly acknowledged and documented so it is not discovered by surprise during a demo.
- **Verification:** `POST /api/v1/appointments/book` and `POST /api/v1/webhooks/payment/:provider` against any non-development deployment → expect `409` every time, regardless of payload.

### Blocker 2 — The repository documents two different, unreconciled database topologies
- **Evidence:** `docs/DISASTER_RECOVERY_ARCHITECTURE.md:3-9` (confirmed verbatim by direct read) states the production **Decision** as Azure Database for PostgreSQL Flexible Server, with self-hosted PostgreSQL + pgBackRest framed as a fallback "to review." But the actual, ARM64-validated, concretely-built deployment artifact (`deploy/single-vm/`) **is** the self-hosted PostgreSQL + pgBackRest topology. `docs/CHECKPOINT.md:2349-2355` already flagged this exact contradiction as a real production blocker in an earlier session; it was never reconciled.
- **What this means:** Anyone reading only the "Decision" section will wrongly conclude production uses a managed Azure PaaS database with Microsoft-operated PITR/HA, and that none of the self-hosted backup readiness gaps below apply. `docs/MILESTONE_7C_DR_REPORT.md` and `docs/DISASTER_RECOVERY_PLAN.md`'s RPO/RTO targets are written for the managed-service topology and have never been re-derived for the single-VM topology actually being shipped.
- **Decision required:** Before any disaster-recovery sign-off, the architecture documents must be reconciled to state unambiguously which topology is production. This is a documentation task, not a technical one, but it is a genuine prerequisite — no one should sign off on "DR is production-ready" while the canonical architecture doc contradicts the shipped artifact.

### Blocker 3 — No backup/restore/Key Vault operation has ever touched real Azure infrastructure
- **Evidence:** Every GREEN result in `docs/recovery-evidence/` and the Milestone 8 docs is local, disposable, or synthetic. No stanza-create/backup/restore has ever run with `repo1-type=azure` against a real Storage Account; no Key Vault fetch has ever run against a real vault with real IMDS/managed-identity token issuance; no real alert receiver has ever been configured or paged.
- **Also BLOCKED, same root cause:** `deploy/single-vm/arm64-images.env.example:18`'s `POSTGRES_BACKUP_IMAGE` is a literal unfilled placeholder digest — the custom pgBackRest-enabled image has never been built-and-published; only an ad hoc local tag exists.
- **Decision required:** This requires a real Azure subscription with a Storage Account, Key Vault, and managed identity provisioned — explicitly out of scope for this read-only audit. Ten exact remaining tests are enumerated in Audit C §"Exact remaining tests."

### Blocker 4 — Azure Communication Services Email has no managed-identity or Key Vault support, and has never sent a real email
- **Evidence:** `config.rs`'s ACS configuration path implements only static access-key authentication; managed identity is explicitly rejected at config-validation time (asymmetric with Blob storage and pgBackRest, both of which default to managed identity). No Key Vault integration exists anywhere in `services/backend/src` for this or any other backend secret — Key Vault usage is confined to the separate pgBackRest container (confirmed by grep: no match for Key Vault SDK/API usage in `services/backend/src`).
- **What this means:** The ACS access key must currently be supplied as a plain environment variable to the backend container; there is no real delivery test on record (none was run in this audit, by design — no Azure credentials available).
- **Decision required:** `PASSWORD_RESET_ENABLED` is already correctly hard-blocked from activating in staging/production by `validate_password_reset_policy` pending a real ACS delivery-acceptance test plus a separate explicit approval — this is enforced at config-validation time, not just documented. The same real-delivery test is also the only way to validate the branded login-OTP cutover actually reaches a real inbox in a production-shaped environment.

---

## 2. Readiness matrix by area

| Area | Status | Evidence | Source |
|---|---|---|---|
| **Dev/debug controls — auth/OTP/password-reset** | **PASS** | All dev-only paths (`debug_otp_code` echo, legacy SMS-OTP, fixture-account login) are double-gated on exact-match `environment == "development"` and fail closed; fixture-user login is enforced both at token issuance and on every subsequent request, independently | Audit A §1, §2, §12.1 |
| **Dev/debug controls — payment/payout** | **CRITICAL — see Blocker 1** | `environment != "development"` gates booking and webhook confirmation; two further payout-related dev-only routes (`wallet_handlers.rs:136,200`) correctly fail closed with `403` and are **not** part of the same gap (deliberate deprecation / fixture-only testing route) | Audit A §3; independently re-read in full this session |
| **Dev/debug controls — storage/push/rate-limit** | **PASS** | Azure Blob `Disabled` state, FCM absent-credentials fallback, and benchmark-only rate-limit overrides are all explicit operator-chosen states confined to non-production compose files, never the real `deploy/single-vm/compose.yml` | Audit A §5, §6, §10 |
| **Dev/debug controls — seed data** | **PASS** | `SEED_DEV_DATA` fatal outside development/test at startup; all four seeded accounts independently blocked from authenticating and from keeping a session alive outside development/test, confirmed by direct grep of both enforcement sites (`seed.rs:262`, `auth.rs:129`, `auth_service.rs:1202`) | Audit A §12.1; independently re-verified this session |
| **Dev/debug controls — admin simulate-log/error routes** | **PASS (staging-available by design)** | `DEV_SIMULATION_ENABLED` permitted in development/test/staging, fatal in production specifically; requires authenticated Admin + ABAC | Audit A §11 |
| **Dev/debug controls — web/mobile apps** | **PASS** | CSP relaxation is standard `NODE_ENV`-gated Next.js dev behavior, automatically off in both apps' production Docker build; `sideDoorGuard.ts` confirmed to be a protective filter despite its name, not a bypass | Audit A §13, §14 |
| **Container images — Rust backend (ARM64)** | **PASS** | Real `docker buildx build --platform linux/arm64 --load` succeeded after fixing a genuine `TARGETARCH` ARG-scoping bug (previous default silently defeated Buildx's automatic platform-arg population); compiled binary executed under QEMU and produced real, expected Rust panics from genuine config-validation code | Audit B §1; built and run this session |
| **Container images — Admin Web / Doctor Web (ARM64)** | **PASS** | Real emulated build + real execution verified | Audit B §1 |
| **Container images — pgBackRest (ARM64)** | **PASS (local tag only)** | Real emulated build + real execution verified locally; no reviewed digest has been published for the ARM64 image-pin template (see Blocker 3) | Audit B §1; Audit C §A |
| **Container images — Postgres/Redis/Caddy (ARM64)** | **PARTIAL** | Confirmed via registry manifest metadata only (multi-arch manifest lists include `linux/arm64/v8`); never actually pulled and run on this session's ARM64 path | Audit B §1 |
| **Container resource budget (4 vCPU / 8 GiB)** | **PASS (calculation only)** | 5952 MiB total app-tier budget fits within 8 GiB with headroom for host/OS overhead, by arithmetic over `compose.yml`'s per-service `mem_limit`; no real load test has measured this under production-shaped traffic | Audit B §2 |
| **Container security/privileges** | **PASS, one open decision** | Per-service `read_only`/`cap_drop`/non-root user table confirmed; Caddy's capability-drop is inconsistent with the `x-web` anchor and backend (needs an explicit decision — Caddy binding ports 80/443 as non-root may genuinely need a capability the others don't — not necessarily a code defect) | Audit B §3 |
| **Container networking** | **PASS** | Postgres/Redis confirmed fully non-public (no published ports, `data` network is `internal: true`); Admin Web correctly on both `app` and `data` (matches its `REDIS_URL`), Doctor Web correctly on `app` only (no Redis env var) | Audit B §4; self-corrected and re-verified directly against `compose.yml:109-138` this session |
| **Persistent volumes / secrets / health checks / migration strategy / logging** | **PASS (static review only)** | All reviewed and judged sound by design; none has been exercised in a real multi-service integration run | Audit B §5-§9 |
| **Startup/shutdown ordering (all 6 services together)** | **BLOCKED** | Never tested — `depends_on: service_healthy` chains are correctly declared but have never been exercised end to end | Audit B §10 |
| **Backup-container Compose integration** | **PASS (wiring review only)** | Static review of `compose.backup.yml`'s image/entrypoint/command swap and the separate scheduler container; logic correctness is Audit C's domain, not re-litigated here | Audit B §11 |
| **Rollback procedure** | **PASS (documented), BLOCKED (never executed)** | Procedure exists; has never been run even once | Audit B §12 |
| **Postgres 16 / image-digest consistency** | **PASS** | Identical pinned manifest digest across `compose.yml`, `arm64-images.env.example`, and `pgbackrest.Dockerfile`, confirmed by direct comparison | Audit C §A |
| **WAL archiving** | **PASS (local only)** | Archive-push/archive-get both GREEN in a disposable local fixture, including one earlier failed attempt kept as evidence (not papered over) before the configuration was fixed; never run against real Azure Blob | Audit C §B |
| **Backup repository (Azure Blob) config** | **CONFIGURED, BLOCKED for production trust — see Blocker 3** | Managed-identity-by-default design is sound; zero real-Azure runs exist | Audit C §C |
| **Managed identity / Key Vault (backup container)** | **CONFIGURED, BLOCKED for production trust — see Blocker 3** | Sound design (IMDS token, never a static credential); only unit-level coverage against synthetic inputs | Audit C §D |
| **Encryption at rest** | **PASS (local), key rotation BLOCKED** | AES-256-CBC GREEN locally including wrong-key rejection; corrected key-rotation procedure documented but never executed even once | Audit C §E |
| **Retention policy** | **CONFIGURED, immutability BLOCKED** | Time-based retention (7/14 day) works locally with shortened test values; Azure Blob immutability is explicitly proposed-only, never applied to a real container, with an acknowledged unresolved interaction risk against pgBackRest's own expiration logic | Audit C §F |
| **Alerting on backup failure** | **CONFIGURED, BLOCKED for production trust** | Two real bugs found and fixed in this audit trail (an inverted exit-code capture that silently reported forced failures as success; a missing `python3` dependency that meant staleness checks could never have run); no real alert receiver has ever been tested | Audit C §G |
| **Recovery-mode application gating** | **PASS (local, real exercised test)** | `RECOVERY_MODE`/`RECOVERY_OPERATOR_TOKEN_SHA256` fail-closed design confirmed by a real exercised local test (`RECOVERY_ACCEPTANCE_7C1B.md`), not just a code read | Audit C §H |
| **Erasure-journal / DR distinction** | **CONFIRMED DISTINCT, journal custody BLOCKED** | Correctly a separate concept from Postgres backup (protects against resurrecting erased patient data on restore); today it only writes to a filesystem path, not the intended Azure immutable Blob | Audit C §H |
| **Full 3-phase recovery transition (RECOVERY → ISOLATED ACCEPTANCE → NORMAL)** | **BLOCKED** | No end-to-end drill with a second operator's sign-off has ever been recorded | Audit C §I |
| **Restore procedures (mechanism)** | **PASS (local/disposable only)** | Strongest evidence in the repository — a real encrypted pgBackRest repository was restored into a real fresh PostgreSQL instance and queried, with verbatim restored-log evidence captured; never run off a disposable VM, never against Azure | Audit C §J |
| **Replacement-VM / regional restore drill** | **BLOCKED** | Never attempted, local or real; the plan's "1 hour" RTO target has never been measured | Audit C §J |
| **Architecture documentation consistency** | **FAIL — see Blocker 2** | `DISASTER_RECOVERY_ARCHITECTURE.md`'s own Decision section contradicts the shipped topology | Audit C headline finding |
| **ACS endpoint/credential shape validation** | **PASS (code-level)** | `validate_azure_email_configuration` forces all three `AZURE_EMAIL_*` vars together outside dev/test; never tested against a real ACS resource | Audit D |
| **ACS managed-identity / Key Vault — see Blocker 4** | **BLOCKED** | Not implemented; explicitly rejected at config-validation time | Audit D |
| **Fail-closed behavior (missing/unreachable/erroring email provider)** | **PASS** | Verified by direct HTTP-level test this session: missing config → 409, unreachable/error outcomes → 503 (fixed from a prior 500-vs-503 mismatch), successful acceptance → 200, each independently asserting `pending_mfa_challenges` row state, not just status code | Audit D; fixed and tested earlier this session |
| **Branded login-OTP cutover (Patient/Doctor/Admin)** | **PASS (code/test level)** | Live, wired to the existing branded templates via the existing tracked-delivery path; zero changes to OTP generation/persistence/verification/rate-limiting/lockout; never sent to a real inbox | Audit D; implemented and tested earlier this session |
| **Password-reset atomicity** | **PASS** | `complete_reset` now invalidates every outstanding recovery challenge for the account (not just the one consumed), inside the same transaction as the password update and session revocation; verified by a new concurrency test asserting exactly one of two simultaneous completions succeeds | Verified earlier this session via `tests/cases/password_reset.rs` |
| **Password-reset production activation** | **BLOCKED by explicit documented policy** | Correctly refused at config-validation time in staging/production pending a real ACS delivery-acceptance test plus separate approval | Audit D |
| **Email delivery durability (outbox/retry/dedup)** | **PASS** | Durable outbox table, worker retry/backoff, event-specific dedup keys; acknowledged PITR-replay residual risk is a known, accepted trade-off | Audit D |
| **Email-specific monitoring/alerting** | **PARTIAL** | Durable outbox + logs exist; no dedicated alerting/metrics channel specifically for email delivery failures | Audit D |

---

## 3. Necessary remediation, ranked by what blocks the stated goal ("internal pre-launch verification")

1. **Decide and document the booking/payment scope for pre-launch verification** (Blocker 1). If live booking is needed for verification, this requires product/engineering scoping work beyond this audit's charter — a real or sandboxed payment integration. If not needed yet, record this limitation explicitly in whatever readiness sign-off follows this matrix.
2. **Reconcile `DISASTER_RECOVERY_ARCHITECTURE.md`'s Decision section** with the actual single-VM/pgBackRest topology (Blocker 2) — a documentation-only fix, but a prerequisite to any DR sign-off.
3. **Run `cargo run --example config_preflight` against the real staging/production `.env`** before any deployment — a cheap, already-built mechanical gate that would catch `ENVIRONMENT=development` or a missing required secret before it ever reaches a reachable host.
4. **Confirm the production `.env` values explicitly:** `ENVIRONMENT=staging|production` (never `development`), `DEV_AUTH_OTP_ENABLED=false`, `SEED_DEV_DATA=false`, `PASSWORD_RESET_ENABLED=false` (until the ACS sign-off happens), `STORAGE_PROVIDER=azure` (not `disabled`), `DEV_SIMULATION_ENABLED` per the staging-allowed/production-forbidden split.
5. **Provision real Azure infrastructure for backup/DR** (Storage Account, Key Vault, managed identity) and execute the ten exact tests enumerated in Audit C — out of scope for this read-only audit, requires a provisioning decision and budget from the user.
6. **Build and publish the real `POSTGRES_BACKUP_IMAGE`** with a reviewed digest, replacing the placeholder in `deploy/single-vm/arm64-images.env.example`.
7. **Decide the Caddy capability-drop inconsistency** (explicit decision, likely no code change needed).
8. **Run the never-yet-executed six-service startup/shutdown integration test** against the real `deploy/single-vm/compose.yml` topology (with no real Azure Blob required for this specific test — local/synthetic backend config is sufficient to validate ordering and health checks).
9. **Rename `docker-compose.configA.yml`** (and sibling benchmark files) to avoid operator confusion with the real production topology — non-urgent, cosmetic.
10. **A real ACS delivery-acceptance test**, prerequisite to both the login-OTP cutover's production validity and password-reset activation — requires real Azure Communication Services provisioning, out of scope for this audit.

---

## 4. Verification commands (collected)

```bash
# Config preflight — catches ENVIRONMENT/secret misconfiguration before boot
cargo run --example config_preflight   # against the real staging/production .env

# Confirm no rate-limit override leaked into the real topology
grep -n GLOBAL_IP_RATE_LIMIT_PER_MIN deploy/single-vm/compose.yml   # expect no match

# Confirm no fixture account can authenticate outside dev/test
# (run against a staging/production deployment)
curl -X POST https://<host>/api/v1/auth/admin/login \
  -d '{"email":"admin@hellodoctor.health","password":"AdminPass123!"}'
# expect 401 "Development fixtures are unavailable."

# Confirm booking/payment are still fail-closed (expected, not a bug to fix)
curl -X POST https://<host>/api/v1/appointments/book -H "Authorization: Bearer <patient token>" -d '{...}'
# expect 409 every time

# Confirm debug_otp_code never leaks outside development
curl -X POST https://<host>/api/v1/auth/doctor/login -d '{...}'
# response JSON must not contain a non-null debug_otp_code

# Confirm real Postgres/Redis are non-public from outside the VM
nc -zv <vm-public-ip> 5432   # expect connection refused/timeout
nc -zv <vm-public-ip> 6379   # expect connection refused/timeout

# ARM64 image build verification (already run this session; re-run after any Dockerfile change)
docker buildx build --platform linux/arm64 --load -t hellodoctor-backend:arm64-verify -f services/backend/Dockerfile services/backend
docker image inspect --format '{{.Architecture}}' hellodoctor-backend:arm64-verify   # expect arm64

# pgBackRest — real Azure checks (requires real Azure resources, not yet run)
pgbackrest --stanza=hellodoctor check
pgbackrest --stanza=hellodoctor info
```

---

## 5. Proposed execution sequence

1. **Decisions (no infrastructure required):** resolve Blocker 1's booking/payment scope question and Blocker 2's documentation reconciliation. Both are prerequisites to everything downstream being honestly labeled.
2. **Mechanical gate:** run `config_preflight` against the real intended `.env` and fix anything it flags.
3. **Six-service integration test, local/disposable:** bring up `deploy/single-vm/compose.yml` (without the backup overlay) on any Docker host and confirm the health-check chain and startup order actually work end to end — this needs no real Azure credentials and closes one of the larger "never tested" gaps cheaply.
4. **Provisioning decision:** if real Azure DR/backup trust is required before go-live, provision the Storage Account, Key Vault, and managed identity, and work through Audit C's ten exact remaining tests in order (stanza-create/check/backup against real Blob first, then Key Vault, then a real replacement-VM restore drill, then alerting, then key rotation, then immutability).
5. **Provisioning decision:** if real email delivery trust is required before go-live, provision ACS and run the real delivery-acceptance test Audit D specifies, then (separately) obtain the explicit approval `validate_password_reset_policy` is gating on if password reset is needed for verification.
6. **Cosmetic/non-urgent cleanup:** rename `docker-compose.configA.yml`, resolve the Caddy capability-drop decision, rename `totp_encryption_key_b64` if convenient.

---

## 6. Independent verification performed while writing this matrix

The following claims from the four sub-audits were re-checked directly against the live source tree before being relied upon here, consistent with this session's established practice of not trusting sub-agent or prior-pass output for high-stakes claims without a direct read:

- `appointment_service.rs:46-48` and `webhook_handlers.rs:161-165` — read in full; guard text and placement (webhook guard precedes HMAC check) confirmed exactly as cited.
- `wallet_handlers.rs:136-141` and `:195-204` — read in full; both confirmed to be clearly-labeled, fail-closed, dev-only/deprecated routes, distinct in character from the booking/webhook gate (no payment is ever silently confirmed).
- `is_fixture_user` double-enforcement — grepped all three call sites (`seed.rs:262`, `auth.rs:129`, `auth_service.rs:1202`); confirmed to exist exactly where Audit A claims.
- `docs/DISASTER_RECOVERY_ARCHITECTURE.md:1-15` — read directly; the "Decision" section's Flexible-Server framing and fallback characterization of pgBackRest confirmed verbatim.
- Audit B and Audit D summary classification tables — re-read directly from the published documents to ensure this matrix's wording matches what was actually written, not a reconstruction from memory.

No claim in this matrix rests solely on a sub-agent's self-report without at least one of: a direct file read performed in this session, or authorship by me directly (Audits B and D).
