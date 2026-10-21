# Adversarial resilience results — 19 October 2026 (Asia/Dhaka)

**Milestone acceptance: INCONCLUSIVE. Do not treat this change as launch approval.**
Implementation and independent source review are recorded below. The isolated
PostgreSQL fixture passed; full application regression, HTTP surge and Android
interoperability have not been executed in the required sandbox.

## Executed environment and evidence

Command: `./services/backend/scripts/resilience/Invoke-IsolatedSql.ps1`.
Machine-readable evidence: [sql-observer.json](audit_evidence/resilience/sql-observer.json).
The final [source manifest](audit_evidence/resilience/source-manifest.json) records
66 scoped code/migration/harness hashes. Docker's daemon was unavailable during
that later metadata capture; the manifest retains only the earlier observed image
ID prefix and does not invent a full image digest. SQL results below are from the
earlier completed isolated run, not a subsequent rerun.
The retained successful run was recorded at `2026-10-18T18:27:55Z` (19 October
in Dhaka). Total fixture wall time was 6.615 seconds, including cluster startup,
migrations, all phases and the economic down/up cycle. This is **not HTTP
latency or a requests-per-second capacity measurement**.

The existing cached `postgres:16` image ran in a disposable container with:

- No external network, published port, shared database, host credential, provider
  call or production account. PostgreSQL used a scratch-local Unix socket.
- Read-only root, migration and harness mounts; PostgreSQL ran as an unprivileged
  user. `/dev/shm` and `/dev/mqueue` were read-only; writable fixture data lived in
  the 512 MiB scratch tmpfs.
- An empty child environment populated only with safe PATH, HOME, TMPDIR and PG
  connection values. HOME was scratch-local.
- Two CPU quota, 512 MiB RAM with no extra swap, 64-process maximum, open-file and
  per-file-size limits, a 180-second wall deadline with a five-second kill grace.
- Synthetic `.invalid` identities, fixed dummy principals and simulated provider
  references. Neither host software nor dependencies were installed.

Every current up migration was applied in its own transaction. The fixture creates
only SQLx's empty metadata **schema**, never fabricated migration rows. This raw
SQL run does not verify SQLx startup or the shared dev backend's schema. The shared
backend was not restarted or migrated; no shared-service availability test ran.
Starting the already-installed Docker Desktop also started its existing local
PostgreSQL/Redis containers; the tests did not connect to those containers.

## Independent database observer

Each phase submitted 100 operations using **at most eight concurrent clients**.
This is not a claim of 100 simultaneous HTTP requests. Workers performed the SQL
operations; `observe.sql` independently queried committed tables after each phase.

| Case | Committed result | Verdict |
| --- | --- | --- |
| Distinct attempted obligations for one appointment | One non-FAILED transaction; two FAILED historical attempts retained | DEFENSE_PASS, DB scope |
| 100 distinct success event IDs | 100 event rows; one PAYMENT_CONFIRMED economic effect | DEFENSE_PASS, DB scope |
| Replay one already-recorded event 100 times | Event count remains 100; confirmation effect remains one | DEFENSE_PASS, DB scope |
| 100 consultation completion calls | One CONSULTATION_SETTLED effect; Doctor net wallet credit exactly KES 80 for KES 100 gross/KES 20 fee | DEFENSE_PASS, DB scope |
| 100 approvals alternating two Admins | One APPROVED transition and one winning Admin UUID retained | DEFENSE_PASS, DB scope |
| 100 payout confirmations | One PAID economic effect | DEFENSE_PASS, DB scope |
| Settled payment changed to FAILED | Database check rejects transition | DEFENSE_PASS |
| Recorded provider reference changed | Database check rejects mutation | DEFENSE_PASS |
| New economic migration down then up | Both transactions succeed in disposable fixture | DEFENSE_PASS |

The winning synthetic Admin was `44444444-4444-4444-4444-444444444444`.
There is one appointment and one payable obligation, with legitimate FAILED
history. Confirmation, settlement and payout have different economic identities;
one event per kind does not mean one journal row across all lifecycle stages.
Patient cash debits and a separate platform cash ledger do not exist here.
Authoritative external provider success, actual consultation activation, HTTP
response recovery, refund execution, audit/notification exactly-once delivery and
reconciliation were not established by this database fixture.

## Source checks and authored regression cases

- Rustfmt parsed/formatted the changed Rust modules and economic suite. This is
  syntax/format evidence, **not a Rust build or typecheck**.
- Node parsed ten focused TypeScript helper/test sources with
  `--experimental-strip-types --check`; target modules were not executed. This
  does not typecheck Next.js or parse/render the TSX widgets.
- Dart formatted eleven changed source/test files. It warned that the local
  `flutter_lints/flutter.yaml` include could not be resolved. No Flutter analyze,
  widget test, Gradle build or device run is claimed.
- Python AST parsing passed for both HTTP harness programs without execution.
- Focused whitespace checks passed. The pre-existing unrelated trailing whitespace
  in `docs/PROGRESS.md` prevents an unqualified whole-workspace clean-diff claim.

Authored cases include the database repository race suite, production admission
30,000-identity permit/bucket checks, expensive-lane isolation checks, all three
Argon2 saturation paths, RFC 6238 SHA256 vectors, real synthetic ES256 proof
tampering/request-binding cases, eleven Turnstile/body-budget cases per web app,
three Admin assertion cases, and recovery-jitter cases across both web and Flutter
apps including deterministic 1,000-client timer distributions. **These test sources
have not been executed.**

Independent source reviewers checked the economic changes, admission reservations,
BFF assertion canonicalization, native JWK/signature format, refresh proof ordering,
TOTP state/locking and credential logging paths. They found and helped correct:
the nested Axum `/api/v1` path mismatch, small-pool auth starvation, missing Doctor
critical-route classification, post-commit device binding, nested auth-pool
acquisition and the native signing executor's unbounded queue. Source review does
not supply missing runtime evidence.

## HTTP and load harness

`scripts/resilience/attack-local.py` is a bounded HTTP producer for a separately
isolated actual HelloDoctor stack: maximum 30,000 logical clients, 32 workers,
bounded producer queue, two-second sockets and small request/response bodies.
It can vary request keys and synthetic account targets. It records responses,
latencies, logical addresses and request/trace IDs without credential values.
It produces no defense verdict. Logical addresses require an explicitly configured
isolated trusted ingress; production trusts no arbitrary forwarded identity.

`observe-local.py` grades independently collected DB/telemetry snapshots. Missing
provider state or resource evidence yields INCONCLUSIVE; a refused/unavailable
checkout is not graded as a successful idempotent provider operation. Neither
program was executed. See [harness instructions](../services/backend/scripts/resilience/README.md).

## Requested financial assertions

| Assertion | Result |
| --- | --- |
| PATIENT_PAYMENT_SAME_KEY_IDEMPOTENCY_VERIFIED | INCONCLUSIVE — HTTP/provider flow unrun, provider adapter absent |
| PATIENT_PAYMENT_DIFFERENT_KEY_BUSINESS_IDEMPOTENCY_VERIFIED | DB_DEFENSE_PASS; HTTP/provider acceptance INCONCLUSIVE |
| ADMIN_FINANCIAL_ACTION_IDEMPOTENCY_VERIFIED | DB approval/payout DEFENSE_PASS; all Admin HTTP/refund/reconciliation paths INCONCLUSIVE |
| PROVIDER_WEBHOOK_REPLAY_SAFE | DB event/effect replay DEFENSE_PASS; authenticated HTTP/provider integration INCONCLUSIVE |
| FINANCIAL_LEDGER_EXACTLY_ONCE_VERIFIED | DB prospective confirmation/settlement/payout identities DEFENSE_PASS; complete financial lifecycle INCONCLUSIVE |
| PAYMENT_OBLIGATIONS_CREATED | 1 active logical obligation in DB fixture |
| ACTIVE_PAYMENT_ATTEMPTS | 1, with 2 FAILED historical rows preserved |
| AUTHORITATIVE_PROVIDER_SUCCESS_EFFECTS | NOT VERIFIED; one synthetic DB confirmation effect |
| CONSULTATIONS_ACTIVATED | NOT VERIFIED by executable fixture |
| PATIENT_DEBITS | NOT APPLICABLE — no patient cash wallet mutation exists |
| DOCTOR_CREDITS | 1 measured wallet credit, KES 80 |
| PLATFORM_FEE_CREDITS | No separate cash ledger; KES 20 withheld in transaction/wallet accounting |
| LEDGER_ECONOMIC_EFFECTS | 1 per tested economic kind/business identity |

## Requested origin resilience assertions

`SOURCE_PRESENT` means implemented and reviewed, not dynamically load verified.

| Assertion | Result |
| --- | --- |
| PER_IP_RATE_LIMIT_PRESENT / PER_IP_LIMIT | SOURCE_PRESENT |
| PER_ACCOUNT_RATE_LIMIT_PRESENT / PER_ACCOUNT_LIMIT | SOURCE_PRESENT |
| GLOBAL_RATE_BUDGET_PRESENT | SOURCE_PRESENT — local buckets plus shared Redis windows |
| GLOBAL_CONCURRENCY_BOUND_PRESENT | SOURCE_PRESENT — no-wait lane permits |
| EXPENSIVE_ENDPOINT_BULKHEAD_PRESENT | SOURCE_PRESENT |
| ARGON2_CONCURRENCY_BOUNDED | SOURCE_PRESENT — 4 actual worker permits, try-acquire |
| UPLOAD_CONCURRENCY_BOUNDED | SOURCE_PRESENT — 4 sanitizers, 2 uploads/account plus route cap |
| DB_PRESSURE_FAIL_FAST_PRESENT | SOURCE_PRESENT — early pool signal, admission and common 250ms transaction acquisition |
| DISTRIBUTED_MULTI_IP_SURGE_TEST | INCONCLUSIVE — full HTTP load not run |
| RECOVERY_STAMPEDE_TEST | INCONCLUSIVE — jitter implemented/test sources authored, not executed |
| ORDINARY_API_SURVIVES_EXPENSIVE_ENDPOINT_FLOOD | INCONCLUSIVE — reserved lane source reviewed; latency/CPU/pool evidence absent |
| UNBOUNDED_REQUEST_QUEUE | No handler admission queue in source; socket/header handling still requires bounded production ingress |
| UNBOUNDED_DB_WAITERS | HTTP-derived work is bounded in source; measured waiter peaks unavailable |
| UNBOUNDED_UPLOAD_DECODERS | FALSE in source worker dispatch; runtime test pending |
| UNBOUNDED_ARGON2_WORK | FALSE in source worker dispatch; runtime test pending |
| EXCESS_TRAFFIC_FAILS_FAST | SOURCE_PRESENT, runtime latency unmeasured |
| CORE_DB_REMAINS_PROTECTED | INCONCLUSIVE under distributed HTTP load |
| CRITICAL_PROVIDER_WEBHOOK_PATH_PROTECTED | Separate bounded admission exists; end-to-end provider/DB starvation test INCONCLUSIVE |

No accepted/rejected HTTP RPS, p50/p95/p99, Redis operation count, CPU/RSS peak,
DB acquire timeout count or ordinary-route latency under attack is invented.
No target-hardware capacity pass threshold was selected without measurements.

## Requested authentication and theft assertions

| Assertion | Result |
| --- | --- |
| PATIENT_LOGIN_DISTRIBUTED_BOT_DEFENSE | SOURCE_PRESENT; 30,000-client runtime INCONCLUSIVE |
| DOCTOR_LOGIN_DISTRIBUTED_BOT_DEFENSE | SOURCE_PRESENT; runtime INCONCLUSIVE |
| ADMIN_LOGIN_DISTRIBUTED_BOT_DEFENSE | SOURCE_PRESENT including required BFF assertion; runtime INCONCLUSIVE |
| GLOBAL_AUTH_BUDGET | SOURCE_PRESENT — separate admission/Redis budget |
| OTP_GLOBAL_BUDGET | SOURCE_PRESENT — 30/min shared delivery budget |
| ADMIN_WEB_TURNSTILE_SERVER_VERIFIED | Server validation implemented; provider/browser execution INCONCLUSIVE |
| DOCTOR_WEB_TURNSTILE_SERVER_VERIFIED | BFF server validation implemented; native-style direct route remains budget/credential protected; runtime INCONCLUSIVE |
| PATIENT_MOBILE_APP_ATTESTATION_STATUS | NOT_IMPLEMENTED — Play Integrity is a deployment/product integration gap |
| DOCTOR_MOBILE_APP_ATTESTATION_STATUS | NOT_IMPLEMENTED |
| ACCESS_JWT_SHORT_LIVED | SOURCE_PRESENT — 15 minutes |
| ACTIVE_SESSION_CHECK_EVERY_REQUEST | SOURCE_PRESENT — every protected API request |
| REFRESH_ROTATION_SINGLE_USE | SOURCE_PRESENT; fresh runtime regression INCONCLUSIVE |
| REFRESH_REUSE_FAMILY_REVOCATION | SOURCE_PRESENT; fresh runtime regression INCONCLUSIVE |
| LOGOUT_SERVER_REVOCATION | SOURCE_PRESENT; fresh runtime regression INCONCLUSIVE |
| WEB_JWT_JAVASCRIPT_READABLE | NO in inspected BFF source |
| WEB_REFRESH_JAVASCRIPT_READABLE | NO in inspected BFF source |
| AUTH_TOKEN_IN_URL | No access/refresh token URL transport found in inspected source; scoped document grants are separate |
| AUTH_TOKEN_IN_LOGS | No explicit credential logging found in inspected application source; deployment/telemetry execution unverified |
| MOBILE_ACCESS_TOKEN_REPLAY_WITHOUT_DEVICE_KEY | DENIED by source for newly bound Android families; runtime INCONCLUSIVE |
| MOBILE_REFRESH_REPLAY_WITHOUT_DEVICE_KEY | DENIED by source for newly bound Android families; runtime INCONCLUSIVE |
| DEVICE_PROOF_REPLAY | Atomic Redis denial implemented; runtime replay race INCONCLUSIVE |
| TOKEN_HIJACKING_IMPOSSIBLE | NO |
| STOLEN_BEARER_TOKEN_ALONE_REUSABLE | YES for legacy/BFF/non-Android bearer sessions; NO in source for newly bound Android sessions |
| STOLEN_REFRESH_TOKEN_ALONE_REUSABLE | YES for unbound sessions; NO in source for newly bound Android sessions |
| WEB_TOKEN_EXPOSED_TO_JS | NO in inspected BFF source; active XSS can still issue authenticated requests |
| SERVER_SESSION_REVOCATION_VERIFIED | Source reviewed; new runtime acceptance INCONCLUSIVE |
| DEVICE_BOUND_SESSION_PROOF_VERIFIED | Source reviewed and synthetic tests authored; Android/backend interoperability INCONCLUSIVE |

## Remaining acceptance and deployment gates

The [security skill](../.agents/skills/security-audit/SKILL.md) requires target
execution in an OS-enforced sandbox and says, "If every control cannot be
enforced, do not execute target code." Cached container images supplied
PostgreSQL/Redis only; isolated Rust, Node/Next and Android toolchains were not
available. Host toolchain presence does not meet those execution controls.
No dependency/image installation was performed to bypass that requirement.

Provide pre-provisioned isolated toolchains and run backend build/Clippy/unit and
serialized API/economic suites, both Next typechecks/tests/builds, both Flutter
analyze/tests/builds, Android synthetic proof interoperability/replay, and all
ten requested HTTP/load/recovery scenarios. Some older functional/race tests
assumed every request reached the handler or that OTP resends were immediate;
identified fixture contracts were adapted with original money/authorization
assertions preserved. Full regression is not claimed green and further conflicts
may remain.

Separately required for production: real collection/refund/payout/reconciliation
adapters and provider semantics, provisioned Admin SHA256 TOTP, coordinated BFF
assertion secret, actual Turnstile keys/actions/hostnames, Azure email delivery,
TLS/proxy configuration, edge/WAF capability verification, restricted origin,
and reverse-proxy socket/header/body/idle limits. App attestation, active-session
UI, server-side public-read single-flight and dedicated Admin recovery admission
remain staged gaps. No host firewall or cloud resources were altered. No immunity
from DDoS or complete prevention of session hijacking is claimed.
