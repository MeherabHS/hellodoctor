# Canonical Benchmark Baseline (Phase 7, 2026-11-03)

A restorable, documented starting state for the `hellodoctor-m8-final-local` disposable
verification stack, built after discovering that hours of continuous, never-reset
benchmarking had drifted the environment (transactions grew from ~107K to ~138K rows,
2,149 stale Redis rate-limit keys, 7,777-item notification backlog) to the point where
system-level before/after comparisons were no longer trustworthy.

## Exact recipe (fully reproducible, deterministic — no `random()` anywhere in the seed SQL)

1. `TRUNCATE` every content table except migration-seeded reference/config tables
   (`legal_documents`, `notification_templates`, `notification_type_registry`,
   `kenyan_holidays`, `article_categories` — these are not reconstructed by seed.rs or
   the capacity scripts and must survive a reset). `legacy_admin_role_assignments` and
   `platform_payment_methods` were cascaded anyway (FK references into `users`) —
   confirmed harmless: the former is a one-time historical migration backfill tied to
   now-deleted user rows, the latter was already empty.
2. Restart the backend container. `SEED_DEV_DATA=true` is already set in
   `compose.local-verification.yml`, so this alone re-triggers `seed_dev_data()`
   (1 admin, 1 fixture patient, 2 fixture doctors + slots/wallets).
3. Run `services/backend/scripts/seed_capacity_dataset.sql` against the database
   (~31s). Adds 40 doctors, 1,150 patients, 10 admins, and ~120 days of deterministic
   appointment/slot/chat/transaction/telemetry/grievance history.
4. Run `services/backend/scripts/seed_capacity_dataset_scaleup_10k.sql` (~1.4s). Adds
   8,850 more bare patients (no history) to reach the 10,000-patient target.
5. `redis-cli FLUSHDB` on the stack's Redis container — zero stale rate-limit/session
   state at benchmark start.

## Verified final state

| Table | Count |
| --- | --- |
| `users` | 10,054 |
| `doctor_profiles` | 42 |
| `patient_profiles` | 10,001 |
| `appointments` | 13,440 |
| `transactions` | 13,440 |
| `chat_conversations` | 13,440 |
| `doctor_schedule_slots` | 62,405 |
| `notifications` | 81 (zero backlog) |
| Redis keys | 0 |

Matches the documented original Milestone 8 dataset sizing exactly (10,001 patients,
42 doctors).

## Snapshot file

`canonical_baseline.dump` (this directory) — PostgreSQL custom-format `pg_dump`,
12.8 MB, schema version `20261204000004` (current at time of dump).

**Restore procedure** (verified working, 2026-11-03):

```
docker cp canonical_baseline.dump hellodoctor-m8-final-local-postgres-1:/tmp/canonical_baseline.dump
docker exec hellodoctor-m8-final-local-postgres-1 psql -U postgres -c "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname='hellodoctor' AND pid <> pg_backend_pid();"
docker exec hellodoctor-m8-final-local-postgres-1 psql -U postgres -c "DROP DATABASE hellodoctor;"
docker exec hellodoctor-m8-final-local-postgres-1 psql -U postgres -c "CREATE DATABASE hellodoctor;"
docker exec hellodoctor-m8-final-local-postgres-1 pg_restore -U postgres -d hellodoctor /tmp/canonical_baseline.dump
docker exec hellodoctor-m8-final-local-redis-1 redis-cli FLUSHDB
docker restart hellodoctor-m8-final-local-backend-1
```

**Do NOT add `--no-owner --no-privileges`.** First attempt used those flags (reasoning:
avoid ownership conflicts) and they strip the original `GRANT` statements to the five
capability roles (`hellodoctor_api`, `_auth_worker`, `_payment_worker`,
`_system_worker`, `_finance_worker`) along with ownership — the backend then
crash-loops on startup (`permission denied for table dr_recovery_state`, confirmed live:
every protected table, since none of the roles the app actually connects as had any
grants left). Since the restore always targets the same Postgres instance that created
the dump, there's no cross-cluster ownership conflict to avoid — restore with full
owner/privilege information intact. Running `pg_terminate_backend` first is necessary
because `DROP DATABASE` fails while the backend still holds open connections.
Re-verified end to end after the fix: backend started clean, zero permission errors,
`/healthz` returned `200` within 5 seconds of restart.

Restoring into the live `hellodoctor` database (not a scratch copy) requires dropping
and recreating it first, since `pg_restore` doesn't truncate existing data on its own —
only ever do this against the disposable `hellodoctor-m8-final-local` stack.

## Not yet captured in this snapshot

Backend/web container image versions and configuration are NOT part of this dump — they
must be pinned/recorded separately per benchmark run (e.g. `docker inspect` the image
digest) alongside whichever dataset state is restored, per the "identical application
versions" requirement for any A/B comparison.
