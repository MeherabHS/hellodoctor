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

# Recovery manifest: what a restore needs to know about *this* backup set,
# captured at the moment pgBackRest itself confirms success — not invented
# after the fact during an actual recovery, when the person doing it is under
# pressure. Deliberately scoped to what pgBackRest and this container can
# state with certainty; it does NOT attempt a new database connection from
# this cron context to pull an application migration version (no DB
# credential/auth path is wired into the backup container today, and adding
# one just for this would be a new, untested failure mode) — that
# correlation is the job of the DR tooling in ops/dr/ at actual recovery
# time, against a live database. Never contains the cipher passphrase itself.
manifest_tmp="$(mktemp /tmp/pgbackrest-manifest.XXXXXX)"
# The info JSON is passed as a file-path argument, not piped via stdin:
# combining `< file` with a `<<'PY'` heredoc on the same command would have
# the heredoc win the stdin redirect (bash applies redirections left-to-right,
# last one wins), silently feeding the script's own source text to
# `json.load(sys.stdin)` instead of the backup data — caught by testing this
# snippet standalone before it shipped.
python3 - "$stanza" "${PGBACKREST_REPO1_TYPE:-posix}" "${AZURE_KEY_VAULT_NAME:-}" "${PGBACKREST_REPO1_CIPHER_PASS_FILE:-}" /var/log/pgbackrest/last-info.json \
  > "$manifest_tmp" <<'PY' || { rm -f "$manifest_tmp"; echo "backup-runner: recovery-manifest generation failed (non-fatal; backup itself succeeded)" >&2; exit 0; }
import json, sys, datetime

stanza, repo_type, kv_name, cipher_pass_file, info_path = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4], sys.argv[5]
with open(info_path) as f:
    d = json.load(f)
backups = d[0].get("backup", []) if d else []
latest = backups[-1] if backups else {}

key_vault_secret_version = None
if cipher_pass_file:
    try:
        with open(cipher_pass_file + ".version") as f:
            key_vault_secret_version = f.read().strip() or None
    except OSError:
        pass

manifest = {
    "generated_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    "stanza": stanza,
    "repo1_type": repo_type,
    "backup_label": latest.get("label"),
    "backup_type": latest.get("type"),
    "backup_timestamp_stop": latest.get("timestamp", {}).get("stop"),
    "lsn_start": latest.get("lsn", {}).get("start"),
    "lsn_stop": latest.get("lsn", {}).get("stop"),
    "database_id": latest.get("database", {}).get("id"),
    "repo1_size_bytes": latest.get("info", {}).get("repository", {}).get("size"),
    "cipher_type": None,  # recorded in pgbackrest.conf itself, intentionally not duplicated here
    "key_vault_name": kv_name or None,
    "key_vault_secret_id": key_vault_secret_version,  # full versioned secret identifier, never the value
    "note": "Application migration version and Blob object/encryption-key-version correlation are NOT captured here; see ops/dr/ tooling at actual recovery time.",
}
json.dump(manifest, sys.stdout, indent=2)
print()
PY
if [ -s "$manifest_tmp" ]; then
  mv "$manifest_tmp" /var/log/pgbackrest/recovery-manifest-latest.json
  chown postgres:postgres /var/log/pgbackrest/recovery-manifest-latest.json 2>/dev/null || true
else
  rm -f "$manifest_tmp"
fi
