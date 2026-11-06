# PostgreSQL 16 Bookworm manifest digest includes a native linux/arm64 child.
# The backup image is built with --platform linux/arm64 on the target release
# pipeline; no x86 emulation is used for production backups.
FROM postgres:16-bookworm@sha256:e8a07d2c7579b038938cdba6c539e580e5001caf9ea203d397b5cc83e76bd938

ARG PGBACKREST_VERSION=2.54
# python3 is required by backup-monitor.sh's JSON/staleness parsing (previously
# used without being installed — a latent defect that silently broke the
# monitor on every real run; fixed here as part of G-05). curl is required by
# alert-notify.sh's webhook delivery.
RUN apt-get update \
    && apt-get install -y --no-install-recommends pgbackrest ca-certificates cron python3 curl \
    && rm -rf /var/lib/apt/lists/*

RUN mkdir -p /etc/pgbackrest /var/lib/pgbackrest /var/log/pgbackrest /var/log/pgbackrest/alerts \
    && chown -R postgres:postgres /etc/pgbackrest /var/lib/pgbackrest /var/log/pgbackrest

COPY backup-entrypoint.sh /usr/local/bin/backup-entrypoint.sh
COPY backup-runner.sh /usr/local/bin/backup-runner.sh
COPY backup-monitor.sh /usr/local/bin/backup-monitor.sh
COPY alert-notify.sh /usr/local/bin/alert-notify.sh
COPY postgres-entrypoint.sh /usr/local/bin/postgres-entrypoint.sh
RUN chmod 0755 /usr/local/bin/backup-entrypoint.sh /usr/local/bin/backup-runner.sh /usr/local/bin/backup-monitor.sh /usr/local/bin/alert-notify.sh /usr/local/bin/postgres-entrypoint.sh

ENTRYPOINT ["/usr/local/bin/backup-entrypoint.sh"]
