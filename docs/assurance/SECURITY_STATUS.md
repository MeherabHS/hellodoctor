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
| Admin step-up re-authentication | HIGH, open | **Fixed (2026-11-20)** | `src/services/admin_step_up.rs`: doctor verify/reject/revoke and Admin-created doctors, peer decisions, grievance refunds and appeal decisions, refund-request transitions, disbursements and period finalisation, platform payment methods, account erasure, identity traces and revoking Admin invitations answer `REAUTH_REQUIRED` until the Admin re-enters their password (`POST /admin/reauth`). One confirmation unlocks that sign-in session for 10 minutes; wrong passwords are audited and locked out after 5 in 15 minutes. Admin Web asks for the password and retries. Test: `tests/cases/admin_step_up.rs`. |
| Payment webhook amount / currency / merchant check | HIGH, open | **Open** | `src/api/v1/webhook_handlers.rs:21` (`PaymentWebhookPayload`) is HMAC-verified, provider-bound and idempotent, but does not compare amount, currency or merchant against the transaction. Must be closed in the real provider adapter before money moves. |
| JWT key rotation (`kid`, overlap window) | MEDIUM, open | **Fixed (2026-11-20)** | `src/crypto.rs`: tokens carry `kid`; verification accepts the current key and `JWT_ED25519_PREVIOUS_PUBLIC_KEYS_B64` by `kid`. Procedure in [AUTH_SECURITY.md](../AUTH_SECURITY.md) §2. Tests: `crypto::jwt_key_rotation_tests`. |
| Machine-readable API route inventory | MEDIUM, open | **Open** | Router is explicit (`src/api/router.rs`) and every non-public handler calls `abac::authorize`, but no generated inventory or regression check guards new routes. |
| Data-retention cleanup | MEDIUM, open | **Partial** | Expired-upload and intake-removal cleanup jobs (`notification_scheduler.rs:933,1016`) and the erasure journal exist; no general retention purge. |
| Incident runbooks / alert routing | MEDIUM, open | **Partial** | [INCIDENT_RESPONSE_RUNBOOK.md](../INCIDENT_RESPONSE_RUNBOOK.md) exists; alert routing depends on the deployment environment. |
| Malware scanning of uploads | Deferred | **Deferred (decided 2026-11-20)** | Structural PDF/image sanitisation and re-encoding exist; no antivirus scanner. A ClamAV integration was built and then dropped by product-owner decision on 2026-11-20. |
| Production TLS, firewall, secrets manager | Deferred | **Deferred (deployment)** | Caddy config is in `deploy/single-vm/`; needs the real host. |

**Count now (2026-11-20):** 5 fixed, 4 partial, 2 open, 2 deferred (antivirus by decision; production TLS, firewall and secrets to deployment). No item is rated
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

## 3. External read-only test — 2026-11-20

A read-only black-box pass was run against the live hosts (`api.hellodoctor.asia`,
`admin.afyamkononi.app`, `doctor.afyamkononi.app`). No critical or high findings. Verified live:
default-deny on every protected route; forged and `alg:none` tokens rejected; the doctor-login
IP jail trips after 4 attempts; admin login requires the bot challenge; generic auth errors (no
user enumeration); strict CSP/HSTS/frame protections on the web apps; CORS returns no
`Allow-Origin`; origin IP hidden behind Cloudflare; empty error bodies (no stack traces).

Three low findings, all information disclosure, were fixed the same day:

| Finding | Fix |
|---|---|
| `Via: 1.1 Caddy` response header revealed the origin reverse proxy | `deploy/single-vm/Caddyfile` strips `Via` on all sites |
| API host lacked `Strict-Transport-Security` and `X-Content-Type-Options` | Caddyfile adds both to the API responses |
| Pre-auth endpoints leaked axum/serde deserialize errors (framework + field names) | `src/api/json.rs` returns the standard error envelope on malformed bodies |

**Not yet done:** the authenticated cross-role authorization matrix (Patient / Doctor / Admin
IDOR/BOLA) could not run from outside — admin login needs the bot challenge and OTP codes are
delivered out of band. It should run against the disposable stack, where accounts can be minted
without touching real data. The backend suite already covers the individual cases
(`test_document_download_url_enforces_ownership`, the RLS tests).

## 4. Still planned

A full external-style penetration test on the disposable stack: OWASP ASVS L2 / API Top 10,
cross-role authorization matrix, JWT and refresh-token tampering, OTP/MFA abuse, upload attacks
against the 20 × 600 KB limits, forged payment webhooks, Agora token scope, plus `cargo audit`,
`npm audit` and secret scanning.
