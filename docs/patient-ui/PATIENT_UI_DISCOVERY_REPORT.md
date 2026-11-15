# Patient UI — Discovery Report

Date: 2026-11-12. Branch `fix/doctor-web-rtc-token` at `3c09124`. Scope: `apps/mobile_patient` only. No application source, backend or database file was changed. This report and its four companion documents are the only files added, plus four baseline screenshots.

Evidence labels used throughout: **[V]** verified by reading source, running a command or sampling the image; **[D]** deduced from code and not confirmed at runtime; **[A]** assumption that needs approval.

Companion documents: `PATIENT_UI_SCREEN_INVENTORY.md`, `PATIENT_UI_COMPONENT_MAP.md`, `PATIENT_UI_EMULATOR_WORKFLOW.md`, `PATIENT_UI_REDESIGN_PLAN.md`.

## Summary

- The Patient app is a working, well-tested clinical client (407 tests pass, analyzer clean) whose UI is the old prototype's: bottom navigation Home, Services, Blogs, Account, and four Home cards.
- Of the nine modules, three are reusable as they stand behind a new skin (Book Consultation, Health Records, HIV Support Chat as the existing Q&A). Two are partly there (education articles and quizzes, without topics, videos or audio). Three have nothing on either the client or the backend (Peer Navigator, Reminders, Facilities). "More" has two of its three destinations.
- Business logic sits in repositories and cubits and is cleanly separable from presentation in all but four pages.
- The reference image and the v2.1 brief contradict each other in several places, the image has no panel for Messages or Profile, and its resolution is too low for pixel-exact measurement. Fifteen decisions are needed before the first batch (section I).

---

## A. Repository and documentation inventory

### Read

| Source | What it establishes |
|---|---|
| `CLAUDE.md` (modified, uncommitted) | The reference image is the pixel-level source of truth for the Patient app; eleven panels; logic preserved; honest states where no backend exists; Emergency Contact banner immediately above the Upcoming Consultation card |
| `AfyaMkononi_Patient_UI_Replication_Fable52_Emulator_v2_1 (1).docx` (repo root, untracked; read in full) | Nine modules M01–M09, Messages and Profile as mandatory redesign targets, finance deferment, emulator-first workflow, acceptance gates. It contains no images |
| `docs/design/afyamkononi-patient-ui-reference.jpeg` (untracked) | The only visual reference. 1214 × 1295 px, eleven panels |
| `AGENTS.md`, `BUSINESS_LOGIC.md`, `checkpoint.md`, `progress.md` | Financial deferment (`GET /config/booking-policy`, default paid) is documented only in the last two |
| `docs/PATIENT_UI_VISUAL_REDESIGN.md` | Not a redesign record. It is a 2026-10-14 correction pass on doctor discovery, doctor profile and Q&A |
| `docs/PATIENT_AUTH_NAVIGATION_FIX.md`, `PATIENT_CHAT_CONSULTATION_VERIFICATION.md`, `PATIENT_PAYMENT_UX_VERIFICATION.md`, `PATIENT_SLOT_AVAILABILITY_VERIFICATION.md`, `PRESCRIPTION_UPLOAD_40KB_20_IMAGE_VERIFICATION.md` | Prior verification of the flows that must not regress |
| `docs/FLUTTER_ARCHITECTURE.md`, `UI_DESIGN_SYSTEM.md`, `VISUAL_FIDELITY.md`, `ASSET_SPEC.md`, `SCREEN_INVENTORY.md`, `USER_FLOWS.md`, `INTERACTION_SPEC.md`, `STATE_MACHINES.md`, `TESTING_STRATEGY.md`, `DEFINITION_OF_DONE.md`, `IMPLEMENTATION_ROADMAP.md`, `OPEN_QUESTIONS.md`, `REALTIME.md`, `FCM_SETUP.md` | The prototype-era specification. Describes five prototype tabs and a forest-teal palette (`#0D5C52`); now stale for the Patient app |
| `docs/PROGRESS.md`, `docs/CHECKPOINT.md` | Read selectively (recent and patient entries) |
| `apps/mobile_patient/lib` | 116 Dart files. The app shell, theme, shared widgets, Home, Services, Account, chat list, blogs, Q&A, directory, sign-in and data-rights pages were read in full; booking, waiting room, chat thread, consultation status, vault and article reader in part; all non-UI files (network, cache, notifications, video, repositories, cubits, models) were read by a delegated pass |
| `apps/mobile_patient/test` | 49 files inventoried; five read in full |
| `services/backend/src`, `services/backend/migrations` | Router and handlers checked for every endpoint the app calls and for each new module. Read only |

### Architectural findings [V]

- State management is `flutter_bloc` cubits plus two `ChangeNotifier` controllers and `provider` for the locale. Five objects are session-scoped in `_SignedInRoot`; the rest are created by their page.
- Navigation is imperative `Navigator.push`; there are no named routes and no router package.
- `ApiClient` enforces HTTPS, an explicit `ENVIRONMENT`, device proof and token rotation. A debug build will not start without dart-defines.
- Chat is HTTP polling every 10 seconds. There are no WebSockets on either side.
- The only bundled asset is the Plus Jakarta Sans font.

### Missing or broken

| Item | State |
|---|---|
| Reference panels for **Messages** and **Profile** | Do not exist in the image |
| References 1–4 named in the brief's Appendix B as separate screenshots | Not in the repo; only the single collage is |
| Logo, wordmark, illustrations, facility photos | Not in the repo |
| `integration_test/` (named in `CLAUDE.md` section 4) | Does not exist |
| Golden tests | None |
| `apps/mobile_patient/README.md` | Flutter template text |
| Any doc describing the shipped app's real tab list or an emulator recipe | None |
| Root `.fig`, sixteen `Hello Doctor [B]` PDFs | The original Bangladesh-era designs; not relevant to this look |

---

## B. Screen inventory

Full table: `PATIENT_UI_SCREEN_INVENTORY.md`. Totals and definitions:

| Thing | Definition | Count |
|---|---|---|
| Screens | Full-page widgets shown as `home`, a tab body, or pushed | 36 (34 reachable, 2 orphaned phone-OTP pages) |
| Tab destinations | Bodies of the shell's `IndexedStack` | 4 |
| Bottom sheets | Distinct `showModalBottomSheet` surfaces | 16 (1 unused) |
| Dialogs and pickers | `showDialog`, date and time pickers | 8 |
| Route call sites | `MaterialPageRoute` occurrences | 38 |
| Widget classes | Stateless or stateful widget classes | 120 (67 public, 53 private) |

---

## C. Widget and component inventory

Full detail: `PATIENT_UI_COMPONENT_MAP.md`. In brief:

- One token file (`PatientTokens`), Material 3, light theme only, primary `#087A5B`.
- Fourteen shared widgets in `core/widgets/`; two are unused.
- Eight duplicated patterns, the largest being the back-arrow header (seven hand-built copies plus fifteen stock `AppBar` uses) and the icon-title-chevron row.
- 35 colour literals outside the token file and at least 70 hardcoded English strings.
- About two dozen reference components are needed; roughly half are new, the rest are restyles of existing widgets.

---

## D. Feature and API dependency matrix

Every endpoint the app calls exists in the backend router [V]. All patient endpoints need a Bearer token except the `/auth/*` and `/legal/documents` routes.

### The nine modules

| # | Module | Current implementation | Endpoints | State | Status | Backend change for the redesign |
|---|---|---|---|---|---|---|
| M01 | HIV Prevention & Sexual Health | `BlogsPage`, `BlogDetailPage` (today's Blogs tab). No tabs, no topic cards | GET `/articles`, `/articles/:id`, `/articles/:id/cover` | `BlogDiscoveryController` | **Partial.** Articles work. Topic filtering is not possible: the feed has no category or tag parameter, the category list is Admin-only, and no categories are seeded | Yes for topic filter. Videos and Audio have no content model at all |
| M02 | HIV Support Chat | `QnaPage`, `AskQuestionPage` (anonymous Q&A) | GET/POST `/patients/me/qna/questions` | `QnaCubit` | **Functional** as generic Q&A. Category list is hardcoded to five general-health topics | No for new category names (the field is free text). Yes for a server-side category filter |
| M03 | Book Consultation | Directory → doctor profile → booking → status → waiting room → Agora call → post-consultation | `/doctors*`, `/config/booking-policy`, `/appointments/*`, `/consultations/*` | `DirectoryCubit`, `BookingCubit`, `WaitingRoomCubit`, `CallController` | **Functional**, including finance deferment | No |
| M04 | Peer Navigator Support | None | None | None | **Missing** on client and backend (no role, table or route) | Yes, entirely |
| M05 | Reminders | None. Local notifications are used only to surface server notifications | None | None | **Missing** on client and backend | Yes, or a local-only design |
| M06 | Health Records | `HealthVaultPage`, PDF and image viewers | GET `/patients/me/documents`, `/appointments/:id/documents/:docId` | `VaultCubit` | **Functional** for the two document kinds that exist (prescriptions, patient uploads) | Yes for anything else panel 6 shows (HIV care summary, lab results, share access) |
| M07 | Videos & Quizzes | Quizzes exist only inside admin articles (`ArticleQuizSection`). No video code | GET `/articles/:id/quiz`, POST `/articles/:id/quiz/check` | `ArticleQuizController` | **Partial.** Quiz flow works; there is no standalone quiz list and results are not stored. Videos are missing | Yes for videos and for a quiz listing |
| M08 | Facilities & Services | None | None | None | **Missing** on client and backend | Yes, entirely |
| M09 | More | No page. Profile settings exist; privacy policy text is shown only during registration; no help or support screen | GET `/legal/documents` exists; no help or support endpoint | None | **Partial** | Only for Help & Support |

### The four destinations

| Destination | Today | Status |
|---|---|---|
| Home | Greeting, bell, four service cards, emergency banner, profile switcher, one upcoming-consultation card | Functional; layout is the old prototype's |
| Services | Six rows; four live, two "coming soon" snackbars (Medicine Delivery, Emergency Dispatch) | Partial |
| Messages | **Not a tab.** A pushed chat list built from the appointments endpoint, filtered to chat consultations; no preview, timestamp, unread count or avatar; English only. Thread works: text, paging, doctor-sent images, 10-second polling | Partial list, functional thread |
| Profile | Tab labelled Account: photo, name, family profiles, language, notification settings, data-rights form, logout. Two "coming soon" rows (Payment Methods, Report an Issue) | Functional with placeholders |

### Other features

| Feature | Endpoints | State | Status |
|---|---|---|---|
| Registration, sign-in, email OTP, consent | `/auth/patient/*`, `/legal/documents` | `AuthCubit` | Functional |
| Password reset | `/auth/password-reset/*` | `AuthCubit` | Functional by code path; see risk R9 |
| Phone-OTP sign-in | `/auth/register-otp`, `/auth/verify-otp-and-login` | `AuthCubit` | Orphaned UI |
| Session restore, refresh, logout | `/auth/refresh`, `/auth/logout`, `/notifications/devices/revoke` | `AuthCubit`, `ApiClient` | Functional |
| Pre-consultation uploads | POST `/appointments/:id/prescriptions` | page-local | Functional; 1–20 files enforced on both sides, 40 KB compression on the server only |
| Payment methods and checkout | `/patients/me/payment-methods`, POST `/appointments/:id/payment` | `BookingCubit`, page-local | Partial; shown only when the server policy requires payment |
| Grievance | GET `/appointments/:id/consultation`, POST `/grievances` | page-local | Partial; submit only, reachable only from a consultation |
| Notifications inbox, settings, push | `/notifications*` | `NotificationCubit` | Functional |
| Family profiles | GET/POST `/patients/me/profiles` | `FamilyProfilesCubit` | Partial; no edit path |
| Language | PUT `/notifications/preferences` | `LocaleController` | Functional |
| Data rights | `/patients/me/data-requests` | page-local, direct API calls | Functional, unstyled |
| Emergency contact | none; `tel:999` constant | none | Functional, hardcoded |

Authorisation note: handler-level guards were verified for every route. Row-level ownership behind the `AnyAuthenticated` routes (appointment detail, conversation, messages, documents, RTC token) was not traced.

---

## E. Reference-to-screen mapping

One reference exists: the eleven-panel collage. Each phone in it is about 216 × 535 px.

| Panel | Replaces / maps to | What matches today | What is missing | Conflict or flag |
|---|---|---|---|---|
| 1 Home | `PatientHomePage`, `PatientBottomNav` | Bell with badge, name greeting | Brand header, greeting card, hero banner, 3 × 3 grid, nav labels | Search field (brief says none). Card 3 reads "Book Testing, PrEP or ART" (brief says "Book Consultation"). No emergency banner or upcoming-consultation card, which `CLAUDE.md` requires |
| 2 Prevention & Education | `BlogsPage` | Article list and reader | Segmented tabs, six topic cards with illustrations and durations | No topic filter, videos, audio or reading-time data |
| 3 HIV Support Chat | `QnaPage` | Anonymous ask flow, feed | Intro banner, five category rows, teal CTA, privacy card | "Chat" must not imply live messaging (brief's safeguard). `CLAUDE.md` lists this panel as having no backend, while the brief maps it to the existing Q&A |
| 4 Peer Navigator | nothing | nothing | Whole screen | No backend. "Start Chat" must not appear to work |
| 5 Reminders | nothing | nothing | Whole screen | No backend. Panel says "Your HIV status is not shown on your lock screen"; the brief asks for detailed reminder wording that can appear there |
| 6 Health Records | `HealthVaultPage` | Prescriptions, documents | "My Records / Share Access" tabs, five category rows, access-control card | HIV Care Summary, Lab Results, Appointments and Share Access have no data source |
| 7 Facilities & Services | nothing | nothing | Whole screen | No backend. Shows distance, "Nearby" and a map; the brief rules out GPS and an in-app map |
| 8 Video Consultation | Entry to directory and booking | Whole booking and call flow exists | Landing layout, illustration, checklist | "Start Video Call" implies an instant call; calls are slot-booked and join-window gated. Urgent card shows "Call 11663"; the app's constant is 999 |
| 9 Dashboard for GHI | nothing | nothing | Whole screen | Programme-staff indicators with a different bottom bar (Home, Programme, Reports, Settings). Not a patient screen; no patient-facing data |
| 10 Multi-channel Access | nothing | nothing | Informational tiles | WhatsApp, SMS and USSD do not exist. Reads as programme description, not an app screen |
| 11 Privacy & Security | `DataRightsPage` (loosely) | Data-rights requests | Checklist layout | Claims such as "Anonymous questions and chat" and "No HIV status on lock screen" must be true before they are shown |
| none | Messages inbox and thread | Chat list and thread exist | **No reference** | Brief requires a full redesign |
| none | Profile and its child screens | Account tab exists | **No reference** | Brief requires a full redesign |
| none | Sign-in, registration, OTP, reset, booking, waiting room, call | All exist | **No reference** | `CLAUDE.md`: restyle with the image's design language |

Measurability [V]: at about 0.6 image pixels per dp, colours can be sampled (the component map lists them; the reference's primary green is about `#009A73` against today's `#087A5B`), but spacing, radii and type sizes can only be estimated to roughly ±2 dp. "Pixel for pixel" is not achievable from this file alone.

---

## F. Emulator baseline

Full detail: `PATIENT_UI_EMULATOR_WORKFLOW.md`.

| Item | Result |
|---|---|
| Toolchain | Flutter 3.47.6, Dart 3.13.5, Java 21, Android SDK platforms 31–36 |
| First inspection | No emulator package, system image or AVD |
| Later the same day | `Pixel_9` AVD present (Android 17, API 37, 1080 × 2424 px at 420 dpi, about 411 × 923 dp, font scale 1.0); WHPX acceleration usable |
| Launch | Debug build from source started on the emulator against the local backend and rendered the sign-in page. No code or security flag was changed; four dart-defines were passed |
| Screenshots | 31 in `docs/patient-ui/screenshots/baseline/pixel_9_api37/`. Signed-out: sign in, registration, date picker, forgot password. Signed-in (test account supplied by the product owner): Home, Services, Blogs, Account and its four sheets, notification settings, data rights, notification center, doctor directory and a filter sheet, chat list, Health Vault, Q&A feed and ask form; plus Home, Services and Account in Swahili and at text scale 1.3 |
| States seen | Mostly empty states, because the local database has no doctors, articles, appointments or documents |
| Not captured | Doctor profile, booking, consultation status, waiting room, call, post-consultation, chat thread, article reader and quiz, populated lists, OTP and reset follow-on pages, splash, startup error |
| Found while capturing | Family Profiles chips overflow by 28 px at text scale 1.3; notification settings showed its error state (cause not investigated); the language modal stays open after a choice |
| Method | Navigation only; nothing was booked, submitted or saved, and no data was seeded |
| Which build | Built from this repository's source. `apps/mobile_patient` is identical at `3c09124` and at `966e85a`, the commit of the distributed test APK, so the UI is the same; the APK itself was not installed |
| Remaining setup | A second, compact (360 dp) AVD |

---

## G. Functional preservation checklist

Test baseline [V]: `flutter analyze` reports no issues; `flutter test` passes 407 of 407 in 49 files. 232 tests have no UI coupling; 175 sit in 18 files that render production widgets.

| Behaviour that must not regress | Where the logic lives | Test coverage |
|---|---|---|
| Registration, sign-in, OTP, consent | `AuthCubit`, `AuthRepository` | Covered (`auth_navigation_test`, `auth_cubit_test`) |
| Auth-gated navigation, logout clears routes and tokens | `app.dart`, `ApiClient` | Covered |
| Session restore | `AuthCubit.restore` | Partial; no widget test boots with stored tokens |
| Password reset | `AuthCubit` | Partial; cubit only, pages never rendered |
| Doctor discovery | `DirectoryCubit` | Covered |
| Slot selection, stale-slot clearing, conflict | `BookingCubit` | Covered |
| Past slot cannot be booked | Server only; the client has no filter | **Not covered** on the client |
| Finance deferment, zero-fee booking, no fee guard | `BookingCubit`, `directory_models.dart`, server policy | Covered |
| No simulated paid state | `ConsultationStatusPage` | Partial |
| Intake upload cap of 20 | `upload_validation.dart`, intake modal, server | Partial; the modal is never rendered in a test |
| 40 KB per image | Server only | Not covered in Flutter, by design |
| Join window, countdown, clock skew | `join_eligibility.dart`, `WaitingRoomCubit` | Covered (20 tests) |
| Agora lifecycle, token mint failures | `CallController`, `mint_failure.dart` | Covered (21 tests); the call page itself is never rendered |
| Chat send, paging, lock | `ChatCubit` | Covered for logic; partial for UI |
| Chat list | page-local state | Partial (fee display only) |
| Grievance submission | grievance modal | **Not covered** |
| Health records and viewers | `VaultCubit`, vault page | Covered for routing |
| Articles and quizzes | controllers | Covered |
| Anonymous Q&A | `QnaCubit` | Covered |
| Notifications: inbox, taps, polling | `NotificationCubit`, navigation host | Covered; real push payloads partial |
| Profile view and edit | `ProfileCubit` | **Not covered**; photo paths only |
| Family profiles | `FamilyProfilesCubit` | Logic covered; UI not |
| Language toggle | `LocaleController` | Partial |
| Emergency banner directly above the upcoming card, never "24/7" | `patient_home_page.dart:166-168` | **Not covered** |
| EAT time labels | `time_format.dart`, models | Covered |
| Bottom navigation safe area | `PatientBottomNav` | Covered, and will need rewriting |
| Cache scoping and PHI rules, API host policy | `PatientCacheStore`, `ApiClient` | Covered |

Tests the redesign will touch:

- **Certain:** `patient_navigation_safe_area_test` (8 tests) asserts the labels Home, Services, Blogs, Account.
- **If copy changes:** `auth_navigation`, `booking_prototype_parity`, `consultation_flow`, `patient_discovery_qna_correction`, `patient_quiz`, `consultation_booking`, `pre_e2e_availability_payment`. They assert English literals.
- **If widget types change:** tests that find `FilledButton`, `PatientField`, `PatientCard`, `PatientBottomNav`, `DoctorDirectoryFilters` and five others.
- **If Home loads more data at boot:** two tests throw on any request outside a fixed list.
- **Structural counts:** exactly four checkboxes on registration OTP, exactly one filled button on the unpaid status page, no `Card` in Q&A.

---

## H. Recommended redesign sequence

Full plan: `PATIENT_UI_REDESIGN_PLAN.md`.

1. Design system: tokens, shared widgets, asset pipeline.
2. Home and bottom navigation (Home, Services, Messages, Profile).
3. Services directory and the nine module screens, with explicit unavailable states for Peer Navigator, Reminders, Facilities, Videos and Audio.
4. Messages: inbox, consultation status, thread.
5. Profile and its child screens, plus the More page targets.
6. Nested flows (directory, booking, waiting room, call, notifications, auth), localization of hardcoded strings, responsive passes.
7. Regression, visual comparison, documentation updates, one signed APK.

Recommended first batch: **the design system (batch 1)**, because every later screen depends on it and it changes no behaviour. It cannot be finalised until decisions D1, D3 and D14 are made.

---

## I. Gaps, risks and decisions required

### Decisions

| # | Decision | Why it is needed | Recommendation |
|---|---|---|---|
| D1 | When the image and the v2.1 brief disagree, which wins? Affects the Home search field and the third card's label | `CLAUDE.md` says the image wins; the brief says no search field and "Book Consultation" | Follow the brief on these two points, since they are explicit product decisions, and record them as approved deviations |
| D2 | Where do the Emergency Contact banner and the Upcoming Consultation card go on Home? | Required by `CLAUDE.md` rule 2.4; absent from panel 1 | Keep both, restyled, below the service grid |
| D3 | References for Messages and Profile | None exist | Supply designs, or approve derivation from the image's design language with a screenshot review |
| D4 | Does the bottom bar stay visible on inner screens, as panels 2–8 show? | Today pushed screens cover it. Keeping it needs per-tab navigators, which touches `PatientShell` | Keep it on module landing screens; hide it in booking, chat thread and call |
| D5 | Message previews, timestamps and unread badges in the inbox | No patient conversations endpoint exists | Ship the inbox without them; plan the endpoint separately |
| D6 | Are panels 9 (GHI dashboard), 10 (Multi-channel) and 11 (Privacy) patient-app screens? | Panel 9 is a staff dashboard with a different navigation; 10 describes channels that do not exist | Exclude 9. Treat 10 and 11 as static information pages under More, showing only claims that are true |
| D7 | Panel 8: what does "Start Video Call" do, and which number is on the urgent card? | Calls are slot-booked and join-gated; image shows 11663, app uses 999 | Button opens doctor discovery. Confirm the number with the product and legal owner |
| D8 | Remove Medicine Delivery and Emergency Dispatch from Services? | The brief defines exactly nine modules | Remove |
| D9 | Facilities: distance, "Nearby" and map preview | The brief rules out GPS and an in-app map | Empty-state shell now; list with a "Get Directions" link when the backend exists |
| D10 | Topic filtering for the six education topics | Needs a feed filter and seeded categories | Separate backend task; topic cards open the full list until then |
| D11 | Help & Support destination | No screen or endpoint | Static contact page, content to be supplied |
| D12 | Profile rows "Payment Methods" and "Report an Issue" | Both are "coming soon" snackbars | Remove both while finance is deferred; grievances stay reachable from a consultation |
| D13 | Sign-in, registration and other screens with no panel | No reference | Restyle with the design language |
| D14 | Assets and fidelity tolerance | No logo or illustrations in the repo; image resolution limits measurement | Supply source artwork and a higher-resolution reference; otherwise approve marked placeholders and a ±2 dp tolerance |
| D15 | Reminder wording on the lock screen | Panel 5 and panel 11 promise no HIV status there; the brief asks for detailed wording | Decide before the Reminders feature is specified |

### Risks

| # | Risk | Basis |
|---|---|---|
| R1 | The redesign cannot make Peer Navigator, Reminders, Facilities, Videos or Audio work. They will be visual shells with unavailable states | [V] no client or backend code |
| R2 | The article topic filter and the message previews need backend work that the brief places outside the visual phase | [V] |
| R3 | About 70 hardcoded English strings sit outside localization, concentrated in auth, chat list and data rights; Swahili users see English there today | [V] |
| R4 | 175 tests render production widgets and assert on copy and widget types; careless restyling will break them, and loosening them would remove real protection | [V] |
| R5 | Four pages mix business logic with widget state (chat list, consultation status, vault, data rights) | [V] |
| R6 | Profile view and edit, grievance submission, and the emergency-banner rule have no tests | [V] |
| R7 | Appointment summaries and vault dates use the device timezone while slots and chat use EAT, which conflicts with the brief's "clinical times are labelled EAT" | [V] `home_models.dart:106-113`, `vault_models.dart:39` |
| R8 | The existing specification documents describe the old prototype and will mislead later work until updated | [V] |
| R9 | Password-reset OTP verification may be rejected client-side on Android: the session-establishment check matches any path ending `/verify-otp` and expects `device_bound: true`, which the reset endpoint does not return | [D] needs a runtime check |
| R10 | Logout or session expiry may wipe the saved language and push device id, because the token store clears the whole namespace | [D] needs a runtime check |
| R11 | `FLAG_SECURE` is off, so clinical screens can be screenshotted; panel 11 presents strong privacy claims | [V] `MainActivity.kt`; a recorded product decision |
| R12 | An Android Studio template project (`com.example.myapplication`) was created at the repository root at 18:17 on 2026-11-12: `app/`, `gradle/`, `.gradle/`, `build.gradle.kts`, `settings.gradle.kts`, `gradle.properties`, `gradlew`, `gradlew.bat`, `local.properties`. It is untracked and unrelated to the product. It was not created by this work and has not been removed | [V] `git status` |

### Not verified

- Screens that need data (booking, waiting room, call, chat thread, article reader) were not viewed on a device; their descriptions come from source.
- Row-level authorisation behind `AnyAuthenticated` endpoints.
- Whether a real payment provider is wired.
- `docs/PROGRESS.md`, `docs/CHECKPOINT.md` and `BUSINESS_LOGIC.md` were read selectively.
- Backend test logs were not reviewed; functional status above rests on source and the Flutter test run.
