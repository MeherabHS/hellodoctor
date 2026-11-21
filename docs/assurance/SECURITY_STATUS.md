# Security status — re-verified against the baseline

Baseline: commit `eef9ce7`, formerly `6f02b84` (see [BASELINE.md](BASELINE.md)). Re-verified 2026-11-17 by reading the
current source, not by carrying forward earlier verdicts.

This page replaces the open-item tables spread across the October security documents (listed in
[HISTORY.md](HISTORY.md)). Those documents remain the detailed evidence for the controls they tested;
their *open/closed* verdicts are superseded by the table below.

> **Scope note.** This is a status consolidation, not a new penetration test. No external
> black-box test (OWASP API Top 10 sweep, ZAP/Burp, authorization fuzzing) has been run against this
> baseline yet. That is the next planned step.

## 1. Items earlier reviews left open — current state

| Item | Oct 2026 verdict | Status now | Evidence in current code |
|---|---|---|---|
| Argon2 password hashing blocks request threads | HIGH, open | **Fixed** | `src/crypto.rs:46-110`: hashing and verification run in `tokio::task::spawn_blocking` behind a 4-permit semaphore, so a burst of logins cannot stall the async runtime. Only the one-shot `admin_bootstrap` CLI and dev seeding hash synchronously, where nothing else is running. |
| Real-time access-token revocation | MEDIUM, open | **Fixed** | `src/api/middleware/auth.rs:120-151`: every authenticated request checks the token's session family is still un-revoked (`auth_sessions.revoked_at IS NULL`), so revoking a session cuts off its access tokens immediately, not after the 15-minute expiry. |
| Currency separation | HIGH, open | **Fixed** | `transactions`, `disbursement_items` and related finance tables carry `currency CHAR(3) NOT NULL ... CHECK (currency = 'KES')` (finance migrations, Oct 2026). |
| Payment `UNKNOWN` state / reconciliation | HIGH, open | **Partial** | `migrations/20261130000001_remediation_a_finance.up.sql` adds `provider_request_state` (`UNKNOWN` / `NOT_STARTED` / `STARTED`) so an in-doubt provider call is recorded durably. No reconciliation worker exists, because no real payment-provider adapter exists yet. |
| Abandoned-payment expiry | MEDIUM, open | **Partial** | `appointment_repo::expire_abandoned_hold_for_slot` frees a stale unpaid hold when someone else tries to book that slot. No scheduled sweeper releases holds nobody contends for. |
| Admin step-up re-authentication | HIGH, open | **Fixed (2026-11-21)** | `src/services/admin_step_up.rs`: doctor verification, Peer Navigator decisions, grievance decisions/refunds/appeals, data erasure and data-request review, finance (period finalize, disbursements, primary payment method) and campaign approve/schedule/resume answer 403 `STEP_UP_REQUIRED` unless this session re-entered the Admin password at `POST /admin/step-up` within 2 minutes (Admin Web shows the time left) (same 5-per-15-min lockout, every attempt audited). Admin Web prompts and retries. Tests: `security_step_up`. |
| Payment webhook amount / currency / merchant check | HIGH, open | **Open** | `src/api/v1/webhook_handlers.rs:21` (`PaymentWebhookPayload`) is HMAC-verified, provider-bound and idempotent, but does not compare amount, currency or merchant against the transaction. Must be closed in the real provider adapter before money moves. |
| JWT key rotation (`kid`, overlap window) | MEDIUM, open | **Fixed (2026-11-21)** | `src/crypto.rs`: access tokens carry a `kid` derived from the signing key; `JWT_PREVIOUS_PUBLIC_KEYS_B64` keeps up to 3 retired public keys valid for the overlap window; an unknown `kid` is refused; tokens without a `kid` still verify. Procedure in `.env.example`. Tests: `jwt_key_rotation_tests`. |
| Machine-readable API route inventory | MEDIUM, open | **Open** | Router is explicit (`src/api/router.rs`) and every non-public handler calls `abac::authorize`, but no generated inventory or regression check guards new routes. |
| Data-retention cleanup | MEDIUM, open | **Fixed for security data (2026-11-21)** | Product-owner decision "security data only": `src/services/security_retention.rs` (hourly, batched) deletes expired sign-in/OTP/MFA/password-reset codes 7 days after expiry and ended sessions (plus orphaned device keys) 90 days after ending, keeping any session the audit log, a consultation trace or a reschedule references (migration `20261222000019`). Clinical, chat, financial, grievance and audit data are kept until a legal retention policy is approved. Tests: `security_retention`. |
| Incident runbooks / alert routing | MEDIUM, open | **Partial** | [INCIDENT_RESPONSE_RUNBOOK.md](../INCIDENT_RESPONSE_RUNBOOK.md) exists; alert routing depends on the deployment environment. |
| Malware scanning of uploads | Deferred | **Deferred (external)** | Structural PDF/image sanitisation exists; no ClamAV-class scanner. |
| Production TLS, firewall, secrets manager | Deferred | **Deferred (deployment)** | Caddy config is in `deploy/single-vm/`; needs the real host. |

**Count now (2026-11-21):** 6 fixed, 3 partial, 2 open, 2 deferred to external dependencies. No item is rated
critical.

## 2. Controls verified earlier and still in place

Each is backed by an automated test that still exists in the suite at this baseline:

| Control | Test |
|---|---|
| Doctor MFA brute-force lockout | `test_doctor_mfa_brute_force_is_blocked` |
| Document download ownership (IDOR) | `test_document_download_url_enforces_ownership` |
| Wallet credited exactly once under concurrent completion | `test_wallet_credited_exactly_once_under_concurrent_completion` |
| Disbursement claims each transaction at most once | `test_disbursement_claims_each_transaction_at_most_once_under_concurrency` |
| Grievance cannot be adjudicated twice | `test_grievance_cannot_be_adjudicated_twice` |
| Row-level security forced on clinical tables, five `NOLOGIN NOBYPASSRLS` capability roles | `migrations/20261003000001_rls_hardening.up.sql`; verified again by the benchmark restore script on every run |
| Admission control and load shedding (429/503 with `Retry-After`) | `src/admission.rs`; exercised by the spike test in [PERFORMANCE_REPORT.md](PERFORMANCE_REPORT.md) |

## 3. Next step

An external-style penetration test against this baseline on the disposable stack: OWASP ASVS L2 /
API Top 10, cross-role authorization matrix (Patient / Doctor / Admin), JWT and refresh-token
tampering, OTP/MFA abuse, upload attacks against the 20 × 40 KB limits, forged payment webhooks,
Agora token scope, plus `cargo audit`, `npm audit` and secret scanning.
