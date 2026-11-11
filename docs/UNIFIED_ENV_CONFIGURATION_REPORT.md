# Milestone 7A — Unified Environment Configuration Report

## Verdict

**GREEN — the unified configuration preparation is implemented and verified for this milestone.**

VM deployment and external-provider activation were intentionally out of scope and remain pending; they are not counted as application defects for this configuration milestone.

The repository remains on the existing uncommitted Remediation A–D worktree. No staging, commit, push, reset, cleanup, Azure provisioning, Firebase setup or deployment was performed.

## Inspection and mapping

The Rust backend already had one `AppConfig` source in `services/backend/src/config.rs`. It loads the backend dotenv file, validates explicit runtime modes, requires the five capability-role passwords, separates migration and request-serving pools, and fails closed for deployed HTTPS/FCM/proxy/metrics requirements. The existing backend template was `services/backend/.env.example`.

The Admin and Doctor Next.js applications use server-only BFF variables from their own runtime environment: `BACKEND_URL`, origin, proxy/auth assertion, Turnstile, Redis refresh coordination and build diagnostics. They do not need database, JWT, FCM, Agora or encryption secrets. Flutter reads only build-time `--dart-define` values such as `API_BASE_URL`, `ENVIRONMENT`, `ALLOW_INSECURE_HTTP`, `DEV_AUTH_OTP_ENABLED`, `DEV_PAYOUT_BINDING` and `APP_BUILD`.

There is no implemented Forgot Password/password-reset route or UI in the inspected backend or clients. Login, logout, MFA and authenticated password changes remain separate existing behavior.

## Implemented changes

- Added the root [`.env.example`](../.env.example) with documented server, web and Flutter build-time settings, ownership, defaults and restart/rebuild notes.
- Added `HELLODOCTOR_ENV_FILE` support to `AppConfig`. When set, the backend loads that explicit dotenv path before validation; otherwise local `services/backend/.env` discovery is unchanged.
- Added `PASSWORD_RESET_ENABLED` to the canonical backend config, defaulting to `false`.
- Password recovery remains unavailable. If an operator sets it to `true`, startup fails closed; the validator first requires the complete existing Azure email configuration and then rejects enablement because the reset workflow is not implemented and verified.
- Added the secret-free `config_preflight` example: `cargo run --example config_preflight --release` with `HELLODOCTOR_ENV_FILE=/etc/hellodoctor/hellodoctor.env` on a VM.
- Added [`deploy/docker-compose.vm.example.yml`](../deploy/docker-compose.vm.example.yml). The backend may receive the private file through `env_file`; Admin Web and Doctor Web receive an explicit allowlist only. No browser-facing `NEXT_PUBLIC_` secret is introduced.
- Extended the root `.dockerignore` to exclude local scratch, database dumps, uploads/documents, logs and other deployment-context data while preserving source migrations and configuration templates.
- Added [the VM operator guide](VM_CONFIGURATION_GUIDE.md).

## Secret isolation

Database superuser and capability-role credentials, JWT private keys, TOTP encryption, FCM service-account material, Agora certificates, HMAC/proxy secrets, storage keys and email access keys remain server-only. They are not mapped into web containers by the VM Compose example and are never Flutter build inputs.

The Admin/Doctor containers receive only their BFF/backend URL, origin, proxy/auth assertion where required, Turnstile values, Redis refresh URL and diagnostic build label. `TURNSTILE_SITE_KEY` is a widget identifier; its secret counterpart remains server-side.

The root template now exposes the implemented Azure Blob provider contract. Azure
still remains unconfigured in this worktree, but `STORAGE_PROVIDER=azure` is
validated explicitly and cannot silently fall back to S3 or local storage.

## Firebase, Agora and payment status

FCM HTTP v1 remains optional in development/test and required as a complete tuple outside those modes. Agora app credentials and token windows remain backend settings. No provider credential was created or activated.

No live payment-provider switch is currently consumed by the backend. Payment execution remains deferred by the existing provider-verification safeguards; only existing reservation-window and development-simulation settings are documented.

## VM packaging safeguards

The Compose example expects pre-built image names supplied through the private VM environment. It does not create an archive or transfer data. The root and backend Docker ignore rules exclude `.env` files, keys, tokens, build trees, scratch evidence, database dumps, local uploads/documents and logs. PostgreSQL/Redis volumes are never part of an application image context.

## Validation and test results

The new Rust configuration path has unit coverage for existing startup-mode validation plus the new fail-closed password-reset branch. The preflight example compiled and ran against the existing development configuration without printing secrets. The VM Compose template parsed successfully with `docker compose ... config --no-interpolate` (actual VM file loading remains pending).

Actual checks:

- `cargo fmt --all -- --check`: PASS.
- `cargo clippy --all-targets -- -D warnings`: PASS.
- `cargo check --all-targets`: PASS.
- `cargo test --lib`: **92 passed, 0 failed**.
- Serialized backend integration (`api_tests`, isolated fixtures): **445 passed, 0 failed, 4 ignored**, exit 0; source digest unchanged.
- Admin Web: typecheck PASS, lint PASS with one pre-existing warning, **126 tests passed**, production build PASS.
- Doctor Web: typecheck/lint/source-safety PASS, **85 tests passed**, production build PASS.
- `cargo audit --deny warnings --ignore RUSTSEC-2023-0071`: PASS.
- Azurite contract test: PASS when run against the owned `hd-7b-azurite` fixture.

Existing Remediation A–D behavior remains unchanged by this configuration-only addition.

## Values still required later

The operator must supply environment-specific database DSNs and role passwords, JWT/TOTP keys, proxy/HMAC secrets, HTTPS origins and trusted proxy IPs, FCM service-account material, Agora credentials, email credentials, monitoring token and the chosen pre-built image references. Azure Blob values remain pending the separately authorized adapter milestone.

## Limitations

This milestone does not provision a VM, configure systemd or Docker registries,
enable password recovery, configure live FCM/Agora/payment services, provision
Azure resources, or validate cloud RBAC. Actual Azure connectivity and managed
identity behavior remain external checks.
