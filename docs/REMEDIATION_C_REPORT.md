# Remediation C — Notification Delivery Reliability

<!-- VERIFICATION:START -->
Final result: **GREEN — scoped local notification reliability gate**.

| Gate | Actual result |
|---|---|
| Formatting | PASS; exit 0;  |
| Clippy, warnings denied | PASS; exit 0;  |
| Compilation, all targets | PASS; exit 0;  |
| Rust library | PASS; exit 0; test result: ok. 91 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 117.47s |
| Final Remediation C integration | PASS; exit 0; test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 423 filtered out; finished in 153.16s |
| One bounded serialized full backend run | PASS; exit 0; test result: ok. 439 passed; 0 failed; 4 ignored; 0 measured; 0 filtered out; finished in 1372.03s |
| Doctor/Patient DTO execution | 8 checks; exit 0 |

Completed final Rust gates record identical before/after source digests; running gates remain unverified. Commands, deadlines, exit codes and exact summaries are retained in `scratch/remediation-c/*-complete.json` and `full.json`.
<!-- VERIFICATION:END -->

## Inspection and reproductions

Starting checkpoint: `main`, `47078ee0cafada469bdfab0b74baeca01abcfb82`, with legitimate uncommitted Remediation A/B and Rust dependency-security changes. Initial status, diffs and source hashes are retained in `scratch/remediation-c/`.

The architecture remains PostgreSQL `notifications` inbox/outbox, unique event deduplication, transaction savepoints, capability-separated pools, FORCE RLS, Nairobi quiet hours, atomic frequency reservations and the existing FCM HTTP v1 transport. Campaign fan-out remains separate from delivery. Templates and notification copy were not changed.

**C1:** permanent failure/suppression wrote `FAILED` with NULL `next_delivery_at`, but claiming considered NULL immediately eligible. Terminal events could be retried and delivered after preferences changed.

Evidence qualification: the first Rust C1 fixture incorrectly toggled engagement for an OPERATIONAL notification, so its initial failure is not counted as a valid suppression reproduction. The exact original repository claim/permanent-update SQL was subsequently executed against a connection-local PostgreSQL temporary table: terminal setup and subsequent reclaim were asserted, and the transaction was rolled back. No application rows changed. The corrected Rust regression first establishes suppression and zero sends, then proves preference changes cannot resurrect the event. See `c1-legacy-reproduction.*` in the evidence directory.

**C2:** up to fifty notifications received two-minute leases together and were processed sequentially after the claim transaction committed. There was no owner token, and completion checked only ID and DUE status. The original-worker reproduction used two worker tasks, independent database sessions, a controlled fake FCM transport and forced expiry. It demonstrated overlapping sends before implementation.

Additional consistency corrections: inbox reads overwrote delivery outcomes; campaign cancellation erased attempted/in-flight outcomes; deferral needed to distinguish before-contact deferral from interruption after provider contact had already consumed an attempt.

## State machine and ownership

| Event | Durable disposition |
|---|---|
| Eligible new/retry/abandoned item | DUE with a fresh claim token and separate lease deadline |
| Provider acknowledgment | SENT_TO_PROVIDER; accepted timestamp/reference recorded; claim cleared |
| Temporary failure below limit | FAILED with an explicit future retry time |
| Permanent failure or exhausted retries | FAILED with NULL retry time; never reclaimed |
| Preference, frequency or stale-event suppression | SUPPRESSED; terminal, with inbox history retained |
| Quiet hours / paused campaign before provider contact | Deferred with future time; provisional attempt refunded |
| Deferral after provider contact starts | Attempt and uncertainty retained; final attempt terminates |
| Owner reads a pending item | Atomically set read_at, suppress its pending push and fence the claim |
| Owner reads an accepted/terminal item | Set read_at while preserving the actual transport outcome |

The worker claims one row immediately before processing. A materialized single-candidate query takes its row lock and session advisory lock. The same existing system-role connection performs subsequent DB operations, avoiding a second pool acquisition while a worker holds a pool slot. **No transaction remains open while waiting for FCM.** Normal release unlocks before pool reuse; cancellation/error/panic closes the session using the installed SQLx 0.8.6 `close_on_drop` API.

Heartbeats run every ten seconds with a two-minute lease. Each transport call has a 45-second bound; the existing FCM HTTP timeout remains ten seconds. An expired lease can be revived only by the session still holding its advisory lock. Every completion compares ID, claim token, DUE status and a live lease. Token invalidation shares a short transaction with ownership renewal, so a stale worker cannot disable a newer registration.

The five-attempt policy remains. A sixth recovery claim may record exhaustion of an abandoned fifth attempt, but cannot make a sixth provider call. Deferrals before contact consume no attempt; deferrals after contact cannot reset the retry limit. Frequency reservation remains atomic and once per logical notification across retries. New claims stop after the pass's 60-second work budget; a multi-device row can run longer across bounded device calls.

[PostgreSQL advisory-lock documentation](https://www.postgresql.org/docs/16/explicit-locking.html#ADVISORY-LOCKS) describes the session lifetime used here. No new role, pool, broker, queue, Firebase SDK or dependency was introduced.

## Recovery and external-delivery limits

`delivery_uncertain` is committed before transport invocation. Timeout, interrupted work or a failed completion write preserves an uncertain outcome. Acknowledgment records provider acceptance; definite token rejection disables only the matching still-owned registration. An unconfigured transport cannot report success.

This is **not exactly-once external delivery**. FCM acceptance followed by lost acknowledgment/crash can lead to an at-least-once retry. A network partition can end the database session while a remote request is already in flight. Fencing prevents stale database completion, but cannot revoke that remote request. No Firebase idempotency capability is assumed.

Existing success semantics remain per notification: at least one device accepted the message, rather than all-device success, device delivery or a read receipt. Cancellation/read actions stop further work but cannot recall an acceptance already in flight. Existing appointment-reminder cancellation semantics are retained, with the same external limitation.

## Campaign, inbox, reporting and RLS

Live campaign pause/schedule state defers queued delivery. Cancellation retains attempted rows, marks them SUPPRESSED and clears ownership in the same transaction as campaign status and the existing audit. Never-attempted CREATED rows retain previous deletion behavior. Audit-failure injection verifies rollback of the related campaign/outbox changes.

Existing campaign response keys remain; additive fields are `suppressed`, `deferred`, `processing` and `uncertain`. The combined preference/quiet-hours field remains compatible. Queue gauges distinguish pending, processing, retrying, deferred, suppressed, provider-accepted, permanently failed, explicit legacy DELIVERED status and legacy READ. Uncertainty overlaps those dispositions rather than forming a disjoint delivery total. Provider-acceptance and invalid-token counters remain; a separate acknowledged-row counter follows durable fenced completion. Claims are never labeled delivered.

For compatibility, campaign `queued` includes its `processing` subset, and the retained combined field overlaps the new separate suppression/deferral fields. Do not sum overlapping campaign fields as independent outcomes. Frequency `sent_count` remains a reservation counter, not a provider receipt.

FORCE RLS and separated roles remain. The only new policy permits canonical ADMIN updates for notifications belonging to an already-CANCELLED campaign, requiring resulting status SUPPRESSED. No general Patient permission, SECURITY DEFINER bypass or worker role was added. Existing inbox DTOs do not expose tokens, lease deadlines or uncertainty. Both unchanged mobile DTO implementations were executed to verify status/read_at compatibility.

## Exact changed files

- `services/backend/src/repository/notification_repo.rs`: eligibility, claim/lease/session ownership, fenced transitions, uncertainty, read/cancel disposition and queue counts.
- `services/backend/src/services/notification_scheduler.rs`: just-in-time claims, heartbeat/timeout, live checks, attempt-aware deferral and outcome metrics.
- `services/backend/src/repository/campaign_repo.rs`: compatible delivery aggregates with distinct outcome buckets.
- `services/backend/src/api/v1/campaign_handlers.rs`: atomic cancellation/fencing and additive reporting fields.
- `services/backend/migrations/20261130000003_notification_delivery_ownership.up.sql`: additive fields/check/index and narrow cancellation RLS.
- `services/backend/migrations/20261130000003_notification_delivery_ownership.down.sql`: refuse destructive downgrade without reviewed recovery.
- `services/backend/tests/api_tests.rs`: register the new test module only.
- `services/backend/tests/cases/remediation_c.rs`: 20 focused PostgreSQL/API/fake-provider tests.
- `services/backend/tests/cases/push_notification_delivery.rs`: assert explicit suppression for stale dispatch.
- `services/backend/tests/cases/notification_localization_preferences.rs`: assert explicit terminal suppression.
- `services/backend/tests/cases/engagement_and_holiday_notifications.rs`: assert explicit suppression for opted-out greetings.
- `services/backend/tests/cases/admin_campaigns.rs`: assert explicit recipient suppression.
- `docs/REMEDIATION_C_REPORT.md`: this report.

Evidence and harness files are retained under `scratch/remediation-c/`, including `DESIGN.md`, baseline/preservation records, exact command/exit JSON, logs, source digests, original-SQL reproduction and the Dart contract harness. Historical reports/evidence were not deleted or rewritten.

## Migration and deployment policy

The forward migration does not UPDATE/backfill historical notifications or replay previously issued events. Legacy FAILED/NULL rows remain terminal. Only SQLx-owned disposable databases received migrations during verification; no persistent application, staging or production database was migrated.

Before future deployment: drain/stop all old backend instances, including delivery and inbox/campaign mutation handlers; inspect legacy pending/ambiguous rows read-only; apply the additive migration; start only the new protocol. Mixed old/new code is not supported because older workers/handlers do not share the ownership/read semantics. Any historical reconciliation/replay requires separate review. The uncertainty flag records this protocol's attempts; its default on historical rows is not proof that past external outcomes were definitely unsent.

Down migration refuses to discard SUPPRESSED, live claims or uncertainty until a separately reviewed drain/reconciliation plan exists. Forward migrations were exercised on fresh fixtures. The down migration was source-reviewed but not executed. Production table-size/locking timing was not benchmarked here.

## Mandatory cross-module gate

| Gate | Modules/invariant | Evidence |
|---|---|---|
| Local correctness | Eligibility, terminal/retry states, preferences, authorization | New PostgreSQL/API cases and existing notification/campaign tests |
| System consistency | Outbox, campaign/audit, frequency reservation, token state, inbox and client DTO agree | Actual cancellation/pause/revoke/read endpoints, reporting assertions, audit rollback injection and DTO execution |
| Failure recovery | No concurrent live processing or stale completion; truthful uncertainty; recoverable abandoned work | Two workers, expiry/renewal, slow transport, timeout, task abort/restart, duplicate ack, completion/audit failure and final-attempt deferral |
| Preserved business modules | Financial/erasure/consultation records and notification creation retain agreement | Focused Remediation A/refund/erasure/consultation cases and the full serialized suite |

Only owned loopback hd-6d fixtures were used, with label/readiness and Redis database-14 ownership checks before integration runs. Cluster-role-mutating suites were serialized. Production timeouts/pool sizes, RLS and assertions were not weakened. No real Firebase sends or credentials were used.

## Verification history and remaining limits

Earlier focused checkpoints passed 54 notification/campaign cases and then 93 cross-module cases. The 93-case checkpoint preceded the final in-flight-deferral refinement; final focused/full results in the verification block cover it. An early expanded run had 11 passed / 4 failed, exit 101: two fixture errors (wrong preference category and expecting HTTP 201 from the existing HTTP 200 campaign-create contract) caused those four failures. They were corrected without weakening business assertions; logs are retained. The original two-case run exited 101; its C1 qualification is stated above. No tests were deleted or newly ignored.

The historical full-backend result was 419 passed / 0 failed / 4 ignored; it is not reused as current evidence. Historical Admin/Flutter full-suite counts were not rerun or presented as new results. The four existing ignored benchmarks remain separate.

Preservation checks compare pre-existing diffs, reconstruct the shared scheduler/API-test files and verify Remediation A sections, notification_service.rs and Cargo.lock remain intact. Staged changes remain unchanged. Admin CMS/MCQs, Remediation B session/editor controls, payments, erasure, Agora, Firebase configuration, templates, mobile navigation, Azure and GitHub workflows were not edited.

An additional 138-file hash comparison matched protected pre-existing files, including retained Remediation A/B evidence, with zero mismatches. Docker preservation checks retained all five container identities, four running services, original start times/mounts and fourteen volumes; the main PostgreSQL/Redis pair remained healthy. See `protected-ab-files.json`, `preservation.json` and `resource-preservation.json` in the evidence directory.

Remaining external/operational work: actual FCM/device acceptance and a separately authorized coordinated migration/deployment with legacy-outcome review. This scoped local gate does not certify production/provider deployment. No Remediation D, Azure or GitHub work starts automatically.
