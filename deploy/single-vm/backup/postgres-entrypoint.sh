#!/usr/bin/env bash
set -Eeuo pipefail
: "${PGBACKREST_STANZA:=hellodoctor}"
: "${PGBACKREST_REPO1_PATH:=/var/lib/pgbackrest/repo1}"
install -d -m 0750 -o postgres -g postgres /etc/pgbackrest /var/log/pgbackrest
install -d -m 0750 -o postgres -g postgres "${PGBACKREST_REPO1_PATH}"
if [ "${PGBACKREST_ARCHIVE_ASYNC:-n}" = y ]; then
  install -d -m 0750 -o postgres -g postgres "${PGBACKREST_SPOOL_PATH:-/var/spool/pgbackrest}"
fi
source /usr/local/bin/pgbackrest-conf-common.sh
generate_pgbackrest_conf "${PGBACKREST_STANZA}" /etc/pgbackrest/pgbackrest.conf
exec /usr/local/bin/docker-entrypoint.sh "$@"
