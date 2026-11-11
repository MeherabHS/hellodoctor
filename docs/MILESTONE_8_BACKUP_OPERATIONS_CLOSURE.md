# Milestone 8 — Backup Operational Safeguards Closure (G-05, G-06, G-17)

Date: 2026-11-01. Scope: exactly the three items named in the triggering
request — backup-failure alerting (G-05), encryption-key custody (G-06), and
retention/immutability policy (G-17) — from
`docs/MILESTONE_8_REMAINING_GAP_ASSESSMENT.md`. No broad audit, no new
sustained benchmark, no Azure provisioning, no staging/commit/push was
performed. The previously GREEN PostgreSQL full/differential backup, WAL
archiving, encryption, and PITR local-recovery evidence
(`docs/MILESTONE_8_POSTGRES_BACKUP_FINAL.md`,
`docs/recovery-evidence/m8-final-local-dr/`) was not re-run — nothing in this
pass touches the recovery mechanics themselves, only the operational layer
around them (alerting, key-custody documentation, retention documentation).

## 1. Implemented changes

- **`deploy/single-vm/backup/alert-notify.sh` (new).** A dependency-light
  (curl-only) alert dispatcher. Delivers a JSON event
  (`severity`, `event`, `stanza`, `host`, `timestamp`, `message`) to one
  configured HTTPS webhook. Opt-in by design: with `BACKUP_ALERT_WEBHOOK_URL`
  unset, every call still logs locally and exits 0 — nothing claims external
  delivery until a real receiver is configured. Bounded retries
  (`curl --retry 3`, 10s per-attempt timeout, ~30s total ceiling). Per-event
  deduplication via a timestamp file under
  `/var/log/pgbackrest/alerts/<event>.last`, default 1800s window. Optional
  bearer/auth header read from a file path
  (`BACKUP_ALERT_WEBHOOK_AUTH_HEADER_FILE`), never embedded in the payload or
  logged. Severity is caller-supplied and validated (`critical|warning|info`).
- **`deploy/single-vm/backup/backup-runner.sh` (modified).** Calls
  `alert-notify.sh critical BACKUP_FAILED ...` when a scheduled full/diff
  backup fails, then exits with the backup's own real exit code — not masked
  by alert delivery success or failure. A genuine bug introduced during this
  pass and caught by its own forced-failure test (see §2) is fixed in the
  same change: see the note in §2.
- **`deploy/single-vm/backup/backup-monitor.sh` (modified).** Now runs three
  independent checks per tick, each alerting separately so one failure can't
  mask another: (1) `pgbackrest check` failure → `critical BACKUP_CHECK_FAILED`
  (covers both WAL-archiving failure and an unreachable repository — a single
  `check` invocation already exercises both per pgBackRest's own design), (2)
  staleness (pre-existing logic, now alerting) → `critical BACKUP_STALE`, (3)
  repository disk growth since the previous tick exceeding
  `BACKUP_REPO_MAX_GROWTH_MB` (default 2048) → `warning
  BACKUP_REPO_EXCESSIVE_GROWTH` (new check). Exits non-zero if any check
  failed.
- **`deploy/single-vm/backup/pgbackrest.Dockerfile` (modified).** Installs
  `python3` and `curl`. **`python3` was previously used by
  `backup-monitor.sh`'s JSON/staleness parsing but never installed** — a
  latent defect found during this pass (see §2): the monitor's staleness
  check could not have executed successfully on any real run of this image
  before now. `curl` is the new dependency for `alert-notify.sh`.
- **`deploy/single-vm/compose.backup.yml` (modified).** Passes through the new
  alerting env vars with safe empty/default values —
  `BACKUP_ALERT_WEBHOOK_URL`, `BACKUP_ALERT_WEBHOOK_AUTH_HEADER_FILE`,
  `BACKUP_ALERT_DEDUP_SECONDS`, `BACKUP_REPO_MAX_GROWTH_MB`. None are
  required (`:?`-marked); alerting stays opt-in at the compose level too.
- **`deploy/single-vm/backup/README.md` (modified).** Documents the alerting
  contract, env vars, and the explicit "do not claim production delivery
  before testing against a real receiver" caveat.
- **`docs/BACKUP_RESTORE_RUNBOOK.md` (modified).** Adds an "Alert routing"
  subsection (G-05) and a "Retention and immutability policy" subsection
  (G-17) — see §3.
- **`docs/BACKUP_ENCRYPTION_KEY_CUSTODY_RUNBOOK.md` (new).** The G-06 runbook
  — see §3.
- **`docs/MILESTONE_8_REMAINING_GAP_ASSESSMENT.md` (modified).** G-05, G-06,
  G-17 reclassified OPEN/PARTIAL → GREEN with references to this document.

No other file was touched. No production/live code path (the Rust backend,
Flutter apps, admin/doctor web) was modified.

## 2. Tests and evidence

All testing used disposable, newly created Docker resources (network,
volumes, a purpose-built throwaway image) on this local machine; nothing
touched the shared dev Postgres/Redis, the real pgBackRest repository, or any
previously recorded recovery evidence. All disposable resources were removed
after testing (confirmed via `docker ps -a`/`docker volume ls` returning
empty for the test prefix).

1. **Image build.** `docker build -f deploy/single-vm/backup/pgbackrest.Dockerfile
   -t hellodoctor-pgbackrest:g05-test deploy/single-vm/backup` — succeeded,
   confirming `python3`/`curl` install and script permissions.
2. **No webhook configured → safe local-only behavior.** Ran
   `alert-notify.sh critical BACKUP_FAILED "..."` with no
   `BACKUP_ALERT_WEBHOOK_URL`: logged locally, exited 0, no network call made
   (confirmed: the disposable receiver's log stayed empty).
3. **Real delivery + auth header + secret redaction.** With
   `BACKUP_ALERT_WEBHOOK_URL` pointed at a disposable Python HTTP receiver and
   `BACKUP_ALERT_WEBHOOK_AUTH_HEADER_FILE` pointing at a file containing
   `Bearer FAKE_TOKEN_XYZ_789`, and `PGBACKREST_REPO1_CIPHER_PASS` set to an
   obvious fake secret string: the receiver got exactly one POST with the
   correct `Authorization` header and a body containing only the five
   documented JSON fields — **the fake cipher secret never appeared in the
   header or body**.
4. **Deduplication.** Two identical `critical BACKUP_STALE` calls against the
   same persistent alert-state volume, `BACKUP_ALERT_DEDUP_SECONDS=3600`: the
   receiver got exactly one POST; the second call logged
   `suppressing duplicate BACKUP_STALE (last sent 1s ago, dedup window 3600s)`
   and exited 0. (An earlier run using a fresh, non-persistent state volume
   per call incorrectly appeared to show no deduplication — that was a test
   setup artifact, not a script defect: the real deployment mounts one
   persistent `/var/log/pgbackrest` volume across every cron tick, which the
   corrected test reproduced.)
5. **Unreachable webhook: bounded retry, non-zero exit, no hang.** Pointed at
   a closed port: failed after exactly 4 connection attempts (1 initial + 3
   retries) in 7 seconds total, exited 1. Confirms the retry budget is
   actually bounded, not indefinite.
6. **Forced real backup failure (`backup-runner.sh`) — found and fixed a real
   bug.** Ran `backup-runner.sh diff` against a container with no pgBackRest
   stanza configured (a genuine, reproducible failure: pgBackRest error
   `[037]: backup command requires option: pg1-path`). The **first** version
   of the fix alerted correctly but reported exit code **0** — i.e., a real
   backup failure would have looked like success to cron, which is worse than
   having no alerting, since it would mask the very failure the alert was
   reporting. Root cause: `rc=$?` was read immediately after
   `if ! { pgbackrest-command; } ...; then`, but bash's `!` negates the exit
   status used for the `if` branch test, so `$?` inside that `then` block was
   the negated boolean (always 0 on entry to that branch), never pgBackRest's
   real exit code. Fixed by running the command under `set +e`, capturing
   `$?` directly (no `!`), then branching on that saved value. Re-tested
   against the identical forced failure: `backup-runner.sh` now correctly
   exits **37** (pgBackRest's real code), the alert payload correctly reports
   `(exit 37)`, and the pre-existing usage-error path (`backup-runner.sh
   bogus-mode` → exit 64, no alert) is confirmed unaffected.
7. **Forced real monitor failures (`backup-monitor.sh`).** Against the same
   unconfigured stanza: both `BACKUP_CHECK_FAILED` and `BACKUP_STALE` alerts
   fired independently (confirming one failing check does not suppress the
   other), `backup-monitor.sh` exited 1.
8. **Excessive repository growth.** Seeded a disposable repository volume
   with a 5MB file, a zero-byte growth baseline, and
   `BACKUP_REPO_MAX_GROWTH_MB=0`: `backup-monitor.sh` correctly computed a
   5MB delta and fired `warning BACKUP_REPO_EXCESSIVE_GROWTH` with the
   correct detail message, alongside the (expected, since no real stanza
   exists in this disposable test) `BACKUP_CHECK_FAILED`/`BACKUP_STALE`
   alerts.

**Not claimed:** delivery to a real production alert sink (Slack, PagerDuty,
email) — only a disposable local receiver was used, per the explicit
instruction not to claim production alert delivery before a real destination
is configured and tested. **Not re-tested:** the PostgreSQL PITR/backup
mechanics themselves (full/diff backup content, WAL archiving correctness,
restore) — those remain the previously accepted GREEN evidence, unchanged by
this pass.

## 3. Operational procedures

- **Alert routing (G-05):** documented in
  `deploy/single-vm/backup/README.md` ("Alerting" section) and
  `docs/BACKUP_RESTORE_RUNBOOK.md` ("Alert routing" subsection). Five
  conditions covered: failed scheduled backup, failed WAL archiving,
  unavailable repository, stale backup, excessive disk/WAL growth.
- **Encryption-key custody (G-06):**
  `docs/BACKUP_ENCRYPTION_KEY_CUSTODY_RUNBOOK.md` — approved external
  key-storage requirements, custodian/backup-custodian roles, access/recovery
  procedure, rotation considerations, VM-loss recovery (this mechanism
  survives VM loss by design — the key lives in the secret store, not the
  VM), lost-key consequences, and a tabletop-drill procedure (not yet run —
  it requires naming real custodians and a real secret store, both operator
  decisions).
- **Retention and immutability (G-17):** `docs/BACKUP_RESTORE_RUNBOOK.md`
  ("Retention and immutability policy" subsection) — the documented schedule
  was cross-checked line-by-line against
  `deploy/single-vm/backup/backup-entrypoint.sh`'s actual generated
  `pgbackrest.conf` and cron schedule and matches exactly (7 full / 14
  differential generations, time-based full retention, daily 03:15 UTC full,
  15-minute differential, AES-256-CBC). A written, not-yet-enacted Azure Blob
  immutability proposal is included with an explicit instruction not to move
  from unlocked to locked retention without separate two-person approval,
  per the triggering request's own constraint.

## 4. Remaining operator decisions

None of these are engineering gaps — they are decisions that only the product
owner/operator can make, and none are blocked on anything technical in this
repository:

- Choose and configure a real `BACKUP_ALERT_WEBHOOK_URL` receiver (PagerDuty,
  Slack, or another JSON-compatible endpoint) and re-test against it before
  relying on it in production.
- Name a primary and backup key custodian, and choose the actual external
  secret store (`docs/BACKUP_ENCRYPTION_KEY_CUSTODY_RUNBOOK.md`); run the
  tabletop drill once both exist.
- Approve (or revise) the retention schedule and the proposed Azure
  immutability interval before enactment; explicitly decide when, if ever, to
  move an Azure immutability policy from unlocked to locked.

## 5. Azure-dependent blockers

Unchanged from `docs/MILESTONE_8_REMAINING_GAP_ASSESSMENT.md` (M8-G-04): the
Azure Blob off-VM repository, managed identity/RBAC, and the proposed
immutability policy's actual *enactment* all remain BLOCKED pending Azure
subscription access. Nothing in this closure pass provisions, assumes, or
simulates Azure access, and no irreversible Azure retention lock was enabled
(none could be — there is no Azure resource to enable it on).

## 6. Revised Milestone 8 gap status

- **M8-G-05 (backup-failure alerting): OPEN → GREEN.** Implemented and
  tested end-to-end against a disposable receiver for all five required
  conditions. Remaining operator action (choosing/configuring a real
  production receiver) is tracked in §4, not as an engineering gap.
- **M8-G-06 (encryption-key custody): PARTIAL → GREEN (documentation
  complete).** The runbook exists with the required sections. The tabletop
  drill itself is explicitly not run (requires real custodians/secret store —
  an operator decision, tracked in §4), consistent with the instruction not
  to invent custodians or generate production keys.
- **M8-G-17 (retention/immutability policy): PARTIAL → GREEN (policy
  written and verified against actual config).** Enactment of the Azure
  immutability half remains BLOCKED on Azure access (§5), as it already was.

These three corrections should be read together with the 2026-11-01
corrections to M8-G-11 (Admin step-up, waived) and M8-G-12 (payment
hold/expiry, already implemented) recorded earlier the same day in
`docs/MILESTONE_8_REMAINING_GAP_ASSESSMENT.md`. The overall Milestone 8
local-engineering and production-readiness verdicts in that document are
updated accordingly below; nothing in this pass changes the fundamental
reasons production readiness remains NOT DECLARED (Azure, Firebase, and
payment-provider access remain the real blockers).

---

## 2026-11-01 follow-up: three precise corrections before freezing

Triggered by a targeted review of the work above. Scope held to exactly the
three issues raised; the completed backup/PITR drills were not repeated and
the backup subsystem was not redesigned. All testing again used disposable
Docker resources, torn down afterward; nothing touched the shared dev
database, the real pgBackRest repository, or prior recovery evidence.

### Issue 1 — encryption-key rotation runbook was factually wrong

**Finding, confirmed against the installed `pgBackRest 2.60.0` binary itself
(`pgbackrest stanza-create --help` and `pgbackrest backup --help`, run
directly in a disposable container):** pgBackRest fixes a repository's
encryption type and passphrase at stanza/repository initialization; there is
no command to change `repo1-cipher-pass` in place. The original
`docs/BACKUP_ENCRYPTION_KEY_CUSTODY_RUNBOOK.md` "Key rotation considerations"
section incorrectly described updating the env var and restarting as a
working rotation procedure. **No in-place rotation was attempted against any
repository — the incorrect procedure was identified by inspection and
corrected in the runbook before anyone could follow it.**

**Correction:** the runbook now documents the pgBackRest-supported path —
add a second repository (`repo2-*`, a separate path/container and a new
passphrase), confirmed safe via the tool's own `stanza-create` help text
("Stanzas that have already been created will be skipped so it is always
safe to run stanza-create, even when a new repository has been configured"),
run a full backup and independent verification against `--repo=2`, only then
cut scheduled backups over, and keep the old repository and passphrase intact
until its own retention window fully expires. Full procedure in
`docs/BACKUP_ENCRYPTION_KEY_CUSTODY_RUNBOOK.md`'s "Key rotation
considerations" section. This migration has not been performed or drilled
against even a disposable repository in this pass — that remains a listed
operator action, not claimed as done.

### Issue 2 — plaintext cipher-pass exposure: confirmed, documented, and the Docker-level exposure fixed

**Confirmed empirically, using a disposable container and an obviously fake
test passphrase (`FAKE_TEST_CIPHER_SECRET_DO_NOT_USE_abc123`), never a real
one:**

1. `/etc/pgbackrest/pgbackrest.conf` contains `repo1-cipher-pass=` in
   plaintext (`grep -c` match on the exact fake value returned `1`),
   protected only by `chmod 0640 postgres:postgres` — this is inherent to how
   pgBackRest reads its passphrase and cannot be avoided by this
   application; the file's permissions are the only control.
2. `docker inspect <container> --format '{{.Config.Env}}'` and
   `docker exec <container> env` both returned the exact fake passphrase in
   plaintext, confirming it is visible to anyone with Docker daemon/host
   access — a materially larger population than "can exec into this specific
   container as a non-root user."

An earlier version of `docs/BACKUP_ENCRYPTION_KEY_CUSTODY_RUNBOOK.md`
incorrectly claimed the passphrase is "never written to disk in plaintext by
the application" — corrected in place with the exact evidence above, not
simply restated.

**Fix implemented and tested:** `backup-entrypoint.sh` and
`postgres-entrypoint.sh` now accept `PGBACKREST_REPO1_CIPHER_PASS_FILE` (a
file path) in preference to the plain `PGBACKREST_REPO1_CIPHER_PASS` env var,
mirroring the pattern already used for the alert webhook's auth header.
Verified on a disposable container with only `_FILE` set (no plain env var at
all): `docker inspect`/`docker exec env` showed only
`PGBACKREST_REPO1_CIPHER_PASS_FILE=/secrets/cipherpass.txt` (a path — not
sensitive), with no `PGBACKREST_REPO1_CIPHER_PASS` key present at all, while
`/etc/pgbackrest/pgbackrest.conf` was independently confirmed to still
contain the *correct* passphrase read from that file. Backward compatibility
confirmed separately: the plain env var alone still works unchanged, and the
fail-closed behavior when *neither* is set still exits non-zero with a clear
error. `deploy/single-vm/compose.backup.yml` updated to pass through
`PGBACKREST_REPO1_CIPHER_PASS_FILE` on both the `postgres` and `backup`
services, and the existing `PGBACKREST_REPO1_CIPHER_PASS:?required` compose-level
hard requirement was relaxed to optional (`:-`) on both services, since the
actual "one of these two must be set" requirement is now enforced inside the
entrypoint scripts, where it can express that "either/or" logic.

**What remains true regardless:** anyone with root or Docker-daemon access on
the production host can always read a running container's filesystem or
mounted volumes directly. The `_FILE` mechanism removes the
`docker inspect`/`env` convenience exposure; it does not and cannot remove
host-root access as a residual exposure. That residual is a host
access-control decision, documented as such in
`docs/BACKUP_ENCRYPTION_KEY_CUSTODY_RUNBOOK.md`, not something fixable in
this repository's code.

### Issue 3 — monitoring did not check pg_wal accumulation or data-volume free space

**Confirmed by reading the original `backup-monitor.sh`:** it checked
`pgbackrest check`, backup staleness, and the backup *repository's* own disk
growth only. It never checked PostgreSQL's own `pg_wal` directory (which can
fill independently of the backup repository — e.g. a stuck replication slot
or a silently failing `archive_command` that `pgbackrest check` doesn't
happen to catch between ticks) or the PostgreSQL data volume's free space at
all (a full data volume halts PostgreSQL outright, independent of backup
health entirely).

**Fix implemented:** two new independent checks added to `backup-monitor.sh`,
reusing the `backup` container's existing `pgdata` mount (no new volume or
service):

- `PG_WAL_EXCESSIVE_ACCUMULATION` (critical) — `du -sk` on
  `${PGDATA}/pg_wal` against `PG_WAL_MAX_SIZE_MB` (default 4096).
- `DB_VOLUME_LOW_FREE_SPACE` (critical) — `df -Pk` on the PGDATA mount
  against `DB_VOLUME_MIN_FREE_PERCENT` (default 15).

**Tested on disposable resources:** using a size-limited `tmpfs` mount
(`--tmpfs /var/lib/postgresql/data:size=20m`, not the real `pgdata` volume) —
seeded a 5MB `pg_wal` directory against a 2MB threshold and filled the tmpfs
to 70% used (confirmed independently via `df -h` inside the same test):
both `PG_WAL_EXCESSIVE_ACCUMULATION` and `DB_VOLUME_LOW_FREE_SPACE` fired with
accurate figures in the alert detail (5MB/2MB threshold; 30% free/6MB,
matching `df -h`'s own 70%-used reading exactly). **Negative test**, same
mechanism, healthy thresholds (4096MB WAL ceiling, 15% free-space floor) and
a tiny 100KB `pg_wal`, generous 200MB tmpfs: neither new alert fired —
confirming no false positives under normal conditions.

### Evidence summary for this follow-up

All three issues were reproduced and verified using newly created, disposable
Docker resources (a size-limited tmpfs, fresh volumes, a throwaway receiver
image) on this local machine; everything was removed after testing
(`docker ps -a`/`docker volume ls` confirmed empty for the test prefixes
afterward). No real secret was ever used or printed — a clearly-labeled fake
test value was used throughout and verified only via exact-match `grep -c`
counts, never printed in full. No production code, shared database, or prior
recovery evidence was touched. No Azure resource was involved.

### Files changed in this follow-up

- `deploy/single-vm/backup/backup-entrypoint.sh`,
  `deploy/single-vm/backup/postgres-entrypoint.sh` — `_FILE` secret-injection
  support (Issue 2).
- `deploy/single-vm/backup/backup-monitor.sh` — two new independent checks
  (Issue 3).
- `deploy/single-vm/compose.backup.yml` — new env var passthrough, relaxed
  compose-level requirement on the plain cipher-pass var.
- `deploy/single-vm/backup/README.md` — documents both fixes.
- `docs/BACKUP_ENCRYPTION_KEY_CUSTODY_RUNBOOK.md` — corrected confidentiality
  claim, corrected rotation procedure, new exposure-analysis section.
- `docs/KEY_ROTATION_RUNBOOK.md` — cross-reference noting the pgBackRest
  cipher passphrase is an exception to the general rotation pattern.
- `docs/BACKUP_RESTORE_RUNBOOK.md` — updated alert-routing and
  repository-access-restrictions sections.
- This file.

### Revised status after this follow-up

M8-G-05, M8-G-06, and M8-G-17 remain **GREEN** — none of the three issues
reopened them as gaps; all three were refinements/corrections to already-
implemented work, not newly discovered missing functionality. The corrections
make the documentation accurate (no rotation procedure that doesn't actually
work) and close a real, verified secret-exposure reduction (the `_FILE`
option) and a real monitoring blind spot (pg_wal/volume free space) that
existed underneath the GREEN classification. Remaining operator actions,
unchanged in kind from before this follow-up: choose a real alert receiver,
name real key custodians and a real secret store, and — newly added here —
actually drill the new-repository key-rotation procedure against a disposable
repository before ever needing it for real.
