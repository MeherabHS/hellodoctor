# Milestone 8 PostgreSQL Backup Final

Date: 2026-11-01

Overall status: **PARTIAL**

Local PostgreSQL backup and restore acceptance is **GREEN**. Production backup
readiness remains **PARTIAL** because Azure Blob provisioning, managed identity
assignment, off-VM retention, alert routing and replacement-VM recovery still
require the unavailable Azure subscription.

## Scope completed

The approved single-VM topology now has an opt-in pgBackRest deployment layer:

- PostgreSQL 16 image with pgBackRest 2.60 and cron.
- Continuous WAL archiving through `archive_command`.
- Configurable full and differential backup schedules.
- Time-based full and differential retention.
- AES-256-CBC repository encryption.
- `flock` concurrency protection for backup jobs.
- Stale-backup and repository health monitoring.
- Dedicated repository, configuration, log and socket volumes.
- Azure Blob repository configuration with secret-managed credentials.
- Azure `repo1-azure-key-type=auto` support for the managed-identity-capable
  configuration documented by pgBackRest.

The layer is enabled only through `deploy/single-vm/compose.backup.yml`.
The ordinary single-VM Compose configuration does not enable archiving or
create a backup repository implicitly.

## Files created or modified

- `deploy/single-vm/backup/pgbackrest.Dockerfile`
- `deploy/single-vm/backup/postgres-entrypoint.sh`
- `deploy/single-vm/backup/backup-entrypoint.sh`
- `deploy/single-vm/backup/backup-runner.sh`
- `deploy/single-vm/backup/backup-monitor.sh`
- `deploy/single-vm/compose.backup.yml`
- `ops/dr/verify_local_pgbackrest.ps1`
- `ops/dr/diagnose_pgbackrest_restore.ps1`
- `docs/BACKUP_RESTORE_RUNBOOK.md`
- `docs/DISASTER_RECOVERY_ARCHITECTURE.md`
- `docs/recovery-evidence/m8-pgbackrest-local/`

Existing Claude and Codex changes, PostgreSQL data, Redis data, benchmark
evidence and shared running services were preserved.

## Restore failure diagnosis and correction

The original disposable target failure was reproduced from PostgreSQL logs.

1. The restored cluster contained recovery state, while the harness supplied
   an empty `restore_command`. PostgreSQL exited with code 1 and logged:
   `must specify restore_command when standby mode is not enabled`.
2. After adding pgBackRest `archive-get`, the harness created the named restore
   point before the base backup. Its LSN preceded the backup recovery start,
   so PostgreSQL exited with code 1 and logged:
   `recovery ended before configured recovery target was reached`.

The smallest correction was to create the full backup first, create the named
restore point afterward, archive subsequent WAL, and start the target with the
encrypted repository plus:

```text
restore_command = 'pgbackrest --stanza=hellodoctor archive-get %f %p'
```

The source and restored target also received separate Unix-socket volumes so
the scheduler could not accidentally inspect the promoted recovery target.

## Local verification evidence

The final disposable run is recorded in:

- `docs/recovery-evidence/m8-pgbackrest-local/pgbackrest-local.json`
- `docs/recovery-evidence/m8-pgbackrest-local/restore-sql-evidence.json`

| Verification | Status | Result |
|---|---|---|
| pgBackRest image build | GREEN | PostgreSQL 16 image with pgBackRest 2.60 built successfully |
| Stanza creation and repository check | GREEN | `stanza-create` and `check` completed |
| Full backup | GREEN | Full backup completed before the recovery boundary |
| WAL archiving | GREEN | WAL segments were archived and retrieved through pgBackRest |
| Differential backup | GREEN | Shortened scheduler run completed a differential backup |
| Retention | GREEN | Retention policy was configured and successive cycles were exercised |
| Repository encryption | GREEN | AES-256-CBC repository used without exposing the passphrase |
| Wrong-key rejection | GREEN | Incorrect cipher key was rejected |
| Repository failure | GREEN | Read-only repository caused `check` to fail |
| Restore to fresh PostgreSQL | GREEN | New target started and accepted SQL connections |
| PITR boundary | GREEN | `before-boundary` exists; `after-boundary` is absent |
| Recovery/promotion state | GREEN | `pg_is_in_recovery()` returned `false` |
| Restore logs | GREEN | WAL retrieval, restore-point stop and timeline promotion recorded |
| Scheduler monitoring | GREEN | Shortened monitor schedule completed repository checks |
| Job overlap protection | GREEN | `flock` lock executed successfully as `postgres` |

The earlier physical `pg_basebackup` and PITR evidence remains preserved in
`docs/recovery-evidence/m8-final-local-dr/`.

## Scheduler defaults

Production defaults remain configurable and unchanged in intent:

- Full backup: 03:15 UTC.
- Differential backup: every 15 minutes.
- Full retention: seven generations by time.
- Differential retention: fourteen generations.
- Backup staleness threshold: 120 minutes.
- pgBackRest process count: one.

`BACKUP_RUN_ON_START=true` is available only for disposable verification. It is
false by default and must remain false in production.

## Azure capability and remaining blockers

The installed pgBackRest 2.60 build accepts `repo1-azure-key-type=auto`. The
templates now default to `auto`, omitting a static Azure key in that mode.
`shared` and `sas` remain available when a secret manager supplies the required
key. Local option acceptance is verified; actual Azure authentication is not.

Remaining production gates:

- **BLOCKED:** Azure Blob repository provisioning and RBAC.
- **BLOCKED:** Azure managed identity assignment and real Blob authentication.
- **BLOCKED:** Off-VM retention and restore verification.
- **BLOCKED:** Production alert sink integration for stale backups, failed
  jobs, WAL failures and local disk/WAL accumulation.
- **BLOCKED:** Replacement-VM restore drill and measured RPO/RTO.
- **PARTIAL:** Production encryption-key custody approval and recovery-key-loss
  procedure.

Loss of the repository encryption key makes encrypted backups unrecoverable.
The repository must remain independent from the PostgreSQL data volume.

## Acceptance verdict

**Local engineering acceptance: GREEN.**

**Production backup acceptance: PARTIAL.** Production readiness cannot be
declared until Azure off-VM backup, identity/RBAC, alerting and replacement-VM
restore evidence are available. No shared database or Docker volume was reset;
no files were staged, committed or pushed.
