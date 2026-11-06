#!/usr/bin/env bash
set -Eeuo pipefail
: "${PGBACKREST_STANZA:=hellodoctor}"
: "${PGBACKREST_REPO1_PATH:=/var/lib/pgbackrest/repo1}"
install -d -m 0750 -o postgres -g postgres /etc/pgbackrest /var/log/pgbackrest
install -d -m 0750 -o postgres -g postgres "${PGBACKREST_REPO1_PATH}"
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
# See backup-entrypoint.sh for the matching comment: file-based secret
# preferred (avoids `docker inspect`/`docker exec env` exposure), plain env
# var remains supported unchanged for backward compatibility.
cipher_pass="${PGBACKREST_REPO1_CIPHER_PASS:-}"
if [ -n "${PGBACKREST_REPO1_CIPHER_PASS_FILE:-}" ]; then
  if [ ! -r "${PGBACKREST_REPO1_CIPHER_PASS_FILE}" ]; then
    echo "PGBACKREST_REPO1_CIPHER_PASS_FILE is set but not readable: ${PGBACKREST_REPO1_CIPHER_PASS_FILE}" >&2
    exit 1
  fi
  cipher_pass="$(tr -d '\n' < "${PGBACKREST_REPO1_CIPHER_PASS_FILE}")"
fi
: "${cipher_pass:?PGBACKREST_REPO1_CIPHER_PASS or PGBACKREST_REPO1_CIPHER_PASS_FILE must be supplied by a secret manager}"

cat > /etc/pgbackrest/pgbackrest.conf <<EOF
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

[${PGBACKREST_STANZA}]
pg1-path=/var/lib/postgresql/data
pg1-socket-path=/var/run/postgresql
EOF
unset cipher_pass
chown postgres:postgres /etc/pgbackrest/pgbackrest.conf
chmod 0640 /etc/pgbackrest/pgbackrest.conf
exec /usr/local/bin/docker-entrypoint.sh "$@"
