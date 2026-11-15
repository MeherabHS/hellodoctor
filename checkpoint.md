# CHECKPOINT — exact resume point for a new Claude Code session

Updated 2026-11-14. Read `progress.md` Task 9 first.

## Latest: Admin Web complete and committed (Task 9) — next is Doctor Web

Commit `54f8582` on `fix/p0-consultation-repair` holds the Admin Web redesign and everything added on 2026-11-14: Admin Team (invitations with password confirmation), Activity Log, pop-up notifications, Admin forgot password, Q&A audit, grievance appeals, removal of "Simulate failure", security hardening (privilege sweep, sanitization, ASCII emails, no-store default, proxy path guard, HSTS) and soft caching. Details: `progress.md` Task 9, `docs/admin-ui/ADMIN_REDESIGN_PLAN.md`, `docs/API_SPEC.md` (§8 and the 2026-11-14 sections at its end). Screenshots: `docs/admin-ui/screenshots/`.

**Next: Doctor Web (`apps/doctor-web`).** Per CLAUDE.md rule 2 the `index.html` prototype stays its visual source of truth, unchanged: no redesign. Start by reading the prototype's Doctor Web screens, `docs/VISUAL_FIDELITY.md` and the Doctor Web parts of `docs/SCREEN_INVENTORY.md` / `docs/USER_FLOWS.md`, then compare with the running app before changing anything. Backend protections added for Admin (sanitization helpers, `normalize_email`, `Cache-Control: no-store`, content cache) already apply to Doctor Web traffic.

**Still uncommitted (deliberately):** Doctor Flutter work in `apps/mobile/**`, the Doctor Rx dispatch changes in `consultation_service.rs` / `prescription_service.rs` and their test in `tests/api_tests.rs` (47-line hunk), `docs/doctor-ui/consultation-schedule.md`, two doctor schedule reference images, the stray Android project at the repo root (`app/`, `gradle*`, `.gradle/`, `local.properties`, `*.gradle.kts`), the Patient UI `.docx`, an empty `services/backend/abac.rs`, and these two docs.

**Local environment:**
- Backend: run the scratchpad launcher `run_backend_azurite.ps1` (sets Azurite storage, `FINANCIAL_MODULE_MODE=deferred`, `PASSWORD_RESET_ENABLED=true`, and `CARGO_TARGET_DIR` to the scratchpad `cargo-target` so it does not rebuild on C:). After adding a migration, restart it and run `services/backend/scripts/verify_backend_schema.ps1` (dev DB is at `20261216000001`).
- Docker: `hellodoctor-postgres`, `hellodoctor-redis`, `hd-azurite`, and the test Redis `hd-test-redis-p0repair` (port 6390; must be running for integration tests; `TEST_REDIS_FIXTURE_ID=final-remediation-p0-20261113`). Run backend integration suites with `--test-threads=1`.
- Admin Web dev server on `http://127.0.0.1:3000`; dev Admin `admin@hellodoctor.health`. Repeated scripted runs trip the per-IP login limit (5 per 15 minutes) and the invite reauth lockout by design.
- Optional `EMAIL_ADMIN_WEB_URL` sets the invitation link base (production `https://admin.afyamkononi.app`).

---

## Earlier: Facilities & Services, education topics, password reset (Task 8, committed `4633445`)

Everything is uncommitted on `fix/p0-consultation-repair`. Facilities & Services is implemented end to end (Admin Web, backend, Patient app) and verified on the emulator (tests A–F in `docs/facilities/FACILITIES_IMPLEMENTATION_REPORT.md`). The HIV Prevention & Sexual Health topics, videos and quizzes are in `docs/patient-ui/EDUCATION_TOPICS.md`. The re-checked gap list is `docs/patient-ui/PATIENT_REMEDIATION_GAP_MAP.md`.

To run locally with photo storage: start the Azurite container `hd-azurite`, then start the backend with STORAGE_PROVIDER=azure, AZURE_STORAGE_AUTH_MODE=azurite, AZURE_STORAGE_USE_EMULATOR=true, AZURE_STORAGE_ACCOUNT_NAME=devstoreaccount1, the Azurite development key, AZURITE_BLOB_STORAGE_URL=http://127.0.0.1:10000, containers `clinical-local`/`articles-local`, and a 32-byte STORAGE_ENCRYPTION_KEY_B64 (keep the same key across restarts or stored photos cannot be decrypted).

Also done: standalone quizzes (Quiz Manager), single-Admin content approval, Android Back on tabs (D-08c), Facilities map strip removed. Doctor-app files and the Doctor Rx dispatch changes in `consultation_service.rs`/`prescription_service.rs` belong to separate Doctor work and were left out of the Patient commit.

Next: Admin Web full redesign (product owner, 2026-11-13); full-size Facilities empty-state art; Swahili review.

---

## Earlier: Patient post-rebuild gap analysis (Task 7, diagnostic, complete)

Report: `docs/patient-ui/PATIENT_POST_REBUILD_GAP_ANALYSIS.md`; screenshots in `docs/patient-ui/screenshots/gap-analysis/pixel_9_api37/`. Source: HEAD `3c09124` plus the uncommitted/untracked redesign (nothing committed). `flutter analyze` clean, `flutter test` 462/462, debug build runs on `Pixel_9`.

Consultation flows were then tested with an approved fixture doctor (report section L): video booking, waiting room, RTC token and server completion work; **chat consultations cannot start in deferred-payment mode** (no conversation is ever created — P0); completion screen and history unreachable afterwards; Home card shows too early and goes stale. Local backend is running with `FINANCIAL_MODULE_MODE=deferred` (process env only). Fixture doctor `fixture.doctor@example.test` exists locally. Counts now P0 4, P1 12, P2 25.

Top findings: (1) **P0 intermittent sign-out** — 2 of 3 sign-ins this session returned to Log in; protected GETs got 401 and no `/auth/refresh` reached the server; sessions were not rotated or revoked; root cause not yet verified. (2) Notification settings never loads (provider scope; no request is sent). (3) Password reset cannot be enabled in staging/production; app shows "This record is no longer available." (4) Home upcoming card ignores the server `join` object. (5) No Peer backend at all. Counts: P0 3, P1 9, P2 19.

Waiting on the product owner: approval of the fixture set (report section K) to test consultations; decisions on password recovery and Peer scope. Decided 2026-11-13: the Home upcoming card appears 15 minutes before start (server `join_open_at`); `CLAUDE.md` rule 2.4 updated; code not yet changed. No fixes have been started. A synthetic patient `gap.audit.1113@example.test` now exists in the local database.

## Also in progress: Doctor Flutter UI replication (Task 6)

Read `docs/doctor-ui/DOCTOR_UI_REPLICATION_STATUS.md`. Batch A (login, registration, approval states) is built and uncommitted in `apps/mobile`. Next: finish the password-reset and legal-agreement layouts, then Batch B (four-tab shell, Home, Appointments, Availability). View screens with `flutter run -t lib/dev/design_preview_main.dart` in `apps/mobile`; no login needed. The login illustration is an interim cut from the reference board and needs a high-resolution replacement. Do not start Peer Navigator backend work without a separate approval; see `docs/doctor-ui/PEER_NAVIGATOR_ROLE_AND_API_GAP_ANALYSIS.md`.

## Current task: Patient Flutter UI replication (Task 5)

**Where it stands:** the Patient app (`apps/mobile_patient`) is being rebuilt to the AfyaMkononi reference and the product owner's supplied designs. Work is on branch `feature/patient-ui-replication`, **uncommitted**. `flutter analyze` is clean and `flutter test` passes 462 of 462. Nothing has been merged, pushed, deployed, or built as a release APK.

**The detailed record** is `docs/patient-ui/PATIENT_UI_REPLICATION_STATUS.md` (per-screen state, differences from each design, code changes). Discovery documents are beside it in `docs/patient-ui/`.

### Done

- Home (nine-module grid, hero, upcoming-consultation card, offline state), bottom navigation (Home, Services, Messages, Profile), Services directory.
- Module screens: HIV Prevention & Sexual Health, HIV Support Chat, Peer Navigator (layout, action disabled), Reminders (working, local), Health Records, Facilities (layout, no data), Video Consultation landing, Privacy & Security, More.
- Messages and Profile redesigned (no reference existed; derived from the design language).
- Log in, create account, forgot password (size-independent layout).
- Notifications empty state, Find a Doctor header and empty state.
- Consultation confirmed, waiting room, consultation complete.
- Artwork: product owner's regenerated illustrations are in `assets/illustrations/`; auth and screen headers are cropped from the supplied designs.

### Next steps, in order

1. **Sign-out defect (open, undiagnosed).** The app signs itself out when it should not.
   - Reproduction on a fresh account: airplane mode on, pull-to-refresh on Home while offline, airplane mode off, refresh again; the app lands on the log-in screen.
   - Also seen: signing in to one older test account returns to log-in right after the code is accepted. The backend creates the session, the first authenticated requests return 401, then the client's refresh fails and it expires the session.
   - Start by logging the 401 response message in `ApiClient`'s error interceptor (temporarily) and replaying the airplane-mode sequence.
2. **See the consultation screens on the emulator.** The local database has no doctor, slot or appointment, so the Home upcoming card, consultation confirmed, waiting room and completion screens have only been checked by tests. Needs the product owner's go-ahead to seed test data.
3. **Remaining screens not yet redesigned:** email code, reset code, new password, reset success, profile completion, doctor profile, booking page, chat thread, call screen chrome, article reader and quiz, notification settings, profile sheets, data rights.
4. **Auth copy is hardcoded English.** Move it into the localization files. All new Swahili strings are machine-quality and need a native speaker's review.
5. **Still to capture:** a 360 dp device, Swahili and text scale 1.3 for the new screens.
6. **Product-owner decisions outstanding:** wording of the `CLAUDE.md` line that still lists Reminders as a "coming soon" panel; whether the Ask-a-question heading should be HIV-specific; replacement art for the Facilities map strip; whether the corner characters should return on the registration screen (needs separate transparent images).

### Do not

- Do not merge, push, deploy or build a release APK without being asked.
- Do not change backend, database schema, Doctor apps or Admin Web as part of this task.
- Do not invent clinical content (consultation summaries, follow-up plans, sample records, facilities, reminders) in the signed-in app.

### How to run

Local backend: `cargo run --bin hellodoctor-backend` in `services/backend` (Docker Postgres and Redis must be up). App on the `Pixel_9` emulator:

```
flutter run -d emulator-5554 --dart-define=ENVIRONMENT=development --dart-define=ALLOW_INSECURE_HTTP=true --dart-define=DEV_AUTH_OTP_ENABLED=true --dart-define=API_BASE_URL=http://10.0.2.2:8080/api/v1
```

The emulator was started with `-no-snapshot-save`, so an app install does not survive closing it. Two throwaway patient accounts (`diag.…@example.test`) exist in the local database from testing. If sign-in reports "too many attempts", the counters are `ratelimit:*`, `failcount:*` and `ip_targets:*` keys in the local Redis container.

### Other loose ends

- An Android Studio template project (`app/`, `gradle/`, `build.gradle.kts`, `settings.gradle.kts`, `gradlew*`, `local.properties`, `.gradle/`) sits untracked at the repository root. It is unrelated to the product and was left alone pending the product owner's decision.
- `CLAUDE.md` has uncommitted edits recording product-owner decisions: the reference image as source of truth, artwork authorisation, design-preview fixtures, and the Home rule (business rule 2.4).

---

# Earlier checkpoint (Task 4, financial deferment) — kept for history

The text below was the resume point before Task 5. `git log` shows `Merge pull request #23 from MeherabHS/feature/financial-module-deferred`, so its "uncommitted" statements are out of date; whether the final deliverables report it describes was written has not been checked.

## Earlier resume text

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
