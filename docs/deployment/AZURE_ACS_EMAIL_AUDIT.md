# Milestone 9 — Audit D: Azure Communication Services Email

**Audit date:** 2026-11-07
**Scope:** `services/backend/src/services/email_transport.rs`, `src/email_delivery.rs`, `src/services/email_dispatch_worker.rs`, `src/services/password_reset_service.rs`, `src/config.rs` (Azure email + password-reset validation), `src/rate_limit.rs`, `src/repository/email_outbox_repo.rs`, the `email_outbox` table (migrations `20261205000001`, `20261206000001`, `20261209000001`), and the generated template gallery under `docs/mail/`.
**Method:** Direct source review, re-verified against the current working tree at the exact lines cited (not reconstructed from memory of having built most of this system earlier in this overall engagement). No email was sent and no Azure resource was queried or created to produce this report.

---

## 1. ACS provisioning requirements

This codebase expects an already-provisioned Azure Communication Services resource with an Email Communication Service attached and at least one verified sender domain. Nothing in this repository provisions ACS itself — there is no Terraform/Bicep/ARM template for it anywhere in the repo (not found by search). The backend only ever *consumes* three pieces of information about an already-existing ACS resource:

- `AZURE_EMAIL_ENDPOINT` — the ACS Email data-plane base URL (`src/config.rs:647`).
- `AZURE_EMAIL_ACCESS_KEY` — base64 HMAC signing key (`src/config.rs:650`).
- `AZURE_EMAIL_SENDER` — the verified "from" address (`src/config.rs:653`).

`AcsEmailTransport::from_config` (`src/services/email_transport.rs:84-121`) additionally validates, at construction time, that the endpoint is `https://` with no embedded credentials and no custom port (`email_transport.rs:94-102`), and that the access key is valid base64 (`email_transport.rs:107-109`). These are input-shape checks only — **nothing in this codebase has ever made a real network call to a real ACS endpoint** (confirmed: the only test that exercises `AcsEmailTransport::send_email` over real sockets uses a local TCP listener that never responds, to test timeout classification, not a real ACS call — `email_transport.rs`'s `unresponsive_endpoint_times_out_and_is_classified_retryable` test).

## 2. Sender domain configuration

`AZURE_EMAIL_SENDER` is a single, flat string (`src/config.rs:181`) — there is no code-level support for multiple sender addresses, per-role senders, or a differently-branded sender for OTP vs. transactional mail; every email this system sends (OTP, password reset, welcome, doctor lifecycle, consultation/payment/refund, password-changed) uses the one configured sender. Verifying the sender domain itself (SPF/DKIM/DMARC records, domain ownership in ACS) is entirely an Azure-portal/DNS-side responsibility with nothing to check in this repository.

## 3. Access-key vs. managed-identity support

**Only `access_key` auth mode is implemented.** `AZURE_EMAIL_AUTH_MODE` defaults to `"access_key"` (`src/config.rs:656`, default confirmed by the env-var read pattern) and `validate_azure_email_configuration` (`src/config.rs:1015-1049`) explicitly rejects `managed_identity` with a hard error:

> "AZURE_EMAIL_AUTH_MODE=managed_identity is not yet implemented: the Microsoft Entra ID / managed-identity REST auth contract for the Azure Communication Services Email data-plane endpoint could not be verified against official documentation. Use access_key." (`config.rs:1019-1025`)

This means the long-lived `AZURE_EMAIL_ACCESS_KEY` HMAC secret is the **only** supported credential path today, in contrast to this project's Azure Blob integration (which documents a managed-identity-preferred stance — see `docs/AZURE_BLOB_SETUP.md`) and the pgBackRest backup container (which supports `repo1-azure-key-type=auto`, i.e. managed identity, for its own Blob repository access — see Audit C). **This is a real, documented asymmetry**: ACS email is the one Azure-facing credential in this stack that cannot currently avoid a long-lived static secret.

## 4. Required environment variables

| Variable | Required when | Default | Notes |
|---|---|---|---|
| `AZURE_EMAIL_ENDPOINT` | Always together with the two below, or fail-closed outside development (`config.rs:1042-1047`) | none (`Option<String>`, `config.rs:647`) | Must be `https://`, no credentials/port embedded. |
| `AZURE_EMAIL_ACCESS_KEY` | Same as above | none (`config.rs:650`) | Base64; never logged (`Debug` impl explicitly redacts it via `secret_opt`, `config.rs:383-385`). |
| `AZURE_EMAIL_SENDER` | Same as above | none (`config.rs:653`) | Single flat string, see §2. |
| `AZURE_EMAIL_AUTH_MODE` | Optional | `"access_key"` (`config.rs:656`) | Only `access_key` actually works (§3). |
| `PASSWORD_RESET_ENABLED` | Optional, gates an entirely separate feature | `false` | See §9 — has its own, stricter fail-closed rule layered on top of the above. |

**Fail-closed rule actually enforced** (`config.rs:1031-1047`): the three core variables must be configured *all together or not at all* (`any_configured && !fully_configured` → hard error), and **outside development/test** (`requires_https()` — staging and production), they are *mandatory*: if any is missing, `AppConfig::validate` fails at process startup with "Azure Communication Services Email must be configured outside development/test (Patient, Doctor and Admin login all depend on it to complete MFA)." This means **a staging/production backend container cannot even start** without all three being set — login/MFA cannot be made to silently degrade to "no OTP sent" in those environments; the process refuses to boot instead.

## 5. Key Vault integration requirements

**None exist for the backend's own ACS credentials today.** A repo-wide search for Key Vault integration (`key_vault`, `KeyVault`, "Key Vault") in `services/backend/src/` returns no matches — the only Key Vault integration anywhere in this repository is `deploy/single-vm/backup/fetch-keyvault-secret.sh`, which is scoped exclusively to the pgBackRest backup container's own encryption cipher passphrase (see Audit C), not to any backend application secret. The backend's three ACS variables (and every other backend secret — JWT keys, DB role passwords, Agora/FCM credentials) are expected to arrive via a plain `env_file` on the host (`deploy/single-vm/compose.yml:70`, `${BACKEND_ENV_FILE:?...}`), i.e. a root-owned, mode-0600 flat file on the VM per the existing hardening guidance in `docs/UBUNTU_2404_ARM64_DEPLOYMENT_READINESS.md` — not fetched from Key Vault at runtime, and not rotatable without an operator editing that file and restarting the container.

**If Key Vault custody of the ACS access key is desired for parity with the backup container's own secret-handling model, that integration does not exist today and would need to be built** — this is a gap, not a partially-implemented feature.

## 6. Real delivery test procedure (prerequisites, not executed by this audit)

No real delivery test has ever been run against a real ACS endpoint in this codebase's history (confirmed by the absence of any such test, and by the explicit statements in `config.rs:1056-1058`'s own doc comment: "this codebase has never completed a real Azure Communication Services delivery acceptance test"). The concrete procedure, were it to be run (not executed here — read-only audit):

1. Provision a real ACS Email Communication Service, verify a sender domain, generate an access key.
2. Set `ENVIRONMENT=staging` (or a disposable equivalent that sets `requires_https()`-equivalent behavior without actually exposing the deployment publicly) with the three `AZURE_EMAIL_*` variables pointed at the real resource, `DEV_AUTH_OTP_ENABLED=false`.
3. Trigger one real OTP send (e.g. a Doctor/Patient/Admin login) and confirm: (a) the real email arrives at a real inbox, with correct branded HTML per the templates already verified in `docs/mail/previews/`; (b) the corresponding `email_outbox` row reaches `delivery_status='SENT_TO_PROVIDER'` with a non-null `provider_message_id` if ACS returns one (`email_transport.rs:187-193` notes ACS's `/emails:send` response does not expose a documented stable message-id field, so this may legitimately stay `NULL` even on success — confirm this against the real response).
4. Separately trigger one transactional send (e.g. `PATIENT_WELCOME` via a real registration) and confirm the same, via the async dispatch worker path (not the inline OTP path — see §7).
5. Intentionally misconfigure the access key (one bad character) and confirm the resulting `Permanent`/401-or-403 classification (`email_transport.rs:202-205`) and the resulting user-facing behavior (§7) rather than assuming it from code reading alone.

## 7. Provider failure and timeout behavior

Two genuinely different delivery models exist, and they fail differently — this distinction matters for the readiness matrix:

**OTP (login, password-reset) — synchronous, must-succeed-or-fail, never silently dropped.** `send_otp_email_tracked_inner` (`email_delivery.rs`) retries a `Retryable` outcome inline, within the same HTTP request, up to `MAX_INLINE_ATTEMPTS = 2` total attempts with `INLINE_RETRY_BACKOFF = 500ms * attempt_number` between them (`email_delivery.rs:75-76`). Each provider call is bounded by `ACS_REQUEST_TIMEOUT = 8s` (`email_transport.rs:74`), so worst-case added latency on a hung provider is `2 * 8s + 500ms ≈ 16.5s` before the login/registration request itself fails. On exhaustion or a `Permanent` outcome, the request fails with `AppError::ServiceUnavailable` (503) — **this was a real bug found and fixed in the immediately preceding session**: it previously returned `AppError::internal` (500, "unexpected server bug") for this exact case; it now correctly distinguishes "a dependency is down, fail closed by design" (503) from a genuine application bug (500). A missing-configuration case (no ACS configured at all, outside development) instead returns `AppError::Conflict` (409) immediately, before any network attempt — a static, non-retryable precondition failure, correctly distinguished from a transient provider failure.

**Transactional (welcome, doctor lifecycle, consultation/payment/refund) — asynchronous, best-effort, never blocks the triggering business event.** As of the Phase 1B work in the immediately preceding session, these use a durable outbox (`email_outbox_repo::enqueue_transactional`) inserted atomically inside the same database transaction as the business event, dispatched later by `services::email_dispatch_worker` — either immediately (a detached, fire-and-forget task spawned right after commit) or by the periodic 60-second scheduler pass as a durable fallback. Retry uses real backoff across ticks, not inline sleeps: `MAX_ATTEMPTS = 8`, exponential `30 * 2^(attempt-1)` seconds capped at 3600s (`email_dispatch_worker.rs`), with a `LEASE_SECONDS = 120` claim lease for stale-worker recovery. Critically: **a transactional email's failure, retried or permanent, never fails or rolls back the business event it's about** — the triggering transaction has already committed independently. The older, still-in-use `send_templated_email` path (used only for `PASSWORD_CHANGED` notifications, not migrated to the new outbox/worker model) is simpler — exactly two inline attempts, then logged and silently dropped, same "never block/fail the caller" contract but no durable retry across ticks.

## 8. Retry/deduplication semantics

Deduplication is uniform across both models: every send is keyed by an event-specific `dedup_key` enforced by a database `UNIQUE` constraint on `email_outbox.dedup_key` (migration `20261205000001`), via `INSERT ... ON CONFLICT (dedup_key) DO NOTHING` (`email_outbox_repo::insert_if_new` / `enqueue_transactional`). Keys are genuinely event-specific, not template-only — e.g. `patient-welcome:{user_id}`, `doctor-approved:{doctor_profile_id}`, `payment-confirmed:{transaction_id}`, `consultation-rescheduled:{reschedule_event_id}` — so a retried request or a replayed webhook cannot create a second email for the same real-world event, only the OTP challenge token itself (always a fresh UUID) varies the OTP dedup key per attempt. **Known, explicitly documented residual risk** (`email_outbox_repo.rs`'s own module doc): a PITR restore to a point before a row reached `SENT_TO_PROVIDER` can cause a genuine duplicate send on replay — this is accepted, at-least-once delivery, not a bug, but must be understood by an operator before any real restore drill (see Audit C).

Separately from per-event dedup, two Redis-backed abuse budgets exist (`src/rate_limit.rs:14-44`): `check_otp_delivery_limit` enforces four simultaneous windows per OTP recipient — global `30/60s` (all recipients combined), per-recipient cooldown `1/60s`, per-recipient rolling `3/900s`, per-recipient daily `20/86400s` — reserved atomically via a single Lua `EVAL` so a rejected attempt never partially consumes any window. Transactional mail has a separate, more generous shared budget: `TRANSACTIONAL_EMAIL_RECIPIENT_LIMIT = 20` per recipient per `TRANSACTIONAL_EMAIL_RECIPIENT_WINDOW_SECS = 86400` (24h), shared across all 9 transactional categories combined (`email_delivery.rs:322,331-332`) — explicitly defense-in-depth against one mailbox being flooded by many *distinct* legitimate-looking events, not a correctness mechanism.

## 9. Password-reset activation prerequisites

`PASSWORD_RESET_ENABLED` (default `false`) is validated by `validate_password_reset_policy` (`config.rs:1065-1092`), which is **strictly fail-closed in staging/production regardless of any other configuration**: if `requires_https()` is true, enabling it is an unconditional hard error at startup — "cannot be enabled in staging/production until a real Azure Communication Services delivery acceptance test has been completed and separately approved" (`config.rs:1074-1078`). This is a deliberate, explicit policy decision recorded in the function's own doc comment, not an oversight: self-service account recovery is a novel, previously-nonexistent attack surface for this system, and the project's stated position is that it must not go live ahead of both (a) the real ACS delivery test in §6, and (b) a separate, explicit approval — neither of which this audit can grant or simulate. Outside production/staging, `development` may rely on the existing `DEV_AUTH_OTP_ENABLED` bypass instead of real email configuration; `test` must configure (even if fake/no-op) email delivery, matching the same rule the three login-OTP paths already use.

A separate, lower-level fix already applied in the immediately preceding session (migration `20261208000001`, `password_reset_repo::upsert_for_user`) closed a genuine issuance race (two concurrent reset requests for the same account could previously leave two live challenge rows) via a database `UNIQUE` constraint on `password_reset_requests.user_id` plus an atomic `INSERT ... ON CONFLICT ... DO UPDATE`. This is a code-correctness fix, independent of and does not substitute for the ACS delivery-acceptance/approval gate above.

## 10. Monitoring and troubleshooting

No dedicated external monitoring/alerting integration exists for email delivery specifically (contrast with the backup container's `alert-notify.sh` webhook mechanism — Audit C). Operational visibility is limited to:

- The `email_outbox` table itself — every send attempt (OTP and transactional) is a durable row with `delivery_status`, `delivery_attempts`, `last_delivery_error` (truncated to 500 chars, sanitized before storage — never the OTP code or any secret), and `provider_message_id` when available. An operator must query this table directly (no admin-UI view of it was found in `apps/admin-web`) to see delivery health.
- `tracing::error!`/`tracing::warn!` log lines at each classification point (`email_delivery.rs`), which (per this codebase's existing convention elsewhere) should flow into whatever log aggregation the deployment configures — not verified as actually wired to anything in this audit.
- `system_error_logs`-style reporting is used elsewhere in this codebase for dependency failures (`AppError::ServiceUnavailable` triggers a `system_error::report` call in `src/error.rs`) — this applies to the OTP path's 503 case, giving it the same operator-visible "dependency down" signal as any other `ServiceUnavailable` in the system; the transactional path's failures do **not** go through `system_error::report` (they only log), since by design a transactional email failure must never look like a system incident the way an OTP failure legitimately is one.
- No metrics/counters specific to email delivery (sent/failed/retried counts) were found exposed via the `/metrics` Prometheus endpoint this project otherwise uses extensively — this is a monitoring gap relative to how thoroughly other subsystems (admission control, notification delivery) are instrumented.

---

## Summary classification

| Item | Status |
|---|---|
| ACS endpoint/credential shape validation | **VERIFIED** (code-level; never tested against a real ACS resource) |
| Managed-identity auth for ACS itself | **BLOCKED** — not implemented, explicitly rejected at config-validation time |
| Key Vault custody of the ACS access key | **BLOCKED** — does not exist; only the separate backup container has any Key Vault integration |
| Fail-closed behavior outside development (missing config) | **VERIFIED** (process refuses to start) |
| OTP delivery failure handling (503/409 distinction) | **VERIFIED** (fixed and tested in the immediately preceding session) |
| Transactional delivery durability (outbox + worker, retry/backoff) | **VERIFIED** (built and tested in the immediately preceding session; 18 new tests, all passing) |
| Real ACS delivery test | **NOT TESTED** — never attempted against a real Azure resource, by design (read-only audit, and no Azure credentials available) |
| Password-reset production activation | **BLOCKED by explicit, documented policy** — requires a real delivery-acceptance test plus separate approval, neither of which exists |
| Monitoring/alerting for email delivery specifically | **PARTIAL** — durable outbox table + logs exist; no dedicated alerting or metrics |
