# Backup and restore runbook

## 2026-11-06 status: code-complete locally, Azure integration still unproven

An external audit of this layer's Azure readiness reached a BLOCKED verdict
for relying on it to protect real production data, while confirming the
local pgBackRest/PITR design itself is sound. The engineering gaps it found
have been fixed in this repository (see the dated notes throughout this
document and `deploy/single-vm/backup/`), but fixing code is not the same as
proving Azure integration — the following remain genuinely unverified and are
**not** closed by anything in this update:

- Real Azure managed-identity token acquisition and container-scoped RBAC
  (`repo1-azure-key-type=auto`) against an actual Storage Account — still
  requires the Azure subscription (M8-G-04, blocked).
- Azure Key Vault secret retrieval via managed identity — now implemented
  (`fetch-keyvault-secret.sh`) and its logic unit-tested against synthetic
  inputs, but never exercised against a real Key Vault.
- Storage firewall / private-endpoint network reachability from the actual
  production VM.
- Immutable-retention policy interaction with pgBackRest's own expiration
  (a real risk: an immutability lock can reject the delete pgBackRest issues
  when a backup set ages out).
- Replacement-VM restore with measured RPO/RTO (M8-G-18, blocked).
- An Azurite-emulated round-trip (upload/backup/restore against pgBackRest's
  Azure driver without real Azure credentials) was attempted in this session
  and abandoned incomplete due to local Docker/Windows tooling friction
  (slow image pulls, path-translation issues), not a technical blocker in
  pgBackRest itself — confirmed directly: `pgbackrest help backup
  repo1-azure-uri-style` and `repo1-azure-endpoint` show pgBackRest 2.60.0
  officially supports pointing the Azure driver at a custom endpoint with
  path-style URIs, which is exactly what an emulator round-trip needs. This
  remains a worthwhile next step — it would be the first actual evidence of
  the Azure driver round-tripping data, short of real Azure access — but it
  has not been done.

Do not read "the audit's findings were fixed" as "Azure disaster recovery is
proven." The safest reading is: the code is now ready to be pointed at a real
Azure subscription for the first time; that first real round-trip has not
happened yet.

## Normal protection

The self-hosted PostgreSQL deployment uses the opt-in pgBackRest layer under
`deploy/single-vm/backup/`. It is enabled only after a dedicated repository,
encryption secret, stanza check and monitor are ready. The base compose file
does not turn on WAL archiving by itself.

### Initial activation

Build the reviewed PostgreSQL image and provision dedicated, private volumes
for the repository, generated configuration, logs and the PostgreSQL socket:

```powershell
docker build -f deploy/single-vm/backup/pgbackrest.Dockerfile -t hellodoctor-pgbackrest:approved deploy/single-vm/backup
docker volume create hellodoctor-pgbackrest-repo
docker volume create hellodoctor-pgbackrest-config
docker volume create hellodoctor-pgbackrest-logs
docker volume create hellodoctor-pgbackrest-socket
```

Supply `PGBACKREST_REPO1_CIPHER_PASS` (or `PGBACKREST_REPO1_CIPHER_PASS_FILE`,
or Azure Key Vault — see "Encryption-key retrieval" below) from the approved
secret manager and set `POSTGRES_BACKUP_IMAGE`, `PGBACKREST_*_VOLUME` and the
normal deployment variables. Run `docker compose -f
deploy/single-vm/compose.yml -f deploy/single-vm/compose.backup.yml config
--quiet`, then start the stack only after reviewing the rendered
configuration. The database entrypoint writes the pgBackRest config before
PostgreSQL starts; the scheduler then runs `check` and refuses to continue on
a repository or archive failure.

Default policy is one full backup at 03:15 UTC, differential backups every 15
minutes, seven full generations by time, fourteen differential generations,
one pgBackRest process, a 300-second (5-minute) `archive_timeout`, and a
30-minute stale-backup alert. These are operator defaults, not fixed product
requirements; change them only after approving the RPO/RTO and storage
budget.

**2026-11-06 follow-up (addressing the external Azure-readiness audit):**

- `PG_ARCHIVE_TIMEOUT_SECONDS` (default 300) bounds how long a partial WAL
  segment can sit unarchived during low-write periods. Without this, "WAL
  archiving is on" only meant "completed segments get archived" — a quiet
  period could silently widen the real RPO well past the documented target.
  This is a ceiling, not a guarantee: archival delays, failed uploads, or a
  slow repository can still push actual data loss higher. Treat
  `pgbackrest check` and the staleness alert as the real gates, not this
  setting alone.
- `BACKUP_STALE_MINUTES` default lowered from 120 to 30 (two missed
  differentials at the configured 15-minute cadence), consistent with the
  documented RPO target while still tolerating one transient failure.
- `PGBACKREST_ARCHIVE_ASYNC` / `PGBACKREST_SPOOL_PATH` add optional
  asynchronous WAL archiving. **Default is `n` (off, unchanged behavior)** —
  the audit correctly flagged this as a reliability improvement that
  introduces its own failure mode (spool disk exhaustion if the repository is
  unreachable for an extended period) and must be exercised under a
  disposable failure drill before being relied on. Do not set
  `PGBACKREST_ARCHIVE_ASYNC=y` in production until that drill has been run.
- `REQUIRE_BACKUP_ALERT_WEBHOOK=true` turns "no alert receiver configured"
  from a silent local-only no-op into a hard container-startup failure. Set
  this only for the real production deployment, after
  `BACKUP_ALERT_WEBHOOK_URL` has been pointed at a tested receiver — this is
  what makes alert delivery non-optional for production rather than merely
  recommended.

Inspect status with:

```sh
docker compose -f deploy/single-vm/compose.yml -f deploy/single-vm/compose.backup.yml exec backup pgbackrest --stanza=hellodoctor check
docker compose -f deploy/single-vm/compose.yml -f deploy/single-vm/compose.backup.yml exec backup pgbackrest --stanza=hellodoctor info
docker compose -f deploy/single-vm/compose.yml -f deploy/single-vm/compose.backup.yml logs backup
```

The scheduler uses a `flock` lock under a writable runtime path, so a slow full
backup cannot overlap a differential run. The monitor checks repository health
and backup age every 15 minutes. `BACKUP_RUN_ON_START=true` exists only for a
disposable verification container; production leaves it false.

### Alert routing (G-05 — closed 2026-11-01, extended 2026-11-01)

`backup-runner.sh` and `backup-monitor.sh` call
`deploy/single-vm/backup/alert-notify.sh` on: a failed scheduled backup, a
failed `pgbackrest check` (WAL-archiving failure or unreachable repository), a
stale backup, excessive repository disk growth, excessive PostgreSQL `pg_wal`
accumulation, and low free space on the PostgreSQL data volume — the last two
added in the same-day follow-up after confirming the original pass only
checked the backup repository's own disk usage, not PostgreSQL's `pg_wal`
directory or the data volume itself (a full data volume halts PostgreSQL
regardless of backup health). See `deploy/single-vm/backup/README.md`'s
"Alerting" section for the exact env vars and delivery contract, and
`docs/MILESTONE_8_BACKUP_OPERATIONS_CLOSURE.md` for the disposable-receiver
test evidence, including the new checks. Alerting is opt-in and local-only
until `BACKUP_ALERT_WEBHOOK_URL` is pointed at a real, tested production
receiver — do not treat this closure as proof that a real on-call channel has
been wired up.

For Azure Blob, set `PGBACKREST_REPO1_TYPE=azure`, the account, container and
endpoint variables. pgBackRest 2.60 accepts `repo1-azure-key-type=auto`, which
is the managed-identity-capable setting documented by pgBackRest; the template
therefore defaults to `PGBACKREST_REPO1_AZURE_KEY_TYPE=auto` and omits a static
key in that mode. Use `shared` or `sas` only when an approved secret manager
delivers the corresponding key. The local check confirmed that the installed
2.60 build accepts the `auto` option. Azure RBAC, managed-identity assignment
and restore remain blocked until the subscription is available.

**Version pin (2026-11-06 follow-up).** `deploy/single-vm/backup/pgbackrest.Dockerfile`
previously declared `PGBACKREST_VERSION=2.54` as a Dockerfile `ARG` but never
used it in the `apt-get install` line — the installed version floated to
whatever PGDG's bookworm repository had published on build day (confirmed
locally installed: 2.60.0), an unpinned, irreproducible build. The Dockerfile
now installs an exact, verified-available version
(`pgbackrest=2.60.0-1.pgdg12+1`, confirmed via `apt-cache madison pgbackrest`
against this exact base image) and the build fails outright if that version
is not what gets installed. Before bumping this pin, re-run that same
`apt-cache madison` check — PGDG does not keep every historical version
published indefinitely — and re-run
`ops/dr/verify_local_pgbackrest.ps1` afterward.

**Single-repository architecture (2026-11-06 follow-up — addresses the
audit's primary structural finding).** This deployment references two Azure
containers (`pg-basebackups`, `pg-wal-archive`), but pgBackRest's repository
model does not let two containers cleanly split "base backups only" from
"WAL only": a repository holds both, and `archive-push` sends WAL to *every*
configured repository. A second repository would duplicate WAL into both
containers, not give an independent split. **Decision: use `pg-basebackups`
as the sole pgBackRest repository (`repo1`) and leave `pg-wal-archive`
unused** until a separately tested, explicitly-approved two-repository design
exists (see `docs/BACKUP_ENCRYPTION_KEY_CUSTODY_RUNBOOK.md`'s key-rotation
section for the one scenario — a passphrase migration — where a second
repository is the documented, correct mechanism).
`deploy/single-vm/backup/pgbackrest-conf-common.sh` enforces this
operationally: if any `PGBACKREST_REPO2_*` variable is set, the container
refuses to start with an error pointing back to this section, rather than
silently ignoring it and leaving an operator to wrongly believe a second
repository is active.

**Encryption-key retrieval (2026-11-06 follow-up).** Azure Key Vault
integration — previously documented as a requirement but not implemented —
now exists: `deploy/single-vm/backup/fetch-keyvault-secret.sh` uses the
container's own Azure managed identity (via Instance Metadata Service, no
account key or service-principal secret involved) to retrieve
`PGBACKREST_REPO1_CIPHER_PASS_FILE`'s content from Key Vault when
`AZURE_KEY_VAULT_NAME` and `AZURE_KEY_VAULT_SECRET_NAME` are set and that file
does not already exist. This automates only the *routine* case — the same
long-lived VM restarting its containers, where its identity has already been
granted `Get` on this one secret. It does not change who can grant that
access, who can read or rotate the secret's value through Key Vault's own
RBAC, or the manual custodian procedure a *replacement* VM's new identity
still needs during a real VM-loss recovery — all of that remains exactly as
`docs/BACKUP_ENCRYPTION_KEY_CUSTODY_RUNBOOK.md` describes. The secret's full
versioned identifier (never its value) is recorded alongside each backup's
recovery manifest (`/var/log/pgbackrest/recovery-manifest-latest.json` inside
the `backup` container, written by `backup-runner.sh` after every successful
backup). This has been unit-tested against synthetic IMDS/Key Vault response
shapes and against IMDS being unreachable (fails loudly, as it will on any
non-Azure host); it has **not** been exercised against a real Key Vault.

1. Provision Flexible Server with PostgreSQL 16 compatibility, private access,
   zone-redundant HA where available, geo-redundant backup selected at
   creation, and retention appropriate to the legal record policy (initially
   35 days for the pilot review).
2. Enable Azure Backup for the server where the selected service/region supports
   the required restore path. Use a separate vault, independent backup
   operator, soft delete and a reviewed immutability policy. Do not lock
   immutability during this local milestone.
3. Monitor backup freshness, WAL/log lag, storage, failed jobs, retention
   changes, vault deletion attempts and the last successful restore drill.
4. Keep the application encryption key outside the database and Blob account;
   retain key version and custody metadata in the recovery manifest — see
   `docs/BACKUP_ENCRYPTION_KEY_CUSTODY_RUNBOOK.md`.

### Retention and immutability policy (G-17 — closed 2026-11-01)

**Intended retention schedule — matches the current pgBackRest configuration
verbatim** (`deploy/single-vm/backup/backup-entrypoint.sh`):

| Setting | Value | Source |
|---|---|---|
| Full backup cadence | Daily, 03:15 UTC | `BACKUP_FULL_SCHEDULE=03:15` |
| Differential backup cadence | Every 15 minutes | `BACKUP_DIFF_SCHEDULE=*/15` |
| Full-generation retention | 7, by time (`repo1-retention-full-type=time`) | `PGBACKREST_REPO1_RETENTION_FULL=7` |
| Differential-generation retention | 14 | `PGBACKREST_REPO1_RETENTION_DIFF=14` |
| Staleness alert threshold | 30 minutes (lowered 2026-11-06 from 120; see "Initial activation" above) | `BACKUP_STALE_MINUTES=30` |
| WAL archive timeout | 300 seconds (added 2026-11-06) | `PG_ARCHIVE_TIMEOUT_SECONDS=300` |
| Repository encryption | AES-256-CBC | `PGBACKREST_REPO1_CIPHER_TYPE=aes-256-cbc` |

These are operator defaults (per "Initial activation" above), not fixed
product requirements — change the two retention variables (and this table)
together if the RPO/storage-budget decision changes; they must never silently
drift apart.

**Backup expiration behavior.** pgBackRest expires old full/differential sets
automatically as part of every successful `backup` command run, based on the
`repo1-retention-full`/`repo1-retention-diff` settings above — no separate
`expire` cron job exists or is needed in normal operation. A differential set
is only removable once its parent full backup falls outside the full-retention
window; pgBackRest enforces this dependency itself and will not expire a full
backup that a still-retained differential depends on.

**Repository access restrictions (current, local).** The repository path is
created `0750 postgres:postgres`
(`install -d -m 0750 -o postgres -g postgres ... /var/lib/pgbackrest/repo1`,
`backup-entrypoint.sh`); only the `postgres` user inside the dedicated `backup`
container can read or write it. The repository lives on its own named Docker
volume (`pgbackrest-repo`), never the application's `pgdata` volume, and is
never exposed through any network port. Contents are encrypted at rest with a
cipher passphrase that must come from a secret store, never `.env` or a
committed file. Prefer supplying it via `PGBACKREST_REPO1_CIPHER_PASS_FILE`
(a file path, e.g. a Docker/Swarm secret mount) rather than the plain
`PGBACKREST_REPO1_CIPHER_PASS` env var — confirmed directly that the plain
env var is visible in plaintext to `docker inspect`/`docker exec ... env`,
while the `_FILE` variant exposes only a path through those same commands.
Neither mechanism protects against host-root/Docker-daemon access to the
container's filesystem itself — see
`docs/BACKUP_ENCRYPTION_KEY_CUSTODY_RUNBOOK.md` for the full exposure
analysis and the still-manual key-rotation procedure (there is no in-place
rotation; a new repository is required).

**Proposed Azure Blob immutability requirements (written proposal only — NOT
enacted).** When the Azure subscription becomes available:

1. A dedicated Storage Account and container used for nothing else but this
   repository; no public network access; access only through a VM-scoped
   managed identity granted `Storage Blob Data Contributor` on that single
   container (not the account), per the existing least-privilege pattern used
   for the application's own PostgreSQL capability roles.
2. Enable Blob *versioning* and a **time-based retention policy** (Azure's
   WORM/immutability feature) scoped to the container, with a retention
   interval chosen to exceed the full-backup retention window above (e.g. 10
   full-backup cycles' worth of calendar time) plus a safety margin for
   detecting a missed drill.
3. **Do not enable the "allow protected append" lock or move the policy from
   *unlocked* to *locked* state without a separate, explicit, two-person
   approval** — an Azure immutability lock is irreversible for the configured
   retention period even against the account owner, including against a
   legitimate future deletion (e.g. GDPR/data-subject erasure of the whole
   environment, or a provider migration). This is a deliberate, high-stakes,
   operator-level decision, not a default to turn on alongside the rest of
   this checklist.
4. Combine with Azure Storage's own soft-delete (separate from the
   immutability policy) as a lower-stakes recovery path for accidental
   deletion before anyone commits to locking immutability.
5. Record the container's resource ID, the chosen retention interval, the
   lock state (unlocked/locked), and the approving operators in the recovery
   manifest the same way the encryption-key version is recorded.

No Azure resource is provisioned by this document. This section is the
written policy requested for engineering sign-off; enactment remains BLOCKED
on Azure subscription access, per
`docs/MILESTONE_8_REMAINING_GAP_ASSESSMENT.md` (M8-G-04).

## Read-only inventory

```powershell
$env:HELLODOCTOR_DR_ROOT = 'C:\helldoc-dr-owned'
python ops/dr/hellodoctor_dr.py inventory
python ops/dr/hellodoctor_dr.py manifest docs
```

The inventory never prints secret values. Docker volumes and persistent
PostgreSQL/Redis data are not deleted by this tool.

## Safe isolated restore

```powershell
New-Item -ItemType Directory C:\helldoc-dr-owned\restore-2026-01 -Force
'HELLODOCTOR-DISPOSABLE-TARGET-V1' | Set-Content C:\helldoc-dr-owned\restore-2026-01\.hellodoctor-disposable-target
$env:HELLODOCTOR_DR_ROOT = 'C:\helldoc-dr-owned'
python ops/dr/hellodoctor_dr.py restore-plan C:\helldoc-dr-owned\restore-2026-01
```

The plan is dry-run. A real `pg_restore` requires `PGTARGET_DSN` from a
disposable PostgreSQL target and an explicit `--confirm-disposable`; the tool
refuses unmarked or outside targets and exits on the first restore error.

## Application recovery checklist

Recovery has three deliberately separate phases:

1. **RECOVERY** — `RECOVERY_MODE=true`; ordinary authentication, clinical
   reads, WebSockets and all external-effect routes are denied. Only health and
   the token-protected recovery status probe are reachable.
2. **ISOLATED ACCEPTANCE** — a separately network-isolated disposable
   deployment is started from the reconstructed database and synthetic data.
   Test clients use that private endpoint only; no public ingress or live
   provider credentials are enabled.
3. **NORMAL** — after reconciliation and acceptance evidence are reviewed, an
   authorized operator changes configuration and restarts the service. There is
   no ordinary HTTP endpoint that disables recovery mode.

- [ ] `RECOVERY_MODE=true` and no live FCM/payment/campaign credentials are
  active in the recovery process.
- [ ] `RECOVERY_OPERATOR_TOKEN_SHA256` is a 64-hex digest for a separately
  delivered, short-lived operator token; the raw token is not logged or stored.
- [ ] Encryption key availability is verified without logging the key.
- [ ] Roles, grants, RLS, functions, extensions and migrations match the binary.
- [ ] Erasure journal prevents restored sessions/devices/accounts becoming usable.
- [ ] Provider-side payments/refunds/disbursements are reconciled, not replayed.
- [ ] FCM outbox is reconciled using claim/fencing state, not bulk resent.
- [ ] Blob references are checked by container/key/hash and only authorized
  objects are recovered.
- [ ] Health and the recovery-status probe pass while ordinary authentication,
  clinical reads, WebSockets and external effects remain blocked.
- [ ] In the isolated acceptance deployment, synthetic authentication, article,
  document and read-only workflow checks pass without live provider activity.
- [ ] A second authorized operator approves disabling recovery mode.

## What not to do

Never restore over a production database by changing a string, copy a running
PostgreSQL data directory, restore every deleted Blob automatically, replay the
payment/refund/notification outbox, or declare recovery from a backup-job
success alone.
