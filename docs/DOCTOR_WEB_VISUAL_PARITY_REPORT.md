> Latest functional fixes and user-requested removals: [follow-up report](DOCTOR_WEB_FUNCTIONAL_FOLLOWUP.md). Older results below retain their original scope.

> Current presentation result: [Doctor Web correction report](DOCTOR_WEB_CORRECTION_REPORT.md). The older combined mobile/Web acceptance below is historical; mobile roots are not Web visual authority.

# Doctor Web visual parity — interim evidence

Canonical source: `prototype/index.html`. Both `doctorAppShell` and the separate
`doctorWebShell` were inspected. The full original CSS and exact font weights
were extracted; prototype scripts and demo data are excluded from the runtime.
This establishes source fidelity only, not rendered acceptance.

The installed Edge browser rendered 55 canonical references: 11 major surfaces
at widths 1440, 1280, 1024, 768 and 390. Evidence is in
`docs/audit_evidence/doctor_web/reference`. The later capture hides the prototype
workbench and toast containers without changing Doctor shell geometry. The
source HTML itself is untouched. The mobile references remove the demo handset
bezel; that platform difference requires explicit acceptance in comparisons.

Real Web screenshots and network/browser checks are in `web` and `workflow` in
the same evidence directory. Controlled synthetic fixture Doctors/patients were
used. No saved screenshot contains session tokens or production patient data.

| Canonical screen | Web component | Render evidence | Acceptance |
|---|---|---|---|
| Queue | Queue | workflow/queue_populated_1440.png; web/queue widths | NOT_COMPLETE |
| Chat list and thread | Chat | workflow/chat_thread_1440.png; chat widths | NOT_COMPLETE |
| Smart Rx | Rx | workflow/rx_storage_unavailable_1440.png; web/Rx widths | NOT_COMPLETE |
| Schedule | Schedule | workflow/schedule_booked_1440.png; web/schedule widths | NOT_COMPLETE |
| Wallet | Wallet | workflow/wallet_1440.png; web/wallet widths | NOT_COMPLETE |
| Separate mobile Q&A | Qna | web/qna_390.png | NOT_COMPLETE |
| Desktop Dashboard | Desktop Dashboard | web/desktop_dashboard_1440.png | NOT_COMPLETE |
| Desktop Appointments | DesktopAppointments | web/desktop_appointments_1440.png | NOT_COMPLETE |
| Desktop Patients | Patients | web/desktop_patients_1440.png | NOT_COMPLETE |
| Desktop Telehealth | Telehealth | web/desktop_telehealth_1440.png | NOT_COMPLETE |
| Desktop Q&A | Qna | workflow/qna_answered_1440.png | NOT_COMPLETE |

The listed screenshot files exist; the final review must
check the current build before accepting any row. Some existing
workflow captures predate the latest dialog and Rx controller changes. No old
capture is treated as proof of the latest rendered output.

Accepted `MATCHED` screen count: **0**. Source-based or build-based visual PASS
is prohibited. Current desktop/mobile shell combination, extra desktop bottom
navigation, modal geometry and responsive header behavior require correction
or an evidence-supported required platform difference. There is no visual PASS
for those differences yet.

The statement, fee-receipt, slot-generator and document surfaces now reuse their
actual canonical wrappers. Their latest screenshot comparison remains pending.
Patient-history/footer controls, camera behavior, desktop drawers and every
remaining interaction must be checked against the recursive 887-element
inventory before visual acceptance.

Stale prototype business copy is replaced only where necessary: KES/Kenya,
KMPDC, platform charge, estimated versus final payable, Finance-scheduled payout,
handwritten prescription images and honest RTC/storage/provider availability.
These are `PROTOTYPE_BUSINESS_RULE_CONFLICT_RESOLVED` changes, not a license to
redesign the original structure.


## Latest Web-only correction evidence - 2026-10-07

Current build: `vBxYbGtSpLPl4rQ_7Fvhi`. The old hybrid-shell screenshots/statuses
above describe the preceding implementation, not the current one. The actual
canonical five Web pages were rendered in installed Edge. Original `canonical_*`
captures remain distinct from explicitly normalized `reference_viewport_*`
captures. Normalization covers the user-required fullscreen shell and flexible
consultation workspace/action wrapping; the prototype file is unchanged.

45 actual Web screen/viewport checks pass: nine workspaces at 1920x1080,
1440x900, 1280x720, 1024x768 and 390x844. Geometry proves viewport width, zero
outer offset/radius/shadow, no root max-width, no rendered mobile shell/tab nodes
and no horizontal page overflow. Actual screenshots were inspected against
rendered canonical/normalized references, including all five primary Web pages.
Populated and dialog captures add clinical/workflow evidence.

The current screen table, comparison differences and data gaps are in
[DOCTOR_WEB_CORRECTION_REPORT.md](DOCTOR_WEB_CORRECTION_REPORT.md).
The report separates retained prototype Web geometry from required fullscreen,
narrow Web drawer and desktop secondary-workflow differences. Global visual
parity remains PARTIAL: no unmodified canonical Web Rx/settings page exists;
some real data is absent; exhaustive shared-modal/control acceptance is not
finished. This is not a source-only visual PASS or a pixel-perfect claim.


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

## 2026-10-07 ? User-approved UX rendered acceptance

Final build `NSx9VY1Mkxp3QUUfkUpDg`. Dashboard/account/five Settings sections and dedicated Patient Chats selected/unselected/empty/read-only/contextual-in-call states were rendered and visually reviewed. Dashboard/settings evidence covers 1920, 1440, 1024, 390px; Chat adds 1280px. Corrected chat card density and text separators after the first rendered review. Fluid shell and two panes confirmed; Patient Chats highlights independently of Telehealth.

Evidence: [Dashboard/settings screenshots](audit_evidence/doctor_web/dashboard-settings/), [Patient Chats screenshots](audit_evidence/doctor_web/patient-chats/), [prescription evidence](audit_evidence/doctor_web/rx-upload/). These approved information-architecture improvements retain prototype visual language and are not strict replicas of the obsolete mixed Profile modal/unsupported Dashboard widgets. Scoped VISUAL_REVIEW = PASS; full original-prototype VISUAL_PARITY remains PARTIAL. Decisions and final matrices: [UX correction report](DOCTOR_WEB_UX_CORRECTION_REPORT.md).


## 2026-10-07 ? Prescription compression and progress

Rendered Rx preparation/progress reviewed on production build `6MUCqdkhsIXrtOLwiipGX`: desktop progress is next to Send Prescription; 1440/1024/390 checks have no horizontal overflow. A supplemental narrow capture starts at 390 and disables transition animations, showing the collapsed Web drawer and visible actual progress. This is scoped Rx visual evidence, not a new whole-prototype parity claim.

Detailed contract, evidence and host integrity: [DOCTOR_WEB_RX_UPLOAD_REPORT.md](DOCTOR_WEB_RX_UPLOAD_REPORT.md).


## 2026-10-07 ? Real slot blocking and semantic cards

Schedule state hierarchy and confirmations rendered/inspected at 1920, 1440, 1024 and 390 on build `vROyHPJLjR3IY3OKge4Ib`. Available clock/no avatar, amber blocked lock, real booked initials, payment hourglass without destructive controls, muted cancelled calendar. Distinct block/cancel dialogs reviewed. Targeted visual review PASS; no whole-prototype parity upgrade is implied.

Contract, tests, mobile exception, telemetry findings and full report: [DOCTOR_WEB_SCHEDULE_CORRECTION.md](DOCTOR_WEB_SCHEDULE_CORRECTION.md).
