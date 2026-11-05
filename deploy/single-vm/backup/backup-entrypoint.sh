#!/usr/bin/env bash
set -Eeuo pipefail

: "${PGHOST:=postgres}"
: "${PGPORT:=5432}"
: "${PGUSER:=postgres}"
: "${PGDATABASE:=postgres}"
: "${PGBACKREST_STANZA:=hellodoctor}"
: "${PGBACKREST_REPO1_PATH:=/var/lib/pgbackrest/repo1}"
: "${PGBACKREST_REPO1_RETENTION_FULL:=7}"
: "${PGBACKREST_REPO1_RETENTION_DIFF:=14}"
: "${BACKUP_FULL_SCHEDULE:=03:15}"
: "${BACKUP_DIFF_SCHEDULE:=*/15}"
: "${BACKUP_STALE_MINUTES:=120}"
: "${BACKUP_MONITOR_SCHEDULE:=*/15}"
: "${BACKUP_RUN_ON_START:=false}"

install -d -m 0750 -o postgres -g postgres /etc/pgbackrest /var/lib/pgbackrest/repo1 /var/log/pgbackrest
chown -R postgres:postgres /var/lib/pgbackrest/repo1
repo_type="${PGBACKREST_REPO1_TYPE:-posix}"
repo_options=""
if [ "$repo_type" = azure ]; then
  azure_key_type="${PGBACKREST_REPO1_AZURE_KEY_TYPE:-auto}"
  azure_key_line=""
  if [ "$azure_key_type" != auto ]; then
    azure_key_line="repo1-azure-key=${PGBACKREST_REPO1_AZURE_KEY:?required for non-auto Azure authentication}"
  fi
  repo_options="repo1-type=azure
repo1-azure-account=${PGBACKREST_REPO1_AZURE_ACCOUNT:?required for Azure repository}
repo1-azure-container=${PGBACKREST_REPO1_AZURE_CONTAINER:?required for Azure repository}
repo1-azure-endpoint=${PGBACKREST_REPO1_AZURE_ENDPOINT:-core.windows.net}
${azure_key_line}
repo1-azure-key-type=${azure_key_type}"
fi
# Prefer a file-based secret (never appears in `docker inspect`/`docker exec
# ... env`) over the plain env var. Reading a path is not itself sensitive, so
# this adds no new exposure; it only removes the env-var exposure vector when
# an operator switches to it. The plain env var remains supported unchanged
# for backward compatibility — this is additive, not a behavior change to the
# existing working path.
cipher_pass="${PGBACKREST_REPO1_CIPHER_PASS:-}"
if [ -n "${PGBACKREST_REPO1_CIPHER_PASS_FILE:-}" ]; then
  if [ ! -r "${PGBACKREST_REPO1_CIPHER_PASS_FILE}" ]; then
    echo "PGBACKREST_REPO1_CIPHER_PASS_FILE is set but not readable: ${PGBACKREST_REPO1_CIPHER_PASS_FILE}" >&2
    exit 1
  fi
  cipher_pass="$(tr -d '\n' < "${PGBACKREST_REPO1_CIPHER_PASS_FILE}")"
fi
: "${cipher_pass:?PGBACKREST_REPO1_CIPHER_PASS or PGBACKREST_REPO1_CIPHER_PASS_FILE must be supplied via a secret store}"

cat > /etc/pgbackrest/pgbackrest.conf <<EOF
[global]
${repo_options}
repo1-path=${PGBACKREST_REPO1_PATH}
repo1-retention-full=${PGBACKREST_REPO1_RETENTION_FULL}
repo1-retention-diff=${PGBACKREST_REPO1_RETENTION_DIFF}
repo1-retention-full-type=time
start-fast=y
process-max=${PGBACKREST_PROCESS_MAX:-1}
compress-type=zst
compress-level=${PGBACKREST_COMPRESS_LEVEL:-3}
log-level-console=info
log-level-file=detail
log-path=/var/log/pgbackrest
repo1-cipher-type=${PGBACKREST_REPO1_CIPHER_TYPE:-aes-256-cbc}
repo1-cipher-pass=${cipher_pass}

[${PGBACKREST_STANZA}]
pg1-path=/var/lib/postgresql/data
pg1-socket-path=/var/run/postgresql
EOF
unset cipher_pass
chown postgres:postgres /etc/pgbackrest/pgbackrest.conf
chmod 0640 /etc/pgbackrest/pgbackrest.conf

cat > /etc/cron.d/hellodoctor-pgbackrest <<EOF
SHELL=/bin/sh
PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
${BACKUP_DIFF_SCHEDULE} * postgres /usr/local/bin/backup-runner.sh diff >>/var/log/pgbackrest/scheduler.log 2>&1
${BACKUP_FULL_SCHEDULE%:*} ${BACKUP_FULL_SCHEDULE#*:} * * * postgres /usr/local/bin/backup-runner.sh full >>/var/log/pgbackrest/scheduler.log 2>&1
${BACKUP_MONITOR_SCHEDULE} * * * * postgres /usr/local/bin/backup-monitor.sh >>/var/log/pgbackrest/monitor.log 2>&1
EOF
chmod 0644 /etc/cron.d/hellodoctor-pgbackrest

until pg_isready -h "$PGHOST" -p "$PGPORT" -U "$PGUSER" -d "$PGDATABASE"; do sleep 2; done
runuser -u postgres -- pgbackrest --stanza="$PGBACKREST_STANZA" stanza-create || true
runuser -u postgres -- pgbackrest --stanza="$PGBACKREST_STANZA" check
if [ "$BACKUP_RUN_ON_START" = true ]; then
  runuser -u postgres -- /usr/local/bin/backup-runner.sh diff
fi
exec cron -f
