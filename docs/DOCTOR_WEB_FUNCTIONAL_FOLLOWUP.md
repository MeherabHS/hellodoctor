> Historical slot/notification follow-up snapshot. The latest preview/build and user-approved Dashboard/account/settings/Patient Chats changes are recorded in [DOCTOR_WEB_UX_CORRECTION_REPORT.md](DOCTOR_WEB_UX_CORRECTION_REPORT.md).

# Doctor Web slot / notification / workflow follow-up - 2026-10-07

The current production preview runs on approved loopback **127.0.0.1:3001**,
build **hCP4P6h566jU4qNYVP249**. This follows the presentation correction and
implements the user's subsequent product changes. Canonical prototype files,
Doctor Flutter and Rust source were not edited in this follow-up.

## Slot creation defect and fix

Reproduced in installed Edge: clicking the date field dismissed the creation
dialog (`audit_evidence/doctor_web/slot-fix/before.json`). The prototype backdrop
handler contains an `event.target === this` guard. The Web adapter previously
treated every bubbled click whose handler mentioned `closeSlotGeneratorModal`
as a close action, including clicks inside the form. Earlier open/Escape checks
did not exercise field entry or actual publishing and missed this defect.

The slot form removes that inner backdrop handler; the outer accessible Modal
already handles genuine outside clicks. Its close and publish handlers are
explicit. The prototype renderer now permits an overridden handler to be
removed rather than converting null into a synthetic action. A successfully
published slot opens the schedule on its actual selected date, with refreshed
server data. End-before-start/past-time validation remains editable, and a
first server overlap/validation rejection unlocks the form to correct the
selection. Ambiguous failures/partially saved batches retain their request keys
and confirmed progress. No backend scheduling/ownership rules were changed.

The final browser regression created one slot and a two-slot batch through real
`POST /doctor/slots` responses (201), verified UTC+3 conversion, exactly one POST
per page/slot, visibility on the selected date, persistence after reload,
409 overlap handling, input/close behavior and past-time validation without a
request. Only those test-created slots were cancelled through the existing API
afterward. No Doctor/Patient/payment fixture was created.

## Requested UI changes

| Request | Result |
|---|---|
| Remove Resume Telehealth Call from Dashboard | Removed from rendered Dashboard |
| Remove Requisition Lab / unsupported laboratory actions | Dashboard requisition and critical-lab panel removed; Telehealth lab button, ordering and lab tab removed |
| Remove Doctor patient registration | Patient Registration control removed; no patient-creation API added |
| Remove Smart Rx vitals | BP/Pulse/Temp/SpO2 and risk placeholder block removed |
| Preserve actual clinical documents | Generic Consultation Documents/authorized document viewer retained; chat document controls renamed; unsupported data-table mode removed |
| Separate workflow entry points | Topbar Doctor tools dropdown: queue, patient chats, patient history, schedule slots, handwritten Rx, earnings, Q&A, notifications |
| Correct notification bell routing | Bell opens Notifications; separate calendar opens schedule; no line-number inference |
| Wire existing notification API | Own inbox, unread badge/count, mark read, mark all read, pagination, refresh and supported deep links |
| Articles | User clarified backend routes were absent and requested notification integration only; no article feature/API added and the nonfunctional My Articles control removed |

Doctor tools uses the existing Web styles; it is secondary navigation and adds
no mobile bottom tabs. It supports keyboard Escape, focus restoration and a
narrow dropdown that remains within the viewport. A screenshot caught its
initial negative left offset at 390px; the media rule was corrected and actual
rectangle bounds were added to browser assertions.

All notification text renders as escaped text. Reminder links resolve an
appointment only from the Doctor's owned queue, otherwise open Appointments;
they grant no history authorization. Earning/payout links open server-backed
earnings; registration links open profile. Only actual server notifications
are displayed. The 60-second unread poll is a read request, not fabricated
notification generation or an alternative scheduling system.

## Backend deployment state

The notification implementation and migration already existed in source, but
the running backend knew only migration 20261019000001 and returned 404 for
notifications. The read-only schema verifier caught the discrepancy. Existing
canonical `start_backend_ready.ps1` rebuilt/restarted the existing backend and
applied its pending existing migration; no new migration was authored and no
manual migration-table edits were made. Existing healthy PostgreSQL/Redis
containers were used without creating/pulling/starting new infrastructure.

```text
SOURCE_LATEST_MIGRATION = 20261020000001
BINARY_LATEST_MIGRATION = 20261020000001
DATABASE_LATEST_MIGRATION = 20261020000001
SCHEMA_VERIFIED = YES
HELLODOCTOR_BACKEND_READY = YES
```

Current routes are `GET /notifications`, `GET /notifications/unread-count`,
`POST /notifications/:id/read` and `POST /notifications/read-all`. Only these
existing operations were added to the Doctor BFF allowlist; device registration,
notification creation and arbitrary recipient queries remain inaccessible.
Server ownership/ABAC/RLS and HttpOnly cookie/session behavior are unchanged.

## Verification and evidence

Typecheck, lint, format checks, **11 unit tests** and production build PASS.
The final production build has **7 passing slot browser checks** and **15
passing notification/workflow checks**, with no unhandled JavaScript exceptions.
The Web showed real backend data, not static prototype fixtures.

- `audit_evidence/doctor_web/slot-fix/results.json`: real single/batch publishing,
  correct time zone, overlap, persistence, validation and successful cleanup.
- `audit_evidence/doctor_web/notification-tools/results.json`: two distinct
  Doctor inboxes, foreign mark-read no-op with owner state unchanged, recipient
  injection 422, real inbox/bell behavior, read-all/unread state, daily-summary
  queue link, feature entry points, requested control removals, local icons,
  reduced motion, menu geometry/Escape and actual UI login/MFA/reload/logout.
- Four feature-menu viewports: 1920x1080, 1440x900, 1024x768 and 390x844.
  Final Rx, schedule, notification and menu screenshots were visually inspected.
- The mark-read UI also ran against an unread controlled fixture in an earlier
  attempt; the script then stopped on a generic hidden Next route-announcer
  `role=alert`, not an application error. The harness now checks application
  error elements and waits for actual loaded inbox rows. Final fixture inboxes
  were already read; conditional fresh mark-read is not counted among the 15
  final checks. Idempotent/foreign mark behavior and read state remain verified.

This verifies the requested functional follow-up, not every historical full
Doctor Web acceptance unit. Full visual parity remains PARTIAL. Successful
prescription storage/document byte access, live RTC, external payouts, every
notification event type/deep link and pending/rejected-account notification UI
are not newly certified by these checks. No provider success or new clinical/
financial completion was fabricated. Overall original verdict remains
**DOCTOR_WEB_PARTIAL**; no broader architecture work is started.

## Assets and host integrity

The prototype already supplied the exact calendar/bell icons and loading ring.
The SVGs were extracted locally, checked for scripts/events/remote references,
and served as local masks. The loading ring has accessible status text and a
static reduced-motion equivalent. No external visual asset or font was needed.
See [THIRD_PARTY_UI_ASSETS.md](THIRD_PARTY_UI_ASSETS.md).

```text
LOCAL_ASSETS_REUSED = 3
OPEN_SOURCE_ASSETS_DOWNLOADED = 0
DOWNLOADED_ASSET_SOURCES = NONE
LICENSES_VERIFIED = YES (no external license dependency introduced)
UNLICENSED_ASSETS_USED = 0 (third-party additions)
REMOTE_HOTLINKED_ASSETS = 0
SVG_SECURITY_CHECK = PASS
DOCTOR_MOBILE_FILES_CHANGED = NONE
BACKEND_SOURCE_FILES_CHANGED_BY_THIS_FOLLOWUP = NONE
HOST_SOFTWARE_INSTALLED = NONE
HOST_SOFTWARE_UNINSTALLED = NONE
WINDOWS_SERVICES_CREATED = NONE
GLOBAL_PACKAGES_INSTALLED = NONE
BROWSER_INSTALLED = NO
PLAYWRIGHT_BROWSER_DOWNLOADED = NO
ANDROID_TOOLING_INSTALLED = NO
NEW_DOCKER_INFRASTRUCTURE = NONE
DOCKER_INFRASTRUCTURE_ADDED = NONE
MINIO_REINTRODUCED = NO
PATH_MODIFIED = NO
REGISTRY_MODIFIED = NO
FIREWALL_MODIFIED = NO
```

Pre-existing dirty/untracked work was preserved. Rebuilding/restarting the
existing backend changed its deployed binary/database migration state as
recorded above; an entirely unchanged running backend is not claimed.
