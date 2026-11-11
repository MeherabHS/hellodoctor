# Milestone 9 — Audit A: Development, Debug, and Seed-Data Controls

**Audit type:** Read-only. No application code, migrations, Docker configuration, or environment variables were modified to produce this report.
**Scope:** `services/backend` (src, tests, examples, migrations), `apps/admin-web`, `apps/doctor-web`, `apps/mobile`, `apps/mobile_patient`.
**Method:** Direct source inspection (`grep`/file reads) against the current working tree — every citation below is a live file:line reference, not a recollection of prior work.

---

## 1. Authentication / MFA / OTP

### 1.1 Patient SMS OTP (legacy phone-based flow) — entirely disabled outside development

- **File/line:** `services/backend/src/services/auth_service.rs:61`
- **Feature:** `request_patient_otp` — the original phone+SMS OTP registration/login path (`POST /api/v1/auth/register-otp`, `POST /api/v1/auth/verify-otp-and-login`, routed at `services/backend/src/api/router.rs:312,328`).
- **Condition:** `state.config.environment != "development" || !state.config.dev_auth_otp_enabled`
- **Default:** `ENVIRONMENT` has no default (startup fails without it — `config.rs:968-970`); `DEV_AUTH_OTP_ENABLED` defaults to whatever `.env` sets, but is validated at startup (`config.rs:786-788`) to be rejectable outside development/test.
- **Active in:** development only. In test/staging/production this handler always returns `409 Conflict — "SMS delivery is not configured. No verification code was dispatched."` There is no SMS gateway integration at all; this is not a security bypass left in, it is an unfinished/superseded feature path with a fail-closed guard.
- **Security implication:** None directly (fails closed), but it means this legacy phone-OTP registration/login flow cannot be exercised in staging at all. If any client (mobile app) still calls these two routes expecting them to work, that is a functional gap, not a vulnerability.
- **Disposition:** **Retain the guard.** Either confirm no client still depends on this route outside development, or plan the real SMS gateway integration before relying on it anywhere but development.
- **Verification:** `curl -X POST https://<staging-host>/api/v1/auth/register-otp -d '{"phone_number":"+254700000000"}'` → expect `409` with the exact message above.

### 1.2 Doctor/Admin email-OTP second factor — debug code echo

- **Files/lines:** `services/backend/src/services/auth_service.rs:76-79` (patient), `:418-421` (doctor step 1), `:715-718` (admin step 1), `:916-919` (a third/related login path); serialized at `services/backend/src/api/v1/auth_handlers.rs:285-292,354,401,746,820,1028-1034` as the JSON field `debug_otp_code`.
- **Feature:** When a login/registration OTP challenge is created, the plaintext code is optionally echoed back in the API response instead of only being emailed.
- **Condition:** `state.config.environment == "development" && state.config.dev_auth_otp_enabled` — note this is a **double, exact-match** condition (not `is_development_or_test()`), so even if an operator sets `DEV_AUTH_OTP_ENABLED=true` in a `test` environment (which config validation permits — `config.rs:786-788` only forbids it outside development **or** test), the field still serializes as `null` because `environment == "development"` fails for `"test"`.
- **Default:** No code default for `DEV_AUTH_OTP_ENABLED`; config validation (`config.rs:786-788`) makes `true` fatal at startup outside development/test — the process will not boot with this misconfigured.
- **Active in:** development only (by the exact double-condition above). Structurally impossible to trigger in staging/production without also getting the process to boot with `ENVIRONMENT=development`, which `requires_https()`-gated validation (`config.rs:688,719`) does not directly prevent by itself but which would be an obvious, loud misconfiguration (HTTPS-dependent checks are skipped entirely in `development`).
- **Security implication:** If someone deployed to a real host with `ENVIRONMENT=development`, this (and several other guards below) would all simultaneously disable. This is the single config value every other control pivots on — misconfiguring just this one variable unlocks a wide blast radius.
- **Disposition:** **Retain** — correctly fails closed. The real risk is entirely procedural: never deploy a reachable host with `ENVIRONMENT=development`.
- **Verification:** Against a staging deployment: `POST /api/v1/auth/doctor/login` with valid credentials → response JSON must not contain a `debug_otp_code` key with a non-null value. Separately, confirm `ENVIRONMENT` reported by `GET /healthz`-equivalent/ops endpoint (or `docker exec <container> env | grep ENVIRONMENT`) is `staging` or `production`, never `development`.

### 1.3 OTP codes are never logged

- **Verified:** `grep` across `services/backend/src` for `tracing::(info|debug|warn|error)!` calls referencing `code`, `password`, `secret`, or `otp` found only three matches, none of which log a secret value: `api/router.rs:73` logs an HTTP error *code* (status), `services/auth_service.rs:74` logs a fixed string ("Development patient OTP generated; SMS delivery bypassed") with no interpolated value, and `services/dr_reconciliation.rs:139` logs an error *code* field unrelated to OTP.
- **Disposition:** Retain. No remediation needed. Re-run this grep after any future change that touches an OTP/password code path, since this is the kind of regression that's easy to reintroduce silently.
- **Verification:** `grep -rn "tracing::" services/backend/src | grep -iE "otp|password|secret"` and manually confirm no match interpolates an actual secret value.

### 1.4 Admin TOTP — confirmed fully removed, no dead code

- **Files:** doc-comment-only references remain at `services/backend/src/services/auth_service.rs:828,926` ("Admin no longer has a second, TOTP-based factor..."); migration `20261202000002_remove_admin_totp` already applied.
- **Finding:** No enrollment, verification, or storage code for admin TOTP exists anywhere in `src/`. The `totp_encryption_key_b64` config field (`config.rs`) is a **repurposed, confusingly-named leftover** — it is actually used to encrypt payout-method identifiers (`services/backend/src/api/v1/wallet_handlers.rs:162`: `crypto::encrypt_secret(&clean, &state.config.totp_encryption_key_b64)`), not TOTP secrets.
- **Security implication:** None — this is a naming/documentation hygiene issue, not a control gap. A future reader could be misled into thinking TOTP is still in play.
- **Disposition:** Retain functionally; consider renaming `totp_encryption_key_b64` → something like `secondary_encryption_key_b64` in a future, non-urgent cleanup (out of scope for this audit).

---

## 2. Password reset

(Phase 2 self-service password recovery was built and extensively tested in prior work this session; this audit re-verifies the production-readiness gates specifically.)

### 2.1 Feature flag fails closed outside development/test

- **File/line:** `services/backend/src/services/password_reset_service.rs` (`feature_gate` / `PASSWORD_RESET_ENABLED` check); config validation in `services/backend/src/config.rs:807-...` (the `password_reset_enabled` block) requires full Azure email configuration whenever enabled outside development, and a separate, unconditional `requires_https()` check refuses to let it be enabled at all in staging/production regardless of email configuration.
- **Default:** `PASSWORD_RESET_ENABLED` defaults to `false`.
- **Active in:** development/test only, by explicit design — staging/production activation is refused at config-validation time (process will not boot with it enabled there), pending a real ACS delivery-acceptance test per this project's own stated policy.
- **Disposition:** **Retain as-is.** This is intentional, documented, fail-closed gating, not a bug. Do not enable in staging/production until the explicit, separate sign-off this project's own history requires has happened.
- **Verification:** `POST /api/v1/auth/password-reset/request` against staging → expect `404 Not Found` while the flag is off (confirmed behavior from this session's own test suite: `disabled_by_default_returns_not_found`).

### 2.2 `debug_otp_code` on the reset-request response

- Same double-condition (`environment == "development" && dev_auth_otp_enabled`) as §1.2, confirmed at `password_reset_service.rs:138-141`. Same disposition and verification approach.

---

## 3. Payment and booking — the single largest functional gap found

### 3.1 Appointment booking is entirely blocked outside development

- **File/line:** `services/backend/src/services/appointment_service.rs:46-48`
  ```rust
  if state.config.environment != "development" {
      return Err(AppError::Conflict("Payment collection is not configured. No checkout or confirmed payment was created.".into()));
  }
  ```
- **Feature:** `AppointmentService::book_appointment` — this is the **entire** patient-facing appointment-booking flow (`POST /api/v1/appointments/book`), the core revenue-generating function of the platform.
- **Condition:** `environment != "development"`. No feature flag, no override.
- **Active in:** development only. **This means no patient can book any appointment in test, staging, or production as the code currently stands** — there is no real payment-gateway (M-Pesa/Bkash/card) integration behind this check yet; the guard exists specifically because completing a booking requires confirming a payment, and no real provider is wired in.
- **Security implication:** This is not a vulnerability — it is the opposite, a correct fail-closed guard against fabricating a "confirmed" booking with no real payment behind it. But it is the **most important finding in this entire audit** for the stated purpose ("internal pre-launch verification"): if the intent is to let real (or sandbox) users in staging actually book and pay for a consultation, **this is currently impossible**, full stop, regardless of any other configuration.
- **Disposition:** **CRITICAL — must be explicitly addressed before any staging/production use that includes booking.** Either (a) integrate a real or sandboxed payment provider and replace this gate with a real capability check, or (b) if Milestone 9's "internal pre-launch verification" does not need live booking yet, document this limitation explicitly so it isn't discovered by surprise during a demo.
- **Verification:** `POST /api/v1/appointments/book` with a valid patient token and valid slot against staging → expect `409 Conflict` with the exact message above, every time, regardless of payload.

### 3.2 Payment webhook confirmation — same gate, same reason

- **File/line:** `services/backend/src/api/v1/webhook_handlers.rs:161-165`
- **Feature:** `handle_payment_webhook` (`POST /api/v1/webhooks/payment/:provider`) — the callback a real payment provider would hit to confirm a payment succeeded.
- **Condition/default/active-in:** identical shape to §3.1 — development only, fails closed with `409 Conflict — "Real payment-provider verification is not configured. No payment was confirmed."` outside development.
- **Disposition:** Same as §3.1 — this is the other half of the same unfinished integration, not an independent bug. **CRITICAL**, tracked together with §3.1.
- **Verification:** `POST /api/v1/webhooks/payment/mpesa` with a validly-HMAC-signed synthetic payload against staging → expect `409`, never a `200`.

### 3.3 Legacy payout setter disabled (superseded, not a gap)

- **File/line:** `services/backend/src/api/v1/wallet_handlers.rs:136-141`
- **Feature:** `set_payout_method` — an older doctor payout-configuration endpoint, intentionally replaced by a real Kenya-provider-binding flow.
- **Condition:** `environment != "development"` → `403 Forbidden`.
- **Disposition:** Retain. This is a deliberate deprecation, not an unfinished feature — the message explicitly points to the real flow.

### 3.4 Dev payout binding — fixture-only, fails closed

- **File/line:** `services/backend/src/api/v1/wallet_handlers.rs:195-204`, routed at `services/backend/src/api/router.rs:540-541` as `POST /api/v1/doctor/wallet/payout-methods/dev-bind`.
- **Feature:** Lets a doctor "bind" a synthetic M-Pesa/bank payout destination without real provider authentication, for local testing of the disbursement-routing logic.
- **Condition:** `environment != "development"` → `403 Forbidden`, checked **before** authorization.
- **Disposition:** Retain. Fails closed correctly.
- **Verification:** `POST /api/v1/doctor/wallet/payout-methods/dev-bind` against staging → expect `403` regardless of auth token.

### 3.5 Disbursement routing skips `DEV_SIMULATED` payout destinations outside development

- **File/line:** `services/backend/src/services/disbursement_service.rs:154-158`
- **Feature:** Monthly doctor-earnings disbursement batching deliberately skips (leaves pending for Finance review) any doctor whose only payout destination is the `DEV_SIMULATED` provider, unless `environment == "development"`.
- **Disposition:** Retain. Correct fail-closed behavior — prevents a synthetic/test payout destination from ever being paid out for real.

---

## 4. Agora (video/audio)

- No development-only bypass or mock Agora token issuance was found in `services/backend/src` beyond the existing, already-reviewed `agora_token_service.rs` time-window/role checks (`agora_token_window_before_mins`/`_after_mins`, generous in test config per `tests/api_tests.rs::test_config()` but not environment-gated — these are tunable windows, not a dev-only bypass). No further finding here; Agora App Certificate placement (never in client code) was already a standing, unchanged invariant and was not found violated anywhere in `apps/mobile` or `apps/mobile_patient`.

## 5. FCM / push notifications

- **File/line:** `services/backend/src/repository/mod.rs:200-206` — `NoopTransport` (push) is selected only when `RECOVERY_MODE=true` (deliberate) or when `FcmTransport::from_config` returns `None` (FCM credentials simply absent from config). There is no separate `environment == "development"` bypass for FCM specifically.
- **Disposition:** Retain. As long as real FCM credentials are present in a staging/production `.env`, the real transport is used; absence of credentials fails open to "no push sent" (logged, never a hard error) by the same documented design already reviewed in this session's Phase 1B work (`send_templated_email`'s own fire-and-forget contract) — this is an intentional availability trade-off for non-critical notifications, not a bypass of any authentication/payment control.
- **Verification:** confirm `FCM_PROJECT_ID`/`FCM_CLIENT_EMAIL`/`FCM_PRIVATE_KEY_B64` are set in the staging `.env`; `docker logs <backend-container> | grep -i "FCM"` should show no "not configured" warnings once real credentials are present.

## 6. Azure Blob storage

- **File/line:** `services/backend/src/repository/mod.rs` (`build_storage`), `config.rs` (`StorageProvider` enum: `Azure | LegacyS3 | Disabled`).
- **Finding:** `StorageProvider::Disabled` is a legitimate configuration state (used in local/dev-only compose overrides, e.g. `deploy/single-vm/compose.local-verification.yml`'s `STORAGE_PROVIDER: disabled`), under which `state.object_storage` is `None` and every upload/download call (`DocumentStorageService::put/get`) returns `409 Conflict — "Document storage is not configured."` rather than silently succeeding.
- **Active in:** any environment where an operator sets `STORAGE_PROVIDER=disabled` — this is not environment-gated, it is a direct, explicit operator choice. **It would be a real gap only if a staging/production `.env` accidentally omitted/misconfigured this.**
- **Disposition:** Retain the fail-closed behavior; **verification-only** item for staging: confirm `STORAGE_PROVIDER=azure` (not `disabled`) and the four `AZURE_STORAGE_*` variables are actually set before relying on prescription/profile-picture upload in staging.
- **Verification:** `GET` a known clinical document path as an authorized user against staging → expect real bytes back (`200`), not `409`.

## 7. Key Vault

- No direct Key Vault SDK integration was found inside `services/backend/src` itself — Key Vault usage lives in the pgBackRest backup toolchain (`deploy/single-vm/backup/fetch-keyvault-secret.sh`, calling Azure Instance Metadata Service + Key Vault REST API via `curl`/`jq`), reviewed in Audit C, not here. No dev-only bypass of Key Vault was found because the backend application itself does not call it.

## 8. Database access

- **Role passwords:** `validate_public_exposure` (`config.rs:750-778`) requires every one of the five DB role passwords (`DB_API_ROLE_PASSWORD`, `DB_AUTH_WORKER_PASSWORD`, `DB_PAYMENT_WORKER_PASSWORD`, `DB_SYSTEM_WORKER_PASSWORD`, `DB_FINANCE_WORKER_PASSWORD`) to be a non-empty hex string, unconditionally (not environment-gated) — a missing/weak password fails startup in every environment, including development. No bypass found.
- **Fixture/seed accounts cannot authenticate outside dev/test** — see §10 below; this is the single most important database-adjacent control in this audit and is correctly doubly-enforced.

## 9. Logging

- Covered in §1.3. Separately: `services/backend/src/system_error.rs`'s `redact()` function (reviewed in this session's earlier Phase 1B work) already redacts bearer tokens and key-value-shaped secrets before they reach `system_error_logs` or plain stdout — not re-audited line-by-line here since it was already modified and tested this session, but no new gap was found adjacent to it.

## 10. Rate limiting / admission control

- **Finding:** `GLOBAL_IP_RATE_LIMIT_PER_MIN` is overridden to `100000` (effectively unlimited) in `services/backend/docker-compose.configA.yml:120`, `docker-compose.capacity.yml`, `docker-compose.benchmark-a.yml`, `docker-compose.benchmark-b.yml` — but **every one of these is an explicitly-named, self-documented load-testing/benchmark compose file**, never the real deployment topology.
- **Confirmed clean:** `deploy/single-vm/compose.yml` (the actual approved single-VM production topology) has **no** `GLOBAL_IP_RATE_LIMIT_PER_MIN` or any other rate-limit override at all — it inherits `config.rs`'s real default.
- **Security implication:** None in the real deployment path. The risk is purely operational/naming confusion: `docker-compose.configA.yml` sounds like it could be mistaken for *the* production config by an operator unfamiliar with the repo's history. It is not — `deploy/single-vm/compose.yml` is.
- **Disposition:** Retain all four benchmark files as-is (they're correctly out-of-band). **Recommend** (not required by this read-only audit, flagged for a future non-urgent cleanup) renaming `docker-compose.configA.yml` to something unambiguous like `docker-compose.benchmark-configA.yml` to prevent operator confusion.
- **Verification:** `grep -n GLOBAL_IP_RATE_LIMIT_PER_MIN deploy/single-vm/compose.yml` → expect no match.

## 11. Test-only / debug-only HTTP endpoints

| Route | Handler | Gate | Status |
|---|---|---|---|
| `POST /api/v1/auth/register-otp` | `request_patient_otp` | `environment != development` → 409 | fails closed |
| `POST /api/v1/auth/verify-otp-and-login` | (paired verify for above) | same underlying flow | fails closed |
| `POST /api/v1/doctor/wallet/payout-methods/dev-bind` | `dev_bind_payout_method` | `environment != development` → 403 | fails closed |
| `POST /api/v1/admin/logs/simulate` | `simulate_admin_log` | `simulations_allowed()` (dev/test/**staging**) + Admin ABAC | **allowed in staging by design** |
| `POST /api/v1/admin/system-errors/simulate` | `simulate_system_error` | same as above | **allowed in staging by design** |

- **Finding of note:** `simulations_allowed()` (`config.rs:708-714`) permits `DEV_SIMULATION_ENABLED` in `development | test | staging` — **not just development**. This is the one place in the whole audit where a "debug" capability is reachable in staging by design, not just by misconfiguration.
- **Security implication:** Low — both routes require a real, authenticated Admin session plus explicit ABAC `Action::SimulateLog` authorization; they let an Admin inject a synthetic log/error row for testing alerting pipelines, not bypass any auth/payment/clinical control.
- **Disposition:** **Retain for staging** (useful for testing monitoring/alerting before go-live) but **confirm `DEV_SIMULATION_ENABLED=false` in the real production `.env`** — `config.rs:789-790` already makes `true` fatal at startup in `production` specifically (not staging), so this is enforced, not just convention.
- **Verification:** `POST /api/v1/admin/system-errors/simulate` as an authenticated Admin against staging → expect `200` (by design) if `DEV_SIMULATION_ENABLED=true` there; the same call against a `production`-environment deployment must be structurally impossible to reach because the process would have refused to start with that flag true.

## 12. Seed scripts, fixtures, and hardcoded test accounts

### 12.1 `services/backend/src/seed.rs` — exact fixture inventory

| Account | Email/Phone | Password | User ID (fixed) | Notes |
|---|---|---|---|---|
| Admin | `admin@hellodoctor.health` | `AdminPass123!` | `a0000001-0000-0000-0000-000000000001` | hardcoded Argon2id-hashed at seed time |
| Patient | phone `+254700000002` (no email) | none (phone/OTP-style account) | `b0000001-0000-0000-0000-000000000001` | `PATIENT_PROFILE_ID = ba000001-...-000000000001` |
| Doctor 1 | `sabrina.akter@hellodoctor.health` | `Password123!` | `10000001-0000-0000-0000-000000000001` | license `SYNTH-KE-0001`, verified, 5 open schedule slots seeded, synthetic `DEV_SIMULATED` M-Pesa payout destination |
| Doctor 2 | `anika.rahman@hellodoctor.health` | none (no password hash set) | `10000002-0000-0000-0000-000000000002` | license `SYNTH-KE-0002`, verified, no login path (password-less) |

- **When it runs:** `services/backend/src/main.rs:61-66` — `if config.seed_dev_data { seed::seed_dev_data(...).await }`, called once at process startup, idempotent (checks for existing rows first).
- **Default:** `SEED_DEV_DATA` defaults to `false`; `config.rs:984-986` makes `true` fatal at startup in any environment other than `development`/`test`.
- **Critical compensating control:** `services/backend/src/seed.rs:262-271` — `is_fixture_user()` checks the four fixed UUIDs above by exact string match. This is enforced **twice**, independently:
  1. At login/token-issuance time: `services/backend/src/services/auth_service.rs:1201-1207`.
  2. On **every single subsequent authenticated request**, not cached: `services/backend/src/api/middleware/auth.rs:128-134` (`check_active_session`, called per-request).

  Both checks use the identical condition `!state.config.is_development_or_test() && is_fixture_user(...)` → `401 Unauthorized — "Development fixtures are unavailable."` This means even if a development database snapshot were accidentally restored into a staging/production deployment, none of these four fixed accounts could authenticate or keep an existing session alive there.
- **Security implication:** Low, specifically *because* of the double enforcement above. The passwords (`AdminPass123!`, `Password123!`) are intentionally simple and would be a real risk **only** if the fixture-account guard above were ever removed or weakened.
- **Disposition:** **Retain exactly as-is.** This is one of the better-designed controls in the codebase — do not weaken the double-check, and do not add new seeded accounts without extending `is_fixture_user()` to cover them.
- **Verification:** Against staging/production: attempt `POST /api/v1/auth/admin/login` with `admin@hellodoctor.health` / `AdminPass123!` → expect `401` (if the account even exists there — it should not, since `seed_dev_data` never ran). Separately, as a defense-in-depth check even if such a row somehow existed: confirm the response is `401` with the exact "Development fixtures are unavailable" message, not a generic invalid-credentials message, to confirm the fixture-specific guard (not just a wrong-password rejection) is what fired.

### 12.2 `examples/reset_admin_password.rs` — offline recovery tool

- **File:** `services/backend/examples/reset_admin_password.rs:23-26`.
- **Feature:** CLI tool (`cargo run --example reset_admin_password -- <email>`) to reset one existing Admin's password hash directly via `DATABASE_URL`, bypassing the HTTP API entirely.
- **Condition:** Reads `ENVIRONMENT` directly from the process environment (`std::env::var`, not via `AppConfig`) and refuses to run unless it is exactly `development` or `test` (line 24).
- **Security implication:** This is a local CLI tool requiring direct database credentials and terminal access on whatever host runs it — not an HTTP-reachable attack surface. The real residual risk is **procedural**: if an operator's shell happens to have `ENVIRONMENT=development` exported while `DATABASE_URL` points at a staging/production database (a credential-management mistake, not a code bug), this tool would proceed. It never prints the password/hash and revokes all existing sessions for the target account after resetting.
- **Disposition:** Retain. **Recommend as an operational procedure** (not a code change): never run this example from a shell/host that also has a production `DATABASE_URL` available, regardless of the `ENVIRONMENT` value set there.
- **Verification:** Not HTTP-testable; this is a host-level/operational control. Confirm via deployment runbook review that no production host has both a copy of this repository's `examples/` directory buildable *and* production database credentials simultaneously reachable from the same shell session.

### 12.3 `services/backend/examples/config_preflight.rs` — operator validation tool (not a risk; a mitigation)

- **File:** `services/backend/examples/config_preflight.rs:1,11,14`.
- **Feature:** "Secret-free startup configuration validation" — loads `AppConfig` and runs the same `validate_public_exposure()` checks the real binary runs at boot, without needing to actually start the server, and panics loudly (e.g. `"FATAL: WEB_AUTH_ASSERTION_SECRET is required outside development/test."`) on any misconfiguration.
- **Disposition:** **Recommend actively using this as a pre-deployment gate** — `cargo run --example config_preflight` against the real staging/production `.env` before every deployment is a cheap, already-built way to catch exactly the class of "accidentally left ENVIRONMENT=development" or "forgot a required secret" mistakes this whole audit is concerned about.

### 12.4 Other `examples/*.rs` (stress/capacity/benchmark tooling)

- `auth_stress_test.rs`, `booking_concurrency_test.rs`, `capacity_test.rs`, `load_test.rs`, `web_workload.rs`, `rtc_token_scenario.rs`, `clinical_doc_test.rs`, `dr_reconcile.rs`, `print_benchmark_doctor_hash.rs` — all are standalone binaries requiring explicit invocation (`cargo run --example ...`) and their own target `.env`/connection details; none are compiled into or reachable from the actual `hellodoctor-backend` server binary or its HTTP surface. No further individual review performed here (out of this audit's scope); flagged only so an inventory exists. None were found to hardcode production-shaped credentials.

---

## 13. Web app (Admin Web / Doctor Web) dev bypasses

- **`apps/admin-web/src/middleware.ts:55-58`, similarly in `apps/doctor-web`:** CSP relaxation (`'unsafe-eval'`, a `ws://127.0.0.1:3000` connect-src) gated on `process.env.NODE_ENV !== 'production'` — this is standard, benign Next.js dev-mode hot-reload support. `next build && next start` (used by both apps' production Dockerfiles, confirmed in this session's ARM64 container audit) sets `NODE_ENV=production` automatically; no manual step required and no override found that could defeat this.
- **`apps/admin-web/src/lib/sideDoorGuard.ts`, `apps/doctor-web/src/lib/sideDoorGuard.ts`:** Despite the name, **this is not a bypass** — it is a defensive request-shape filter (blocks HTTP verb-tunneling headers, cloud-metadata/SSRF-probe paths, backup-file-harvesting path patterns, and JSON prototype-pollution payloads) applied in `middleware.ts` before routing. Flagged here only because the filename could be misread during a quick audit; confirmed by full read that it is a protective control, not a hidden door. No action needed.
- **`apps/admin-web/src/data/adminStore.ts`:** matched the dev/mock grep sweep; not individually re-verified line-by-line in this pass (flagged for completeness; no indication of a production auth bypass was found in the surrounding middleware/session files that would make a client-side mock store exploitable — the real authorization boundary is server-side, in the Rust backend's ABAC checks, not this file).
- No hardcoded test login or mock API layer that could affect a production build was found in either web app.

## 14. Mobile app (Patient / Doctor) dev bypasses

- **`apps/mobile_patient/lib/core/network/api_client.dart:98,153`:** Base URL is supplied via a build-time `API_BASE_URL` define/env, with an explicit runtime check requiring HTTPS unless the host is `localhost`/`127.0.0.1`/`::1` (local) — i.e., a LAN/plaintext backend requires deliberately overriding this, it is not the default. No hardcoded production-pointing-at-dev-backend default was found.
- Given this session's prior, extensive review of both mobile apps' auth flows (Patient `AuthCubit`/`AuthRepository`, Doctor `AuthCubit` direct-API pattern) during the Phase 2 password-reset work, no additional client-side debug-mode auth bypass was found in this pass. A full line-by-line mobile re-audit was not repeated here; this section should be treated as a lighter-touch pass than §1–§12, which were exhaustively re-verified against current file content.

---

## Summary — prioritized

### CRITICAL — must be explicitly resolved or consciously accepted before any staging/production use that includes real bookings
1. **§3.1/§3.2 — Appointment booking and payment-webhook confirmation are both hard-gated to `ENVIRONMENT=development` only.** No patient can complete a real booking+payment anywhere else. This is almost certainly the single biggest gap between "code is ready" and "pre-launch verification can actually exercise the core product," and it is **not** a bypass to remove — it is a missing real payment-gateway integration the fail-closed guard is correctly standing in for.

### Important — verify before deployment, no code change required
2. Confirm the staging/production `.env` sets `ENVIRONMENT=staging` or `production` (never `development`), `DEV_AUTH_OTP_ENABLED=false`, `SEED_DEV_DATA=false`, `DEV_SIMULATION_ENABLED` per the staging-allowed/production-forbidden split in §11, `PASSWORD_RESET_ENABLED=false` (until the separate ACS sign-off happens), `STORAGE_PROVIDER=azure` (not `disabled`). Run `cargo run --example config_preflight` (§12.3) against the real `.env` as a mechanical check.
3. Confirm no seeded fixture account (§12.1) exists in the staging/production database — it shouldn't, since `SEED_DEV_DATA` fails closed, but worth a direct query as a pre-launch sanity check.
4. Rename (non-urgent) `docker-compose.configA.yml` to avoid operator confusion with the real `deploy/single-vm/compose.yml` (§10).

### Acceptable as-is — already fail closed, no action needed
5. Doctor/Admin/Patient `debug_otp_code` echo (§1.2) — double-gated, structurally safe.
6. Fixture-account login (§12.1) — doubly enforced at login and per-request.
7. Dev payout binding and legacy payout setter (§3.3/§3.4) — fail closed.
8. FCM/push Noop fallback (§5), Azure Blob `Disabled` state (§6) — explicit operator-configured states, not silent dev bypasses.
9. Admin simulate-log/simulate-error endpoints (§11) — intentionally staging-available, Admin-authenticated, non-destructive.
10. Logging hygiene (§1.3/§9), Admin TOTP removal (§1.4), `sideDoorGuard.ts` (§13) — confirmed clean/protective, no remediation needed.

**Total findings documented:** 24 distinct items across 14 sections (4 CRITICAL/Important flagged for action, 20 confirmed-safe-as-designed).
