# Authentication abuse and session theft model

## Inspection baseline (before this milestone's implementation)

This model records the current working tree, including existing uncommitted remediation, rather than assuming the supplied architecture description matches the code.

| Boundary | Current authority and invariant | Gap identified before editing |
| --- | --- | --- |
| Access JWT | `crypto.rs`: Ed25519, issuer/audience, `sub`, `user_id`, role, permissions, `sid`, `iat`, 15 minute `exp` | Bearer credentials; no sender constraint |
| Protected request | `api/middleware/auth.rs`: query active unrotated refresh-family leaf and active user on every request; current DB role must match JWT | Each accepted request needs an auth DB lookup; origin admission must precede this middleware |
| Refresh | SHA-256 token hash storage, opaque random credential, family advisory transaction lock, reread after lock, atomic consume + successor insert | No device-key binding; stolen refresh alone remains a bearer credential |
| Refresh reuse | Consumed rows retained; reuse under family lock revokes entire family, transaction committed before rejection | Already implemented; must preserve |
| Logout | Presented current or ancestral refresh identifies family; same lock + family revocation + security audit | Already implemented; must preserve |
| Patient password step | Email/password, generic bad credential errors, real/dummy Argon2, email OTP via Azure | No global or proactive account-attempt budget; account failure jail alone allows concurrent guesses before failures land |
| Patient OTP | Challenge row lock, attempt limit, expiry, single-use consumption | No global email cost budget; verification needs early shared admission |
| Doctor password step | Email/password, role and profile checks, real/dummy Argon2 | Second factor still a development SMS stub: production intentionally refuses delivery; must use approved email delivery |
| Admin password step | Explicit ADMIN role + password + emailed OTP; row-locked verification and account/IP failure tracking | Current repository deliberately replaced TOTP with email OTP. Supplied requirement for TOTP is not currently satisfied |
| Abuse store | Redis fixed-window Lua counters and account/IP jail, fail closed | Per-IP and failure tracking do not bound distributed many-account work |
| Argon2 | Shared four-slot semaphore, blocking worker | `acquire().await` permits unbounded waiters; dummy hash first initialization runs on the async thread |
| Mobile | Flutter secure token storage and bearer API calls | No Android Keystore proof key, request proof, refresh proof, or Play Integrity verification |
| Browser | Existing BFF cookie sessions; separate web audit needed | No server verified Turnstile in current auth handlers |

## Required ordering and acceptance policy

Edge controls and origin-wide admission must run before auth DB lookup. Backend authentication additionally needs a shared process concurrency gate before Redis, a service-wide request budget before account-specific counters, proactive normalized account attempt limits, existing IP limits, and bounded password workers. Successful passwords do not reset proactive target budgets. OTP issuance needs a global budget plus shared normalized recipient cooldown/rolling limits before persisting a challenge or calling the provider. Dependency failures fail closed.

Account target keys must be hashed rather than recording email credentials in Redis key names or logs. IP is an abuse signal, not session identity. Client headers never establish a cryptographic device identity.

## Session theft target and residual risk

Device binding must be end-to-end: native nonexportable installation key, backend binding during authenticated session establishment, access/refresh proof verification, method/URI/token binding, freshness and atomic proof replay rejection. An optional header or server-only proof verifier would leave bearer sessions reusable and cannot satisfy the target. Until complete native/client/backend integration and adversarial verification exist, stolen bearer access/refresh credentials alone remain reusable subject to active-session checks, expiry, rotation and revocation. Do not report sender constraint as implemented based on a schema field or unverified client claim.

Android app attestation and edge production configuration require actual package/signing/deployment policy. Neither substitutes for authentication. No changes to host firewall or deployed infrastructure are authorized by this document.

Implementation details and evidence are appended after source changes. Dynamic tests require the isolated execution environment described by the security skill. Source inspection alone is not a passing adversarial test.

## Implemented changes

The outer `admission` layer now owns process/service/route pressure before auth work. In addition, `auth_service::abuse` reserves five attempts per normalized account and realm per 15 minutes before account lookup, regardless of IP and independent of the post-failure jail. It deliberately does not reset on password success. Patient registration also uses this control. Existing per-IP controls remain. Authenticated requests reserve a shared account plus cost-class budget after JWT verification and before active-session SQL: normal 120/min, consultation 240/min, upload 20/min, payment 30/min, expensive 60/min. These are initial policy values, not measured deployment capacity.

Every asynchronous password path uses the same four-permit **try-acquire** bulkhead. There are zero semaphore waiters. Once admitted, the blocking closure owns the permit until actual Argon2 completion, even if the HTTP request is cancelled. Cold dummy-hash initialization moved into that worker. Password parameters are unchanged. `argon2_active_workers` and `argon2_operations_total` expose actual admitted worker activity. Saturation is a controlled 503; it is not counted as a wrong password by account/IP jail logic.

Patient, Doctor and Admin email delivery reserve a shared 30/min global budget, then recipient cooldown 1/min, rolling 3/15min and 20/day before challenge creation/provider dispatch. Recipient keys use the SHA-256 normalized address. A failed provider call still consumes budget. Doctor delivery now uses the existing Azure email sender rather than the former production-disabled SMS stub. The legacy phone OTP endpoint remains development only; no SMS service was introduced. Device attestation and device-specific OTP budgets remain unimplemented: an untrusted arbitrary fingerprint would not provide an authoritative device control.

### Native proof contract

`auth_service::device_proof` validates ES256 compact JWS with `typ=hd-device+jwt` and a public P-256 JWK. The RFC 7638 public-key thumbprint is stored by refresh family in `auth_session_device_keys`. Updated Patient and Doctor Android apps generate and sign using a nonexportable Android Keystore installation key. Hardware backing depends on the device; no attestation claim is made. The private key never crosses the Flutter method channel. Different sessions may share the installation key, and each family remains independently revocable.

The signed payload contains uppercase `htm`, original external request path `htu` (including `/api/v1`, excluding query), Unix seconds `iat`, UUID `jti`, and base64url SHA-256 `binding`. Binding is the exact access token for protected requests, refresh token for refresh/logout, or verification challenge token during OTP completion. The legacy development phone verifier binds its phone challenge identity. All handlers use Axum `OriginalUri` so nested routing cannot strip the signed prefix. This is a project-specific DPoP-inspired protocol, **not an RFC 9449 interoperability claim**.

The server requires matching key, method, path, binding, timestamp within 60 seconds in the past/5 seconds in the future, valid ES256 signature, and atomic Redis `SET NX EX 120` replay reservation. Replay-store acquisition and command share a 250ms deadline and fail closed. A bound family's access, refresh and logout operations require proof regardless of any client-type header. Refresh proof is checked before reuse revocation, so a stolen bound refresh cannot trigger family revocation by itself. Family rotation preserves binding. Logout cannot resurrect a revoked family. Active session checks remain authoritative on every protected request.

At OTP completion a proof is verified before consuming the challenge; session insertion and key binding then commit atomically before credentials are returned. Updated Android callers request strict proof establishment with `X-Device-Proof-Required: 1`, but no client header is treated as cryptographic identity. The response includes `device_bound=true`; the updated Android clients reject a successful establishment response lacking it. Native clients regenerate proof on retries and refresh, reject signing failure, and require new login for previously stored bearer sessions. Losing/replacing the installation key requires normal authentication to establish a new family. Existing BFF clients and non-Android clients retain the explicit bearer contract; a caller cannot remove an existing bound family's key by changing its headers. Android native signing has one active worker and at most eight queued tasks; additional signing calls fail fast. OTP consumption precedes session issuance as in the existing flow, so an issuance failure requires a new login challenge but creates no unbound session row.

### Admin MFA and enrollment

Production/staging Admin login now requires password, emailed OTP **and** six-digit RFC 6238 HMAC-SHA256 authenticator TOTP. It uses the existing encrypted MFA table and `last_totp_step`, with a row lock and a monotonically consumed time step, 30-second period, plus/minus one-step clock window. Successful TOTP consumption and OTP deletion commit together. Failed TOTP increments the same pending challenge attempts and account/IP failure controls. The only bypass is explicit `ENVIRONMENT=development` with `DEV_AUTH_OTP_ENABLED=true`; merely setting `test` does not bypass MFA.

Migration `20261110000004_admin_totp_algorithm` labels all existing secrets `SHA1`; the new verifier rejects those rather than silently interpreting an old enrollment with another algorithm. **Unenrolled deployed Admins fail closed. Provision a SHA256 authenticator before enabling this release in production.** The owner must perform the following offline with tightly controlled database access; never send the enrollment secret in chat, telemetry, shell command arguments or issue trackers:

1. Generate a fresh random 32-byte secret using an approved local secret tool and encode uppercase Base32. Enroll it in an authenticator supporting `otpauth://totp/HelloDoctor:ADMIN?secret=<BASE32>&issuer=HelloDoctor&algorithm=SHA256&digits=6&period=30`. Transfer the provisioning URI through an approved private enrollment channel.
2. Use `encrypt_admin_totp` with the deployment's existing `TOTP_ENCRYPTION_KEY_B64` injected through its approved secret mechanism; supply the Base32 value on standard input through a private pipe. The helper emits ciphertext hex only. Do not execute this with real secrets in tests.
3. In one owner transaction, identify the intended active ADMIN by UUID, revoke prior active TOTP credentials, then insert `mfa_credentials(user_id,method,totp_secret_encrypted,is_enabled,enabled_at,totp_algorithm)` with that UUID, `TOTP`, `decode(<CIPHERTEXT_HEX>,'hex')`, `true`, `now()`, `SHA256`. Preserve owner audit evidence without recording the secret. Test an actual code and replay rejection using an isolated synthetic Admin before production enrollment.
4. Lost authenticator recovery repeats the owner-controlled enrollment and revokes existing session families. There is no public enrollment shortcut or email-only production fallback.

The Admin BFF separately verifies Turnstile and signs its exact credential request with a server-only short-lived single-use assertion checked by Rust. The challenge is consumed once at the BFF; Rust verifies the assertion instead of attempting a second Siteverify. See the distributed abuse document for secrets and origin deployment policy. Native Doctor authentication remains available independently of browser challenges.

## Verification status and deployment dependencies

Added Rust regression cases cover RFC 6238 SHA256 published vectors, all three password-path fail-fast decisions when the four permits are held, and cryptographic proof rejection for missing proof, different key, wrong method/path/token, stale/future proof. Execution results belong in `ADVERSARIAL_RESILIENCE_RESULTS.md`; authored tests do not count as passed tests. Android Keystore end-to-end signing, Redis proof replay races, 30,000-client load, real Azure dispatch, actual Turnstile availability and production origin restriction require the corresponding isolated or owner-controlled environment.

Apply both new auth migrations before API startup. Restrict origin and BFF upstream access, provision the Admin authenticator and BFF assertion secret, configure real Azure email delivery and Turnstile keys/hostnames/actions, then validate on deployment hardware. App attestation (Patient and Doctor Play Integrity) is **NOT IMPLEMENTED**; expected signing identity and enforcement policy are still required. User-visible session management remains staged. No hard IP/session binding, host firewall change, token-in-URL mechanism, new third-party SMS dependency or weaker Argon2 parameters was introduced.

`TOKEN_HIJACKING_IMPOSSIBLE=NO`. For newly bound Android sessions, source enforces `STOLEN_BEARER_TOKEN_ALONE_REUSABLE=NO` and `STOLEN_REFRESH_TOKEN_ALONE_REUSABLE=NO`; dynamic verification remains required. Across all sessions including BFF/legacy/non-Android bearer sessions those answers remain `YES`. Fully compromised devices, active XSS and theft of an entire browser cookie jar remain residual risks. The session key is an installation capability, not attestation that the device or account is uncompromised.
