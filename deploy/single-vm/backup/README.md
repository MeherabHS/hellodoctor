# PostgreSQL pgBackRest activation

This layer is deliberately opt-in. The production PostgreSQL service must not
start WAL archiving until the repository, cipher key, permissions and monitor
have been validated.

## Local repository activation

1. Build `deploy/single-vm/backup/pgbackrest.Dockerfile` as the reviewed
   PostgreSQL image (it contains `pgbackrest` and is used by both the database
   and backup scheduler containers).
2. Provision a dedicated repository volume or an approved private Azure Blob
   container. Never use the application data volume as the only repository.
3. Supply `PGBACKREST_REPO1_CIPHER_PASS` through Docker secrets/Key Vault; it
   must not appear in `.env`, logs, or compose files.
4. Run `docker compose -f compose.yml -f compose.backup.yml config` and then
   `up -d` only after `pgbackrest check` is green.

The defaults are seven full backups, fourteen differential generations,
15-minute differential scheduling, a 03:15 UTC full backup, one pgBackRest
process, zstd level 3 compression, a 300-second `archive_timeout`
(`PG_ARCHIVE_TIMEOUT_SECONDS`) and a 30-minute staleness alert
(`BACKUP_STALE_MINUTES`, lowered 2026-11-06 from 120). Adjust these through
environment/secrets after approving RPO/RTO and storage costs.

Optional, off by default: `PGBACKREST_ARCHIVE_ASYNC=y` +
`PGBACKREST_SPOOL_PATH` for asynchronous WAL archiving — do not enable in
production before running a disposable failure drill (spool disk exhaustion
if the repository is unreachable is a new failure mode this introduces).
`REQUIRE_BACKUP_ALERT_WEBHOOK=true` makes a missing alert receiver a hard
startup failure instead of a silent no-op — set this only for the real
production deployment.

## Azure Blob

Set `PGBACKREST_REPO1_TYPE=azure`, `PGBACKREST_REPO1_AZURE_ACCOUNT`,
`PGBACKREST_REPO1_AZURE_CONTAINER`, and the pgBackRest Azure SAS/managed-identity
credential mechanism supported by the reviewed pgBackRest release. Prefer a
VM managed identity with narrowly scoped Blob Data Contributor access to a
dedicated container. Do not place account keys in this repository. Azure
subscription provisioning, RBAC and restore are still pending in this
environment.

**Single repository only (2026-11-06).** Use one container (`pg-basebackups`)
as `repo1`; a second container cannot be used as an independent WAL-only
repository under pgBackRest's model (it would duplicate WAL into both, not
split them) — see `docs/BACKUP_RESTORE_RUNBOOK.md`'s "Single-repository
architecture" section. Setting any `PGBACKREST_REPO2_*` variable makes the
container refuse to start, rather than silently doing nothing.

**Automated Key Vault retrieval (2026-11-06).** Set `AZURE_KEY_VAULT_NAME` +
`AZURE_KEY_VAULT_SECRET_NAME` (optionally `AZURE_KEY_VAULT_SECRET_VERSION`,
`AZURE_CLIENT_ID` for a user-assigned identity) to have
`fetch-keyvault-secret.sh` retrieve the cipher passphrase via this
container's managed identity instead of mounting it directly. See
`docs/BACKUP_RESTORE_RUNBOOK.md` for what this does and does not change about
custody. Pinned, verified pgBackRest version:
`2.60.0-1.pgdg12+1` (`pgbackrest.Dockerfile`) — confirmed available via
`apt-cache madison` against the exact base image before pinning; re-check
before bumping.

## Recovery

Use `pgbackrest info`, `check`, and `restore --type=time` into a new disposable
PostgreSQL instance. Never restore over the live `pgdata` volume. Keep the
encryption key and its version/custody record outside the database; loss of the
key makes encrypted repository contents unrecoverable — see
`docs/BACKUP_ENCRYPTION_KEY_CUSTODY_RUNBOOK.md` for the custody procedure.

## Alerting

`alert-notify.sh` is a dependency-light (curl-only) webhook dispatcher that
`backup-runner.sh` and `backup-monitor.sh` call on these conditions: a failed
scheduled backup, a failed `pgbackrest check` (covers both WAL-archiving
failure and an unreachable repository), a stale backup (no success within
`BACKUP_STALE_MINUTES`), excessive repository disk growth since the previous
monitor tick, excessive PostgreSQL `pg_wal` accumulation (independent of the
backup repository — a stuck replication slot or failing `archive_command` can
fill `pg_wal` even while the repository itself looks healthy), and low free
space on the PostgreSQL data volume (independent of backup health entirely —
a full data volume halts PostgreSQL outright). The last two reuse the
`backup` container's existing `pgdata` mount; no new volume or service was
added for them.

- `PG_WAL_MAX_SIZE_MB` (default 4096) — `pg_wal` above this size raises
  `critical PG_WAL_EXCESSIVE_ACCUMULATION`. Tune based on your
  `max_wal_size`/checkpoint settings; the default is a conservative generic
  starting point, not a measured value for this deployment's actual workload.
- `DB_VOLUME_MIN_FREE_PERCENT` (default 15) — free space below this percent
  on the volume backing `/var/lib/postgresql/data` raises
  `critical DB_VOLUME_LOW_FREE_SPACE`.

It is opt-in and safe by default: with `BACKUP_ALERT_WEBHOOK_URL` unset, every
condition is still logged to `scheduler.log`/`monitor.log` and to
`/var/log/pgbackrest/alerts/`, but nothing is claimed as delivered externally.
Set these to activate real delivery:

- `BACKUP_ALERT_WEBHOOK_URL` — any HTTPS endpoint that accepts a JSON POST
  (`{"severity","event","stanza","host","timestamp","message"}`). Compatible
  with a Slack incoming webhook, PagerDuty Events API v2, or a generic
  receiver.
- `BACKUP_ALERT_WEBHOOK_AUTH_HEADER_FILE` — optional path, inside the
  container, to a file containing the full `Authorization` header value (e.g.
  `Bearer <token>`). Mount it from a secret store, never from `.env` or a
  committed file. The token is read at alert time only; it is never logged or
  included in the alert payload.
- `BACKUP_ALERT_DEDUP_SECONDS` (default 1800) — the same `event_code` is sent
  at most once per this window, so a flapping condition cannot storm the
  receiver.
- `BACKUP_REPO_MAX_GROWTH_MB` (default 2048) — repository growth above this
  per monitor tick raises a `warning`-severity `BACKUP_REPO_EXCESSIVE_GROWTH`
  alert.

**Secret injection.** `PGBACKREST_REPO1_CIPHER_PASS` as a plain env var is
visible in plaintext via `docker inspect`/`docker exec ... env` to anyone with
Docker daemon/host access — confirmed directly in this environment (see
`docs/BACKUP_ENCRYPTION_KEY_CUSTODY_RUNBOOK.md`). Prefer
`PGBACKREST_REPO1_CIPHER_PASS_FILE` (a path to a file containing the
passphrase, e.g. a Docker/Swarm secret mount): confirmed that with only the
`_FILE` variant set, neither `docker inspect` nor `docker exec ... env` show
the passphrase at all — only the file path. The plain env var remains
supported unchanged for existing deployments. This does not protect against
host-root/Docker-daemon access to the container's filesystem itself (nothing
can, short of restricting who has that access) — it only removes the
lower-effort `docker inspect`/`env` exposure.

Delivery uses bounded retries (`curl --retry 3`, 10s timeout per attempt) and
never raises its own failure above the condition it was reporting: if the
webhook itself is unreachable, the original backup/monitor failure is still
logged and the script's exit code still reflects it.

**Do not claim production alert delivery until a real receiver is configured
and a forced-failure test against it has actually been run** — see
`docs/MILESTONE_8_BACKUP_OPERATIONS_CLOSURE.md` for the disposable-receiver
test evidence from this environment, which is not a substitute for testing
against the real production receiver.
