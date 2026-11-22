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

## See it in Admin Web

Admin Web shows a **Database backups** card at the top of **Operations & Safety → App Errors & Incidents**:
**Healthy**, **Failed** (with the reason), **Overdue** (no successful backup for 26 hours, which also catches a timer
that stopped), **Never run** or **Not set up**, with the last backup's time and size, copies kept and the last Sunday
test restore. It is admin-only on purpose: backup timing is useful to an attacker, so it is never on a public page.

How it is wired: after every run the script writes small JSON files (no data, no secrets) to
`/var/lib/hellodoctor-backup-status` (`BACKUP_STATUS_DIR`). `../compose.yml` mounts that folder read-only into the
backend at `/run/backup-status`, and `GET /api/v1/admin/system/backup-status` reads it. After pulling this change,
recreate the backend once so it gets the mount: `sudo docker compose ... up -d backend` with your usual files.

## Telegram alerts

All alerts go to your own Telegram bot.

| Alert | Sent by | When | Sound |
|---|---|---|---|
| Backup failed | this script on the VM | the moment a run fails, with the reason | loud |
| Backup overdue | `hellodoctor-backup-check.timer` on the VM | 09:00 Nairobi daily, if the newest success is over 26 hours old (catches a stopped timer) | loud |
| Backup ok | this script | after each nightly success (`BACKUP_TELEGRAM_ON_SUCCESS=off` to skip) | silent |
| Site down | `.github/workflows/uptime-watch.yml` on GitHub | every 5 minutes while `/healthz` fails; works when the whole VM is dead | loud |

### Create the bot (5 minutes)

1. In Telegram, open **@BotFather**, send `/newbot`, pick a name. It replies with the **bot token**.
2. Open your new bot and press **Start**, so it is allowed to message you.
3. Get your **chat id**: open `https://api.telegram.org/bot<TOKEN>/getUpdates` in a browser after pressing Start and
   read `"chat":{"id":...}`. For a group, add the bot to the group and send a message first.
4. To make alerts ring loudly, set a custom notification sound for this chat in Telegram and allow it through
   Do Not Disturb in your phone's settings. Silent "ok" messages still arrive without sound.

### Connect it

On the VM, in `/etc/default/hellodoctor-db-dump` (readable by root only: `sudo chmod 600`):

```
TELEGRAM_BOT_TOKEN=123456:ABC...
TELEGRAM_CHAT_ID=123456789
```

On GitHub, under Settings, Secrets and variables, Actions: secrets `TELEGRAM_BOT_TOKEN` and `TELEGRAM_CHAT_ID`, and
a variable `UPTIME_URLS` with the addresses to watch separated by spaces, for example
`https://api.example.org/healthz https://admin.example.org`. Run the workflow once by hand with **Send a test
message** ticked to check the bot reaches you.

Never put the bot token in the repository or on a personal computer.

## Install (once, on the VM)

```bash
cd ~/hellodoctor            # the repository checkout on the VM
sudo install -m 0750 deploy/single-vm/nightly-dump/hellodoctor-nightly-db-dump.sh /usr/local/bin/
sudo install -m 0644 deploy/single-vm/nightly-dump/hellodoctor-db-dump.service /etc/systemd/system/
sudo install -m 0644 deploy/single-vm/nightly-dump/hellodoctor-db-dump.timer /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now hellodoctor-db-dump.timer
sudo install -m 0644 deploy/single-vm/nightly-dump/hellodoctor-backup-check.service /etc/systemd/system/
sudo install -m 0644 deploy/single-vm/nightly-dump/hellodoctor-backup-check.timer /etc/systemd/system/
sudo systemctl daemon-reload && sudo systemctl enable --now hellodoctor-backup-check.timer

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
TELEGRAM_BOT_TOKEN=123456:ABC...
TELEGRAM_CHAT_ID=123456789
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
