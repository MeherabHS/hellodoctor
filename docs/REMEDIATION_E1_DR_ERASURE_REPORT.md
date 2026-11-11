# Remediation E1 — Disaster-Recovery Account-Erasure Integrity

## Result

**GREEN for the scoped Remediation E1 application gate**, fully verified
end-to-end against a real, disposable Postgres/Redis stack. The sandbox
initially had no reachable Docker/Postgres; local source-level correctness
(compilation, `clippy -D warnings`, `cargo fmt --check`, `cargo test --lib`
108/108, `cargo audit` clean, a live manual cross-language interoperability
proof against `ops/dr/erasure_journal.py`) was verified first. The operator
then started the project's Docker stack (`hellodoctor-postgres`,
`hellodoctor-redis`) mid-task, and the full Postgres-backed suite was run
against it, using a disposable, explicitly-owned, non-default-port test Redis
instance per this project's own test-fixture-ownership contract
(`TEST_REDIS_URL`/`TEST_REDIS_FIXTURE_ID`, never the shared dev Redis on
6379) — removed again immediately after:

- The five new Remediation E1 integration tests, including the core
  backdated-restore reproduction, all **passed** on first run against the
  real database (`cargo test --test api_tests remediation_e1 --
  --test-threads=1`: 5 passed, 0 failed).
- The full serialized backend suite (`cargo test --test api_tests --
  --test-threads=1`): **452 passed, 0 failed, 4 ignored** (the 4 are the
  project's existing A/B capacity benchmarks, not part of the regular gate),
  exit code 0 — confirming this change introduced no regression anywhere
  else in the ~450-test suite.

No commits, pushes, resets, or cleans were performed. No Azure resources were
provisioned. No real patient records were touched — all reproduction used
`#[sqlx::test]`'s disposable, per-test throwaway databases, and the one
Redis container created purely to supply the required disposable test
fixture was removed immediately after the run. Remediations A–D and
Milestones 7A–7C.1B are preserved: no existing migration was edited, no
existing test was weakened, and the full existing suite remains green.

## Original defect, re-verified independently

The prior audit pass (the "Adversarial re-verify DR account-resurrection
claim" agent in this engagement) found that a PostgreSQL restore to a point
**before** an account-erasure event is not protected: `check_active_session`'s
live-DB check cannot distinguish "genuinely active" from "resurrected by a
backdated restore," and the one mechanism that could catch this
(`ops/dr/erasure_journal.py`) was never actually called by the application —
real erasure in `data_rights_service.rs` wrote only to Postgres, never to the
journal.

This pass re-verified every one of those claims against current source before
designing a fix, rather than inheriting them:

- Confirmed `data_rights_service::finish_step` (where an erasure job reaches
  `COMPLETED`) had no call into `ops/dr/erasure_journal.py` or any Rust
  equivalent — `grep -rn "erasure_journal" services/backend/src` returned no
  matches before this change.
- Confirmed `RECOVERY_MODE` transitions (`config.rs`, `global_guard.rs`) had
  no dependency on any reconciliation evidence — `recovery_mode: bool` is read
  once at startup from an env var with no DB-backed gate on leaving it.
- Confirmed `ops/dr/erasure_journal.py::reconcile()` is classify-only
  (`ERASED_OR_RETAINED` / `REVIEW_REQUIRED`), never corrective, and is called
  from nothing but its own test file.

### Phase 1 — full inventory of state-changing operations (beyond the original audit's scope)

Grepped every writer of `users.is_active` / `users.deleted_at` /
`auth_sessions.revoked_at` / `notification_device_tokens.enabled` in
`services/backend/src`:

| Writer | Path | In scope for this fix? |
|---|---|---|
| `data_rights_service::deactivate_step` | Patient/Doctor erasure (admin-initiated) | Yes — the only durable, intentional deactivation path; now journaled |
| `data_rights_service::disable_devices_step` | Same erasure flow | Yes — covered by the same outbox entry |
| `auth_service` brute-force/account jail | Temporary session/account lockouts (not `is_active`/`deleted_at`) | No — these are transient rate-limiting states, not erasure; a restore resurrecting a *lockout* only ever makes an account temporarily *more* restricted, never less, so it is not a privacy/security regression and is out of this fix's scope |
| Grievance adjudication | No direct `users`/`auth_sessions` write found; grievance actions affect appointments/refunds, not account activation | No new write path found |

No other durable account-deactivation/erasure path exists in the current
codebase beyond the one already covered by Remediation A's
`account_erasure_jobs` state machine.

## Phase 2 — reproduction

A full reproduction of the attack scenario is encoded as an automated test
(`services/backend/tests/cases/remediation_e1.rs::e1_reconciliation_detects_and_corrects_backdated_restore_then_passes_gate`),
using only `#[sqlx::test]`'s disposable per-test database — never any
persistent or shared database:

1. A real erasure is executed through the actual admin HTTP endpoint
   (`POST /api/v1/admin/patients/:id/data-rights/erase`) against the seeded
   patient.
2. The resulting outbox row is synced to a disposable, temp-directory external
   journal.
3. A backdated restore is simulated directly at the row level — exactly the
   shape a PITR restore to an earlier point would produce: `users.is_active`
   flipped back to `true` and `deleted_at` cleared, a fresh unrevoked
   `auth_sessions` row inserted, and a fresh enabled
   `notification_device_tokens` row inserted.
4. The external journal file is untouched by this simulated restore (it lives
   outside the row-level mutation, modeling it living outside the Postgres
   backup domain).
5. Before reconciliation, `enforce_recovery_gate` is asserted to refuse
   normal-mode startup.
6. `reconcile_after_restore` is run and asserted to detect the drift, correct
   it, and report `PASSED`.
7. After reconciliation, the resurrected session is asserted revoked, the
   resurrected device token disabled, the account deactivated again, and
   `enforce_recovery_gate` now succeeds.
8. Re-running reconciliation is asserted idempotent (reports `PASSED` again
   with nothing left to correct).

This test passed on its first run against the real Postgres/Redis stack
described in **Result** above, confirming every step of the scenario:
detection, correction (session revoked, device token disabled, account
re-deactivated), the gate blocking before correction and allowing after, and
idempotency on re-run.

## Phase 3 — durable journal integration: design and honesty about atomicity

**New table, same transaction as the completion itself**
(`20261201000001_dr_erasure_reconciliation.up.sql`):
`durable_erasure_journal_outbox` — one row per completed erasure, written by
`data_rights_service::finish_step` in the **same Postgres transaction** that
flips the job to `COMPLETED`. This guarantees the outbox row and the
completion commit or roll back together — no erasure can complete without its
outbox obligation also being durably recorded, and no outbox row can exist for
an erasure that didn't actually commit.

**The outbox-to-journal hop is explicitly not atomic with the above.** A
background pass (`dr_reconciliation::sync_outbox_to_journal`, wired into the
existing 60-second scheduler loop in `notification_scheduler.rs`, the same
place `retry_pending_erasures` already runs) reads unsynced outbox rows and
appends them to the external journal via a new Rust-native implementation of
the journal's wire format (`services/dr_erasure_journal.rs`). A journal-append
failure (I/O error, integrity error, malformed state) leaves the outbox row
unsynced with an incremented `journal_sync_attempts` counter and a fixed,
non-sensitive diagnostic code — never marks it synced on failure, and never
fabricates a shared commit between Postgres and the external store. This is an
honest outbox pattern, not a disguised two-phase commit.

**Disclosed residual window:** an erasure whose outbox row has not yet been
synced to the external journal at the moment a backup is taken is not
protected — the outbox row itself rolls back along with everything else in
Postgres if restored to before it existed. This narrows, but does not
eliminate, the gap the original audit found; it is bounded by the sync
interval (60 seconds, matching this codebase's existing scheduler cadence) and
should be monitored in production (alert on a growing unsynced-outbox
backlog — the `journal_sync_attempts`/`last_sync_error` columns exist
specifically to make this observable).

**Interoperability, proven live, not just by inspection.** The new Rust
reader/writer (`dr_erasure_journal.rs`) deliberately reimplements the exact
wire format `ops/dr/erasure_journal.py` already defines: append-only JSON
Lines, a separate `.anchor` checkpoint file, HMAC-SHA256 over canonical JSON
with sorted keys and `(",", ":")` separators. This was verified by an actual
cross-process round trip during this pass (not committed to the repository —
a disposable, temporary scratch check, removed afterward):

1. The new Rust code wrote a journal entry.
2. `ops/dr/erasure_journal.py`'s own `ErasureJournal.read_verified()`
   successfully verified it and computed the identical `pseudonymous_subject_ref`.
3. The Python tool then appended a second entry on top of the Rust-written
   HMAC chain.
4. The Rust code re-read the resulting mixed Rust+Python chain and verified it
   end to end.

This means an operator's existing Python DR tooling can inspect, and in a real
incident even directly manipulate for review, a journal this binary wrote, and
vice versa — the production custody model (today a local path; intended end
state, Azure immutable Blob Storage) is not locked to one language's tooling.

**Limitation, stated plainly:** the local filesystem adapter is **not** an
independently-protected store. Restoring the same disk/volume as Postgres
rolls the journal back too. `EXTERNAL_ERASURE_JOURNAL_PATH` must point outside
any Postgres backup domain in production (a separately-backed-up volume today;
Azure immutable Blob Storage with its own retention policy as the intended end
state). This code does not implement Azure immutability and the module's own
doc comment says so explicitly — it is a local-correctness building block, not
a production guarantee.

## Phase 4 — corrective reconciliation

`dr_reconciliation::reconcile_after_restore` (invoked only by the new
operator CLI, see Phase 5):

- Reads and fully verifies the external journal (fails closed on any
  integrity violation — a tampered or truncated journal produces an `Err`,
  never a false `PASSED`; see
  `e1_reconciliation_fails_closed_on_tampered_journal`).
- Computes the pseudonymous `subject_ref` for every currently-active user and
  checks it against the journal's `COMPLETED` set.
- For every match (a resurrected account), re-applies deactivation through
  **existing** authorized mutation code — `user_repo::anonymize_and_deactivate`
  (the same function `deactivate_step` already calls) plus the same
  session-revocation and device-token-disabling SQL `deactivate_step`/
  `disable_devices_step` already use — via two new `pub(crate)` functions
  co-located in `data_rights_service.rs`
  (`reapply_erasure_deactivation_for_resurrected_account`,
  `disable_device_tokens_for_resurrected_account`) rather than a second,
  independently-written deletion path. A resurrected account's
  `account_erasure_jobs` row does not exist in the restored snapshot by
  definition (the restore predates it), so the existing job-claim machinery
  does not apply here; this deviation is deliberate and disclosed, not
  accidental duplication.
- Every corrective action is audit-logged (`DR_RECONCILIATION_REAPPLIED_ERASURE`).
- A journal entry whose state is not `COMPLETED` (e.g. an interrupted
  in-flight erasure) is **never** auto-corrected — it is classified
  `REVIEW_REQUIRED` and blocks the overall outcome from reaching `PASSED`
  (`e1_reconciliation_blocks_on_review_required_journal_entry`).
- Never deletes clinical/financial records — only re-applies the existing
  account-level deactivation scope.
- Idempotent: re-running after a `PASSED` result finds nothing left to correct
  and reports `PASSED` again (asserted in the core reproduction test).

## Phase 5 — recovery-exit gate

- **Entering recovery:** `main.rs` calls `dr_reconciliation::mark_recovery_boot`
  whenever `RECOVERY_MODE=true`, which durably inserts (idempotently, via
  `ON CONFLICT DO NOTHING`) a `dr_recovery_state` row keyed by the new
  required `RECOVERY_INSTANCE_ID` setting, status `AWAITING_RECONCILIATION`.
  This exists in Postgres precisely because it is written while serving the
  actual (just-restored) database for this incident — it cannot be
  independently rolled back relative to the data it is tracking.
- **Leaving recovery:** `main.rs` calls `dr_reconciliation::enforce_recovery_gate`
  whenever `RECOVERY_MODE=false` (i.e., every normal boot — a single cheap
  `SELECT` against a table that stays empty in ordinary operation, so this
  adds no friction to ordinary `cargo run`/dev/test startup). If any recovery
  instance is not `PASSED`, the process panics before serving any route. This
  is a **startup-time** gate, not a runtime-togglable flag, and there is no
  HTTP route anywhere that can mark a recovery instance `PASSED` — only the
  operator-run `dr_reconcile` CLI can, and it independently re-verifies the
  recovery-operator token before doing anything.
- **Database-level backstop:** a new trigger, `protect_dr_recovery_state`,
  rejects any `UPDATE` that would downgrade a row away from `PASSED` — even a
  bug or a compromised `hellodoctor_system_worker` connection cannot silently
  un-pass a recovery instance.
- **Operational-authority limitation, stated explicitly, not overclaimed:**
  this gate protects the ordinary request-serving path. It does **not**
  protect against an administrator with unrestricted infrastructure access who
  could, for example, reconfigure the deployment to skip calling
  `enforce_recovery_gate`, truncate `dr_recovery_state` directly as a
  superuser, or replace the binary entirely. No application-level control can
  prevent that; the real boundary there is operational (deployment-pipeline
  review, infrastructure access control, the separately-custodied journal
  keys), not code.

The operator CLI (`examples/dr_reconcile.rs`) is deliberately **not** an HTTP
endpoint — there is no network-accessible way to pass the gate. It
authenticates with the same constant-time `operator_digest_matches` function
`global_guard.rs`'s HTTP middleware already uses (made `pub` for this reuse,
not reimplemented), requires the raw `RECOVERY_OPERATOR_TOKEN` as an
environment variable, and requires the `--recovery-instance-id` argument to
match the instance the environment actually booted with.

## Phase 6 — negative and failure-recovery test coverage

| # | Scenario | Coverage |
|---|---|---|
| 1 | Normal account erasure | `e1_completed_erasure_writes_and_syncs_outbox_to_external_journal` |
| 2 | Erasure with external journal unavailable | `e1_sync_fails_closed_when_journal_not_configured` |
| 3 | DB failure after journaling an intent | Structurally guaranteed (single transaction for completion + outbox insert), not fault-injection tested — disclosed, not assumed |
| 4 | Journal-write timeout / ambiguous ack | Structurally guaranteed (every non-success outcome is treated identically as "retry later"), not fault-injection tested — disclosed |
| 5 | Duplicate event delivery | `dr_erasure_journal::tests::append_is_idempotent_on_duplicate_event_id` (unit, passing) + re-sync assertion in test #1 |
| 6 | Out-of-order event rejection | `dr_erasure_journal::tests::truncation_is_detected_via_anchor_mismatch` (sequence/anchor mismatch; unit, passing) |
| 7 | Missing checkpoint/anchor | `dr_erasure_journal::tests::missing_anchor_with_present_data_fails_closed` (unit, passing) |
| 8 | HMAC key mismatch | `dr_erasure_journal::tests::hmac_key_mismatch_is_rejected` (unit, passing) |
| 9 | Journal truncation/rollback | `dr_erasure_journal::tests::truncation_is_detected_via_anchor_mismatch` (unit, passing) + `e1_reconciliation_fails_closed_on_tampered_journal` (integration, **passing**) |
| 10 | Backdated database restoration | `e1_reconciliation_detects_and_corrects_backdated_restore_then_passes_gate` (integration, **passing**) |
| 11 | Restored valid session | Same test, asserted and passing |
| 12 | Restored device registration | Same test, asserted and passing |
| 13 | Incomplete reconciliation | `e1_reconciliation_blocks_on_review_required_journal_entry` (integration, **passing**) |
| 14 | Activate NORMAL without verified approval | Same test + core reproduction test, asserted and passing |
| 15 | Successful reconciliation, isolated acceptance | Core reproduction test, asserted and passing |

Unit tests: 8/8 passing. Integration tests: 5/5 passing, run against a real,
disposable Postgres/Redis stack.

## Phase 7 — regression evidence

| Check | Command | Result |
|---|---|---|
| Format | `cargo fmt --all -- --check` | PASS (clean after one auto-format pass) |
| Clippy, warnings denied | `cargo clippy --all-targets -- -D warnings` | PASS |
| Compilation, all targets | `cargo check --all-targets` | PASS |
| Rust library unit tests | `cargo test --lib` | PASS — 108 passed, 0 failed (up from 99) |
| Integration test binary compiles | `cargo test --test api_tests --no-run` | PASS |
| Remediation E1 integration tests | `cargo test --test api_tests remediation_e1 -- --test-threads=1` | PASS — 5 passed, 0 failed (run against real Postgres + a disposable, explicitly-owned test Redis fixture, started by the operator mid-task) |
| Full serialized backend suite | `cargo test --test api_tests -- --test-threads=1` | PASS — **452 passed, 0 failed, 4 ignored** (existing A/B capacity benchmarks), exit 0 |
| Dependency audit | `cargo audit --deny warnings --ignore RUSTSEC-2023-0071` | PASS, exit 0, no new exceptions |
| Cross-language journal interop | manual, temporary scratch binaries (removed) | PASS — Rust↔Python round trip verified live |

The disposable test Redis container created solely to satisfy this project's
`TEST_REDIS_URL`/`TEST_REDIS_FIXTURE_ID` ownership contract (loopback,
non-default port, explicit ownership marker — never the shared dev Redis on
6379) was removed immediately after the run; only the operator's own
pre-existing `hellodoctor-postgres`/`hellodoctor-redis` containers remain.

No existing migration was edited; one new additive migration pair
(`20261201000001_dr_erasure_reconciliation.{up,down}.sql`) was added, with a
down-migration that cleanly drops only the new objects.

## Changed files

- `services/backend/migrations/20261201000001_dr_erasure_reconciliation.{up,down}.sql` (new)
- `services/backend/src/services/dr_erasure_journal.rs` (new)
- `services/backend/src/services/dr_reconciliation.rs` (new)
- `services/backend/examples/dr_reconcile.rs` (new, operator CLI)
- `services/backend/src/services/mod.rs` (register the two new modules)
- `services/backend/src/services/data_rights_service.rs` (outbox insert in
  `finish_step`; two new `pub(crate)` reconciliation-corrective functions;
  `short_suffix` made `pub(crate)` for reuse)
- `services/backend/src/config.rs` (new `RECOVERY_INSTANCE_ID`,
  `EXTERNAL_ERASURE_JOURNAL_{PATH,HMAC_KEY_B64,ANCHOR_KEY_B64,KEY_VERSION}`
  settings and their validation)
- `services/backend/src/main.rs` (startup-time call into
  `mark_recovery_boot`/`enforce_recovery_gate`)
- `services/backend/src/services/notification_scheduler.rs` (one new
  60-second scheduler pass calling `sync_outbox_to_journal`)
- `services/backend/src/api/middleware/global_guard.rs`
  (`operator_digest_matches` made `pub` for reuse by the CLI tool — same
  function, not reimplemented)
- `services/backend/tests/api_tests.rs` (new config fields in `test_config()`
  and `deployed_config()`; one new unit test for the validation logic; module
  registration for the new test file)
- `services/backend/tests/cases/remediation_e1.rs` (new, five integration tests)

## Production Azure requirements (unfilled by this pass, by design)

- An Azure resource (today: a separately-backed-up volume; intended end state:
  an immutable Blob Storage container with its own retention policy) must
  host `EXTERNAL_ERASURE_JOURNAL_PATH`'s target, genuinely outside the
  PostgreSQL backup domain.
- `EXTERNAL_ERASURE_JOURNAL_HMAC_KEY_B64` and `_ANCHOR_KEY_B64` must be
  independently custodied (the validation layer already rejects identical
  values, but true independent custody — e.g., two separate Key Vault
  secrets with different access policies — is an operational decision this
  pass cannot enforce from code).
- `RECOVERY_INSTANCE_ID` must be chosen by the operator per real incident
  (e.g., an incident-tracker ID) and supplied consistently to both the booted
  server and the `dr_reconcile` CLI invocation for that incident.
- The `dr_reconcile` CLI must be added to the disaster-recovery runbook
  (`docs/DISASTER_RECOVERY_PLAN.md`) as a mandatory step between "database
  restored, `RECOVERY_MODE=true`" and "restart with `RECOVERY_MODE=false`" —
  this pass implements the gate but does not yet edit that runbook document
  (left for a future pass, since the runbook's broader DR narrative is a
  larger document this task's scope did not ask to rewrite).
- Done: the integration test suite in `tests/cases/remediation_e1.rs` was run
  to completion against a real Postgres/Redis stack and passed (see
  **Result**). Re-run it again (`cargo test --test api_tests --
  --test-threads=1`) after any future change touching this area, per this
  project's standard regression practice.

## Known limitations (restated for visibility)

1. The disclosed sync-interval window (up to ~60 seconds of unsynced erasure
   intent is not yet externally journaled).
2. The local filesystem journal adapter is not production-grade independent
   storage by itself.
3. The gate protects the application's own startup path, not an operator with
   unrestricted infrastructure access.
