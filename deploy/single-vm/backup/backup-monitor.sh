#!/usr/bin/env bash
# Runs every BACKUP_MONITOR_SCHEDULE tick (see backup-entrypoint.sh's cron
# line). Checks, independently, the conditions G-05 (plus the 2026-11-01
# follow-up) requires beyond a backup run's own failure (which
# backup-runner.sh alerts on directly):
#   - failed WAL archiving / unavailable repository (via `pgbackrest check`)
#   - stale backup (no successful backup within BACKUP_STALE_MINUTES)
#   - excessive repository disk growth since the previous tick
#   - excessive PostgreSQL pg_wal accumulation (independent of the backup
#     repository — a stuck replication slot or failing archive_command can
#     fill pg_wal even while the repository itself looks healthy)
#   - low free space on the PostgreSQL data volume (independent of backup
#     health entirely — a full data volume halts PostgreSQL outright)
# The last two require the data volume to be mounted into this container
# (`pgdata:/var/lib/postgresql/data` in compose.backup.yml's `backup`
# service, already present for other reasons) — no new volume or service.
# Each condition alerts independently so one failure does not mask another;
# the script's own exit code is non-zero if any check failed, for scheduler.log.
set -Eeuo pipefail
stanza="${PGBACKREST_STANZA:-hellodoctor}"
# Kept in sync with backup-entrypoint.sh's default — see that script's comment
# for why 30 (not the original 120) is the floor consistent with the
# documented ~15-minute RPO.
max_age="${BACKUP_STALE_MINUTES:-30}"
alert="${PGBACKREST_ALERT_SCRIPT:-/usr/local/bin/alert-notify.sh}"
repo_type="${PGBACKREST_REPO1_TYPE:-posix}"
repo_path="${PGBACKREST_REPO1_PATH:-/var/lib/pgbackrest/repo1}"
log_dir="${PGBACKREST_LOG_DIR:-/var/log/pgbackrest}"
growth_state="$log_dir/repo-size.last"
max_growth_mb="${BACKUP_REPO_MAX_GROWTH_MB:-2048}"
pgdata_path="${PGDATA_PATH:-/var/lib/postgresql/data}"
max_wal_mb="${PG_WAL_MAX_SIZE_MB:-4096}"
min_free_percent="${DB_VOLUME_MIN_FREE_PERCENT:-15}"

overall_rc=0

# --- 1. Repository reachability + WAL archiving round-trip -----------------
check_log="$(mktemp /tmp/pgbackrest-check.XXXXXX)"
if ! pgbackrest --stanza="$stanza" check >"$check_log" 2>&1; then
  detail="pgbackrest check failed for stanza ${stanza}: repository unreachable or WAL archiving broken. $(tail -c 400 "$check_log" | tr '\n' ' ')"
  "$alert" critical BACKUP_CHECK_FAILED "$detail" \
    || echo "backup-monitor: alert-notify dispatch failed for BACKUP_CHECK_FAILED" >&2
  cat "$check_log" >&2
  overall_rc=1
fi
rm -f "$check_log"

# --- 2. Staleness: no successful backup within the configured window -------
info=$(pgbackrest --stanza="$stanza" info --output=json 2>/dev/null) || info='[]'
printf '%s\n' "$info" > "$log_dir/last-info.json"

last=$(printf '%s' "$info" | python3 -c '
import json, sys
try:
    d = json.load(sys.stdin)
    b = d[0].get("backup", []) if d else []
    print(b[-1].get("timestamp", {}).get("stop", 0) if b else 0)
except Exception:
    print(0)
' 2>/dev/null || echo 0)

if ! python3 - "$last" "$max_age" <<'PY'
import sys, time
last = float(sys.argv[1] or 0)
max_age = float(sys.argv[2]) * 60
if not last or time.time() - last > max_age:
    raise SystemExit(1)
PY
then
  detail="No successful backup recorded within ${max_age} minutes for stanza ${stanza} (last successful-stop epoch: ${last:-0})."
  "$alert" critical BACKUP_STALE "$detail" \
    || echo "backup-monitor: alert-notify dispatch failed for BACKUP_STALE" >&2
  overall_rc=1
fi

# --- 3. Excessive repository disk/WAL growth since the previous tick -------
# `du` on $repo_path only measures a local POSIX filesystem; for repo_type=
# azure that path isn't where the data lives at all, so `du` would silently
# report ~0 forever and this check would never fire — the audit's "monitor
# does not measure a remote Azure repository" finding. For azure, use the
# cumulative repository size pgBackRest itself reports for the latest backup
# (info[].backup[-1].info.repository.size) instead of a local disk reading.
if [ "$repo_type" = azure ]; then
  current_size_kb=$(printf '%s' "$info" | python3 -c '
import json, sys
try:
    d = json.load(sys.stdin)
    b = d[0].get("backup", []) if d else []
    size = b[-1].get("info", {}).get("repository", {}).get("size", 0) if b else 0
    print(int(size) // 1024)
except Exception:
    print(0)
' 2>/dev/null || echo 0)
else
  current_size_kb=$(du -sk "$repo_path" 2>/dev/null | cut -f1)
fi
current_size_kb=${current_size_kb:-0}
if [ -f "$growth_state" ]; then
  previous_size_kb=$(cat "$growth_state" 2>/dev/null || echo "$current_size_kb")
  case "$previous_size_kb" in ''|*[!0-9]*) previous_size_kb=$current_size_kb ;; esac
  growth_mb=$(( (current_size_kb - previous_size_kb) / 1024 ))
  if [ "$growth_mb" -gt "$max_growth_mb" ]; then
    detail="Backup repository (${repo_type}) grew by ${growth_mb}MB since the previous monitor tick (threshold ${max_growth_mb}MB) — possible WAL/backup accumulation fault or retention misconfiguration."
    "$alert" warning BACKUP_REPO_EXCESSIVE_GROWTH "$detail" \
      || echo "backup-monitor: alert-notify dispatch failed for BACKUP_REPO_EXCESSIVE_GROWTH" >&2
  fi
fi
echo "$current_size_kb" > "$growth_state"

# --- 4. PostgreSQL pg_wal accumulation (independent of the repository) -----
pg_wal_path="${pgdata_path}/pg_wal"
if [ -d "$pg_wal_path" ]; then
  wal_size_kb=$(du -sk "$pg_wal_path" 2>/dev/null | cut -f1)
  wal_size_kb=${wal_size_kb:-0}
  wal_size_mb=$(( wal_size_kb / 1024 ))
  if [ "$wal_size_mb" -gt "$max_wal_mb" ]; then
    detail="PostgreSQL pg_wal at ${pg_wal_path} has accumulated ${wal_size_mb}MB (threshold ${max_wal_mb}MB) — check for a stuck replication slot, a failing archive_command, or a checkpoint/WAL-retention misconfiguration. This is independent of backup-repository size (check 3 above)."
    "$alert" critical PG_WAL_EXCESSIVE_ACCUMULATION "$detail" \
      || echo "backup-monitor: alert-notify dispatch failed for PG_WAL_EXCESSIVE_ACCUMULATION" >&2
    overall_rc=1
  fi
else
  echo "backup-monitor: pg_wal path not found at ${pg_wal_path} (PGDATA not mounted into this container, or not yet initialized) — skipping WAL-accumulation check this tick" >&2
fi

# --- 5. PostgreSQL data-volume free space (independent of backup health) ---
if [ -d "$pgdata_path" ]; then
  # POSIX df: field 4 is available 1K-blocks, field 5 is Use% (with '%').
  df_line=$(df -Pk "$pgdata_path" 2>/dev/null | tail -n1)
  avail_kb=$(printf '%s' "$df_line" | awk '{print $4}')
  used_percent=$(printf '%s' "$df_line" | awk '{print $5}' | tr -d '%')
  case "$avail_kb" in ''|*[!0-9]*) avail_kb='' ;; esac
  case "$used_percent" in ''|*[!0-9]*) used_percent='' ;; esac
  if [ -n "$avail_kb" ] && [ -n "$used_percent" ]; then
    free_percent=$(( 100 - used_percent ))
    if [ "$free_percent" -lt "$min_free_percent" ]; then
      avail_mb=$(( avail_kb / 1024 ))
      detail="Database volume at ${pgdata_path} has only ${free_percent}% free (${avail_mb}MB) — below the ${min_free_percent}% threshold. A full data volume halts PostgreSQL outright; this is independent of backup-repository or pg_wal health above."
      "$alert" critical DB_VOLUME_LOW_FREE_SPACE "$detail" \
        || echo "backup-monitor: alert-notify dispatch failed for DB_VOLUME_LOW_FREE_SPACE" >&2
      overall_rc=1
    fi
  else
    echo "backup-monitor: could not parse 'df' output for ${pgdata_path} — skipping free-space check this tick" >&2
  fi
else
  echo "backup-monitor: PGDATA path not found at ${pgdata_path} — skipping free-space check this tick" >&2
fi

exit "$overall_rc"
