#!/usr/bin/env bash
# Generic, dependency-light alert dispatcher for the pgBackRest backup layer.
# Delivers a JSON event to one configured HTTPS webhook (Slack-compatible,
# PagerDuty-Events-v2-compatible receivers, or any generic JSON endpoint all
# accept this shape). Introduces no new service: curl is the only dependency.
#
# Usage: alert-notify.sh <severity> <event_code> <message>
#   severity    critical | warning | info
#   event_code  short machine key, e.g. BACKUP_FAILED, BACKUP_STALE
#   message     human-readable detail — callers must never pass secrets here
#
# Behavior:
#   - No BACKUP_ALERT_WEBHOOK_URL configured -> logs locally only, exits 0.
#     Alerting is opt-in; nothing claims delivery until a real receiver is set.
#   - Configured -> POSTs JSON with bounded retries (curl --retry), exits
#     non-zero only if delivery ultimately fails after retries.
#   - Deduplicates identical event_codes within BACKUP_ALERT_DEDUP_SECONDS
#     (default 1800s) so a flapping condition cannot storm the receiver.
#   - Never includes the webhook auth token, cipher passphrase, or any other
#     secret in the delivered payload or in stdout/stderr.
set -Eeuo pipefail

severity="${1:?usage: alert-notify.sh <severity> <event_code> <message>}"
event="${2:?usage: alert-notify.sh <severity> <event_code> <message>}"
message="${3:-}"

case "$severity" in
  critical|warning|info) ;;
  *) echo "alert-notify: invalid severity '$severity' (want critical|warning|info)" >&2; exit 64 ;;
esac

stanza="${PGBACKREST_STANZA:-hellodoctor}"
host="$(hostname 2>/dev/null || echo unknown-host)"
ts="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

state_dir="${BACKUP_ALERT_STATE_DIR:-/var/log/pgbackrest/alerts}"
mkdir -p "$state_dir"
dedup_seconds="${BACKUP_ALERT_DEDUP_SECONDS:-1800}"
state_file="$state_dir/${event}.last"

now_epoch=$(date -u +%s)
if [ -f "$state_file" ]; then
  last_epoch=$(cat "$state_file" 2>/dev/null || echo 0)
  case "$last_epoch" in ''|*[!0-9]*) last_epoch=0 ;; esac
  elapsed=$(( now_epoch - last_epoch ))
  if [ "$elapsed" -lt "$dedup_seconds" ]; then
    echo "alert-notify: suppressing duplicate ${event} (last sent ${elapsed}s ago, dedup window ${dedup_seconds}s)" >&2
    exit 0
  fi
fi

# Minimal JSON string escaping: backslash, double-quote, strip newlines.
escaped_message=$(printf '%s' "$message" | sed 's/\\/\\\\/g; s/"/\\"/g' | tr '\n' ' ')

payload=$(printf '{"severity":"%s","event":"%s","stanza":"%s","host":"%s","timestamp":"%s","message":"%s"}' \
  "$severity" "$event" "$stanza" "$host" "$ts" "$escaped_message")

if [ -z "${BACKUP_ALERT_WEBHOOK_URL:-}" ]; then
  echo "alert-notify: BACKUP_ALERT_WEBHOOK_URL not configured; alert recorded locally only." >&2
  echo "alert-notify: event=${event} severity=${severity} message=${message}" >&2
  echo "$now_epoch" > "$state_file"
  exit 0
fi

auth_header_args=()
if [ -n "${BACKUP_ALERT_WEBHOOK_AUTH_HEADER_FILE:-}" ] && [ -r "${BACKUP_ALERT_WEBHOOK_AUTH_HEADER_FILE}" ]; then
  auth_value="$(tr -d '\n' < "${BACKUP_ALERT_WEBHOOK_AUTH_HEADER_FILE}")"
  if [ -n "$auth_value" ]; then
    auth_header_args=(-H "Authorization: ${auth_value}")
  fi
fi

delivery_log="$state_dir/last-delivery.log"
if curl --fail --silent --show-error \
    --max-time 10 \
    --retry 3 --retry-delay 2 --retry-max-time 30 --retry-connrefused \
    -H 'Content-Type: application/json' \
    "${auth_header_args[@]}" \
    -d "$payload" \
    "$BACKUP_ALERT_WEBHOOK_URL" >"$delivery_log" 2>&1; then
  echo "alert-notify: delivered ${event} (${severity})" >&2
  echo "$now_epoch" > "$state_file"
  exit 0
else
  rc=$?
  echo "alert-notify: FAILED to deliver ${event} (${severity}) after retries (curl exit ${rc}); see ${delivery_log}" >&2
  cat "$delivery_log" >&2 2>/dev/null || true
  # Record the attempt anyway: a down receiver must not be hammered every
  # monitor/runner tick for the same still-unresolved condition.
  echo "$now_epoch" > "$state_file"
  exit 1
fi
