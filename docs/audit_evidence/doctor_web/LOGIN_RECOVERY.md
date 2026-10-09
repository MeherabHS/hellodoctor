# Doctor Web login recovery — 2026-10-07

The reported support-reference login failure reproduced as HTTP 500. Backend
logs identified PostgreSQL authentication failure for the auth-worker role.
A read-only SCRAM consistency check confirmed the five capability-role
passwords matched the fixed API-test credential instead of the running
development configuration. PostgreSQL roles are shared across the cluster;
temporary test databases do not isolate their passwords.

The API harness now reads existing development role credentials for both role
bootstrap and test connection pools. Isolated CI without a development
configuration retains its existing test credential. Canonical backend startup
restored the running configuration; after the targeted API test, all five
roles still matched development configuration and none matched the fixed
test credential. No security controls were disabled or Redis counters cleared.

The configured browser origin is `http://127.0.0.1:3001`. Equivalent localhost
navigation now redirects through the server page before any form submission.
Next middleware normalizes loopback redirects against its internal hostname,
so the redirect belongs in the page. Foreign origins still receive HTTP 403.

## Verified results

`login-ui-recovery.json` records an actual installed Edge walkthrough using
controlled development Doctor 2 credentials. No password, OTP, token, cookie
value or patient response was saved to the evidence.

- UI password login: HTTP 200.
- UI MFA: HTTP 200, Doctor shell rendered.
- Browser reload restored the authenticated session.
- Session cookies remained inaccessible to ordinary browser JavaScript.
- Foreign-origin login remained rejected with HTTP 403.
- UI logout: HTTP 200, sign-in form returned.
- Localhost redirected to the configured 127.0.0.1 origin without a loop.
- No browser JavaScript exceptions.
- Web unit tests: 10 passed; typecheck, lint, formatting checks and production build passed.
- Backend targeted treatment-context API test passed with the updated harness.
- Backend Clippy passed. Whole-backend formatting check encountered unrelated
  concurrent notification-source formatting differences; those files were
  not changed by this login repair. A new full API-suite run is not claimed.

Repeated earlier infrastructure failures temporarily locked the controlled
Doctor 1 account. Its cooldown expired normally; read-only TTL returned -2
after this verification. No account unlock or rate-limit bypass was performed.
The user's own credentials were neither requested nor tested.

This verifies login recovery, not complete Doctor Web visual acceptance.
No Doctor Flutter source was changed for this repair. No software was
installed, no Windows service created, and no new infrastructure introduced.
The existing approved preview remains loopback-only on port 3001.
