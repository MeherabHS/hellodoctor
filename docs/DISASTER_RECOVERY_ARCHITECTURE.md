# HelloDoctor disaster recovery architecture

## Decision

The production design selects **Azure Database for PostgreSQL Flexible Server**
for the authoritative database and an application VM only for the stateless
Rust and web containers. PostgreSQL on the VM with pgBackRest remains a
disposable local simulation and a fallback design to review only if the pilot
cannot use the managed service.

| Concern | Flexible Server | PostgreSQL on VM + pgBackRest |
|---|---|---|
| Failure-domain isolation | Service, zones and optional geo-redundant backup are outside the app VM | Requires a second backup account/VM and independent operators |
| Pilot RPO | PITR target is normally under 15 minutes when logs and service health are current; geo-restore has a larger regional RPO | WAL archive lag can meet 15 minutes, but must be monitored and repaired by the team |
| Major recovery RTO | New server restore plus role/RLS validation; target <=4 hours | Base backup plus WAL replay; target <=4 hours only after measured drills |
| Operator complexity | Lower; HA, backups and WAL handling are managed | High; pgBackRest repository, WAL retention, corruption checks and failover are our responsibility |
| Cost drivers | Compute tier, HA standby, storage, IOPS, geo-redundant backup and retention | VM, managed disks, second backup repository, cross-region storage and operations |
| Limitations | PITR restore creates a new server; HA is not automatically enabled on the restored server; extensions and role privileges require compatibility checks | Full PostgreSQL control, but a compromised VM or role can damage the backup path unless the repository is separately protected |

Flexible Server backup retention is configured at server creation (7–35 days)
and geo-redundant backup cannot be enabled after creation. A restored server
must be treated as a new recovery target and reconfigured for HA, private
networking and application DNS. See Microsoft's [backup and restore
documentation](https://learn.microsoft.com/en-us/azure/postgresql/backup-restore/concepts-backup-restore).

Native Flexible Server PITR is the rapid local-region recovery path. HA is the
automatic failover path when the selected tier/zone model supports it. Geo-
restore is the regional-disaster path and has a larger RPO/RTO. Azure Backup
for PostgreSQL Flexible Server must be evaluated against the selected region,
server tier and API version: v1 generally provides a daily recovery-point
objective, while v2 is preview and has a narrower support matrix. Neither
vaulted path is advertised as a 15-minute RPO. Independent restore requires a
new compatible target server, private-network/DNS setup and post-restore HA
configuration.

## Failure-domain diagram

```text
Patients/Doctors/Admins
          |
     approved ingress
          |
   Azure VM (stateless app) ---- managed identity ----> private Blob containers
          |                                             clinical / article-media
          +-------------------- private DNS ------------+
          |
 Azure Database for PostgreSQL Flexible Server (HA + PITR)
          |
          +--> geo-redundant PITR / Azure Backup vault (immutable, separate admin)
          +--> recovery journal / manifest (separate protected store)
```

The database is the authority for ownership, publication, erasure, finance and
notification state. Blob metadata includes the fixed container, object key,
expected hash, encryption-key identifier/version and retention disposition.
No restore operation blindly restores every deleted blob.

## Local engineering boundary

The single-VM fallback now has an explicit pgBackRest integration under
`deploy/single-vm/backup/`. It provides continuous WAL archive-push, scheduled
full/differential backups, time-based retention, AES-256-CBC repository
encryption, a stale-backup monitor and a non-overlap lock. Activation is
opt-in and fails closed when the repository or cipher secret is absent. The
repository must be a separate volume or Azure Blob container; the PostgreSQL
data volume is never the only backup copy.

The local disposable verification evidence is
`docs/recovery-evidence/m8-pgbackrest-local/pgbackrest-local.json`. It covers
full backup, WAL archive checks, retention configuration, encrypted repository
access and wrong-key rejection. It does not establish Azure off-VM durability,
managed-identity authentication, or a replacement-VM restore drill.

`ops/dr/hellodoctor_dr.py` provides read-only inventory, artifact manifests,
guarded recovery plans and a guarded `pg_restore`. A destructive restore target
must be a child of `HELLODOCTOR_DR_ROOT`, contain the exact
`.hellodoctor-disposable-target` marker and receive `--confirm-disposable`.
There is no default production DSN and no cloud credential handling.

The application has `RECOVERY_MODE=true`: the scheduler does not start, FCM is
replaced by the no-op transport, and the global guard rejects every external
route except health and the token-protected recovery status probe. Patient,
Doctor and Admin login, refresh, logout, registration, WebSocket upgrades and
clinical reads are unavailable until reconciliation is complete. The probe
requires a separately delivered token whose SHA-256 digest is configured as
`RECOVERY_OPERATOR_TOKEN_SHA256`; the raw token is never stored. A restart with
`RECOVERY_MODE=false` is required after an authorized cutover decision.
