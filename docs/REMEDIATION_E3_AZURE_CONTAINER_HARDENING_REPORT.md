# Remediation E3 — Azure and Container Security Hardening

Status: **GREEN**

Scope: application-level Azure Blob Storage authentication controls, secret
redaction in `AppConfig`'s diagnostic formatting, and backend Docker image
hardening (non-root execution, base-image digest pinning). No Azure cloud
resources were created, inspected, or modified. No Git state was staged,
committed, pushed, reset, or cleaned. No SQL migrations, Flutter apps,
financial/clinical/notification behavior, or unrelated modules were touched.

## 1. Confirmed original findings

Phase 1 inspection of `services/backend/src/config.rs`,
`services/backend/Dockerfile`, `docs/AZURE_BLOB_SETUP.md`, the Docker Compose
definitions, and existing configuration tests confirmed three real,
independently verifiable gaps (a fourth candidate — other configuration
structs sharing `AppConfig`'s secret-disclosure risk — was investigated and
found *not* to exist; see §3):

1. **No gate on `AZURE_STORAGE_AUTH_MODE=account_key` in staging/production.**
   `validate_storage_configuration`'s `"account_key"` arm only checked that
   `AZURE_STORAGE_ACCOUNT_KEY` was present and `AZURE_STORAGE_CLIENT_ID` was
   absent — it accepted a long-lived, full-account-scope shared credential in
   a deployed environment exactly as readily as in development, with no
   separate acknowledgment required.
2. **`AppConfig` derived `Debug` while holding plaintext credentials.**
   `#[derive(Clone, Debug)]` on `AppConfig` (`config.rs:22`, pre-change) meant
   any `{:?}`/`{cfg:?}` formatting — a stray debug log, an assertion failure
   message, a panic payload — would print database role passwords, the JWT
   private key, the TOTP encryption key, the Agora certificate, webhook HMAC
   secrets, storage access/secret keys, the Azure account key, the erasure
   journal's HMAC/anchor keys, the metrics scrape token, and the Azure email
   and FCM credentials in full.
3. **Backend Docker image ran as root with unpinned base image tags.**
   `services/backend/Dockerfile`'s runtime stage had no `USER` directive (the
   container ran as root), and both `FROM rust:1-bookworm` and
   `FROM debian:bookworm-slim` referenced mutable tags rather than immutable
   digests.

## 2. Exact changed files

| File | Change |
|---|---|
| `services/backend/src/config.rs` | Added `azure_storage_account_key_risk_acknowledged: bool` field + env parsing; added the production/staging account-key gate in `validate_storage_configuration`; replaced `derive(Debug)` with a hand-written redacting `Debug` impl; added `remediation_e3_tests` module (7 unit tests). |
| `services/backend/Dockerfile` | Pinned both base images to verified digests; added a dedicated non-root `hellodoctor` user/group (UID/GID 10001); `COPY --chown` for the binary and migrations only; added `USER hellodoctor:hellodoctor`; documented the safe digest-update procedure. |
| `services/backend/.env.example` | Documented `AZURE_STORAGE_ACCOUNT_KEY_RISK_ACKNOWLEDGED` (default `false`). |
| `.env.example` | Same, for the root template. |
| `docs/AZURE_BLOB_SETUP.md` | Documented the new gate, the operational risk the acknowledgment accepts, and what it explicitly does *not* grant (no Azure RBAC, no bypass of other checks). |
| `services/backend/tests/api_tests.rs` | Added the new field to the `test_config()`/`deployed_config()` fixtures (the latter now explicitly acknowledges account-key mode, matching its existing deliberate use of that mode); added `azure_account_key_mode_requires_explicit_production_acknowledgment` integration test. |

No other files were changed. (The working tree already carried a large body
of unrelated, pre-existing uncommitted work from earlier remediation
milestones — A–D, E1, E2 — which this change was layered on top of, never
staged, and never touched.)

## 3. Azure authentication decision matrix

Gate added in `validate_storage_configuration`'s `"account_key"` arm
(`config.rs`):

| `AZURE_STORAGE_AUTH_MODE` | Environment | `AZURE_STORAGE_ACCOUNT_KEY_RISK_ACKNOWLEDGED` | Result |
|---|---|---|---|
| `managed_identity` | any | n/a (flag has no effect on this mode) | Allowed (unchanged, already the default) |
| `azurite` | development/test only | n/a | Allowed (unchanged; `AZURE_STORAGE_USE_EMULATOR=true` still required) |
| `account_key` | development/test | `false` (default) | **Allowed** — existing local/Azurite-adjacent workflows are unaffected |
| `account_key` | staging/production | `false` (default) | **Rejected** — fails closed with a named error pointing at `docs/AZURE_BLOB_SETUP.md` |
| `account_key` | staging/production | `true` | Allowed, subject to every other existing Azure check (container names, encryption key, endpoint/HTTPS, emulator restrictions) |

The flag only widens this one gate. It does not relax container-name
validation, the 32-byte `STORAGE_ENCRYPTION_KEY_B64` check, endpoint/HTTPS
rules, the Azurite/emulator restriction, or any other check in
`validate_storage_configuration` or `validate_public_exposure` — verified by
`acknowledgment_flag_does_not_bypass_unrelated_azure_checks` (unit test) and
exercised end-to-end against the full `deployed_config()` production fixture
in `azure_account_key_mode_requires_explicit_production_acknowledgment`
(integration test). The flag is an **application-level configuration
acknowledgment only** — it does not configure, grant, or verify any Azure
RBAC role, firewall rule, or Conditional Access policy; those remain the
operator's responsibility and are documented as such in
`docs/AZURE_BLOB_SETUP.md`.

**Other configuration types investigated for the same disclosure risk
(Phase 3):** the only other struct resembling a server-wide configuration
aggregate is `services/dr_reconciliation.rs`'s `JournalConfig`, which holds
raw HMAC/anchor key bytes — it has no `Debug` derive at all, so it already
carried no disclosure risk. No change was needed there. (Separately, several
API request DTOs — e.g. `PatientLoginRequest`, `DoctorLoginRequest`,
`AdminLoginRequest` — derive `Debug` and contain a plaintext `password`
field; this is a different class of object (per-request DTO, not persistent
server configuration) and is called out under Known Limitations rather than
changed, since Phase 3 scoped this remediation to "configuration types.")

## 4. Secret-redaction test evidence

`AppConfig`'s `Debug` impl now lists every field explicitly; credential/key
fields render as `<redacted>` or `<unset>` instead of their real value, and a
field added to the struct in the future is simply absent from `{:?}` output
until someone deliberately adds it to the impl (fail-safe-by-omission,
instead of `derive(Debug)`'s fail-open default of printing anything new).

Test `config::remediation_e3_tests::debug_format_redacts_all_known_secret_fields`
constructs a full `AppConfig` with synthetic, recognizable secret markers
(`SYNTH_API_PW_111111`, `SYNTH_JWT_PRIVATE_777777`, `SYNTH_AZURE_ACCOUNT_KEY_GGGGGG`,
etc. — 15 distinct markers covering every redacted field, plus the derived
32-byte storage encryption key) and asserts none of them appear anywhere in
`format!("{cfg:?}")`, while confirming non-secret diagnostic fields (JWT
*public* key, Agora app ID, environment name) remain visible. Result: **pass**.

## 5. Docker runtime identity verification

Docker was available; a disposable image (`hellodoctor-e3-test`) was built
from the hardened Dockerfile and verified, then removed:

```
$ docker run --rm hellodoctor-e3-test id
uid=10001(hellodoctor) gid=10001(hellodoctor) groups=10001(hellodoctor)

$ docker inspect hellodoctor-e3-test --format 'User={{.Config.User}}'
User=hellodoctor:hellodoctor

$ docker run --rm hellodoctor-e3-test sh -c "ls -lan /app"
-rwxr-xr-x 1 10001 10001 37550792 ... hellodoctor-backend
drwxr-xr-x 2 10001 10001    20480 ... migrations

$ docker run --rm hellodoctor-e3-test sh -c "touch /app/should_fail"
touch: cannot touch '/app/should_fail': Permission denied   # confirms no excess write access

$ docker run --rm hellodoctor-e3-test
thread 'main' (1) panicked at src/config.rs:...:
FATAL: ENVIRONMENT must be explicitly set to development, test, staging, or production.
```

The last line confirms the application starts, loads `libssl`/`ca-certificates`,
and fails at the same pre-existing application-level configuration gate it
always did — not an OS permission error — proving startup behavior is
unchanged under the new non-root identity. Only the binary and `migrations/`
are `chown`'d to `hellodoctor:hellodoctor`; `/app` itself remains root-owned
and is not writable by the app user (no blanket filesystem ownership grant).
Network exposure (`EXPOSE 8080`, no published ports added) is unchanged.

The stock `debian:bookworm-slim` base image ships several setuid binaries
(`su`, `mount`, `passwd`, etc.) — these are unrelated to this change (present
in the base image before and after) and out of scope for a Dockerfile-level
non-root hardening task; a future base-image attack-surface reduction
(e.g. `distroless`) is noted under Known Limitations.

## 6. Container image pinning status

Both base images were pulled live and pinned by digest, verified for
`linux/amd64` (the host's actual platform):

```
rust:1-bookworm@sha256:9181b56c2a21ab1d5e73cffe04fc762426d30aa7fac3c91141dd033f49a7fc8b
debian:bookworm-slim@sha256:99c0b25c46d0214d5a2091092e7908907dcaecd6a962cde0ac700acee222b6f3
```

Both digests were obtained directly from `docker pull --platform linux/amd64 <image>:<tag>`
followed by `docker inspect <image>:<tag> --format '{{index .RepoDigests 0}}'`
against Docker Hub during this session — not invented or reused from another
architecture/variant. The Dockerfile documents the same safe update
procedure inline: re-pull with the explicit platform flag, re-read the digest
Docker Hub actually serves, confirm the upstream changelog, rebuild, run the
full Phase 6 regression suite, and only then replace the pinned value.

## 7. Test results and exit codes

| Check | Command | Result |
|---|---|---|
| Format | `cargo fmt --check` | Clean (exit 0) |
| Lint | `cargo clippy --all-targets --locked -- -D warnings` | Clean, zero warnings (exit 0) |
| Compilation | `cargo build --lib`, `cargo build --example config_preflight` | Success, zero errors |
| Library tests | `cargo test --lib` | **115 passed**, 0 failed (108 pre-existing + 7 new config tests) |
| Integration suite | `cargo test --test api_tests --locked -- --test-threads=1` (against a disposable, explicitly-owned Redis fixture per the suite's own safety convention — the shared dev Redis was never used) | **453 passed**, 0 failed, **4 ignored** (the intentionally preserved capacity benchmarks) — the 452-test Remediation E1 baseline plus the one new `azure_account_key_mode_requires_explicit_production_acknowledgment` test |
| Dependency audit | `cargo audit --deny warnings --ignore RUSTSEC-2023-0071` (same documented ignore as `security-scan.yml`, re-verified still applicable) | Clean (exit 0), no new advisories |
| Docker build & runtime | `docker build` + `docker run` against the disposable test image | Success; see §5 |

An initial integration run under this session surfaced 442 failures; root
cause was this session's own test-harness setup (the required
`TEST_REDIS_URL`/`TEST_REDIS_FIXTURE_ID` disposable-Redis fixture had not yet
been started), not a code regression — confirmed by immediately rerunning a
single failing test, which failed with `TEST_REDIS_URL must select an
explicitly owned disposable Redis fixture: NotPresent`. A disposable
`redis:7-alpine` container on a non-default port (`16380`) was started with
the required `hd:test-fixture:owner` marker, per the suite's own existing
safety design and per this task's explicit instruction not to use the shared
development Redis. The shared dev Postgres/Redis containers
(`hellodoctor-postgres`, `hellodoctor-redis`) were never touched; the
integration suite uses `#[sqlx::test]`'s own disposable, freshly-migrated,
per-test databases.

A second rerun (with the fixture correctly configured) surfaced exactly 2
failures, both caused by this remediation's own new account-key gate firing
against the project's existing `deployed_config()` test fixture — which
deliberately models `account_key` mode but had not yet been updated to set
the new acknowledgment flag. Fixed by adding
`config.azure_storage_account_key_risk_acknowledged = true;` to that one
fixture (consistent with its stated intent of modeling an already-reviewed
production deployment), confirmed by rerunning both affected tests
individually, then the full suite, with the clean 453-passed result above.

## 8. Known limitations

- Azurite/emulator and local development behavior were verified only at the
  configuration-validation level (unit/integration tests), not against a
  live Azurite container, since no finding in this remediation touched that
  path.
- The non-root Docker image was verified for identity, filesystem ownership,
  startup, and dynamic-library loading — not against a live Postgres/Redis
  stack inside Docker Compose, to avoid connecting a disposable test
  container to the shared, long-running dev database (see the Migration
  discipline note in `CLAUDE.md`).
- `debian:bookworm-slim`'s stock setuid binaries (`su`, `mount`, `passwd`,
  etc.) are unrelated to this change and were not removed; a stricter base
  (e.g. `gcr.io/distroless/cc-debian12`) would reduce this further but is a
  larger change than this remediation's scope.
- Login/registration request DTOs (`PatientLoginRequest` et al.) derive
  `Debug` over a plaintext `password` field. This is a distinct class of
  risk from `AppConfig` (a per-request DTO vs. persistent server
  configuration) and was out of this remediation's "configuration types"
  scope; flagged here for a future, separately-scoped pass.
- No GitHub Actions workflow currently builds the Dockerfile (confirmed via
  `grep -i docker .github/workflows/*.yml`), so this change has zero CI
  blast radius today; if a CI image-build job is added later, it should pull
  through the same digest-pinned `FROM` lines unchanged.

## 9. Azure production requirements that cannot be verified locally

This remediation verifies **application configuration security** only. It
does not and cannot verify:

- Actual Azure RBAC role assignments (e.g. `Storage Blob Data Contributor`)
  on the real storage account or containers.
- Storage account firewall rules, private endpoint policy, or network
  isolation.
- Managed identity provisioning or its actual Azure AD/Entra binding.
- Account-key rotation processes or secret-manager custody in a real
  deployment.
- Conditional Access policies or any other Azure AD governance control.

No Azure cloud resources were created, modified, or inspected in this
session. `AZURE_STORAGE_ACCOUNT_KEY_RISK_ACKNOWLEDGED=true` is an
application-level acknowledgment only, as stated in §3 and in
`docs/AZURE_BLOB_SETUP.md`; it carries no Azure-side authorization effect by
itself.

## 10. Classification

**GREEN.** All four acceptance conditions are met:

- Production/staging Azure account-key usage is explicitly controlled
  (fail-closed default, tested both ways) — §3, §4 unit/integration tests.
- Secret-bearing configuration cannot leak through ordinary `Debug`
  formatting — §4, regression-tested against synthetic secrets.
- The hardened backend image builds and runs as non-root, with startup
  behavior and filesystem least-privilege verified — §5.
- Base image provenance is pinned to live-verified digests for the correct
  platform, with the safe update procedure documented inline — §6.
- The full regression suite (format, lint, build, 115 library tests, 453
  integration tests, dependency audit, Docker build/runtime) passes cleanly
  — §7.

No remediation beyond E3 was started. No destructive, Git, or Azure
deployment action was taken.
