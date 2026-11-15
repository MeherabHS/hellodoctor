# Doctor UI — Component Map

Date: 2026-11-13. What can be restyled without touching logic, and what must not be touched.

## 1. Presentation components (safe to replace)

| Component | File | Used by | Redesign action |
|---|---|---|---|
| `DoctorTokens` | `core/theme/tokens.dart` | everything | Extend with the board palette. Preliminary samples from the board: primary button about `#017A4F`–`#048157`, greeting card `#DAEDE9`, screen background near `#FAFCFD`, caption band `#E0EFF9`. The board is small and anti-aliased; re-sample per screen before fixing values |
| `DoctorCard` | `core/widgets/doctor_widgets.dart` | most screens | Keep API; restyle border, radius, shadow |
| `StatusChip` | same | queue, schedule, chat | Restyle to the "Upcoming" pill |
| `DoctorPageHeader` | same | tab screens | Replace with back-arrow title bar (panels 3–11) and the logo bar (panel 2) |
| `NotificationBellAction` | same | headers | Keep; move into the Home logo bar |
| `ErrorPanel`, `InlineError`, `CacheStatusBar` | same | most screens | Restyle only. Keep offline and last-updated behaviour |
| `DoctorBottomNav` | `app.dart` | shell | Rebuild as four tabs. `DoctorShell.switchTab` and the screen names sent to `ClientErrorReporter` are index-based and must be updated together |
| `_SplashPage`, `LoginPage`, `RegistrationPage` | `app.dart` | signed out | Rebuild layout around the same `AuthCubit` calls |
| `AppointmentCard` | `doctor_pages.dart` | queue, schedule | Rebuild layout; keep `ConsultationActions` inside |
| `_DoctorProfileBar`, `_QnaBanner`, `_QueueFilter`, `_ScheduleSummaryBanner`, `_DateSlider`, `_ScheduleSlotCard`, `_ConvCard`, `_ChatTopBar`, `_ChatExpiryBar`, `_RxMetaBar`, `_RxPagesStrip`, `_RxPadFooter`, `_SettingsTile`, `_NotificationTile` | `doctor_pages.dart`, settings, notifications | their screens | Restyle or replace |
| `ClinicalMessageBubble` | `clinical_message_bubble.dart` | chat | Restyle |
| `ProfilePhotoAvatar` | `profile_photo_avatar.dart` | profile | Keep logic; restyle frame |
| `_InCallView`, `_CallControlButton`, `_CenteredStatus` | `agora_page.dart` | call | Restyle chrome only |

## 2. Logic that must be preserved unchanged

| Area | Where | Why it matters |
|---|---|---|
| Session restore, sign-in, emailed code, logout, password reset | `features/auth/auth_cubit.dart` | Account security; development auto-code path is gated on build flags |
| Token refresh, device proof, host policy, redirect ban | `core/network/api_client.dart`, `device_proof.dart` | Transport security |
| Verification gate | `app.dart` `_ShellState.build` | Unverified doctors must not reach clinical screens |
| Join gating and labels | `AppointmentDto.joinBlocked`, `joinLockedLabel`, `canStart`, `isQueueActionable`; `core/video/join_eligibility.dart` | Server join window |
| Call lifecycle | `core/video/call_controller.dart`, `agora_engine.dart`, `call_screen_guard.dart`, `mint_failure.dart` | Agora join, reconnect, single call screen |
| Queue refresh | `ScreenRefresh`, `_ShellState` notification listener | 30-second and on-notification refresh |
| Slot create, cancel, block | `_ScheduleState` methods | Overlap errors and partial bulk results are reported truthfully |
| Rx page upload and temporary-file cleanup | `_ScannerState`, `cleanupPickedRxFile` | PHI residue on disk |
| Document opening | `DocumentButton._loadVerifiedBytes`, viewers | Mime cross-check; never an external app |
| Chat send, pagination, attachment, trace acknowledgements | `_ChatThreadState`, repository | Audit trail |
| Notification routing | `features/notifications/*`, `core/notifications/*` | Ownership-checked navigation, background delivery |
| Offline cache | `core/cache/doctor_cache_store.dart`, `DoctorRepository` cache calls | Account-scoped, cleared on sign-out |
| Earnings presentation | `EarningsBreakdownCard` and the 20% note; `DeferredWalletNotice` | Business rule 2.2; deferred-finance mode |
| Error reporting | `ClientErrorReporter`, startup incident queue | Field diagnostics |

## 3. New components the board needs

Logo header; greeting card; counter tile; quick-action row; segmented tabs (Today / Upcoming / Completed; Overview / History / Files; Summary / Visits / Results / Files; notification filters); patient header card; labelled info row with leading icon; full-width primary button with leading icon; waiting-room illustration block; in-call tab strip; form field with required marker and dropdown; search field; verified badge; settings row with icon tile; notification row with icon tile and unread dot; role-choice card and peer form (Batch F).

These should be built as each screen is built, not as a separate design-system phase.

## 4. Assets

Present: `assets/fonts/PlusJakartaSans.ttf` only. To add: logo (reuse the Patient app's), login scene, waiting-room illustration, review and success marks. See the discovery report, section 10.

## 5. Tests that pin presentation

`doctor_widgets_test.dart` (600 lines), `doctor_navigation_safe_area_test.dart` (tab labels and safe area), `notification_center_page_test.dart` and `qna_rendering_test.dart` (goldens), `consultation_completion_test.dart`, `pre_e2e_queue_chat_test.dart`. Expect label and golden updates in every batch; logic tests (cubit, repository, call controller, join window, auth security) must stay green without edits.
