# Browser authentication abuse controls

This document covers the Admin Web and Doctor Web changes only. Source controls are implemented; runtime, provider and deployment verification are separate evidence requirements. No live provider API, real account or production system was exercised.

## Before implementation

Both applications already forwarded credentials through a BFF. Access and refresh tokens were stored in host-only HttpOnly cookies with SameSite=Strict, Path=/api and Secure when NODE_ENV=production. Existing origin checks protect state-changing BFF routes. Both applications already supplied CSP nonces and trusted-ingress configuration checks. Those controls remain in place.

Admin password entry led to emailed OTP. Authenticator MFA is now additionally supported by the BFF and UI using the backend's `requires_totp` and `totp_code` fields. Production/staging backend policy is authoritative; an unenrolled Admin must complete the separately managed authenticator enrollment before deployment.

## Invariants

- Deployed Admin password requests and Doctor password/registration requests must pass server-side Siteverify before upstream credential hashing, account lookup or email delivery is requested by the BFF.
- Siteverify must return boolean success, the exact configured origin hostname, the expected action, and a challenge timestamp within five minutes (at most 30 seconds ahead for clock skew). Missing, oversized, expired, replayed, forged and wrong-context proofs are rejected. Validation success is never cached as a reusable authorization grant.
- Siteverify traffic has a four-second timeout, redirects disabled, no response caching and eight concurrent operations per BFF process. Overflow fails with 503 and Retry-After=5; invalid proofs fail with 403. These process bounds supplement backend/global budgets and edge limits.
- Admin login is forwarded with a fresh HMAC assertion after challenge verification. The Rust API must verify the assertion and reject replays before credential work. The signature binds exact body bytes, method, path, timestamp and nonce. Calling the backend directly must not bypass the Admin challenge gate.
- Sign-in forms are limited to 16 KiB by streamed byte count as well as Content-Length, with a five-second total body-read deadline and sixteen concurrent body readers per BFF process; additional readers fail fast. Doctor authenticated clinical upload limits remain separately enforced.
- Sequential Siteverify requests also have a process-wide token bucket (12/second refill, burst 60), in addition to the eight-request concurrency cap. These are process controls; backend Redis budgets and edge policy remain the shared service boundary across replicas.
- An attempt consumes its browser proof even when credentials fail or the response is lost. UI resets the widget after every attempt. Tokens, OTPs and challenge tokens are never logged by the new code.
- Existing access and refresh credentials remain unavailable to browser JavaScript. Button disabling is a UX measure; Siteverify and the backend assertion are security controls.

## Configuration

Set these independently for each deployed BFF using server environment/secrets management:

| Variable | Value and exposure |
| --- | --- |
| ENVIRONMENT | production or staging; explicit development/test bypasses only the browser challenge |
| ADMIN_WEB_ORIGIN | Exact HTTPS Admin origin, no trailing slash or path |
| DOCTOR_WEB_ORIGIN | Exact HTTPS Doctor origin, no trailing slash or path |
| TURNSTILE_SITE_KEY | Public widget key for that surface; supplied at runtime through GET /api/bot-challenge |
| TURNSTILE_SECRET_KEY | Matching server-only secret; never prefix with NEXT_PUBLIC_ |
| WEB_AUTH_ASSERTION_SECRET | Admin BFF and Rust API shared independent secret, at least 32 characters with no whitespace; server-only |
| WEB_PROXY_SECRET | Existing independent ingress secret; do not reuse the assertion secret |

Expected actions are `admin_login`, `doctor_login`, and `doctor_register`. Siteverify checks the hostname against the corresponding configured origin, not a caller-controlled Host header. Configure exact allowed widget hostnames with Cloudflare. Official test secrets are rejected in staging/production. Development/test uses the explicit local bypass; provider test credentials may be used only in a separately isolated provider integration validation. There is no production fail-open flag.

Admin assertion format is `v1.{unix_seconds}.{nonce_hex_32}.{mac_hex}` in `x-hellodoctor-web-proof`. HMAC-SHA256 input is the UTF-8 string:

```text
v1\n{unix_seconds}\n{nonce}\nPOST\n/api/v1/auth/admin/login\n{sha256_exact_body_hex}
```

The backend must check no more than 30 seconds of clock skew and atomically reserve each nonce with Redis NX EX90. Keep both servers' clocks synchronized. Rotate the shared assertion secret together; never return it through the public challenge-config endpoint. Only Admin password entry is asserted because later MFA verification is bound to the short-lived password challenge and backend OTP/TOTP limits.

CSP now permits only the necessary Cloudflare challenges host for script and child-frame loading while retaining nonce requirements for first-party bootstrap scripts, frame-ancestors=none and object-src=none. Login pages display a generic verification failure and a retry action. Missing provider configuration or provider outage prevents new deployed sign-ins; existing authenticated sessions do not call Turnstile.

## Verification sources and remaining evidence

Each web app has eleven `tests/turnstile.test.ts` cases covering missing configuration, malformed proof, success/context checks, expiry/replay responses, provider failures, outbound request policy, bounded concurrency, production test-secret rejection, streamed input limits, body-reader saturation and sequential provider-call budgets. Admin additionally has three `tests/web-assertion.test.ts` cases covering exact-body signature binding, configuration denial and nonce uniqueness. Provider responses in these unit cases are synthetic; they are not evidence of actual provider replay behavior.

Tests and typechecks were not executed by this implementation agent because the required OS-enforced security sandbox was unavailable. No dependencies or host tools were installed. Parent-run validation must record any subsequent execution separately. `git diff --check` passed for these app changes.

Safe isolated validation still required: run both apps' typechecks and test suites with no external network and bounded resources; validate challenged login/registration, provider timeout and retry UI in a local browser; test actual official provider test credentials only in an explicitly authorized integration environment; verify CSP in that browser; verify Admin direct-backend denial, signed request acceptance and nonce replay denial against synthetic local backend state; verify deployed cookie flags, CSRF and enrolled TOTP completion.

Doctor native applications share the Doctor backend credential routes. Browser Turnstile cannot be imposed on all native requests. Direct native-style calls remain governed by backend per-account/IP/global budgets and bounded hashing/email work. Neither User-Agent nor X-Client-Type is trusted as proof of browser challenge completion. This work does not attest native apps or bind mobile sessions to a device key.

Production ingress, WAF/rate limits and direct-origin restrictions remain deployment dependencies. This implementation does not alter host firewalls and does not establish protection against arbitrary volumetric DDoS.

Provider contract references: [server validation](https://developers.cloudflare.com/turnstile/get-started/server-side-validation/), [widget rendering](https://developers.cloudflare.com/turnstile/get-started/client-side-rendering/), [CSP](https://developers.cloudflare.com/turnstile/reference/content-security-policy/). Source inspection used the locally installed Next 16 route-handler and Script component documentation before editing Admin Web.
