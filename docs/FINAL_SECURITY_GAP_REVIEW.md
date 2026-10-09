> Latest completed security/Articles pass (2026-10-08): SECURITY_REGRESSION_GREEN; 50/50 Rust library, 129/129 serialized API, 44/44 Doctor Web, 13/13 Admin Web and 81/81 Flutter tests; both web builds and Flutter analysis pass. Direct Doctor publication has no Admin review. No slot approvals. Migration source/binary/database = 20261025000001. External TLS and sensitive Admin step-up remain open: PUBLIC_EXPOSURE_SECURITY_GATES_PARTIAL. [Evidence](audit_evidence/public-exposure-security.json).

> Current public-exposure/session hardening and direct Doctor Articles (2026-10-08): [security controls and deployment requirements](PUBLIC_EXPOSURE_HARDENING.md), [Doctor Articles without Admin review](DOCTOR_ARTICLES.md). Earlier dated results below are historical snapshots. Slot approvals are explicitly excluded by product policy.

# HelloDoctor final security gap review

Review date: 2026-10-04

This review is deliberately evidence-based. It does not treat provider
credentials, a future deployment, or an unrun external sandbox as implemented.

## Phase 0 verification

| Control | Classification | Evidence |
|---|---|---|
| Doctor MFA brute-force protection | VERIFIED_COMPLETE | `test_doctor_mfa_brute_force_is_blocked`; atomic attempt counter and account jail |
| Document-download ownership/IDOR | VERIFIED_COMPLETE | `test_document_download_url_enforces_ownership` |
| PDF structural sanitization | VERIFIED_COMPLETE | four `upload_security_service` unit tests; AV remains external/deferred |
| Wallet double-credit race | VERIFIED_COMPLETE | `test_wallet_credited_exactly_once_under_concurrent_completion` |
| Disbursement double-payment race | VERIFIED_COMPLETE | `test_disbursement_claims_each_transaction_at_most_once_under_concurrency` |
| Grievance terminal state | VERIFIED_COMPLETE | `test_grievance_cannot_be_adjudicated_twice` |
| Same-transaction grievance audit | VERIFIED_COMPLETE | finance-worker audit write migration and grievance integration coverage |
| Concurrent five-document cap | VERIFIED_COMPLETE | `test_five_document_cap_enforced_under_concurrent_uploads` |
| Agora participant/time/UID controls | IMPLEMENTED_AWAITING_EXTERNAL_SANDBOX | local time-gate, ownership, deterministic collision-safe UID tests; live Agora validation remains external |
| System-error redaction | VERIFIED_COMPLETE | `system_error` unit tests and persistence test |
| Audit RLS bypass | VERIFIED_COMPLETE | audit snapshot migration and Super-Admin-only test |
| Telemetry ordering/bounds | VERIFIED_COMPLETE | telemetry timeline/reconnect tests and bounded event ingestion |
| Scoped admin list queries | VERIFIED_COMPLETE | production-scale telemetry query fix and dashboard regression test |
| Argon2 async isolation | STILL_OPEN | password hashing remains synchronous in `crypto.rs` |
| Production log default | VERIFIED_COMPLETE | quiet fallback in `telemetry/mod.rs`; deployment must still set production filter explicitly |
| M-Pesa finance bucket | VERIFIED_COMPLETE | explicit M-Pesa match arm and finance coverage |

## Focused loophole review

| Area | Classification | Finding |
|---|---|---|
| Webhook provider-to-transaction binding | FIXED_AND_TESTED_BY_CODE_REVIEW | callback provider is now checked against the transaction gateway before recording an event |
| Webhook amount/currency/merchant validation | STILL_OPEN | generic stub payload has no provider-specific amount/currency/merchant contract |
| Payment UNKNOWN/reconciliation worker | STILL_OPEN | no provider adapter or durable UNKNOWN state exists yet |
| Abandoned-payment expiry worker | STILL_OPEN | no production scheduler/conditional expiry transition is present |
| Provider refund execution | DEFERRED_EXTERNAL_DEPENDENCY | internal grievance state exists; provider refund API/spec is not configured |
| Provider payout execution | DEFERRED_EXTERNAL_DEPENDENCY | internal claim-once batches exist; external payout acknowledgement is not configured |
| Currency separation | STILL_OPEN | transactions/wallets/batches have no explicit currency column |
| Admin step-up reauthentication | STILL_OPEN | explicitly deferred; sensitive admin actions rely on the normal session |
| Refresh-token logout revocation | FIXED_AND_TESTED_BY_CODE_REVIEW | new `/api/v1/auth/logout` revokes the presented refresh-token family; access JWTs retain their documented short TTL |
| Real-time access-token revocation | STILL_OPEN | no JWT session-version lookup; compromise response relies on short expiry and refresh-family revocation |
| Malware scanning | DEFERRED_EXTERNAL_DEPENDENCY | structural PDF checks exist; no ClamAV/equivalent service is installed |
| Production deployment hardening | DEFERRED_EXTERNAL_DEPENDENCY | TLS/reverse proxy/firewall/secrets manager/backups require deployment environment |
| Backup/restore drill | DEFERRED_EXTERNAL_DEPENDENCY | no real off-host backup target exists in this repository |
| Incident runbooks/alert routing | STILL_OPEN | metrics/error persistence exists; operational alert thresholds/runbooks are not complete |
| API route inventory | STILL_OPEN | router is explicit, but no generated machine-readable inventory/regression check exists |
| Key rotation | STILL_OPEN | static key configuration exists; no JWT `kid`/overlap rotation runbook is implemented |
| Data retention cleanup | STILL_OPEN | retention policy and cleanup jobs are not implemented |
| SSRF | VERIFIED_NOT_PRESENT_FOR_CURRENT_FEATURES | no client-controlled outbound URL fetch path was found |
| Raw card-data storage | VERIFIED_NOT_PRESENT | no PAN/CVV fields or card-data processing path was found |

## Current counts

These counts intentionally include external/deployment work rather than hiding
it behind a generic “ready” label:

- CRITICAL_OPEN: 0
- HIGH_OPEN: 4 (UNKNOWN payment recovery, currency integrity, step-up auth, Argon2 isolation)
- MEDIUM_OPEN: 7 (expiry worker, access-token revocation, API inventory, key rotation, retention, incident operations, AV pipeline)
- LOW_OPEN: 0

The repository is not declaring real-money production readiness. The internal
ledger/concurrency foundation is strong; the remaining high items are control
loops and deployment/external-provider dependencies.
