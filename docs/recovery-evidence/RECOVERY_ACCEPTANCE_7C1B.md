# Local Recovery Acceptance synthetic-7c1b-local-20261029

- Fixture: `docker-tmpfs-pg16-recovery-fixture`
- Source database: `disposable-postgresql-source`
- Target database: `disposable-postgresql-target`
- Selected recovery timestamp: `2026-10-28T15:03:35Z`
- Latest restored transaction: `2026-10-28T15:03:35Z`
- Artifact version: `hellodoctor-backend-v2.4.0`
- Migration version: `20261130000003`

## Durations (milliseconds)

- application_reconstruction: `None`
- backup_creation: `None`
- database_restore: `None`
- object_restore: `1066.28`
- reconciliation: `1407.96`
- total_validated_recovery: `None`

## Checks

- **PASSED** postgresql-physical-pitr: pre-point row restored; post-point row absent; target promoted
- **PASSED** recovery-mode-safety: health and operator status allowed; ordinary auth, clinical and external-effect routes denied
- **PASSED** erasure-journal-integrity: 7 local tests cover tampering, reordering, truncation and missing checkpoint
- **PASSED** encrypted-object-recovery: in-memory backup-copy restore, hash and key checks passed
- **NOT_EXECUTED** full-seeded-application-reconstruction: synthetic business records were not fully seeded through the application
- **NOT_EXECUTED** azurite-backup-copy: Azurite container setup was not completed
- **NOT_EXECUTED** azure-cloud-recovery: no Azure resources are authorized

## Outcome: PARTIAL
