#!/usr/bin/env bash
# Nightly PostgreSQL dump for the single-VM deployment.
#
# Run by hellodoctor-db-dump.timer every night at 03:00 Africa/Nairobi.
# Writes one compressed, timestamped dump per night, verifies it can be read
# back, keeps the newest BACKUP_KEEP nights and deletes older ones, so the
# VM's disk use stays bounded.
#
# The dump is pg_dump's custom format at maximum compression: smaller than a
# plain .sql.gz, and pg_restore can restore the whole database or a single
# table from it. Files are readable by root only.
#
# This replaces nothing in the database: pg_dump takes a consistent snapshot
# and does not block normal reads and writes.
set -euo pipefail

CONTAINER="${PG_CONTAINER:-hellodoctor-vm-postgres-1}"
BACKUP_DIR="${BACKUP_DIR:-/var/backups/hellodoctor}"
BACKUP_KEEP="${BACKUP_KEEP:-14}"
BACKUP_MIN_FREE_MB="${BACKUP_MIN_FREE_MB:-2048}"
BACKUP_WAIT_SECONDS="${BACKUP_WAIT_SECONDS:-300}"
# Day of the week for the full test restore (Mon..Sun), "always" or "never".
BACKUP_RESTORE_TEST_DAY="${BACKUP_RESTORE_TEST_DAY:-Sun}"
# Optional: a Slack/Teams/Discord-style webhook that receives failures.
BACKUP_ALERT_WEBHOOK_URL="${BACKUP_ALERT_WEBHOOK_URL:-}"
# Small status files (no data, no secrets) that Admin Web shows on the
# App Errors & Incidents page. Mounted read-only into the backend.
BACKUP_STATUS_DIR="${BACKUP_STATUS_DIR:-/var/lib/hellodoctor-backup-status}"
# Optional Telegram alerts: failures and overdue backups are sent with
# sound; the nightly "ok" is sent silently (BACKUP_TELEGRAM_ON_SUCCESS=off
# to skip it). A dead VM is reported by .github/workflows/uptime-watch.yml.
TELEGRAM_BOT_TOKEN="${TELEGRAM_BOT_TOKEN:-}"
TELEGRAM_CHAT_ID="${TELEGRAM_CHAT_ID:-}"
BACKUP_TELEGRAM_ON_SUCCESS="${BACKUP_TELEGRAM_ON_SUCCESS:-silent}"
# The daily --check run alerts when the newest success is older than this.
BACKUP_OVERDUE_HOURS="${BACKUP_OVERDUE_HOURS:-26}"
PREFIX="hellodoctor"

log() { printf '%s %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$*"; }
alert() {
  [ -n "$BACKUP_ALERT_WEBHOOK_URL" ] || return 0
  local text="HelloDoctor nightly DB dump on $(hostname): $1"
  text="${text//\\/\\\\}"; text="${text//\"/\\\"}"
  curl -fsS -m 10 -H 'Content-Type: application/json' \
    -d "{\"text\":\"$text\",\"content\":\"$text\"}" "$BACKUP_ALERT_WEBHOOK_URL" >/dev/null 2>&1 \
    || log "could not reach the alert webhook"
}
# JSON string escaping for the few values the status files carry.
json_str() { local v="$1"; v="${v//\\/\\\\}"; v="${v//\"/\\\"}"; v="${v//$'\n'/ }"; printf '"%s"' "$v"; }
now_utc() { date -u +%Y-%m-%dT%H:%M:%SZ; }
# Writes one status file atomically: readers never see a half-written file.
write_status() {
  local name="$1" body="$2"
  mkdir -p "$BACKUP_STATUS_DIR" 2>/dev/null && chmod 755 "$BACKUP_STATUS_DIR" 2>/dev/null || return 0
  printf '%s\n' "$body" > "$BACKUP_STATUS_DIR/.$name.tmp" && chmod 644 "$BACKUP_STATUS_DIR/.$name.tmp" \
    && mv -f "$BACKUP_STATUS_DIR/.$name.tmp" "$BACKUP_STATUS_DIR/$name.json" || true
}
# Sends a Telegram message; "loud" rings the phone, "silent" does not.
# Never lets a network problem fail the backup itself.
telegram() {
  [ -n "$TELEGRAM_BOT_TOKEN" ] && [ -n "$TELEGRAM_CHAT_ID" ] || return 0
  local quiet=false; [ "$1" = "silent" ] && quiet=true
  curl -fsS -m 10 --retry 3 -o /dev/null \
    --data-urlencode "chat_id=$TELEGRAM_CHAT_ID" \
    --data-urlencode "text=$2" \
    --data-urlencode "disable_notification=$quiet" \
    "https://api.telegram.org/bot${TELEGRAM_BOT_TOKEN}/sendMessage" 2>/dev/null \
    || log "could not reach Telegram"
}
fail() {
  log "FAILED: $*"
  telegram loud "🚨 HelloDoctor backup FAILED on $(hostname): $*"
  write_status last_run "{\"at\":$(json_str "$(now_utc)"),\"result\":\"failed\",\"message\":$(json_str "$*")}"
  alert "FAILED: $*"
  exit 1
}

case "$BACKUP_KEEP" in ''|*[!0-9]*) fail "BACKUP_KEEP must be a whole number, got '$BACKUP_KEEP'";; esac
[ "$BACKUP_KEEP" -ge 1 ] || fail "BACKUP_KEEP must be at least 1"

if [ "${1:-}" = "--check" ]; then
  last="$(sed -n 's/.*"at":"\([^"]*\)".*/\1/p' "$BACKUP_STATUS_DIR/last_success.json" 2>/dev/null | head -n1 || true)"
  if [ -z "$last" ]; then
    telegram loud "⚠️ HelloDoctor: no successful backup recorded on $(hostname). Is the nightly dump installed?"
    log "check: no successful backup recorded"; exit 1
  fi
  age_h=$(( ( $(date -u +%s) - $(date -u -d "$last" +%s) ) / 3600 ))
  if [ "$age_h" -gt "$BACKUP_OVERDUE_HOURS" ]; then
    telegram loud "⚠️ HelloDoctor backup OVERDUE on $(hostname): last success $age_h hours ago ($last)."
    log "check: overdue, last success $age_h h ago"; exit 1
  fi
  log "check: ok, last success $age_h h ago"; exit 0
fi

umask 077
mkdir -p "$BACKUP_DIR"
chmod 700 "$BACKUP_DIR"

# One run at a time.
exec 9>"$BACKUP_DIR/.lock"
flock -n 9 || fail "another dump is already running"

# Refuse to start when the disk is nearly full: a half-written dump that fills
# the disk would also stop PostgreSQL from writing.
free_mb="$(df -Pm "$BACKUP_DIR" | awk 'NR==2 {print $4}')"
[ "${free_mb:-0}" -ge "$BACKUP_MIN_FREE_MB" ] \
  || fail "only ${free_mb} MB free in $BACKUP_DIR, need at least ${BACKUP_MIN_FREE_MB} MB"

# After a reboot the timer's catch-up run can fire before Docker has started
# the database, so wait (up to BACKUP_WAIT_SECONDS) for it to accept
# connections instead of skipping the night.
waited=0
until docker exec "$CONTAINER" sh -c 'pg_isready -q -U "$POSTGRES_USER" -d "$POSTGRES_DB"' >/dev/null 2>&1; do
  [ "$waited" -lt "$BACKUP_WAIT_SECONDS" ] || fail "database in $CONTAINER not ready after ${BACKUP_WAIT_SECONDS}s"
  sleep 10
  waited=$((waited + 10))
done
[ "$waited" -eq 0 ] || log "database ready after ${waited}s"

stamp="$(TZ=Africa/Nairobi date +%Y-%m-%d_%H%M)"
final="$BACKUP_DIR/${PREFIX}-${stamp}.dump"
globals="$BACKUP_DIR/${PREFIX}-${stamp}.globals.sql"
partial="$BACKUP_DIR/.${PREFIX}-${stamp}.dump.partial"
trap 'rm -f "$partial"' EXIT

log "dumping $CONTAINER to $final"
# pg_dump runs inside the database container, outside this service's own
# limits, so lower its CPU and disk priority there: live traffic always wins.
docker exec "$CONTAINER" sh -c 'exec nice -n 10 ionice -c 3 pg_dump -U "$POSTGRES_USER" -d "$POSTGRES_DB" --format=custom --compress=9' > "$partial" \
  || fail "pg_dump exited with an error"

# A dump only counts once pg_restore can read its table of contents.
entries="$(docker exec -i "$CONTAINER" pg_restore --list < "$partial" | grep -c ';' || true)"
[ "${entries:-0}" -gt 0 ] || fail "the new dump could not be read back by pg_restore"

mv "$partial" "$final"
trap - EXIT

# Roles live outside the database, so pg_dump never includes them. Without
# them, a restore onto a NEW server fails on every GRANT and RLS policy that
# names hellodoctor_api and the other capability roles. Passwords are left
# out: the backend sets them from its environment at start-up.
docker exec "$CONTAINER" sh -c 'pg_dumpall -U "$POSTGRES_USER" --globals-only --no-role-passwords' > "$globals" \
  || fail "could not dump database roles"
( cd "$BACKUP_DIR" && sha256sum "$(basename "$final")" "$(basename "$globals")" > "$(basename "$final").sha256" )
log "ok: $(du -h "$final" | cut -f1), $entries catalogue entries, roles saved"

# Weekly proof that the dump really restores: load it into a throwaway
# database, count real rows, drop it. Reading the table of contents alone
# only proves the file opens.
today="$(TZ=Africa/Nairobi date +%a)"
if [ "$BACKUP_RESTORE_TEST_DAY" = "always" ] || [ "$BACKUP_RESTORE_TEST_DAY" = "$today" ]; then
  scratch="restore_check_$(date +%s)"
  log "test restore into scratch database $scratch"
  docker exec "$CONTAINER" sh -c "createdb -U \"\$POSTGRES_USER\" $scratch" \
    || fail "could not create the scratch database for the test restore"
  if docker exec -i "$CONTAINER" sh -c "exec nice -n 10 ionice -c 3 pg_restore -U \"\$POSTGRES_USER\" -d $scratch --no-owner --exit-on-error" < "$final" >/dev/null 2>&1; then
    rows="$(docker exec "$CONTAINER" sh -c "psql -U \"\$POSTGRES_USER\" -d $scratch -Atc 'select count(*) from _sqlx_migrations'" 2>/dev/null || echo 0)"
    docker exec "$CONTAINER" sh -c "dropdb -U \"\$POSTGRES_USER\" $scratch" || log "could not drop $scratch; drop it by hand"
    [ "${rows:-0}" -gt 0 ] || fail "test restore produced an empty database"
    log "test restore ok: $rows migrations present"
    write_status last_restore_test "{\"at\":$(json_str "$(now_utc)"),\"result\":\"ok\",\"file\":$(json_str "$(basename "$final")"),\"migrations\":$rows}"
  else
    docker exec "$CONTAINER" sh -c "dropdb -U \"\$POSTGRES_USER\" --if-exists $scratch" >/dev/null 2>&1 || true
    write_status last_restore_test "{\"at\":$(json_str "$(now_utc)"),\"result\":\"failed\",\"file\":$(json_str "$(basename "$final")")}"
    fail "test restore of $(basename "$final") failed"
  fi
fi

# Keep the newest BACKUP_KEEP dumps; delete older ones and their checksums.
removed=0
while IFS= read -r old; do
  [ -n "$old" ] || continue
  rm -f -- "$old" "$old.sha256" "${old%.dump}.globals.sql"
  removed=$((removed + 1))
done < <(ls -1t "$BACKUP_DIR/${PREFIX}"-*.dump 2>/dev/null | tail -n +"$((BACKUP_KEEP + 1))")
[ "$removed" -eq 0 ] || log "removed $removed dump(s) older than the newest $BACKUP_KEEP"

kept="$(ls -1 "$BACKUP_DIR/${PREFIX}"-*.dump 2>/dev/null | wc -l | tr -d ' ')"
total_bytes="$(du -sb "$BACKUP_DIR" 2>/dev/null | cut -f1 || echo 0)"
size_bytes="$(wc -c < "$final" | tr -d ' ')"
log "kept $kept dump(s), $(du -sh "$BACKUP_DIR" | cut -f1) in total"
done_at="$(now_utc)"
write_status last_success "{\"at\":$(json_str "$done_at"),\"file\":$(json_str "$(basename "$final")"),\"size_bytes\":${size_bytes:-0},\"dumps_kept\":${kept:-0},\"total_bytes\":${total_bytes:-0},\"keep\":$BACKUP_KEEP}"
write_status last_run "{\"at\":$(json_str "$done_at"),\"result\":\"ok\",\"message\":\"\"}"
[ "$BACKUP_TELEGRAM_ON_SUCCESS" = "off" ] || telegram silent "✅ HelloDoctor backup ok: $(basename "$final"), $(du -h "$final" | cut -f1), $kept kept"
