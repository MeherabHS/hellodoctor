# Remediation A: financial integrity and account erasure

## Checkpoint and scope

Starting checkpoint: `main`, `47078ee`, initially clean worktree. The audit references still matched the active paths. No Git history operations, production changes, provider configuration, Azure work or historical data repairs were performed. Only owned disposable PostgreSQL/Redis fixtures were used for mutations and failure injection. The persistent local database inventory was read-only.

**Final result: GREEN for the scoped automated Remediation A application gate.** Local correctness, system consistency and applicable recovery paths have runtime evidence. Provider/device/cloud and deployment acceptance remain explicitly unverified below.

## Independent reproductions and corrections

| Defect | Reproduced behavior | Correction |
| --- | --- | --- |
| A1 | Unpaid hold expired and released its slot while the transaction stayed INITIATED, blocking close. | New checkouts explicitly record NOT_STARTED provider provenance. Expiry atomically retires only proven unsubmitted obligations to existing FAILED state, with an audit event. Historical UNKNOWN and provider STARTED attempts remain unresolved. |
| A2 | Finance completion left the transaction REFUND_PENDING and grievance REFUND_APPROVED despite the request becoming REFUNDED. | Authorized completion locks the records and atomically transitions request, transaction and grievance, audit and notification outbox. Approval alone is not completion. Wallet reversal remains at approval and is not repeated. |
| A3 | Injected linked-account deactivation failure still returned HTTP 200 after profile mutation. | A durable erasure journal with immutable subject/media identity records intent first. Role-separated phases disable login/revoke sessions, disable devices, scrub the profile and delete its owned avatar. HTTP 202/PENDING persists until every required phase succeeds. |

The three corrected isolated reproductions failed on their intended assertions before the implementation changes. Their logs remain under `scratch/remediation-a/reproduction-corrected.*`.

An intermediate reporting verification caught two remaining total-calculation failures (16 passed, 2 failed); both were corrected without weakening assertions, and the failed evidence is retained.

Additional reporting reproductions found that backend totals included unfunded checkouts and completed refunds, and the Admin mapper classified both as held escrow. Current funded-volume projections now include PAYMENT_HELD, SETTLED_TO_DOCTOR, DISBURSED and REFUND_PENDING; INITIATED, FAILED and REFUNDED_TO_PATIENT do not contribute. Booking counts remain booking counts. This does not rewrite transaction amounts, append-only economic evidence or finalized statements. Admin projections distinguish unfunded attempts, held funds, pending refunds, settlement, payouts and completed refunds; unknown states fail visibly instead of silently becoming inflows.

## Cross-module acceptance map

| Area | Records/services/contracts that must agree | Recovery evidence and preserved invariant |
| --- | --- | --- |
| Booking expiry/payment | Appointment, slot, transaction, provider-event identity, economic-effect guard, trace/audit, financial-close guard and Admin totals | Expiry/payment race and duplicate late callback tests preserve captured funds without resurrecting a released booking or clinical session. Late captured funds remain PAYMENT_HELD with reconciliation_required. UNKNOWN/STARTED and real held obligations still block close. |
| Refund | Refund request, transaction, grievance, wallet, economic-effect evidence, audit, notification outbox and financial read models | Completion failure injection rolls back related records and notification; concurrent completion has one terminal effect. Provider failure never becomes successful payment/refund. Existing approval wallet reversal is retained. |
| Erasure | Durable job, linked user, auth sessions, device registrations, profile/avatar metadata and object, retained clinical/financial records, Admin response and Doctor/Patient auth clients | Failed login/profile/media phases remain pending and recover idempotently. RLS freezes in-flight profile edits after intent. Duplicate requests share one job. Guardian/unrelated accounts remain unaffected; clinical objects are never treated as avatars. |

RLS, ABAC, capability pools, production deadlines and encryption remain intact. No new worker role or SECURITY DEFINER bypass was introduced. Recovery uses a bounded pass in the existing lifecycle scheduler; notification delivery and its lease logic were not changed. Erasure phase/claim ownership and immutable job identity prevent stale workers from advancing a different claim.

The affected-module review also found a delayed, already-authenticated device-registration request could re-enable a token after erasure's device-disabling phase. The pre-fix regression accepted a new enabled registration after completed erasure; the shared account-lock/erasure-intent guard now covers that identity boundary. This is a registration correction, not a delivery-worker rewrite. Both delayed-registration and concurrent-registration regressions pass in focused and full runs.

## Database and rollout

- `20261130000001_remediation_a_finance`: additive provider provenance and reconciliation marker/index. Existing rows default to UNKNOWN, with no guessed historical financial-state backfill. Down migration refuses outstanding reconciliation markers.
- `20261130000002_durable_account_erasure`: FORCE-RLS journal, narrowly scoped role/column grants, immutable subject/media identity, ordered phases, and restrictive profile-update freeze policies. Down migration refuses incomplete jobs.

State transitions:

- Proven unsubmitted expiry: INITIATED ? FAILED, with no fabricated economic effect. Late provider confirmation: INITIATED/FAILED ? PAYMENT_HELD plus reconciliation_required when the reservation cannot be confirmed.
- Authorized refund completion: request ? REFUNDED, transaction REFUND_PENDING ? REFUNDED_TO_PATIENT, grievance REFUND_APPROVED ? REFUNDED, committed together. Failed execution retains the pending financial obligation and approved grievance decision.
- Erasure recovery: PENDING_LOGIN ? PENDING_PROFILE ? PENDING_MEDIA ? COMPLETED. Login/session changes commit with the first phase advance; profile disposition commits with the next; owned-media deletion and completion audit precede the terminal commit. Devices are disabled before profile scrubbing and registration is blocked from durable intent onward.

Drain old writers and apply migrations before the new binary. Existing migration checksums are unchanged. A future real payment adapter must durably record STARTED before any external request, including uncertain outcomes. Current payment initiation remains explicitly unavailable.

Read-only inventory of local persistent development data found **0** stranded expired/cancelled INITIATED obligations, **0** INITIATED transactions and **0** completed-refund state mismatches. The fixture base database had no application schema; individual SQLx databases are created and migrated per test. This is not an inventory of staging or production. Historical erasure/account mapping and any other environment's inconsistencies require a separately reviewed repair. See [reconciliation and rollout guide](REMEDIATION_A_RECONCILIATION.md).

## Verification

Current final-source focused verification: **21 passed, 0 failed** (5 booking/payment, 4 refund and 12 erasure/identity cases). Admin Web: **109 passed, 0 failed** (95 native tests and 14 component tests), typecheck/build pass, ESLint has 0 errors and the existing Campaign warning. Targeted Doctor auth: **12 passed**; Patient auth: **13 passed**; no Dart changes or fresh full Flutter/device claim.

Final-source backend prerequisites and the full serialized run completed successfully. Evidence lives under `scratch/remediation-a/`; logs do not form permission to repair historical data.

| Final-source check | Actual result |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo check --all-targets` | PASS |
| `cargo test --lib` | 91 passed, 0 failed |
| Focused `remediation_a::` | 21 passed, 0 failed |
| Existing refund filter | 7 passed, 0 failed; overlaps focused/full counts |
| Existing data-rights filter | 6 passed, 0 failed; included in full suite |
| Schedule-exclusivity module | 15 passed, 0 failed; included in full suite |
| Admin Web typecheck / lint / build | PASS; 0 lint errors, 1 existing Campaign warning |
| Admin Web suite | 109 passed, 0 failed: 95 native + 14 component tests |
| Doctor targeted auth tests | 12 passed, 0 failed |
| Patient targeted auth tests | 13 passed, 0 failed |
| Full serialized backend | **419 passed, 0 failed, 4 existing benchmarks ignored**; native exit 0, 1196.53 seconds, 40-minute cap, source hash unchanged |

Focused and full counts overlap and must not be added as unique tests. The first Admin test launch omitted its mandatory disposable Redis environment and failed the fixture guard; the corrected launch passed with that guard unchanged. Native Node module-format warnings and existing Next/React deprecation notices were not remediated as unrelated work.

The historical 6D full result (378 passes, one acquisition timeout, four ignored) remains recorded. The exact failed case was `test_cms_abandoned_media_cleanup_preserves_current_and_clinical_objects`, at `tests/cases/admin_articles.rs:781`, failing the existing 250 ms connection-acquisition guard before cleanup assertions. It passed individually and the subsequent 6F full run passed 398/0/4. Fixture role pools have two connections and 100 ms idle reaping; production acquisition deadlines and pool settings were not altered here.

Remediation A preliminary full launches are retained separately: an invalid synthetic role-password format was rejected by the existing fixture guard, then corrected to the required hex format without changing the guard; a later run was deliberately interrupted after 136 reported passes and no reported failures to add the financial reporting correction. Neither is a completed regression pass. The final coordinated run used the finished source without overlapping fixture-mutating suites. Its source digest matched before/after: `0352700e53a0d49e33506a506624ff9908a21fbf6ebccdb4ea4aa88dff454fc7`. The historical acquisition timeout did not recur.

## Limitations

Real provider payment/refund execution, physical device session behavior, Azure connectivity, provider version/backups/legal-hold deletion and production deployment are unverified. Worker restart coverage simulates a persisted abandoned claim and recreates the worker context; it does not kill/relaunch a deployed OS process. Finance completion continues to rely on the existing authorized confirmation contract and provider reference; this task does not implement a live money-transfer adapter. Avatar completion confirms the configured storage API's deletion, not erasure of infrastructure backups. No notification was sent to a real device. Other audit findings and Remediation B are outside this task.

The mandatory local-correctness/system-consistency/failure-recovery gate is now recorded in root AGENTS.md, the Definition of Done and Testing Strategy.

## Exact changed files

- `AGENTS.md`
- `apps/admin-web/src/app/page.tsx`
- `apps/admin-web/src/data/adminStore.ts`
- `apps/admin-web/src/lib/api.ts`
- `apps/admin-web/src/lib/dataRightsOutcome.ts`
- `apps/admin-web/src/lib/financeProjection.ts`
- `apps/admin-web/src/lib/useBackendSync.ts`
- `apps/admin-web/tests/data-rights-outcome.test.ts`
- `apps/admin-web/tests/finance-projection.test.ts`
- `docs/CHECKPOINT.md`
- `docs/DEFINITION_OF_DONE.md`
- `docs/PROGRESS.md`
- `docs/REMEDIATION_A_RECONCILIATION.md`
- `docs/REMEDIATION_A_REPORT.md`
- `docs/TESTING_STRATEGY.md`
- `services/backend/migrations/20261130000001_remediation_a_finance.down.sql`
- `services/backend/migrations/20261130000001_remediation_a_finance.up.sql`
- `services/backend/migrations/20261130000002_durable_account_erasure.down.sql`
- `services/backend/migrations/20261130000002_durable_account_erasure.up.sql`
- `services/backend/src/api/v1/admin_handlers.rs`
- `services/backend/src/domain/models.rs`
- `services/backend/src/repository/appointment_repo.rs`
- `services/backend/src/repository/transaction_repo.rs`
- `services/backend/src/services/appointment_service.rs`
- `services/backend/src/services/data_rights_service.rs`
- `services/backend/src/services/grievance_service.rs`
- `services/backend/src/services/notification_scheduler.rs`
- `services/backend/src/services/notification_service.rs`
- `services/backend/src/services/profile_picture_service.rs`
- `services/backend/tests/api_tests.rs`
- `services/backend/tests/cases/remediation_a.rs`

Disposable runner scripts, test logs and inventory aggregates are ignored local evidence under `scratch/remediation-a/`; they are not staged.

## Added focused regressions

- `a1_unpaid_expiry_allows_accounting_close`
- `a2_refund_completion_updates_all_authoritative_records`
- `a3_failed_login_deactivation_is_not_completed_erasure`
- `a1_unknown_and_provider_started_attempts_keep_close_blocked`
- `a1_late_and_duplicate_callbacks_preserve_captured_funds_after_close`
- `a1_payment_and_expiry_race_keeps_one_authoritative_obligation`
- `a2_completion_failure_rolls_back_every_record_and_outbox`
- `a2_concurrent_completion_has_one_terminal_effect`
- `a2_provider_failure_does_not_mark_refund_complete`
- `a3_media_cleanup_failure_is_pending_and_recoverable`
- `a1_captured_live_payment_is_never_retired_by_expiry`
- `a3_profile_failure_keeps_durable_progress_and_account_disabled`
- `a3_erasure_retains_clinical_history_and_disposes_personal_avatar`
- `a3_invalid_avatar_reference_never_deletes_clinical_objects`
- `a3_concurrent_requests_share_one_journal_and_completion`
- `a3_job_rls_and_subject_immutability`
- `a3_dependent_erasure_never_deactivates_guardian`
- `a3_registration_racing_erasure_leaves_no_enabled_tokens`
- `a3_inflight_device_registration_cannot_reenable_erased_account`
- `a3_restarted_worker_recovers_expired_claim_without_duplicate_erasure`
- `a3_rollback_refuses_to_discard_pending_erasure_jobs`
