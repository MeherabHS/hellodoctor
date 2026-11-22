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
fail() { log "FAILED: $*"; alert "FAILED: $*"; exit 1; }

case "$BACKUP_KEEP" in ''|*[!0-9]*) fail "BACKUP_KEEP must be a whole number, got '$BACKUP_KEEP'";; esac
[ "$BACKUP_KEEP" -ge 1 ] || fail "BACKUP_KEEP must be at least 1"

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
  else
    docker exec "$CONTAINER" sh -c "dropdb -U \"\$POSTGRES_USER\" --if-exists $scratch" >/dev/null 2>&1 || true
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

log "kept $(ls -1 "$BACKUP_DIR/${PREFIX}"-*.dump 2>/dev/null | wc -l) dump(s), $(du -sh "$BACKUP_DIR" | cut -f1) in total"
