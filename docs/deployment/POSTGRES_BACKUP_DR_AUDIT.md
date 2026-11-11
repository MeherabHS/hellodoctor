# Milestone 9 — Audit C: PostgreSQL Backup and Disaster Recovery

**Audit date:** 2026-11-07
**Scope:** `deploy/single-vm/backup/*` (pgBackRest container, entrypoint, scripts), `deploy/single-vm/compose.backup.yml`, `docs/BACKUP_RESTORE_RUNBOOK.md`, `docs/BACKUP_ENCRYPTION_KEY_CUSTODY_RUNBOOK.md`, `docs/DISASTER_RECOVERY_ARCHITECTURE.md`, `docs/DISASTER_RECOVERY_PLAN.md`, `docs/MILESTONE_7C_DR_REPORT.md`, `docs/MILESTONE_8_POSTGRES_BACKUP_FINAL.md`, `docs/MILESTONE_8_BACKUP_OPERATIONS_CLOSURE.md`, `docs/AZURE_BLOB_SETUP.md`, `docs/recovery-evidence/*`, `ops/dr/*`, `services/backend/src/services/dr_erasure_journal.rs`, `services/backend/src/services/dr_reconciliation.rs`, `services/backend/src/config.rs` (recovery-mode settings).
**Method:** Direct file review and cross-referencing of documentation, scripts, configuration and recorded evidence files. **No pgbackrest/backup/restore command was executed, no Azure resource was touched, and no data was modified or deleted as part of producing this audit** — every GREEN/PARTIAL/BLOCKED classification below is sourced from existing recorded evidence, not from a fresh test run in this session.

---

## Headline finding: the repository documents two different, unreconciled database backup architectures

This is the single most important fact this audit surfaces, so it is stated first.

`docs/DISASTER_RECOVERY_ARCHITECTURE.md:3-9` states the production **Decision** as:

> "The production design selects **Azure Database for PostgreSQL Flexible Server** for the authoritative database and an application VM only for the stateless Rust and web containers. PostgreSQL on the VM with pgBackRest remains a disposable local simulation and a fallback design to review only if the pilot cannot use the managed service."

That is, as written, pgBackRest is explicitly framed as a *fallback*, not the production path. But the concrete, fully-built, dated deployment artifact in this repository — `deploy/single-vm/` (Compose topology, Dockerfiles, Caddy reverse proxy, the entire `backup/` pgBackRest subsystem) — *is* a self-hosted containerized PostgreSQL on a single VM, which is the architecture the same document calls the fallback. Audit B (`docs/deployment/ARM64_CONTAINER_ADAPTER_AUDIT.md`) independently confirms `deploy/single-vm/` is the actual, concretely-built, ARM64-verified deployment target — not a discarded prototype.

This is not a new observation — the project's own `docs/CHECKPOINT.md:2349-2355` already flagged it in writing, prior to this audit:

> "Identified, explicitly NOT fixed: no backup/WAL-archiving mechanism exists for the now-mandatory self-hosted PostgreSQL. `docs/BACKUP_RESTORE_RUNBOOK.md` and `docs/DISASTER_RECOVERY_ARCHITECTURE.md` are both written around Azure Database for PostgreSQL Flexible Server's managed backups — which the single-VM decision eliminates entirely. This is owned as separate work outside this session (the user's own instruction: do not interfere with it), flagged here because it is a real production blocker independent of every benchmark number above it."

The pgBackRest subsystem under `deploy/single-vm/backup/` was built *after* that note (dated 2026-11-01, vs. the checkpoint entry's earlier date) specifically to close this gap for the self-hosted case, and `DISASTER_RECOVERY_ARCHITECTURE.md` itself has a later "Local engineering boundary" section (lines 58-87) that accurately describes the pgBackRest implementation in detail. **But the document's own "Decision" section at the top (lines 3-9) was never updated to match** — it still names Flexible Server as the selected production database and pgBackRest as a fallback "to review," while the rest of the same file, `docs/BACKUP_RESTORE_RUNBOOK.md`, `docs/BACKUP_ENCRYPTION_KEY_CUSTODY_RUNBOOK.md`, and the entire `deploy/single-vm/` build artifact all treat self-hosted PostgreSQL + pgBackRest as the real, only production backup path.

**Practical consequence:** anyone reading only the "Decision" section of `DISASTER_RECOVERY_ARCHITECTURE.md` (the normal entry point) will conclude the production database is a managed Azure PaaS service with Microsoft-operated PITR/HA/geo-redundant backup — and therefore that none of the pgBackRest readiness gaps below apply to production. That conclusion is wrong for the topology this repository actually builds and ARM64-validates. `docs/MILESTONE_7C_DR_REPORT.md` (the design decision record) and `docs/DISASTER_RECOVERY_PLAN.md`'s RPO/RTO table are likewise written for the Flexible Server topology and have not been re-derived for a self-hosted-PostgreSQL-on-a-single-VM topology (e.g., the plan's "Database PITR RPO: 15 minutes or better" and "Replacement VM RTO: 1 hour or better when the database and Blob service... are intact" lines assume a managed service is doing the heavy lifting).

**This audit evaluates the pgBackRest/single-VM implementation on its own merits below** (it is the one that matches the actual deployment artifact), but the documentation inconsistency itself is a readiness blocker: no one should sign off on "disaster recovery is production-ready" until the architecture documents agree on which database topology is actually shipping.

---

## A. PostgreSQL 16 version and image consistency

| Reference | Value | File:line |
|---|---|---|
| Base `postgres` service image | `postgres:16-bookworm@sha256:e8a07d2c7579b038938cdba6c539e580e5001caf9ea203d397b5cc83e76bd938` | `deploy/single-vm/compose.yml:33` |
| ARM64 image pin template | same digest | `deploy/single-vm/arm64-images.env.example:8` |
| pgBackRest image `FROM` | same digest | `deploy/single-vm/backup/pgbackrest.Dockerfile:1` |

**CONSISTENT.** All three references resolve to the identical pinned PostgreSQL 16 manifest digest. This is a real, verified cross-file consistency — not assumed.

**MISSING / INCONSISTENT:** `deploy/single-vm/arm64-images.env.example:18` sets `POSTGRES_BACKUP_IMAGE=registry.example.invalid/hellodoctor/postgres-pgbackrest@sha256:REPLACE_WITH_REVIEWED_ARM64_DIGEST` — this is a **literal unfilled placeholder**. The custom pgBackRest-enabled PostgreSQL image that `deploy/single-vm/compose.backup.yml` requires for *both* the `postgres` service override and the `backup` service (`compose.backup.yml` uses the same `${POSTGRES_BACKUP_IMAGE:?set the reviewed pgBackRest-enabled PostgreSQL image}` for both — internally consistent with itself) has never actually been built-and-published with a real digest for the ARM64 env template. Only an ad hoc local tag (`hellodoctor-pgbackrest:m8-local`, per `docs/recovery-evidence/m8-pgbackrest-local/pgbackrest-local.json`) has ever been built and exercised.

---

## B. WAL archiving configuration

| Setting | Value | File:line |
|---|---|---|
| `archive_mode` | `on` | `deploy/single-vm/compose.backup.yml` (postgres service `command:` override) |
| `archive_command` | `pgbackrest --stanza=hellodoctor archive-push %p` | same |
| `archive_timeout` | `${PG_ARCHIVE_TIMEOUT_SECONDS:-300}` | same |

**CONFIGURED and internally consistent** with `pgbackrest-conf-common.sh`'s generated `pgbackrest.conf` (same stanza name `hellodoctor`, same `pg1-path=/var/lib/postgresql/data`). Local evidence (`docs/recovery-evidence/m8-pgbackrest-local/pgbackrest-local.json`, `docs/MILESTONE_8_POSTGRES_BACKUP_FINAL.md:84`) records WAL archive-push and archive-get both completing successfully in a disposable container. **Never exercised against a real Azure Blob repository** — every recorded WAL archiving test ran against either `posix` (local-disk) repo type or an ad hoc local container.

Note one real recorded failure in the evidence trail, not papered over: `docs/recovery-evidence/m8-final-local-dr/local-postgres-dr.failed.json` records an earlier attempt (`2026-11-01T03:22:53Z`, 38 seconds before the successful run in `local-postgres-dr.json`) that got as far as `pg_basebackup`/`pg_verifybackup` passing and then failed with `"No archived WAL segments found"` — i.e., the current GREEN evidence is itself downstream of a fixed, previously-broken WAL archiving configuration in the same test fixture, not a first-try success. This is consistent with the project's general practice of keeping failed runs as evidence rather than deleting them, which is good audit hygiene, but it means "GREEN" here rests on one successful run following a fix, not a repeatable track record.

## C. Backup repository (Azure Blob) settings

| Setting | Value / behavior | File:line |
|---|---|---|
| Repo type | `repo1-type=azure` (only emitted when `PGBACKREST_REPO1_TYPE=azure`) | `deploy/single-vm/backup/pgbackrest-conf-common.sh` (`pgbackrest_repo_options`) |
| Account / container | `repo1-azure-account`, `repo1-azure-container` — both `${...:?required}` (hard-fail if unset) | same |
| Endpoint | `repo1-azure-endpoint=${...:-core.windows.net}` | same |
| Key type | `repo1-azure-key-type=${PGBACKREST_REPO1_AZURE_KEY_TYPE:-auto}` — **defaults to managed identity**, only emits a static key line if explicitly overridden | same |
| Second repository | explicitly refused — script exits 1 if any `PGBACKREST_REPO1_...REPO2_*`-style var is set, enforcing the documented single-repository architecture decision | same |

**CONFIGURED, design-consistent.** Managed identity is the default and must be deliberately overridden to use a static key — this matches `docs/AZURE_BLOB_SETUP.md:137` ("Managed identity is the required default outside development/test") and the application's own Blob-storage pattern for medical records (same managed-identity-first posture documented in the backend's object-storage services, confirmed in prior work this session).

**BLOCKED — never tested against real Azure:** no run in this repository's evidence trail has exercised `repo1-type=azure` against a real Storage Account. `docs/MILESTONE_8_POSTGRES_BACKUP_FINAL.md:8-10` states this directly: "Production backup readiness remains PARTIAL because Azure Blob provisioning, managed identity... require the unavailable Azure subscription." `docs/recovery-evidence/RECOVERY_ACCEPTANCE_7C1B.md`'s own outcome table lists `azurite-backup-copy: NOT_EXECUTED — Azurite container setup was not completed` and `azure-cloud-recovery: NOT_EXECUTED — no Azure resources are authorized`. `docs/BACKUP_RESTORE_RUNBOOK.md`'s 2026-11-06 status note states the same Azurite round-trip attempt was abandoned incomplete due to local tooling friction, while noting this is "not a pgBackRest technical blocker" (pgBackRest 2.60.0 does support custom/path-style endpoints per its own help text).

## D. Managed identity / Key Vault integration

| Component | Behavior | File:line |
|---|---|---|
| Token source | Azure IMDS (`http://169.254.169.254/metadata/identity/oauth2/token`), optional `client_id` for user-assigned identity — **no static credential ever used for this call** | `deploy/single-vm/backup/fetch-keyvault-secret.sh` |
| Secret fetch | `https://${vault}.vault.azure.net/secrets/${name}[/${version}]?api-version=7.4`, Bearer token | same |
| Output handling | Secret value → file, mode 0600, owner `postgres:postgres`; secret's versioned `id` (never the value) → separate `.version` file, mode 0644 | same |
| Cipher passphrase resolution order | (1) existing `PGBACKREST_REPO1_CIPHER_PASS_FILE`, (2) Key Vault fetch if name+secret vars set, (3) plain env var (discouraged, documented as visible via `docker inspect`/`exec env`) | `pgbackrest-conf-common.sh` (`resolve_cipher_pass`) |

**CONFIGURED.** The precedence order and file-permission handling are sound and match standard managed-identity practice (no static key/SAS/service-principal secret ever required for Key Vault access).

**BLOCKED — never tested against a real vault.** `docs/BACKUP_ENCRYPTION_KEY_CUSTODY_RUNBOOK.md` confirms the Key Vault retrieval path has unit-level coverage only, against synthetic inputs — never a real Azure Key Vault, real IMDS token issuance, or real RBAC assignment. No evidence file in `docs/recovery-evidence/` records a real Key Vault round-trip.

## E. Encryption at rest

| Property | Value | File:line |
|---|---|---|
| Cipher | AES-256-CBC, pgBackRest repository-level encryption | `pgbackrest-conf-common.sh` (`generate_pgbackrest_conf`): `repo1-cipher-type=aes-256-cbc`, `repo1-cipher-pass=${cipher_pass}` |
| Passphrase on-disk exposure | Confirmed **present by design**: the resolved passphrase is written in plaintext into `/etc/pgbackrest/pgbackrest.conf` (inherent to pgBackRest's own config model, not a defect introduced here) | `docs/BACKUP_ENCRYPTION_KEY_CUSTODY_RUNBOOK.md` (live-evidence-cited) |
| Env-var exposure (separate issue, mitigated) | A plain `PGBACKREST_REPO1_CIPHER_PASS` env var is visible via `docker inspect`/`docker exec env`; the `_FILE` variant is confirmed, via live before/after testing recorded in the runbook, to expose only a path via both commands, never the value | same |

**CONFIGURED and validated locally.** `docs/MILESTONE_8_POSTGRES_BACKUP_FINAL.md:87-88` records both "repository encryption" and "wrong-key rejection" as GREEN against the local disposable fixture (`pgbackrest-local.json`'s `wrong_key: "GREEN: wrong cipher key rejected"` corroborates this).

**Key rotation: explicitly not production-safe as previously documented, now corrected but still never executed.** `docs/BACKUP_ENCRYPTION_KEY_CUSTODY_RUNBOOK.md` states its own earlier version incorrectly described an in-place rotation that does not work against a real pgBackRest repository and must never be attempted. The corrected procedure (add `repo2-*` with a new passphrase, re-run `stanza-create`, full backup to `--repo=2`, verify, cutover, retain the old repo for the full retention window) is documented but **has not been performed even once in a disposable test**, per the same document.

## F. Repository retention policy

| Setting | Value | File:line |
|---|---|---|
| Full backup retention | `repo1-retention-full=7` (days, `repo1-retention-full-type=time`) | `pgbackrest-conf-common.sh` (`generate_pgbackrest_conf`) |
| Differential retention | `repo1-retention-diff=14` | same |
| Env defaults | `PGBACKREST_REPO1_RETENTION_FULL=7`, `PGBACKREST_REPO1_RETENTION_DIFF=14` | `deploy/single-vm/backup/backup-entrypoint.sh` |

**CONSISTENT** between the entrypoint's defaults and the generated config. `docs/MILESTONE_8_POSTGRES_BACKUP_FINAL.md:86` and `pgbackrest-local.json`'s `"retention": "GREEN: retention-full=1 and retention-diff=1 configured"` both record retention cycling exercised locally (note the local test used shortened `1`/`1` values for a fast test cycle — the production defaults of `7`/`14` days have not themselves been exercised through a full real cycle, only through configuration inspection).

**BLOCKED — immutability not implemented.** `docs/BACKUP_RESTORE_RUNBOOK.md` documents a **PROPOSED-ONLY** Azure Blob immutability policy (5 numbered items) with an explicit, repeated warning never to lock immutability without two-person approval, since it is irreversible even against legitimate GDPR erasure obligations. No immutable-retention policy has been applied to any real Azure resource (none exists to apply it to). The interaction between immutable retention and pgBackRest's own expiration/pruning logic is flagged in the runbook as a real, analyzed-but-unresolved risk — pgBackRest expects to be able to delete expired backup files; an immutability lock that prevents deletion before pgBackRest's own retention window closes could cause expiration failures or unexpected storage growth. This has not been tested because no immutable container exists to test it against.

## G. Alerting on backup failure

| Check | Severity | Event code | File:line |
|---|---|---|---|
| `pgbackrest check` (reachability + WAL archiving) | critical | `BACKUP_CHECK_FAILED` | `backup-monitor.sh` |
| Staleness vs. `BACKUP_STALE_MINUTES` (default 30) | critical | `BACKUP_STALE` | same |
| Repository growth vs. `BACKUP_REPO_MAX_GROWTH_MB` (default 2048) | warning | `BACKUP_REPO_EXCESSIVE_GROWTH` | same |
| Local `pg_wal` accumulation vs. `PG_WAL_MAX_SIZE_MB` (default 4096) | warning/critical | `PG_WAL_EXCESSIVE_ACCUMULATION` | same |
| Volume free space vs. `DB_VOLUME_MIN_FREE_PERCENT` (default 15) | critical | `DB_VOLUME_LOW_FREE_SPACE` | same |
| Scheduled backup failure | critical | `BACKUP_FAILED` | `backup-runner.sh` |

**CONFIGURED**, with a real documented bug-and-fix in the evidence trail: `backup-runner.sh`'s original exit-code capture (`if ! cmd; then rc=$?; fi`) captured the *negated* boolean of the `!` operator rather than the command's real exit code, meaning a forced pgBackRest failure (exit 37) was silently reported as success (exit 0) — caught by the project's own forced-failure test (`docs/MILESTONE_8_BACKUP_OPERATIONS_CLOSURE.md` §2) and fixed with explicit `set +e; cmd >log 2>&1; rc=$?; set -e` capture. A second real latent defect is recorded in the same closure doc: `backup-monitor.sh`'s staleness check depends on `python3` for JSON parsing, but `python3` was not installed in `pgbackrest.Dockerfile` until this same remediation pass — meaning the staleness check could not have executed successfully on any earlier build of this image.

Alerting is **opt-in by design**: with no `BACKUP_ALERT_WEBHOOK_URL` configured, every alert still logs locally and the dispatcher exits 0 — nothing claims external delivery until a receiver is configured. `REQUIRE_BACKUP_ALERT_WEBHOOK=true` with an empty webhook URL is a hard startup failure, giving operators a way to force this to be configured before production use.

**BLOCKED — no real alert receiver has ever been configured or tested.** Every "alerting works" claim in the evidence trail is a local webhook-echo or log-only test. No evidence file records a delivered alert reaching a real on-call/paging system.

**Repository-growth monitoring gap, documented and fixed for Azure, not yet exercised:** `backup-monitor.sh`'s repo-growth check correctly switches from local `du` (which cannot see remote storage) to reading pgBackRest's own reported `info[].backup[-1].info.repository.size` when `repo_type=azure` — this fix is present in the script, but since no Azure repository has ever been created, this code path has never actually run against real data.

## H. Alerting / escalation vs. recovery-mode gating (application layer)

| Setting | Behavior | File:line |
|---|---|---|
| `RECOVERY_MODE` | boolean; `true` blocks the notification scheduler from starting, forces FCM to a no-op transport, and makes the global guard reject every external route except health and the token-protected recovery status probe | `services/backend/src/config.rs:618`, `src/api/middleware/global_guard.rs:26`, `src/main.rs:113` |
| `RECOVERY_OPERATOR_TOKEN_SHA256` | required (64-hex-char digest) whenever `RECOVERY_MODE=true`; comparison is constant-time; raw token never persisted | `src/config.rs:619, 796` |
| `RECOVERY_INSTANCE_ID` | required whenever `RECOVERY_MODE=true`, identifies which recovery incident this boot belongs to | `src/config.rs:801` |
| Exit gate | `dr_reconciliation::enforce_recovery_gate` runs on every *normal*-mode boot and refuses to serve ordinary traffic while any recovery instance remains unresolved (`AWAITING_RECONCILIATION`) | `src/services/dr_reconciliation.rs:147-178` |

**CONFIGURED and reasonably strong on paper** — fail-closed defaults, constant-time token comparison, durable Postgres-recorded reconciliation state rather than an in-memory flag. `docs/recovery-evidence/RECOVERY_ACCEPTANCE_7C1B.md` records `recovery-mode-safety: PASSED` (health/operator-status allowed; ordinary auth/clinical/external-effect routes denied) as a real exercised local test, not just a code read.

**Important distinction this audit explicitly verified, per the directive's instruction not to conflate the two "DR" concepts:** `services/backend/src/services/dr_erasure_journal.rs` and `dr_reconciliation.rs` are **data-rights/erasure reconciliation**, not PostgreSQL backup/restore. They implement an externally-custodied, HMAC-chained append-only journal proving which patient-erasure requests completed, so that a PostgreSQL *restore* (which could resurrect a deleted patient's data by rolling back to a point before their erasure was processed) can be detected and corrected (`reconcile_after_restore` re-applies deactivation/session-revocation through existing authorized mutation functions, operator-CLI-only, never HTTP-reachable). This module's own doc comment states its real limitation plainly: it currently only writes to a filesystem path, which is not independently protected from the same restore event that would roll back Postgres — production must point `EXTERNAL_ERASURE_JOURNAL_PATH` at a volume outside the Postgres backup domain, and the intended end state (Azure immutable Blob) is **not yet implemented**. This is a second, independent BLOCKED item beyond the Postgres-repository immutability gap in §F — the two gaps compound (an attacker or an operator error that destroys both the Postgres backup's immutability and the erasure journal's separate custody would be unrecoverable against GDPR erasure guarantees) but are architecturally distinct concerns and must not be reported as the same gap.

## I. Recovery mode: three-phase model

RECOVERY → ISOLATED ACCEPTANCE → NORMAL, per `docs/BACKUP_RESTORE_RUNBOOK.md`'s ~11-item gate checklist, requiring a second operator's approval to exit recovery. `dr_reconciliation.rs`'s module doc independently confirms the same three-phase shape at the code level (`mark_recovery_boot` / `enforce_recovery_gate` / `reconcile_after_restore`), and `RECOVERY_ACCEPTANCE_7C1B.md`'s `recovery-mode-safety: PASSED` is real exercised evidence of the ISOLATED ACCEPTANCE phase's route-blocking behavior specifically — not the full three-phase transition end to end.

**BLOCKED — no recorded evidence of a full RECOVERY → ISOLATED ACCEPTANCE → NORMAL transition being exercised in sequence**, with a second operator's approval, against a restored (not merely booted-in-recovery-mode) database.

## J. Restore procedures (what is, and is not, real evidence)

Recorded local evidence, in order of how much it actually proves:

- `docs/recovery-evidence/m8-final-local-dr/local-postgres-dr.json` (2026-11-01T03:23:31Z): `pg_basebackup`/`pg_verifybackup` GREEN, 6 archived WAL segments observed, PITR promoted correctly at a named restore point with pre-boundary row present / post-boundary row absent — this is a real, successful physical PITR drill, but explicitly marks `retention: BLOCKED` (no production scheduler/retention policy existed in that fixture), `encryption: BLOCKED`, and `azure_storage: BLOCKED` (no Azure credentials available). This evidence is about the *mechanism* (pg_basebackup/WAL replay), predating the later pgBackRest-specific work.
- `docs/recovery-evidence/m8-pgbackrest-local/pgbackrest-local.json` + `restore-sql-evidence.json` (dated ~2026-11-01T09:47): the pgBackRest-specific restore drill — full backup, encrypted repository, wrong-key rejection, PITR via `pgbackrest restore --type=time`, timeline promotion, a shortened cron cycle completing a differential backup — all GREEN, with the actual restored PostgreSQL log output captured verbatim (not summarized) in `restore-sql-evidence.json`, including the real `recovery stopping at restore point "m8-boundary"` line. This is the strongest evidence in the repository, and it is real: an actual encrypted repository was restored into an actual fresh PostgreSQL instance and the result was actually queried.
- `docs/recovery-evidence/RECOVERY_ACCEPTANCE_7C1B.md` (2026-10-29, earlier/broader acceptance pass): PASSED on PITR, recovery-mode-safety, erasure-journal-integrity, encrypted-object-recovery; explicitly `NOT_EXECUTED` for full-seeded-application-reconstruction, Azurite-backup-copy, and azure-cloud-recovery. Overall outcome recorded as **PARTIAL**, not PASS.

**What every one of these runs has in common, stated explicitly in each source document:** all restores were performed against a **disposable local/containerized target**, never a real Azure-hosted repository, never a replacement VM standing in for a lost production host, and the authoritative runbook's own "What not to do" list states this should never be mistaken for disaster-recovery certification. `docs/BACKUP_RESTORE_RUNBOOK.md` names the still-open gates explicitly: real managed-identity/RBAC against a real Storage Account (blocked on subscription availability), a replacement-VM restore drill with measured RPO/RTO (also blocked), and the immutable-retention-vs-expiration interaction (analyzed, not tested).

**BLOCKED — no replacement-VM or regional-failure restore drill has ever been performed**, local or real. `docs/DISASTER_RECOVERY_PLAN.md:7` states a Replacement VM RTO target of "1 hour or better" but this number has never been measured against any actual drill — it is a target, not a demonstrated result, for either the Flexible Server topology the plan describes or the single-VM topology the repository actually builds.

---

## Milestone-9-required classification summary

| Area | Status | Evidence basis |
|---|---|---|
| PostgreSQL 16 version consistency across configs | **PASS** | Digest matches across 3 files (§A) |
| Custom pgBackRest image publication | **BLOCKED** | Placeholder digest only (§A) |
| WAL archiving configuration | **PASS (local only)** | Local disposable fixture GREEN (§B); real Azure never attempted |
| Backup repository (Azure Blob) config | **CONFIGURED, BLOCKED for production trust** | Code/config sound; zero real-Azure runs (§C) |
| Managed identity / Key Vault | **CONFIGURED, BLOCKED for production trust** | Synthetic unit tests only (§D) |
| Encryption at rest | **PASS (local), key rotation BLOCKED** | GREEN locally; rotation procedure never executed even once (§E) |
| Retention policy | **CONFIGURED, immutability BLOCKED** | Time-based retention works locally; immutability proposed-only (§F) |
| Alerting on failure | **CONFIGURED, BLOCKED for production trust** | Two real bugs found+fixed in-repo; no real receiver ever tested (§G) |
| Recovery-mode application gating | **PASS (local)** | Real exercised test, `RECOVERY_ACCEPTANCE_7C1B.md` (§H/I) |
| Erasure-journal/DR distinction | **CONFIRMED DISTINCT, journal custody BLOCKED** | Filesystem-only today; Azure immutable Blob not implemented (§H) |
| Full 3-phase recovery transition | **BLOCKED** | No end-to-end drill recorded (§I) |
| Restore procedures | **PASS (local/disposable only)** | Strongest evidence in repo, but explicitly never off-VM (§J) |
| Replacement-VM / regional restore drill | **BLOCKED** | Never attempted, local or real (§J) |
| **Architecture documentation consistency** | **FAIL — unreconciled** | `DISASTER_RECOVERY_ARCHITECTURE.md`'s own "Decision" section contradicts the actual shipped single-VM topology (Headline finding, above) |

---

## Exact remaining tests before real disaster-recovery trust can be claimed

1. Reconcile `docs/DISASTER_RECOVERY_ARCHITECTURE.md`'s "Decision" section (and `MILESTONE_7C_DR_REPORT.md`, `DISASTER_RECOVERY_PLAN.md`) with the actual single-VM/self-hosted-PostgreSQL+pgBackRest topology that `deploy/single-vm/` builds — a documentation task, not a technical one, but a prerequisite to any readiness sign-off.
2. Build and publish the real `POSTGRES_BACKUP_IMAGE` with a reviewed digest, replacing the placeholder in `deploy/single-vm/arm64-images.env.example`.
3. Provision a real Azure Storage Account + container, and run `stanza-create`/`check`/a real full backup with `repo1-type=azure` and `repo1-azure-key-type=auto` against a VM's actual system-assigned managed identity (not emulated/local).
4. Provision a real Azure Key Vault, grant the VM's managed identity `get` on the cipher-passphrase secret, and exercise `fetch-keyvault-secret.sh` against it for real.
5. Perform one full restore onto a genuinely separate replacement VM/host (not the same disposable container the backup ran on), and measure actual RTO against the plan's "1 hour" / "4 hours" targets.
6. Configure a real alert-receiving endpoint and confirm `BACKUP_FAILED`/`BACKUP_STALE`/`BACKUP_CHECK_FAILED` actually page someone.
7. Execute the documented repo2-based key-rotation procedure once, end to end, against a disposable (not production) repository, per `BACKUP_ENCRYPTION_KEY_CUSTODY_RUNBOOK.md`'s own corrected procedure.
8. Decide and test the Azure Blob immutability policy's interaction with pgBackRest's own retention/expiration, against a real (not proposed-only) immutable container.
9. Point `EXTERNAL_ERASURE_JOURNAL_PATH` at infrastructure genuinely outside the Postgres backup domain (today it is documented as a gap, not yet closed) and exercise `reconcile_after_restore` against a real restore.
10. Run the full RECOVERY → ISOLATED ACCEPTANCE → NORMAL transition once, end to end, with a second operator's sign-off, against a database that was actually restored (not merely booted with `RECOVERY_MODE=true` and never restored).

## What the brief asked about that could not be found or confirmed

- No evidence file in this repository records any backup, restore, or Key Vault operation ever executed against real Azure infrastructure — every GREEN result in `docs/recovery-evidence/` and the Milestone 8 docs is local/disposable/synthetic. This audit could not find a single exception.
- No recorded measurement of actual RPO/RTO exists for either topology (Flexible Server or single-VM) — both plan documents state targets, not measured results.
- No evidence of the Azure Blob immutability policy ever being applied to a real container (the policy itself is explicitly proposed-only, per the runbook).
- No tabletop or live key-rotation drill has been run even once, per `BACKUP_ENCRYPTION_KEY_CUSTODY_RUNBOOK.md`'s own explicit statement.
