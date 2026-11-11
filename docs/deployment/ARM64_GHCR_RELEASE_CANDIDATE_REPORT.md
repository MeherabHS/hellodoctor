# ARM64 GHCR Release Candidate — Preparation and Validation Report

**Status:** Committed locally, in the isolated worktree only. **Stops at the push/PR/publish gate** — nothing has been pushed, no package published, no Azure resource touched, no change made to the primary working tree (`C:\Users\CUBE\Desktop\helodoc old site`) or its 371 unrelated uncommitted files.

**Commit:** `f801f7b3a1b09e33a1cbcd1f0b23d30cff4af398` on branch `release/arm64-ghcr-candidate-20261107`, worktree `C:\Users\CUBE\Desktop\helodoctor-source-integration-20261107`. Worktree status is clean (nothing left uncommitted).

**Isolation method:** an existing, already-clean git worktree was found and reused — `C:\Users\CUBE\Desktop\helodoctor-source-integration-20261107`, checked out at `origin/main`'s tip (`e3488e54`), detached HEAD, zero modifications. A new local branch, `release/arm64-ghcr-candidate-20261107`, was created there. All release content was built in this isolated worktree only.

---

## 0. A material discovery made before any work began

Before touching anything, a check of the primary repo's remotes and branches found that **`origin/main` on the real GitHub remote (`MeherabHS/hellodoctor`) already contains reviewed, human-merged work** (PRs #1, #2, #5, #6) that substantially overlaps this release: a real `arm64-ghcr-publish.yml` (approval-gated via a GitHub Environment, CI-check-verified), a real fix to `arm64-disposable-integration.yml`'s GHCR login gap, a new `arm64-disposable-recovery.yml` workflow, an `arm64-ghcr-visibility-canary.yml` workflow, and a real `docs/ARM64_GHCR_PUBLICATION_HANDOFF.md`. This was surfaced to you directly and you confirmed: **base the release candidate on `origin/main`**, not on the local branch's own `1ee38d1e` tip. Every step below reflects that decision. `docs/deployment/ARM64_GHCR_RELEASE_HANDOFF.md` (written in an earlier step of this engagement) still contains some now-superseded GHCR-workflow design content from before this discovery; treat `docs/ARM64_GHCR_PUBLICATION_HANDOFF.md` (on `origin/main`) as the authoritative publication plan, and this document plus `AZURE_VM_OPERATOR_RUNBOOK.md` as the operational layer on top of it.

---

## 1. Source isolation — what was included, and why

### 1.1 Method

Rather than resetting, stashing, or cleaning the primary working tree, every approved change was identified as either:
- a **tracked, modified file** — extracted via `git diff HEAD -- <path>` against the primary tree and applied with `git apply` inside the worktree, or
- an **untracked, new file** — copied directly, or
- a **mixed file** (approved and unrelated changes interleaved in the same file) — reconstructed by hand, keeping only the approved hunks.

Nothing was staged or committed in the primary working tree at any point. All 371 of its uncommitted files remain exactly as they were.

### 1.2 Clean whole-file inclusions (patch-applied or copied as-is)

**ARM64 deployment & CI** — already present on `origin/main`, no local changes needed: `arm64-compatibility.yml`, `arm64-disposable-integration.yml`, `arm64-disposable-recovery.yml`, `arm64-ghcr-publish.yml`, `arm64-ghcr-visibility-canary.yml`, `deploy/single-vm/*` (compose, Caddyfiles, `.dockerignore`s), `services/backend/Dockerfile` (the TARGETARCH fix already matches local, confirmed functionally identical before being left as-is).

One real ARM64/CI file *did* need a local patch: **`.github/workflows/backend-migrations.yml`** — hardened with ephemeral masked DB-role passwords (replacing a hardcoded `hellodoctor_dev_password`), a job timeout, a `cargo check --all-targets` step, and a pinned toolchain version. Applied cleanly on top of `origin/main`'s own unrelated `actions/checkout` version bump to the same file (non-overlapping lines, verified before applying).

**Email Phase 0/1B** (all untracked/new unless noted): `email_delivery.rs` (modified), `services/email_templates.rs`, `services/email_transport.rs`, `services/email_dispatch_worker.rs`, `repository/email_outbox_repo.rs`, `services/appointment_service.rs` (modified), `services/grievance_service.rs` (modified), `api/v1/trace_handlers.rs` (modified), migrations `20261205`/`20261206`/`20261209`, `tests/cases/email_outbox_delivery.rs`, `transactional_email_dispatch.rs`, `transactional_email_triggers.rs`, `examples/generate_email_previews.rs`, `docs/mail/`, `docs/EMAIL_OUTBOX_RECOVERY_POLICY.md`.

**Password-reset Phase 2**: `services/password_reset_service.rs`, `repository/password_reset_repo.rs`, migrations `20261207`/`20261208`, `tests/cases/password_reset.rs`, and all 4 new Flutter pages (`create_new_password_page.dart`, `forgot_password_page.dart`, `password_reset_success_page.dart`, `reset_otp_verify_page.dart`) in **both** `apps/mobile` and `apps/mobile_patient`.

**Branded login-OTP cutover + admin-TOTP removal** (one cohesive unit, confirmed by full diff read — every hunk is one or the other, nothing unrelated mixed in): the complete `services/auth_service.rs` diff, plus the deletion of `services/admin_totp.rs` and `examples/encrypt_admin_totp.rs`, plus the matching test removal in `tests/cases/security_acceptance.rs` (170 lines of now-obsolete TOTP-enrollment test code removed).

**Shared infrastructure** (included wholesale as *reported, proven dependencies* — see §1.4): `config.rs`, `.env.example`, `repository/mod.rs`, `domain/models.rs`, `repository/user_repo.rs`, `repository/session_repo.rs`.

### 1.3 Files requiring hand-extraction (mixed approved + unrelated content)

Beyond the two files you named, **two more mixed files were found** during integration — reported here explicitly, as asked:

| File | Kept | Excluded |
|---|---|---|
| `services/notification_scheduler.rs` | The `run_email_outbox_expiry_sweep` + `run_transactional_email_dispatch_pass` wiring inside `start()` | A `RECOVERY_MODE` early-return guard, two erasure-journal/data-rights reconciliation calls, and an unrelated `is_still_eligible_for_dispatch` signature refactor (all Remediation-E1/DR work) |
| `services/api/v1/admin_handlers.rs` | The `DOCTOR_APPROVED`/`DOCTOR_REJECTED` email-enqueue hunks inside `review_doctor_professional_registration` | `get_admin_overview`, `list_admin_doctors`, `list_admin_transactions`, `get_finance_summary`, `execute_patient_data_rights`/`execute_doctor_data_rights`, `list_admin_grievances`, `resolve_consultation_recovery` (all M8-G20/finance-projection/DR work) |
| **`services/mod.rs`** *(newly identified this pass)* | `pub mod`/`pub use` declarations for `email_dispatch_worker`, `email_templates`, `email_transport`, `password_reset_service` | `dr_erasure_journal`, `dr_reconciliation` module declarations (would have pulled in the entire DR-erasure feature as an unwanted transitive dependency) |
| **`services/api/router.rs`** *(newly identified this pass)* | The 3 `/auth/password-reset/*` routes and `/auth/change-password` | A `HeaderMap` import + `recovery_status` handler + `/recovery/status` route (DR), `/notifications/:id` (notification platform), `/admin/consultations/live-reservations/release` (single-live-consultation-reservation feature) |

`services/main.rs` was reviewed and **excluded entirely** — both of its changed hunks (a Milestone-8 DB-pool-capacity experiment and the DR recovery-gate boot check) are unrelated, and excluding it required no compensating change elsewhere.

### 1.4 Proven compile/runtime dependencies pulled in beyond the original plan (explicitly reported, per instruction)

Four were found only by letting `cargo check` and the real test run fail and tracing each error to its root cause — this is reported in full, not glossed over:

1. **`repository/mod.rs`, full diff, including an Azure Blob multi-provider storage refactor and `RECOVERY_MODE` wiring for `email_transport`/`notification_transport`.** The three new `AppState` fields the email code needs (`email_transport`, `email_assets`, `email_links`) are constructed in the same function body as this unrelated storage refactor and `RECOVERY_MODE` gating; surgically removing just the unrelated parts would have required rewriting `AppState::new()` from scratch, a materially higher risk for a release candidate than including a few dozen lines of already-additive, backward-compatible storage code. Reported, not silently absorbed.
2. **`domain/models.rs`'s `Transaction.reconciliation_required` field**, needed because the approved `appointment_service.rs` constructs a `Transaction` literal that sets it. This pulled in a tiny (8-line), self-contained migration, **`20261130000001_remediation_a_finance`**, that adds the column — otherwise unrelated finance-reconciliation work, included only for this one column.
3. **`repository/transaction_repo.rs`'s 4 explicit `SELECT ... reconciliation_required ...` column-list edits** (not the whole file — the much larger finance-projection additions in that file, `list_in_range`/`funded_volume_totals`/`finance_summary_totals`, were deliberately left out), needed because adding the field to the `Transaction` struct broke every hand-written column-list query that didn't also list it — a real, `cargo check`-invisible runtime failure (`sqlx::FromRow` deserialization) only caught by actually running the integration tests.
4. **Two missing shell scripts referenced by the already-included `pgbackrest.Dockerfile`**: `deploy/single-vm/backup/fetch-keyvault-secret.sh` and `deploy/single-vm/backup/pgbackrest-conf-common.sh` (plus their `README.md`), which exist locally but not on `origin/main` and were simply omitted from the original plan. Found only by actually attempting the ARM64 image build, which failed with a literal "file not found" until these were added.

---

## 2. Migration chain

Confirmed directly from the SQL bodies (not inferred from filenames): migrations `20261205`→`20261206`→`20261207`→`20261208`→`20261209` form one inseparable chain (each `ALTER`s a constraint or table the previous one created), and the housekeeping migrations `20261202000001`–`20261204000004` are logically independent but chronologically interleaved before them — sqlx applies every file in a single directory in strict timestamp order with no mechanism to skip one in the middle, so **all of 20261202 through 20261209, plus the small standalone `20261130000001`, travel together** as the complete migration set for this release.

**Verified, not assumed**: the full chain was run against a genuinely disposable, freshly created PostgreSQL 16 container (`rc-disposable-pg`, port 55432, destroyed and recreated mid-session to rule out any stale state) — never the existing development database, never any Azure database. `sqlx::test`'s own per-test database creation re-ran the entire migration set from scratch for every one of the 169 + 47 tests below.

---

## 3. Test report

| Check | Result |
|---|---|
| `cargo fmt --check` | **PASS** — zero diffs |
| `cargo clippy --all-targets --locked -- -D warnings` | **PASS** — zero warnings |
| `cargo check --all-targets --locked` | **PASS** — zero errors, after resolving the dependencies in §1.4 |
| `cargo test --lib --locked` (unit tests, no DB) | **PASS — 169/169** |
| Targeted integration suite: `password_reset::*`, `email_outbox_delivery::*`, `transactional_email_dispatch::*`, `transactional_email_triggers::*` (`--test-threads=1`, against the disposable Postgres+Redis) | **PASS — 47/47**, after fixing the two real issues in §1.4 items 2–3 |
| Full `cargo test --test api_tests --locked -- --test-threads=1` (every test in the suite, same disposable DB) | **PASS — 447 passed, 0 failed, 4 ignored, 0 filtered out** (21.6 minutes, well inside the CI job's own 40-minute budget) |
| `apps/admin-web`: `npm ci && npm run build` (production build) | **PASS** — compiled, typechecked, and statically generated cleanly |
| `apps/doctor-web`: `npm ci && npm run build` (production build) | **PASS** — compiled, typechecked, and statically generated cleanly |
| `docker compose -f deploy/single-vm/compose.yml config --quiet` (with synthetic, non-secret placeholder values for every required var) | **PASS** — valid configuration; fail-loud `${VAR:?...}` behavior confirmed working as designed when values are absent |
| ARM64 image build — Rust backend (raw) | **PASS** — `docker image inspect` confirms `arm64` |
| ARM64 image build — backend production runtime wrapper (`deploy/single-vm/backend.Dockerfile`) | **PASS** — confirmed `arm64` |
| ARM64 image build — Admin Web | **PASS** — confirmed `arm64` |
| ARM64 image build — Doctor Web | **PASS** — confirmed `arm64` |
| ARM64 image build — PostgreSQL/pgBackRest | **PASS** — confirmed `arm64`; `pgbackrest version` inside the running image reports `2.60.0` as pinned |
| Secrets sweep over the full staged diff (common key/password/private-key patterns) | **PASS** — no match found |
| `.env.example` review | **PASS** — all changed lines are either empty placeholders or non-secret tunables (e.g. `MAX_ACTIVE_USERS`) |

### 3.1 Full-suite result

The complete run (every module in `tests/api_tests.rs`, not just the targeted subset) finished clean: **447 passed, 0 failed, 4 ignored, 0 filtered out**, in 1297.7s (~21.6 minutes) against the same disposable, freshly migrated PostgreSQL 16 + Redis instances used throughout this validation — never the existing development database. The 4 ignored tests are pre-existing `#[ignore]`-marked cases unrelated to this release (not newly skipped by anything in this candidate). No new failure, flake, or regression was introduced by the 108-file change set.

---

## 4. Exact file manifest

**108 files staged** in the release-candidate worktree (`git add -A`, nothing committed): 85 additions, 21 modifications, 2 deletions. Full `git status --short` output:

```
M  .github/workflows/backend-migrations.yml
A  apps/mobile/lib/features/auth/create_new_password_page.dart
A  apps/mobile/lib/features/auth/forgot_password_page.dart
A  apps/mobile/lib/features/auth/password_reset_success_page.dart
A  apps/mobile/lib/features/auth/reset_otp_verify_page.dart
A  apps/mobile_patient/lib/features/auth/create_new_password_page.dart
A  apps/mobile_patient/lib/features/auth/forgot_password_page.dart
A  apps/mobile_patient/lib/features/auth/password_reset_success_page.dart
A  apps/mobile_patient/lib/features/auth/reset_otp_verify_page.dart
A  deploy/single-vm/backup/README.md
M  deploy/single-vm/backup/backup-entrypoint.sh
M  deploy/single-vm/backup/backup-monitor.sh
M  deploy/single-vm/backup/backup-runner.sh
A  deploy/single-vm/backup/fetch-keyvault-secret.sh
A  deploy/single-vm/backup/pgbackrest-conf-common.sh
M  deploy/single-vm/backup/pgbackrest.Dockerfile
M  deploy/single-vm/backup/postgres-entrypoint.sh
A  docs/EMAIL_OUTBOX_RECOVERY_POLICY.md
A  docs/mail/ (16 files: design brief, content doc, preview HTML x14, SVG hero assets x6, README)
M  services/backend/.env.example
A  services/backend/examples/config_preflight.rs
D  services/backend/examples/encrypt_admin_totp.rs
A  services/backend/examples/generate_email_previews.rs
A  services/backend/migrations/20261130000001_remediation_a_finance.{up,down}.sql
A  services/backend/migrations/20261202000001_fix_stale_admin_role_references.{up,down}.sql
A  services/backend/migrations/20261202000002_remove_admin_totp.{up,down}.sql
A  services/backend/migrations/20261202000003_chat_messages_insert_rls_performance.{up,down}.sql
A  services/backend/migrations/20261202000004_maintenance_job_volatile_timestamp_scans.{up,down}.sql
A  services/backend/migrations/20261202000005_t15_reminder_scan_indexes.{up,down}.sql
A  services/backend/migrations/20261203000001_single_live_consultation_reservation.{up,down}.sql
A  services/backend/migrations/20261204000001_doctor_rls_identity_lookup_performance.{up,down}.sql
A  services/backend/migrations/20261204000002_chat_conversations_doctor_index.{up,down}.sql
A  services/backend/migrations/20261204000003_chat_conversation_last_message_denorm.{up,down}.sql
A  services/backend/migrations/20261204000004_consultation_sessions_monitoring_index.{up,down}.sql
A  services/backend/migrations/20261205000001_email_outbox.{up,down}.sql
A  services/backend/migrations/20261206000001_transactional_email_types.{up,down}.sql
A  services/backend/migrations/20261207000001_password_reset.{up,down}.sql
A  services/backend/migrations/20261208000001_password_reset_requests_user_id_unique.{up,down}.sql
A  services/backend/migrations/20261209000001_transactional_email_dispatch_worker.{up,down}.sql
M  services/backend/src/api/router.rs
M  services/backend/src/api/v1/admin_handlers.rs
M  services/backend/src/api/v1/auth_handlers.rs
M  services/backend/src/api/v1/trace_handlers.rs
M  services/backend/src/config.rs
M  services/backend/src/domain/models.rs
M  services/backend/src/email_delivery.rs
A  services/backend/src/repository/email_outbox_repo.rs
M  services/backend/src/repository/mod.rs
A  services/backend/src/repository/password_reset_repo.rs
M  services/backend/src/repository/session_repo.rs
M  services/backend/src/repository/transaction_repo.rs
M  services/backend/src/repository/user_repo.rs
D  services/backend/src/services/admin_totp.rs
M  services/backend/src/services/appointment_service.rs
M  services/backend/src/services/auth_service.rs
A  services/backend/src/services/email_dispatch_worker.rs
A  services/backend/src/services/email_templates.rs
A  services/backend/src/services/email_transport.rs
M  services/backend/src/services/grievance_service.rs
M  services/backend/src/services/mod.rs
M  services/backend/src/services/notification_scheduler.rs
A  services/backend/src/services/password_reset_service.rs
M  services/backend/tests/api_tests.rs
A  services/backend/tests/cases/email_outbox_delivery.rs
A  services/backend/tests/cases/password_reset.rs
M  services/backend/tests/cases/security_acceptance.rs
A  services/backend/tests/cases/transactional_email_dispatch.rs
A  services/backend/tests/cases/transactional_email_triggers.rs
```

(`tests/api_tests.rs`'s modification also includes removing 7 `mod` declarations for unrelated test files — `admin_query_g20_investigation`, `admission_control_fairness`, `consultation_live_reservation`, `remediation_a`, `remediation_c`, `remediation_d`, `remediation_e1` — whose backing files were deliberately not copied into this release candidate.)

**108 files, 14,042 insertions, 650 deletions** total.

---

## 5. Commit — created

**Six pre-commit checks, all passed:**

1. `git diff --cached --check`: clean. The only hits (80 lines) are intentional Markdown hard-line-break trailing spaces in two prose files, `docs/mail/AFYAMKONONI_EMAIL_CONTENT_V1.md` and `...DESIGN_BRIEF.md` — no code file, no conflict marker.
2. Exactly **108 files** staged, confirmed via `git status --short | wc -l`; nothing left unstaged or untracked. A final secrets/filename sweep (private-key headers, API-token patterns, `.env`/`.pem`/`.key`/`.bak` filenames) found only `fetch-keyvault-secret.sh` — an intentional, already-reported script name, no actual secret content anywhere in the diff.
3. **Admin TOTP removal confirmed complete**: zero remaining references to `admin_totp`/`admin_verify_otp_with_totp`/`AdminTotp` anywhere in `services/backend/src`, `examples`, or `tests`; both files (`services/admin_totp.rs`, `examples/encrypt_admin_totp.rs`) confirmed deleted. Coverage: `test_admin_mfa_login_two_step_verification` and `test_admin_otp_code_cannot_be_replayed` (both pre-existing, now exercising the renamed `admin_verify_otp` path) plus the entire branded-OTP-email and password-reset suites all pass in the 447/0 full run.
4. Full integration suite result re-confirmed: **447 passed, 0 failed, 4 ignored** (§3.1).
5. Migration chain and pgBackRest/backup scripts confirmed intact: all 15 migration pairs (30 files, `20261130000001` + the contiguous `20261202000001`–`20261209000001` block) staged; all 8 backup/pgBackRest files staged (`backup-entrypoint.sh`, `backup-monitor.sh`, `backup-runner.sh`, `fetch-keyvault-secret.sh`, `pgbackrest.Dockerfile`, `pgbackrest-conf-common.sh`, `postgres-entrypoint.sh`, `README.md`).
6. `origin/main` re-fetched immediately before committing: still at `e3488e54`, identical to the worktree branch's merge-base — **no divergence**, nothing new landed upstream since the candidate was built.

**Commit created:**
- **SHA:** `f801f7b3a1b09e33a1cbcd1f0b23d30cff4af398`
- **Branch:** `release/arm64-ghcr-candidate-20261107`
- **Worktree:** `C:\Users\CUBE\Desktop\helodoctor-source-integration-20261107`
- **Worktree status:** clean — nothing left uncommitted
- **Message:** the exact proposed text from the prior draft of this report (Co-Authored-By: Claude Sonnet 5 trailer included)

**Not done, per the stated authorization boundary:** no push, no PR, no GHCR publication trigger, no Azure change, no container deploy.

### 5.1 GHCR publication-workflow prerequisite — discrepancy resolved

The report previously quoted `arm64-ghcr-publish.yml`'s own header comment, which states prerequisite #2 as "this file exists only on `test/ubuntu-arm64-validation` and not on the repository's default branch" and frames opening a bootstrap PR as a still-pending, separate step. Re-verified directly against current state, not just the comment text:

- `git symbolic-ref refs/remotes/origin/HEAD` → `refs/remotes/origin/main`, confirming **`main` is in fact the repository's configured default branch**.
- `git cat-file -e origin/main:.github/workflows/<file>` confirms **all three workflows** (`arm64-ghcr-publish.yml`, `arm64-disposable-integration.yml`, `arm64-disposable-recovery.yml`) **already exist on `origin/main` right now**.

**Resolution:** the comment is stale, not wrong at the time it was written — it describes the state *before* commit `0599e788` (merged via PR #2) added exactly these three files to `main`, and *before* PR #1 additionally merged all the application/infra/compose content the comment assumed would stay separate. Prerequisite #2 is **already satisfied**; the "separate minimal PR, not yet opened" language in my earlier draft was wrong and has been removed. It was never a real outstanding blocker — I misread the comment's own past-tense framing as still-current.

**What genuinely remains unverified** (correctly flagged before, still correctly flagged now): prerequisite #1, the GitHub Environment `arm64-ghcr-publish` with required reviewers configured in repo Settings. This cannot be checked from this local environment — `gh` CLI is not installed here, and the workflow's own comment is explicit that this needs repo-admin-level API or Settings-UI access to confirm. Someone with that access must verify it before the first real dispatch, regardless of anything in this release candidate. Prerequisite #3 (the five named CI checks reporting green for the exact SHA being published) is a structural, self-enforcing gate in the workflow itself — it will simply refuse to proceed if unmet, once this branch is eventually pushed and CI runs.

---

## 6. Remaining blockers

1. **This commit has not been pushed anywhere** — it exists only in the local worktree. Pushing, opening a PR, and merging all require your separate, explicit authorization.
2. **The GitHub Environment `arm64-ghcr-publish`'s required-reviewers configuration is unverified** — needs repo-admin access to GitHub Settings or an authenticated `gh api` call, neither available from this local environment. See §5.1.
3. **GHCR publication itself is gated by CI going green on the exact SHA eventually pushed** — a structural, self-enforcing requirement of `arm64-ghcr-publish.yml`'s own `verify-revision` job, not something to pre-satisfy now.
4. **The booking/payment `environment != "development"` guard is unchanged**, correctly, per your explicit instruction — this release candidate does not and should not alter it.
5. **GHCR publication, VM deployment, and Azure configuration remain untouched**, per the authorization boundary — nothing beyond creating this local commit has been done.
