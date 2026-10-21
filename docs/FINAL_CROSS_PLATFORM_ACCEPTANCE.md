# HelloDoctor cross platform acceptance — 20 October 2026

This report records the current dirty worktree at commit `dca9b3c5fcdb6303d31abbb77d3a1fd4ac1f879f`. Source aggregate hashes and file counts are in [the run manifest](audit_evidence/final_cross_platform_20261020/source-hashes.json). Historical October 19–20 reports remain unchanged.

The current owned fixtures were PostgreSQL `hd-remediation-pg-20261020` on loopback port 15433 and Redis `hd-remediation-redis-20261020` on loopback port 16380, with the ownership marker `final-remediation-20261020`. No shared fixture was reset.

## Executed gates

| Subsystem | Command | Result |
|---|---|---|
| Rust | `cargo fmt --all -- --check` | PASS |
| Rust | `cargo check --all-targets` | PASS |
| Rust | `cargo clippy --all-targets -- -D warnings` | PASS |
| Rust | `cargo test --lib` | 76/76 PASS |
| Rust API/security | `cargo test --test api_tests -- --test-threads=1` with owned Redis | 274/275 PASS; missing assertion-secret guard only |
| Rust focused | exact HMAC test with synthetic assertion secret | 1/1 PASS; 275 distinct regular API tests PASS across runs |
| Admission benchmarks | four explicit `--ignored` benchmark functions | 4/4 PASS, separately measured |
| Admin Web | `npx tsc --noEmit`, `npm run lint`, owned-fixture `npm test`, `npm run build` | PASS; 49/49 tests |
| Doctor Web | `npm run typecheck`, `npm run lint`, `npm test`, `npm run build` | PASS; 78/78 tests |
| Doctor Flutter | offline `flutter pub get`, `dart format --output=none --set-exit-if-changed .`, `flutter analyze`, `flutter test` | PASS; 231/231 tests |
| Patient Flutter | same commands in `apps/mobile_patient` | PASS; 229/229 tests |

The initial API invocation without `TEST_REDIS_URL` was rejected by its safety guard and is not counted as an application failure. The initial parallel Flutter invocation was discarded because Flutter’s startup lock serialized commands; the listed Flutter results are clean serial executions.

## Acceptance matrix

| Gate | State | Evidence |
|---|---|---|
| BACKEND_FORMAT / CHECK / CLIPPY | PASS | current commands above |
| BACKEND_LIB_TESTS | PASS | 76/76 |
| BACKEND_API_TESTS | PASS | 275 distinct regular tests across full and focused runs |
| BACKEND_FINANCIAL_TESTS | PASS | financial and idempotency cases in API suite |
| BACKEND_RLS_NEGATIVE_TESTS | PASS | RLS, ABAC and BOLA cases in API suite |
| ADMIN_WEB_TYPECHECK / LINT / TESTS / REFRESH / BUILD | PASS | 49 tests, five Redis refresh tests included |
| DOCTOR_WEB_TYPECHECK / LINT / TESTS / BUILD | PASS | 78 tests |
| PATIENT_FLUTTER_ANALYZE / FORMAT / TESTS | PASS | 0 analyzer issues, 229 tests |
| DOCTOR_FLUTTER_ANALYZE / FORMAT / TESTS / QUEUE_LIFECYCLE | PASS | 0 analyzer issues, 231 tests |
| ADMIN_TOTP_FULL_FLOW | PASS | synthetic HTTP/API security cases |
| BROWSER_TURNSTILE_SYNTHETIC | INCONCLUSIVE | helper and controlled-response tests pass; no real Siteverify/provider acceptance |
| ADMIN_BFF_HMAC / SESSION_ROTATION / DEVICE_PROOF_SYNTHETIC_HTTP | PASS | current API security cases |
| ANDROID_NATIVE_BUILD | BLOCKED | Gradle 9.3.1 distribution is not cached; wrapper download was stopped |
| ANDROID_KEYSTORE_RUNTIME | DEFERRED_BY_USER | requires physical device/runtime |
| RLS_AND_BOLA_NEGATIVE_TESTS | PASS | current serialized API suite |
| REDIS_OUTAGE_RECOVERY / DB_POOL_SATURATION | PASS | current API cases against owned fixtures |
| DISTRIBUTED_AUTH_LOAD | INCONCLUSIVE | retained 30,000-request run has unresolved transport errors |
| TRANSPORT_ERROR_ROOT_CAUSE | INCONCLUSIVE | see resilience report |
| AUTHENTICATED_CONSULTATION_UNDER_PRESSURE / UPLOAD_PRESSURE | NOT_RUN | no current mixed workload fixture was safely available |
| WEBHOOK_PRESSURE | INCONCLUSIVE | route/race coverage exists; no dedicated flood run |
| RECOVERY_STAMPEDE | PASS | current Web/Flutter recovery tests |
| TLS_CONFIG_REVIEW / MONITORING_REVIEW | PASS | source/configuration review only |
| ORIGIN_NETWORK_REVIEW / PRIVATE_DB_REDIS_REVIEW | INCONCLUSIVE | deployment topology not inspected live |
| BACKUP_RESTORE_DISPOSABLE | NOT_VERIFIED | no safe backup artifact/restore fixture was available |
| RELEASE_SIGNING_REVIEW | INCONCLUSIVE | source configuration reviewed; signed release build not rerun |
| REAL_AZURE_CONFIGURATION / REAL_CLOUDFLARE_ACCEPTANCE | NOT_RUN | requires operator access |

`TESTS_DISCOVERED=866`, `TESTS_EXECUTED=866`, `TESTS_PASSED=866`, `TESTS_FAILED=0`, `TESTS_IGNORED=4` (explicit benchmarks), and `TESTS_NOT_RUN=0` for the listed local application suites. These totals do not convert provider, native-device, deployment, or mixed-workload states into passes.

`ASSERTIONS_WEAKENED=0`, `TESTS_DELETED=0`, `SHARED_RESOURCES_MODIFIED=0`, `PRODUCTION_RESOURCES_MODIFIED=0`, and `NEW_CRITICAL_DEFECTS=0`.

## Verdict

`LOCAL_AUTOMATED_ACCEPTANCE=PASS`.

`SECURITY_INTEGRATION_ACCEPTANCE=PASS` for the synthetic local API and browser helper scope, with provider-backed browser acceptance pending.

`RESILIENCE_ACCEPTANCE=INCONCLUSIVE` because the distributed transport investigation and authenticated consultation mixed workload are incomplete.

`DEPLOYMENT_CONFIGURATION_READINESS=INCONCLUSIVE`.

`PRODUCTION_LAUNCH_ACCEPTANCE=NOT_IMPLEMENTED` pending Azure/Cloudflare configuration, real payment provider integration, Azure Blob compatibility, backup restore evidence, native device runtime, and Agora live consultation acceptance.
