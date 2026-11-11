# Milestone 8.9 — Release Source Reconciliation

**Status:** Read-only investigation. No file modified, nothing staged, nothing committed, nothing pushed. The primary working tree's ~371 uncommitted files and the clean release-candidate worktree are both untouched.
**Verified current main:** `aadcb77aa52311fa343f6947ba08397598b5caa9` (PR #7 merge `b9d72dea` + PR #8's CI fix on top), confirmed by direct `git fetch`.

---

## 1. External build-check investigation (resolved)

The persistent `build` / `deploy` / `report-build-status` check-run trio seen on recent commits is **GitHub's own built-in Pages-deployment workflow** (`name: "pages build and deployment"`, `path: dynamic/pages/pages-build-deployment`, `event: dynamic`) — not a `.github/workflows/*.yml` file in this repo at all. It fails at a "Build with Jekyll" step, almost certainly because this repository isn't Jekyll-shaped content. Confirmed via the GitHub Actions run API, not guessed.

**It is not a required release gate**: `main` has no branch protection configured at all (`GET .../branches/main/protection` → 404 "Branch not protected"), so nothing is enforced by GitHub itself. It's a GitHub Pages platform feature, unrelated to the application. Per your instruction, not modified — leave it as-is, or disable GitHub Pages in repo Settings separately if the noise is unwelcome.

---

## 2. What PR #7 already integrated (confirmed, not re-verified from scratch — this is what I built and validated earlier this session)

- Branded Patient/Doctor/Admin login-OTP emails (full cutover from plaintext)
- Transactional email outbox + dispatch worker (Phase 0/1B) — `email_templates.rs`, `email_transport.rs`, `email_dispatch_worker.rs`, `email_outbox_repo.rs`, migrations `20261205`-`20261209`
- Self-service password recovery (Phase 2), 4 Flutter pages × both apps
- Admin TOTP removal
- ARM64 deployment: all 5 Docker images buildable/verified, `deploy/single-vm/` compose/Dockerfiles, backup scripts (`alert-notify.sh`, `backup-entrypoint.sh`, `backup-monitor.sh`, `backup-runner.sh`, `pgbackrest.Dockerfile`, `pgbackrest-conf-common.sh`, `fetch-keyvault-secret.sh`, `postgres-entrypoint.sh`), hardened `backend-migrations.yml` CI job
- **As reported shared-infrastructure dependencies, also already on main**: the full Azure Blob multi-provider storage implementation (`build_storage()`, `StorageProvider` enum, all `AZURE_STORAGE_*` config) in `repository/mod.rs`/`config.rs`, `RECOVERY_MODE` config plumbing, `domain/models.rs` additions, `user_repo.rs`/`session_repo.rs` additions, and the small `reconciliation_required` finance migration (`20261130000001`)
- Migrations `20261202000001` through `20261209000001` (15 pairs, contiguous)

Housekeeping: PR #8's CI-gate fix (merged, `aadcb77a`).

---

## 3. Exact missing-file/change manifest

### Backend (`services/backend`)

| Area | Files | Status |
|---|---|---|
| Azure Blob test coverage | `tests/azure_storage.rs`, `tests/recovery_blob.rs` | Genuinely needed — the already-shipped Blob feature has no test coverage on main yet |
| Notification/presence platform | `notification_scheduler.rs` (+716 remaining lines: hold/greeting/health-education/holiday passes), `notification_transport.rs` (+337), `notification_repo.rs` (heavily rewritten), `consultation_presence_service.rs` (new), `chat_service.rs`, `global_guard.rs` | Genuinely needed, substantial — own integration track |
| Migrations | `20261130000002_durable_account_erasure`, `20261130000003_notification_delivery_ownership`, `20261201000001_dr_erasure_reconciliation` | Genuinely needed — **see migration-ordering risk, §4** |
| Admission control / query performance | `admission.rs` (+411), `telemetry/mod.rs`, `crypto.rs`, test suites `admission_control_fairness.rs`, `admin_query_g20_investigation.rs`, `consultation_live_reservation.rs` | Genuinely needed, substantial — own integration track |
| DR erasure journal | `dr_erasure_journal.rs` (635 lines), `dr_reconciliation.rs` (364 lines) + its docs (`DISASTER_RECOVERY_ARCHITECTURE.md`, `BACKUP_RESTORE_RUNBOOK.md`, `BACKUP_ENCRYPTION_KEY_CUSTODY_RUNBOOK.md`, `AZURE_BLOB_SETUP.md`) | Genuinely needed as one cohesive unit with the `20261201000001` migration above |
| Finance/remediation | `data_rights_service.rs`, `grievance_service.rs` remaining hunks, `transaction_repo.rs`'s `list_in_range`/`funded_volume_totals`/`finance_summary_totals`, `remediation_a/c/d/e1.rs` test files | Genuinely needed, ties to the finance-projection admin-web work (§3, Web) |
| Dev/benchmark tooling | `docker-compose.configA.yml`, `*.phonetest.*`, `examples/capacity_test.rs`, `dr_reconcile.rs`, `rtc_token_scenario.rs`, `web_workload.rs`, `clinical_doc_test.rs`, various `scripts/*.ps1` | **Obsolete/local-only** — not release-blocking, don't integrate |
| Report artifacts | `docs/MILESTONE_8_*.md`, `docs/REMEDIATION_*.md`, `docs/recovery-evidence/`, `docs/performance-evidence/`, `ops/` | Not application code; several are the authoritative design docs for the items above and should travel with their matching PR |

### Flutter (`apps/mobile`, `apps/mobile_patient`)

| Area | Files | Status |
|---|---|---|
| Push-notification navigation | `notification_taps.dart`, `notification_destination.dart`, `notification_navigation.dart`, `notification_appointment_page.dart` + tests, `core/l10n/`, `l10n.yaml` | Genuinely needed, appears complete — turns a push tap into validated in-app navigation, never trusts push-payload content as navigation authority |
| Clinical PDF viewer ("Remediation E2") | `clinical_pdf_viewer.dart` (both apps) + tests | Genuinely needed — real security remediation: keeps clinical PDFs in-app (decoded in memory, no temp file, no external-app hand-off) instead of leaking them into another app's cache via `launchUrl` |
| Blog/article discovery | `blog_discovery_controller.dart` + tests (patient app) | Genuinely needed |
| Android `fieldtest` build type | `build.gradle.kts`, `src/fieldtest/` (both apps) | Optional dev-tooling (LAN test build + APK-size trim), low priority |
| **`MainActivity.kt` — `FLAG_SECURE` removed** (Doctor app) | — | **Flagging explicitly, not silently carrying forward**: the code comment states this was done "at the product owner's explicit request," knowingly reintroducing screenshot/screen-recording capability on clinical screens in every build including release. This is a deliberate decision already on record, not a bug — but it needs your conscious re-confirmation before any integration PR ships it, since it's a real PHI-exposure trade-off. |

### Web (`apps/admin-web`, `apps/doctor-web`)

| Area | Files | Status |
|---|---|---|
| Genuinely new | `api/session/route.ts`, `components/navigation/NavigationGuard.tsx`, `lib/dataRightsOutcome.ts`, `lib/financeProjection.ts` + tests | Needed — `financeProjection.ts` specifically requires the backend `transaction_repo.rs` finance functions above to actually function |
| **Do not carry forward** | `apps/admin-web/.dockerignore`, `apps/doctor-web/.dockerignore` (local versions) | Superseded — `origin/main`'s already-merged allow-list versions (from PR #5) are the reviewed ones |
| Tooling-only | `tests/browser/`, `editor-concurrency.test.tsx`, `eslint.config.mjs` | Low priority, harmless to carry alongside a feature PR touching the same area |

---

## 4. Remaining dependencies and migration risks

**Real risk, confirmed directly** (not just the sub-agent's claim — I re-verified): the 3 remaining migrations are timestamped `20261130000002`, `20261130000003`, `20261201000001` — all **earlier** than `20261209000001`, which is already the highest-applied version on `main`. sqlx's migrator will refuse a new file whose version is older than one already recorded as applied (without `--ignore-missing`, which this project's own migration-discipline rule doesn't sanction working around). **These 3 files must be renamed with timestamps after `20261209000001`** before integration, preserving their internal order (`durable_account_erasure` before `dr_erasure_reconciliation`, since the latter FKs into tables the former creates). No reference to the already-merged email/password-reset/finance tables was found in any of the three — no content conflict, only an ordering one.

---

## 5. Tests required for a release candidate (next integration round)

- `tests/azure_storage.rs`, `tests/recovery_blob.rs` — straightforward, add alongside existing Blob code, no new dependency
- Full notification/presence test coverage for the hold/greeting/health-education/holiday passes once re-timestamped and merged
- `admission_control_fairness.rs`, `admin_query_g20_investigation.rs`, `consultation_live_reservation.rs` — already written, need to compile/pass against the renamed migration set
- `remediation_a/c/d/e1.rs` — same
- A disposable-DB migration run exactly like the one performed for PR #7, extended to cover the 3 renamed migrations, to prove the new ordering applies cleanly from scratch
- Flutter: `notification_navigation_test.dart`, `doctor_document_viewer_test.dart`, `health_vault_document_viewer_test.dart`, `article_discovery_test.dart`, `blog_discovery_controller_test.dart` — already written, should run clean once their source files are committed
- Web: `navigation-guard.test.tsx`, `data-rights-outcome.test.ts`, `finance-projection.test.ts` — already written

## 6. Proposed minimal integration PRs

Grouped by genuine feature cohesion, not by file-type, so each PR is independently reviewable and revertable:

1. **Blob test coverage** (`tests/azure_storage.rs`, `tests/recovery_blob.rs`) — trivial, no app-code change, fastest to land.
2. **Notification/presence platform** (backend `notification_scheduler.rs`/`notification_transport.rs`/`notification_repo.rs`/`consultation_presence_service.rs` + Flutter push-navigation files) — these pair naturally since the Flutter navigation code is the client side of the same feature.
3. **Admission control / query performance** (`admission.rs`, `telemetry/mod.rs`, the 3 performance test suites) — independent of everything else, no migration needed.
4. **DR erasure journal + finance remediation** (`dr_erasure_journal.rs`, `dr_reconciliation.rs`, `data_rights_service.rs`, `grievance_service.rs` remainder, `transaction_repo.rs` finance functions, `financeProjection.ts`/`dataRightsOutcome.ts` web files, the 3 renamed migrations) — these are genuinely coupled (erasure journal needs its migration; finance web display needs its backend functions) and should land together.
5. **Clinical PDF viewer + blog discovery** (Flutter, no backend dependency) — can land independently and quickly.
6. **`MainActivity.kt` FLAG_SECURE decision** — deliberately called out as its own line item, not bundled into PR #2 or #5, so it gets an explicit yes/no at review time rather than riding in silently with unrelated notification work.

## 7. Is current `main` suitable for synthetic-data deployment?

**Yes, for the scope PR #7 covers.** Login (all three roles, branded OTP), password recovery, ARM64 container builds, Azure Blob (managed identity), and the backup/pgBackRest container scripts are all present and were validated end-to-end against a disposable database earlier this session (447/0 test result). Booking/payment remains correctly `development`-only by design, exactly as intended for this stage.

**Not yet complete** if the internal demo needs: push-notification deep-linking, in-app clinical PDF viewing, blog discovery, admission-control fairness under load, or GDPR-style data-erasure — those are the items in §3/§6 still sitting uncommitted. None of them block a *first* synthetic-data deployment exercising login/booking-gate/Blob/backup; they're additive scope for a *second* integration round.

---

## Next concrete integration action

**Land integration PR #1 (Blob test coverage) first** — it's the only item with zero migration risk, zero new feature surface, and zero outstanding decision (unlike the `FLAG_SECURE` question or the migration-renaming work). It's a clean, fast way to confirm the integration-PR pattern works smoothly post-fix before tackling the larger, migration-touching PR #4. I can prepare it in a new isolated worktree the same way as the prior two fixes, the moment you authorize it.
