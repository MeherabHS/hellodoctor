# ARM64 GHCR Release Handoff

**Status:** Pre-publication review document. **Stops at the release-publication approval gate** — no Git staging, commit, push, image build-and-push, or Azure/VM change has been made to produce this document. Everything below is a reviewed plan awaiting explicit authorization.
**Branch:** `test/ubuntu-arm64-validation`, HEAD `1ee38d1e` ("Prepare native Ubuntu ARM64 validation").
**Registry:** GitHub Container Registry (GHCR), private packages.
**Deployment target:** existing Azure Ubuntu 24.04 ARM64 VM, `deploy/single-vm/compose.yml`, digest-pinned images.

## VM preflight (operator-confirmed, 2026-11-07)

| Check | Result |
|---|---|
| OS / arch | Ubuntu, ARM64 |
| Docker Engine | 29.9.3 |
| Docker Compose | v5.7.0 |
| GHCR reachability | HTTPS reachable; `HTTP 401` unauthenticated (expected — registry is up, no credentials configured yet) |
| Docker log driver | bounded, `10m × 3` rotation already configured |
| Running containers | none — clean slate |
| Free disk | ~120 GB |

No blocker found in this preflight. The VM is ready to receive a `docker login` (read-only token, see §5) and image pulls once real digests are published.

---

## 1. Reviewed source revision and commit dependencies

**There is no existing commit that contains the approved work.** All of it — ARM64 Dockerfile fixes, email Phase 0/1B, password-recovery Phase 2, and the `deploy/single-vm/` deployment files — sits uncommitted on top of `1ee38d1e`, mixed together with a large amount of unrelated, still-in-progress milestone work (notification-platform features, data-rights/grievance performance work, admin-web navigation/finance UI, mobile blog/vault features, and dozens of `docs/MILESTONE_8_*`/`docs/REMEDIATION_*` reports). Per your explicit instruction, this release must **not** sweep that unrelated work in.

`git status`/`git diff --stat` against the working tree show **183 modified/deleted files and 188 untracked paths** (371 total). A full categorization was performed and spot-checked directly against the diffs (not filename-guessed) before being relied on here.

### 1.1 Category counts

| Category | Modified | Untracked |
|---|---|---|
| ARM64 deployment & CI | 9 | 15 |
| Email Phase 0/1B | 9 (2 require hunk-level split) | 10 |
| Password-reset Phase 2 | 1 | 11 |
| Shared infra (config/env) | 3 | 1 |
| Unrelated milestone work (excluded) | ~164 | ~152 |

### 1.2 Recommended staged-file list (whole-file, safe to `git add` as-is)

```
# ARM64 deployment & CI
services/backend/Dockerfile
deploy/single-vm/
.github/workflows/arm64-compatibility.yml
.github/workflows/arm64-disposable-integration.yml
.dockerignore
.gitignore
docs/UBUNTU_2404_ARM64_DEPLOYMENT_READINESS.md
docs/ARM64_SUITABILITY_ASSESSMENT.md
docs/VM_CONFIGURATION_GUIDE.md
docs/deployment/

# Email Phase 0/1B
services/backend/src/email_delivery.rs
services/backend/src/services/email_templates.rs
services/backend/src/services/email_transport.rs
services/backend/src/services/email_dispatch_worker.rs
services/backend/src/repository/email_outbox_repo.rs
services/backend/src/services/appointment_service.rs
services/backend/src/services/grievance_service.rs
services/backend/src/api/v1/trace_handlers.rs
services/backend/migrations/20261205000001_email_outbox.up.sql
services/backend/migrations/20261205000001_email_outbox.down.sql
services/backend/migrations/20261206000001_transactional_email_types.up.sql
services/backend/migrations/20261206000001_transactional_email_types.down.sql
services/backend/migrations/20261209000001_transactional_email_dispatch_worker.up.sql
services/backend/migrations/20261209000001_transactional_email_dispatch_worker.down.sql
services/backend/tests/cases/email_outbox_delivery.rs
services/backend/tests/cases/transactional_email_dispatch.rs
services/backend/tests/cases/transactional_email_triggers.rs
services/backend/examples/generate_email_previews.rs
docs/mail/
docs/EMAIL_OUTBOX_RECOVERY_POLICY.md

# Password-reset Phase 2
services/backend/src/services/password_reset_service.rs
services/backend/src/repository/password_reset_repo.rs
services/backend/migrations/20261207000001_password_reset.up.sql
services/backend/migrations/20261207000001_password_reset.down.sql
services/backend/migrations/20261208000001_password_reset_requests_user_id_unique.up.sql
services/backend/migrations/20261208000001_password_reset_requests_user_id_unique.down.sql
services/backend/tests/cases/password_reset.rs
apps/mobile/lib/features/auth/create_new_password_page.dart
apps/mobile/lib/features/auth/forgot_password_page.dart
apps/mobile/lib/features/auth/password_reset_success_page.dart
apps/mobile/lib/features/auth/reset_otp_verify_page.dart
apps/mobile_patient/lib/features/auth/create_new_password_page.dart
apps/mobile_patient/lib/features/auth/forgot_password_page.dart
apps/mobile_patient/lib/features/auth/password_reset_success_page.dart
apps/mobile_patient/lib/features/auth/reset_otp_verify_page.dart

# Shared infra (additive config/validation only)
services/backend/src/config.rs
services/backend/.env.example
services/backend/src/api/v1/auth_handlers.rs
services/backend/examples/config_preflight.rs
```

### 1.3 Files requiring hunk-level staging (`git add -p`), not whole-file

Two files mix approved work with unrelated feature work in the same file — confirmed by direct `git diff` inspection, not filename guessing:

- **`services/backend/src/services/notification_scheduler.rs`** — only the hunk at old-line ~60-76 (the `run_email_outbox_expiry_sweep` and `email_dispatch_worker::run_transactional_email_dispatch_pass` wiring inside `start()`) is Phase-1B. Everything else in this file's ~700 changed lines (hold/greeting/health-education/holiday notification passes, confirmed spanning old-lines 238 through 1743+) is unrelated milestone-6F-adjacent work and must stay unstaged.
- **`services/backend/src/api/v1/admin_handlers.rs`** — only the hunks at old-line 1541 and 1594 inside `review_doctor_professional_registration` (the `DOCTOR_APPROVED`/`DOCTOR_REJECTED` email-enqueue calls) are Phase-1B. The other ~9 hunks in this file (`get_admin_overview`, `list_admin_doctors`, `list_admin_transactions`, `get_finance_summary`, `execute_patient_data_rights`/`execute_doctor_data_rights`, `list_admin_grievances`, `resolve_consultation_recovery`) are an unrelated M8-G20 query-scoping/finance-projection effort and must stay unstaged.

### 1.4 Explicitly excluded, with reasoning

- `docs/FCM_SETUP.md` — push-notification document, not email; only 2 incidental email/OTP keyword hits in an otherwise FCM-specific file.
- `docs/DISASTER_RECOVERY_ARCHITECTURE.md`, `docs/BACKUP_RESTORE_RUNBOOK.md`, `docs/BACKUP_ENCRYPTION_KEY_CUSTODY_RUNBOOK.md`, all `docs/MILESTONE_8_*.md` / `docs/REMEDIATION_*.md` reports — real, separable DR/backup and milestone-reporting work, larger in scope than this release. Leave for a dedicated DR-focused commit.
- `deploy/single-vm/backup/*` (pgBackRest scripts) — path-wise under `deploy/single-vm/`, but functionally DR/backup-domain, not required for a basic ARM64 + email + password-reset release. **Decision needed from you:** include it now (the four-image publication list below assumes yes, since you listed the pgBackRest image explicitly) or defer — see §2.

### 1.5 Migration-ordering finding (important — read before staging)

sqlx applies every `.sql` file under `services/backend/migrations/` in strict timestamp order at process start; it cannot skip a file in the middle of the sequence. Direct inspection of the SQL bodies confirms:

- `20261202000001`–`20261204000004` (admin-role RLS fix, admin-TOTP removal, chat/live-consultation/RLS performance indexes) are **self-contained and independent** of the email/password-reset work — zero references to `email_outbox` or `password_reset_requests`.
- `20261205` → `20261206` → `20261207` → `20261208` → `20261209` form **one inseparable chain**: `20261206` `ALTER TABLE email_outbox DROP/ADD CONSTRAINT email_outbox_email_type_check` on the exact constraint `20261205` created; `20261207` alters that same constraint again and creates `password_reset_requests`; `20261208` adds a UNIQUE constraint onto that table; `20261209` alters `email_outbox` again and grants the roles that `appointment_service.rs`/`grievance_service.rs`/`trace_handlers.rs`/`admin_handlers.rs` depend on at runtime. **All five must ship together, in order, with no gaps.**
- Since 202-204 sit chronologically *before* 205 and sqlx has no mechanism to selectively skip files mid-sequence, **the simplest and safest choice is to migrate 202 through 209 as one contiguous run** rather than attempting to exclude 202-204 (which would mean deleting/renaming already-authored files — strictly riskier than just including them; they are independently reviewed, non-destructive, and each has its own down-migration).

**Practical consequence for §1.2's staged-file list:** add `services/backend/migrations/2026020{2,3,4}*` (the housekeeping migrations) to the staged set even though they're logically "unrelated," specifically because migration sequencing makes them inseparable from the ones you do want. This is the one place where "don't sweep in unrelated work" and "don't break the migration chain" are in tension — recommend resolving it in favor of including the full 202-209 contiguous block.

---

## 2. Images to publish

Per your instruction, four images:

| # | Image | Dockerfile | Built by existing CI? |
|---|---|---|---|
| 1 | Rust backend runtime image | `deploy/single-vm/backend.Dockerfile` (wraps `services/backend/Dockerfile`'s output via `BACKEND_IMAGE` build-arg) | Yes — `arm64-compatibility.yml` job `rust-backend-arm64` |
| 2 | Admin Web image | `apps/admin-web/Dockerfile` | Yes — job `admin-web-arm64` |
| 3 | Doctor Web image | `apps/doctor-web/Dockerfile` | Yes — job `doctor-web-arm64` |
| 4 | PostgreSQL/pgBackRest backup image | `deploy/single-vm/backup/pgbackrest.Dockerfile` | Yes — job `pgbackrest-arm64` |

All four are **already built and verified for real `linux/arm64` architecture** by the existing `arm64-compatibility.yml` workflow (confirmed by direct read this session: each job asserts `docker image inspect --format '{{.Architecture}}'` equals `arm64`, and the backend/admin-web/doctor-web jobs additionally execute the built image and assert the in-container reported architecture). **None of the five builds in that workflow currently pushes anywhere** — every build uses `--load` (local-only), confirmed by direct read of lines 44-56, 78-82, 108-112, 138-142. `permissions:` is `contents: read` only (lines 13-14) — no `packages: write`.

`deploy/single-vm/compose.yml` hard-requires exactly 3 of these 4 via fail-loud `${VAR:?message}` syntax (confirmed): `BACKEND_RUNTIME_IMAGE` (line 69), `ADMIN_WEB_IMAGE` (line 111), `DOCTOR_WEB_IMAGE` (line 129). The 4th, `POSTGRES_BACKUP_IMAGE`, is consumed only by the `compose.backup.yml` overlay, not the base topology — confirmed its env-var template slot in `deploy/single-vm/arm64-images.env.example:18` is currently a **literal unfilled placeholder** (`registry.example.invalid/.../REPLACE_WITH_REVIEWED_ARM64_DIGEST`), meaning this image has never been built-and-published with a real digest before — only an ad hoc local tag has ever existed.

---

## 3. Minimum GitHub Actions changes for private GHCR publishing

**Recommendation: a new, separate workflow file** (e.g. `arm64-ghcr-publish.yml`), not an edit to `arm64-compatibility.yml`. Rationale: `arm64-compatibility.yml` already has an automatic `pull_request` trigger (lines 4-11). Putting publish logic in a brand-new, `workflow_dispatch`-only file makes "this never auto-publishes" true *structurally* — no trigger exists that could fire it — rather than true only as long as a conditional inside a file that also auto-triggers is never misconfigured. This directly matches "stop at the release-publication approval gate."

Minimal shape per image (illustrative — not created yet):

```yaml
on:
  workflow_dispatch:
permissions:
  contents: read
  packages: write
jobs:
  publish-backend-runtime:
    runs-on: ubuntu-24.04-arm
    steps:
      - uses: actions/checkout@<pinned-sha>
      - run: docker buildx build --platform linux/arm64 --load --tag hellodoctor-backend:arm64-ci --file services/backend/Dockerfile services/backend
      - run: docker build --platform linux/arm64 --build-arg BACKEND_IMAGE=hellodoctor-backend:arm64-ci --tag hellodoctor-backend-runtime:arm64-ci --file deploy/single-vm/backend.Dockerfile deploy/single-vm
      - run: test "$(docker image inspect --format '{{.Architecture}}' hellodoctor-backend-runtime:arm64-ci)" = arm64
      - uses: docker/login-action@<pinned-sha>
        with: { registry: ghcr.io, username: ${{ github.actor }}, password: ${{ secrets.GITHUB_TOKEN }} }
      - run: |
          TAG="ghcr.io/<OWNER>/hellodoctor-backend-runtime:$(date +%Y%m%d)-${GITHUB_SHA::12}"
          docker tag hellodoctor-backend-runtime:arm64-ci "$TAG"
          PUSH_OUT="$(docker push "$TAG")"
          DIGEST="$(echo "$PUSH_OUT" | grep -oE 'sha256:[0-9a-f]{64}')"
          echo "Published: $TAG @ $DIGEST" >> "$GITHUB_STEP_SUMMARY"
```

Same pattern repeats for admin-web, doctor-web, and pgbackrest, reusing their existing build steps unchanged. Using plain `docker tag` + `docker push` after the existing `--load` build (rather than re-invoking `buildx --push`) matters specifically for the backend image: the production wrapper's `FROM ${BACKEND_IMAGE}` build is a plain `docker build` (not buildx) that depends on the first image already being visible in the local Docker image store — confirmed this two-stage pattern already works today at lines 52-56. Digest capture via `docker push`'s own stdout is the simplest first-cut option; `docker buildx imagetools inspect <tag> --format '{{json .Manifest}}'` is more durable against future CLI output-format changes if this is ever actually implemented.

**Requires verification:** the real GitHub org/user that will own these packages (shown as `<OWNER>` above — do not invent one), and whether GHCR package visibility needs an explicit "private" setting after first publish (it defaults to the host repo's visibility, but confirm explicitly rather than assume).

### 3.1 A real gap found in the existing six-service integration test

`.github/workflows/arm64-disposable-integration.yml` is the one workflow that could actually validate all six services starting together from real images — it's `workflow_dispatch`-triggered and takes `backend_runtime_image`/`admin_web_image`/`doctor_web_image` as **required** digest-reference inputs (confirmed, lines 6-17) — but it has apparently **never been run for real**, because no image has ever been published for it to reference. Confirmed by full read: it has **no `docker login` step anywhere** and only `permissions: contents: read` (line 20), so even with real GHCR digests supplied as inputs, `docker compose ... up -d` (lines 89-93) would fail to pull private images. **Minimal fix needed:** add `permissions: packages: read` and one `docker login ghcr.io` step (using `secrets.GITHUB_TOKEN`, which already has in-repo read access to packages published from the same repository — no separate PAT needed for this CI-internal case) before the "Render disposable Compose configuration" step. Separately, note this workflow only exercises `compose.yml` + `compose.local-verification.yml` + `compose.arm64-ci.yml` — it does not include `compose.backup.yml`, so it never exercises the 4th (pgBackRest) image; flagged for completeness, out of scope to fix here unless you want backup-overlay coverage added too.

---

## 4. Confirmed: all four target images are `linux/arm64` and digest-pinnable

- Backend runtime, Admin Web, Doctor Web, pgBackRest: all four verified this session via real `docker buildx build --platform linux/arm64 --load` + `docker image inspect --format '{{.Architecture}}'` checks (backend also executed under QEMU and produced genuine Rust config-validation panics, proving real code execution, not just a label match).
- All four Dockerfiles pin their base images by immutable `sha256:` manifest digest already (confirmed: `rust:1-bookworm@sha256:...`, `debian:bookworm-slim@sha256:...`, and the Postgres 16 digest is consistent across `compose.yml`, `arm64-images.env.example`, and `pgbackrest.Dockerfile`).
- GHCR supports `docker push`ing a tag and reading back its resulting immutable digest via `docker buildx imagetools inspect` — standard OCI registry behavior, no GHCR-specific blocker. Digest-pinning the four published images in `compose.yml`'s `${..._IMAGE}` variables is the same `@sha256:...` pattern already used for every base image in this repo.

---

## 5. Secure, minimal registry authentication for the VM (read-only)

```bash
# Token generated by the operator out-of-band at github.com/settings/tokens:
# classic PAT, scope read:packages ONLY — never write:packages or delete:packages.
install -m 600 /dev/stdin /root/.hellodoctor-ghcr-ro-token <<< '<PASTE_TOKEN_HERE>'
cat /root/.hellodoctor-ghcr-ro-token | docker login ghcr.io -u <github-username> --password-stdin
```

- Store the token file **outside any git-tracked path** (e.g. directly under `/root/`, never under `deploy/single-vm/`, which could be synced/copied alongside tracked deployment files).
- **Requires verification:** whether the packages end up org-owned or user-owned. Org-owned private packages may need the PAT's SSO authorized separately (if the org enforces SAML SSO) and/or explicit per-package "read" access granted to the pulling identity, beyond just the `read:packages` scope. Also confirm whether GHCR's fine-grained PAT support (tighter least-privilege, scoped to only this repo's Packages:read) is viable today — it has historically lagged classic PATs for package pulls; verify before choosing between the two.
- Never embed the token value in any script, Dockerfile, Compose file, or tracked repo path. The VM preflight already confirmed GHCR returns `401` unauthenticated — this is the expected pre-login state, not an error.

---

## 6. Ingress / port-conflict check against 1Panel and Cloudflare Tunnel

Confirmed by direct read of `deploy/single-vm/compose.yml`: **only the `proxy` (Caddy) service publishes host ports** — `postgres`, `redis`, `backend`, `admin-web`, `doctor-web` all have zero `ports:` entries (network-only addresses on the internal `app`/`data` networks). The `proxy` block (lines 151-153):

```yaml
ports:
  - "${PROXY_BIND_ADDRESS:-0.0.0.0}:${PROXY_HTTP_PORT:-80}:80"
  - "${PROXY_BIND_ADDRESS:-0.0.0.0}:${PROXY_HTTPS_PORT:-443}:443"
```

**If left at defaults, this binds `0.0.0.0:80` and `0.0.0.0:443` — exactly the ports 1Panel conventionally claims for its own dashboard/reverse-proxy.** This is a real collision risk, not a hypothetical one.

**Recommended fix (env values only, zero code change):** set `PROXY_BIND_ADDRESS=127.0.0.1` and non-80/443 host ports (e.g. `PROXY_HTTP_PORT=18080`, `PROXY_HTTPS_PORT=18443`) in the VM's env file, then add **one new ingress rule** to the *existing* Cloudflare Tunnel configuration pointing the three HelloDoctor hostnames at `localhost:18443` (exact scheme/port depends on the tunnel's current `originRequest` TLS settings — not visible from this repo). Do not touch 1Panel's own port bindings or any other existing tunnel ingress rule.

**Requires verification on the VM before any value is finalized:** run `sudo ss -tlnp | grep -E ':(80|443|8888|9999)\b'` to see what's actually listening before picking replacement ports — do not assume 80/443 are free, and do not assume 1Panel uses exactly 8888/9999 either.

---

## 7. Environment-variable inventory (no live secrets)

Variables `compose.yml` requires via fail-loud `${VAR:?message}` syntax (names only — no values):

| Variable | Purpose |
|---|---|
| `POSTGRES_USER`, `POSTGRES_DB`, `POSTGRES_PASSWORD` | Database bootstrap credentials |
| `BACKEND_RUNTIME_IMAGE`, `ADMIN_WEB_IMAGE`, `DOCTOR_WEB_IMAGE` | Digest-pinned image references (this release's deliverable) |
| `BACKEND_ENV_FILE` | Absolute path to the backend's own secret env file (not itself in Compose — a separate file the operator maintains) |
| `API_HOST`, `ADMIN_HOST`, `DOCTOR_HOST` | Caddy routing hostnames |
| `WEB_PROXY_SECRET` | Shared secret between Caddy and the web apps |
| `TURNSTILE_SITE_KEY`, `TURNSTILE_SECRET_KEY` | Bot-defense challenge keys |
| `WEB_AUTH_ASSERTION_SECRET` | Web-session assertion signing secret |
| `ACME_EMAIL` | Let's Encrypt contact for Caddy's automatic TLS |
| `POSTGRES_VOLUME`, `REDIS_VOLUME`, `CADDY_DATA_VOLUME`, `CADDY_CONFIG_VOLUME` | External (pre-provisioned, not Compose-created) volume names |
| `ERASURE_JOURNAL_DIR` | Filesystem path for the DR erasure journal |
| `PROXY_BIND_ADDRESS`, `PROXY_HTTP_PORT`, `PROXY_HTTPS_PORT` | Ingress bind address/ports — see §6 |

Plus, inside `BACKEND_ENV_FILE` (per prior audits, names only): the five `DB_*_ROLE_PASSWORD` values, `AZURE_EMAIL_ENDPOINT`/`AZURE_EMAIL_SENDER`/`AZURE_EMAIL_AUTH_MODE`/`AZURE_EMAIL_ACCESS_KEY`, `AZURE_STORAGE_*` values, `ENVIRONMENT` (must be `production` or `staging`, never `development`), `SEED_DEV_DATA=false`, `DEV_AUTH_OTP_ENABLED=false`, `PASSWORD_RESET_ENABLED` (per its own documented production-activation gate).

For the backup overlay (`compose.backup.yml`): `POSTGRES_BACKUP_IMAGE`, `PGBACKREST_REPO1_TYPE=azure`, backup-repo Azure account/container names, `PGBACKREST_REPO1_AZURE_KEY_TYPE=auto` (managed identity — do not override), `PGBACKREST_REPO1_CIPHER_PASS_FILE` (preferred over the plain env-var variant), `PGBACKREST_REPO1_RETENTION_FULL=7`/`_DIFF=14`.

**No live value is reproduced anywhere in this document.**

---

## 8. Payment/booking guard — explicitly unchanged, and that's correct

Confirmed directly: `appointment_service.rs:46-48` and `webhook_handlers.rs:161-165` still unconditionally gate real booking/payment-webhook confirmation behind `environment != "development"`. This release makes **zero changes** to that behavior. For a synthetic-data, internal pre-launch deployment, this is the correct and desired state — it means no fabricated "confirmed" booking/payment can occur in the deployed environment. If the eventual `ENVIRONMENT` value set on the VM is anything other than exactly `development`, booking and payment-webhook confirmation will correctly return `409` every time, consistent with "synthetic data only."

---

## 9. Exact sequence of Git operations, workflow execution, registry publication, digest collection, VM pulls, Compose validation, startup, rollback and health checks

This is the proposed sequence. **Every numbered step below requires your separate, explicit go-ahead before execution** — nothing in this list has been run.

1. **Stage the reviewed file set** (§1.2 whole-file list + §1.5's full `2026020{2..9}` migration block), using `git add -p` for `notification_scheduler.rs` and `admin_handlers.rs` to pick only the hunks named in §1.3.
2. **Review the staged diff** (`git diff --cached --stat` then a full read) to confirm nothing from the excluded/unrelated bucket slipped in.
3. **Commit** with a message describing exactly this scope (ARM64 deployment + email Phase 0/1B + password-reset Phase 2) — a new commit, not an amend.
4. **Push the branch** (or open a PR against `main`, your choice) — only after you approve step 3's commit content.
5. **Add the new `arm64-ghcr-publish.yml` workflow** (§3) in its own commit/PR, reviewed separately from the application-code commit.
6. **Add the minimal `docker login` + `packages: read` fix** to `arm64-disposable-integration.yml` (§3.1), same review discipline.
7. **Manually trigger `arm64-ghcr-publish.yml`** via `workflow_dispatch` once merged — this is the actual publish action; it requires your explicit authorization at the time, separate from authorizing the code changes above.
8. **Collect the four resulting digests** from the workflow's `$GITHUB_STEP_SUMMARY` output; record them in `deploy/single-vm/arm64-images.env.example`'s real (non-template) counterpart env file.
9. **On the VM:** configure read-only GHCR auth (§5), then `docker pull` all four digest-pinned images and verify each via `docker image inspect --format '{{index .RepoDigests 0}}'` matches the recorded digest exactly.
10. **On the VM:** set `PROXY_BIND_ADDRESS`/`PROXY_HTTP_PORT`/`PROXY_HTTPS_PORT` per §6's verified-safe values, confirm external volumes already exist (`docker volume inspect`), then `docker compose --env-file <env> -f deploy/single-vm/compose.yml up -d`.
11. **Trigger `arm64-disposable-integration.yml`** (now fixed per step 6) against the same real digests as an independent six-service smoke test, separate from the live VM.
12. **Health checks:** `docker compose ... ps` (all six healthy), `curl` against `/healthz` through the new proxy port, confirm `debug_otp_code` is absent from a real login response, confirm Postgres/Redis remain unreachable via `ss -tlnp` from outside the VM.
13. **Rollback path, if needed at any point:** `docker compose --env-file <env> -f deploy/single-vm/compose.yml down` (never `-v`) — external volumes are untouched since they're declared `external: true` and owned outside this Compose project.

---

## 10. Remaining blockers

1. **`<OWNER>` (real GHCR namespace) is unknown** — needs to be supplied before any workflow file can be finalized.
2. **The read-only GHCR PAT does not exist yet** — must be generated by the operator out-of-band; never invented here.
3. **The real VM's current port usage (1Panel, anything else on 80/443/8888/9999) is unverified** — must be checked with `ss -tlnp` on the VM before finalizing `PROXY_HTTP_PORT`/`PROXY_HTTPS_PORT`.
4. **The existing Cloudflare Tunnel's ingress config is not visible from this repo** — the new hostname rule must be added by whoever administers that tunnel.
5. **`POSTGRES_BACKUP_IMAGE`'s digest has never been real** — only a local ad hoc tag has ever been built; this is the 4th image this release needs to actually publish for the first time.
6. **A decision on §1.4's `deploy/single-vm/backup/*` inclusion** is needed from you — these scripts are needed if the pgBackRest image's *configuration* (not just the image build) is part of this release.
7. **Nothing in this document has been staged, committed, built-and-pushed, or deployed.** §9 is a proposal; each step needs separate authorization at the time you want it executed.

---

*A companion operator-facing document, written for execution via the 1Panel Terminal on the existing VM across four phases (registry pull/startup, Azure Blob/Key Vault/pgBackRest, ACS email, final verification), has been prepared separately per your Azure Deployment Operator Guidance request.*
