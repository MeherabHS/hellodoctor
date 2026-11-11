# Recovery security model

## Trust boundaries

- Application capability roles retain their existing RLS and ABAC boundaries.
- The VM identity may read/write application data but cannot delete or shorten
  immutable recovery points.
- Backup writers cannot administer the vault, Resource Guard or retention lock.
- Restore operators receive time-limited access to an isolated target; a second
  authorized person approves cutover and destructive changes.
- Encryption-key custodians are separate from database and Blob operators.

Normal application credentials must not delete protected backups, lower
retention, disable policies or destroy recovery keys. Azure Resource Guard,
multi-user authorization, separate subscription/security administration and
locked immutability are production controls to configure and test with the
security owner. This milestone intentionally does not create or lock them.

## Anti-resurrection controls

The account-erasure journal must be protected independently enough to survive a
database PITR. The local contract uses pseudonymous subject references, a
versioned journal HMAC and a separately protected sequence-anchor HMAC. Missing,
reordered, truncated or rolled-back journal data fails closed. Production must
place the journal and anchor in an append-only protected store with separate
custody and retention controls; the local file contract does not prove Azure
immutability. During recovery, a restored user is denied ordinary
authentication until its account and erasure disposition are reconciled. The
application's `RECOVERY_MODE` disables schedulers, all ordinary authentication,
WebSocket upgrades and all application routes except health and the
token-protected recovery status probe. It cannot be disabled through an API
request.

Recovery acceptance runs in a separately network-isolated disposable
deployment. That phase may use synthetic credentials and records for controlled
checks, while public ingress and live provider credentials remain disabled.

Financial state is reconciled against provider-side events using transaction and
idempotency identities. No collection, refund, settlement or disbursement is
automatically replayed. Notification state is reconciled using delivery claim
tokens and uncertainty markers; no entire restored outbox is resent.

## Blob/database consistency

Every protected object manifest records database identity, fixed container,
server-generated key, expected SHA-256, encryption-key version, ownership and
retention/erasure disposition. Recovery first restores metadata and verifies
references. An object uploaded before a DB commit is an orphan candidate; a DB
row with a missing object is an integrity incident. A deleted object present in a
protected backup is not automatically reattached.

## Key loss

Without the approved AES-256-GCM application key, recovery fails closed. Azure
account authentication keys and the application encryption key are different
secrets. The system must never return ciphertext as if it were recovered
clinical data or log key material.
