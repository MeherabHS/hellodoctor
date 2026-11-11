# Disaster recovery plan

## Provisional objectives

- Database PITR RPO: **15 minutes or better** in the primary region.
- Major database recovery RTO: **4 hours or better**.
- Replacement VM RTO: **1 hour or better** when the database and Blob service
  remain available.
- Regional recovery has a separate, larger RPO and must never be reported as a
  local PITR result.

| Failure | Detection | Containment | Recovery | RPO/RTO | Approval and proof |
|---|---|---|---|---|---|
| App process crash | health/readiness failure | restart one container | restart stateless services | minutes / <1h | on-call; health and auth checks |
| VM loss | VM/ingress failure | isolate old identity and DNS | recreate VM from reviewed images and private env | 0 app data / <=1h | platform operator; smoke tests |
| DB unavailable | connection/readiness alerts | stop mutations, set recovery mode | PITR to a new server, validate roles/RLS/migrations | <=15m / <=4h | DBA + recovery operator |
| Accidental row/table deletion | audit/monitoring or report | freeze writes and workers | PITR to isolated target, export reviewed rows or cut over | <=15m / <=4h | DBA + data owner |
| Bad migration | migration/health failure | keep recovery mode; do not rerun blindly | restore pre-migration point, validate binary/schema pair | <=15m / <=4h | release owner + DBA |
| DB corruption | checksum/query/restore drill failure | isolate source | restore prior valid point and reconcile external journals | <=15m / <=4h | incident authority |
| Blob deletion/corruption | manifest/hash probe | deny affected reads; preserve metadata | recover only referenced authorized object from protected copy | object-specific / measured | clinical/privacy owner |
| Ransomware/credential compromise | IAM and vault alerts | revoke identity, lock vault, isolate VM | restore from immutable independent copy | vault policy / measured | two-person security approval |
| Regional outage | Azure regional incident | fail closed; do not claim local RPO | geo-restore and reconfigure private networking | larger / measured | incident authority |
| Lost encryption key | key custody check | do not claim object recovery | retrieve approved key version; otherwise encrypted data remains unreadable | no data exposure / measured | key custodian |

## Recovery phases

1. Declare incident and record the incident ID, operator and chosen recovery
   point.
2. Revoke or isolate the old VM identity and stop ordinary workers.
3. Inventory recovery points, journal freshness, Blob manifests and key
   versions.
4. Restore into an isolated target and start with `RECOVERY_MODE=true`.
5. Validate migrations, capability roles, FORCE RLS policies, security-definer
   functions, extensions, grants and health checks.
6. Reconcile account-erasure journal, finance/provider events, notification
   claims and appointment/consultation state. Do not replay external effects.
7. Verify synthetic patient/doctor/admin flows, protected documents and article
   media. Record actual elapsed time and latest recovered transaction.
8. Obtain explicit approval, switch DNS only after the checklist is complete,
   then restart with `RECOVERY_MODE=false`.

## Operational schedule

Continuous freshness and vault alerts; daily backup-status review; weekly
synthetic restore where infrastructure supports it; monthly application-level
restore drill; quarterly database/credential-compromise exercise; and a drill
after each migration or storage architecture change. Each drill records the
point, scope, elapsed time, result, gaps and approving operator.
