# M8-G-30 / M8-G-31 — Implementation Evidence

Date: 2026-11-01. Implements Revision 2 of
`docs/MILESTONE_8_SINGLE_LIVE_CONSULTATION_DESIGN.md`, as approved. Scope held
to exactly that design — no broad audit, no performance benchmark, no
production provider configuration, no changes to the 180-second empty-room
grace period, booking exclusivity, the 10-minute payment hold, financial
settlement, refund/interruption-review logic, or any Patient/Doctor
consultation screen. All testing used disposable, newly created resources
(a throwaway Redis fixture on a non-default loopback port); nothing touched
the shared dev database or any prior evidence.

## 1. Implemented changes

- **New migration** `20261203000001_single_live_consultation_reservation`
  (up/down): `consultation_live_reservations` table (`user_id` PRIMARY KEY —
  at most one live reservation per authenticated human, regardless of
  Doctor/Patient/guardian role, per the approved product decision), FORCE
  RLS with self/admin-read/system-worker/internal-function policies (mirrors
  `consultation_provider_presence_events`'s and
  `consultation_recovery_resolutions`'s existing RLS shape), and a
  `SECURITY DEFINER` function `admin_release_live_reservation` modeled
  directly on the existing `admin_resolve_consultation_recovery`.
- **`services/backend/src/repository/consultation_repo.rs`** — three new
  functions: `acquire_live_reservation` (mint-time, atomic
  `INSERT ... ON CONFLICT ... WHERE ... RETURNING`, design §4),
  `confirm_live_reservation` (webhook-path CONFIRMED upgrade, same atomic
  shape, design §7), `release_live_reservation` (webhook-path release on a
  trusted Left, design §8), plus the `RESERVATION_PENDING_GRACE_SECS = 120`
  constant.
- **`services/backend/src/services/consultation_service.rs::mint_agora_token`**
  — the acquire check, inserted between the existing `NEEDS_REVIEW` check and
  `tx.commit()`. Returns `409 Conflict` ("You are already live in another
  consultation.") when the reservation cannot be acquired.
- **`services/backend/src/services/consultation_presence_service.rs::apply_provider_event`**
  — resolves the real authenticated `user_id` behind the accepted event
  (whichever of patient/guardian/doctor matched, never re-derived), then
  confirms/releases the reservation after the existing per-role presence
  UPDATE. On a genuine conflict (a different, still-live session already
  holds the reservation), writes one `CONCURRENT_LIVE_PARTICIPATION_DETECTED`
  audit event and leaves both sessions' own presence bookkeeping untouched
  (design §6 — conflict remediation, not prevention).
- **`services/backend/src/config.rs`** — new
  `consultation_live_reservation_max_secs` field (default `21_600` = 6 hours,
  env override `CONSULTATION_LIVE_RESERVATION_MAX_SECS`), added to the struct,
  `Debug` impl, env-parsing block, and both test-config literals
  (`config.rs`'s own `#[cfg(test)]` default and `tests/api_tests.rs`'s
  separate literal — both needed updating, confirmed by the compiler).
- **`services/backend/src/api/v1/admin_handlers.rs`** — new
  `release_live_reservation` HTTP handler (`ReleaseLiveReservationRequest`
  DTO), gated on the same `Action::ViewConsultationTimeline` permission as
  the sibling recovery-resolution endpoint, audited via the existing
  `AuditService::log_in_tx`/`AuditLogEntry`, explicit comment that releasing
  a reservation never touches appointment/session/financial state.
- **`services/backend/src/api/router.rs`** — new route
  `POST /admin/consultations/live-reservations/release`.
- **`services/backend/tests/cases/consultation_live_reservation.rs`** (new,
  488 lines) — the 12 approved regression tests. Registered in
  `tests/api_tests.rs`.

## 2. Tests and evidence

All 12 approved tests, run against a disposable Postgres fixture
(`#[sqlx::test]`, as the whole suite already does) and a disposable,
explicitly-owned Redis fixture created for this session on a non-default
loopback port (`redis://127.0.0.1:16380/0`, ownership marker
`final-remediation-g30`) — never the shared dev Redis:

```
cargo test --test api_tests -- --test-threads=1 consultation_live_reservation
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 468 filtered out
```

Two real defects were found and fixed by this run, not merely confirmed
green on the first try:

1. **Slot-timing fixture bug (test setup, not product code).** The first
   attempt scheduled synthetic appointments 10 days out, which silently
   exceeded the test harness's own `agora_token_window_before/after_mins`
   (±1440 minutes = 24h) and separately collided with
   `seed::seed_dev_data`'s own pre-existing DOCTOR1 fixture slots at
   `now+1h..now+5h`. Fixed by scheduling at `now+8h`, confirmed clear of
   both.
2. **G-31 test ordering bug (test code, not product code).** The original
   `test_rtc_token_reissue_rejected_after_needs_review` set
   `recovery_state='NEEDS_REVIEW'` on a `consultation_sessions` row before
   one had ever been created — `mint_agora_token` auto-creates that row on
   first mint, so the `UPDATE` matched zero rows and the subsequent mint
   legitimately returned `200`. Fixed by minting once first (creating the
   session), then setting `NEEDS_REVIEW`, then asserting the retry is
   rejected.

**Regression checks**, per the instruction not to run a broad suite unless
necessary to verify an actual regression:

- The 13 pre-existing `consultation_recovery.rs` tests (the file most
  directly adjacent to the modified `apply_provider_event`) re-run clean:
  `test result: ok. 13 passed; 0 failed`.
- A targeted search across every other call site of the `rtc-token` endpoint
  in the existing suite (`tests/api_tests.rs` — no other `tests/cases/*.rs`
  file references it) found no existing test that mints tokens for two
  *different* appointments as the same authenticated user expecting both to
  succeed — the one pattern the new guard could plausibly have broken. No
  such test exists, so a full-suite run was judged unnecessary on top of the
  two checks above.
- `cargo check --tests`, `cargo clippy --all-targets -- -D warnings`, and
  `cargo fmt --all -- --check` (scoped to every file this pass touched) are
  all clean.

## 3. What is and is not proven by this implementation

Unchanged from the design doc's own §0 — repeated here so this evidence
record cannot be read as claiming more than it does:

- **Proven**: a user cannot mint a second RTC token for a different
  appointment within ~120 seconds of an unconfirmed first mint, or at any
  point once the first appointment's presence has been provider-confirmed
  and not yet released (test 1-10 above exercise exactly this, including the
  true zero-row concurrent-acquisition race, the CONFIRMED-state
  preservation issue, the backstop-independent-of-mint-window issue, and the
  conflict-remediation audit path).
- **Not proven, and not claimed**: that a user cannot be physically connected
  to two Agora RTC channels at once. Agora admits a connection before this
  backend's webhook ever hears about it; no database check can retroactively
  prevent that. See design doc §6.
- **Not proven, and not claimed**: that an already-issued token stops working
  once a session reaches `NEEDS_REVIEW` (G-31). Only new issuance is
  rejected.

## 4. Acceptance classification

- **M8-G-30, local reservation enforcement (the `PENDING`/mint-time layer and
  the `CONFIRMED`/webhook-upgrade layer, both as explicitly scoped above):
  GREEN.** Implemented per the approved design, tested, race-safety verified
  under real concurrency, no regression found in the adjacent existing
  suite.
- **M8-G-31: GREEN.** The pre-existing `409`-on-`NEEDS_REVIEW` guard is now
  covered by a passing regression test that asserts exactly what it proves
  and nothing more.
- **Full physical Agora-level exclusivity: remains BLOCKED**, pending the
  real Agora provider webhook (already tracked as blocked under
  M8-G-19/M8-G-07) and, for the complete guarantee, the separate,
  not-yet-scoped provider-side connection-management integration described
  in design doc §6. This implementation does not change that blocker's
  status, and production readiness is not declared by this pass.

## 5. Remaining operator/engineering follow-ups (not blocking, not done here)

- Approve or revise the two numeric defaults in production configuration if
  different from the values tested here (`RESERVATION_PENDING_GRACE_SECS =
  120`, `consultation_live_reservation_max_secs` default `21_600`) — both
  are already configurable/overridable without a code change.
- The admin manual-release endpoint
  (`POST /admin/consultations/live-reservations/release`) exists and is
  audited but has no dedicated automated test in this pass (it reuses the
  already-proven `admin_resolve_consultation_recovery` SECURITY DEFINER
  pattern verbatim) — a follow-up regression test is reasonable but was not
  part of the 12 approved tests.
- The provider-side kick/connection-management control for true physical
  exclusivity (design doc §6) remains unscoped future work, contingent on
  Agora RESTful API credentials separate from the RTC certificate already
  configured.

## 6. Files changed

```
services/backend/migrations/20261203000001_single_live_consultation_reservation.up.sql   (new)
services/backend/migrations/20261203000001_single_live_consultation_reservation.down.sql (new)
services/backend/src/repository/consultation_repo.rs
services/backend/src/services/consultation_service.rs
services/backend/src/services/consultation_presence_service.rs
services/backend/src/config.rs
services/backend/src/api/v1/admin_handlers.rs
services/backend/src/api/router.rs
services/backend/tests/api_tests.rs            (module registration + one config-literal field)
services/backend/tests/cases/consultation_live_reservation.rs (new)
```

No application code outside `services/backend/` was touched. No migration
was run against the shared dev database (only ephemeral `#[sqlx::test]`
databases). No staging, commit, or push was performed.
