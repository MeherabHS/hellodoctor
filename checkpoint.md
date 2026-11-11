# CHECKPOINT — exact resume point for a new Claude Code session

Read `progress.md` first for full context. Short version: **all implementation
work for Task 4 (FINANCIAL_MODULE_MODE=deferred|enabled) is done and green
across every app/service** — Backend, Patient Flutter, Doctor Flutter, Doctor
Web, and Admin Web. The disposable test Redis fixture has been stopped and
removed; nothing is running in the background.

## What's left

Only the **final comprehensive deliverables report** for Task 4, per the
original spec's 9-point list (see `progress.md`'s intro + each app's section
for the raw material). Do NOT merge, push, deploy, or change any Azure
environment variables — the user was explicit about this, and about the
release-gate ordering:

Production backend → Free/policy-waived booking → Confirmed appointment →
Real Agora consultation → Clinical completion → Notifications and records →
only then restore `ENVIRONMENT=production` on Azure with
`FINANCIAL_MODULE_MODE=deferred` explicitly set.

The report should cover:
1. Source-level impact assessment (what touches payment in booking/
   completion/earnings/wallet/refunds/payouts across every app — see
   `progress.md`'s per-app breakdown).
2. Proposed architecture/schema changes (the `payment_policy` column/enum,
   the `doctor_complete_consultation` SQL function split, `/config/booking-
   policy`).
3. Minimal implementation summary (what was actually built, per app).
4. Regression tests + results — all green:
   - Backend: `540 passed; 0 failed; 5 ignored` (full `cargo test --test
     api_tests -- --test-threads=1` suite, ~28 min).
   - Patient Flutter: `flutter analyze` 0 issues, `flutter test` 375/375.
   - Doctor Flutter: `flutter analyze` 0 issues, `flutter test` 312/312.
   - Doctor Web: `npm run lint` clean, `tsc --noEmit` clean, `npm test`
     96/96.
   - Admin Web: `tsc --noEmit` clean, `npm run lint` clean (1 pre-existing
     unrelated warning), `npm test` 127/127.
5. Files changed — full list is in `progress.md`, organized by
   app/service.
6. Required rebuilds per app/service (Rust binary rebuild + `sqlx migrate
   run` against whichever Postgres it targets; Flutter APK/IPA rebuilds for
   both mobile apps; Next.js rebuild for Doctor Web and Admin Web).
7. Production configuration instructions: set `FINANCIAL_MODULE_MODE=deferred`
   as an Azure env var on the backend service (exact name/value; defaults to
   `enabled` if unset; any other value panics the process at startup — fails
   closed). This single var is sufficient once the new backend build with
   migrations applied is actually running — clients all read the effective
   policy from `/config/booking-policy` rather than deciding locally, so no
   other per-client config is needed.
8. Rollback procedure: set `FINANCIAL_MODULE_MODE=enabled` (or unset the var)
   and restart the backend — this is a pure config flip, no data migration
   needed to roll back (deferred appointments remain distinguishable forever
   via their `payment_policy='NOT_REQUIRED'` column value; re-enabling the
   financial module never retroactively charges them — confirmed by the
   `test_deferred_booking_can_join_agora_and_complete_with_no_earnings`
   integration test). The `payment_policy` column/enum and the rewritten
   `doctor_complete_consultation` function are themselves NOT rolled back by
   this — only the down-migrations would remove those, and that's a separate,
   much more disruptive action not needed for an ordinary mode-flip rollback.
9. Residual risks / judgment calls to flag explicitly for reviewer sign-off:
   - Admin Web's "Settlements & Payouts" nav item is deliberately left
     visible (not hidden) in deferred mode, unlike Doctor Web's Wallet nav
     item which IS hidden — see `progress.md`'s Admin Web section for the
     reasoning. Flag this inconsistency for an explicit decision.
   - The `/config/booking-policy` endpoint is reachable by any authenticated
     user (`abac::authorize(&ctx, Action::AnyAuthenticated)`), which was the
     simplest contract that works for Patient/Doctor/Admin alike — worth
     reviewer confirmation that this is the intended exposure level.
   - No code has been merged, committed, pushed, or deployed. Everything
     described above is sitting as uncommitted local changes across 5
     apps/services. A reviewer needs to actually look at a diff before any
     of this goes anywhere.

## Files changed (for the report's "files changed" list)

Full per-app breakdown is in `progress.md` — every file touched, by
Backend / Patient Flutter / Doctor Flutter / Doctor Web / Admin Web. Nothing
to re-derive; just transcribe it into the final report's format.

## Everything else from the original session (Tasks 1-3)

Already complete — see `progress.md`'s Task 1/2/3 sections. Not part of what's
left to do.
