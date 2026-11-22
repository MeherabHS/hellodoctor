# Nightly database dump

Every night at **03:00 Africa/Nairobi**, the VM writes one compressed, timestamped dump of
the production database and keeps the newest **14**. Older dumps are deleted automatically, so
disk use stays bounded.

| What | Value |
|---|---|
| Schedule | `03:00 Africa/Nairobi` daily (systemd timer; catches up after downtime) |
| File | `/var/backups/hellodoctor/hellodoctor-YYYY-MM-DD_HHMM.dump` plus `.sha256` |
| Format | `pg_dump --format=custom --compress=9` (restore all or one table with `pg_restore`) |
| Kept | newest 14 (`BACKUP_KEEP`) |
| Resources | nothing between runs (a oneshot service started by a systemd timer); a few seconds a night at low CPU and disk priority (`nice`, `ionice` idle class) |
| After downtime | runs once when the VM is back, waiting up to 5 minutes for the database to start (`BACKUP_WAIT_SECONDS`) |
| Roles | `hellodoctor-…globals.sql` each night (roles without passwords), so a restore onto a new server works |
| Proof | every Sunday it restores the new dump into a throwaway database, checks it, and drops it (`BACKUP_RESTORE_TEST_DAY`) |
| Alerts | optional `BACKUP_ALERT_WEBHOOK_URL` receives any failure; failures always show in `systemctl status` and the journal |
| Guards | readable by root only; refuses to run with under 2 GB free; verified with `pg_restore --list` before it counts; no overlapping runs; failed runs leave no partial file |

This protects against a damaged database, a bad migration or a mistaken delete. It does
**not** protect against losing the VM's disk, because the dumps live on the same VM. The full
off-VM backup design (pgBackRest to Azure) is in `../compose.backup.yml` and is not enabled.

## Install (once, on the VM)

```bash
cd ~/hellodoctor            # the repository checkout on the VM
sudo install -m 0750 deploy/single-vm/nightly-dump/hellodoctor-nightly-db-dump.sh /usr/local/bin/
sudo install -m 0644 deploy/single-vm/nightly-dump/hellodoctor-db-dump.service /etc/systemd/system/
sudo install -m 0644 deploy/single-vm/nightly-dump/hellodoctor-db-dump.timer /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now hellodoctor-db-dump.timer

# Take the first dump now and check it worked
sudo systemctl start hellodoctor-db-dump.service
sudo journalctl -u hellodoctor-db-dump.service -n 20 --no-pager
sudo ls -lh /var/backups/hellodoctor
systemctl list-timers hellodoctor-db-dump.timer
```

To change the defaults, create `/etc/default/hellodoctor-db-dump`, for example:

```
BACKUP_KEEP=30
PG_CONTAINER=hellodoctor-vm-postgres-1
BACKUP_RESTORE_TEST_DAY=Sun
BACKUP_ALERT_WEBHOOK_URL=https://hooks.example.invalid/your-webhook
```

## Check a dump

```bash
cd /var/backups/hellodoctor
sudo sha256sum -c hellodoctor-2026-11-24_0300.dump.sha256
```

## Restore

On a **new** server, create the roles first (the backend sets their passwords when it starts):

```bash
sudo docker exec -i hellodoctor-vm-postgres-1 sh -c 'psql -U "$POSTGRES_USER" -d postgres' < /var/backups/hellodoctor/hellodoctor-2026-11-24_0300.globals.sql
```

"Role already exists" errors for the owner role are expected and harmless.

**Practise on a scratch database first.** This restores into a new database named
`restore_test` and leaves production untouched:

```bash
F=/var/backups/hellodoctor/hellodoctor-2026-11-24_0300.dump
sudo docker exec hellodoctor-vm-postgres-1 sh -c 'createdb -U "$POSTGRES_USER" restore_test'
sudo docker exec -i hellodoctor-vm-postgres-1 sh -c 'pg_restore -U "$POSTGRES_USER" -d restore_test --no-owner' < "$F"
sudo docker exec hellodoctor-vm-postgres-1 sh -c 'psql -U "$POSTGRES_USER" -d restore_test -c "select count(*) from appointments;"'
sudo docker exec hellodoctor-vm-postgres-1 sh -c 'dropdb -U "$POSTGRES_USER" restore_test'
```

Restoring over production is a deliberate outage: stop the backend
(`sudo docker stop hellodoctor-vm-backend-1`), take a fresh dump of the current state first,
restore with `pg_restore --clean --if-exists`, then start the backend again.

Never copy a dump to a personal computer: it contains patient data.
