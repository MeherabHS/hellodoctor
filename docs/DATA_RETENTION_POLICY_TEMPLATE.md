# Data retention policy template

Durations require product, clinical, and jurisdiction-specific legal approval.

| Data class | Duration | Disposal/archive rule | Owner |
|---|---|---|---|
| OTPs and MFA challenges (incl. password-reset codes) | **7 days after expiry** (product owner, 2026-11-21) | purged hourly by `security_retention` | Security |
| Refresh sessions/revocations | **90 days after the session ended** (product owner, 2026-11-21) | purged hourly; sessions referenced by audit/trace/reschedule records are kept | Security |
| Medical documents/clinical records | LEGAL_POLICY_REQUIRED | archive or delete only under approved policy/legal hold | Clinical/Privacy |
| Financial transactions/refunds/payouts | LEGAL_POLICY_REQUIRED | immutable audit retention | Finance |
| Audit events | LEGAL_POLICY_REQUIRED | append-only retention | Security/Compliance |
| System errors/telemetry | LEGAL_POLICY_REQUIRED | aggregate then purge detailed events | Engineering |

No deletion job should be enabled until the duration, legal hold behavior, and
restore implications are approved. Only the two security rows above are
approved and purged; every other class is kept until its duration is approved.
