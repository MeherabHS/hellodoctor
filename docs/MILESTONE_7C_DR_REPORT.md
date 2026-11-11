# Milestone 7C disaster recovery engineering report

## Scope and decision

Inspection selected Azure Database for PostgreSQL Flexible Server for the
production database, with the application VM kept stateless. The design uses
PITR/HA/geo-redundant backup plus a separately administered, immutable-capable
Azure Backup vault. PostgreSQL 16 Docker instances are retained for local
restore simulations; pgBackRest is not introduced as a second production
backup system.

## Implemented

- `RECOVERY_MODE` is a canonical backend setting. In recovery mode the
  notification scheduler does not start, FCM uses the no-op transport and the
  global guard rejects every external route except health and the
  token-protected recovery status probe. Patient/Doctor/Admin login, refresh,
  registration, WebSocket upgrades and clinical reads are blocked. Normal mode
  requires an explicit restart after approval.
- `RECOVERY_OPERATOR_TOKEN_SHA256` is required when recovery mode is enabled.
  The raw operator token is never persisted; token comparison is digest-bound
  and constant-time.
- `ops/dr/hellodoctor_dr.py` supplies read-only inventory and manifests, a dry
  restore plan, recovery prerequisite checks and a guarded disposable-target
  `pg_restore` path. It refuses ambiguous or persistent targets.
- `ops/dr/test_dr_tool.py` proves target ownership, fail-closed recovery checks
  and dry-run behavior. `ops/dr/erasure_journal.py` adds an HMAC-chained,
  pseudonymous erasure-reconciliation contract with a separately protected
  sequence anchor, key version and fail-closed rollback/truncation checks.
- `ops/dr/recovery_report.py` validates and renders durable machine-readable and
  human-readable local acceptance evidence under `docs/recovery-evidence/`.
- `services/backend/tests/recovery_blob.rs` verifies encrypted backup-copy
  restore, SHA-256 integrity, wrong-key failure and deliberate non-resurrection
  of an erased object.
- Architecture, plan, runbook and security model are documented in the four
  DR documents in `docs/`.

## Verification classification

| Gate | Result |
|---|---|
| Local guarded tooling and erasure journal | PASS (7 Python tests) |
| Emergency application posture | PASS (recovery route/auth tests; Rust library 99/99) |
| Disposable PostgreSQL logical restoration | PASS for guarded dry-run contract; `pg_restore` path remains available |
| Disposable PostgreSQL physical/WAL PITR | PASS: PostgreSQL 16 base backup + archived WAL restored into a separate container; pre-point row present and post-point row absent; roles/RLS policy/SECURITY DEFINER metadata verified |
| Encrypted Blob recovery | PASS with isolated in-memory backup-copy simulation; wrong-key and erased-object cases rejected |
| Azurite backup-copy recovery | NOT EXECUTED in this run; no independent vault semantics claimed |
| Financial/erasure/notification reconciliation | Design and existing remediation invariants preserved; provider-side reconciliation remains an operational drill |
| Azure cloud backup/restore/RBAC/immutability | NOT VERIFIED; no Azure subscription/resources are authorized |
| Measured local RPO/RTO | PARTIAL: component timings are retained in `docs/recovery-evidence/RECOVERY_ACCEPTANCE_7C1B.json`; full backup, restore, reconstruction and total-duration timings remain unmeasured |

## Required production values and decisions

The operator must later supply the Flexible Server name/network, backup
retention and redundancy, vault and Resource Guard ownership, managed identity
assignments, independent monitoring destinations, a protected recovery journal,
artifact image digests, and approved encryption-key custody. No secrets are in
the repository.

## 7C.1 executed evidence

- Docker was available for this validation. Three disposable PostgreSQL 16
  containers were used for the WAL drill, plus two disposable containers for
  the application reconstruction. They used tmpfs data directories and were
  removed by exact container name; existing `hellodoctor-postgres` and
  `hellodoctor-redis` remained healthy and untouched.
- The physical drill created a base backup, committed a pre-point row, then a
  post-point row, archived WAL and restored to `2026-10-28 15:03:35 UTC`. The
  restored target contained the pre-point row and excluded the post-point row,
  then promoted successfully. A second drill restored RLS policy metadata,
  role records and a `SECURITY DEFINER` function; role isolation returned one
  permitted row after an explicit grant and denied ordinary API-role writes.
- A disposable HelloDoctor backend ran its actual embedded migrations against
  a fresh PostgreSQL 16 container. `/healthz` returned 200 with migrations and
  Redis ready. The token-protected recovery status returned 200. Patient,
  Doctor and Admin login, refresh, logout, clinical reads, webhook and
  WebSocket-shaped requests returned 503 while `RECOVERY_MODE=true`.
- Encrypted-object recovery tests passed for backup-copy restore, SHA-256,
  AES-256-GCM wrong-key failure and deliberate non-resurrection of an erased
  object.
- Recovery is documented as three phases: RECOVERY mode, isolated synthetic
  acceptance, then an explicit restart into NORMAL mode. The runbook no longer
  suggests ordinary authentication testing while RECOVERY_MODE is active.

## Final verdict

**PARTIAL.** Recovery authentication safety, erasure-journal contract,
PostgreSQL physical/WAL recovery and encrypted-object recovery passed locally.
The full HelloDoctor application reconstruction, Azurite backup-copy drill,
independent protected journal deployment, Azure vault/RBAC/immutability and
cloud RPO/RTO remain unverified. No claim is made that Azure operational
recovery has passed.
