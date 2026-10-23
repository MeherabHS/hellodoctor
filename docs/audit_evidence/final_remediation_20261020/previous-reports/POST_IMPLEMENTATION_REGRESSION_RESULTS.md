# Post Implementation Regression Results — 20 October 2026

Updated from **executed local development verification**, 19–20 October 2026. This is not formal sandbox certification or production acceptance. The owner authorized local toolchains/disposable services and froze original application source, tests, assertions, configuration and the security skill. Only new fixture/evidence files and these reports were changed.

Branch `main`; commit `dca9b3c5fcdb6303d31abbb77d3a1fd4ac1f879f`; dirty working tree on entry. [Entry baseline](audit_evidence/post_implementation_runtime_20261019/baseline.json) and [final preservation check](audit_evidence/post_implementation_runtime_20261019/source-freeze-final.json): **539 original files checked, zero changes, skill unchanged**. Web and Flutter manifests independently checked 132 and 425 original files (overlapping inventories, not additive). Earlier reports remain in [the historical archive](audit_evidence/post_implementation_runtime_20261019/previous-reports/POST_IMPLEMENTATION_REGRESSION_RESULTS.md); earlier source corrections are not new fixes in this frozen campaign.

## Build and regression gate

| Suite / command | Exit | Tests / observation | Duration | Evidence |
| --- | --- | --- | --- | --- |
| `cargo fmt --all -- --check` | 0 | PASS | 2.266s | [backend-format.json](audit_evidence/post_implementation_runtime_20261019/backend-format.json) |
| `cargo check --offline --locked --all-targets` | 0 | PASS with existing LLVM MinGW on clean PATH | 376.965s | [backend-check-toolpath.json](audit_evidence/post_implementation_runtime_20261019/backend-check-toolpath.json) |
| `cargo clippy --offline --locked --all-targets -- -D warnings` | 101 | FAIL: two unique diagnostics | 15.337s | [backend-clippy.stderr.log](audit_evidence/post_implementation_runtime_20261019/backend-clippy.stderr.log) |
| `cargo test --offline --locked --lib -- --test-threads=1` | 0 | 69 passed, 0 failed/ignored | 518.594s including compile; tests 230.27s | [backend-unit.stdout.log](audit_evidence/post_implementation_runtime_20261019/backend-unit.stdout.log) |
| `cargo build --offline --locked --bin hellodoctor-backend` | 0 | PASS; fresh binary used for HTTP | 78.871s | [backend-build.json](audit_evidence/post_implementation_runtime_20261019/backend-build.json) |
| `cargo test --offline --locked --test economic_idempotency -- --test-threads=1` | 0 | 1 passed after fixture prerequisite correction | 17.766s; test 3.09s | [backend-economic-prerequisite.stdout.log](audit_evidence/post_implementation_runtime_20261019/backend-economic-prerequisite.stdout.log) |
| `cargo test --offline --locked --test api_tests -- --list` | 0 | Compiled/listed 264 tests; no bodies | 93.859s | [backend-api-list.stdout.log](audit_evidence/post_implementation_runtime_20261019/backend-api-list.stdout.log) |
| `cargo test --offline --locked --test api_tests test_enum_add_value_migration_reversibility_limitation_pattern -- --exact --test-threads=1` | 0 | 1 passed; 263 filtered | 1.159s; test .30s | [backend-api-enum.stdout.log](audit_evidence/post_implementation_runtime_20261019/backend-api-enum.stdout.log) |
| Raw SQL harness | 0 | Six phases, 600 operations, eight clients max; independent observer passes | 8.749s | [sql-20261019T200439272.json](audit_evidence/post_implementation_runtime_20261019/sql-20261019T200439272.json) |
| Production OTP Lua fixture | 0 | Five cases; 207 reservations; eight workers max | 1.904s | [otp-lua-20261019T201022793.json](audit_evidence/post_implementation_runtime_20261019/otp-lua-20261019T201022793.json) |
| Admin Web typecheck / build | 0 / 0 | PASS / PASS | 6.151s / 32.818s | [web-summary.json](audit_evidence/post_implementation_runtime_20261019/web-summary.json) |
| Admin Web safe tests | 0 | 40 passed, 0 failed/skipped, eight files | .439s | [web-admin-tests-safe.stdout.log](audit_evidence/post_implementation_runtime_20261019/web-admin-tests-safe.stdout.log) |
| Admin Web ESLint | 1 | 3 errors, 9 warnings | 55.938s | [web-admin-lint.stdout.log](audit_evidence/post_implementation_runtime_20261019/web-admin-lint.stdout.log) |
| Doctor Web typecheck / package lint / build | 0 / 0 / 0 | PASS; package lint is custom, not ESLint | 5.307s / .086s / 41.954s | [web-summary.json](audit_evidence/post_implementation_runtime_20261019/web-summary.json) |
| Doctor Web tests | 0 | 78 passed, 0 failed/skipped, 16 files | .783s | [web-doctor-tests.stdout.log](audit_evidence/post_implementation_runtime_20261019/web-doctor-tests.stdout.log) |
| Patient Flutter tests | 0 | 229 passed, 0 failed/skipped | 59.055s | [flutter-mobile_patient-tests-complete-environment.json](audit_evidence/post_implementation_runtime_20261019/flutter-mobile_patient-tests-complete-environment.json) |
| Doctor Flutter tests | 1 | 228 passed, 1 failed, 0 skipped | 47.320s | [flutter-mobile-tests-complete-environment.json](audit_evidence/post_implementation_runtime_20261019/flutter-mobile-tests-complete-environment.json) |
| Doctor lifecycle reproduction | 1 | 1 passed / 1 failed; unchanged test | 6.432s | [flutter-mobile-queue-lifecycle-reproduction.json](audit_evidence/post_implementation_runtime_20261019/flutter-mobile-queue-lifecycle-reproduction.json) |
| Doctor focused auth/security/recovery | 0 | 15 passed; duplicates of full suite | 8.920s | [flutter-mobile-security-recovery-focused.json](audit_evidence/post_implementation_runtime_20261019/flutter-mobile-security-recovery-focused.json) |
| Both Flutter analyzers | 1 each | Four informational `curly_braces_in_flow_control_structures` findings each; no reported type errors | Per-command metadata below | [flutter-summary.json](audit_evidence/post_implementation_runtime_20261019/flutter-summary.json) |
| Both Dart format checks (no writes) | 1 each | Patient 127 files / 6 would change; Doctor 66 / 5 would change | Per-command metadata below | [flutter-summary.json](audit_evidence/post_implementation_runtime_20261019/flutter-summary.json) |

Both `flutter pub get --offline --enforce-lockfile` checks passed on disposable copies. Flutter 3.47.6 / Dart 3.13.5; Node 24.18.0; Rust/Cargo 1.99.0; Docker 29.7.2. Dependencies were cached; no images/software/dependencies were downloaded. Next versions were 16.4.0 Admin and 15.5.27 Doctor. Exact command arguments, working directories, timestamps, durations, exit codes and stdout/stderr are retained below.

Counts are not inflated by reruns: 647 distinct named tests were executed across Rust (71), web (118) and Flutter (458), giving **646 passed and one failed**. Focused reruns, 27 HTTP assertions, five Lua cases and 600 SQL operations are separate evidence categories. Five Admin refresh tests and 263 API tests were not run; they are not reported as test-runner skips. Analyzer/lint/build failures are independent gates, not hidden in the named test total.

## Failures retained without source changes

1. Strict Clippy: `services/backend/src/admission.rs:142` (`manual_clamp`) and `src/services/admin_totp.rs:84` (`unnecessary_map_or`), repeated for lib/lib-test builds. No lint allowance or source fix was added.
2. Admin ESLint: `src/app/page.tsx:2013:126`, `:161`, and `src/components/BotChallenge.tsx:57:42`, `react/no-unescaped-entities`. Nine warnings also remain.
3. Doctor Flutter `apps/mobile/test/queue_lifecycle_refresh_test.dart:73`: expected queueReads 2, observed 1 after resume. Reproduced unchanged. Source suggests the immediate expectation conflicts with intentional recovery jitter; no assertion was changed and no product bug/fix is claimed from that inference.
4. Both Flutter analyze/format checks returned nonzero. Formatting was checked using `--output=none`; no file was formatted in place.

## Fixture/tooling corrections, not application fixes

- Initial Cargo check failed because clean PATH omitted the already-installed MinGW `dlltool`; adding that existing toolchain path made the same command pass. The initial failed record remains.
- First SQLx economic test failed before its body: migration 20261003000001 references database `hellodoctor`, absent from the fresh cluster. Creating that new empty prerequisite database within the owned fixture allowed the unchanged test to pass. No shared DB or migration was changed.
- Admin Turbopack rejected a dependency junction outside the disposable project root. Copying the cached dependency directory into scratch allowed the stock production build to pass.
- Initial Flutter clean-environment attempts lacked existing Git/PowerShell/toolchain environment entries; completed-environment runs above are the authoritative executions. Earlier failures remain in the command log.
- The new OTP observer initially expected `redis-cli --raw` to print an `ERR ` prefix. Only that newly authored scratch observer's parsing was corrected; production Lua and all economic assertions were unchanged. The initial failure is retained.
- HTTP fault fixture initially lacked Docker on clean PATH. The first forwarder also lost its Redis listener under pause, confounding recovery/pool tests. Only the owned fixture was recreated with listener supervision; the unchanged backend then recovered. Initial failed/invalid fixture evidence is retained and does not count as an application failure or pass.
- A subsequent new SQL-lock fixture independently established the lock before offering traffic. This produced real blocked-query evidence, not true full-pool saturation.

## Suites not run and exact prerequisites

| Suite | Why not run | Safe execution prerequisite |
| --- | --- | --- |
| Other 263 API integration cases | Test helper hardcodes `redis://127.0.0.1:6379/15` and `FLUSHDB`; this is the shared Redis service. Environment override cannot redirect it. | Pre-provisioned isolated OS namespace/toolchain in which that exact localhost endpoint is disposable, or a separately authorized future test-fixture correction. No test mutation here. |
| Admin `tests/refresh.test.ts` (5 cases) | Hardcodes localhost Redis `/14` and probes it at module import. | Isolated namespace with fixture-owned localhost6379. Not a connection to shared Redis and not a transport mock. |
| Android compilation (both apps) | SDK36 exists, but cached Gradle 9.3.1 absent; AGP 9.1.0/Kotlin 2.4.0 cached artifacts not established. Wrapper would download before offline mode. | Approved prepopulated Gradle distribution/plugins/dependencies; no installation/download performed. |
| Physical Android / video E2E | DEFERRED_BY_USER. | A separately scheduled authorized stage; not a blocker for already-executed automated checks. |
| Browser/provider acceptance | Unit tests inject synthetic Siteverify; no real provider credentials or browser acceptance fixture. | Approved synthetic browser/route environment, then separately authorized provider/deployment acceptance. |

## Assertion preservation

```text
EXISTING_TESTS_DELETED=0
EXISTING_ASSERTIONS_DELETED=0
TEST_ASSERTIONS_WEAKENED=0
TEST_SKIP_IGNORE_ADDED=0
TEST_FAILURES_SWALLOWED=0
APPLICATION_SOURCE_CONFIG_TEST_EDITS=0
```

These values cover the frozen campaign only; unrelated pre-existing dirty work was preserved. The recorders can return a successful shell invocation while a child fails, so all decisions use the recorded **child exit code**, not the outer PowerShell exit status. New HTTP supervisors likewise report collected evidence without asserting that every scenario passed.

## Full recorded command index

Every linked JSON contains the exact executable, arguments, working directory, start time, child exit code, timeout result and duration; adjacent stdout/stderr logs retain sanitized diagnostics. Paths use disposable copies. The original SQL and OTP wrapper commands are recorded separately in the table above.

| Evidence record | Child exit | Seconds | Exact argument vector (after executable) |
| --- | --- | --- | --- |
| [backend-admission-focused.json](audit_evidence/post_implementation_runtime_20261019/backend-admission-focused.json) | 0 | 0.037 | `admission::tests --test-threads=1` |
| [backend-api-enum.json](audit_evidence/post_implementation_runtime_20261019/backend-api-enum.json) | 0 | 1.159 | `test --offline --locked --test api_tests test_enum_add_value_migration_reversibility_limitation_pattern -- --exact --test-threads=1` |
| [backend-api-list.json](audit_evidence/post_implementation_runtime_20261019/backend-api-list.json) | 0 | 93.859 | `test --offline --locked --test api_tests -- --list` |
| [backend-build.json](audit_evidence/post_implementation_runtime_20261019/backend-build.json) | 0 | 78.871 | `build --offline --locked --bin hellodoctor-backend` |
| [backend-check-toolpath.json](audit_evidence/post_implementation_runtime_20261019/backend-check-toolpath.json) | 0 | 376.965 | `check --offline --locked --all-targets` |
| [backend-check.json](audit_evidence/post_implementation_runtime_20261019/backend-check.json) | 101 | 20.764 | `check --offline --locked --all-targets` |
| [backend-clippy.json](audit_evidence/post_implementation_runtime_20261019/backend-clippy.json) | 101 | 15.337 | `clippy --offline --locked --all-targets -- -D warnings` |
| [backend-economic-prerequisite.json](audit_evidence/post_implementation_runtime_20261019/backend-economic-prerequisite.json) | 0 | 17.766 | `test --offline --locked --test economic_idempotency -- --test-threads=1` |
| [backend-economic.json](audit_evidence/post_implementation_runtime_20261019/backend-economic.json) | 101 | 77.394 | `test --offline --locked --test economic_idempotency -- --test-threads=1` |
| [backend-format.json](audit_evidence/post_implementation_runtime_20261019/backend-format.json) | 0 | 2.266 | `fmt --all -- --check` |
| [backend-proof-focused.json](audit_evidence/post_implementation_runtime_20261019/backend-proof-focused.json) | 0 | 0.033 | `device_proof::tests --test-threads=1` |
| [backend-unit-list.json](audit_evidence/post_implementation_runtime_20261019/backend-unit-list.json) | 0 | 0.033 | `--list` |
| [backend-unit.json](audit_evidence/post_implementation_runtime_20261019/backend-unit.json) | 0 | 518.594 | `test --offline --locked --lib -- --test-threads=1` |
| [flutter-mobile-analyze.json](audit_evidence/post_implementation_runtime_20261019/flutter-mobile-analyze.json) | 1 | 12.752 | `/d /c C:\Users\CUBE\.codex\tools\flutter\bin\flutter.bat --no-version-check analyze --no-pub` |
| [flutter-mobile-format.json](audit_evidence/post_implementation_runtime_20261019/flutter-mobile-format.json) | 1 | 1.948 | `format --output=none --set-exit-if-changed lib test` |
| [flutter-mobile-pub-offline.json](audit_evidence/post_implementation_runtime_20261019/flutter-mobile-pub-offline.json) | 0 | 6.552 | `/d /c C:\Users\CUBE\.codex\tools\flutter\bin\flutter.bat --no-version-check pub get --offline --enforce-lockfile` |
| [flutter-mobile-queue-lifecycle-reproduction.json](audit_evidence/post_implementation_runtime_20261019/flutter-mobile-queue-lifecycle-reproduction.json) | 1 | 6.432 | `/d /c C:\Users\CUBE\.codex\tools\flutter\bin\flutter.bat --no-version-check test --no-pub --concurrency=1 --reporter=json test/queue_lifecycle_refresh_test.dart --dart-define=API_BASE_URL=http://127.0.0.1:1/api/v1` |
| [flutter-mobile-security-recovery-focused.json](audit_evidence/post_implementation_runtime_20261019/flutter-mobile-security-recovery-focused.json) | 0 | 8.920 | `/d /c C:\Users\CUBE\.codex\tools\flutter\bin\flutter.bat --no-version-check test --no-pub --concurrency=2 --reporter=json test/auth_security_test.dart test/auth_state_test.dart test/recovery_stampede_test.dart --dart-define=API_BASE_URL=http://127.0.0.1:1/api/v1` |
| [flutter-mobile-tests-complete-environment.json](audit_evidence/post_implementation_runtime_20261019/flutter-mobile-tests-complete-environment.json) | 1 | 47.320 | `/d /c C:\Users\CUBE\.codex\tools\flutter\bin\flutter.bat --no-version-check test --no-pub --concurrency=2 --reporter=json --dart-define=API_BASE_URL=http://127.0.0.1:1/api/v1` |
| [flutter-mobile-tests.json](audit_evidence/post_implementation_runtime_20261019/flutter-mobile-tests.json) | 1 | 1.288 | `/d /c C:\Users\CUBE\.codex\tools\flutter\bin\flutter.bat --no-version-check test --no-pub --concurrency=2 --reporter=json --dart-define=API_BASE_URL=http://127.0.0.1:1/api/v1` |
| [flutter-mobile-toolchain-version.json](audit_evidence/post_implementation_runtime_20261019/flutter-mobile-toolchain-version.json) | 0 | 1.019 | `/d /c C:\Users\CUBE\.codex\tools\flutter\bin\flutter.bat --no-version-check --version` |
| [flutter-mobile_patient-analyze.json](audit_evidence/post_implementation_runtime_20261019/flutter-mobile_patient-analyze.json) | 1 | 42.634 | `/d /c C:\Users\CUBE\.codex\tools\flutter\bin\flutter.bat --no-version-check analyze --no-pub` |
| [flutter-mobile_patient-format.json](audit_evidence/post_implementation_runtime_20261019/flutter-mobile_patient-format.json) | 1 | 0.595 | `format --output=none --set-exit-if-changed lib test` |
| [flutter-mobile_patient-pub-offline-complete-path.json](audit_evidence/post_implementation_runtime_20261019/flutter-mobile_patient-pub-offline-complete-path.json) | 0 | 13.266 | `/d /c C:\Users\CUBE\.codex\tools\flutter\bin\flutter.bat --no-version-check pub get --offline --enforce-lockfile` |
| [flutter-mobile_patient-pub-offline.json](audit_evidence/post_implementation_runtime_20261019/flutter-mobile_patient-pub-offline.json) | 1 | 0.218 | `/d /c C:\Users\CUBE\.codex\tools\flutter\bin\flutter.bat --no-version-check pub get --offline --enforce-lockfile` |
| [flutter-mobile_patient-tests-complete-environment.json](audit_evidence/post_implementation_runtime_20261019/flutter-mobile_patient-tests-complete-environment.json) | 0 | 59.055 | `/d /c C:\Users\CUBE\.codex\tools\flutter\bin\flutter.bat --no-version-check test --no-pub --concurrency=2 --reporter=json --dart-define=API_BASE_URL=http://127.0.0.1:1/api/v1` |
| [flutter-mobile_patient-tests.json](audit_evidence/post_implementation_runtime_20261019/flutter-mobile_patient-tests.json) | 1 | 1.536 | `/d /c C:\Users\CUBE\.codex\tools\flutter\bin\flutter.bat --no-version-check test --no-pub --concurrency=2 --reporter=json --dart-define=API_BASE_URL=http://127.0.0.1:1/api/v1` |
| [flutter-version.json](audit_evidence/post_implementation_runtime_20261019/flutter-version.json) | 1 | 0.113 | `/d /c C:\Users\CUBE\.codex\tools\flutter\bin\flutter.bat --version --no-version-check` |
| [http-driver-first.json](audit_evidence/post_implementation_runtime_20261019/http-driver-first.json) | 0 | 119.741 | `-I -S scratch/acceptance-runtime-20261019/http-fixture/verify-http.py --binary scratch/acceptance-runtime-20261019/cargo-target/debug/hellodoctor-backend.exe --repo "C:\\Users\\CUBE\\Desktop\\helodoc old site"` |
| [http-fault-driver-repaired-forwarder.json](audit_evidence/post_implementation_runtime_20261019/http-fault-driver-repaired-forwarder.json) | 0 | 54.476 | `-I -S scratch/acceptance-runtime-20261019/http-fixture/verify-faults-retry.py` |
| [http-fault-driver-toolpath.json](audit_evidence/post_implementation_runtime_20261019/http-fault-driver-toolpath.json) | 0 | 63.561 | `-I -S scratch/acceptance-runtime-20261019/http-fixture/verify-faults.py` |
| [http-fault-driver.json](audit_evidence/post_implementation_runtime_20261019/http-fault-driver.json) | 1 | 0.181 | `-I -S scratch/acceptance-runtime-20261019/http-fixture/verify-faults.py` |
| [http-original-observer.json](audit_evidence/post_implementation_runtime_20261019/http-original-observer.json) | 0 | 0.113 | `-I -S services/backend/scripts/resilience/observe-local.py --before docs/audit_evidence/post_implementation_runtime_20261019/observer-before.json --after docs/audit_evidence/post_implementation_runtime_20261019/observer-after.json --attack docs/audit_evidence/post_implementation_runtime_20261019/observer-attack-classification.json --output docs/audit_evidence/post_implementation_runtime_20261019/observer-distributed-verdict.json` |
| [http-pressure-driver.json](audit_evidence/post_implementation_runtime_20261019/http-pressure-driver.json) | 1 | 41.903 | `-I -S scratch/acceptance-runtime-20261019/http-fixture/verify-pressure.py` |
| [http-pressure-independent-lock.json](audit_evidence/post_implementation_runtime_20261019/http-pressure-independent-lock.json) | 0 | 46.157 | `-I -S scratch/acceptance-runtime-20261019/http-fixture/verify-pressure-fixed.py` |
| [web-admin-build-copied-cache.json](audit_evidence/post_implementation_runtime_20261019/web-admin-build-copied-cache.json) | 0 | 32.818 | `--max-old-space-size=1536 node_modules/next/dist/bin/next build` |
| [web-admin-build.json](audit_evidence/post_implementation_runtime_20261019/web-admin-build.json) | 1 | 11.453 | `--max-old-space-size=1536 node_modules/next/dist/bin/next build` |
| [web-admin-lint.json](audit_evidence/post_implementation_runtime_20261019/web-admin-lint.json) | 1 | 55.938 | `--max-old-space-size=1536 node_modules/eslint/bin/eslint.js .` |
| [web-admin-tests-safe.json](audit_evidence/post_implementation_runtime_20261019/web-admin-tests-safe.json) | 0 | 0.439 | `--max-old-space-size=1536 --experimental-strip-types --test tests/api-traffic.test.ts tests/investigation.test.ts tests/polling.test.ts tests/professional-verification.test.ts tests/public-exposure.test.ts tests/schedule.test.ts tests/turnstile.test.ts tests/web-assertion.test.ts` |
| [web-admin-typecheck.json](audit_evidence/post_implementation_runtime_20261019/web-admin-typecheck.json) | 0 | 6.151 | `--max-old-space-size=1536 node_modules/typescript/bin/tsc --noEmit --incremental false` |
| [web-doctor-build.json](audit_evidence/post_implementation_runtime_20261019/web-doctor-build.json) | 0 | 41.954 | `--max-old-space-size=1536 node_modules/next/dist/bin/next build` |
| [web-doctor-lint.json](audit_evidence/post_implementation_runtime_20261019/web-doctor-lint.json) | 0 | 0.086 | `--max-old-space-size=1536 scripts/check-source.mjs` |
| [web-doctor-tests.json](audit_evidence/post_implementation_runtime_20261019/web-doctor-tests.json) | 0 | 0.783 | `--max-old-space-size=1536 --experimental-strip-types --test tests/*.test.ts` |
| [web-doctor-typecheck.json](audit_evidence/post_implementation_runtime_20261019/web-doctor-typecheck.json) | 0 | 5.307 | `--max-old-space-size=1536 node_modules/typescript/bin/tsc --noEmit --incremental false` |
| [web-node-version.json](audit_evidence/post_implementation_runtime_20261019/web-node-version.json) | 0 | 0.062 | `--version` |
