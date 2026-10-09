> Latest functional fixes and user-requested removals: [follow-up report](DOCTOR_WEB_FUNCTIONAL_FOLLOWUP.md). Older results below retain their original scope.

> Current presentation result: [Doctor Web correction report](DOCTOR_WEB_CORRECTION_REPORT.md). The older combined mobile/Web acceptance below is historical; mobile roots are not Web visual authority.

# Doctor Web acceptance — not accepted

Current evidence-supported verdict: **DOCTOR_WEB_PARTIAL**.

The application is implemented in one new project, `apps/doctor-web`. The
production build passes and major workflows have real backend integration.
Exact rendered visual acceptance and complete prototype control coverage are
unfinished. This document does not approve a production deployment or claim
that the original build request is complete.

Ground truth: `main`, commit `e3739ae555f198c537dfa7f505fe506b0cd62df2`, with
extensive pre-existing uncommitted work preserved. Canonical visual authority is
`prototype/index.html`. Current Flutter source and Rust routes/services were
read; no Flutter application source was changed.

| Acceptance item | Current status / evidence |
|---|---|
| DOCTOR_WEB_PROJECT | created — one Next/React application, HttpOnly BFF |
| AUTH | PARTIAL — real login/restoration/logout exercised; complete latest-build lifecycle gate pending |
| MFA | PARTIAL — two real independent Doctor MFA sessions passed; current release rerun pending |
| QUEUE | PARTIAL — actual owned appointments and source card structure; visual parity pending |
| QUEUE_UPCOMING_FIRST | VERIFIED — real descending backend pages consumed and active/upcoming sorted globally before history; Node regression passes |
| DUTY | VERIFIED — real server-confirmed read/change/read workflow |
| CHAT_LIST | PARTIAL — real HTTP list and filters; backend lacks preview/presence/triage metadata |
| CHAT_THREAD | PARTIAL — actual messages, escaped text and canned reply verified; every header action/expiry interaction pending |
| SMART_RX | PARTIAL — photo workflow and truthful current storage failure rendered |
| MULTI_PAGE_RX | PARTIAL — two-page failure/remove/retry browser checks; tested controller preserves successful pages and stops at account boundaries; live persistence unavailable |
| SCHEDULE | PARTIAL — real create/list/cancel and booked/overlap conflicts verified; latest generator rendering review pending |
| WALLET | PARTIAL — real server KES amounts and financial states; rendered comparison pending |
| MONTHLY_STATEMENT | PARTIAL — month-scoped DTO integrated; latest canonical dialog opens without browser exceptions; visual parity pending |
| PAYOUT_SETTINGS | PARTIAL — actual readiness/destination list; real provider onboarding deferred |
| QNA | PARTIAL — real answer submission and concurrent 409 verified; every filter/detail visual sequence pending |
| CONSULTATION_SUMMARY | PARTIAL — real contextual API access verified |
| PATIENT_HISTORY | PARTIAL — own authorized access, too-early and foreign denial verified; every historical episode/document sequence pending |
| DOCUMENT_VIEWER | PARTIAL — server-only grants and reauthorization implemented; STORAGE_CONFIGURATION_REQUIRED prevents live document-byte acceptance |
| CONSULTATION_COMPLETION | PARTIAL — reproducible no-Rx shared-contract defect fixed; targeted current API regression and current-binary browser completion/retry pass; full acceptance remains pending |
| PROFILE | PARTIAL — actual profile, permitted name update and readonly credentials implemented; latest update walkthrough pending |
| REGISTRATION | PARTIAL — current server pending-verification contract integrated; latest browser end-to-end flow pending |
| NOTIFICATIONS | BACKEND_GAP — GENUINE_BACKEND_GAP_DOCTOR_NOTIFICATIONS |
| ARTICLES | BACKEND_GAP — GENUINE_BACKEND_GAP_DOCTOR_ARTICLES |
| RTC | DEFERRED_EXTERNAL_INTEGRATION — backend token support is not a Web Agora client |
| EXTERNAL_PAYMENT | DEFERRED_EXTERNAL_INTEGRATION — fixture development webhook is not real payment |
| KENYA_KES_ONLY | PARTIAL — active source check passes; final production-bundle/content walkthrough pending |
| CROSS_DOCTOR_IDOR | PARTIAL — distinct MFA sessions denied foreign summary/history/chat/wallet injection; extra foreign slot checks confirm denial and unchanged rows; notifications/articles absent |
| SESSION_LIFECYCLE | PARTIAL — cold start/reload/logout and concurrent refresh verified; full multiple-tab/account-switch/revocation/sleep/network matrix pending |
| WEB_SECURITY | PARTIAL — CSRF, HttpOnly/Secure/Strict cookies, route allowlist, no token/PHI storage, escaped messages and zero-vulnerability dependency audit; full release review pending |
| ERROR_DIAGNOSTICS | PARTIAL — sanitized BFF diagnostics/reference IDs; current backend lacks general frontend ingestion contract |
| RESPONSIVE | PARTIAL — five viewport sizes captured; prototype geometry differences remain |
| ACCESSIBILITY | PARTIAL — labels, focus visibility, dialog focus trap/Escape and keyboard controls implemented; full walkthrough pending |
| PROTOTYPE_SCREENS_TOTAL | 11 canonical screen roots, including separate desktop workstation surfaces |
| PROTOTYPE_SCREENS_MATCHED | 0 — no source/build-only visual acceptance |
| PROTOTYPE_SUBFLOWS_TOTAL | 20 explicitly enumerated in the recursive inventory |
| PROTOTYPE_SUBFLOWS_MATCHED | 0 — complete rendered subflow acceptance pending |
| NOT_COMPLETE | 31 acceptance units (11 screens + 20 subflows); recursive inventory has 887 element rows awaiting final coverage acceptance |
| TESTS | Web: 8 passed, typecheck/lint/format passed. Flutter: analyze passed, 50 tests passed, source unchanged. Rust: fmt/clippy passed, 29 library tests passed; API second run 52 passed/1 failed on an older early-timing fixture; corrected targeted history test passes (53 distinct API tests pass across runs); one final full green run not claimed |
| PRODUCTION_BUILD | PASS — Next 15.5.24, React 18.3.1 |
| BROWSER_WALKTHROUGH | FAIL acceptance gate — substantial real browser checks completed, complete latest-release walkthrough unfinished |

Evidence: `docs/audit_evidence/doctor_web/web/smoke.json`,
`workflow/results.json`, `workflow/extra-results.json`,
`dependency-audit-final.json`, `reference/computed_styles.json`, screenshots,
and `coverage-current.json`. The raw first workflow report labels foreign slot
409 as failure because its initial expectation was too narrow. The later check
proves the slot remains unchanged; no evidence file is rewritten to conceal
that earlier test mistake. The old completion screenshot predates the backend
correction and is not authoritative completion evidence.

The single narrow Rust change runs existing authoritative completion inside the
no-Rx transaction while preserving the DTO and current clinical-document access
window. No migration, financial model redesign, alternate API or Flutter source
change was made. See the backend mapping for the exact defect and regression.

Full remaining acceptance is concrete: finish shell/nav geometry and nested
controls; capture the latest rendered release; compare every screen/dialog with
the actual prototype; complete registration/session/history/document/security
walkthroughs; update every inventory row with test and screenshot evidence.
External storage/provider capability remains truthfully unavailable.

## Host integrity for this Doctor Web build

```text
HOST_SOFTWARE_INSTALLED = NONE
HOST_SOFTWARE_UNINSTALLED = NONE
WINDOWS_SERVICES_CREATED = NONE
PATH_MODIFIED = NO
REGISTRY_MODIFIED = NO
FIREWALL_MODIFIED = NO
NEW_DOCKER_INFRASTRUCTURE = NONE
MINIO_REINTRODUCED = NO
BROWSER_INSTALLED = NO
ANDROID_TOOLING_INSTALLED = NO
GLOBAL_PACKAGES_INSTALLED = NONE
NO_INSTALLATION_PERFORMED = YES
```

The user explicitly approved the loopback-only `127.0.0.1:3001` preview.
Only existing Node/Rust/Flutter/Edge/Python tooling, project-local dependency
restores and approved existing PostgreSQL/Redis infrastructure were used.

## Login recovery follow-up — 2026-10-07

The reported support-reference login error was traced to shared PostgreSQL
role passwords overwritten by the API test harness. The harness now reuses
development credentials, and the running backend connection was restored.
Actual Edge UI password login, MFA, reload/session restoration and logout
passed against the backend, with no JavaScript exceptions. Localhost now
redirects to the configured loopback origin; foreign-origin login remains
rejected. See `audit_evidence/doctor_web/LOGIN_RECOVERY.md` and
`login-ui-recovery.json` for precise evidence and verification limits.
The overall verdict remains DOCTOR_WEB_PARTIAL; visual acceptance and the
full session/authorization matrix are not established by this login check.

Current-binary follow-up: `workflow/completion-current-results.json` verifies
UI success agrees with authoritative completion, same-reason retry has no
financial change, statement canonical dialog opens, and slot generator
dismisses with Escape; zero browser exceptions. Source/binary/database migration
versions all equal 20261019000001 and canonical startup reports ready.


## Latest presentation correction acceptance - 2026-10-07

See [DOCTOR_WEB_CORRECTION_REPORT.md](DOCTOR_WEB_CORRECTION_REPORT.md) for the
mandatory current results matrix, complete Web-screen table and host report.
Fullscreen shell, viewport use, outer-frame removal, duplicate/mobile desktop
navigation removal and Rx/Telehealth/Q&A/Appointments/Dashboard desktop layouts
PASS. All five actual Web roots are accounted for; real API integration survives.
The correction pass did not modify Flutter/Rust, manifests or host software.

Typecheck/lint/format, 10 unit tests and production build PASS. Installed Edge
walkthrough PASS for this corrective regression: 45 screen/viewport checks,
real UI login/MFA/reload/logout and 14 populated workflow checks, with no
unhandled JavaScript exceptions. See correction JSON/screenshots and baseline
source preservation report. BROWSER_WALKTHROUGH here has a narrower declared scope
than the historical original full-project acceptance gate above.

VISUAL_PARITY = PARTIAL. Broader registration/session/authorization families,
all shared-modal interaction parity and actual storage/RTC/provider capabilities
are not newly certified. NOT_COMPLETE = 0 is not claimed.
Final overall verdict: **DOCTOR_WEB_PARTIAL**. The reported presentation defects
are corrected; stop this pass without reopening backend/mobile architecture.


## User-approved functional follow-up - 2026-10-07

Latest result: [DOCTOR_WEB_FUNCTIONAL_FOLLOWUP.md](DOCTOR_WEB_FUNCTIONAL_FOLLOWUP.md).
Slot-dialog field clicks no longer dismiss it; real single/batch slot publishing,
UTC+3 conversion, overlap handling and reload persistence are verified. Publishing
opens the schedule on the chosen date. Dashboard Resume Call, unsupported lab
controls, Doctor patient registration and Smart Rx vitals are removed as requested.
Doctor tools provides separate workflow links; calendar and notification bell
have distinct actions. Existing notifications are wired through the HttpOnly BFF.
Canonical startup resolved stale running-backend 404s; source/binary/database
migration versions all equal 20261020000001. No Rust/Flutter source was edited.

11 Web unit tests and typecheck/lint/format/build PASS. Final Edge walkthrough:
7 slot checks + 15 notification/workflow checks PASS; zero JS exceptions. Real
UI login/MFA/reload/logout and two-Doctor notification ownership checks pass.
22 is the declared follow-up regression count, not full historic app acceptance.
Overall full-project visual parity remains PARTIAL; see the linked report's limits.
The article backend is absent and the user explicitly narrowed this follow-up
to existing notifications. No articles API/workflow was invented.

3 local prototype visual assets reused, 0 external downloads, SVG inspection PASS,
0 hotlinked/unlicensed third-party additions and no software/browser/global package/
Docker infrastructure installation. THIRD_PARTY_UI_ASSETS.md records details.

## 2026-10-07 ? Dashboard / account / settings verified checkpoint

This targeted correction is complete before the dedicated Patient Chats routing follow-up. Build `lGgKwZC70Jn19ywa_XOwV`: production build PASS; typecheck, source lint, format checks and 11 unit tests PASS; 13 browser/API checks PASS with no JavaScript exceptions or unexpected 5xx. Installed Edge only. Evidence: `audit_evidence/doctor_web/dashboard-settings/results.json` and adjacent screenshots.

Dashboard: four supported summaries, one upcoming/active list preserving server order; duplicate sidebar queue and modality metrics removed. Instant Rx removed. Unsupported recent dispatch aggregation omitted (BACKEND_DATA_NOT_AVAILABLE). No developer-unavailable cards or emoji placeholders remain on Dashboard.

Account: compact keyboard-accessible popover, View profile opens the read-only Professional section, Settings is a full desktop page with Profile/Professional/Availability/Payout/Security sections. Display-name PATCH and server-confirmed duty tested then restored on the synthetic Doctor. Protected professional fields rejected by server on attempted mass assignment; only display name editable. Payout masking/empty state preserved; actual provider binding remains deferred. Wallet is a separate existing domain, not a profile action strip.

Prescription: camera removal/file picker/recoverable failed-page states separately PASS (7 browser checks). Current actual backend returns 409 RESOURCE_CONFLICT because private storage is unconfigured. STORAGE_CONFIGURATION_REQUIRED; successful server persistence has not passed.

Mobile and Rust source untouched by this pass; no installation or host/infrastructure change. Original whole-app acceptance remains DOCTOR_WEB_PARTIAL; this checkpoint is scoped to the user-authorized UX changes.

## 2026-10-07 ? Final targeted UX verification

Preview: http://127.0.0.1:3001; final build `NSx9VY1Mkxp3QUUfkUpDg`. Dashboard/account/settings completed first; dedicated Patient Chats followed. Production build, typecheck, lint, format and 11 unit tests PASS. Installed-Edge walkthroughs PASS: 7 prescription + 13 Dashboard/settings + 10 Patient Chats = 30 checks, all on this build, no JavaScript exceptions or unexpected 5xx.

Patient Chats: primary sidebar destination `/doctor/chats`; fluid two-pane desktop; correct selected navigation; back/forward/reload and cold login/MFA pass. In-call Chat remains contextual inside Telehealth, sharing the same thread implementation. Two-Doctor list/read/send authorization and real backend expiry pass. Unsupported category filters removed; unread-count/preview APIs are not supplied and were not fabricated. Dashboard has four supported summaries and one queue; Settings has five coherent full-page sections. All requested rendered states reviewed.

Prescription camera removed. Local file picker/previews/retry/remove work; real POST returns 409 RESOURCE_CONFLICT because document storage is not configured. `STORAGE_CONFIGURATION_REQUIRED`; successful persistence remains unverified. No MinIO/provider/storage replacement configured.

Mobile/backend source hashes unchanged; no installation/system/Docker/browser change. Complete scoped matrices, endpoint mapping, screenshots, reversible fixture-testing scope and host/asset integrity: [DOCTOR_WEB_UX_CORRECTION_REPORT.md](DOCTOR_WEB_UX_CORRECTION_REPORT.md). Whole-app acceptance remains DOCTOR_WEB_PARTIAL, not a whole-prototype parity claim. Stop this correction scope.


## 2026-10-07 ? Prescription compression and progress

AUTO_COMPRESS_400KB_PER_PAGE = PASS. REAL_UPLOAD_PROGRESS = PASS. RETRY_PAYLOAD_PRESERVED = PASS. SESSION_TRANSFER_ABORT = PASS. CAMERA_CAPTURE = REMOVED. PRODUCTION_BUILD = PASS (`6MUCqdkhsIXrtOLwiipGX`). UNIT_TESTS = 18 PASS. TARGETED_BROWSER_CHECKS = 15 PASS plus supplemental narrow visual check. Actual server persistence remains STORAGE_CONFIGURATION_REQUIRED, with genuine 409/429 failures displayed; whole-app verdict remains DOCTOR_WEB_PARTIAL. Mobile/backend source and host software unchanged.

Detailed contract, evidence and host integrity: [DOCTOR_WEB_RX_UPLOAD_REPORT.md](DOCTOR_WEB_RX_UPLOAD_REPORT.md).


## 2026-10-07 ? Schedule correction verified

User-approved Schedule correction VERIFIED on build `vROyHPJLjR3IY3OKge4Ib`: real own-slot PATCH AVAILABLE?BLOCKED, separate cancellation, atomic booking race protection, correct counts/filter/semantic icons and EAT dates. Source=binary=database migration `20261021000001`. Rust format/Clippy PASS, 43 library + 75 API tests PASS (13 new blocking cases); Web 21 units + 12 real browser checks PASS; Flutter analysis and 51 tests PASS. Minimal mobile compatibility exception omits BLOCKED from the legacy booked-patient view. No installation or new infrastructure. Web client diagnostics still log locally rather than feed Admin; this question was answered, not broadened into a telemetry implementation. Full matrix/evidence: [DOCTOR_WEB_SCHEDULE_CORRECTION.md](DOCTOR_WEB_SCHEDULE_CORRECTION.md).
