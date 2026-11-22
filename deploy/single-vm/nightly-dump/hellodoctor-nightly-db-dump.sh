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
PREFIX="hellodoctor"

log() { printf '%s %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$*"; }
fail() { log "FAILED: $*"; exit 1; }

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

docker inspect -f '{{.State.Running}}' "$CONTAINER" 2>/dev/null | grep -qx true \
  || fail "container $CONTAINER is not running"

stamp="$(TZ=Africa/Nairobi date +%Y-%m-%d_%H%M)"
final="$BACKUP_DIR/${PREFIX}-${stamp}.dump"
partial="$BACKUP_DIR/.${PREFIX}-${stamp}.dump.partial"
trap 'rm -f "$partial"' EXIT

log "dumping $CONTAINER to $final"
docker exec "$CONTAINER" sh -c 'pg_dump -U "$POSTGRES_USER" -d "$POSTGRES_DB" --format=custom --compress=9' > "$partial" \
  || fail "pg_dump exited with an error"

# A dump only counts once pg_restore can read its table of contents.
entries="$(docker exec -i "$CONTAINER" pg_restore --list < "$partial" | grep -c ';' || true)"
[ "${entries:-0}" -gt 0 ] || fail "the new dump could not be read back by pg_restore"

mv "$partial" "$final"
trap - EXIT
( cd "$BACKUP_DIR" && sha256sum "$(basename "$final")" > "$(basename "$final").sha256" )
log "ok: $(du -h "$final" | cut -f1), $entries catalogue entries"

# Keep the newest BACKUP_KEEP dumps; delete older ones and their checksums.
removed=0
while IFS= read -r old; do
  [ -n "$old" ] || continue
  rm -f -- "$old" "$old.sha256"
  removed=$((removed + 1))
done < <(ls -1t "$BACKUP_DIR/${PREFIX}"-*.dump 2>/dev/null | tail -n +"$((BACKUP_KEEP + 1))")
[ "$removed" -eq 0 ] || log "removed $removed dump(s) older than the newest $BACKUP_KEEP"

log "kept $(ls -1 "$BACKUP_DIR/${PREFIX}"-*.dump 2>/dev/null | wc -l) dump(s), $(du -sh "$BACKUP_DIR" | cut -f1) in total"
