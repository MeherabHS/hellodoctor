# Remediation A: historical records and rollout

The fixes govern future transitions. They do not authorize automatic repairs of existing financial or erasure records. No historical financial-state backfill is included.

## Read-only local inventory

Before changing application code, the persistent local `hellodoctor-postgres` development database was inspected inside a READ ONLY transaction with a five-second statement limit. Counts were: expired/cancelled appointments with INITIATED transactions: **0**; all INITIATED transactions: **0**; completed refund requests whose transaction/grievance terminal states disagree: **0**.

The owned `hd-6d-postgres` fixture's base database did not provide the application-schema inventory; SQLx creates and migrates separate disposable databases for each test. No inference about staging, production, Azure or other databases follows from the local zero counts. Aggregated evidence: `scratch/remediation-a/historical-inventory.json`. No row identifiers, credentials or clinical content were exported.

## Non-destructive inventory for an authorized environment

Run only against an explicitly authorized environment. These queries report counts and do not repair records.

```sql
BEGIN READ ONLY;
SET LOCAL statement_timeout = '5s';
SELECT count(*) AS expired_or_cancelled_initiated
FROM transactions t JOIN appointments a ON a.id=t.appointment_id
WHERE a.status IN ('EXPIRED','CANCELLED') AND t.payment_status='INITIATED';

SELECT count(*) AS completed_refund_state_mismatches
FROM financial_refund_requests f
JOIN transactions t ON t.id=f.transaction_id
JOIN grievance_reports g ON g.id=f.grievance_id
WHERE f.status='REFUNDED'
  AND (t.payment_status<>'REFUNDED_TO_PATIENT' OR g.status<>'REFUNDED');
COMMIT;
```

After the new migrations, inspect `transactions.reconciliation_required` and pending `account_erasure_jobs` through an authorized backend/DB operator. Journal keys and user identifiers belong in protected operational tooling, not ordinary logs or chat.

## Separate reconciliation review

1. Preserve a read-only evidence record and obtain review for each proposed repair.
2. Resolve provider truth using approved provider records. An absent local gateway reference does not prove no external request was sent.
3. Classify historical `provider_request_state='UNKNOWN'` conservatively. The migration adds UNKNOWN as its default; it never guesses that old attempts were unsubmitted.
4. Review completed refunds against provider reference, request, transaction, grievance, wallet reversal, accounting allocation and audit history together. Do not repeat wallet debits or fabricate provider success.
5. Reconcile late captured payments explicitly. They remain PAYMENT_HELD with `reconciliation_required=true`; closed statements are not rewritten and expired bookings are not resurrected.
6. Review old erasure success audits against linked-account/session state and personal-media ownership. A missing hard-deleted profile can require additional protected evidence to identify its old login. The new journal does not invent that historical mapping.
7. Prepare a bounded, idempotent repair with before/after evidence, rollback handling and isolated tests. Apply only after separate approval. No repair script or historical UPDATE is executed by this task.

## Deployment and rollback limitations

- Apply the two new forward migrations before starting the new backend binary. Existing migration files/checksums are preserved.
- Drain old application writers, especially erasure actions, during rollout. Old binaries do not create the erasure journal or new checkout provenance; such writes cannot be assumed safe merely because the schema is additive.
- Deploy the matching Admin Web bundle with the backend and require operators to reload it before erasure actions. Older cached Admin code does not understand the new PENDING receipt and must not be used to infer completion.
- A future payment provider adapter must persist STARTED **before** an external request, including requests with uncertain outcomes. Current payment initiation remains explicitly unavailable; no provider integration is added here.
- Rollback of the erasure migration refuses while jobs remain incomplete. Financial rollback refuses while reconciliation flags remain set. Do not clear those protections merely to force rollback.
- An erasure returns HTTP202/PENDING until required phases finish. Repeating the same authorized erase action returns the same journal ID/outcome; the existing lifecycle scheduler retries pending work. Completion retains clinical/financial records according to existing FK restrictions.
- Keep the existing lifecycle scheduler running and monitor incomplete jobs/retry counts. Disabled workers or unavailable storage leave truthful pending jobs; repeating an authorized erase action can process a due job. Device registrations retain ownership history with `enabled=false` after erasure.
- Avatar deletion confirms the existing provider delete API result. It does not certify physical deletion from provider versions, legal holds or backups; those policies require infrastructure/retention review. No Azure connectivity is claimed.
- Retained appointments, clinical records, financial obligations and professional-verification history are not automatically cancelled, settled or refunded by erasure. Any continuity-of-care or financial disposition must use its authorized workflow; erasure must not fabricate those effects.
