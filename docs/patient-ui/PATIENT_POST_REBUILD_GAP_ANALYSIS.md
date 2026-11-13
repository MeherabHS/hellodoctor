# Patient App — Post-Rebuild Gap Analysis

Date: 2026-11-13. Diagnostic only: no product code, backend, migration, configuration or database row was changed. One synthetic patient account was created through the app's normal registration (see A.5).

Evidence labels used throughout:

- **Verified (runtime)** — observed on the emulator or in backend metrics/database during this session.
- **Verified (code)** — read in source with file:line; not exercised at runtime.
- **Verified (tests)** — covered by a passing automated test.
- **Suspected** — a hypothesis supported by code but not confirmed. Never presented as a root cause.
- **Not tested** — not exercised this session; reason stated.

Screenshots: `docs/patient-ui/screenshots/gap-analysis/pixel_9_api37/` (file names are cited as `[NN-name]`).

---

## A. Build and test baseline

### A.1 Source revision

| Item | Value |
|---|---|
| Branch | `feature/patient-ui-replication` |
| HEAD | `3c09124423ef21ba21856f4f1ece9aeb35e4d5d6` ("test(doctor-web): prove the RTC token exemption…") |
| Staged changes | none |
| Uncommitted | 51 tracked files modified (3,966 insertions, 2,520 deletions) across Patient app, Doctor app, `CLAUDE.md`, `checkpoint.md`, `progress.md` |
| Untracked | 34 paths, including **all new Patient UI code** (`modules/`, `reminders/`, `afya_widgets.dart`, `afya_bottom_nav.dart`, `auth_scene.dart`, `consultation_scene.dart`, `upcoming_consultation_section.dart`), all new assets, three new test files, **the design references themselves** (`docs/design/`), and all of `docs/patient-ui/` and `docs/doctor-ui/` |
| Other branches | None contains the redesign. `git branch -a` shows only earlier fix/feature branches; the redesign exists only in this working tree |

Every test and screenshot in this report was produced from this working tree (HEAD `3c09124` + the uncommitted/untracked changes above).

### A.2 Toolchain

| Item | Value |
|---|---|
| Flutter | 3.47.6 stable (framework 5fc346839b, engine 692136cb65) |
| Dart | 3.13.5 |
| Package | `apps/mobile_patient`, applicationId `com.hellodoctor.patient` |
| Dependencies | `flutter pub get` OK; 64 packages have newer versions outside constraints (not upgraded) |

### A.3 Results

| Step | Result |
|---|---|
| `flutter pub get` | exit 0 |
| `flutter analyze` | **No issues found** (13.2 s) |
| `flutter test` (52 test files) | **462 / 462 passed**; no overflow or exception lines in the test log |
| `flutter run` (debug) | Built `app-debug.apk`, installed and launched. No release or signed build was produced |
| Backend | `cargo run --bin hellodoctor-backend` (v2.4.0), local, `ENVIRONMENT=development`, Docker Postgres 16 and Redis healthy. Not re-tested with `cargo test` this session |

Run defines: `ENVIRONMENT=development`, `ALLOW_INSECURE_HTTP=true`, `DEV_AUTH_OTP_ENABLED=true`, `API_BASE_URL=http://10.0.2.2:8080/api/v1`.

### A.4 Emulator

| Item | Value |
|---|---|
| AVD | `Pixel_9`, Android 17 (API 37), 1080 × 2424 px, 420 dpi (≈ 411 × 923 dp) |
| Extra configurations exercised | text scale 1.3; 360 dp width (via `wm size 1080x2400`, `wm density 480`); Swahili. All restored to 1080 × 2424, 420 dpi, scale 1.0 afterwards |
| Device time zone | **Asia/Dhaka (UTC+6)**, not EAT. Useful for EAT-display checks |
| Clock skew vs host | −1.1 s at time of test |

### A.5 Data and environment touched

- Local database only (`hellodoctor-postgres` container). Read-only counts: 2 pre-existing patient users, 1 admin, **0 doctors, 0 slots, 0 appointments, 0 questions, 0 notifications**, 1 admin-authored article with no quiz.
- Created through the app's public registration: one synthetic patient, `gap.audit.1113@example.test`, name "Gap Audit Tester", phone `0712990417`. Three sign-in sessions and two device registrations exist for it. Disposable; delete when convenient.
- One password-reset request for `gap.audit@example.test` (non-existent account) returned 404.
- **Fixture doctor (approved by the product owner, 2026-11-13; see section L):** `fixture.doctor@example.test`, "Dr Fixture Test (synthetic)", KMPDC `TEST-FIX-0001`, registered through `POST /auth/doctor/register`. Two direct database updates were needed: verification status PENDING → VERIFIED (the admin password is not available, so the Admin Web approval screen was **not** exercised) and fees (no API can set them, see D-10). Three 10-minute slots created through `POST /doctor/slots`; two appointments booked through the Patient app: `APT-20261113-2960ECF6` (video, now COMPLETED_NO_RX) and `APT-20261113-6B723306` (chat, CONFIRMED).
- **Local backend restarted** with the process environment variable `FINANCIAL_MODULE_MODE=deferred` (the planned production setting) so bookings need no payment. No file or production setting was changed; it was still running in that mode at the end of the session.

---

## B. Feature inventory

| Feature | Status | Evidence |
|---|---|---|
| App start, debug config guard | Verified working | Launched; startup guard per `api_client.dart` (prior docs) |
| Log in (email + password + email code) | **Broken (intermittent)** | 1 of 3 sign-ins reached Home; 2 returned to Log in. See D-01 |
| Registration | Verified working (server side) | Account created, session created; then the same sign-out as D-01 `[05]`–`[08]` |
| Email code page + legal consents | Verified working | `[06]`, `[07]` |
| Session restore on relaunch | Verified working | Relaunch → one `/auth/refresh` 200 → Home |
| Forgot password | **Broken in every environment except development with a flag** | D-02 `[03]` |
| Home dashboard, offline layout | Verified working (online); offline not re-tested | `[11-signin-t8s]` |
| Home upcoming-consultation card | Partial | Not rendered (no appointment data). Logic gaps in D-03 |
| Bottom navigation (4 tabs) | Verified working | `[30]`–`[32]` |
| Services directory | Verified working | `[30-tab-services]` |
| HIV Prevention & Sexual Health | Verified working (layout); content Not tested | `[20]`; no published article with topic content locally |
| HIV Support Chat (anonymous Q&A entry) | UI verified; submission Not tested | `[21]` |
| Book Consultation | Partial | Opens video-only landing `[22]`. D-06 |
| Doctor directory, profile, slot selection (EAT) | Verified working | `[61]`–`[64]`; small copy defects D-12 |
| Video booking (deferred payment) → confirmation | Verified working | `[65]` |
| Waiting room, camera/mic permissions, join gating | Verified working | `[66]`–`[68]` |
| RTC token and Agora join (patient) | Partial | Token minted 200; `joinChannel` result 0; local preview `[69]`. Join-success callback not observed; no doctor media (one device) |
| Doctor and patient on the same channel | Verified (server) | Doctor's token returned the same channel name as the patient's |
| Two-party audio/video, reconnection, audio-only fallback | Not tested | Needs a second client with camera |
| Chat consultation | **Broken in deferred mode** | No conversation is ever created. D-09 `[74]` |
| Consultation completion → patient | **Partial** | Server records COMPLETED_NO_RX; the redesigned completion screen is unreachable afterwards; notification opens a placeholder `[81]`. D-11 |
| Past consultation history | **Missing** | Completed video consultation appears nowhere (Messages Past, Health Records) `[78]`, `[82]` |
| Backend notifications (booking, T-15, ready, completed) | Verified working (in-app) | `[80]`; PHI-free text |
| Peer Navigator Support | UI only | Action disabled with honest note `[23]`; no backend (section F) |
| Reminders | Partial | Layout verified `[24]`; local delivery Not tested this session (previous session saw the alarm registered, not delivered) |
| Health Records | UI verified; populated Not tested | `[25]` |
| Videos & Quizzes | Partial | Opens Videos tab "Not available yet" `[26]`; D-07 |
| Facilities & Services | UI only (honest empty state) | `[27]` |
| More | Verified working | `[28]` |
| Messages | UI verified (empty) | `[31]`; no preview/unread data from API (E.3) |
| Profile | Verified working | `[32]` |
| Notification inbox | Verified working (empty) | `[44-notifications-from-bell]` |
| Notification settings | **Broken** | D-04 `[40]` |
| Privacy & Security → Data rights | Verified working (old layout) | `[41]` |
| Language switch EN ↔ SW | Verified working | `[47]`–`[49]`, `[46]` |
| Family profiles | UI verified; add dependent Not tested | `[32]` |
| Push notifications | Not tested | Backend FCM transport is a no-op locally (log at startup) |
| Local notifications (reminders) | Not tested this session | — |

---

## C. Visual fidelity gaps

Severity: all P2 unless stated. "Ref" is the reference panel or the product owner's supplied design.

| ID | Screen | Ref | Screenshot | Observed | Expected |
|---|---|---|---|---|---|
| V-01 | Create account | Healthcare Registration design | `[05-register-dob]` | Header artwork and logo draw under the status bar; the clock overlaps the tree and logo | Content below the status bar inset, as on Log in |
| V-02 | Forgot password | Forgot Password design | `[03-forgot-submitted]` | With the keyboard open, the error line shows but the primary button is hidden behind the keyboard | Button scrolls into view above the keyboard |
| V-03 | Log in, Forgot password | Welcome Back / Forgot Password designs | `[01]`, `[02]` | Lower ~25% of the screen is empty blurred scenery | Matches design or intentionally fills |
| V-04 | Profile (text scale 1.3) | none | `[51-profile-sw-ts13]` | Family chips overflow by 28 px and 6 px (debug stripes); `family_profile_chip_row.dart:126`. Same defect was recorded in the 2026-11-12 baseline and is still present. **P1** (accessibility) | No overflow at 1.3 |
| V-05 | Profile (Swahili) | none | `[46-profile-sw]` | Chip label breaks mid-word: "Gap (Mwen / yewe)" | Whole-word wrap or ellipsis |
| V-06 | Home (Swahili, 360 dp and 1.3) | Panel 1 | `[50]`, `[52]`, `[53]` | Tile labels truncated: "Kinga ya VVU na Afy…", "Msaada wa Mwenzi Ele…", "Weka Miadi ya …" | Full labels (two lines) |
| V-07 | Module screens | Panels 2–8 | `[20]`–`[27]` | Opened from Home, the bottom bar highlights **Services**, not Home | Highlight matches origin tab, or a consistent rule |
| V-08 | More | Panel 1 "More" | `[28-mod-more]` | No bottom navigation, unlike every other module | Consistent chrome |
| V-09 | Facilities | Panel 7 | `[27]` | "Youth Friendly" chip clipped at the right edge; map strip is a dimmed crop of the reference | Known; awaiting art |
| V-10 | Peer Navigator | Panel 4 | `[23]` | Lists "Appointment reminders", "Follow-up support", "Linkage to services", "Emotional support" as offered while no peer feature exists | Clearly marked as not yet available (CLAUDE.md rule 2, no-backend panels) |
| V-11 | Email code, Data rights, Notification settings | none | `[06]`, `[41]`, `[40]` | Still the old visual style; Data rights shows raw enum "ACCESS" | Redesigned per plan; human-readable labels |
| V-12 | Not captured | Panels 8 and supplied consultation designs | — | Upcoming card, consultation confirmed, waiting room, call, completion never rendered with real data on a device | Needs fixture data (section K) |

Not redesigned (carried from the replication status, still true): email code, reset code, new password, reset success, profile completion, doctor profile, booking page, chat thread, call chrome, article reader and quiz, notification settings, profile sheets, data rights.

---

## D. Functional gaps

### D-01 Intermittent sign-out immediately after a successful sign-in — P0

- **Reproduction (this session, fresh build, fresh account):** register → enter email code → Verify → app shows Log in `[08-home]` (expected Home). Sign in again with the same account → Log in again `[10-after-login]`. Third attempt reached Home `[11-signin-t8s]`. 2 of 3 failed.
- **Evidence (Verified, runtime):**
  - Backend metrics after the two failures: `GET /patients/me`, `/patients/me/profiles`, `/patients/me/photo`, `/notifications`, `/notifications/unread-count` each returned **401** once; the same endpoints returned 200 in another batch.
  - **No `POST /auth/refresh` reached the server** during either failure (the counter first appears on the later relaunch, status 200).
  - `auth_sessions` rows for the account were created at 05:06:24 and 05:07:40 with `rotated_at` and `revoked_at` both null: the server did not end the session.
  - Third, successful sign-in: all protected requests 200, `POST /notifications/devices` 201.
- **Root cause: not verified.** What the evidence shows: protected requests received 401 and the client ended the session locally without attempting a refresh over the network.
- **Suspected mechanisms, in order:**
  1. The client's refresh aborts before sending: `_rotate()` throws `ApiException(401)` when `read('refresh_token')` returns null, and that path calls `expireSession()` (`api_client.dart:452-453`, `:473-475`). That would mean the refresh token was not yet readable when the first 401 arrived — e.g. a race between token writes after `verify-otp` and the burst of concurrent first requests (`app.dart:263-303` starts profile, family, notifications and push registration together).
  2. Why the first requests got 401 at all is unknown. The server returns 401 for a missing or invalid `X-Device-Proof` (`device_proof.rs:90-131`; proof `iat` must be within −60/+5 s), for an invalid access token, or for a revoked family. Clock skew was measured at −1.1 s, inside tolerance.
- **Affected code:** `apps/mobile_patient/lib/core/network/api_client.dart` (interceptor `onError` 315-362, `refreshSession` 437-447, `_rotate` 449-477), `lib/features/auth/auth_cubit.dart` (94-98, 170-197), `lib/app.dart` (263-303).
- **Next diagnostic step:** temporary debug-only logging of the 401 response body and of whether `refresh_token` is present at the time of the refresh decision (the checkpoint's suggested step), then repeat registration → verify.
- **Scope:** frontend first; backend only if the 401 reason is server-side.
- **Required tests:** widget/integration test that signs in with a delayed token store and a burst of concurrent 401 responses, asserting the session survives and exactly one refresh is sent.

### D-02 Password recovery cannot work outside development — P1

- **Reproduction:** Log in → Forgot password → enter any email → Reset password. Message: **"This record is no longer available."** `[03-forgot-submitted]`. Backend: `POST /auth/password-reset/request` → 404.
- **Root cause (Verified, code):** reset is disabled unless `PASSWORD_RESET_ENABLED` is set (`password_reset_service.rs:53-57` returns 404 "Password reset is not available."), and config validation **refuses** to enable it in any HTTPS environment, i.e. staging and production (`config.rs:1114-1128`), pending an email-delivery acceptance test. The client maps 404 to a generic "record no longer available" message.
- **Impact:** in production, a patient who forgets their password has no recovery path and sees a misleading message.
- **Proposed fix:** product decision on recovery (complete the email-delivery acceptance test, or hide the link and show a support route). Client: give 404 on this endpoint a specific, honest message.
- **Do not** change the backend guard in this phase.

### D-03 Home "Upcoming Consultation" card does not follow the server's join information — P1

- **Verified (code):**
  - The card appears for the first `status=upcoming` appointment regardless of how far away it is, including `PENDING_PAYMENT` (`patient_home_page.dart:129-133`, `home_cubit.dart:26-27`, `patient_handlers.rs:369`).
  - Its button always reads **"Join Consultation"** for video, even days ahead (`patient_home_page.dart:551-557`). It opens the waiting room, which does gate joining correctly, so this is not an authorization hole.
  - It ignores the server-computed `join` object that the same response carries (`patient_handlers.rs:472-482`; parsed into `AppointmentSummaryDto.join`, `home_models.dart:109`, unused by the card).
  - The "Starts in N min" badge uses the device clock without the server-time offset (`patient_home_page.dart:469`), unlike the waiting room (`join_eligibility.dart:53-59`). No timer, so the badge does not update while Home stays open.
  - Home is not reloaded when the app resumes; resume only refreshes the session (`app.dart:88-94`). The card can show a cancelled or completed appointment until a manual refresh or reconnect.
  - A 5-minute cache is shown first on load (`home_repository.dart:55-56`, `home_cubit.dart:84-90`).
- **Confirmed at runtime (section L):** right after booking, Home showed no card for a consultation starting in 5 minutes until pulled to refresh `[75]`/`[76]`; later it showed the chat consultation card "Starts in 32 min" `[77]`, i.e. earlier than the 15-minute rule.
- **Decision (product owner, 2026-11-13):** the card appears 15 minutes before the start and not earlier. `CLAUDE.md` rule 2.4 has been updated to say so; the current code contradicts it (shows the card for any upcoming appointment).
- **Proposed fix:** show the card only from `join.join_open_at` (server time via `server_time` offset) until completion, cancellation or `join_close_at`; add a periodic tick so it appears on time while Home stays open, and reload on resume.
- **Required tests:** widget tests at T−16, T−15, T, end+60, end+61 minutes with skewed device clock; resume reload; CANCELLED/COMPLETED removal.

### D-04 Notification settings never loads — P1

- **Reproduction:** Profile → Notification settings → "Unable to load notification settings." `[40-notification-settings]`. Same symptom as the 2026-11-12 baseline.
- **Evidence (runtime):** no `GET /notifications/preferences` request reached the server.
- **Root cause (Verified, code; no stack trace captured):** the page calls `context.read<NotificationCubit>()` (`notification_settings_page.dart:46`), but `NotificationCubit` is provided inside `_SignedInRoot` (`app.dart:332-338`), below the `MaterialApp` navigator. The page is pushed onto that navigator without re-providing the cubit (`patient_profile_page.dart:108-109`), so the lookup throws and `catch (_)` shows the generic error. Every other pushed route or sheet that needs a session cubit re-provides it with `BlocProvider.value` (`patient_modules.dart:98-100`, `add_dependent_sheet.dart:22`, `profile_edit_sheet.dart:17`, `patient_booking_page.dart:124`).
- **Fix:** pass the repository or re-provide the cubit when pushing; narrow the `catch`.
- **Tests:** widget test that opens the page from Profile inside the real `_SignedInRoot` tree.

### D-05 Refresh-token reuse detection can sign users out after a network drop — P1 (suspected)

- **Verified (code):** the server revokes the whole token family the moment an already-rotated refresh token is presented, with no grace window (`auth_service.rs:1061-1068`). The client rotates on **every** app resume (`app.dart:88-94`) as well as on 401. If a rotation reaches the server but the response is lost (flapping network), the client keeps the old token; the next refresh is "reuse" → family revoked → 401 → sign-out (`api_client.dart:466-471`).
- **Status:** suspected explanation for the previously reported airplane-mode sign-out. Not reproduced this session. The backend logs `SESSION_REFRESH_REUSE_DETECTED` when it happens, which would confirm it.
- **Do not** change token policy in this phase; options (short grace window for the immediately previous token, fewer resume rotations) need a security decision.

### D-06 Book Consultation tile only offers video — P1

`[22-mod-book-consult]`: the tile opens "Video Consultation" with "Book a Video Consultation". A chat consultation can only be booked from Messages → "Book a chat consultation" `[31]`. Chat booking is a documented core flow; it is hard to find from Home.

### D-07 Videos & Quizzes opens on an empty Videos tab — P2

`[26]`: tile opens HIV Prevention with the Videos tab active, "Not available yet". The backend serves quizzes per article (`router.rs:400-401`), and the Quizzes tab exists; whether it lists anything was not tested (no published quiz locally).

### D-09 Chat consultations cannot start when payments are deferred — P0

- **Reproduction:** with `FINANCIAL_MODULE_MODE=deferred`, book a chat consultation (Messages → Book a chat consultation → doctor → Book chat → slot → Confirm) → "Consultation Confirmed!" → Open consultation `[74]`.
- **Observed:** the status screen shows, at once, "Payment is confirmed. Your consultation is being prepared; please retry shortly", "Payment required — Complete payment to confirm your consultation" and "No payment required". From a confirmed video booking, "Open 24h Chat" shows "Chat opens once your appointment is confirmed." although it is confirmed. `GET /appointments/:id/conversation` → 404.
- **Evidence (Verified, runtime + code):** `chat_conversations` has **0 rows** for both confirmed appointments. The only code that creates a conversation is inside `process_payment_webhook` (`appointment_service.rs:703-725`, `chat_repo::insert_conversation`); the deferred booking path (`appointment_service.rs:284-417`) never calls it, and no migration or trigger inserts one.
- **Root cause (Verified, code):** conversation activation was tied to payment success and was not added to the deferred path.
- **Impact:** with the planned production setting, every chat consultation and the 24-hour chat after a video consultation are unusable.
- **Scope:** backend (activate the conversation for deferred bookings, under the same advisory lock); client copy for the 404 state.
- **Tests:** backend integration test "deferred chat booking → conversation exists → both parties can send"; Patient widget test for the status screen with `payment_policy=NOT_REQUIRED`.

### D-10 No way to set consultation fees — P1

`PATCH /doctor/profile` accepts only `full_name` (`doctor_handlers.rs:597-599`), and no backend code writes `consultation_fee_video` or `consultation_fee_chat`. A newly approved doctor has fees 0.00; the directory then hides them from the chat filter (`doctor_handlers.rs:50`). Fees could only be set here by a direct database update. Backend + Doctor/Admin UI work.

### D-11 Consultation completion and history are not reachable afterwards — P1

- The redesigned completion screen (`PostConsultationPage`) is opened only from the waiting room's poll (`waiting_room_page.dart:137`). If the patient is not on that screen when the doctor completes, they never see it.
- Tapping the "Consultation completed" notification opens `NotificationAppointmentPage`, which shows only the booking reference and "Completed" `[81]`.
- The completed video consultation does not appear in Messages → Past `[78]` or Health Records `[82]` (a no-prescription completion creates no record). There is no appointment history screen.

### D-12 Consultation copy defects — P2

| Where | Text | Problem |
|---|---|---|
| Waiting room `[66]` | "Doctor Calls You at Your Booked Time — Patients cannot place outgoing calls. When your booked slot arrives, join from here." | Contradicts the "Join Video Call" button on the same screen |
| Booking page `[64]` | "Doctor calls you directly at scheduled time" | Patient joins from the waiting room |
| Chat booking sheet `[72]` | "Paid clinical text consultation… Consultation Fee: KES 500" | Payments are deferred; the next step says KES 0 |
| Doctor profile `[62]` | "About Dr. Dr" | "Dr." prefixed to a name already starting with "Dr"; "Verified clinician" shown twice |
| Notification inbox `[80]` | "Nov 13 · 11:34 AM" | Device time zone (Dhaka), not EAT like the rest of the app |
| T-15 reminder `[80]` | "Your consultation starts in 15 minutes." | Sent 4 minutes before start (booking made inside the window) |

Also observed: the server accepted `complete-no-rx` although the doctor never joined the call (consultation session ACTIVE, doctor never present). Whether completion should require the doctor to have joined is a product question.

### D-08 Smaller defects — P2

| ID | Defect | Evidence |
|---|---|---|
| D-08a | An error from one auth screen persists onto the next ("Enter a valid email address." shown on Create account after a failed Log in). Shared `AuthCubit` state | `[04-register]`; runtime |
| D-08b | The four legal notices can be ticked without opening any, despite "Read each notice before creating your account" | `[07-otp-consent]`; product/compliance decision |
| D-08c | Android Back on a non-Home tab (Profile) exits the app instead of returning to Home | runtime, observed twice |
| D-08d | Home bell's accessibility label is "Notification settings" though it opens the inbox | UI dump |
| D-08e | T-15 reminder body always says "starts in 15 minutes", including bookings made with less notice | `notification_scheduler.rs:1352-1359` |
| D-08f | Out-of-date doc comment: `upcoming_consultation_section.dart:36-39` says the card moved off Home; it is back on Home | code |
| D-08g | 53 hardcoded English `Text('…')` literals in 19 files (auth, consultation status, chat, upcoming section "Active Consultation"/"Rejoin", PDF/image viewers) bypass Swahili | grep count; lower bound |

---

## E. Backend integration

### E.1 Patient API surface (Verified, code)

Every HTTP call in `apps/mobile_patient/lib` has a matching route in `services/backend/src/api/router.rs`. No client call points at a missing route. All routes below sit behind `auth_middleware` (`router.rs:157`) except `/auth/*`, `/legal/documents`.

| Area | Client (file:line) | Endpoint(s) | Authorization (server) | Runtime this session |
|---|---|---|---|---|
| Auth | `auth_repository.dart:18-139` | `/auth/patient/{login,register,verify-otp}`, `/auth/refresh`, `/auth/logout`, `/auth/password-reset/*` | device-proof binding; family lock and reuse detection (`auth_service.rs:1014-1088`) | login/register/verify 200; refresh 200 on restore; reset 404 |
| Doctor discovery | `directory_repository.dart:51,93` | `GET /doctors`, `GET /doctors/:id` | list filters `VERIFIED` (`doctor_handlers.rs:50`); **by-id does not** (`doctor_repo.rs:100-108`) | not exercised (no doctors) |
| Slots | `booking_repository.dart:42` | `GET /doctors/:id/slots` | AVAILABLE, future, verified doctor only (`doctor_repo.rs:301`) | — |
| Booking | `booking_repository.dart:78` | `POST /appointments/book` | verified-doctor check, patient advisory lock, slot row lock, `appt_patient_no_overlap` and `appt_doctor_no_overlap` exclusion constraints (`appointment_service.rs:73-117, 293-319`; migration `20261111000001`) | — |
| Booking policy | `booking_repository.dart:102` | `GET /config/booking-policy` | any authenticated | — |
| Join / RTC | `booking_repository.dart:246,267` | `GET …/join-eligibility`, `POST …/rtc-token` | participant check, one window policy (15 min before start to 60 min after end), VIDEO + CONFIRMED/IN_CONSULTATION only, NEEDS_REVIEW denial, one live reservation per user (`consultation_service.rs:143-260`, `join_window.rs`) | — |
| Chat | `chat_repository.dart:28-171` | `/appointments/:id/conversation`, `/chat/:id/messages`, attachments | RLS-scoped transaction + service checks | — |
| Records | `vault_repository.dart:45,87` | `/patients/me/documents`, `/appointments/:id/documents/:doc` | `authorized_document` + single-use grant (`prescription_handlers.rs:318-340`) | empty list |
| Q&A | `qna_repository.dart:26,53` | `/patients/me/qna/questions` | patient role | — |
| Articles/quizzes | `blogs_repository.dart`, `quiz_repository.dart` | `/articles`, `/articles/:id/quiz[/check]` | authenticated | — |
| Notifications | `notification_repository.dart:38-171` | inbox, read, devices, preferences | own-inbox only | inbox 200; devices 201; **preferences never called (D-04)** |
| Profile/family | `profile_repository.dart`, `family_profiles_repository.dart` | `/patients/me`, `/photo`, `/profiles[/:id]` | own; dependent ownership tested | 200 |
| Data rights | `data_rights_page.dart:33,49` | `/patients/me/data-requests` | own | page loads |

### E.2 Consultation rules (Verified, code)

- Verification filtering, overlap prevention, ownership and the join window are enforced server-side (above). The Patient waiting room uses the server window with clock-offset correction (`join_eligibility.dart`).
- **10-minute slots: not enforced anywhere.** Slot creation only checks end > start, future start and no overlap (`doctor_handlers.rs:202-228`; `chk_slot_time` in the initial migration). Any length is accepted.
- Financial deferment: deferred bookings confirm immediately with no transaction (`appointment_service.rs:284-338`). Inconsistency: the directory's CHAT filter requires a fee > 0 (`doctor_handlers.rs:50`), while deferred booking accepts fee 0 (`appointment_service.rs:334`).
- EAT display: the card formats with `kenyaTime`/`formatTimeEat` (`patient_home_page.dart:541-542`). Not observed at runtime (no appointment); the emulator's UTC+6 zone makes this a good later check.

### E.3 Missing or unverified backend capabilities

| Gap | Status |
|---|---|
| Chat list preview, last-message time, unread count | Missing: the patient chat list is built from `/patients/me/appointments`, which has no message fields (`chat_repository.dart:111`) |
| Facilities directory, videos, GHI dashboard, SMS/USSD/WhatsApp | Missing (honest empty states in the app) |
| Reminders sync | None by design (device-local) |
| Push delivery | Code present (`notification_transport.rs`, `push_notification_service.dart`); **not verifiable locally** — FCM transport is a no-op without `FCM_*` variables (backend startup warning) |
| Password reset in staging/production | Blocked by config (D-02) |
| Peer Navigator | Missing entirely (section F) |

---

## F. Peer Navigator integration

Verified independently this session (not copied from the earlier analysis): a case-insensitive search for `peer` in `services/backend/src` and `services/backend/migrations` returns **no matches**; `peer.?nav|PEER_NAVIGATOR` returns none in Admin Web, Doctor Web or `apps/mobile/lib`. The Doctor app's `features/peer/peer_pages.dart` is design-preview only (labelled sample data; its action shows "Not connected yet — Peer Navigator design preview.").

| # | Question | Answer |
|---|---|---|
| 1 | Separate Peer role in the backend | **No.** `user_role_enum` is PATIENT, DOCTOR, PLATFORM_ADMIN, CLINICAL_ADMIN, FINANCE_ADMIN, SUPPORT, COMPLIANCE, SECURITY_ADMIN (`20261001000001_initial_schema.up.sql:9-11`) plus ADMIN (`20261106000001_canonical_admin_enum.up.sql:2`) |
| 2 | Admin approval mandatory and enforced | **No** — nothing to approve |
| 3 | Unapproved peers excluded from discovery | N/A — no discovery endpoint |
| 4 | Patient can request support | **No** |
| 5 | Acceptance creates an authorized conversation | **No**; existing chat tables require an appointment (`chat_conversations.appointment_id NOT NULL UNIQUE`, per the earlier analysis) |
| 6 | Patient talks only to authorized peers | N/A |
| 7 | Audio/video independent of doctor booking | **No**; RTC tokens are minted only against an appointment and its join window |
| 8 | Peer notifications | **No** notification types exist |
| 9 | Separate Peer vs Doctor access control | **No** |
| 10 | Patient app handles unavailability honestly | **Mostly yes**: "Start Chat" disabled with "Peer Navigator chat is not available yet." `[23]`. Exception: V-10 lists peer services as if offered |

Cross-client work required (unchanged from `docs/doctor-ui/PEER_NAVIGATOR_ROLE_AND_API_GAP_ANALYSIS.md`): new role and peer tables with RLS and down-migrations; registration, sign-in, admin review queue and decisions; approved-peer directory; request/accept state machine; peer conversations separate from clinical chat; call request/accept and a peer RTC token path; notification types with PHI-free payloads; negative tests proving a peer token is refused on every doctor route. Several weeks across backend, Admin Web, Patient app and professional app.

Peer notifications (brief section C): **none implemented**; nothing to verify.

---

## G. Security and privacy

| Item | Finding | Evidence |
|---|---|---|
| RTC token | Server-only minting, participant- and window-gated; no certificate in the client | Verified (code), `consultation_service.rs:167-260` |
| Agora App Certificate | Backend env only | `.env` key name present; no `certificate` reference anywhere in `apps/mobile_patient` |
| Session tokens | Refresh rotation with reuse detection; device-bound proof (ES256, Android Keystore) | Verified (code). Availability risk in D-01, D-05 |
| BOLA coverage | Backend tests exist for: another patient's appointment, posting into another patient's chat, notifications, payment methods, dependents, health vault documents, photos, non-participant RTC/eligibility | Test names in `services/backend/tests/api_tests.rs` and `tests/cases/` |
| BOLA gap | **No test identified** (by name) proving a patient cannot **read** another patient's chat messages or attachments | Test gap, not a known defect |
| Unverified doctor exposure | `GET /doctors/:id` returns pending, rejected or revoked doctors' public profile fields to any signed-in user | `doctor_handlers.rs:59-73`, `doctor_repo.rs:100-108`. P2 |
| Push payload PHI | Titles/bodies are generic templates ("New consultation message", "Consultation starting soon"); `data` carries only ids and type, asserted by a test | `chat_service.rs:184-205`, `notification_transport.rs:254-264, 466-481` |
| Clinical image caching | Images use `Image.network` (Flutter in-memory cache only, no disk); PDFs render in memory (`clinical_pdf_viewer.dart:5-20`). The in-memory `imageCache` is not cleared at sign-out | No `imageCache` reference in `lib/`. P2 |
| Sensitive client logging | No `print`/`debugPrint`/`log(` in `lib/` | grep |
| Dev-only surfaces | "Development code" banner shown only with `ENVIRONMENT=development` + `DEV_AUTH_OTP_ENABLED` (`auth_cubit.dart:117-121`); backend forbids the flag outside dev/test (`config.rs:835-837`) | Verified (code) |
| Reminder privacy | Local notifications marked private; text hidden on lock screen (per replication status) | Not re-tested |
| Anonymous Q&A | The doctor question feed joins no patient identity (`doctor_handlers.rs:552`) | Verified (code) |

---

## H. Regression and test coverage

Present: 462 Patient widget/unit tests (all pass), including join-window gating, waiting-room polling, booking, auth layouts at five sizes and two scales, completion screen, reminders. Backend has extensive integration and security suites (not re-run this session).

Missing, highest risk first:

1. Sign-in → first authenticated burst → refresh behaviour (D-01). No test reproduces concurrent 401s right after sign-in.
2. Network loss during refresh rotation (D-05).
3. Notification settings opened from the real signed-in tree (D-04 would have been caught).
4. Home card visibility and label against the server `join` object, clock skew and resume (D-03).
5. Cross-patient **read** of chat messages and attachments (backend).
6. `integration_test/` does not exist; no end-to-end run against a backend.
7. Cross-client Patient ↔ Doctor consultation (booking → RTC join on both → completion → outcome visible to patient) — never run on this build.
8. Swahili and text-scale 1.3 coverage for Home tiles and Profile chips (V-04 to V-06).

---

## I. Production readiness

**Proven this session**

- Analyzer clean; 462/462 tests pass; debug build installs and starts.
- Registration and email-code verification create accounts and sessions server-side.
- Session restore on relaunch.
- Home, Services, Messages, Profile and all nine module screens render; English and Swahili; navigation between them.
- Honest unavailable states for Peer, Facilities, Videos, Share Access.
- Server-side controls for booking, verification filtering, overlap, join window and RTC (by code reading; backend tests exist).

- With the fixture doctor: discovery, profile, EAT slot display, deferred video booking, confirmation, waiting room, permission prompts, server join gating, RTC token minting for patient and doctor on one shared channel, Agora `joinChannel` accepted, server completion (COMPLETED_NO_RX), in-app notifications (booking, T-15, ready, completed) without PHI.

**Not proven or not working**

- Reliable sign-in (D-01) — **blocks everything else**.
- **Chat consultations in deferred mode (D-09)** — no conversation is ever created.
- Two-party video (doctor media, remote rendering, reconnection, audio-only fallback): needs a second client.
- Seeing the outcome after completion (D-11); consultation history.
- Push delivery (no FCM locally); the 15-minute reminder end to end.
- Local reminder delivery at its scheduled time.
- Password recovery in staging/production (D-02).
- Notification settings (D-04).
- Peer Navigator: nothing to prove; not built.

Recommendation: **not ready for a release candidate.** Do not produce a release APK until D-01 is fixed and a controlled two-client consultation passes.

---

## J. Prioritized remediation backlog

Each item: ID · severity · feature · description · evidence · reproduction · root cause (status) · source · scope · fix · tests · dependencies.

### P0

| ID | Feature | Description | Evidence | Reproduction | Root cause | Source | Scope | Proposed fix | Required tests | Depends on |
|---|---|---|---|---|---|---|---|---|---|---|
| P0-1 (D-01) | Auth/session | 2 of 3 sign-ins end at Log in | Metrics: protected GETs 401, no refresh call; sessions not rotated/revoked | Register or sign in → verify code | **Not verified**; suspects in D-01 | `api_client.dart`, `auth_cubit.dart`, `app.dart` | Frontend (backend if 401 reason is server-side) | Instrument, confirm, then fix the token-availability/refresh path | Concurrent-401 sign-in test; repeat ×20 on device | — |
| P0-2 | Consultation | Two-party video never verified (doctor media, remote rendering, reconnection) | Section L: patient side only, one device | — | Test gap | call controller, Doctor app/Web | Both clients | Run a controlled Patient (emulator) ↔ Doctor (Doctor Web or second device) call with the fixture doctor | Scripted two-client checklist | P0-1 |
| P0-4 (D-09) | Chat consultation | Deferred bookings never get a chat conversation; chat consultations and post-video chat unusable; contradictory status copy | `chat_conversations` 0 rows; `/conversation` 404; `[74]` | Book chat with payments deferred → Open consultation | **Verified (code)**: activation only in `process_payment_webhook` | `appointment_service.rs:284-417, 703-725`; `consultation_status_page.dart` | Backend + frontend copy | Activate conversation in the deferred booking path | Backend integration test; status-screen widget test | — |
| P0-3 | Peer Navigator (launch blocker for Peer only) | No backend, admin, or patient integration | Section F | — | Not built | — | Backend, Admin Web, both apps | Implement per Peer gap analysis after product decisions | Role-separation negative tests on every doctor route | Product decisions (location, review states, call initiation, retention) |

### P1

| ID | Feature | Description | Evidence | Fix | Tests | Depends on |
|---|---|---|---|---|---|---|
| P1-1 (D-04) | Notification settings | Never loads (provider scope) | `[40]`; no request reached server | Re-provide cubit / pass repo when pushing | Widget test in real tree | — |
| P1-2 (D-02) | Password recovery | Disabled outside dev; misleading 404 message | `[03]`; `config.rs:1114-1128` | Product decision; honest client message | Client message test | Email delivery acceptance |
| P1-3 (D-03) | Home card | Shows for any upcoming appointment instead of from 15 min before start; ignores server `join`; device clock; no resume reload | code | Show from `join_open_at` using `server_time`; tick; resume reload | Time-boundary widget tests (T−16, T−15, start, close) | None (decided 2026-11-13) |
| P1-4 (D-05) | Session | Strict reuse + resume rotation can log users out after a network drop | code (suspected) | Security decision on grace/rotation | Network-drop refresh test | P0-1 findings |
| P1-5 | Source control | Entire redesign, design references and UI docs are uncommitted/untracked; one working tree | `git status` | Commit to the feature branch (Patient and Doctor separately) once reviewed | — | Owner approval |
| P1-6 (D-06) | Booking entry | Chat booking not reachable from Book Consultation | `[22]`, `[31]` | Offer both modalities | Navigation test | — |
| P1-7 (V-04) | Accessibility | Profile chips overflow at text scale 1.3 (still open since baseline) | `[51]`, run log | Flexible chip height / wrap | Golden or overflow test at 1.3 | — |
| P1-8 | Push | Delivery unverifiable locally; no evidence it works on this build | backend startup warning | Verify on staging with FCM configured | Device push test incl. tap routing | Staging access |
| P1-9 | Localization | 53+ hardcoded English strings; Swahili machine-quality | grep; replication status | Move to ARB; native review | String-coverage test | Swahili reviewer |
| P1-10 (D-10) | Doctor fees | No API sets consultation fees; new doctors have 0.00 and are hidden from chat | code; fixture needed DB update | Fee endpoint + Doctor/Admin UI | Backend test | Product decision on who sets fees |
| P1-11 (D-11) | Completion / history | Completion screen only reachable from the waiting room; notification opens placeholder; no consultation history | `[78]`, `[81]`, `[82]` | Route notification taps to the completion/status screen; add past consultations | Widget + navigation tests | — |
| P1-12 | Admin approval | Doctor approval via Admin Web not exercised (no admin credentials available locally) | section L | Run once with admin access | Manual check | Admin password |

### P2

| ID | Description |
|---|---|
| P2-1 | V-01 registration header under status bar |
| P2-2 | V-02 keyboard hides Reset button |
| P2-3 | V-05/V-06 Swahili label wrapping and tile truncation |
| P2-4 | V-07/V-08 tab highlight from Home; More without bottom bar |
| P2-5 | V-10 Peer screen lists services that do not exist |
| P2-6 | V-11 remaining un-redesigned screens; raw "ACCESS" enum |
| P2-7 | D-07 Videos & Quizzes lands on empty Videos tab |
| P2-8 | D-08a stale auth error across screens |
| P2-9 | D-08b consents tickable unread (product/compliance decision) |
| P2-10 | D-08c Back on a tab exits the app |
| P2-11 | D-08d bell accessibility label |
| P2-12 | D-08e T-15 text always "15 minutes" (backend copy) |
| P2-13 | `GET /doctors/:id` exposes unverified doctors (backend) |
| P2-14 | 10-minute slot length not enforced (product decision; backend) |
| P2-15 | Chat list has no preview/time/unread (backend contract) |
| P2-16 | Clear `imageCache` at sign-out |
| P2-17 | Backend test for cross-patient chat **read** |
| P2-18 | Stale doc comment in `upcoming_consultation_section.dart`; `CLAUDE.md` still lists Reminders as no-backend "coming soon" |
| P2-19 | Untracked Android Studio template at repository root (owner decision) |
| P2-20 | D-12 waiting-room "Patients cannot place outgoing calls" contradicts the Join button |
| P2-21 | D-12 booking page "Doctor calls you directly" |
| P2-22 | D-12 chat sheet says "Paid… KES 500" while payments are deferred |
| P2-23 | D-12 "About Dr. Dr"; duplicate "Verified clinician" |
| P2-24 | D-12 notification inbox times in device time zone instead of EAT |
| P2-25 | Completion accepted although the doctor never joined (product question) |

Counts: **P0 4 · P1 12 · P2 25.**

---

## K. Fixture set (originally proposed; created on approval — see section L)

Target: local Docker database `hellodoctor-postgres` only (dev, `ENVIRONMENT=development`). Nothing touches staging, production or the two existing patient accounts.

Preferred mechanism: create everything through the public and admin APIs (doctor registration → admin verify in Admin Web → doctor creates slots → patient books), so every row passes the same validation, RLS and triggers as real data. No manual `_sqlx_migrations` edits.

| Item | How |
|---|---|
| 1 approved Doctor (`fixture.doctor@example.test`), VIDEO and CHAT fees set | `/auth/doctor/register` → Admin Web verify (needs the local admin account) |
| Doctor availability: several slots today and tomorrow, including one starting ~20 min from the test | Doctor app or `POST /doctor/slots` |
| Confirmed VIDEO appointment and confirmed CHAT appointment for `gap.audit.1113@example.test` | Patient app booking (deferred mode) |
| Synthetic chat history | Send messages from both apps |
| Synthetic health document | Pre-consultation intake upload of a generated test image |
| Article with quiz | Admin Web article + quiz import |
| Approved Peer Navigator / peer conversation | **Not possible** — no backend support |

This would let the Home card (T−15 boundary), confirmation, waiting room, Agora call with both clients, chat, completion and records be verified and captured.

---

## L. Consultation flow test with a fixture doctor (2026-11-13, 08:27–08:37 EAT)

One emulator, one patient client; the doctor acted through the backend API only.

| Step | Action | Result | Evidence |
|---|---|---|---|
| 1 | Register doctor via `POST /auth/doctor/register` | 200, PENDING | API response |
| 2 | Approve (direct DB update, reviewer = local admin) and set fees 800 / 500 (direct DB update) | VERIFIED | D-10; admin screen not exercised |
| 3 | Doctor signs in via API, creates three 10-minute slots | 08:33, 08:49, 09:07 EAT | `POST /doctor/slots` 200 |
| 4 | Patient: Book Consultation → Book a Video Consultation | Directory lists the doctor, fees, "Off duty" | `[61]` |
| 5 | View profile → Book video | Slots shown in EAT on a UTC+6 device; payment "deferred, KES 0" | `[62]`, `[64]` |
| 6 | Select 08:33 → Confirm | "Consultation Confirmed!", 8:33–8:43 AM EAT, reference APT-20261113-2960ECF6 | `[65]` |
| 7 | Open waiting room; tap Camera / Microphone | Android permission prompts; tiles turn "Ready"; Join enabled (window open from 08:18) | `[66]`–`[68]` |
| 8 | Join Video Call | `POST /rtc-token` 200; Agora `joinChannel` result 0; local preview; "Waiting for the doctor to join…" | `[69]`, logcat |
| 9 | Doctor mints token via API | Same channel `rtc_16112b42…` as the patient | API response |
| 10 | Hang up → Open 24h Chat | Snackbar "Chat opens once your appointment is confirmed." (it is confirmed); `/conversation` 404 | `[71]`, D-09 |
| 11 | Book chat for 09:07 → Open consultation | Confirmed, but status screen contradictory; no conversation row | `[72]`–`[74]`, D-09 |
| 12 | Home | No card until pull-to-refresh; then "Starting now" card for 08:33 | `[75]`, `[76]` |
| 13 | Doctor `POST /complete-no-rx` | 200; appointment COMPLETED / COMPLETED_NO_RX | DB |
| 14 | Home after refresh | Card for the 09:07 chat, "Starts in 32 min" (violates the 15-minute rule) | `[77]` |
| 15 | Messages Past / Active | Past empty; Active lists the chat consultation | `[78]`, `[79]` |
| 16 | Notification inbox | Booking confirmed ×2, T-15 reminder, Consultation ready, Consultation completed, first-consultation message; all generic, no PHI; times in device zone | `[80]` |
| 17 | Tap "Consultation completed" | Placeholder page: reference and "Completed" only | `[81]`, D-11 |
| 18 | Health Records → All Records | Empty | `[82]` |

Not exercised: doctor joining with media, remote video, reconnection, audio-only fallback, chat send/receive (blocked by D-09), pre-consultation photo upload, a prescription with an image, grievance, cancellation, overlapping booking attempt at runtime (covered by backend constraint and tests), local reminder delivery, push delivery.

Data left in the local database: fixture doctor, three slots (one booked and completed, one booked confirmed, one available), two appointments, notifications for the synthetic patient. All synthetic and disposable.
