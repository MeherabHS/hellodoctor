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
# Lowered from the original 120-minute default: the configured differential
# cadence is 15 minutes, so a 120-minute staleness window tolerated eight
# missed runs before alerting. 30 minutes (two missed differentials) is
# consistent with the documented ~15-minute RPO target while still tolerating
# one transient failure without paging. Override per-deployment as needed.
: "${BACKUP_STALE_MINUTES:=30}"
: "${BACKUP_MONITOR_SCHEDULE:=*/15}"
: "${BACKUP_RUN_ON_START:=false}"

install -d -m 0750 -o postgres -g postgres /etc/pgbackrest /var/lib/pgbackrest/repo1 /var/log/pgbackrest
chown -R postgres:postgres /var/lib/pgbackrest/repo1
source /usr/local/bin/pgbackrest-conf-common.sh
generate_pgbackrest_conf "${PGBACKREST_STANZA}" /etc/pgbackrest/pgbackrest.conf

cat > /etc/cron.d/hellodoctor-pgbackrest <<EOF
SHELL=/bin/sh
PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
${BACKUP_DIFF_SCHEDULE} * postgres /usr/local/bin/backup-runner.sh diff >>/var/log/pgbackrest/scheduler.log 2>&1
${BACKUP_FULL_SCHEDULE%:*} ${BACKUP_FULL_SCHEDULE#*:} * * * postgres /usr/local/bin/backup-runner.sh full >>/var/log/pgbackrest/scheduler.log 2>&1
${BACKUP_MONITOR_SCHEDULE} * * * * postgres /usr/local/bin/backup-monitor.sh >>/var/log/pgbackrest/monitor.log 2>&1
EOF
chmod 0644 /etc/cron.d/hellodoctor-pgbackrest

# Alerting is opt-in by design (alert-notify.sh logs locally and exits 0 with
# no BACKUP_ALERT_WEBHOOK_URL set) so local/dev/CI verification never needs a
# receiver. For a real production deployment, set REQUIRE_BACKUP_ALERT_WEBHOOK
# =true to turn the "no receiver configured" state into a hard startup
# failure instead of a silent no-op — addressing the audit finding that alert
# delivery was opt-in and unverified. This is a deliberate operator opt-in,
# not a default, because it would otherwise break every existing local
# verification run that has never configured a webhook.
if [ "${REQUIRE_BACKUP_ALERT_WEBHOOK:-false}" = true ] && [ -z "${BACKUP_ALERT_WEBHOOK_URL:-}" ]; then
  echo "FATAL: REQUIRE_BACKUP_ALERT_WEBHOOK=true but BACKUP_ALERT_WEBHOOK_URL is not set. A production backup scheduler must not run with alerting silently disabled." >&2
  exit 1
fi

until pg_isready -h "$PGHOST" -p "$PGPORT" -U "$PGUSER" -d "$PGDATABASE"; do sleep 2; done
runuser -u postgres -- pgbackrest --stanza="$PGBACKREST_STANZA" stanza-create || true
runuser -u postgres -- pgbackrest --stanza="$PGBACKREST_STANZA" check
if [ "$BACKUP_RUN_ON_START" = true ]; then
  runuser -u postgres -- /usr/local/bin/backup-runner.sh diff
fi
exec cron -f
