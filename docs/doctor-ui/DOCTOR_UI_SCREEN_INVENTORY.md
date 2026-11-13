# Doctor UI — Screen Inventory

Date: 2026-11-13. Scope: `apps/mobile` (Doctor Flutter, package `hellodoctor_mobile`, applicationId `com.example.hellodoctor_mobile`). Discovery only; no application code was changed.

Every row was traced from the widget's `build`, its navigation calls and the repository method it invokes, not from file names. "Ref panel" is the panel number on the two reference boards in `docs/design/` (`afyamkononi-doctor-ui-reference-11-panel.png`, panels 1–11; `afyamkononi-doctor-ui-reference-14-panel.png`, which repeats 1–11 and adds 12–14).

Totals: **35 distinct screens, sheets and full-page states** (34 reachable, 1 unreachable), built from **79 widget classes** in **55 Dart files** (15,823 lines; `features/doctor/doctor_pages.dart` alone is 4,955 lines and holds 22 of the screens).

---

## 1. Navigation structure as built

- No `go_router` routes are used, although the package is a dependency. Navigation is imperative: `Navigator.push(MaterialPageRoute(...))`, modal bottom sheets and dialogs.
- `HelloDoctorApp` (`lib/app.dart`) switches the whole `MaterialApp` on `AuthState`: `SessionRestoring` → splash; signed out → `LoginPage`; `SignedIn` → `DoctorShell` inside `NotificationNavigationHost`.
- `DoctorShell` first checks `profile.verified`. If the profile is loaded and not `VERIFIED`, it renders the verification gate instead of the tabs.
- Bottom navigation (`DoctorBottomNav`), **six tabs**: Queue · Chat · Scan Rx · Schedule · Wallet · Settings.
- Pages are not kept alive: `pages[tab]` rebuilds the tab on every switch.

The reference boards show **four tabs**: Home · Appointments · Patients · More. Section 4 maps one to the other.

---

## 2. Signed-out screens

| # | Screen | Source | What it does today | Backend | Ref panel |
|---|---|---|---|---|---|
| 1 | Splash | `app.dart` `_SplashPage` | Icon tile, "AfyaMkononi", "Restoring secure Doctor session…", spinner. Shown only while `AuthCubit.restore()` runs | `POST /auth/refresh` | none (spec §5 "Splash / startup") |
| 2 | Startup error | `core/widgets/startup_error_page.dart` | Shown when service wiring throws (e.g. bad `API_BASE_URL`); "Try again" | none | none |
| 3 | Login — password step | `app.dart` `LoginPage` | Icon, "Physician Secure Login", "Licensed KMPDC practitioners gateway.", email, password, "Forgot password?", "Continue to MFA", "Register as Doctor" | `POST /auth/doctor/login` | 1 |
| 4 | Login — code step | same widget, `MfaRequired` state | "One-time verification code", "Verify and sign in". Skipped automatically in development when the backend returns `debug_otp_code` and `DEV_AUTH_OTP_ENABLED` is set | `POST /auth/doctor/verify-otp` | 1 (the "2FA" note) |
| 5 | Forgot password | `features/auth/forgot_password_page.dart` | Email field, "Send verification code", "Back to sign in" | `POST /auth/password-reset/request` | none |
| 6 | Reset code | `features/auth/reset_otp_verify_page.dart` | 6-digit code | `POST /auth/password-reset/verify-otp` | none |
| 7 | New password | `features/auth/create_new_password_page.dart` | New password with strength rule | `POST /auth/password-reset/complete` | none |
| 8 | Reset success | `features/auth/password_reset_success_page.dart` | Confirmation, back to login | none | none |
| 9 | Doctor registration | `app.dart` `RegistrationPage` | One long form, ten fields: full name, KMPDC registration number, phone, email, password, primary specialty, experience years, current hospital, qualifications, residential address. Client checks password strength and Kenyan phone format, then "Continue" | none yet | 12 (shown as two steps) |
| 10 | Legal and professional agreements | `features/auth/legal_consent_page.dart` | Loads the current legal documents, requires acceptance of Doctor Terms, Privacy Notice, Professional Verification Declaration, Telemedicine Practitioner Policy; Marketing is optional. "Submit application" | `GET /legal/documents`, `POST /auth/doctor/register` | 12 (not drawn) |
| 11 | Legal document reader | route pushed from #10 | Full text of one document | none | none |
| 12 | Registration submitted | state inside #9 and #10 (`RegistrationSubmitted`) | Icon, "Registration submitted", "Your Doctor account is pending ClinicalAdmin verification.", "Return to login" | none | 12 ("Account Under Review") |

## 3. Signed-in screens

| # | Screen | Source | What it does today | Backend | Ref panel |
|---|---|---|---|---|---|
| 13 | Verification gate | `app.dart` `_ShellState.build` | Plain text: name, KMPDC number, "Status: Pending review / Rejected / Verification unavailable", the admin's review note if any, "Refresh verification status", "Log out". No tabs | `GET /doctor/profile` | 12 (third phone) |
| 14 | Queue (tab 1) | `doctor_pages.dart` `QueuePage` | Profile bar with **Online/Break duty toggle**, write-article and bell actions; Q&A banner with unanswered count; "Current and remaining today"; search; "Consultation History" button; one `AppointmentCard` per actionable appointment. Auto-refresh every 30 s | `GET /doctor/appointments`, `GET/PUT /doctor/duty`, `GET /doctor/qna/questions` | 2 and 3 |
| 15 | Appointment card actions | `AppointmentCard`, `ConsultationActions` | Patient initials, modality, time chip, chief complaint; "Start chat/video consultation" (disabled with "Room opens at …" outside the server join window); "Rejoin" banner for an active video session; "Follow-up messages"; "Write prescription" | `GET /appointments/{id}/conversation` | 3, 4 |
| 16 | Consultation history | `ConsultationHistoryPage` | Paged list of past encounters (100 per page) | `GET /doctor/appointments` (history query) | 3 ("Completed" tab) |
| 17 | Encounter detail | `EncounterHistoryPage` | Patient name, modality, status, presenting concern and intake notes, "View chat transcript", clinical documents and prescriptions | `GET /doctor/consultations/{id}/summary` | 4, 9 |
| 18 | Chat list (tab 2) | `ChatListPage` | "Consultation Chats", search, All/Active/Closed filters, conversation cards, paging | `GET /doctor/chats` | none (panel 6 shows chat inside the call) |
| 19 | Chat thread | `ChatThreadPage` | Message thread, expiry bar, clinical image attach (JPEG/PNG, 10 MB), retry, read-only after close. Top bar: patient history, start video call (video appointments only), "Snap & Send Rx" | `GET/POST /chat/{id}/messages`, `POST /chat/{id}/attachments`, `POST /appointments/{id}/trace-ack` | 6 (chat tab) |
| 20 | Scan Rx (tab 3) | `ScannerPage` | "Smart Rx Scanner & Dispatch": pick the consultation, capture pages by camera, gallery or file (image or PDF), up to 5 pages, optional advice text, per-page upload progress and retry, "Dispatch Prescription to Patient" | `POST /consultations/{id}/prescription` | 14 (different concept, see §5) |
| 21 | Schedule (tab 4) | `SchedulePage` | "Consultation Schedule": today summary banner with "Create 10-Min Slots" (six consecutive slots from a chosen start), 7-day date strip, filters All/Blocked/Booked/Available, slot cards with Cancel, Block, Unblock; booked slots show the patient and the consultation actions | `GET/POST /doctor/slots`, `PATCH/DELETE /doctor/slots/{id}` | 13 |
| 22 | Wallet (tab 5) | `WalletPage` | Earnings breakdown with the 20% platform charge, monthly statement, payout method, ledger. Replaced by `DeferredWalletNotice` ("Financial module not yet active") when the server reports payment not required | `GET /doctor/wallet/*`, `GET /config/booking-policy` | none |
| 23 | Payout setup sheet | `_PayoutBindingSheet` | M-Pesa number entry | `POST /doctor/wallet/payout-methods/bind` | none |
| 24 | Settings (tab 6) | `features/settings/settings_pages.dart` `SettingsHomePage` | Tiles: Profile, Availability (jumps to Schedule tab), Payout (jumps to Wallet tab), Notifications, Security | none | 10 |
| 25 | Profile | `doctor_pages.dart` `ProfilePage` | "My Articles" tile; photo avatar (upload/remove); editable full name; read-only regulator, country, KMPDC number, specialty, hospital, qualifications, verification; "Save profile"; logout icon | `GET/PATCH /doctor/profile`, `/doctor/profile/photo` | 10 |
| 26 | Security | `SecuritySettingsPage` | "Current session" text and "Log out". No password change, no 2FA management | `POST /auth/logout` | 10 ("Security" row) |
| 27 | Notification settings | `features/settings/notification_settings_page.dart` | Language (English/Swahili), category switches (greetings, religious, routine, announcements), quiet hours | `GET/PUT /notifications/preferences` | 10 ("Consultation Settings" is a different thing) |
| 28 | Notification centre | `features/notifications/notification_center_page.dart` | List, mark all read, load older, empty and error states. No category filter | `GET /notifications`, `POST /notifications/{id}/read`, `/read-all` | 11 |
| 29 | Appointment from a notification | `features/notifications/notification_appointment_page.dart` | Appointment reference and status after an ownership-checked read | `GET /appointments/{id}` | none |
| 30 | Video call | `features/doctor/agora_page.dart` | Pre-join gate ("Checking the consultation room…", "Opens in mm:ss", window closed), permission request, connecting, in-call (remote video, local preview, mute, camera on/off, switch camera, end), reconnecting, failure with retry | `GET /consultations/{id}/join-eligibility`, `POST /consultations/{id}/rtc-token`, `/telemetry/events` | 5 and 6 |
| 31 | Patient history sheet | `HistoryPage` | List of prior episodes the server authorises for this appointment; tapping one opens a dialog that prints the **raw JSON** of the episode plus document buttons | `GET /doctor/appointments/{id}/patient-history[/{episode}]` | 4 (History tab), 9 |
| 32 | Clinical document viewers | `clinical_image_viewer.dart`, `clinical_pdf_viewer.dart`, `DocumentButton` | In-app image and PDF viewing after a mime-type cross-check; never an external app | `GET /appointments/{id}/documents/{doc}` | 4 (Files), 9 (Files) |
| 33 | Patient Q&A | `QnaPage` (modal sheet) | Open and answered anonymous questions, preset replies, "Publish Verified Doctor Answer" | `GET /doctor/qna/questions`, `POST …/answers` | none |
| 34 | My Articles and editor | `features/doctor/articles_page.dart` | List, create, edit, publish, archive | `/doctor/articles` | none |
| 35 | Consultation detail | `doctor_pages.dart` `ConsultationPage` | Summary, patient history, join, prescription, and the **completion dialog** (Complete with Rx / without Rx / Refer / Escalate) | `POST /consultations/{id}/complete`, `/complete-no-rx` | 7 |

**Screen 35 is unreachable.** Nothing in `lib/` constructs `ConsultationPage`; only `test/consultation_completion_test.dart` does. The Doctor mobile app therefore has no way to record a consultation outcome today. Doctor Web's request allow-list (`apps/doctor-web/src/lib/contracts.ts`) does include the completion endpoints. Whether this was deliberate has not been established; it is the most important functional finding of this inventory.

---

## 4. Reference navigation against the current tabs

| Reference tab | Reference content | Current equivalent | Gap |
|---|---|---|---|
| Home | Greeting, four counters (Today, Upcoming, In Progress, Completed), three quick actions | Queue tab top section | No dashboard exists. Counters can be derived from `GET /doctor/appointments` (queue plus history); no dedicated counts endpoint |
| Appointments | Today / Upcoming / Completed tabs, cards with avatar and status pill | Queue list plus Consultation History | Queue shows only actionable appointments; there is no "Upcoming" grouping |
| Patients | Searchable patient list with ID, sex, age, last visit; detail with Summary / Visits / Results / Files | None | **No backend support.** There is no doctor-facing patient list. History is reachable only through an appointment the doctor holds |
| More | Profile, availability, settings, security, notifications | Settings tab | Chat, Scan Rx, Wallet, Q&A and Articles have no place in the reference navigation and need one |

Features the app has that the reference boards do not show: chat consultations and the chat list, duty toggle (Online/Break), patient Q&A, articles, wallet and payout, notification preferences, forgot-password flow, legal consent. They must survive the redesign.

## 5. Reference panels with no working equivalent

| Panel | Shows | Reality |
|---|---|---|
| 1 | "Two-factor authentication (2FA)" note | Real: sign-in is password plus an emailed code |
| 4 | Patient ID "P-03456", sex, age, "Key Information: Known HIV positive, On ART" | The appointment payload carries a display name and chief complaint only. No patient number, sex, age or structured clinical flags are exposed to the doctor |
| 5 | "Patient is waiting", "Joined at 2:58 PM" | The server returns `patient_present` / `doctor_present` only for an active session. There is no doctor-side waiting-room screen and no join timestamp in the client model |
| 6 | Chat / Notes / Checklist tabs inside the call, screen-share button | The call screen has video and four controls only. No in-call chat, notes or checklist. No screen share |
| 7 | Clinical notes, diagnosis, management plan, follow-up | **No backend fields.** `clinical_notes` in the schema is the patient's intake text (`appointment_clinical_intake`). The doctor can record only an outcome code and, for no-Rx, a reason |
| 8, 9 | Patient list; HIV status, regimen, viral load, next review; "Update Record" | No tables or endpoints |
| 10 | "Verified Doctor" badge, "Consultation Settings", "Security: password and two-factor" | Badge is real (verification status). Consultation settings do not exist. Security has logout only |
| 11 | Filter chips All / Appointments / Messages / System; "System update: new clinical guideline" | Notifications are real; there is no filter and no guideline feed |
| 13 | Weekday chips, working hours, slot duration, "Accept video consultations" switch | Slots are created individually for specific dates. No weekly template, no stored slot duration, no modality switch |
| 14 | Structured e-prescription: medicine, dose, frequency, duration, instructions | Prescriptions are **photographed or uploaded pages** (max 5, 40 KB each after server compression) with optional advice text. No structured medicine model |

These are design targets, not existing behaviour. Each needs a product decision before it is drawn as live (see the redesign plan).
