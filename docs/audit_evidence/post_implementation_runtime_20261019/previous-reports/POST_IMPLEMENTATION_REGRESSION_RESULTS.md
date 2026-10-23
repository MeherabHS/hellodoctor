# Post-implementation regression results — 19 October 2026

**Runtime regression gate: BLOCKED_ENVIRONMENT. Application test bodies executed: 0. No application build/typecheck passed in this campaign.** Parser successes below are deliberately separate.

Revision: `main`, `dca9b3c5fcdb6303d31abbb77d3a1fd4ac1f879f`, dirty on entry. [Baseline](audit_evidence/post_implementation_20261019/baseline.json) identifies the actual scoped working tree. The earlier implementation/evidence files were retained.

## Isolation decision

Fresh command: `docker version --format "{{json .Server}}"`, repository root. Exit 1, approximately 0.162 seconds; Docker's Linux-engine named pipe does not exist. [Exact sanitized output](audit_evidence/post_implementation_20261019/isolation-probe.json).

No functioning alternative was demonstrated that enforces no external network, an empty allowlisted environment, no credentials/shared services, read-only source/toolchain, scratch-only writes, CPU/RAM/process/file/disk limits and a wall deadline. Installed host Cargo/Node/Flutter/Java tools do not meet those properties. The [security skill](../.agents/skills/security-audit/SKILL.md) says **“If every control cannot be enforced, do not execute target code.”** The user additionally prohibited starting Docker/host services, installing dependencies/images, or changing host configuration. Those boundaries were preserved.

## Fresh source-only checks

Trusted stock parsers read explicit source paths without importing/evaluating target modules. Child environments were cleared and populated only with Windows runtime paths and parser-local HOME/temp values; Dart also received CI/analytics-suppression values. These were source inspections, not sandboxed target execution. No package manager, build script, macro, test function or provider was executed.

| Check | Exact command shape / scope | Exit | Observed duration | Meaning |
| --- | --- | ---: | ---: | --- |
| Rust source formatting | Installed `rustfmt.exe --config-path <evidence>/source-rustfmt.toml` and six corrected Rust files | 0 | 2.144 s | Formatting only |
| Rust parse/format check | Same stock binary, `--check --config-path <evidence>/source-rustfmt.toml`, thirteen explicit Rust inputs | 0 | 0.331 s | Syntax/format only; no compilation |
| TypeScript parsing | Installed `node.exe --experimental-strip-types --check <file>`, ten separate invocations | 0 each | 0.056–0.312 s each | No imports, test bodies, TSX rendering or typechecking |
| Dart source format check | Installed SDK `dart.exe format --output=none --set-exit-if-changed <eight explicit files>` | 0 | 0.231 s | Eight parsed, zero changed; no Flutter analysis/tests |
| Python harness parsing | `python.exe -I -S -c <ast.parse-only program>` for both resilience Python files | 0 | 0.071 s | Harness source parsed; harness not run |
| Focused whitespace | `git -c core.safecrlf=false diff --check -- services/backend/src/rate_limit.rs services/backend/src/services/auth_abuse.rs services/backend/src/telemetry/mod.rs services/backend/src/api/router.rs services/backend/src/api/v1/webhook_handlers.rs services/backend/tests/api_tests.rs` | 0 | See evidence | Existing Git line-ending normalization retained |
| Migration filename/hash inspection | Parent-side enumeration and SHA384 hashing of 62 up migrations | Completed | Not a runtime benchmark | 62 unique ordered versions; all down pairs present; no DB comparison |

Exact executable paths, argument arrays, working directories, output, durations and environment policy are in [source-checks.json](audit_evidence/post_implementation_20261019/source-checks.json), [dart-source-check.json](audit_evidence/post_implementation_20261019/dart-source-check.json), [whitespace-check-final.json](audit_evidence/post_implementation_20261019/whitespace-check-final.json), and [migration-inventory.json](audit_evidence/post_implementation_20261019/migration-inventory.json).

Rust inputs: the six corrected files listed in the whitespace command, plus `admission.rs`, `crypto.rs`, `services/upload_security_service.rs`, `services/device_proof.rs`, `services/admin_totp.rs`, `api/middleware/web_auth.rs` and `tests/economic_idempotency.rs` under `services/backend`. The parser emitted a warning that `skip_children=true` is nightly-only; it still exited zero. The final baseline comparison checks which files actually changed.

TypeScript inputs: Admin `src/lib/{turnstile,web-assertion,polling}.ts`, `tests/{turnstile,web-assertion,polling}.test.ts`; Doctor `src/lib/{turnstile,schedule-sync}.ts`, `tests/{turnstile,schedule-sync}.test.ts`. The ten parser invocations do not execute any of those test cases.

Dart inputs: each mobile application's `lib/core/network/{api_client,device_proof,screen_refresh}.dart` and `test/recovery_stampede_test.dart`. Formatter warnings report unresolved `package:flutter_lints/flutter.yaml`; this is not a successful dependency-resolution or analysis result.

Parent-side evidence collection initially used `Select-Object` incorrectly on ordered dictionaries; migration metadata was recomputed from retained JSON with explicit failure handling. An initial whitespace invocation stopped on a PowerShell native CRLF warning; another invocation with `core.autocrlf=false` misclassified existing CRLF bytes as trailing whitespace (exit 2). That diagnostic remains in `whitespace-check.json`. The final command uses the repository's normal line-ending behavior and passes; no global Git setting or source-wide line-ending conversion was made. These are collector corrections, not swallowed application-test failures.

## Actual project regression commands and status

Every row below is **NOT_RUN / BLOCKED_ENVIRONMENT**. Exit status, duration and passed/failed/skipped counts are **not available**, rather than fabricated zeros. Run inside the prepared fixture only. Add `--offline --locked` to Cargo commands to prevent resolution/fetching or lockfile drift.

| Working directory | Command | Coverage / qualification |
| --- | --- | --- |
| `services/backend` | `cargo fmt --all -- --check` | Complete workspace formatting gate not run; explicit stock-parser checks above are narrower |
| `services/backend` | `cargo check --offline --locked --all-targets` | Includes tests/examples compilation; not run |
| `services/backend` | `cargo clippy --offline --locked --all-targets -- -D warnings` | Initial warnings must be classified; do not suppress new warnings to pass |
| `services/backend` | `cargo test --offline --locked --lib` | Units, pure proof/HMAC/TOTP/admission, worker/content tests |
| `services/backend` | `cargo test --offline --locked --test api_tests -- --test-threads=1` | Existing HTTP/RLS/BOLA/financial/session/migration regressions and new cases |
| `services/backend` | `cargo test --offline --locked --test economic_idempotency -- --test-threads=1` | Separate economic binary; existing CI does not explicitly run it |
| `apps/admin-web` | `node node_modules/typescript/bin/tsc --noEmit --incremental false` | Installed local compiler entrypoint; package has no `typecheck` script; target dependency code execution is blocked |
| `apps/admin-web` | `npm run lint`; `npm test`; `npm run build` | Actual scripts: ESLint, Node TS test runner, Next build; all unrun |
| `apps/doctor-web` | `npm run typecheck`; `npm run lint`; `npm run format:check`; `npm test`; `npm run build` | Actual scripts; lint/format invoke repository code and are not source-only parser passes |
| `apps/mobile_patient` | `flutter analyze --no-pub`; `flutter test --no-pub` | SDK caches/plugins/package config must already be prepared; no pub get |
| `apps/mobile` | `flutter analyze --no-pub`; `flutter test --no-pub` | Same restriction; Doctor application |
| Either mobile application | `flutter build apk --debug --no-pub --dart-define=API_BASE_URL=https://fixture.invalid/api/v1` | Compilation-only candidate in the isolated offline Android toolchain; no device, production credentials or release signing. All Gradle artifacts must already be cached and offline policy enforced |
| Repository root | `./services/backend/scripts/resilience/Invoke-IsolatedSql.ps1` | Existing no-pull, no-network disposable SQL runner; daemon unavailable, not rerun |

Separate commands in one cell are separate runs, not an instruction to execute an unbounded chain. No release artifact was built. Both checked-in release configurations currently use debug signing; production distribution must use separately managed release signing. The Doctor field-test script auto-discovers a LAN IP and enables insecure/development options, so it was **not used** as an acceptance runner.

Existing CI uses `cargo test --lib --locked` and serialized `api_tests`, but permits network/provisioning and service ports. It is not proof that this skill's offline isolation requirements are met, nor evidence that the current dirty tree passed CI.

## Corrections and regression integrity

Four regression functions were added: three SQLx API integration cases and one Rust metric-label unit case. Two existing API cases were strengthened: healthy readiness gains a Redis assertion; callback replay retains the same reference and adds wrong-reference rejection plus economic observations. All are **authored, NOT_RUN**.

| Correction | Original failure evidence | Violated invariant | Revalidation |
| --- | --- | --- | --- |
| Atomic OTP reservation | Source previously called global/cooldown/rolling/daily INCR independently | Recipient-denied delivery must not consume unrelated global capacity | Source independently reviewed, Rust parse passed; integration blocked |
| Finite method labels | Raw custom method text entered persistent metrics | Caller variation must not create unlimited metric label values | Pure test authored; parser passed; runtime blocked |
| Redis-aware readiness | DB-only success while Redis-required admission could fail | Ready must reflect dependencies needed to serve ordinary requests | Positive test retained; fixture Redis-loss case authored; runtime blocked |
| Honest webhook response | Fixed success text contradicted expired/cancelled authoritative status | Response must not claim consultation activation that did not happen | Expired/replayed synthetic callback integration authored; runtime blocked |
| Correct replay fixture | Different event test changed both event ID and immutable payment reference | A valid same-payment retry must retain provider identity | Original success assertions retained; invalid-reference 409 assertions added; runtime blocked |

The OTP global test uses 100 distinct contenders distributed across eight workers, then four extra overflow checks. The cooldown test uses 96 calls across eight workers plus four sequential calls. It does not send emails or execute a failing synthetic email provider. TTL expiry, Redis failure and actual failed dispatch remain separate targets. The expiry webhook test models an already-expired booking; it does not create a real provider event or prove expiration interleaving.

For **this campaign**, patch review and independent source review establish:

```text
EXISTING_TESTS_DELETED=0
EXISTING_ASSERTIONS_DELETED=0
TEST_ASSERTIONS_WEAKENED=0
TEST_SKIP_IGNORE_ADDED=0
TEST_FAILURES_SWALLOWED=0
```

This does not certify unrelated pre-existing dirty test changes. [Final source/integrity evidence](audit_evidence/post_implementation_20261019/final-source-integrity.json) lists changed hashes and unchanged test files. No middleware was disabled, limits relaxed, pool enlarged, applied migration rewritten, or dependency added.

## Safe continuation procedure

An operator must first provision an approved disposable VM/container toolchain **outside this campaign**. Do not start shared Docker Desktop or install packages as a hidden prerequisite.

1. Provide the reviewed dirty-tree snapshot, including untracked implementation/test files. Prepare it parent-side from an explicit non-secret file allowlist. Exclude every real `.env`, token/key/signing file, host home, socket, shared volume, prior PHI evidence and production artifact. Verify source hashes. A `HEAD`-only archive is insufficient for this worktree.
2. Mount toolchains and dependency caches read-only. All Cargo crates, Node packages, Flutter/pub SDK plugins, Java/Android/Gradle artifacts must already exist. Source needing in-place writes must have a parent-prepared disposable copy inside assigned scratch. No credential-based caches or real Android signing material.
3. Enforce no external network at OS level. Use only the fixture's loopback namespace for PostgreSQL, Redis, backend/BFF, synthetic providers and observer. No host-network mode, published/shared DB ports or host control sockets. Use one fresh PG cluster and one disposable Redis instance; serialize capability-role tests across all binaries.
4. Enforce a read-only root/toolchain/source and scratch-only writes; a concrete initial envelope is 2 CPU, 2 GiB RAM/no additional swap, 128 processes, 128 open files, 64 MiB individual file size, 4 GiB scratch and a 1,800-second wall deadline with five-second kill grace. This is an isolation envelope, not measured application capacity. A build exceeding it is a resource-blocked run; changes require explicit owner review, not silent expansion.
5. Start target children from an empty environment. Allow only tool paths, scratch HOME/temp/cache/output paths and synthetic configuration: fixture-local `DATABASE_URL`, `redis://127.0.0.1:6379/15`, fixed test-only encryption/role keys where needed, explicit environment and safe BFF placeholders. No inherited NODE_OPTIONS, production Azure/Turnstile/provider credentials or real enrollment keys. Keep `.env` absent; SQLx/tests otherwise can load it.
6. Run the exact project commands above one at a time; preserve exit status and sanitized output. Record discovered test counts, skip counts, wall duration, tool versions and source/fixture identities. Never execute these commands directly against the host/shared services. On failure, preserve it, classify it, apply a minimal correction and rerun only affected checks/dependencies.
7. After target processes terminate, retain only small, allowlisted, parent-verified regular result files under the skill's promotion rules. Never copy arbitrary scratch trees, secrets, tokens or build archives into reports.

The SQL-only runner already expresses a smaller concrete envelope (2 CPU, 512 MiB RAM/scratch, 64 processes, 180 seconds) and exact invocation. It does not provision the missing application toolchains or establish complete SQLx upgrade/checksum acceptance. HTTP and device acceptance still require their own supported fixtures.
