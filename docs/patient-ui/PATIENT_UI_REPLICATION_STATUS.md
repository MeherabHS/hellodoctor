# Patient UI — Replication Status

Updated 2026-11-13 (sixth pass). Branch `feature/patient-ui-replication`, uncommitted. Device: `Pixel_9` emulator, Android 17 (API 37), 1080 × 2424 px at 420 dpi (about 411 × 923 dp), text scale 1.0, debug build against the local backend.

- Emulator screenshots: `screenshots/replication/pixel_9_api37/`
- Reference-beside-emulator pairs at the same horizontal scale: `screenshots/replication/comparisons/`
- `flutter analyze`: no issues. `flutter test`: 462 of 462 pass (31 added for the auth layouts, 16 for reminders, 8 for the completion screen). No overflow reported in the run log.

Open defect, not yet diagnosed: the app signs itself out when it should not.

- Signing in to one existing test account returns to the log-in screen right after the code is accepted. The backend creates the session, then the first authenticated requests get 401 and the app signs out. A newly registered account signs in and out normally through the same path.
- Seen once on the newly registered account too: airplane mode on, pull-to-refresh on Home while offline, airplane mode off, refresh again; the app went to the log-in screen. Repeating airplane mode on and off without the offline refresh kept the session. This sequence is a reproducible starting point.

## 1. Artwork

| Asset | Source | State |
|---|---|---|
| Home hero girl, Peer Navigator, Doctor | Product owner's high-resolution regenerations (transparent PNG), trimmed and scaled to 900 px tall | In the build; sharp on device |
| Topic thumbnails: HIV Basics, SRH, Stigma & Disclosure | Product owner's characters composed onto the reference's thumbnail background colours, 480 px | In the build |
| Topic thumbnails: PrEP, Condoms | Product owner's images, cropped inside their rounded background, 480 px | In the build |
| Topic thumbnail: Healthy Relationships | Product owner's heart on the reference's pink, 480 px | In the build |
| Logo mark | Redrawn as vector shapes from the reference, 512 px transparent PNG | In the build |
| Support-chat category tiles | Native Flutter gradient tiles with vector glyphs | In the build |
| Facilities map strip | Still an enlarged crop of the reference, shown dimmed | **Not replaced** |

The character PNGs sit on native Flutter backgrounds (banner gradient, mint card, blue card with inner panel), so no artwork carries embedded UI.

## 2. Screens

| Screen | Reference | Screenshot | State | Differences that remain |
|---|---|---|---|---|
| Home | Panel 1 | `01-home` | Rebuilt; three comparison passes. All nine cards and the bottom navigation are visible without scrolling | Card 3 reads "Book Consultation". Tile glyphs are Material icons, close to the reference's. The reference phone is taller, so its tiles are slightly taller than ours |
| Bottom navigation | Panel 1 | all | Done | Messages and Services glyphs are Material equivalents |
| Services | none | `09-services` | Derived from the design language | Needs visual review |
| HIV Prevention & Sexual Health | Panel 2 | `02-education` | Built, compared, corrected | "3 min" lines omitted (no such data). A topic opens the article list with the topic as search text. Videos and Audio show unavailable states |
| HIV Support Chat | Panel 3 | `03-support-chat` | Built, compared | Rows slightly shorter than the reference. Glyphs are Material icons plus a drawn ribbon |
| Peer Navigator | Panel 4 | `04-peer-navigator` | Built, compared | "Start Chat" is disabled with a note, so it is paler. "(SMS/WhatsApp)" removed from one line |
| Reminders | Panel 5 | `05-reminders` | Layout only | No reminder cards (no backend): tabs, empty state, privacy note |
| Health Records | Panel 6 | `06-health-records` | Built, compared | Rows limited to what exists: All Records, Prescriptions, Documents. Share Access shows an unavailable state |
| Facilities & Services | Panel 7 | `07-facilities` | Layout only | No facility cards (no backend). Search, chips and "View on Map" disabled. Last chip scrolls off the right edge |
| Video Consultation | Panel 8 | `08-video-consultation` | Built, compared | Button reads "Book a Video Consultation" and opens doctor discovery. Urgent card shows 999. The patient's upcoming consultation, when one exists, appears under the button |
| Privacy & Security | Panel 11 | not captured | Built | Reached from More → Privacy Policies |
| Messages | none | `10-messages` | **Redesigned** from the design language | Needs visual review. No message preview, time or unread badge (the API returns none). Only the empty state has been seen on device; the populated card has not |
| Profile | none | `11-profile` | **Redesigned** from the design language | Needs visual review. "Payment Methods" and "Report an Issue" rows removed (both were "coming soon" snackbars) |
| Auth, booking, waiting room, call, chat thread, consultation status, article reader, notifications, profile sheets | none | not captured | **Not redesigned** | They pick up the new colours through the theme only |

## 2A. Auth screens (added in the fourth pass)

Designs supplied by the product owner: "Welcome Back Login", "Forgot Password" and "Healthcare Registration" (1024 × 1536 each).

| Screen | Screenshot | State | Differences from the design |
|---|---|---|---|
| Log in | `20-login` | Rebuilt | The painted scenery beside and below the card is replaced by a blurred copy of the artwork. The small spark beside "Welcome back" is not drawn |
| Forgot password | `22-forgot-password` | Rebuilt | Label and sentence say email and code only, because recovery is by email code. Scenery below the card is the blurred copy |
| Create account | `21-register`, `21b-register-lower`, `21-register-360x640` | Rebuilt | Keeps required email, date of birth and gender (the design shows four fields). Password hint states the real rule. The two characters in the design's bottom corners are not shown |

How they adapt to screen size: only the artwork above the form is used, as a header that scales with the screen width (capped at 42% of the height). The card is a native widget that takes the height of its content, and the page scrolls when it does not fit. Nothing is aligned to UI painted into the artwork. A first attempt that laid native cards over the full painted designs was discarded because it only lined up at one screen shape.

Verification: `test/auth_scene_adaptive_test.dart` renders all three screens at 320 × 568, 360 × 640, 411 × 923, 800 × 1280 and 740 × 360 (landscape), at text scale 1.0 and 1.3, plus log in with the keyboard open: no overflow, the primary button reachable and at least 48 high, and exactly one primary button on forgot password. On the emulator, registration was also viewed at 360 × 640 dp.

Not redesigned: email code, reset code, new password, reset success, profile completion.

Auth copy is still hardcoded English, as it was before; it is not yet in the localization files.

## 2B. Fifth pass (2026-11-13): notifications, Find a Doctor, reminders, support topics

Designs supplied: "Notifications Empty State", "Doctor Search Empty State", "Reminder Creation Suite".

| Screen | Screenshot | State | Differences from the design |
|---|---|---|---|
| Notifications, empty | `30-notifications-empty` | Rebuilt to the design | The mint wave behind the logo is a plain gradient |
| Find a Doctor, empty | `31-find-doctor-empty` | Rebuilt to the design | No bell or avatar in the header and no bottom navigation (this screen is opened from several places without the shell). Filter glyphs are Material icons |
| Reminders list | `32-reminders-empty`, `34-reminders-with-item` | Working | Derived from panel 5; a card shows the patient's own title, note, next time and repeat, with delete |
| Create reminder | `33-create-reminder-art`, `33b-create-reminder-lower` | Working, built to the design | One form serves all six categories; picking a category changes heading, description, icon, suggested title and note hint. A "Weekly" repeat option is added. Category glyphs are Material icons. The illustration is the notifications artwork, which includes a bell |
| Ask a question | `35-ask-question` | Topic list changed | Topics are now HIV Testing, PrEP, ART, Sexual Health, Stigma & Mental Health, Other. The heading and intro text are still general ("Ask a health question") |

### How reminders work

- **Local only.** Reminders are saved on the device in encrypted storage, one list per account, and are never sent to the backend. There is no backend for them.
- **Alerts** are Android local notifications at the device's local time. Timing is inexact: Android may deliver a few minutes late. Exact alarms need an extra permission that is not requested.
- **Repeat:** once, daily, weekly, monthly.
- **Lock screen:** the notification is marked private, so Android hides its text on a locked screen and shows it after unlock.
- **Sign-out:** alerts are cancelled when the session ends and re-armed when the same account signs in again. The saved list survives sign-out.
- **Notifications off:** the reminder is still saved and the list shows a notice that it cannot alert.
- **Manifest:** `RECEIVE_BOOT_COMPLETED` and the two `flutter_local_notifications` receivers were added so alerts fire and survive a reboot.
- **Checked on the emulator:** a reminder was created through the form, appeared in the list after reopening the screen, and `dumpsys alarm` showed its alarm registered for 08:00 local time. The notification actually appearing at its time was not observed.

Not built: edit, pause, completion tracking, cross-device sync. A reminder is lost if the app's data is cleared or the app is reinstalled.

`CLAUDE.md` still lists Reminders among panels without a backend that must show an honest empty state; that line now needs the product owner's wording for a local-only feature.

## 2C. Sixth pass (2026-11-13): offline Home, upcoming card, consultation screens

Designs supplied: "Offline Services Screen", "Upcoming Consultation", "Consultation Confirmed", "Video Consultation Waiting Room", "Consultation Complete".

| Screen | Screenshot | State | Differences from the design |
|---|---|---|---|
| Home, offline | `40-home-offline`, `40b-home-offline-lower` | Built and seen on the emulator in airplane mode | Reminders and Health Records keep their own icon colours instead of turning green. Content scrolls on this phone |
| Home, upcoming consultation card | not captured (no appointment in the local database) | Built | Shows start time only, not a time range. The "Starts in N min" pill appears within an hour of the start. The button opens the waiting room, where joining stays gated; it does not start a call |
| Consultation confirmed | not captured | Built on the existing status page | No "Reason" row (the app does not hold it on this screen). The join note does not promise "10 minutes": the window is set by the server. No bell, avatar or bottom navigation. The existing status text, payment, refresh and "Attach health records" controls remain below the card |
| Waiting room | not captured | Built on the existing waiting room | "Join Video Call" stays, and stays disabled until the server's window opens; the design's "you will be automatically connected" is replaced by wording that matches that. Camera and microphone tiles show the real permission state after a tap; the old separate "test camera and mic" button is removed. "Leave Waiting Room" goes back |
| Consultation complete | not captured | Rebuilt | The design's "Consultation Summary" and "Follow-up Plan" cards are not shown: the app receives only the recorded outcome, so one "Consultation outcome" card shows that. No qualifications or years-of-experience line. "View Health Records" replaces "View Consultation Details" |

Why three consultation screens have no emulator screenshot: the local database has no doctor, slot or appointment, and seeding one means writing to the database. They are covered by tests instead:

- The existing booking, join-window and consultation tests render the status page and waiting room at 320 to 412 dp and still pass unchanged in behaviour.
- `test/post_consultation_layout_test.dart` renders the completion screen at 320 × 568, 411 × 923 and 800 × 1280, at text scale 1.0 and 1.3, and checks that no invented summary or plan appears.

Behaviour notes:

- **Offline** is driven by the device's connectivity signal, so Home changes as soon as the connection drops and reloads when it returns. Tapping a greyed module shows "This service needs an internet connection."
- **15-minute notification** is the backend's existing `CONSULTATION_REMINDER_15M`; nothing was added for it.
- **Upcoming card on Home** reverses part of the 2026-11-12 Home change; `CLAUDE.md` business rule 2.4 is rewritten to match. The Emergency Contact banner stays off Home.
- Two tests that host the status page gained localization delegates, because the page now uses localized strings for its new parts. Its existing English status strings are unchanged.

## 3. Home changes of 2026-11-12

- Emergency Contact banner, family-profile switcher and Upcoming Consultation card removed from Home.
- Upcoming consultations: the same card, with its Rejoin strip and waiting-room entry, now renders on the Book Consultation landing when the patient has one. Family profiles remain on the Profile tab.
- `CLAUDE.md` business rule 2.4 rewritten to match.
- `emergency_contact_banner.dart` deleted; the emergency number constant is still used by the consultation landing.

One consequence to be aware of: an active consultation's "Rejoin" is now two taps from Home (Book Consultation, then the card) instead of on Home itself. Notifications still deep-link to the appointment.

## 4. Code changes

All under `apps/mobile_patient`. No repository, cubit, model, network or backend file was edited.

| File | Change |
|---|---|
| `lib/core/theme/tokens.dart` | Reference palette; primary, background and text colours updated |
| `lib/core/widgets/afya_widgets.dart` (new) | Logo, initials avatar, icon tile, service tile, back header |
| `lib/core/widgets/afya_bottom_nav.dart` (new) | `PatientBottomNav`, restyled; still exported from `app.dart` |
| `lib/core/widgets/patient_widgets.dart` | `PatientPageHeader` shows a back arrow on pushed routes |
| `lib/app.dart` | Tabs are Home, Services, Messages, Profile. Messages is built on first visit |
| `lib/features/patient/home/patient_home_page.dart` | Presentation rebuilt |
| `lib/features/patient/home/upcoming_consultation_section.dart` (new) | The consultation card moved out of Home unchanged |
| `lib/features/patient/home/emergency_contact_banner.dart` | Deleted |
| `lib/features/patient/services/patient_services_page.dart` | Nine-module directory |
| `lib/features/patient/modules/` (new) | Module list and routing, shared module widgets, eight module screens, More, unavailable state |
| `lib/features/patient/chat/patient_chat_list_page.dart` | `build` redesigned and localized; paging and Active/Past logic unchanged |
| `lib/features/patient/profile/patient_profile_page.dart` | Presentation redesigned |
| `lib/features/patient/blogs/blogs_page.dart` | Optional `initialQuery` |
| `lib/features/patient/vault/health_vault_page.dart` | Optional `initialKind` |
| `lib/l10n/app_en.arb`, `app_sw.arb` | About 140 new strings each. The Swahili is machine-quality and needs a native speaker's review |
| `pubspec.yaml` | Asset folders registered. No new package |
| `assets/brand/`, `assets/illustrations/` (new) | Logo and ten illustrations, about 3.2 MB |
| `test/patient_navigation_safe_area_test.dart` | Tab labels updated |
| `test/patient_chat_list_deferred_fee_test.dart` | Localization delegates added, because the page is now localized |
| `CLAUDE.md` | Imagery rule and Home rule updated to the product owner's decisions |

## 5. Not done

- Redesign of auth, booking, waiting room, call, chat thread, consultation status, article reader, notification screens and the profile sheets.
- Facilities map artwork.
- Design-preview fixtures for the sample reminders and facilities.
- Screenshots of Privacy & Security, More, the populated Messages card, a 360 dp device, Swahili and text scale 1.3 for the new screens.
- Widget tests for the new screens.
