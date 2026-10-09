# Public exposure and session hardening - 2026-10-08

## PUBLIC INTERNET EXPOSURE REQUIREMENTS

Development must not be exposed to the public internet. Synthetic LAN HTTP testing is development only. Application guards do not install or replace TLS infrastructure: **EXTERNAL_TLS_TERMINATION_REQUIRED**.

A deployed environment requires HTTPS termination; explicit production-safe configuration; independent production secrets; no development OTP or seed fixtures; authenticated metrics; disabled simulation endpoints; fail-closed rate limiting; immediate server-enforced session invalidation; and private database/Redis/backend networks. Provider acceptance, backup/restore, monitoring and real-money readiness remain separate gates. Automated tests are security regressions, not a penetration test.

## Session policy

Access JWTs carry required `sid`, the existing refresh-token family UUID. Each protected request verifies signature, issuer, audience, expiry, subject, active account/current role, and an unrevoked, unexpired active family leaf in the authoritative `auth_sessions` database. There is no positive revocation cache and no Redis-dependent bypass. Database failure returns 503.

Logout preserves the existing current-family policy. It locks and revokes that family, including tokens rotated from the presented ancestor; unrelated devices remain valid. The next authentication check rejects every access token from the revoked family immediately. Requests already authorized before logout are not retroactively cancelled. Existing notification-device disablement remains account-wide; this is separate from access-session policy.

Legacy JWTs without `sid` are rejected; a still-valid refresh token can obtain a new session-bound JWT. Doctor Web, Admin Web and Flutter call server logout. Local credentials are cleared on failure, but the UI explicitly states that remote revocation was not confirmed. Admin and Doctor Web coordinate parallel refresh requests per refresh credential so normal parallel reads do not consume it twice.

## Configuration and ingress

| Setting | Explicit development | Staging / production |
|---|---|---|
| `ENVIRONMENT` | `development` | explicit valid `staging` / `production`; missing/unknown fails |
| `SEED_DEV_DATA` | opt-in | forbidden |
| `DEV_AUTH_OTP_ENABLED` | opt-in; random OTP returned, never logged | forbidden; no SMS provider is fabricated |
| `DEV_SIMULATION_ENABLED` | opt-in | staging opt-in; production forbidden |
| `TRUSTED_PROXY_IPS` | optional | exact physical TLS ingress/BFF socket IPs required |
| `PUBLIC_API_BASE_URL`, `ALLOWED_ORIGINS` | LAN/loopback HTTP allowed | HTTPS required |
| `METRICS_SCRAPE_TOKEN` | empty permits anonymous dev scrapes | required, 32+ non-whitespace ASCII characters |

Rust trusts `X-Forwarded-Proto` and one-IP `X-Forwarded-For` only from a configured physical socket peer. All other forwarded headers are ignored. Deployed API/metrics HTTP is rejected. Loopback `/healthz` is the sole plaintext operator-readiness exception. The trusted proxy must overwrite these headers, never append untrusted client values. Sanitized client addresses feed existing IP abuse controls.

Next.js cannot establish a physical socket peer in middleware. Both web apps therefore require a separate server-only `WEB_PROXY_SECRET` of at least 32 characters in staging/production. The approved TLS ingress must overwrite `X-HelloDoctor-Proxy`, `X-Forwarded-Proto: https`, and a single-client-IP `X-Forwarded-For`. Canonical `DOCTOR_WEB_ORIGIN` / `ADMIN_WEB_ORIGIN` must be HTTPS. Network access must be restricted to that ingress. Apps authenticate ingress before forwarding only validated protocol/client-IP metadata to Rust; the ingress credential is not forwarded or returned to the browser. This credential is different from the monitoring token. Missing/unsafe web configuration fails closed. Set `DEV_AUTH_OTP_ENABLED=false` for deployed web apps.

Flutter shipping clients require explicit `ENVIRONMENT` and HTTPS. A LAN HTTP build requires both `ENVIRONMENT=development` and `ALLOW_INSECURE_HTTP=true`. Development OTP auto-completion additionally requires `DEV_AUTH_OTP_ENABLED=true`; production rejects development OTP, LAN HTTP and development payout flags. API redirects are disabled to prevent transport downgrade or credential forwarding. Existing Android platform cleartext permission does not authorize insecure production API requests.

No TLS proxy, certificate, firewall rule, service or monitoring product was installed. The current local setup remains development; it is not an HTTPS production deployment.

## Metrics and simulation

Production `/metrics` requires `Authorization: Bearer <monitoring credential>` through approved TLS ingress. Query strings do not authenticate scrapes; normal user JWTs are not monitoring credentials. Explicit development/test without a configured scrape credential retains anonymous metrics.

Both `/admin/logs/simulate` and `/admin/system-errors/simulate` retain SecurityAdmin/PlatformAdmin ABAC and reject production even for those roles. Staging simulation requires explicit opt-in. Admin Web discovers permitted simulation controls at runtime and hides them in production; the backend remains authoritative.

## Abuse boundaries and remaining step-up work

Redis-backed counters increment and set expiry atomically. Redis failure returns `503 RATE_LIMIT_SERVICE_UNAVAILABLE` and records a System Error, while real exhaustion remains `429 RATE_LIMIT_EXCEEDED`. There is no unlimited-traffic fallback. Revocation checks also fail closed on their authoritative DB dependency.

Existing controls remain: global per-IP budget; OTP resend budget of 3 per phone/10 minutes; patient OTP single-use row locking and 3 attempts; Doctor/Admin login ceiling of 5/IP/15 minutes; account jail after 5 failures/15 minutes; distinct-account IP breadth jail after 20 accounts/15 minutes; atomic Doctor MFA challenge attempts and account jail; Doctor registration ceiling of 5/IP/15 minutes. Refresh/logout inherit the global limiter. No password-reset/MFA-reset public routes exist in the current router. Production SMS delivery is still an external dependency, so the API refuses to generate a development challenge there.

Sensitive Admin financial/security actions currently use ordinary authorized sessions. **STEP_UP_AUTH_REMAINS_OPEN**. This pass does not add a new business-wide reauthentication subsystem. Stolen credentials can still exercise permissions; authentication does not establish user intent.

**SLOT_APPROVAL_WORKFLOW = NOT_IMPLEMENTED_BY_PRODUCT_DESIGN**. Verified Doctors manage their own slots; existing authorized Admin/Support interventions remain. Auditing records actions. No slot approval or two-person workflow was added.

## Credential hygiene

Required fixed JWT/TOTP/password material in integration tests/examples and seed constants is synthetic fixture material, never production credentials. Known seeded user IDs cannot authenticate or use access JWTs outside development/test. Local backend `.env` and both web `.env.local` files are ignored local secrets. No standalone PEM/KEY/DER/JWT or expired bearer artifacts were found in the active worktree scan. Tracked-file and credential-filename history scans found no committed deployment private-key file or literal populated deployment JWT private-key assignment. This is a scoped scan, not proof of an empty Git history.

`.gitignore` and Docker contexts exclude local environment files, exported tokens and signing-key files. No keys were rotated automatically. If a real committed secret is later identified, deleting HEAD is insufficient: rotate it according to the existing key runbook.

## Regression evidence

Final counts and live checks are recorded in `docs/audit_evidence/public-exposure-security.json` after completion. Raw logs are local under `services/backend/target/public-exposure/`. Backend checks run strictly in order: fmt, clippy, library tests, full serialized API tests. Redis-outage tests use an isolated unreachable pool; they do not stop shared Redis. No competing integration suites run against the shared roles.

Doctor Flutter source changes require rebuilding/reinstalling the app; passing source tests does not update an already-installed phone binary or replace the user's earlier device acceptance.

## Completed verification results

SECURITY_REGRESSION_GREEN. The required ordered backend run passed fmt, clippy with warnings denied, 50/50 library tests and 129/129 serialized API tests. Doctor Web passed typecheck, source checks, 44/44 tests and production build. Admin Web passed typecheck, 13/13 tests and production build. Flutter passed analysis, 81/81 tests and an offline development ARM64 APK build.

Live browser/API checks verified structured direct Doctor publication, patient visibility, archive removal, and immediate access/refresh rejection after actual Doctor/Admin Web logout. A synthetic patient API journey separately passed. All nine executable unsafe-startup cases rejected before DB initialization; both built web servers rejected anonymous/spoofed production HTTP ingress. Source = binary = database migration 20261025000001. All 62 prior migration-file hashes remain unchanged.

PUBLIC_EXPOSURE_SECURITY_GATES_PARTIAL. External TLS deployment and sensitive Admin step-up remain open. No host software or infrastructure was installed. The new phone-test APK has not been installed on a device. [Machine-readable evidence](audit_evidence/public-exposure-security.json).
