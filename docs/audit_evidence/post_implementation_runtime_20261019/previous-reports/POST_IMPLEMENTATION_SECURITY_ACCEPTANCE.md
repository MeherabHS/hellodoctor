# Post-implementation security acceptance — 19 October 2026

**Acceptance is INCONCLUSIVE_ENVIRONMENT. Source review and parser checks do not establish runtime acceptance.** This is a scoped verification of the delivered milestone, not a replacement audit or a launch certificate. Historical evidence remains in its original files.

## Reviewed revision and evidence

- Branch `main`; commit `dca9b3c5fcdb6303d31abbb77d3a1fd4ac1f879f`; extensive dirty working tree on entry.
- [Fresh baseline](audit_evidence/post_implementation_20261019/baseline.json): 539 scoped source, test, manifest, migration and harness hashes. This is the working-tree revision; commit identity alone does not describe it.
- [Historical comparison](audit_evidence/post_implementation_20261019/historical-source-comparison.json): all 66 files in the previous implementation manifest matched at campaign entry. This supports source continuity, not repeat execution.
- Three independent, read-only subsystem reviewers examined financial, authentication/native/web, and resilience/deployment paths. The parent authored the corrections below; reviewers separately reviewed those corrections. None executed target code.
- Required economic/auth/abuse/web/results/harness documents and the security skill were read. Cargo manifests/lockfile, migrations, test sources, BFF routes, native bridges, Flutter clients, workflows and deployment documentation were inspected. Admin Web's applicable `AGENTS.md` was read; no Admin Web code was changed.
- Current lockfile includes Axum 0.7.9, SQLx 0.8.6, Argon2 0.5.3, deadpool-redis 0.23.1, jsonwebtoken 9.3.1 and Tokio 1.53.2. This was manifest inspection, not dependency resolution, a vulnerability scan or a successful build.

Runtime execution requires the [security skill](../.agents/skills/security-audit/SKILL.md), which states: **“If every control cannot be enforced, do not execute target code.”** The Docker API is unavailable. No alternative sandbox with all required controls was demonstrated. The user explicitly prohibited starting services, installing software or downloading images. No target application, test, browser, emulator, provider or shared database was executed.

## Phase 0 control inventory

Paths below are relative to the repository root. `Yes` in the test column means authored coverage exists for part of the control; it does not mean complete coverage of every acceptance scenario. Every runtime execution column is **No** for this campaign.

| Control | Source location | Present/classification | Test exists | Test executed | Result |
| --- | --- | --- | --- | --- | --- |
| Financial obligation uniqueness | `services/backend/migrations/20261110000001_economic_idempotency.up.sql:3`; `src/repository/transaction_repo.rs` | SOURCE_PRESENT | `tests/economic_idempotency.rs` | No | BLOCKED_ENVIRONMENT; historical raw-SQL pass only |
| Callback replay protection | `src/repository/payment_event_repo.rs`; `src/services/appointment_service.rs:266` | SOURCE_PRESENT | Economic repository race and API webhook replay | No | BLOCKED_ENVIRONMENT |
| Economic journal | Economic migration, `financial_economic_effects`, unique `(effect_type,business_id)` | SOURCE_PRESENT | SQL observer/economic test | No | BLOCKED_ENVIRONMENT; historical confirmation/settlement/payout DB evidence |
| Consultation settlement idempotency | Economic migration's `doctor_complete_consultation`; `src/services/consultation_service.rs` | SOURCE_PRESENT | Completion races/economic test | No | BLOCKED_ENVIRONMENT |
| Payout uniqueness | Economic migration's provider-reference index; `src/repository/wallet_repo.rs` | SOURCE_PRESENT | Admin payout/economic race | No | BLOCKED_ENVIRONMENT; reference collision coverage incomplete |
| Global admission | `src/admission.rs`; `src/api/router.rs` | SOURCE_PRESENT | Four admission unit cases | No | BLOCKED_ENVIRONMENT |
| Local concurrency bounds | `src/admission.rs:135`; no-wait owned permits | SOURCE_PRESENT | Admission permit/reservation cases | No | BLOCKED_ENVIRONMENT |
| Redis global budgets | `src/admission.rs`; `src/rate_limit.rs` | SOURCE_PRESENT | Partial unit/API coverage | No | BLOCKED_ENVIRONMENT; distributed HTTP absent |
| DB pool pressure handling | `src/admission.rs`; `src/repository/mod.rs`; `src/main.rs` | SOURCE_PRESENT | Partial admission/API coverage | No | BLOCKED_ENVIRONMENT; controlled saturation not exercised |
| Argon2 bounded workers | `src/crypto.rs` | SOURCE_PRESENT | Exhausted-permit case, three password paths | No | BLOCKED_ENVIRONMENT; actual four-worker/cancellation measurement absent |
| Upload sanitizer bounded workers | `src/services/upload_security_service.rs`; `src/rate_limit.rs` | SOURCE_PRESENT | Content/compression tests; no dedicated worker cancellation/saturation case found | No | BLOCKED_ENVIRONMENT |
| Account login throttling | `src/services/auth_abuse.rs`; auth-service call sites | SOURCE_PRESENT | Existing account/IP jail tests; proactive cross-IP acceptance incomplete | No | BLOCKED_ENVIRONMENT |
| OTP delivery budgets | `src/rate_limit.rs:14`; `src/services/auth_abuse.rs:19` | SOURCE_PRESENT, corrected this campaign | New atomic reservation integration case | No | BLOCKED_ENVIRONMENT |
| Turnstile | Both web apps' `src/lib/turnstile.ts`, login routes/widgets | SOURCE_PRESENT | Eleven helper tests per app | No | BLOCKED_ENVIRONMENT; browser/provider acceptance absent |
| Admin HMAC assertion | Admin `src/lib/web-assertion.ts`; Rust `src/api/middleware/web_auth.rs` | SOURCE_PRESENT | Three JS helper cases; Rust pure verification case | No | BLOCKED_ENVIRONMENT; full nonce/HTTP replay absent |
| Admin TOTP | `src/services/admin_totp.rs`; auth service; `20261110000004_admin_totp_algorithm` | SOURCE_PRESENT | Published SHA256 vector unit case | No | BLOCKED_ENVIRONMENT; enrollment/replay/concurrent/legacy HTTP cases absent |
| Android Keystore signing | Both apps' `android/app/src/main/kotlin/**/DeviceProof.kt`, `MainActivity.kt` | SOURCE_PRESENT | No native bridge test found | No | BLOCKED_ENVIRONMENT |
| Backend device-proof enforcement | `src/services/device_proof.rs`; auth middleware/service | SOURCE_PRESENT | Pure cryptographic rejection test | No | BLOCKED_ENVIRONMENT; native interoperability unverified |
| Refresh proof replay denial | Device-proof Redis NX reservation and auth-service refresh ordering | SOURCE_PRESENT | Pure validation only; Redis/family replay integration missing | No | BLOCKED_ENVIRONMENT |
| Recovery jitter | Both web coordinators and Flutter `screen_refresh.dart` | SOURCE_PRESENT | Seven recovery cases per web app; three per Flutter app | No | BLOCKED_ENVIRONMENT |

`src/` and `tests/` without an application prefix in this table refer to `services/backend/`. Machine-readable inventory is [control-inventory.json](audit_evidence/post_implementation_20261019/control-inventory.json).

## Financial acceptance boundaries

The intended database invariant is at most one **non-FAILED** transaction per appointment. Settled/refunded transactions still occupy that obligation; legitimate FAILED history is retained. Provider/session/reference identities and amounts cannot be rewritten after confirmation. Callback code locks appointment then transaction, rereads state, checks gateway/reference and event ownership, and conditionally changes payment state. Expired/cancelled bookings do not activate consultations.

The prospective journal is append-only for application roles and unique per event kind/business identity. Confirmation, consultation settlement, refund approval and payout are distinct kinds. One consultation can legitimately have several journal rows across these kinds; “exactly one economic effect” must be asserted per kind. Historical balances are not backfilled into invented journal events.

Completion uses the database state transition and credits wallet state in the same transaction. Repeat completion returns before repeating earning notification/audit/trace. Admin approvals record the winning actor through conditional updates. Refund approval locks/rereads and performs its local reversal/request/status changes transactionally. None proves external notification exactly-once delivery or real provider money movement.

| Acceptance area | Current evidence | Result |
| --- | --- | --- |
| DB_ECONOMIC_INVARIANTS | Historical fixture: 600 SQL operations, maximum eight clients, one active obligation, two FAILED rows, one confirmation/settlement/payout effect, KES 80 Doctor net credit | Historical DATABASE_TESTED; current rerun BLOCKED_ENVIRONMENT |
| HTTP_FINANCIAL_IDEMPOTENCY | Source and unrun API/economic tests; corrected replay fixture | SOURCE_PRESENT_RUNTIME_UNVERIFIED |
| PROVIDER_EFFECTS | Live collection adapter/production callback verification absent; payment-resume refuses unavailable checkout | NOT_IMPLEMENTED |
| REFUND_EXECUTION | Local approval/reversal/request lifecycle; no actual provider refund driver | NOT_IMPLEMENTED |
| PAYOUT_EXECUTION | Conditional Admin-recorded payout confirmation; no independently observed provider transfer | NOT_IMPLEMENTED |
| RECONCILIATION | Late-capture trace/manual workflow; no authoritative provider-result reconciler demonstrated | INCONCLUSIVE |

Missing dedicated acceptance cases include cross-payment event-ID replay, callbacks after refund/cancellation, paid/refunded obligation reopening, concurrent refunds with wallet/journal observations, provider-reference collisions, and lost-response recovery. The new expiry case models an already-expired fixture; it does not establish the actual expiration race. Checkout recovery is `INCONCLUSIVE_PROVIDER_NOT_INTEGRATED`.

## Authentication/session invariants

Protected requests validate the signed access token and authoritative active family/account/role. Bound Android families require signed proofs for access, refresh and logout regardless of spoofable client-type headers. Proofs bind method, original external path, time, unique JTI and the relevant credential/challenge. Redis replay reservation fails closed. Refresh proof validation precedes family reuse revocation, so possession of a stolen bound refresh token alone must not let an attacker revoke the legitimate family. Session insertion and family key binding share a transaction.

The native and Flutter source agree on channel/method/argument names, P-256 public-key representation, 64-byte ES256 signature representation and token-binding hash. These are source observations, not compilation or physical-device results. Existing injected-Dio tests disable the native signer outside release; they cannot certify sender constraint. No native bridge test or cross-language signature fixture was found.

Admin production/staging policy requires password, email OTP and SHA256 TOTP. Row locking and consumed time steps are present. Existing SHA1 metadata is explicitly rejected; a deployment must provision a SHA256 authenticator. The explicit development OTP bypass is not production acceptance. Enrollment, replay, concurrent TOTP and session issuance need integration tests.

OTP account/recipient/global limits precede challenge persistence/provider dispatch. The corrected delivery reservation is one four-key Lua operation on the current standalone Redis architecture. A rejected recipient no longer consumes other windows in source. Successful reservations remain charged on dispatch failure or ambiguous timeout. Arbitrary distinct recipients can still exhaust legitimate shared capacity; this is bounded service availability, not an immunity claim. Redis Cluster would need colocated keys before this multi-key script could be used there.

Registration still returns a duplicate-registration conflict. Therefore the campaign's blanket account-existence nondisclosure requirement is **SOURCE_PARTIAL**: login has generic errors/dummy hashing, but registration needs an explicit product-policy decision. No equal-latency network result is claimed.

BFF challenges precede upstream credential work. Admin additionally signs exact request bytes for Rust. HttpOnly/Secure production/SameSite cookies and origin checks remain. Helper tests are synthetic; rendered widget reset, CSP, cookie behavior, full BFF-to-Rust nonce replay, real Siteverify and Azure delivery are unverified.

## Abuse/admission acceptance

Admission precedes IP Redis checks, credential/session SQL, extraction and handlers. At the ten-connection-per-role default, ordinary concurrency is 20; Normal/Auth/Upload/Payment/Expensive lane caps are 8/4/4/2/2. Consultation/Webhook/Health reservations are 4/2/2, yielding at most 28 admitted handler requests combined. Local buckets and Redis windows supplement these no-wait permits. These are policy values, not tested RPS or capacity.

Argon2 and sanitizer permits are owned by the blocking closure until actual completion, including caller cancellation. Each has four workers with try-acquire. Per-account upload concurrency remains two. Early DB pressure sampling and common 250ms transaction acquisition are present; direct pool acquisitions can retain the existing five-second pool deadline. Reservations do not reserve DB connections.

The corrected `/healthz` checks DB/migration readiness and, if that is ready, Redis with a separate 250ms deadline. It is **dependency readiness**, not an instruction to restart the process when Redis is unavailable. DB failure short-circuits before Redis is checked. No readiness outage test was executed.

Custom HTTP methods now map to `OTHER` before metric labels are retained. Route labels are matched templates or `__unmatched`. No memory/cardinality load measurement is claimed.

## Narrow corrections and independent source review

| ID | Original source defect | Correction | Regression coverage/status |
| --- | --- | --- | --- |
| C1 | Sequential OTP reservations consumed global capacity even when recipient cooldown/rolling/daily denied | One atomic check-all/reserve-all Lua invocation; same budgets/deadline/hashed identities | New integration test: 100 cooldown denials; rolling/daily unchanged counters; 100 distinct global contenders, at most eight workers; overflow. NOT_RUN |
| C2 | Arbitrary valid HTTP methods became persistent metric labels | Nine standard methods plus `OTHER` | New pure unit case with 1,000 custom methods. NOT_RUN |
| C3 | Readiness could report ready while required Redis admission failed closed | Bounded Redis PING after successful DB/migration check | Existing healthy assertions preserved; new fixture-owned unavailable-Redis case. NOT_RUN |
| C4 | Webhook text claimed a locked consultation even for expired/cancelled state | Neutral message directs callers to authoritative status | New expired/replayed signed development callback with committed 0 activation/1 confirmation/1 event assertions. NOT_RUN |
| C5 | Existing distinct-event replay test changed immutable provider reference but expected HTTP 200 | Reuse reference when changing event ID; separate invalid-reference 409 case | Original assertions retained; additional committed event/effect assertions. NOT_RUN |

These are source corrections, not observed runtime test failures repaired into passing suites. No critical vulnerability was confirmed by this campaign. Absence of a confirmed critical finding does not certify absence of critical defects.

## Evidence interpretation and next acceptance targets

Fresh source-format/parsing checks passed for thirteen Rust inputs, ten TypeScript files, eight Dart files and two Python harnesses. They do not typecheck, execute test bodies, resolve Next/Flutter application dependencies, compile Android or validate SQLx startup. See [regression results](POST_IMPLEMENTATION_REGRESSION_RESULTS.md).

Next targets, in order: provision the fully isolated offline toolchain; run builds and preserved regressions; run serialized SQLx financial/migration/RLS suites; add/execute missing TOTP/device replay/provider-fixture cases; measure isolated load with an independent observer; then perform separately authorized provider, device and deployment acceptance. [Deployment gaps](POST_IMPLEMENTATION_DEPLOYMENT_GAPS.md) distinguishes missing implementation from missing evidence. [Final verdict](POST_IMPLEMENTATION_FINAL_VERDICT.md) contains all owner-facing assertions.
