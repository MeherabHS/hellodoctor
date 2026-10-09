# Startup diagnostics and schedule synchronization

Date: 2026-10-08. Local development repair; no clinical-production readiness claim.

## Confirmed causes

* Flutter constructed its authenticated error reporter after `ApiClient`. An API
  configuration exception therefore reached the startup error page without a
  reporter. Pre-authentication failures were also discarded.
* Admin slot availability used `AVAILABLE <-> CANCELLED`, whereas Doctor Web
  uses `AVAILABLE <-> BLOCKED`. Mobile explicitly filtered out `BLOCKED` rows.
* Doctor schedules did not poll for external changes. Mobile initially fetched
  only 25 slots. Admin slot/error updates depended on unrelated dashboard reads
  completing, and overlapping polling reads could overwrite newer results.

## Changes

Flutter saves sanitized startup/pre-authentication failures in a separate native
secure-storage namespace, before an API client or session is available. The queue
holds at most eight distinct envelopes for seven days. It preserves source frames,
the error-type reference, occurrence time, and installed version/build, without
exception prose, credentials, account identifiers or clinical data. Storage
failures are swallowed; diagnostic work never prevents application startup.

Successful authentication, connectivity restoration, app resume and a bounded
foreground retry trigger delivery through the existing Doctor-only `/client-errors`
endpoint. Entries are removed only after HTTP 202. Failed delivery remains queued;
diagnostic requests cannot refresh or expire the session. Backend validation stays
authenticated, bounded and rate limited. Startup reports have error code
`CLIENT_STARTUP_EXCEPTION`, source `deferred_startup_report`, and a validated
eight-character `startup_reference`. Admin details display the reference and
explain that the associated Doctor uploaded the report; a pre-authentication crash
does not establish the original user's identity.

No network request can deliver an incident while the installed application has an
unusable API configuration. Delivery requires installing the corrected build and
subsequently authenticating. Failures from the old APK, which had no journal,
cannot be reconstructed retrospectively. The journal is best effort, not guaranteed
delivery across keystore failures, uninstall, TTL expiry or permanent offline use.

Admin now explicitly requests BLOCK or UNBLOCK. PostgreSQL compare-and-set permits
only AVAILABLE -> BLOCKED or BLOCKED -> AVAILABLE, so concurrent booking, duplicate
requests and stale clicks cannot overwrite a reservation or revive a cancellation.
The committed transition and audit snapshots share one transaction. Doctor-created
slots now also record actor, source, times and status in the creation audit trail.

Doctor Web refreshes mounted schedules every 15 seconds while visible/online and
on focus/reconnect. Mobile displays blocked slots as unavailable, excludes them
from booked counts and patient actions, reads all authorized pages, polls the
mounted schedule every 15 seconds and refreshes on resume. Both discard responses
after account/view disposal. Admin refreshes slots and incidents independently of
other dashboard permissions/failures, and ignores superseded critical reads.

## Existing development data

Initial read-only inspection found 41 persisted slots: 18 AVAILABLE, 1 BLOCKED,
2 BOOKED, 20 CANCELLED. A subsequent legacy admin action changed one AVAILABLE
slot to CANCELLED before the backend restart; the total stayed 41. Three IDs
exactly match the historical browser-test artifact
`audit_evidence/doctor_web/slot-fix/results.json`:

* `5d51eecc-8ba8-4124-b563-313b88c01884`
* `2a250890-3f5f-43e1-b2dc-a608b97cafc5`
* `2725119d-1042-4581-80b3-9f3723af2194`

That probe cancelled its fixtures rather than deleting them, so they remain visible
in the Admin all-time matrix. Many other historical creations have no audit record;
their origin cannot be established from the current database. No existing slots,
bookings, historical cancellations or payments were deleted, reopened or recoded
by this repair. No new slot was created in the shared database during verification.
Backend regressions use SQLx throwaway databases.

Three old AVAILABLE -> CANCELLED admin actions are candidates for correction to
BLOCKED. An atomic, audited repair is prepared but has not been applied: historical
records cannot distinguish an intended block from an intended cancellation, so
the user's choice is pending. None of the three has an active booking or overlap.

The four historical slot-mutating browser probes now refuse to run by default.
They require `HELLODOCTOR_ALLOW_SHARED_DEV_FIXTURES=true`, which is appropriate only
when persistent synthetic fixture creation has been explicitly approved. The
normal backend remains running with `SEED_DEV_DATA=false`.

## Verification

* Flutter analysis: no issues. Full suite: **72 passed, 0 failed**. Dart formatting
  check: zero changes. Current native ARM64 APK build succeeded.
* Doctor Web: 32/32 tests, TypeScript, source-safety/format checks and production build.
* Admin Web: TypeScript and production build.
* New backend regressions cover admin/doctor/patient agreement, creation/transition
  audits, booked/payment-locked/cancelled protection, concurrent duplicate blocks,
  role denial, and deferred startup ingestion/admin retrieval/reference validation.
* Backend `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`: clean.
* `cargo test --lib`: **50 passed, 0 failed**, 124.92 seconds.
* `cargo test --test api_tests -- --test-threads=1`: **106 passed, 0 failed**,
  758.73 seconds. This includes all three new regressions; none ignored.
* Full backend suite logs and machine-readable `verification-summary.json` are in
  `services/backend/target/startup-slot-verification/`. Flutter suite output is
  `apps/mobile/build/startup-slot-tests.log`.
* Read-only installed-Edge probe: actual Doctor MFA and Admin TOTP login, same
  existing BLOCKED row in both portals, rendered blocked cards/chips, and observed
  Doctor schedule polling. Results: `browser-results.json`; zero slot mutations.
* All four fixture mutation guards tested: default refusal before any writes.
* Backend restarted using the canonical readiness script; source, binary and
  database schema agree at `20261023000001`.

The phone must install the newly built `HelloDoctor-phone-test.apk` to receive
these mobile changes. No Android device is currently connected, so on-phone
delivery and rendering are not claimed as verified.
