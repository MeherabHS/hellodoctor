# Doctor Web presentation correction - 2026-10-07

The corrected production preview runs at **http://127.0.0.1:3001**. Build identity:
`vBxYbGtSpLPl4rQ_7Fvhi`. Repository: `main`,
`e3739ae555f198c537dfa7f505fe506b0cd62df2`, with pre-existing uncommitted work
preserved. This report covers the presentation correction, not final acceptance
of the earlier, larger Doctor Web build.

The application now occupies the browser viewport. The rendered mobile shell,
bottom tabs, reserved bottom-tab height, outer frame, shadow, rounded outer
corners and root maximum width are removed. The sole primary navigation is the
canonical Web sidebar: Dashboard, Appointments, Patients, Telehealth, Free Health
Q&A, Settings. Its Patient Queue counter belongs to the actual Web prototype.
At narrow width the same Web navigation becomes a drawer; no mobile navigation
is substituted. Rx, availability, chats and earnings remain secondary workflows.

## Prototype and screen coverage

The canonical source is `prototype/index.html`, not repository-root `index.html`.
Web styles are at lines 2547-3125, Web shell/pages at 8986-9952 and Web routing
at 13128-13235. There are **five actual Web page roots**. Settings has a simulated
toast handler; the Web Rx handler simulates dispatch rather than defining a Web
composer. Neither provides a rendered desktop page to copy pixel for pixel.
The earlier inventory's mobile `doc-view-*` roots are not Web presentation
authority and are excluded from this correction's screen count.

The root fullscreen requirement explicitly overrides the prototype demonstration
frame. Every comparison retains the original canonical capture separately.
`reference_viewport_*` additionally normalizes the workbench/body canvas, shell
width/height/frame, workspace minimum sizes, consultation action wrapping and
EMR panel width. These are disclosed platform adjustments, not unmodified
prototype screenshots. No canonical prototype file was edited.

Evidence below is relative to `audit_evidence/doctor_web/correction/`. Each primary
screen has original canonical, normalized reference and actual Web captures at
1920x1080, 1440x900, 1280x720, 1024x768 and 390x844. The last size exercises the
required Web drawer difference; the prototype supplies no collapsed drawer.

| Screen / surface | Prototype locator | Web component | Current backend | Full-width desktop | Mobile nav removed | Visual compared | Status / limit |
|---|---|---|---|---|---|---|---|
| Web shell | `#doctorWebShell`, `.dw-topbar`, `.dw-sidebar` | DoctorApplication | Profile, duty, appointments | YES | YES | All five sizes, original and normalized | MATCHED_WITH_REQUIRED_WEB_PLATFORM_DIFFERENCE: fullscreen and narrow drawer |
| Dashboard | `#dwPage_dashboard` | Dashboard | Appointments, wallet summary | YES | YES | Canonical and `web_dashboard_*` | BACKEND_DATA_NOT_AVAILABLE for average duration, dispatch totals/recent dispatches and structured clinical alerts; retained Web grid |
| Appointments / queue | `#dwPage_appointments`, `#dwApptCardsGrid` | DesktopAppointments | Doctor appointments | YES | YES | Canonical, empty and populated | MATCHED_WITH_REQUIRED_WEB_PLATFORM_DIFFERENCE: fullscreen; upcoming-first preserved; rescheduling truthfully unavailable |
| Patients | `#dwPage_patients`, `#dwPatientsTable` | Patients | Owned appointments, appointment-anchored history | YES | YES | Canonical and `web_patients_*` | BACKEND_DATA_NOT_AVAILABLE for independent patient registration and authoritative allergy/chronic classifications; authorized table and history retained |
| Telehealth | `#dwPage_telehealth`, `.dw-meet-container` | Telehealth | Summary, history, chats | YES | YES | Canonical, empty, populated, in-call chat | DEFERRED_EXTERNAL_INTEGRATION for RTC; Web workspace and EMR geometry corrected |
| Free Health Q&A | `#dwPage_qna`, `#dwQnaListContainer` | Qna | Question list/detail/answers | YES | YES | Canonical and `web_qna_*` | BACKEND_DATA_NOT_AVAILABLE for helpful-vote aggregate; original distinct Web layout retained |
| Settings / profile | Web Settings toast handler; shared profile flow | ProfileSettings | Doctor profile, duty | YES, desktop dialog | YES | `web_settings_dialog_1440.png`; no real Web settings page in prototype | MATCHED_WITH_REQUIRED_WEB_PLATFORM_DIFFERENCE; existing allowed edits retained |
| Chat list/thread | `#dwMeetChatView`, `#dwChatMsgStream`, `#dwChatInputField` | Chat, ChatThread | HTTP chats/messages | YES, rail + thread | YES | Web thread classes and `web_chat_populated_1440.png` | MATCHED_WITH_REQUIRED_WEB_PLATFORM_DIFFERENCE; canned replies and expiry preserved; no prototype standalone list root |
| Smart Rx | `triggerDoctorWebQuickRx`, `#btnDwSendRx` | Rx | Existing prescription upload | YES, two columns | YES | `web_rx_*`, `web_rx_populated_1440.png`; Web handler has no composer DOM | MATCHED_WITH_REQUIRED_WEB_PLATFORM_DIFFERENCE for desktop workspace; storage unavailable, no upload persistence PASS |
| Availability | Web calendar / Add control; `#slotGeneratorModal` | Schedule, SlotForm | Doctor slots | YES, date strip + grid | YES | `web_schedule_*`, populated and slot dialog | MATCHED_WITH_REQUIRED_WEB_PLATFORM_DIFFERENCE; booked/locked protection checked |
| Earnings | Dashboard earnings KPI; shared statement flow | Wallet | Summary, ledger | YES, breakdown + table | YES | `web_wallet_*`; no standalone canonical Web earnings root | MATCHED_WITH_REQUIRED_WEB_PLATFORM_DIFFERENCE; server values, KES and scheduled Finance payout |
| Monthly statement / receipt | `#doctorStatementModal`, `#docConsultationFeeModal` | Statement, Receipt | Month statement, ledger entries | YES, desktop dialog | YES | Statement canonical source and populated screenshot; receipt not separately accepted | PROTOTYPE_BUSINESS_RULE_CONFLICT_RESOLVED for charge/estimated/final labels; complete receipt visual acceptance NOT_COMPLETE |
| EMR / episode | `#dwMeetHistView`, `#patientHistoryModal` | Telehealth, History, Episode | Appointment-anchored history | YES, side panel/dialog | YES | Populated EMR and history screenshots | Desktop presentation checked; every historical episode state NOT_COMPLETE for full parity |
| Documents / labs | `#dwHistLabBtn`, `#labReportModal` | Documents, DocumentViewer | Authorized document access | YES, desktop dialog | YES | `web_documents_dialog_1440.png` | BACKEND_DATA_NOT_AVAILABLE for lab-specific records; authorized empty state checked, real file-byte viewer not verified |
| Summary / completion | Appointment chart and Telehealth completion controls | Summary, Completion | Existing summary/complete/complete-no-rx | YES, desktop dialog | YES | Summary/completion populated screenshots | Form/context guard checked; no new completion mutation or financial credit in this pass; exhaustive modal parity NOT_COMPLETE |
| Payout settings | Shared wallet/profile secondary flow | PayoutSettings | Existing payout profile/destinations | YES, desktop dialog | YES | `web_payout_dialog_1440.png` | External provider deferred; no Withdraw Now; full provider interaction NOT_COMPLETE |
| Authentication / MFA | Shared `#doctorAuthSection` | Auth | Login, MFA, session, logout | Existing auth form | YES | Actual Edge UI login/MFA/reload/logout | Verified correction regression; registration and full multi-tab lifecycle not rerun |

The raw Web controls inventory contains **83 static interactive elements**:
12 shell, 11 Dashboard, 12 Appointments, 19 Patients, 24 Telehealth and 5 Q&A.
Repeated demo patient controls are replaced with equivalent controls on owned
backend rows, not preserved as fake production patients. Dynamic Q&A/EMR rows,
non-handler form controls and shared dialogs require separate coverage; 83 is
not the total interaction count and is not a claim that 83 actions were tested.
See the current prototype inventory for their implementation dispositions.

## Verified results

| Required result | Result |
|---|---|
| FULLSCREEN_WEB_SHELL | PASS |
| ROOT_USES_VIEWPORT | PASS |
| OUTER_FLOATING_DEVICE_CONTAINER_REMOVED | PASS |
| DESKTOP_BOTTOM_MOBILE_NAV_REMOVED | PASS |
| DUPLICATE_NAVIGATION_REMOVED | PASS |
| SMART_RX_DESKTOP_LAYOUT | PASS |
| TELEHEALTH_DESKTOP_LAYOUT | PASS |
| QNA_DESKTOP_LAYOUT | PASS |
| APPOINTMENTS_DESKTOP_LAYOUT | PASS |
| DASHBOARD_DESKTOP_LAYOUT | PASS |
| ALL_PROTOTYPE_WEB_SCREENS_ACCOUNTED_FOR | YES - five canonical Web roots plus shared/secondary surfaces above |
| REAL_BACKEND_DATA | YES - runtime API data; controlled backend fixtures only for evidence |
| MOBILE_APP_MODIFIED | NO - correction baseline hash comparison |
| BACKEND_MODIFIED | NO by this correction; separate workspace changes disclosed below |
| PRODUCTION_BUILD | PASS |
| BROWSER_WALKTHROUGH | PASS for this corrective regression scope |
| VISUAL_PARITY | PARTIAL - not a full pixel/interaction acceptance of every shared dialog |

Web typecheck, lint, formatting checks, **10 unit tests** and production build
passed. Installed Edge produced **45 screen/viewport checks** across nine
workspaces and five sizes. Every shell spans the viewport with no frame or root
maximum width; there is no rendered `doctorAppShell` or mobile bottom-tab node,
no horizontal page overflow and no unhandled JavaScript exception. The UI
password/MFA login, reload restoration, slot-dialog Escape and logout checks pass.

The separate populated regression has **14 passing checks**: server-confirmed
duty with original state restored, upcoming-first order, foreign summary/history
denial, authorized history/documents, Web EMR/in-call chat, two-page Rx failure
recovery/retry/removal, completion form, booked-slot cancellation protection,
Web canned replies, actual HTTP message/canned sends and wallet/statement/payout
controls. Chat POST returns the current correct **201** response. An earlier
harness assertion expected 200; the harness was corrected, not the API.
HTML-like chat text remains escaped. Network logs omit tokens/query strings.

`visual-browser-results.json` and `workflow-results.json` contain exact checks,
geometry and sanitized response records. Populated tests use previously created
synthetic development fixtures and test-only claims, not a second password/MFA
acceptance claim. The real UI auth check is independent. No new Doctor, Patient
or payment fixture was created in this correction pass.

## Boundaries and remaining evidence

Storage returns the real `409 RESOURCE_CONFLICT` stating it is not configured.
Both Rx pages remain recoverable; no fake persistence or local storage service
was introduced. Live camera hardware, successful persisted Rx/document viewing,
RTC, external payouts, fresh slot creation/cancellation and new consultation
completion were not claimed as newly verified. Broader registration,
multi-tab/revocation/account-switch, all IDOR families and exhaustive shared
dialog visual acceptance remain outside the completed correction regression.
Missing aggregates use truthful supported empty states. No financial calculation
or frontend clinical classification replaces backend truth.

The correction is complete for the reported fullscreen/duplicate-navigation/
phone-width presentation defects. The larger original build remains
**DOCTOR_WEB_PARTIAL**, and this report does not declare NOT_COMPLETE = 0 or
DOCTOR_WEB_VERIFIED. Notifications/articles were not changed or expanded.

## Host and source integrity

```text
DOCTOR_MOBILE_FILES_CHANGED = NONE
BACKEND_FILES_CHANGED = NONE
HOST_SOFTWARE_INSTALLED = NONE
HOST_SOFTWARE_UNINSTALLED = NONE
WINDOWS_SERVICES_CREATED = NONE
GLOBAL_PACKAGES_INSTALLED = NONE
NEW_DOCKER_INFRASTRUCTURE = NONE
MINIO_REINTRODUCED = NO
BROWSER_INSTALLED = NO
PLAYWRIGHT_BROWSER_DOWNLOADED = NO
ANDROID_TOOLING_INSTALLED = NO
PATH_MODIFIED = NO
REGISTRY_MODIFIED = NO
FIREWALL_MODIFIED = NO
NO_INSTALLATION_PERFORMED = YES
```

Only existing Node, Python/Playwright and installed Edge were used. Preview is
the already approved loopback-only port 3001, not a Windows service. Existing
uncommitted backend changes from earlier work are not changes by this pass.
Source preservation evidence is `source-baseline.json` and
`source-integrity-final.json`. Flutter hashes are unchanged. The final baseline
comparison also detects separate changes in `notification_repo.rs`,
`notification_service.rs` and `tests/api_tests.rs`. This correction did not edit
those files; their changes are preserved, and an entirely unchanged backend
workspace is not claimed. No dependency manifest/lockfile was changed in
this correction. Stop here; no backend/mobile architecture cycle is reopened.


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
