# Patient App — Remediation Gap Map

Date: 2026-11-13. Maps every finding in `PATIENT_POST_REBUILD_GAP_ANALYSIS.md` (4 P0, 12 P1, 25 P2) against the code as it stands on branch `fix/p0-consultation-repair` (HEAD `6213cef`), and maps the remediation brief's 9 phases and 15 requirements onto it.

**How this was checked:** by reading source and the commit messages for `23f245e`…`6213cef`. No tests, builds or emulator runs were performed for this map. "FIXED" therefore means *changed in code, with the tests named in the commit*, not *re-verified on a device*.

Status labels:

- **FIXED** — code changed and covered by named tests; not re-verified at runtime for this map.
- **OPEN** — unchanged since the gap analysis; work needed.
- **BLOCKED** — cannot be closed without something outside the code (environment, credentials, second device).
- **NEEDS DECISION** — waiting for a product or security decision.
- **DEFERRED** — moved to a separate milestone on purpose.

---

## 1. Summary

| Severity | Total | FIXED | OPEN | BLOCKED / NEEDS DECISION | DEFERRED |
|---|---|---|---|---|---|
| P0 | 4 | 2 | 1 | — | 1 |
| P1 | 12 | 6 | 3 | 3 | — |
| P2 | 25 | 7 | 15 | 3 | — |
| **All** | **41** | **15** | **19** | **6** | **1** |

P2-18 is partly fixed and is counted as OPEN.

One P1 (D-10 fees) is counted as FIXED because the brief replaced it: zero-fee doctors are bookable, and fee-setting is no longer required.

**Still not proven anywhere:** a real two-device Patient ↔ Doctor video and chat consultation, push delivery, password recovery outside development, and Admin Web doctor approval.

---

## 2. P0

| ID | Finding | Status | Evidence | Remaining |
|---|---|---|---|---|
| P0-1 (D-01) | Intermittent sign-out after sign-in | **FIXED** | `23f245e`: root cause confirmed on the emulator. `flutter_secure_storage` kept a non-default file name, so token reads returned null. Every store now names its file; regression test fails if any store omits it. `43f94fd`: a slow refresh at startup no longer signs the patient out | Confirm on a device during the final gate (a few real sign-ins, not a scripted loop) |
| P0-2 | Two-party video never verified | **OPEN** | No two-client run recorded since the report | Run a Patient ↔ Doctor call: remote media both ways, reconnection, audio-only fallback, completion seen by the patient. Needs a second client with a camera |
| P0-3 | Peer Navigator has no backend | **DEFERRED** | Separate milestone per the brief | UI must not claim peer services exist (see P2-5) |
| P0-4 (D-09) | Deferred bookings never get a chat conversation | **FIXED** | `d201eb4`: `ensure_consultation_activated` runs on deferred booking, webhook and `GET /conversation` repair; tests for chat exchange, unrelated accounts, idempotency. `financial_module_deferment` 16/16 | Runtime chat send/receive between the two apps not yet observed |

## 3. P1

| ID | Finding | Status | Evidence | Remaining |
|---|---|---|---|---|
| P1-1 (D-04) | Notification settings never loads | **FIXED** | `66e6b45`: the page no longer reads `NotificationCubit` outside its provider scope (`notification_settings_page.dart:23`) | Open it once on the emulator |
| P1-2 (D-02) | Password recovery | **FIXED in code (2026-11-13); awaiting a staging check** — the hard guard was removed and only the "Azure email must be configured" check remains (`config.rs`, test `password_reset_in_staging_or_production_requires_azure_email`). The Patient app now shows "Password reset isn't available right now…" (EN/SW) for a 404 (`PasswordResetUnavailable`). Backend `password_reset` integration tests 15/15; Patient `flutter test` 486/486. Original finding below. Was: **NEEDS DECISION** + **OPEN** | Not about the Azure key. In staging/production, `validate_password_reset_policy` refuses `PASSWORD_RESET_ENABLED` unconditionally, before it looks at email configuration (`config.rs:1123-1127`). A unit test pins this: "production/staging must stay disabled regardless of email config" (`config.rs:1341-1343`). Setting the flag in production fails startup validation (`config.rs:855-866`). Production already requires Azure email to be configured (`config.rs:1091`), and login codes already go through it. Client still shows the generic `ApiException` message for a 404 (`auth_cubit.dart:293-294`): "This record is no longer available." | Decision: remove the unconditional staging/production refusal and keep the "email must be configured" check. Then verify one real reset email on staging. Open now: honest client message for "reset not available" |
| P1-3 (D-03) | Home card ignores server join data | **FIXED** | `66e6b45`: shown only inside the server window from T−15, server clock, 30 s tick, reloads on resume, reconnect and return to Home. `consultation_routing_test` covers T−16 and T−15 for both modalities, plus clock skew | Observe at T−15 on the emulator |
| P1-4 (D-05) | Reuse detection + rotation on every resume can sign users out after a network drop | **NEEDS DECISION** | Unchanged. No grace window (`auth_service.rs:1064`); refresh still runs on every resume (`app.dart:89-94`) | Security decision: a grace window for the previous token, or fewer rotations on resume. Then a network-drop test |
| P1-5 | Redesign uncommitted | **FIXED** | Committed in `5c7bafe`, `9966e1a`, `a2df897` | Screenshot folders are still untracked. Uncommitted Doctor-app work sits on this branch (see section 6) |
| P1-6 (D-06) | Chat booking not reachable from Book Consultation | **FIXED** | `66e6b45`: the landing offers video, chat and "My consultations" (`patient_modules.dart:84-122`) | — |
| P1-7 (V-04) | Profile chips overflow at text scale 1.3 | **OPEN** | `family_profile_chip_row.dart` unchanged since the redesign: fixed 92 px row, 84 px chips, two-line label (`:29-31`, `:69`, `:87`) | Let the row height follow text scale; add an overflow test at 1.3 |
| P1-8 | Push delivery unverified | **BLOCKED** | FCM transport is a no-op locally | Needs a staging or test environment with `FCM_*` configured; test delivery and tap routing (cold start, background, foreground) |
| P1-9 | Hardcoded English / Swahili quality | **OPEN** | 44 `Text('…')` literals across 16 files (was 53/19). Worst: `auth_entry_page.dart` (12), `email_otp_verify_page.dart` (5). `AuthFailure` messages in `auth_cubit.dart` and the whole `data_rights_page.dart` are also English-only | Move to ARB; native Swahili review (needs a reviewer) |
| P1-10 (D-10) | No way to set doctor fees | **FIXED (by redefinition)** | `d201eb4`: in deferred mode zero-fee verified doctors are listed for video and chat (`doctor_handlers.rs:44-50`). `66e6b45`: the patient app shows no price | No fee feature, per the brief. Enabled mode still hides unpriced doctors on purpose |
| P1-11 (D-11) | Completion and history unreachable afterwards | **FIXED** | `66e6b45`: `consultation_history_page.dart`; `appointment_destination.dart` resolves push, cold-start and inbox taps from the latest server state. `efcff09`: the appointment detail carries the routing fields | See a completed consultation via notification and history on a device |
| P1-12 | Admin Web doctor approval never exercised | **OPEN** | The fixture doctor was approved by a database update | A local admin account exists (`admin@hellodoctor.health`); run one approval through Admin Web |

## 4. P2

| ID | Finding | Status | Evidence / where |
|---|---|---|---|
| P2-1 (V-01) | Registration header draws under the status bar | **OPEN** | No change since the report |
| P2-2 (V-02) | Keyboard hides the Reset button | **OPEN** | `forgot_password_page.dart` unchanged since the redesign |
| P2-3 (V-05/V-06) | Swahili labels break mid-word; Home tiles truncate | **OPEN** | Chip label `maxLines: 2` + ellipsis (`family_profile_chip_row.dart:87`); tiles unchanged |
| P2-4 (V-07/V-08) | Module screens highlight Services; More has no bottom bar | **OPEN** | `AfyaModuleScaffold` defaults to `selectedTab = 1` (`afya_module_widgets.dart:17`); `MorePage` is a plain Scaffold (`patient_modules.dart:264`) |
| P2-5 (V-10) | Peer screen lists services that don't exist | **OPEN** | Four feature cards are still rendered as offered (`module_screens.dart:341-364`) |
| P2-6 (V-11) | Screens not yet redesigned; raw "ACCESS" enum | **OPEN** | `data_rights_page.dart:69-77, 98` shows enum values verbatim, in English only |
| P2-7 (D-07) | Videos & Quizzes opens on the empty Videos tab | **OPEN** | `openEducation(c, nav, initialTab: 1)` (`patient_modules.dart:167`) |
| P2-8 (D-08a) | An auth error carries over to the next screen | **OPEN** | No state reset on navigation in `AuthCubit` |
| P2-9 (D-08b) | Consents can be ticked without being opened | **NEEDS DECISION** | Product/compliance |
| P2-10 (D-08c) | Back on a non-Home tab exits the app | **FIXED** | `PopScope` on `PatientShell` (`lib/app.dart`): Back on Services, Messages or Profile returns to Home; Back on Home leaves the app. Test in `auth_navigation_test.dart` |
| P2-11 (D-08d) | Bell's accessibility label is "Notification settings" | **OPEN** | `tooltip: l10n.notificationSettingsMenuTitle` (`patient_home_page.dart:239`) |
| P2-12 (D-08e) | T-15 text always says "15 minutes" | **FIXED** | `f54a269` |
| P2-13 | `GET /doctors/:id` exposes unverified doctors | **OPEN** | `find_public` reads `doctor_public_directory` with no status filter (`doctor_repo.rs:100-108`). The view does not filter on verification (`20261202000001…up.sql:28-35`) |
| P2-14 | 10-minute slot length not enforced | **NEEDS DECISION** | No duration check in slot creation. The brief asks for "approved scheduling rules"; the rule itself must be confirmed first |
| P2-15 | Chat list has no preview, time or unread count | **OPEN** | Backend contract; `chat_repository.dart` has no message fields |
| P2-16 | `imageCache` not cleared at sign-out | **OPEN** | No `imageCache` reference in `lib/`. Now more relevant: chat images were added in `6213cef` |
| P2-17 | No test for cross-patient chat **read** | **FIXED** | Messages: `financial_module_deferment.rs:624-657`. Attachments: `api_tests.rs:18776-18792` (another patient and another doctor get 404) |
| P2-18 | Stale doc comment; CLAUDE.md listed Reminders as "coming soon" | **Partly FIXED** | CLAUDE.md no longer lists Reminders. The comment at `upcoming_consultation_section.dart:36-39` still says the card moved off Home |
| P2-19 | Android Studio template untracked at the repository root | **NEEDS DECISION** | Still present (`app/`, `gradle/`, `gradlew*`, …) |
| P2-20 | Waiting room says patients cannot place calls | **FIXED** | `66e6b45` |
| P2-21 | Booking page says "Doctor calls you directly" | **FIXED** | `66e6b45` |
| P2-22 | Chat sheet shows "Paid… KES 500" | **FIXED** | `66e6b45`: no price anywhere in the patient app |
| P2-23 | "About Dr. Dr"; "Verified clinician" twice | **FIXED** | `66e6b45` fixes "Dr. Dr". The duplicate badge is not mentioned; check on screen |
| P2-24 | Inbox times shown in the device time zone | **FIXED** | `66e6b45`: inbox uses EAT |
| P2-25 | Completion accepted though the doctor never joined | **OPEN** | Product question. Not addressed; the uncommitted `consultation_service.rs` change only adds a lock against Rx uploads |

---

## 5. The brief's phases against the current state

| Phase | Work | State |
|---|---|---|
| 1 | Sign-out and refresh reliability | Done in code (P0-1). D-05 network-drop path waits on a security decision |
| 2 | Chat conversation under deferred payments; no payment UI | Done in code (P0-4, `66e6b45`) |
| 3 | Real two-device video and chat | **Not started** (P0-2) |
| 4 | Completed consultations from notifications; history | Done in code (P1-11) |
| 5 | 15-minute Home card; notification → waiting room | Done in code (P1-3, P1-11) |
| 6 | Zero-fee doctors bookable without a fee feature | Done in code for deferred mode (P1-10) |
| 7 | Remaining P1 | Open: settings runtime check, password recovery, accessibility (V-04), localization, push, admin approval |
| 8 | All P2 | 7 fixed, 18 open or waiting on a decision (section 4) |
| 9 | Full regression and release readiness | Not started |

## 6. The brief's 15 requirements against the current state

| # | Requirement | Covered? |
|---|---|---|
| 1 | Reassess all 41 findings against the latest code | **This document** (by code reading; runtime re-checks still needed) |
| 2 | Fix remaining defects; reliability and security first | Partly. Open security items: P2-13, P2-16, D-05 |
| 3 | No fees or payment reintroduced | Holding: `66e6b45` removed every price and payment surface |
| 4 | No dependency on doctors setting fees | Holding in deferred mode (`doctor_handlers.rs:49-50`) |
| 5 | Settings, history, lifecycle, responsive layouts, language, navigation | Settings, history and lifecycle fixed. Responsive layouts (V-04/05/06), language (P1-9) and navigation (V-07/08, D-08c, D-07) open |
| 6 | Password recovery via verified email delivery | A hard-coded guard blocks it in staging/production even with Azure configured (P1-2). Needs approval to lift the guard, then one real reset email on staging |
| 7 | FCM delivery and taps | **Blocked** on an FCM-configured environment |
| 8 | UI and accessibility at standard and large text | Open (V-04, V-06, D-08d) |
| 9 | Authorization tests incl. cross-patient chat and attachments | **Covered** (P2-17) |
| 10 | 10-minute slot policy | Needs the rule confirmed (P2-14) |
| 11 | Keep the design, artwork, workflows and four tabs | Holding |
| 12 | Peer UI not presented as functional | Mostly. V-10 still lists peer services |
| 13 | Each finding marked FIXED / VERIFIED / BLOCKED / DEFERRED | This document; the gap report itself is not yet annotated |
| 14 | Tests, analyzer and emulator after each batch | Commits report green suites (Patient 484/484 at `6213cef`). Not re-run for this map |
| 15 | Final Patient ↔ Doctor integration test | Not done |

## 7. Other things to know

- **`checkpoint.md` and `progress.md` are out of date.** Both still say "no fixes have been started". Neither records commits `23f245e`–`6213cef`.
- **Features added after the gap analysis have no runtime verification yet:** reminder alarms with sound (`4f2fead`), uploads before a consultation and Health Records grouped by consultation (`c61213a`), and image attachments in chat (`6213cef`). They need a place in the final test gate.
- **Uncommitted Doctor-app work is on this branch** (`doctor_pages.dart` +1,395/−505, the Rx dispatch lock in `prescription_service.rs`, `call_close_out_page.dart`, new tests). It is outside the Patient scope but shares the branch.

## 8. Suggested order for what's left

1. **Quick fixes in code:** P1-7, P2-3, P2-4, P2-7, P2-8, P2-10, P2-11, P2-16, P2-18 comment, P2-6 enum labels, the P1-2 client message, P2-5 peer copy, P2-13 (backend status filter + test).
2. **Localization sweep (P1-9),** then queue the native Swahili review.
3. **Decisions needed:** D-05 token grace window, slot-length rule, consent-must-open (D-08b), completion without the doctor joining (P2-25), the Android template.
4. **Environment-dependent checks:** two-device consultation (P0-2), Admin Web approval (P1-12), FCM (P1-8), password-reset email (P1-2).
5. **Final gate:** full Flutter and backend suites, emulator pass at 360 dp, Swahili and text scale 1.3, and a Patient ↔ Doctor integration run. Then update `checkpoint.md`, `progress.md` and the gap report.
