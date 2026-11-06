#!/usr/bin/env bash
set -Eeuo pipefail
mode="${1:-diff}"
stanza="${PGBACKREST_STANZA:-hellodoctor}"
lock="${PGBACKREST_LOCK_PATH:-/tmp/hellodoctor-pgbackrest.lock}"
alert="${PGBACKREST_ALERT_SCRIPT:-/usr/local/bin/alert-notify.sh}"
mkdir -p "$(dirname "$lock")"
exec 9>"$lock"
flock -n 9 || { echo "backup already running; refusing overlap"; exit 75; }

backup_log="$(mktemp /tmp/pgbackrest-run.XXXXXX)"
trap 'rm -f "$backup_log"' EXIT

# `rc` must be captured from $? immediately after the command, with `set +e`
# in effect — `if ! cmd; then rc=$?; fi` is a trap: `!` negates the exit
# status for the `if` branch test itself, so `$?` inside that `then` block is
# the *negated* boolean (0/1), not the command's real exit code. That bug was
# caught live in this environment: a forced pgbackrest failure (exit 37) was
# being reported by this script as exit 0, which would have made a real
# backup failure invisible to cron/scheduler.log while still alerting —
# silently worse than having no alerting at all.
set +e
case "$mode" in
  full) pgbackrest --stanza="$stanza" --type=full backup ;;
  diff|incremental) pgbackrest --stanza="$stanza" --type=diff backup ;;
  *) echo "usage: backup-runner.sh {full|diff}" >&2; exit 64 ;;
esac >"$backup_log" 2>&1
rc=$?
set -e

if [ "$rc" -ne 0 ]; then
  cat "$backup_log" >&2
  "$alert" critical BACKUP_FAILED "pgbackrest --type=${mode} backup failed for stanza ${stanza} (exit ${rc}). See scheduler.log for full output." \
    || echo "backup-runner: alert-notify dispatch itself failed; see monitor.log/alerts state" >&2
  exit "$rc"
fi
cat "$backup_log"
pgbackrest --stanza="$stanza" info --output=json > /var/log/pgbackrest/last-info.json
