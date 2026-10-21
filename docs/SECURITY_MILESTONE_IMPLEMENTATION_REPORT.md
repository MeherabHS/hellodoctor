# HelloDoctor economic, resilience and authentication implementation report

19 October 2026, Asia/Dhaka. **Implementation delivered; full security acceptance
remains INCONCLUSIVE. This is not a launch-ready or DDoS-immunity certification.**

## What changed

### Economic identity is anchored to the appointment

The implementation first mapped current booking, transaction, callback,
consultation, wallet, refund, statement and payout behavior. The important existing
gap was that request-key replay protection did not itself prevent multiple active
transaction rows for the same appointment. Another gap allowed distinct success
callbacks to use stale transaction state and repeat confirmation side effects.

The new database invariant permits historical FAILED attempts but allows only one
non-FAILED transaction for an appointment. Paid, settled, disbursed and refunded
obligations keep occupying that business identity; a new request key cannot open
another active obligation. Deterministic reads select the current attempt while
history endpoints retain old failed attempts.

Callbacks now lock the appointment/payment, re-read authoritative state, validate
provider/event ownership and immutable references, and conditionally confirm once.
Late callbacks cannot regress settled/refunded history or activate an expired
booking. Missing activation work can be recovered on a later valid callback.
Late captured funds remain a reconciliation problem rather than a fabricated
refund or consultation.

The prospective `financial_economic_effects` journal has a stable
`(effect_type, business_id)` primary key. Database triggers record confirmation,
settlement, refund approval and payout completion in the same transaction as the
existing state/money changes. Application roles cannot write or edit this journal.
Amounts/provider identity are immutable, and payout provider references are unique.

Doctor completion returns an authoritative ALREADY_COMPLETED result before
repeating notifications, trace or audit. Existing Admin approval/payout conditional
updates retain the winning Admin identity. Repeated disbursement preparation now
conflicts before creating empty batches. Payment initiation has its own account
budget. Existing Admin financial-button busy/disabled behavior remains a UX aid;
the backend/database is the security boundary.

There is still **no live collection checkout, payout/refund driver or provider
reconciler**. Payment initiation continues to report unavailable rather than invent
provider success. Successful same-key/different-key checkout recovery cannot be
certified without that integration.

### Distributed origin work is admitted before dependencies

A shared per-application admission controller runs before Redis, active-session
SQL, body decoding and handlers. It uses try-acquired permits and local aggregate
token budgets, followed by shared Redis service/route windows. IP rotation does not
create new global capacity. Normal, auth, upload, payment and expensive routes have
separate caps; consultation, callback and health paths have independent admission
reservations. Some capacity remains for ordinary work when expensive lanes fill.

For the existing default ten-connection role pools, ordinary admitted work is
capped at 20 requests, with eight additional reserved slots across consultation,
callback and health/metrics lanes. This is a configured bound, not measured safe
deployment throughput. Smaller-pool auth admission leaves one connection available
when possible; one-connection deployments cannot provide that isolation.

Pool availability/closed state supplies an early pressure signal. Common authed
transaction acquisition and readiness checks have 250ms deadlines. Other direct
pool callers remain bounded by admission but retain the existing pool deadline.
Redis checks have short deadlines. Excess work returns controlled 429/503 with
Retry-After. Dependency outage incidents are coalesced instead of producing a
database error row for each rejected request. Unmatched metric labels no longer
grow with arbitrary attacker URL paths.

Uploads retain the two-per-account concurrency limit. Sanitization now rejects
size/MIME/magic errors before dispatch and uses four no-wait worker permits, held
until actual blocking work ends. New fixed-label admission, pool, hashing and
sanitizer counters support subsequent independent load measurement.

No database pool was enlarged. No host firewall, Azure network or edge subscription
configuration was changed. No new queue broker, service split or orchestration
platform was introduced.

### Authentication work and email cost are bounded

Patient/Doctor/Admin password paths have a proactive normalized account budget
before lookup, independent of source IP and the existing post-failure jail.
Authenticated API calls also have account plus cost-class limits before session
SQL. Argon2 uses four try-acquired blocking workers; cancellation does not release a
permit while work is still running. Cold dummy-hash initialization now stays inside
the same bound. Existing actual Argon2 defaults are unchanged: 19,456 KiB memory,
two iterations and one lane; older design documents describing other parameters
are not evidence of a parameter change in this milestone.

Email OTP delivery shares a 30/min global budget across roles, a recipient
cooldown, rolling and daily limits. Failed delivery still consumes budget. Doctor
OTP now uses the existing Azure email path instead of the prior development SMS
stub. No SMS/Twilio/Africa's Talking integration was added.

Deployed Admin login requires password, email OTP and enrolled six-digit HMAC-SHA256
TOTP. Credential/time-step row locking prevents replay. Existing SHA1 enrollments
are explicitly labelled and rejected rather than silently reinterpreted. An offline
encryption helper and owner enrollment/recovery procedure are supplied. **Unenrolled
deployed Admins cannot sign in; provision and test enrollment before rollout.**

### Web challenges are verified on the server

Both web portals have Turnstile UI plus server Siteverify validation with exact
hostname/action/freshness checks, fail-closed provider behavior, bounded response
time/concurrency and sequential provider-call budgets. Sign-in body readers are
size/time/concurrency bounded. Widget tokens reset after credential attempts.

Admin's Rust login endpoint also requires a fresh, exact-body HMAC assertion from
the trusted BFF, with timestamp and atomic nonce replay rejection. Calling the API
directly cannot skip the Admin web challenge in deployed mode. Backend and BFF
must share a new independent server-only `WEB_AUTH_ASSERTION_SECRET`.

Doctor mobile and web share backend credential routes. The browser challenge
protects the BFF; native-style direct requests still require backend credentials
and face account/IP/global/worker budgets. An arbitrary client-type header is not
proof of app integrity or challenge completion.

Existing BFF HttpOnly, production Secure, SameSite=Strict cookies and origin checks
remain. Authentication cookies use the existing `/api` path architecture; no
`__Host-` cookie claim is made. No inspected browser auth path writes raw access or
refresh credentials to JavaScript storage. Active XSS and stolen cookie jars remain
risks despite HttpOnly.

### New Android sessions are sender constrained

Both Android apps now generate a nonexportable P-256 installation key in Android
Keystore and sign method/path/time/UUID/token-bound proofs through a native method
channel. The signing executor has one active task and eight queued tasks maximum.
The key never enters Flutter token storage or the server. Hardware backing depends
on the device and is not attested by this implementation.

The backend validates ES256 proofs, binds their public-key thumbprint to the refresh
family in the same transaction as session insertion, and requires that key for
access, refresh and logout. Proofs have a short freshness window and atomic Redis
replay rejection. Refresh proof precedes reuse detection so a copied refresh token
alone cannot revoke the legitimate bound family. OriginalUri handling ensures the
signed external `/api/v1` path survives Axum nesting.

Existing short access lifetime, active `sid` checks, hash-only refresh storage,
atomic single-use rotation, reuse-family revocation and logout revocation remain.
This is a project-specific DPoP-inspired protocol, **not a claim of RFC 9449
compatibility**. New Android clients reject a bearer-only establishment response
and require login for previously stored legacy sessions. BFF, legacy and non-Android
sessions remain bearer sessions. Their stolen credentials can still be reused
subject to existing expiry/revocation. Fully compromised devices can act as users.

### Recovery no longer synchronizes automatic reads

Admin Web, Doctor Web, Patient Flutter and Doctor Flutter automatic recovery now
uses a coalesced 250–2000ms jitter timer. Outage history resets only after a
successful read. Manual refresh remains immediate. Timer versions prevent disposed
or superseded callbacks from rearming work. Deterministic 1,000-client distribution,
overlap, outage-history and disposal tests were authored; they remain unexecuted.

## What was actually verified

The isolated PostgreSQL fixture executed 600 operations in six phases with at most
eight concurrent clients per phase. It independently observed one payable
obligation, one confirmation, one settlement/wallet credit, one Admin approval and
one payout effect, while retaining FAILED history. Same-event replay left counts
unchanged. Invalid settled-state regression and reference changes were rejected.
Every up migration and the new economic migration's down/up cycle passed.

The fixture had no external network, host credential or provider connection. Its
two-CPU/512-MiB quota describes only this PostgreSQL test container. It does not
establish backend VM capacity. Exact retained command, timing, rows and limits are
in [the results report](ADVERSARIAL_RESILIENCE_RESULTS.md) and
[machine-readable evidence](audit_evidence/resilience/sql-observer.json).

Rust, TypeScript, Dart and Python syntax/format checks ran without executing target
application modules. Full compile/typecheck/lint/regression did not run. Dart
formatting reported unresolved Flutter lint-package configuration. No application
RPS, p50/p95/p99, CPU/RSS, pool waiter peak or expensive-flood survival number is
claimed.

Independent source reviews found and corrected substantive integration issues;
their work was separate from the implementing agent. Authored tests and attacker
HTTP responses were not accepted as independent proof of financial correctness.
This is focused implementation/review, not complete codebase audit coverage.

## Test integrity and harness limitations

No existing financial balance, duplicate-row, cross-account authorization, winner
or participant-count requirement was intentionally removed. Identified old fixtures
were adapted for the new contracts: confirmed payments cannot be forced back to
FAILED; surplus concurrent requests may be explicitly shed before handlers; a
successful preflight uses a separate account from the lockout attack; OTP resend
fixtures model cooldown expiry; legacy migration tests create legacy rows instead
of calling new handlers against an old schema. Shed requests require exact codes,
nonempty safe messages, request IDs and Retry-After, rather than accepting arbitrary
errors. Full execution is required to establish all regressions pass; additional
functional-test timing/rate-budget conflicts may remain.

The new HTTP producer and separate observer support local synthetic identity/key
variation and authoritative snapshots. They have bounded worker/body/socket limits
and refuse external origins. They were not executed. Artificial commit-response
loss, real provider intent recovery, Android proof replay and all ten requested
full-stack load cases still need an isolated complete stack.

## Remaining launch blockers and deployment work

The applied [security-audit skill](../.agents/skills/security-audit/SKILL.md)
explicitly says: "If every control cannot be enforced, do not execute target code."
This prevents using the unrestricted host toolchains as a substitute for the
missing isolated application test environment. It does not prevent the source
changes, parser checks or the completed resource-limited PostgreSQL fixture.

1. Provision an OS-enforced isolated Rust/Node/Next/Flutter/Android toolchain and run
   complete build, regression and adversarial suites. The loaded security skill
   prohibits target execution without all its isolation/resource controls. Cached
   PostgreSQL/Redis images were available; suitable application toolchain images
   were not. Dependencies and host software were not installed to bypass that rule.
2. Integrate and independently test actual collection, payout, refund and provider
   reconciliation semantics. Until then checkout initiation is unavailable and
   successful real-world money movement cannot be certified.
3. Apply reviewed migrations through normal SQLx startup/migration tooling, restart
   the development backend before any persistent-schema verification, and use the
   existing schema verifier. This task changed source; it did not migrate/restart
   the shared backend. Review duplicate non-FAILED obligations/provider references
   before production migration; do not delete financial history to force it through.
4. Provision Admin SHA256 TOTP, coordinate the BFF assertion secret, configure Azure
   email and real Turnstile hostnames/actions/keys, and validate cookie/CSP/CSRF and
   direct-backend denial in an isolated browser/provider test setup.
5. Verify edge/WAF subscribed capabilities and restrict direct Azure origin exposure
   while deliberately preserving SSH, deployment, monitoring and recovery access.
   Add/review proxy connection/header/body-read/idle limits. No firewall change has
   been made by this task.
6. Measure safe target-hardware budgets and all distributed, login, upload and
   recovery scenarios. Reserved request slots do not guarantee dedicated DB
   connections or application availability under arbitrary mixed floods.

Play Integrity/app attestation, user-visible session management, server-side read
single-flight and a dedicated Admin incident-recovery reservation remain staged.
The current changes protect admitted work; they do not make sockets, mobile
endpoints or browser sessions immune to compromise.

## Deliverables

- [Economic invariant map](ECONOMIC_IDEMPOTENCY_MODEL.md).
- [Admission/edge/capacity model](DISTRIBUTED_ABUSE_AND_LOAD_SHEDDING.md).
- [Authentication and theft model, including MFA enrollment](AUTH_ABUSE_AND_SESSION_THEFT_MODEL.md).
- [Browser challenge configuration](WEB_AUTH_BOT_DEFENSE.md).
- [All requested final assertions and evidence limits](ADVERSARIAL_RESILIENCE_RESULTS.md).
- [Local harness and safe execution instructions](../services/backend/scripts/resilience/README.md).
- Economic, device-family binding and TOTP-algorithm paired up/down migrations,
  new regression test sources and implementation changes in the corresponding
  Rust, Next.js and Flutter/Android paths.

**TOKEN_HIJACKING_IMPOSSIBLE=NO. Tested financial database invariants passed under
the stated fixture conditions. Full launch-critical acceptance remains open.**
