#!/usr/bin/env bash
# Shared pgbackrest.conf generation, sourced by both backup-entrypoint.sh and
# postgres-entrypoint.sh. Previously this logic was duplicated verbatim in
# both scripts — a real risk, since a config knob added to one and forgotten
# in the other would silently desynchronize the two containers' view of the
# same repository. Single source of truth here removes that risk by
# construction.
#
# Exposes one function: generate_pgbackrest_conf <stanza> <conf-path>
# Requires PGBACKREST_REPO1_PATH to already be set (callers differ on its
# default and on whether the path needs pre-creating).
set -Eeuo pipefail

# Fail fast and clearly if an operator sets PGBACKREST_REPO2_* expecting it to
# do something: this repository's config generator only ever emits repo1-*.
# A second Azure container (pg-wal-archive) cannot be used as a standalone
# WAL-only repository under pgBackRest's model (a repository holds both base
# backups and WAL; a second repository duplicates WAL into both, it does not
# split them) — see docs/BACKUP_RESTORE_RUNBOOK.md's single-repository
# decision. Silently ignoring these vars would let an operator believe a
# second repository is active when it is not.
for _v in PGBACKREST_REPO2_TYPE PGBACKREST_REPO2_AZURE_ACCOUNT PGBACKREST_REPO2_AZURE_CONTAINER PGBACKREST_REPO2_PATH; do
  if [ -n "${!_v:-}" ]; then
    echo "FATAL: ${_v} is set, but this deployment deliberately supports only repo1." >&2
    echo "A second pgBackRest repository duplicates WAL into both repositories and" >&2
    echo "does not give you an independent base-only/WAL-only split. See the" >&2
    echo "'single-repository architecture' decision in docs/BACKUP_RESTORE_RUNBOOK.md" >&2
    echo "before adding repo2-* support to this config generator." >&2
    exit 1
  fi
done
unset _v

pgbackrest_repo_options() {
  local repo_type="${PGBACKREST_REPO1_TYPE:-posix}"
  if [ "$repo_type" = azure ]; then
    local azure_key_type="${PGBACKREST_REPO1_AZURE_KEY_TYPE:-auto}"
    local azure_key_line=""
    if [ "$azure_key_type" != auto ]; then
      azure_key_line="repo1-azure-key=${PGBACKREST_REPO1_AZURE_KEY:?required for non-auto Azure authentication}"
    fi
    printf 'repo1-type=azure\nrepo1-azure-account=%s\nrepo1-azure-container=%s\nrepo1-azure-endpoint=%s\n%s\nrepo1-azure-key-type=%s\n' \
      "${PGBACKREST_REPO1_AZURE_ACCOUNT:?required for Azure repository}" \
      "${PGBACKREST_REPO1_AZURE_CONTAINER:?required for Azure repository}" \
      "${PGBACKREST_REPO1_AZURE_ENDPOINT:-core.windows.net}" \
      "$azure_key_line" \
      "$azure_key_type"
  fi
  # Async archiving + a persistent spool path reduce per-segment archive-push
  # latency and let pgBackRest batch pushes, but they are a new failure mode
  # (queue buildup, spool disk exhaustion if the repository is unreachable for
  # an extended period) that must be exercised under a disposable failure
  # drill before relying on it — default is "n" (unchanged, synchronous
  # behavior) until that drill has actually been run. See
  # docs/BACKUP_RESTORE_RUNBOOK.md.
  printf 'archive-async=%s\n' "${PGBACKREST_ARCHIVE_ASYNC:-n}"
  if [ "${PGBACKREST_ARCHIVE_ASYNC:-n}" = y ]; then
    printf 'spool-path=%s\n' "${PGBACKREST_SPOOL_PATH:-/var/spool/pgbackrest}"
  fi
}

# Resolves the repository cipher passphrase, in priority order:
#   1. PGBACKREST_REPO1_CIPHER_PASS_FILE, if it already exists and is readable
#      (an operator-mounted secret, e.g. a Docker/Swarm secret).
#   2. Azure Key Vault, if AZURE_KEY_VAULT_NAME + AZURE_KEY_VAULT_SECRET_NAME
#      are set: fetch-keyvault-secret.sh uses the container's Azure managed
#      identity (via IMDS) to retrieve the secret and writes it to
#      PGBACKREST_REPO1_CIPHER_PASS_FILE (creating the file). This is the
#      previously-"documented but not implemented" automation gap — it
#      complements, not replaces, the human custodian procedure in
#      docs/BACKUP_ENCRYPTION_KEY_CUSTODY_RUNBOOK.md, which still governs who
#      can read/rotate the secret's *value* in Key Vault itself (via Key
#      Vault RBAC, independent of what this container's identity can do) and
#      how a *replacement* VM gets its identity granted access in the first
#      place. Routine restart-of-the-same-VM is exactly the case managed
#      identity exists to automate.
#   3. PGBACKREST_REPO1_CIPHER_PASS, the plain env var (backward compatible,
#      but visible to `docker inspect`/`docker exec ... env` — prefer one of
#      the above).
resolve_cipher_pass() {
  local cipher_pass_file="${PGBACKREST_REPO1_CIPHER_PASS_FILE:-}"
  if [ -n "$cipher_pass_file" ] && [ ! -r "$cipher_pass_file" ] \
     && [ -n "${AZURE_KEY_VAULT_NAME:-}" ] && [ -n "${AZURE_KEY_VAULT_SECRET_NAME:-}" ]; then
    echo "resolve_cipher_pass: ${cipher_pass_file} not yet present; fetching from Azure Key Vault ${AZURE_KEY_VAULT_NAME}/${AZURE_KEY_VAULT_SECRET_NAME}" >&2
    /usr/local/bin/fetch-keyvault-secret.sh "$AZURE_KEY_VAULT_NAME" "$AZURE_KEY_VAULT_SECRET_NAME" "${AZURE_KEY_VAULT_SECRET_VERSION:-}" "$cipher_pass_file"
  fi

  local cipher_pass="${PGBACKREST_REPO1_CIPHER_PASS:-}"
  if [ -n "$cipher_pass_file" ]; then
    if [ ! -r "$cipher_pass_file" ]; then
      echo "PGBACKREST_REPO1_CIPHER_PASS_FILE is set but not readable: ${cipher_pass_file}" >&2
      exit 1
    fi
    cipher_pass="$(tr -d '\n' < "$cipher_pass_file")"
  fi
  : "${cipher_pass:?PGBACKREST_REPO1_CIPHER_PASS, PGBACKREST_REPO1_CIPHER_PASS_FILE, or Azure Key Vault (AZURE_KEY_VAULT_NAME/AZURE_KEY_VAULT_SECRET_NAME) must resolve to a non-empty passphrase}"
  printf '%s' "$cipher_pass"
}

# generate_pgbackrest_conf <stanza> <conf-path>
generate_pgbackrest_conf() {
  local stanza="$1" conf_path="$2"
  local repo_options
  repo_options="$(pgbackrest_repo_options)"
  local cipher_pass
  cipher_pass="$(resolve_cipher_pass)"

  cat > "$conf_path" <<EOF
[global]
${repo_options}
repo1-path=${PGBACKREST_REPO1_PATH}
repo1-retention-full=${PGBACKREST_REPO1_RETENTION_FULL:-7}
repo1-retention-diff=${PGBACKREST_REPO1_RETENTION_DIFF:-14}
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

[${stanza}]
pg1-path=/var/lib/postgresql/data
pg1-socket-path=/var/run/postgresql
EOF
  unset cipher_pass
  chown postgres:postgres "$conf_path"
  chmod 0640 "$conf_path"
}
