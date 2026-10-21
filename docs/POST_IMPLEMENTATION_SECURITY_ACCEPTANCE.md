# Post Implementation Security Acceptance — 20 October 2026

Updated from **executed local development verification**, 19–20 October 2026. This is not formal sandbox certification or production acceptance. The owner authorized local toolchains/disposable services and froze original application source, tests, assertions, configuration and the security skill. Only new fixture/evidence files and these reports were changed.

Branch `main`; commit `dca9b3c5fcdb6303d31abbb77d3a1fd4ac1f879f`; dirty working tree on entry. [Entry baseline](audit_evidence/post_implementation_runtime_20261019/baseline.json) and [final preservation check](audit_evidence/post_implementation_runtime_20261019/source-freeze-final.json): **539 original files checked, zero changes, skill unchanged**. Web and Flutter manifests independently checked 132 and 425 original files (overlapping inventories, not additive). Earlier reports remain in [the historical archive](audit_evidence/post_implementation_runtime_20261019/previous-reports/POST_IMPLEMENTATION_SECURITY_ACCEPTANCE.md); earlier source corrections are not new fixes in this frozen campaign.

**Current verdict: NOT_ACCEPTED** (unchanged overall — see [`FINAL_AUTOMATED_ACCEPTANCE_MATRIX.md`](FINAL_AUTOMATED_ACCEPTANCE_MATRIX.md) for why). Docker availability allowed real builds, tests, database races and HTTP verification. The previous blanket environment blocker no longer applies. Remaining failures and missing acceptance evidence prevent accepting the complete milestone.

## 20 October continuation: backend Rust row updated

The "Rust" evidence row below is **superseded** for the Clippy and API-test-execution claims specifically, by a later same-day continuation authorized to make non-destructive backend corrections (unlike the frozen run this report otherwise describes). Full detail: [`FINAL_DEFECT_REMEDIATION_LOG.md`](FINAL_DEFECT_REMEDIATION_LOG.md); architecture/measurement for the admission-capacity defect found and fixed: [`FINAL_RESILIENCE_ROOT_CAUSE.md`](FINAL_RESILIENCE_ROOT_CAUSE.md).

- **Strict Clippy now PASSES** (0 diagnostics, `-D warnings`, full `--all-targets`) — the two diagnostics below are resolved.
- **All 275 `api_tests` cases now execute and pass** (two independent consecutive full-suite runs), not merely 1 of 264 compiled/listed. This includes the connection-pool/RLS-context cross-user-bleed stress test (`test_identity_concurrent_http_requests_no_cross_user_pool_bleed`, 128 interleaved identity-isolation requests) and all session-rotation/reuse/revocation/refresh-family cases — all now verified passing, with **zero cross-user data bleed detected** across every configuration tested, including a dedicated A/B capacity benchmark run four separate times.
- A genuine production defect was found and fixed in this pass: the admission-control layer's capacity was computed from one role's DB-pool size instead of the combined capacity across the five independent per-role pools the application actually runs — empirically confirmed to have **no effect on production's real default configuration** and to be safely bounded regardless of input (see `FINAL_RESILIENCE_ROOT_CAUSE.md` §4).
- This continuation did not touch or re-verify: Admin Web ESLint, Doctor Flutter's lifecycle-test failure, Flutter analyzer/format, Android, Turnstile/browser, Admin TOTP full-flow acceptance, or the distributed-load fixture. Those remain exactly as reported below.

## Executed evidence (19 October — original, unchanged below except as noted above)

| Surface | Actual outcome |
| --- | --- |
| Rust | Formatting, offline locked all-target check and backend binary build pass. 69 unit tests + one economic integration test + one enum migration test pass. API test binary compiles/lists 264 cases; the other 263 are not executed. Strict Clippy fails at two source locations. |
| Financial SQL | Fresh independent raw-SQL fixture: 62 migrations, six phases × 100 operations, at most eight workers; all observer phases DEFENSE_PASS. Actual economic SQLx test also passes. |
| OTP | Five real-Redis production-Lua cases, 207 reservations, at most eight workers, all final cases pass. No actual email or SMS. |
| Web | Admin 40 safe tests pass; five refresh tests not run. Doctor 78 pass. Both typechecks/builds pass. Admin ESLint fails three errors/nine warnings. Doctor custom package lint passes; ESLint is not configured. |
| Flutter | Patient 229 pass. Doctor 228 pass/one fail, reproduced unchanged. Each analyzer exits 1 on four informational lints; non-mutating format checks exit 1. |
| HTTP sessions | 27/27 synthetic ES256-bound access/refresh/logout checks pass against actual newly built Rust backend. |
| HTTP admission | Five staged role-auth runs totaling 31,300 requests; actual Argon2 peak four; authenticated profile and health samples remain 200. Large run has transport failures and incomplete telemetry, so full resilience acceptance is INCONCLUSIVE. |
| Dependency faults | Repaired fixture: 100/100 Redis-outage requests return 503, one incident row, readiness recovers to 200. Independently established SQL lock yields five blocked auth connections and five 10-second 408s; true pool exhaustion was not established. |

See [regression commands/counts](POST_IMPLEMENTATION_REGRESSION_RESULTS.md), [load measurements](POST_IMPLEMENTATION_LOAD_RESULTS.md), [deployment gaps](POST_IMPLEMENTATION_DEPLOYMENT_GAPS.md) and [final decision](POST_IMPLEMENTATION_FINAL_VERDICT.md).

## Control inventory

Backend-relative locations below refer to `services/backend/src/`, except migrations under `services/backend/migrations/`. Presence is source-backed; each verdict states its executed scope. Test presence never substitutes for execution.

| Control | Source location | Present | Test coverage executed | Result |
| --- | --- | --- | --- | --- |
| Financial obligation uniqueness | `migrations/20261110000001_economic_idempotency.up.sql; repository/transaction_repo.rs` | SOURCE_PRESENT | Economic repository race + raw SQL; Yes | VERIFIED_PASS (DB scope) |
| Callback replay protection | `repository/payment_event_repo.rs; services/appointment_service.rs` | SOURCE_PRESENT | Same/distinct-event repository and SQL phases; Yes | VERIFIED_PASS (tested DB cases; provider HTTP unverified) |
| Economic journal | `economic migration; financial_economic_effects` | SOURCE_PRESENT | SQL observer and economic assertions; Yes | VERIFIED_PASS (confirmation/settlement/payout DB scope) |
| Consultation settlement idempotency | `doctor_complete_consultation; consultation_service.rs` | SOURCE_PRESENT | 100-way completion race; Yes | VERIFIED_PASS (DB scope) |
| Payout uniqueness | `economic migration; disbursement repositories` | SOURCE_PRESENT | Two-Admin approval/payout SQL races; Yes | VERIFIED_PASS (local state; external transfer absent) |
| Global admission | `admission.rs` | SOURCE_PRESENT | Four unit tests + actual HTTP load; Yes | VERIFIED_PASS (auth/profile scope) |
| Local concurrency bounds | `admission.rs` | SOURCE_PRESENT | Unit lane bounds + sampled auth occupancy; Yes | VERIFIED_PASS (tested lanes; not all runtime lanes) |
| Redis shared budgets | `admission.rs; rate_limit.rs` | SOURCE_PRESENT | Actual HTTP admission + OTP Lua; Yes | VERIFIED_PASS (single backend; cross-replica not tested) |
| DB pool pressure | `admission.rs; repository acquisition helpers` | SOURCE_PRESENT | SQL-lock HTTP attempt and metrics; Yes, partial | INCONCLUSIVE (five SQL waits; pool not exhausted) |
| Argon2 workers | `crypto.rs` | SOURCE_PRESENT | Fail-fast unit + actual hash flood; Yes | VERIFIED_PASS (sampled peak four; cancellation race not tested) |
| Upload sanitizers | `services/upload_security_service.rs; rate_limit.rs` | SOURCE_PRESENT | Content/compression unit cases; Yes, content only | SOURCE_PRESENT_RUNTIME_UNVERIFIED (global flood/cancellation) |
| Account login throttling | `services/auth_abuse.rs; auth service` | SOURCE_PRESENT | 100 distinct-IP same-target calls per role; Yes | VERIFIED_PASS (development role routes; five hashes each) |
| OTP delivery budgets | `rate_limit.rs; services/auth_abuse.rs` | SOURCE_PRESENT | 207 actual Lua reservations, five cases; Yes | VERIFIED_PASS (Redis scope; Rust dispatch/provider not tested) |
| Turnstile | `both web src/lib/turnstile.ts and BFF routes` | SOURCE_PRESENT | 11 helper cases per app; Yes | VERIFIED_PASS (synthetic helper); browser/provider NOT_RUN |
| Admin HMAC assertion | `Admin web-assertion.ts; api/middleware/web_auth.rs` | SOURCE_PRESENT | Three Node cases + Rust pure validation; Yes | VERIFIED_PASS (pure helpers); Redis/HTTP gate NOT_RUN |
| Admin SHA256 TOTP | `services/admin_totp.rs; algorithm migration` | SOURCE_PRESENT | Published-vector unit; Yes, vector only | SOURCE_PRESENT_RUNTIME_UNVERIFIED (full factor/replay flow) |
| Android Keystore | `both DeviceProof.kt/MainActivity.kt bridges` | SOURCE_PRESENT | No native test found; No | SOURCE_PRESENT_RUNTIME_UNVERIFIED; compile cache missing |
| Backend device proof | `services/device_proof.rs; auth middleware` | SOURCE_PRESENT | Rust crypto unit + 27 HTTP checks; Yes | VERIFIED_PASS (synthetic ES256 client; not Keystore) |
| Refresh proof replay denial | `device_proof.rs; auth-service refresh ordering` | SOURCE_PRESENT | Actual access replay/rotation/reuse/logout; Yes | VERIFIED_PASS (sequential synthetic HTTP); concurrent races remain |
| Recovery jitter | `both web coordinators; Flutter screen_refresh.dart` | SOURCE_PRESENT | Seven tests/web, three/app; deterministic 1,000-client model; Yes | VERIFIED_PASS (unit scope); Doctor lifecycle failure separate |

## Economic identities and acceptance limits

The appointment is the obligation identity: at most one **non-FAILED** transaction, retaining legitimate FAILED history. Request keys cannot create another permitted obligation. Provider references are immutable after confirmation. The unique journal identity is `(effect_type, business_id)`; confirmation, settlement, refund approval and payout are distinct effects, so a consultation can legitimately have several journal rows of different types.

Fresh DB observers found one active obligation, two FAILED historical attempts, one confirmation effect, 100 event records unchanged by replay, one settlement effect, one Doctor earning/net KES 80 credit, one winning Admin and one payout effect. Conditional confirmation/completion/approval/payout and uniqueness were exercised. These tests use synthetic superuser fixture connections; they are not comprehensive negative capability-role/RLS tests. Actual HTTP profile operations did use the application's capability pools, without proving every isolation boundary.

| Acceptance category | Decision and boundary |
| --- | --- |
| DB_ECONOMIC_INVARIANTS | PASS for executed economic repository/raw SQL cases. Refund concurrency, every cross-payment event/late callback and full financial permission negatives remain unexecuted. |
| HTTP_FINANCIAL_IDEMPOTENCY | SOURCE_PRESENT_RUNTIME_UNVERIFIED; no same/different-key checkout/lost-response HTTP acceptance. |
| PROVIDER_EFFECTS | NOT_IMPLEMENTED for actual provider collection intent/success acceptance. No synthetic 503 is graded as a payment idempotency success. |
| REFUND_EXECUTION / PAYOUT_EXECUTION | Local database state is not external money movement; real provider execution NOT_IMPLEMENTED. |
| RECONCILIATION | Authoritative real-provider late-result/recovery acceptance INCONCLUSIVE. |

## Session security demonstrated

The 27 HTTP assertions include two bound-session establishments, valid access, denial without proof, wrong key/method/path/time/token binding, first proof acceptance and replay denial, stolen-refresh denial without revoking legitimate access, valid rotation, reuse-family revocation with a valid key and logout revocation. Independent SQL reads observed two bound families and all three corresponding session rows revoked after those flows. Proof private keys/tokens remained in process memory and were not retained in evidence.

This is a project-specific DPoP-inspired protocol, not an RFC 9449 compliance claim. It verifies backend interoperability with a synthetic Node ES256 client, not Android Keystore execution. It does not cover concurrent proof/refresh races, Redis replay failure during a bound request, lost installation key UI, cross-session key reuse or every malformed proof case. Unbound legacy/BFF/non-Android sessions still have bearer-token residual risk. `TOKEN_HIJACKING_IMPOSSIBLE=NO`.

## OTP and database abuse answer

An attacker can keep sending OTP requests; a limit cannot stop packets arriving. Actual production Lua testing shows denied recipient/global/rolling/daily reservations do not partially consume other quotas, one same-recipient delivery reservation survives 101 concurrent attempts, and at most 30 different-recipient reservations succeed within the modeled minute. Dispatch failures remain charged by design in reviewed source, but actual failed-email delivery was not tested. These are delivery reservations, not 207 emails. Real-time 15-minute/day expiry and Azure delivery are unverified.

Similarly, the backend rejects much distributed authentication traffic before proportional hashing/SQL work. In the 30,000-request model, auth admissions increased by 87 and Argon2 operations by 50, rather than 30,000. SQL query count and DB waiters were not measured; acquisition counters are not query counters. A bot can consume the allowed global budget and affect availability. Public launch still requires verified ingress and measured legitimate consultation availability; neither OTP abuse nor DDoS immunity is claimed.

## Unchanged failures and authorization boundary

No application fixes were applied during this frozen run. Clippy, Admin ESLint, Flutter analyzer/format results and the Doctor queue lifecycle assertion are retained as failures. The latter expects two queue reads but observes one at line 73; an interaction with intentional jitter is a source inference, not an accepted test correction. No queue feature reimplementation was undertaken.

The owner's local-execution override applies to this development verification only; the security skill remains unchanged. Cached offline dependencies and fresh synthetic fixtures were used. No shared DB/Redis, provider, production service, physical phone, firewall or live edge was used. Full API/Admin refresh tests remain unexecuted because they explicitly target shared localhost Redis; the original tests were not changed to bypass this boundary.
