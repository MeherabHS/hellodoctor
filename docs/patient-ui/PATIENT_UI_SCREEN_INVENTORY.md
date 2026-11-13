# Patient UI — Screen Inventory

Discovery date: 2026-11-12. Source: `apps/mobile_patient/lib` (116 hand-written Dart files, about 21,000 lines). Paths below are relative to `lib/`.

## Counting definitions and totals

| Thing | Definition | Count |
|---|---|---|
| Screen | A full-page widget shown as the app `home`, as a tab body, or pushed with `MaterialPageRoute` | **36** (34 reachable, 2 orphaned) |
| Tab destination | A body of the `IndexedStack` in `PatientShell` | 4 |
| Bottom sheet | A distinct `showModalBottomSheet` surface | 16 (15 reachable, 1 unused helper) |
| Dialog / picker | `showDialog`, `showDatePicker`, `showTimePicker` call sites | 8 |
| Route call site | Occurrences of `MaterialPageRoute` | 38 |
| Named routes | `routes:` / `onGenerateRoute` / `pushNamed` | 0 |
| Widget class | `class X extends StatelessWidget|StatefulWidget` | 120 (67 public, 53 private) |

Status legend: **functional** (real endpoint, real state, real UI), **partial** (works with a stated gap), **placeholder** (UI stub only), **orphan** (code exists, nothing navigates to it).

---

## 1. Navigation architecture

Imperative Navigator 1.0. There is no router package and there are no named routes; every transition is `Navigator.push(MaterialPageRoute(...))` with the default Material page transition.

```
main.dart  runZonedGuarded → HelloDoctorPatientApp        (StartupErrorPage on any boot failure)
└─ app.dart  _AppState: builds SecureTokenStore, PatientCacheStore, ConnectivityService,
   │         ApiClient, AuthCubit, LocaleController; AuthCubit.restore()
   └─ MaterialApp(key: ValueKey(state is SignedIn))       ← Navigator is recreated only at the auth boundary
      ├─ SessionRestoring → _SplashPage
      ├─ signed out       → AuthEntryPage
      └─ SignedIn         → _SignedInRoot                 (owns ProfileCubit, HomeCubit, FamilyProfilesCubit,
         │                                                 NotificationCubit, PushNotificationService)
         ├─ profile loading        → _SplashPage
         ├─ placeholder profile    → ProfileCompletionPage
         └─ NotificationNavigationHost → PatientShell
               IndexedStack [ PatientHomePage | PatientServicesPage | BlogsPage | PatientProfilePage ]
               bottomNavigationBar: PatientBottomNav      (labels: Home, Services, Blogs, Account)
```

Facts that matter for the redesign:

- **The third tab is Blogs and the fourth is labelled Account.** There is no Messages tab. The chat list is a pushed screen opened from a Home card or a Services row.
- Pushed screens cover the bottom navigation (they are pushed on the root Navigator). The reference shows the bottom bar on inner screens (panels 2–8 and 11), so this needs a decision: per-tab nested navigators, or a bar repeated on inner pages.
- Tab state survives tab switches (`IndexedStack`). Tab selection is plain `setState`; it is not restorable and has no deep link.
- Dependencies are passed by constructor. Pushed pages receive `ApiClient`, the cache and connectivity by reading them off `HomeCubit.repo`, and receive `FamilyProfilesCubit` as an argument, because pushed routes sit outside the tab's provider scope in practice.
- Notification taps are routed by `NotificationNavigationHost` (`features/notifications/notification_navigation.dart`), which resolves the destination through authorised API reads and then pushes `NotificationAppointmentPage`, `PatientChatPage`, `BlogDetailPage` or the notification center.
- Only `AgoraCallPage` uses `PopScope`. System back elsewhere is default behaviour.
- `popUntil(route.isFirst)` is used after booking, after the post-consultation screen and after password reset to return to the shell or sign-in page.

---

## 2. Startup and authentication

| # | Screen | File | Reached from | State | API | Status |
|---|---|---|---|---|---|---|
| 1 | `_SplashPage` | `app.dart:192` | App start; also while the profile first loads | `AuthCubit` (`SessionRestoring`), `ProfileCubit` | POST `/auth/refresh` during restore | functional |
| 2 | `StartupErrorPage` | `core/widgets/startup_error_page.dart` | Service construction throws (for example a missing `ENVIRONMENT` define); also the global `ErrorWidget.builder` | none | none | functional |
| 3 | `AuthEntryPage` (sign-in and register modes in one widget) | `features/auth/auth_entry_page.dart` | Signed-out `home` | `AuthCubit` | POST `/auth/patient/login`, `/auth/patient/register` | functional. All copy is hardcoded English |
| 4 | `EmailOtpVerifyPage` | `features/auth/email_otp_verify_page.dart` | `EmailChallengeSent` state | `AuthCubit` | POST `/auth/patient/verify-otp`; GET `/legal/documents` (registration shows four consent checkboxes) | functional |
| 5 | `ForgotPasswordPage` | `features/auth/forgot_password_page.dart` | "Forgot password?" | `AuthCubit` | POST `/auth/password-reset/request` | functional |
| 6 | `ResetOtpVerifyPage` | `features/auth/reset_otp_verify_page.dart` | after 5 | `AuthCubit` | POST `/auth/password-reset/verify-otp` | functional by code path; a possible client-side rejection on Android is flagged in the report, section I |
| 7 | `CreateNewPasswordPage` | `features/auth/create_new_password_page.dart` | after 6 | `AuthCubit` | POST `/auth/password-reset/complete` | functional |
| 8 | `PasswordResetSuccessPage` | `features/auth/password_reset_success_page.dart` | after 7 | none | none | functional |
| 9 | `ProfileCompletionPage` | `features/auth/profile_completion_page.dart` | `_SignedInRoot` when `profile.isPlaceholder` | `ProfileCubit` | PATCH `/patients/me` | functional |
| 10 | `PhoneEntryPage` | `features/auth/phone_entry_page.dart` | nothing | `AuthCubit` | POST `/auth/register-otp` | **orphan** (legacy phone-OTP entry; has one widget test) |
| 11 | `OtpVerifyPage` | `features/auth/otp_verify_page.dart` | only from 10 | `AuthCubit` | POST `/auth/verify-otp-and-login` | **orphan** |

Dialogs and pickers: legal-document `AlertDialog` (4), date-of-birth `showDatePicker` (3 and 9).

---

## 3. Tab 1 — Home

| # | Screen | File | State | API | Status |
|---|---|---|---|---|---|
| 12 | `PatientHomePage` | `features/patient/home/patient_home_page.dart` | `HomeCubit`, `ProfileCubit`, `FamilyProfilesCubit`, `NotificationCubit` (all root-scoped) | GET `/patients/me/appointments?status=upcoming`, `/patients/me`, `/patients/me/profiles`, `/notifications/unread-count` | functional |

What renders, top to bottom:

1. `_Greeting`: "Welcome back" and the name, notification bell with unread badge, avatar icon (tap switches to the Account tab).
2. `_CoreServicesGrid`: a 2 × 2 grid of four cards: Book Specialist → directory; Chat with Doctor → chat list; Ask Free Q&A → Q&A; Health Records → vault. Each card has an icon, a badge, a title and a description.
3. `EmergencyContactBanner`: dark green strip, "Call 999" button (`tel:` link). The number is a client constant.
4. `_ProfileSwitcherBar`: "Managing for …"; opens the profile switcher sheet.
5. `_UpcomingConsultationSection`: skeleton, inline error, "no upcoming" card, or `_ConsultCard` (doctor photo, name, specialty, date and EAT time, status chip, "Active Consultation / Rejoin" strip, reconnect hints). Tapping opens the waiting room.

There is no search field on Home today. Pull-to-refresh reloads all three cubits.

Sheets: `_ProfileSwitcherSheet`, then `showAddDependentSheet`.

Pushed from Home: 16 `NotificationCenterPage`, 20 `PatientDirectoryPage`, 24 `PatientChatListPage`, 27 `WaitingRoomPage`, 30 `HealthVaultPage`, 33 `QnaPage`.

---

## 4. Tab 2 — Services

| # | Screen | File | State | API | Status |
|---|---|---|---|---|---|
| 13 | `PatientServicesPage` | `features/patient/services/patient_services_page.dart` | none of its own (reads `HomeCubit.repo`, `FamilyProfilesCubit`) | none | partial |

A `PatientPageHeader` ("Core Services") over six rows: Book Specialist, Chat with Doctor, Ask a Free Question and Health Records are live; **Medicine Delivery and Emergency Dispatch only show a "coming soon" snackbar.** The route builders duplicate those in the Home page.

---

## 5. Tab 3 — Blogs (the slot the reference gives to Messages)

| # | Screen | File | State | API | Status |
|---|---|---|---|---|---|
| 14 | `BlogsPage` | `features/patient/blogs/blogs_page.dart` | `BlogDiscoveryController` (ChangeNotifier, page-local) | GET `/articles?limit=20&search&language` with cursor paging; GET `/articles/:id/cover` | functional |
| 36 | `BlogDetailPage` | `features/patient/blogs/blog_detail_page.dart` | `FutureBuilder`; `ArticleQuizController` inside `ArticleQuizSection` | GET `/articles/:id`; GET `/articles/:id/quiz`; POST `/articles/:id/quiz/check` | functional |

`BlogsPage`: header, search field with 300 ms debounce, skeleton / error / empty states, article cards (cover, title, summary, author or "Editorial", "Read more"), "Load more". `BlogDetailPage`: cover, category label, title, author row, Markdown body for admin articles, references, and a quiz section when an admin article has a published quiz.

There are no topic filters, no tabs, no videos and no audio.

---

## 6. Tab 4 — Account (the reference calls it Profile)

| # | Screen | File | State | API | Status |
|---|---|---|---|---|---|
| 15 | `PatientProfilePage` | `features/patient/profile/patient_profile_page.dart` | `ProfileCubit`, `FamilyProfilesCubit`, `LocaleController`, `AuthCubit` | GET `/patients/me`, `/patients/me/profiles`, `/patients/me/photo` | functional, with two placeholder rows |
| 18 | `NotificationSettingsPage` | `features/notifications/notification_settings_page.dart` | page-local, via `NotificationCubit.repo` | GET/PUT `/notifications/preferences` | functional |
| 19 | `DataRightsPage` | `features/patient/profile/data_rights_page.dart` | page-local; calls the API directly | GET/POST `/patients/me/data-requests` | functional. Unstyled, hardcoded English, raw enum values as labels |

What renders on the tab: `_Hero` card (photo avatar, name, gender and age, edit button), Family Profiles chip row, then rows: Language, Notifications, Payment Methods (**"coming soon" snackbar**), Privacy & Security (opens `DataRightsPage`), Report an Issue (**"coming soon" snackbar**), and a red outlined Log out button.

Sheets: `showProfileEditSheet` (name, date of birth, gender, blood group, weight and further health fields; PATCH `/patients/me`), photo options sheet (camera, gallery, remove; POST/DELETE `/patients/me/photo`), `showAddDependentSheet` (POST `/patients/me/profiles`), `showLanguageModal` (English / Kiswahili, applied instantly). Pickers: date of birth in the edit and dependent sheets; quiet-hours `showTimePicker` in notification settings.

Not present: a help or support screen, a privacy-policy reader (policy text is only shown during registration), change password, dependent editing.

---

## 7. Notifications

| # | Screen | File | State | API | Status |
|---|---|---|---|---|---|
| 16 | `NotificationCenterPage` | `features/notifications/notification_center_page.dart` | `NotificationCubit` (root) | GET `/notifications`, POST `/notifications/:id/read`, `/read-all` | functional |
| 17 | `NotificationAppointmentPage` | `features/notifications/notification_appointment_page.dart` | page-local | GET `/appointments/:id` | functional |

---

## 8. Book Consultation flow

| # | Screen | File | State | API | Status |
|---|---|---|---|---|---|
| 20 | `PatientDirectoryPage` | `features/patient/directory/patient_directory_page.dart` | `DirectoryCubit` (page) | GET `/doctors`, `/config/booking-policy`, `/doctors/:id/photo` | functional |
| 21 | `DoctorProfilePage` | `features/patient/directory/doctor_profile_page.dart` | `FutureBuilder`, local tab state | GET `/doctors/:id`, `/doctors/:id/slots`, `/config/booking-policy` | functional |
| 22 | `PatientBookingPage` | `features/patient/booking/patient_booking_page.dart` | `BookingCubit` (page), `AvailabilityRefresh` (30 s) | GET `/doctors/:id/slots`, `/config/booking-policy`, `/patients/me/payment-methods`; POST `/appointments/book` | functional |
| 23 | `ConsultationStatusPage` | `features/patient/chat/consultation_status_page.dart` | page-local, `ScreenRefresh` (15 s), hold countdown | GET `/appointments/:id`; POST `/appointments/:id/payment`, `/appointments/:id/trace-ack` | functional. Partly hardcoded English |
| 27 | `WaitingRoomPage` | `features/patient/booking/waiting_room_page.dart` | `WaitingRoomCubit` (page), 1 s ticker | GET `/appointments/:id`, `/consultations/:id/join-eligibility`, `/appointments/:id/conversation` | functional |
| 28 | `AgoraCallPage` | `features/patient/video/agora_call_page.dart` | `CallController` | POST `/consultations/:id/rtc-token`, `/consultations/:id/telemetry/events` | functional |
| 29 | `PostConsultationPage` | `features/patient/consultation/post_consultation_page.dart` | props from the waiting room | none of its own | functional |

Directory: back arrow, title, search field, three filter buttons (specialty, availability, visit type; each opens a sheet), doctor count with Reset, `PatientDoctorCard` list (photo, name, specialty, verified icon, hospital, experience, duty status, fees in KES, "View profile" and a book button). There are no star ratings anywhere, as required.

Booking page: doctor banner, step indicator, "Booking for" profile bar (sheet), date slider, slot grid with "Times shown in EAT (UTC+3)", reason field, fee summary, payment-method picker only when the server policy says payment is required, and a sticky bottom button ("Confirm Booking" when deferred, "Proceed to payment" when paid).

Waiting room: doctor card with slot label and status chip, completed and terminal banners, join-gate notice (`Key('join-gate-notice')`), camera and microphone permission check, "Join Video Call" button enabled only inside the server window, chat entry, grievance entry.

Sheets: specialty, availability and visit-type filters (20); `showConsultationChoices` (20, 21); `showPaidChatBookingModal` (20, 21); "Booking for" switcher (22); `showAddPaymentMethodSheet` (22); `PreConsultIntakeModal` (23; 1 to 20 files, JPEG / PNG / PDF); `showGrievanceModal` (25, 27, 29).

---

## 9. Messages (not a tab today)

| # | Screen | File | State | API | Status |
|---|---|---|---|---|---|
| 24 | `PatientChatListPage` | `features/patient/chat/patient_chat_list_page.dart` | page-local `setState`, `ScreenRefresh` (30 s); no cubit | GET `/patients/me/appointments?limit=100&offset` filtered client-side to `modality == 'CHAT'` | partial |
| 25 | `PatientChatPage` | `features/patient/chat/patient_chat_page.dart` | `ChatCubit` (page), `ScreenRefresh` (10 s polling) | GET/POST `/chat/:id/messages`; GET `/chat/:id/messages/:mid/attachment`; POST `/appointments/:id/trace-ack` | functional |
| 26 | `ClinicalImageViewer` | `features/patient/chat/clinical_image_viewer.dart` | page-local | attachment bytes (above) | functional |

Chat list: stock `AppBar` "Chat consultations", "Book a chat consultation" button, Active / Past chips, cards showing doctor name, specialty, raw lower-cased status, fee (only when payment is required) and an "Open consultation" button that leads to `ConsultationStatusPage`, not directly to the thread. **It shows no last message, no timestamp, no unread indicator and no avatar**, because the endpoint returns none of them. Every string is hardcoded English. Chats attached to video appointments are not listed.

Chat thread: custom top bar (back, doctor name, report-issue button), expiry bar, message bubbles, older-message paging (50 per page), text composer, locked and expired states. Patients can view images a doctor sent; they cannot send attachments. Delivery is by polling, not sockets.

---

## 10. Health Records

| # | Screen | File | State | API | Status |
|---|---|---|---|---|---|
| 30 | `HealthVaultPage` | `features/patient/vault/health_vault_page.dart` | `VaultCubit` (page) | GET `/patients/me/documents`, `/appointments/:id/documents/:docId` | functional |
| 31 | `ClinicalPdfViewer` | `features/patient/vault/clinical_pdf_viewer.dart` | page-local | document bytes, verified against the declared MIME type | functional |
| 32 | `_ImageViewerPage` | `features/patient/vault/health_vault_page.dart:358` | none | short-lived download URL | functional |

Tabs are three chips: All, e-Prescriptions, My Uploads. The backend has exactly two document kinds (`PRESCRIPTION`, `INTAKE`). Documents follow the active family profile.

---

## 11. Q&A (the brief's "HIV Support Chat")

| # | Screen | File | State | API | Status |
|---|---|---|---|---|---|
| 33 | `QnaPage` (`QnaFeed`) | `features/patient/qna/qna_page.dart` | `QnaCubit` (page) | GET `/patients/me/qna/questions?include_own_open=true` | functional |
| 34 | `AskQuestionPage` | `features/patient/qna/qna_ask_sheet.dart` | `QnaCubit` | POST `/patients/me/qna/questions` | functional |
| 35 | Full-answer page (inline `Scaffold`) | `features/patient/qna/qna_page.dart:165` | none | none | functional |

Feed: title "Ask a Doctor", subtitle, "Ask" button, "Recent" list of `QuestionArticle` entries (category, "Anonymous", question, answering doctor, truncated answer, "Read full answer"). The ask form has a category dropdown fixed to five general-health topics, a 1 to 4,000 character question field (`Key('qna-question')`, `Key('qna-submit')`) and a profile selector.

---

## 12. Modules with no screen at all

| Module | Client code | Backend |
|---|---|---|
| Peer Navigator | none | none |
| Reminders (patient-authored) | none. `flutter_local_notifications` is present but is used only to surface server notifications | none |
| Facilities & Services | none | none |
| Videos | none | none |
| Audio | none | none |
| "More" page (Profile Settings, Help & Support, Privacy Policies) | none | legal documents endpoint exists; no help or support endpoint |
| GHI dashboard, Multi-channel access, Privacy & Security information page | none | none patient-facing |

---

## 13. Cross-cutting behaviour per screen type

| Concern | Current state |
|---|---|
| Loading | `SkeletonBlock` on Home, blogs, directory, booking, vault, Q&A; bare `CircularProgressIndicator` on Account and the chat list |
| Error | `ErrorPanel` (full) or `InlineError` (row), usually with Retry. `ApiException.userMessage` supplies the text |
| Empty | `EmptyState` on blogs, directory and Q&A; plain text on the chat list and Home |
| Offline | `CacheStatusBar` on vault and notification center; cached data for profile, home, directory, vault, family profiles |
| Refresh | Pull-to-refresh on every list; timed refresh on chat (10 s), status (15 s), chat list and slots (30 s), notifications (60 s) |
| Keyboard | Sheets add `viewInsets.bottom`; full pages rely on `Scaffold` default resize |
| Text scale / small widths | Handled deliberately only in `PatientDoctorCard` (stacks buttons above text scale 1.2 or below 300 dp) |
| Accessibility | Sparse: `semanticLabel` or `tooltip` appears in seven files. The emergency button and the Home avatar have tap targets below 48 dp |
| Localization | 485 keys each in `app_en.arb` and `app_sw.arb`. Auth pages, chat list, data rights and parts of consultation status and the directory bypass it |
| Dark mode | Not implemented |
