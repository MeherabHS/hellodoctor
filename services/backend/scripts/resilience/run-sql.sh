#!/bin/sh
set -eu
umask 077
initdb --auth=trust --no-locale --encoding=UTF8 > /scratch/init.log 2>&1
pg_ctl -l /scratch/postgres.log -o "-k /scratch -c listen_addresses='' -c shared_buffers=16MB -c max_connections=24 -c dynamic_shared_memory_type=mmap -c fsync=off -c statement_timeout=8000 -c lock_timeout=5000" start > /scratch/start.log 2>&1
trap 'pg_ctl -m immediate stop >/dev/null 2>&1 || true' EXIT INT TERM
createdb hellodoctor
export PGDATABASE=hellodoctor
# SQLx normally creates this empty metadata table before migration one. This
# raw-SQL fixture creates its schema only; it never fabricates migration rows.
psql -X -q -v ON_ERROR_STOP=1 -c 'CREATE TABLE _sqlx_migrations(version BIGINT PRIMARY KEY,description TEXT NOT NULL,installed_on TIMESTAMPTZ NOT NULL DEFAULT now(),success BOOLEAN NOT NULL,checksum BYTEA NOT NULL,execution_time BIGINT NOT NULL)'
for migration in /migrations/*.up.sql; do
  if ! psql -X -q -v ON_ERROR_STOP=1 -1 -f "$migration" > /scratch/migration.log 2>&1; then
    echo "MIGRATION_FAIL: $(basename "$migration")"
    tail -n 20 /scratch/migration.log
    exit 1
  fi
done
echo 'MIGRATIONS_UP=DEFENSE_PASS'
if [ "${1:-all}" = migrations ]; then exit 0; fi
psql -X -q -v ON_ERROR_STOP=1 -f /harness/fixture.sql > /scratch/fixture.log 2>&1 || { tail -n 20 /scratch/fixture.log; exit 1; }
for phase in obligation confirmation replay settlement approval payout; do
  if [ "$phase" = settlement ]; then
    # Activation is the preceding callback effect, not part of each Doctor
    # completion. Do not introduce a different lock order in the attacker.
    psql -X -q -v ON_ERROR_STOP=1 -c "UPDATE appointments SET status='CONFIRMED' WHERE id='aaaaaaaa-4444-4444-4444-444444444444' AND status='PENDING_PAYMENT'"
  fi
  # 100 operations per phase, at most 8 clients at once; no 100-process spike.
  if ! seq 1 100 | xargs -P 8 -I '{}' sh -c 'psql -X -q -v ON_ERROR_STOP=1 -v attempt="$1" -f "/harness/$2.sql" > "/scratch/$2-$1.log" 2>&1' sh '{}' "$phase"; then
    for log in /scratch/"$phase"-*.log; do
      if grep -q 'ERROR' "$log"; then tail -n 12 "$log"; break; fi
    done
    exit 1
  fi
  psql -X -q -v ON_ERROR_STOP=1 -v phase="$phase" -f /harness/observe.sql
done
psql -X -q -v ON_ERROR_STOP=1 -f /harness/negative.sql
# Verify reversible new economic migration on disposable data only.
psql -X -q -v ON_ERROR_STOP=1 -1 -f /migrations/20261110000001_economic_idempotency.down.sql > /scratch/down.log 2>&1 || { tail -n 20 /scratch/down.log; exit 1; }
psql -X -q -v ON_ERROR_STOP=1 -1 -f /migrations/20261110000001_economic_idempotency.up.sql > /scratch/reup.log 2>&1 || { tail -n 20 /scratch/reup.log; exit 1; }
echo 'ECONOMIC_DOWN_UP=DEFENSE_PASS'
