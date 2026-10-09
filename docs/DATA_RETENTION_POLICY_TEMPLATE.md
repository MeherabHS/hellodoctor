# Data retention policy template

Durations require product, clinical, and jurisdiction-specific legal approval.

| Data class | Duration | Disposal/archive rule | Owner |
|---|---|---|---|
| OTPs and MFA challenges | LEGAL_POLICY_REQUIRED; operationally short-lived | purge expired records | Security |
| Refresh sessions/revocations | LEGAL_POLICY_REQUIRED | retain security evidence, then purge | Security |
| Medical documents/clinical records | LEGAL_POLICY_REQUIRED | archive or delete only under approved policy/legal hold | Clinical/Privacy |
| Financial transactions/refunds/payouts | LEGAL_POLICY_REQUIRED | immutable audit retention | Finance |
| Audit events | LEGAL_POLICY_REQUIRED | append-only retention | Security/Compliance |
| System errors/telemetry | LEGAL_POLICY_REQUIRED | aggregate then purge detailed events | Engineering |

No deletion job should be enabled until the duration, legal hold behavior, and
restore implications are approved.
