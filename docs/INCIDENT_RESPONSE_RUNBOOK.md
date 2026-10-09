# Incident response runbook

## Payment provider outage

1. Stop treating uncertain callbacks as success.
2. Keep transactions in an explicit unknown/reconciliation queue.
3. Reconcile provider status before retrying a payment, refund, or payout.
4. Prevent duplicate effects with transaction/provider-reference idempotency.
5. Record the incident trace and notify finance/on-call staff.

## Authentication incident

Revoke the affected refresh-token families, rotate compromised signing secrets,
step up or disable affected admin accounts, and verify audit history.

## PHI/document incident

Contain the account/object, disable affected download paths, preserve audit and
system-error evidence, and escalate jurisdiction-specific notification duties
to legal/privacy staff.

## Infrastructure incident

Protect database integrity first, fail closed for financial mutations, verify
Redis/PostgreSQL health, restore from a tested backup only under the documented
RPO/RTO decision, and run post-restore integrity checks.
