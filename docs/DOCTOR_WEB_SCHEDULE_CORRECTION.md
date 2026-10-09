# Doctor Web schedule blocking correction

2026-10-07. Branch `main`; starting SHA `e3739ae555f198c537dfa7f505fe506b0cd62df2`. Existing dirty work preserved. Preview build `vROyHPJLjR3IY3OKge4Ib` at `http://127.0.0.1:3001`.

## Current-source findings and minimal change

Before this correction, the actual enum contained AVAILABLE, LOCKED_IN_PAYMENT, BOOKED, CANCELLED. Doctor routes supported list/create and DELETE cancellation. No genuine blocking route existed. The Web's Block Slot control opened an unsupported-capability message, and available cards used a missing-patient `?` placeholder. This was an approved product gap, not a request to redesign scheduling.

Added `BLOCKED` via canonical migration `20261021000001_doctor_slot_blocking`. PostgreSQL enum removal is unsafe; the paired down migration deliberately refuses rollback and requires a forward correction, preserving existing blocked rows. No old migration, `_sqlx_migrations` row, grant or RLS policy was edited. Existing non-CANCELLED exclusion constraints keep blocked time reserved on the Doctor's calendar; existing patient availability and booking queries require AVAILABLE.

One new route extends the existing resource: **PATCH `/api/v1/doctor/slots/:slot_id`**, body `{ "status": "BLOCKED" }` or `{ "status": "AVAILABLE" }`. Unknown fields and other target states are rejected. Verified Doctor context determines ownership. The existing authed API transaction and explicit `doctor_id` condition retain RLS/ABAC protection.

The repository uses a single conditional UPDATE for AVAILABLE → BLOCKED or BLOCKED → AVAILABLE. PostgreSQL row locking coordinates this with the existing patient booking transaction. A lost race, duplicate transition, foreign/missing slot, BOOKED, LOCKED_IN_PAYMENT or CANCELLED yields the existing concealment-style 409 conflict without mutation. Cancellation remains DELETE, now valid for own AVAILABLE or BLOCKED unbooked slots; it cannot reopen a cancelled row or alter a patient booking.

## Web presentation

| State | Meaning and icon | Actions |
| --- | --- | --- |
| AVAILABLE | Local prototype clock icon; Open for booking | Block slot, Cancel slot |
| BLOCKED | Lock icon, amber neutral card; Blocked by you | Make available, Cancel slot |
| BOOKED | Real linked patient initials, appointment/modality | Open consultation, Chat |
| LOCKED_IN_PAYMENT | Hourglass; Booking in progress / PAYMENT PENDING | None; brief reservation explanation |
| CANCELLED | Muted calendar-cross; Slot cancelled | None |

Block, restore and cancellation have distinct accessible confirmation dialogs. The existing Modal supplies keyboard focus and Escape handling. Buttons show pending state; cards change only after server confirmation and refresh. A real 409 closes stale confirmation, explains the state change with safe request reference, and refreshes authoritative rows. Other errors retain current sanitized API handling.

All / Available / Blocked / Booked / Cancelled filters remain local presentation over real authorized rows. Complete server pagination is loaded before counting AVAILABLE rows for today; blocked, payment-reserved and booked rows are excluded. UI rendering remains paginated in batches of 50. Dates use unambiguous `7 Oct 2026` text alongside the native date input, in Africa/Nairobi. Clearing the date input cannot crash rendering. No fake patients or browser business-state mutation was added.

## Mobile compatibility exception

The current mobile DTO already accepts string statuses, but its existing schedule view classified every non-AVAILABLE row as a patient booking. A BLOCKED row would therefore display false booked/patient controls. The minimal shared-contract correction filters BLOCKED rows out of that legacy mobile schedule presentation. No mobile navigation/layout/business workflow was redesigned and no mobile blocking feature was added.

Intentional mobile files: `apps/mobile/lib/features/doctor/doctor_pages.dart` (one compatibility filter/comment) and `apps/mobile/test/doctor_widgets_test.dart` (one permanent regression). The regression verifies BLOCKED parses, contributes no booked/available count, and exposes no patient-call/cancel action.

## Verification

| Result | Evidence |
| --- | --- |
| 13 new backend integration tests PASS | Own block/unblock, unavailable patient booking/directory, foreign Doctor denial, protected states, duplicate conflict, three concurrent booking/block races, Doctor listing, cancellation distinction, DTO/auth limits |
| Web typecheck/lint/format and 21 unit tests PASS | Includes complete-list count across pagination, account-boundary suppression and BFF route restriction |
| Production build PASS | `vROyHPJLjR3IY3OKge4Ib` |
| Installed Edge walkthrough 12/12 PASS | [Results](audit_evidence/doctor_web/schedule/results.json); current API requests, two Doctor Web sessions, real concurrent-status conflict, no mocked production responses |
| Responsive rendered inspection PASS | 1920×1080, 1440×900, 1024×768, 390×844; no horizontal overflow |
| Browser exceptions / unexpected 5xx | 0 / 0 |
| Flutter analysis / full tests PASS | No analysis issues; 51/51 tests |
| Canonical startup and schema verification PASS | Source = binary = database = `20261021000001` |
| Full Rust regression PASS | `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, 43/43 library tests and 75/75 API tests; [API log](audit_evidence/doctor_web/schedule/backend-api-tests.log) |

Screenshots: [all states, wide](audit_evidence/doctor_web/schedule/schedule-states-1920.png), [block confirmation](audit_evidence/doctor_web/schedule/block-confirmation-1440.png), [cancel confirmation](audit_evidence/doctor_web/schedule/cancel-confirmation-1440.png), [narrow](audit_evidence/doctor_web/schedule/schedule-states-390.png).

Browser fixtures were newly created unbooked slots owned by the existing synthetic Doctor. The payment-reserved example used one of these isolated rows, not an actual checkout/payment. Booked card identity came from existing controlled fixture appointments. Newly created unbooked rows were cleaned up by exact UUID/Doctor ownership in finally. Permanent Rust tests run in isolated SQLx databases against the current routes, transactions and RLS; booking race tests execute real booking logic there. No real patient/financial record was changed and no credentials were captured in screenshots.

## Answer to the telemetry question

Web browser, Promise, render and unexpected API errors POST an allowlisted category to `/api/diagnostics`. That Next route currently writes a sanitized release/component/reference record to **local server logs**. It does **not** insert these client events into the backend `system_error_logs` Admin feed. Backend-originated failures use the existing shared System Error Service and can appear in Admin independently of client platform.

Flutter `DoctorRepository.telemetry` exposes the existing consultation event endpoint, but source search found no automatic caller in `apps/mobile/lib`. Web also does not submit consultation event telemetry; external calling remains deferred. Therefore equal automatic client telemetry coverage is not verified for either application, and the Web diagnostics route must not be described as Admin ingestion. This question was investigated and documented; adding instrumentation was outside the explicitly scoped schedule correction.

## Host / asset integrity

Existing Node/npm, Rust/Cargo, Flutter/Dart, Git, Python Playwright and installed Edge were used. Optional pnpm/yarn were not found and were not needed. Existing PostgreSQL/Redis containers remained in use. Flutter performed normal existing-manifest dependency restoration; no dependency was added or upgraded.

HOST_SOFTWARE_INSTALLED = NONE

HOST_SOFTWARE_UNINSTALLED = NONE

GLOBAL_PACKAGES_INSTALLED = NONE

WINDOWS_SERVICES_CREATED = NONE

PATH_MODIFIED = NO

REGISTRY_MODIFIED = NO

FIREWALL_MODIFIED = NO

NEW_DOCKER_INFRASTRUCTURE = NONE

MINIO_REINTRODUCED = NO

BROWSER_INSTALLED = NO

PLAYWRIGHT_BROWSER_DOWNLOADED = NO

ANDROID_TOOLING_INSTALLED = NO

NO_INSTALLATION_PERFORMED = YES

OPEN_SOURCE_ASSETS_DOWNLOADED = 0

No external asset/package/provider was introduced. Clock markup reuses the local prototype; simple static lock/hourglass/calendar-cross paths are project-authored, decorative, contain no executable/remote content, and match existing stroke styling.


## Final acceptance matrix

| Item | Result |
| --- | --- |
| QUESTION_MARK_PLACEHOLDER | REMOVED |
| AVAILABLE_SLOT_ICON | Local prototype clock SVG |
| BLOCK_BACKEND_CAPABILITY | NEWLY_IMPLEMENTED |
| BLOCKED_STATE | VERIFIED |
| UNBLOCK | VERIFIED |
| BLOCK_CANCEL_SEPARATION | PASS |
| BOOKED_PROTECTION | PASS |
| LOCKED_IN_PAYMENT_PROTECTION | PASS |
| PATIENT_BOOKING_BLOCK_RACE | PASS |
| CROSS_DOCTOR_BLOCK_IDOR | PASS |
| AVAILABLE_COUNT_CORRECT | PASS |
| BLOCKED_FILTER | PASS |
| DATE_DISPLAY_UNAMBIGUOUS | PASS |
| BACKEND_TESTS | 43/43 library + 75/75 API, including 13 new blocking tests |
| WEB_TESTS | 21/21 unit + 12/12 real browser/API |
| MOBILE_REGRESSION | PASS: analysis clean, 51/51 tests |
| HOST_SOFTWARE_INSTALLED | NONE |
| PRODUCTION_BUILD | PASS |
| VISUAL_REVIEW | PASS |

Source/binary/database equality was rerun after the full regression. Protected-source changes are exactly five backend Rust files, two migrations and the two documented mobile compatibility/test files; all other protected files match the baseline. [Final summary](audit_evidence/doctor_web/schedule/final-summary.json) records the exact list and counts.

This user-approved Schedule scope is verified and complete. It does not upgrade the unrelated original whole-app acceptance or claim frontend telemetry ingestion into Admin. Stop; no further screens/architecture/services are changed.
