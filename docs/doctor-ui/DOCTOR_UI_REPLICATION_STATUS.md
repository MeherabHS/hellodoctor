# Doctor UI — Replication Status

Updated 2026-11-13. Scope: `apps/mobile`. Target: `docs/design/afyamkononi-doctor-ui-reference-11-panel.png` (panels 1–11) and the 14-panel board (panels 12–14). Uncommitted, on branch `feature/patient-ui-replication` alongside the Patient work. No backend, Doctor Web, Admin Web or Patient file was changed.

`flutter analyze`: no issues. `flutter test`: 324 passed, 10 failed — the same ten golden-image tests that failed before any change (Windows font drift against Linux-captured goldens; not confirmed on Linux). The goldens for the notification centre and Q&A will also need regenerating on CI because the app-wide green and background changed.

## Product-owner decisions recorded (2026-11-13)

1. Panels with no backend (7, 8, 9, 14): build to the board; sample content only in the design preview; honest empty states in the real app.
2. Four tabs. Chat inside Appointments; Scan Rx, Wallet, Q&A and Articles under More; duty toggle on Home.
3. The existing completion screen will be wired into the mobile flow as the home of panel 7.
4. Artwork problems are dealt with later; work proceeds with interim images.
5. UI is viewed through the design-preview entry point rather than by signing in.

## Batch A — authentication, registration, approval

| Screen | Panel | State | Capture (`screenshots/batch-a/`) |
|---|---|---|---|
| Login | 1 | Built to the panel | `01-login` |
| Login, emailed-code step | 1 | Built in the same layout; not captured (needs a real account) | — |
| Sign Up | 12, phone 1 | Built to the panel | `02-sign-up`, `02b-sign-up-filled` |
| Professional Details | 12, phone 2 | Built to the panel | `03-professional-details` |
| Account Under Review (after registering, and as the signed-in gate) | 12, phone 3 | Built to the panel | `04-account-under-review` |
| Registration not approved / verification withdrawn / unavailable | none | Derived from the panel | `06-registration-not-approved` |
| Splash | none | Brand header and spinner | — |
| Forgot password, reset code, new password, reset success | none | New colours, buttons and bars through the theme; layout not yet reworked | `05-forgot-password` |
| Legal and professional agreements | none | Theme only; layout not yet reworked | — |

### Differences from the panels, and why

- **Login artwork is interim.** `assets/auth/login_scene.jpg` is cut from the board and enlarged five times, so it is soft and its top edge is a straight cut. It needs a separate high-resolution file.
- **"Forgot password?" and "Register as Doctor"** sit under the 2FA note. The panel does not draw them; they are required.
- **Sign Up has a fourth field, Phone Number,** and a one-line password rule. The server requires a Kenyan phone number.
- **Professional Details uses typed fields, not dropdowns,** and has two more fields (Qualifications, Residential Address). The server takes free text for specialty and facility, there is no approved list to choose from, and all six values are required. "Facility / Organization" carries a required mark because the server requires it.
- **The legal agreements step** follows "Next". The panel omits it; it is mandatory.
- **The signed-in gate's button reads "Refresh status"** with "Log out" beneath, instead of "OK", because the doctor is signed in and waiting.
- **Checklist ticks are fixed text,** as on the panel. The server reports one status, not per-step progress.
- Tall layouts: the panel is drawn at about 411 × 986 dp; the emulator is 411 × 923 dp, so vertical gaps on Login are slightly tighter than measured.

### Code

New: `lib/core/widgets/afya_auth_widgets.dart` (logo, brand header, fields, button, security note, review-status view), `lib/dev/design_preview_main.dart`, `assets/brand/afya_logo.png` (the Patient app's logo), `assets/auth/login_scene.jpg`.

Changed: `lib/app.dart` (splash, login, registration as two steps, verification gate; `AuthCubit` calls, password and phone checks, and the request sent are unchanged), `lib/core/theme/tokens.dart` (board colours; app bar, button and background defaults), `lib/features/auth/legal_consent_page.dart` (submitted state uses the shared view), `pubspec.yaml` (asset folders).

Tests changed: `test/kenya_phone_test.dart` (helper follows the two registration steps; same three assertions), `test/doctor_client_test.dart` (expects the new primary green).

The client now recognises the server's `REVOKED` status and words it as "Verification Withdrawn".

### Finished after the first pass

Forgot password, reset code, new password and reset success now share one layout (logo, centred heading, icon fields, the green button); the legal agreements list uses bordered cards. Their logic is unchanged. They were not re-captured after this change.

### Still to do in Batch A

- Capture the reset and agreement screens; capture at 360 dp, in Swahili and at text scale 1.3. Auth copy is still hardcoded English.
- Final login artwork.

## Batch B — shell, Home, Appointments, Availability

Captures are in `screenshots/batch-b/`, taken in the design preview (sample data, labelled).

| Screen | Panel | State | Capture |
|---|---|---|---|
| Bottom navigation: Home, Appointments, Patients, More | 2–11 | Built to the board | all |
| Home | 2 | Built to the panel: logo bar, greeting, four counters, three quick actions | `01-home` |
| Appointments | 3 | Built to the panel: Today / Upcoming / Completed, appointment rows with status pill | `02-appointments`, `07-completed-tab` |
| Patient Details (interim) | 4 | Header and info rows in the panel's style, carrying the working actions. The tabs, files and "Open Consultation" of panel 4 come in Batch C | `05-appointment-detail` |
| Patients | 8 | Honest unavailable state; there is no patient-list backend | `03-patients` |
| More | none | Rows in the style of panel 10 for every feature that lost its tab | `04-more` |
| Availability | 13 | Existing schedule screen with the board palette, a back arrow and the Available / Booked / Blocked legend. Not yet laid out as panel 13 | `06-availability` |

### Differences from the panels, and why

- **Counters come from the appointment list** the app already loads: Today is everything scheduled today, Upcoming is confirmed work not yet finished, In Progress is live sessions, Completed is completed today. No request was added.
- **Initials instead of portraits.** Patients have no photographs in this system.
- **Home has an Online / On break switch** under the counters. The board does not draw it; it is the app's existing duty toggle, kept on Home by decision.
- **Appointments has a chat icon** in its title bar, opening the consultation chat list ("Chat inside Appointments").
- **Tapping an appointment** opens the interim details screen rather than acting from the row, because the panel's rows have no buttons.
- **Availability** keeps per-date slots, "Create 10-Min Slots", block, unblock and cancel exactly as before. Panel 13's weekday chips, working hours, slot-duration picker and video switch have no backend and are not drawn.
- **Screens reached from More** (chats, prescriptions, wallet, Q&A, articles, notification settings, security, profile) still have their old layouts with the new colours.

### Code

New: `lib/features/home/home_pages.dart` (Home, Appointments, interim details, Patients, More and their shared row, tile and title-bar widgets).

Changed: `lib/app.dart` (four tabs; Home and Appointments are refreshed by the same 30-second and on-notification refresh the Queue tab had), `lib/core/widgets/doctor_widgets.dart` (page header shows a back arrow when the screen was pushed), `lib/features/settings/settings_pages.dart` (old Settings home removed; Security kept), `lib/features/doctor/doctor_pages.dart` (schedule banner colours and legend; two text defects fixed: history rows printed raw timestamps with "?" separators, and the blocked-slot line had a "?"), `lib/dev/design_preview_main.dart` (past encounters in the sample list).

Tests changed: `doctor_navigation_safe_area_test.dart` (new tab labels; opens Patients instead of Schedule), `queue_lifecycle_refresh_test.dart` (tab names), `doctor_widgets_test.dart` (the corrected blocked-slot wording).

### Known loose ends

- `QueuePage` and `AppointmentCard` are no longer shown anywhere but are still in the code and still covered by tests. Their action logic (`ConsultationActions`) is what the new details screen uses. Remove the old page and move its tests when Batch C replaces the details screen.
- The consultation-completion screen is still not wired in. That is Batch D.
- Availability needs its panel 13 layout.

## Welcome screen and Peer Navigator form (designs supplied 2026-11-13)

References: `docs/design/afyamkononi-doctor-welcome-reference.png`, `docs/design/afyamkononi-peer-registration-reference.png`. Captures in `screenshots/batch-cde/`.

| Screen | State | Capture |
|---|---|---|
| Welcome ("Welcome, Team", two role cards, Sign In) | Built to the design; now the signed-out entry screen | `01-welcome` |
| Register as Peer Navigator | Built to the design. **Not connected:** there is no Peer role or endpoint on the server. The form checks its fields, then states that applications are not open; it sends nothing and creates no account. No KMPDC or other clinical field is asked for | `02-peer-registration`, `03-peer-registration-lower` |
| Login | Reached from "Sign In"; now uses the high-resolution scene cut from the welcome design, replacing the earlier soft image | `04-login` |

Differences: the stethoscope icon is a medical-bag glyph (no stethoscope in the bundled icon set); the small accent strokes beside "Welcome, Team" are not drawn; Location is Kenya's 47 counties (granularity is still an open decision); the Terms and Privacy links in the Peer form are styled but open nothing, since Peer terms do not exist yet.

## Batch C — details, waiting room, call

| Screen | Panel | State | Capture |
|---|---|---|---|
| Patient Details | 4 | Built to the panel: header, Overview / History / Files, info rows, "Open Consultation". Replaces the interim screen | `05-patient-details` |
| Waiting Room | 5 | Built to the panel, with a natively drawn stand-in for the illustration | `06-waiting-room` |
| Video Consultation | 6 | Title added over the call. Controls unchanged | not captured (needs a live call) |
| Chat thread | none | New colours only | — |

Differences: no sex, age or patient number (the server gives a doctor none of these; the appointment number is shown instead); "Key Information" is replaced by the patient's own intake note, labelled as that; "Patient is waiting" and the joined line appear only when the server reports the patient present, otherwise the screen says the room is open; "Start Consultation" is disabled outside the server's join window and shows when the room opens. On panel 6 the timer, in-call chat, notes, checklist and screen-share are not built.

## Batch D — outcome and prescription

| Screen | Panel | State | Capture |
|---|---|---|---|
| Consultation Notes / outcome | 7 | The existing completion screen is now reachable ("Record outcome" on Patient Details). It records the four real outcomes. The panel's notes, diagnosis, plan and follow-up fields are **not offered** because nothing stores them; a notice says so | `09-consultation-outcome` |
| Prescription | 14 | Existing page-capture flow, with a "Prescription Submitted!" confirmation once every page is confirmed by the server. No structured medicine entry | not captured |
| Patient Record | 9 | Not built; no backend. The Patients tab states that records are not available | `batch-b/03-patients` |

The outcome screen still has its old card layout; only its title, wording and the notice changed.

## Batch E — notifications and profile

| Screen | Panel | State | Capture |
|---|---|---|---|
| Notifications | 11 | Filter chips (All, Appointments, Messages, System) and rows with an icon and unread dot. Filtering is on the device | `07-notifications` |
| My Profile | 10 | Built to the panel: header with photo, specialty and Verified Doctor badge; four rows | `08-my-profile` |
| Professional profile, Security, notification settings, Q&A, articles, wallet, chat list | none | New colours only | — |

Differences: the board's "Consultation Settings" row has nothing behind it, so the third row is Notification Settings; Security still offers sign-out only; there is no "System update" feed.

## Sign-in, Home quick action and Schedule (designs supplied 2026-11-13)

- **Sign-in** rebuilt to `docs/design/afyamkononi-doctor-signin-reference.png`: logo with tagline, the doctor and Peer Navigator scene, "Welcome, Team", fields, Sign In, 2FA note, "Forgot password?". Registration is reached from Welcome only. Capture: `batch-cde/04-login`.
- **Home quick actions** gained "Answer HIV-related open questions from patients" (opens the existing question-and-answer screen, shows the unanswered count). The same entry on More was renamed. The Q&A screen's own headings are unchanged.
- **Schedule** rebuilt to `docs/design/afyamkononi-doctor-schedule-reference.png`: logo bar with bell and refresh, doctor illustration beside the title, and it now lives inside the Appointments tab with the bottom navigation visible (calendar icon on Appointments; also opened from Home, More and My Profile). Slot logic is unchanged. Capture: `peer-and-schedule/01-schedule`. Differences: the slot cards keep their existing layout, slightly larger than drawn; the action icons beside "Follow-up messages" and "Write prescription" are not drawn.

## Peer Navigator screens (designs supplied 2026-11-13)

References: `docs/design/afyamkononi-peer-{dashboard,clients,client-profile,messages,profile}-reference.png`. Code: `lib/features/peer/peer_pages.dart`. Captures: `peer-and-schedule/02`–`08`.

**These are presentation only.** The server has no Peer Navigator role, client list, requests, conversations or calls. The screens take their content as a value and nothing in the signed-in app can reach them. They are viewable only in the design preview:

`flutter run -t lib/dev/design_preview_main.dart --dart-define=PREVIEW_ROLE=PEER`

| Screen | State |
|---|---|
| Dashboard | Built to the design: hero, identity card with Approved badge, three overview tiles, new request with Accept / Decline, four quick actions, banner |
| My Clients | Built to the design: hero, counters, search, filter chips, Add Client, client rows with status pills |
| Client profile | Header card, four actions and tabs built to the design. The Care Progress, Recent Interactions and Latest Note cards are **empty states**: sample clinical detail about a client is not invented |
| Messages | Built to the design: hero, search, filter chips, conversation rows with unread counts, confidentiality note, Compose |
| Conversation | Reuses the Doctor app's chat thread screen, as instructed |
| My Profile | Built to the design: photo, role pill, ID, bio, Edit Profile, four statistics, three rows |
| Notifications tab | Placeholder; no design supplied |

Differences and limits:
- The five designs draw five different bottom bars. One is used throughout: Home, Clients, Messages, Notifications, Profile.
- Every button that would act (Accept, Decline, Add Client, Call, Schedule Follow-up, Compose, Edit Profile, search, filters) shows "Not connected yet".
- Names, counts and message previews are sample content from the designs, shown under the preview ribbon. "Total Clients" and the other client counters are counted from the five sample rows, so they read 5 / 2 / 1 rather than the design's 42 / 28 / 8.
- Client rows use initials; the client photograph in the design is not reproduced. Age and sex are not shown.
- In the reused chat thread the sample messages come from the Doctor fixtures ("Sample doctor reply") and both sit on the left, because the thread aligns messages by the signed-in doctor's identity.
- Artwork: the navigator portrait and the banner are cut from the designs at their native size (about 225 px wide), so they are slightly soft on a high-density screen. The hospital illustration in the hero backgrounds is not drawn.

## Tests

`flutter analyze`: no issues. `flutter test`: 322 passed, 12 failed. All twelve are golden-image comparisons (eight Q&A, four notification centre). Ten failed before this work from platform font drift; the notification-centre images now also differ because that screen was redesigned. The reference images need regenerating on CI.

## Remaining

- Panel 13 layout for Availability; panel 6 in-call chat; restyling of the screens marked "new colours only".
- Remove the unused Queue screen and move its tests.
- Captures at 360 dp, in Swahili and at text scale 1.3; move copy into the translation files.
- Artwork: waiting-room illustration; stethoscope icon.
- Peer Navigator backend, admin review, and a role-aware shell that shows the Peer screens to an approved Peer after sign-in (separate approval). Until then the Peer screens stay preview-only.
