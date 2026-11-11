# PostgreSQL 16 Bookworm manifest digest includes a native linux/arm64 child.
# The backup image is built with --platform linux/arm64 on the target release
# pipeline; no x86 emulation is used for production backups.
FROM postgres:16-bookworm@sha256:e8a07d2c7579b038938cdba6c539e580e5001caf9ea203d397b5cc83e76bd938

# Pinned, not floating: `apt-get install pgbackrest` with no version against
# the postgres:16-bookworm base (which already configures the PGDG apt repo)
# previously installed whatever PGDG happened to have published for bookworm
# on build day — confirmed to have drifted from the declared 2.54 default to
# an actually-installed 2.60.0 in local evidence, an unpinned/irreproducible
# build the external audit correctly flagged. 2.54.0 is the first release with
# Azure managed-identity support (`repo1-azure-key-type=auto`).
# 2.60.0-1.pgdg12+1 is confirmed available right now via
# `apt-cache madison pgbackrest` run against this exact base image
# (bookworm-pgdg/main, 2026-11-06) — matching the version already exercised in
# local evidence (docs/BACKUP_ENCRYPTION_KEY_CUSTODY_RUNBOOK.md references
# "the installed pgBackRest 2.60.0"). Re-run that same `apt-cache madison`
# check before bumping this pin — PGDG does not keep every historical version
# published indefinitely — and re-run the full local pgBackRest verification
# suite (ops/dr/verify_local_pgbackrest.ps1) after any bump.
ARG PGBACKREST_VERSION=2.60.0-1.pgdg12+1
# python3 is required by backup-monitor.sh's JSON/staleness parsing (previously
# used without being installed — a latent defect that silently broke the
# monitor on every real run; fixed here as part of G-05). curl is required by
# alert-notify.sh's webhook delivery and by fetch-keyvault-secret.sh's calls to
# Azure Instance Metadata Service and Key Vault. jq is required by
# fetch-keyvault-secret.sh to parse those JSON responses without a Python
# dependency on a path that must fail loudly, not silently, on malformed input.
RUN apt-get update \
    && apt-get install -y --no-install-recommends "pgbackrest=${PGBACKREST_VERSION}" ca-certificates cron python3 curl jq \
    && rm -rf /var/lib/apt/lists/* \
    && pgbackrest version | grep -qF "pgBackRest ${PGBACKREST_VERSION%-*}" \
    || { echo "FATAL: installed pgbackrest version does not match pinned ${PGBACKREST_VERSION}" >&2; exit 1; }

RUN mkdir -p /etc/pgbackrest /var/lib/pgbackrest /var/log/pgbackrest /var/log/pgbackrest/alerts \
    && chown -R postgres:postgres /etc/pgbackrest /var/lib/pgbackrest /var/log/pgbackrest

COPY pgbackrest-conf-common.sh /usr/local/bin/pgbackrest-conf-common.sh
COPY fetch-keyvault-secret.sh /usr/local/bin/fetch-keyvault-secret.sh
COPY backup-entrypoint.sh /usr/local/bin/backup-entrypoint.sh
COPY backup-runner.sh /usr/local/bin/backup-runner.sh
COPY backup-monitor.sh /usr/local/bin/backup-monitor.sh
COPY alert-notify.sh /usr/local/bin/alert-notify.sh
COPY postgres-entrypoint.sh /usr/local/bin/postgres-entrypoint.sh
RUN chmod 0755 /usr/local/bin/backup-entrypoint.sh /usr/local/bin/backup-runner.sh /usr/local/bin/backup-monitor.sh /usr/local/bin/alert-notify.sh /usr/local/bin/postgres-entrypoint.sh /usr/local/bin/pgbackrest-conf-common.sh /usr/local/bin/fetch-keyvault-secret.sh

ENTRYPOINT ["/usr/local/bin/backup-entrypoint.sh"]
