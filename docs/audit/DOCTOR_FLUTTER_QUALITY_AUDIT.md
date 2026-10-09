# Doctor Flutter Quality / UX / Prototype / Offline Audit (Agent D)

Date: 2026-10-07
Scope: `apps/mobile/lib/features/doctor/*`, `apps/mobile/lib/app.dart`, `apps/mobile/lib/features/auth/auth_cubit.dart`, `apps/mobile/lib/core/widgets/doctor_widgets.dart`, cross-checked against `prototype/index.html` (`#doc-view-0` .. `#doc-view-5`, canonical — NOT the root `index.html`).

This audit is independent. It does not reference or rely on any other agent's findings. All claims below were re-derived directly from the current source and current tool output, not from `docs/DOCTOR_FLUTTER_REPLICATION_REPORT.md`, `docs/DOCTOR_APP_FINAL_SYSTEM_AUDIT.md`, or `docs/DOCTOR_PAYOUT_ARCHITECTURE.md` (those were read for orientation only, and are contradicted in places — see Finding H2).

## Environment / verification method

- **No real Android emulator or device was available.** `flutter devices` returned only `Windows (desktop)` and `Edge (web)`. `flutter emulators` reported none configured. `adb` is not on PATH (an `adb.exe` process (PID 5812) was running, but not reachable via the `flutter`/`adb` tooling used here, and no device enumerated through it). Per the task's fallback instruction, this audit is **static source/structural comparison + the existing automated widget-test suite**, not a rendered-screenshot visual comparison. No screenshot evidence exists or is claimed anywhere below.
- Backend process `hellodoctor-backend` (PID 16620) was confirmed running on the machine, but was not exercised since no device/emulator could attach to it for live network calls.
- Flutter SDK used via `C:\Users\CUBE\.codex\tools\flutter\bin` prepended to PATH, from `apps/mobile`.

### Static/test suite results (actual, current — not historical)

| Command | Result |
|---|---|
| `dart format --set-exit-if-changed lib test` | 1 file reformatted (`doctor_pages.dart`); exit 0 after. No remaining formatting diffs. |
| `flutter analyze` | **No issues found!** (0 errors, 0 warnings) |
| `flutter test` | **49/49 passed, 0 failed.** (`api_client_host_policy_test.dart`, `auth_security_test.dart`, `auth_state_test.dart`[not separately listed but discovered], `cache_store_test.dart`, `consultation_completion_test.dart`, `doctor_client_test.dart`, `doctor_cubit_test.dart`, `doctor_models_test.dart`, `doctor_widgets_test.dart`) |

This is the actual current count — do not cite any historical number (e.g. "41 passing") from the other docs; it is stale.

`doctor_widgets_test.dart` already contains genuine responsive assertions at exactly **360/390/412** logical px for: Queue action row ("Call Patient"/"Chat"/"Write Rx" never wrap/split), Schedule slot card (human-readable time, no raw ISO, no overflow), Wallet `EarningsBreakdownCard` (business wording, no forbidden strings), and Q&A (`_QnaOpenCard`, presets, no overflow). All of these passed.

**Screens with no dedicated responsive (360/390/412) widget-test coverage found:** Chat list/thread, Smart Rx scanner, Payout Method card / Payout Binding sheet, Monthly Statement card, Registration form. These were checked only by static code reading, not by an automated width sweep.

## Screen-by-screen status

| Screen | Prototype source | Flutter source | Status |
|---|---|---|---|
| Queue (benchmark) | `#doc-view-0` (line 6477) | `QueuePage` (doctor_pages.dart:31) | **No regression.** Text-wrap guard intact (`FittedBox`+`softWrap:false`+`maxLines:1` in `_QueueActionButton`); passing test confirms 360/390/412. |
| Chat | `#doc-view-4` (line 7347) | `ChatListPage`/`ChatThreadPage` (1987, 2219) | Mostly faithful; **missing canned-reply strip and 3 of 4 header quick-icons** (see M1, M2). Wired to real backend (`repo.messages`/`sendMessage`), no mock fallback found. |
| Smart Rx | `#doc-view-1` (line 6653) | `ScannerPage` (2553) | Real multi-page (1-5) capture/upload flow, no fake OCR. **Missing "Attached Lab Reports" quick action** (M3). |
| Schedule | `#doc-view-2` (line 6908) | `SchedulePage` (523) | Slot creation is genuine date+start+end (3 pickers), human-readable times confirmed. **Booked slots have no cancel path** vs. prototype's locked-cancel affordance (M5). |
| Wallet | `#doc-view-3` (line 7165) | `WalletPage` (1122) | 3-step breakdown + mandatory disclosure line present verbatim; Monthly Statement + Payout Method/Settings implemented and KES-correct. **Ledger row styling doesn't visually distinguish HELD from SETTLED** (M4). |
| Q&A | `#doc-view-5` (line 7532) | `QnaPage` (3136) | Correctly uses its own `flow-top`-style header (`_QnaFlowTop`), not the generic `DoctorPageHeader`. Matches prototype pattern. |
| Registration | Auth modal doctor section (prototype) | `RegistrationPage` (app.dart:230) | **Single flat form, not a 4-step wizard** — contradicts historical docs (H2). |
| Agora video call | N/A (prototype fakes it with a static viewfinder) | `AgoraPage` (agora_page.dart) | **Not implemented.** Real token fetch, but always ends on a static "Live video is temporarily unavailable" placeholder; `agora_rtc_engine` is not even a pubspec dependency (H1). |

## Findings

### HIGH severity

**H1 — Agora RTC video is not implemented at all.**
`apps/mobile/pubspec.yaml` has no `agora_rtc_engine` dependency. `AgoraPage` (`apps/mobile/lib/features/doctor/agora_page.dart`) fetches a real RTC token from the backend (`repo.rtcToken`) but then unconditionally renders a static "Live video is temporarily unavailable ... No remote video or fake call state is shown while Agora integration is deferred" screen. This is the honest thing to do (it does not fake a connected call), but it means **"Call Patient" is non-functional for actual video** anywhere in the current Doctor app (Queue, Schedule, Chat, Consultation all route to this same dead end). This is a core-feature gap, not a polish item.

**H2 — Registration is a single flat form, not the claimed 4-step wizard.**
`docs/DOCTOR_FLUTTER_REPLICATION_REPORT.md` / `docs/DOCTOR_APP_FINAL_SYSTEM_AUDIT.md` describe an Account → Professional Details → Payout Setup → Review & Submit flow. The actual current code (`RegistrationPage` / `_RegistrationState`, `apps/mobile/lib/app.dart:230-341`) is one `ListView` with 10 flat `TextField`s (name, license, phone, email, password, specialty, experience, hospital, qualifications, address) and a single "Submit for verification" button. There is:
- no step indicator,
- no Back/Continue,
- no Payout Setup step at all (no masked payout destination preview is ever shown during registration — payout is only ever configured later from the Wallet tab),
- no per-field client-side validation or inline error messages (only one whole-form `AuthFailure` string shown at the very bottom after a failed submit; empty-field/experience-range/email-format checks exist in `AuthCubit.registerDoctor` but surface as one generic message, not per-field).
The end state ("Registration submitted … pending ClinicalAdmin verification") is implemented and correct. **The historical docs do not describe the current implementation** — note this explicitly in `docs/PROGRESS.md` reconciliation, since CLAUDE.md prohibits phantom completions.

### MEDIUM severity

**M1 — Chat composer is missing the prototype's entire "canned clinical response" strip.**
Prototype `.doc-canned-strip` (line ~7504) offers 5 one-tap pills ("Take after meals", "Check fever", "View Lab Report", "Patient History", "Snap/Send Rx") directly above the chat input. `ChatThreadPage` (doctor_pages.dart:2219) has no equivalent — doctors must type every message by hand. This is a real workflow-speed regression for a clinical messaging tool, not just a cosmetic gap.

**M2 — Chat header quick-actions reduced from 4 to 1.**
Prototype `.chat-actions-top` (line ~7470) exposes Patient History, Lab Report, Video Call, and Snap/Send Rx icons directly from the thread header. `_ChatTopBar` (doctor_pages.dart:2377) only has the History icon. (Video call and Rx are reachable from other screens, just not one tap from an open chat thread, as the prototype intends.)

**M3 — Smart Rx is missing the "Attached Lab Reports" quick action.**
Prototype's Rx screen header (line ~6668) has both "Patient Case History" and "Attached Lab Reports" buttons. `ScannerPage` only ports the History one (doctor_pages.dart:2731-2757).

**M4 — Ledger row styling can look more settled than it is.**
`_LedgerCard` (doctor_pages.dart:1851) always renders the net amount in success-green with the label "Final Earning," regardless of `payment_status`. Only the small `StatusChip` badge (correctly, per `settlementBadge`/its passing unit test) distinguishes `PAYMENT_HELD` ("Held", amber) from `SETTLED_TO_DOCTOR` ("Settled ✓", green). A doctor skimming the right-hand numbers past the badge could read a `PAYMENT_HELD` row as already-final money. Recommend the amount color/weight also key off settlement status, not just the badge.

**M5 — Booked schedule slots have no cancel affordance at all.**
Prototype booked-slot cards (line ~7013-7024) show a 3rd action button that is either a real cancel or a visibly `🔒 Locked (<1h)` disabled state. `_ScheduleSlotCard` (doctor_pages.dart:913) for `booked` slots only renders Call Patient / Chat / Write Rx — no cancel/locked indicator exists for booked slots in the current Flutter UI (only fully `available` slots get a Cancel button). Confirm whether doctor-side cancellation of a booked slot is intentionally out of scope; if so, this is fine, but it is a functional reduction from the prototype as currently built.

**M6 — No proactive session-expiry re-check on app resume.**
No `WidgetsBindingObserver`/`AppLifecycleState` listener exists anywhere in `apps/mobile/lib`. Session-expiry detection is entirely reactive: `ApiClient.onSessionExpired` (wired in `AuthCubit`, `auth_cubit.dart:44-48`) only fires on the *next* outgoing API call that gets a refresh failure. Architecturally this is sound — when it fires, the entire `SignedIn` subtree (including `DoctorCubit` and its cache) is torn down via `BlocProvider`'s normal disposal and the UI returns to `LoginPage` — but a doctor who backgrounds the app, has their session genuinely expire server-side, and returns to the foreground will still briefly see the last cached Queue/Schedule/Wallet/Chat-list screen (correctly labeled "Offline — showing saved data" only if connectivity is actually down, otherwise unlabeled cached content) until something triggers a network call. Patient History is unaffected by this gap since it is never cached (see "Good" list). Recommend a lifecycle observer that force-validates the session on resume.

### LOW severity

**L1 — `TextEditingController` leak in the "Complete without Rx" dialog.**
`_ConsultationState.chooseCompletion()` (doctor_pages.dart:3508) does `final reason = TextEditingController();` as a local variable inside the method and never calls `reason.dispose()` after the dialog closes. Minor, repeatable leak (one undisposed controller per "Complete without Rx" selection). Every other controller in the audited files (`ChatThreadPage.input`, `ScannerPage.notes`, `_QnaOpenCardState.controller`, `ProfilePage.name`, all 10 of `RegistrationPage`'s controllers) is correctly disposed.

**L2 — Dead code.** `PageHeader` (core/widgets/doctor_widgets.dart:36) appears unused in the audited doctor files (superseded by `DoctorPageHeader`). Not a defect, just worth a cleanup pass.

## Confirmed compliant (no defect — stated for completeness since the brief asked to verify, not assume)

- **Queue regression check:** passes; text-wrap guard (`FittedBox` + `softWrap:false` + `maxLines:1`) intact at 360/390/412, confirmed by a passing test that specifically seeds a long name + long complaint.
- **Schedule slot creation:** genuinely date + explicit start-time picker + explicit end-time picker (`createSlot`, doctor_pages.dart:667), never a single duration dropdown. All displayed times go through `_timeRange`/`_shortTime`, producing e.g. `"10:30 AM – 10:40 AM (10m)"`; a passing test explicitly asserts no raw `T00:00:00` ISO string ever renders.
- **Wallet 20% rule:** `EarningsBreakdownCard` renders the exact 3 steps (Gross → 20% Withheld → Net) and the exact mandated sentence "Total calculation is based including the platform charge 20%" verbatim (doctor_pages.dart:1595-1606), with a passing test asserting it.
- **Monthly Statement:** month prev/next selector works (`shiftStatementMonth`, clamped to not go past the current month), "Estimated Payout" vs "Final Payable" keyed off `status != 'FINALIZED'`, payout status and masked destination shown only when the server actually returns `payout_destination` (never fabricated client-side).
- **Payout Method / Settings:** only ever shows `masked_destination`/`display_label`, never a raw identifier; `DEV_PAYOUT_BINDING` dev-simulation path is unambiguously labeled ("Connect M-Pesa (Dev Simulation)", "DEV MODE: M-Pesa destination connected (simulated)", "Development simulation only. No provider request or payout state is changed."); no literal "withdraw"/"cash out" action exists anywhere (grepped, and asserted by a passing test).
- **No obsolete prototype wording survives:** grepped the entire `apps/mobile/lib` tree for `bKash|Nagad|BDT|৳|escrow|withdraw|cash.?out` — the only hit is a code *comment* documenting why "escrow" wording is deliberately excluded. KES formatting confirmed throughout (`currency ?? 'KES'`).
- **`PAYMENT_HELD` is never shown as settled/escrow** (`settlementBadge`, doctor_pages.dart:1090, plus a dedicated unit test).
- **Business Rule 6 (4 clinical outcomes):** `chooseCompletion()` offers Escalate / Refer / Complete without Rx / Complete with Rx, matching CLAUDE.md exactly; a dedicated test names this rule and passes.
- **KMPDC (not BMDC) used consistently** in `_DoctorProfileBar`, `_QnaFlowTop`, `_RxPadFooter` — correct Kenya-domain adaptation.
- **Bottom nav order** (`DoctorBottomNav`, app.dart:389) is exactly Queue, Chat, Scan Rx, Schedule, Wallet — matches canonical order.
- **Smart Rx multi-image flow is real:** `_maxRxPages = 5` hard cap enforced on camera/gallery/file-picker add paths; per-page thumbnail strip with index badge, independent uploading/done/failed states, per-page retry and remove (disabled mid-upload); dispatch re-attempts only not-yet-done pages; no fabricated OCR/extracted-text field exists anywhere in the capture flow.
- **Patient History PHI handling exceeds the minimum bar:** `DoctorRepository.history()`/`historyDetail()` (doctor_repository.dart:239-253) call `requireOnline()` and **never cache** — there is no `cacheWrite` call for history data at all, so there is no stale-PHI-after-expiry risk because nothing is ever retained offline in the first place. `summary()` caches only an explicit allow-listed subset of fields (`_safeSummary`, line 452) that excludes chief complaint/clinical notes, with the comment "Clinical intake requires fresh authorization and is never an offline grant."
- **Loading states:** every audited screen (Queue, Schedule, Wallet, Chat list/thread, Q&A, Consultation, History) shows a real `CircularProgressIndicator` when no cache exists yet, never a blank screen; cache-first render + `refreshing`/`CacheStatusBar`'s `LinearProgressIndicator` covers the "cache available, background refresh" case; every load path's `finally` block resets its loading flag, so no permanently-stuck spinner was found.
- **Offline indicator wording is exact and consistent:** `CacheStatusBar` renders "Offline — showing saved data · Last updated HH:MM" (or without the time suffix when none is known) uniformly via a single shared widget used by Queue/Schedule/Wallet/Chat/Q&A/Consultation.
- **No mutation is ever presented as successful without server confirmation:** chat send shows "NOT SENT — reconnect and tap send again." on failure and does not optimistically clear the input; Rx dispatch tracks true per-page done/failed and shows "NOT SENT — internet connection required." when offline; slot create/cancel and Q&A publish all surface real errors (Q&A publish additionally reloads state on a 409 rather than silently claiming success).
- **No cubit/state duplication:** a single `DoctorCubit` is created once per signed-in session (`app.dart:80-83`) and torn down automatically (with its `ConnectivityService` subscription cancelled in `close()`) on sign-out/session-expiry; `_refreshInFlight` guards `loadQueue`/`loadAll` against duplicate concurrent requests.
- **No cross-context state leaks found:** `ChatThreadPage` is a freshly-pushed route per chat (new State per chat, nothing shared); `_QnaOpenCard` list items are built with `key: ValueKey(q.id)`, preventing a draft-answer `TextEditingController` from bleeding across different questions when the list re-renders.

## Notable but out-of-scope-for-severity observation

`DoctorShell` (app.dart:358-387) renders `pages[tab]` directly with no `IndexedStack`/`PageStorageBucket`. Switching bottom-nav tabs fully unmounts the previous tab's `State` and mounts a fresh one. Concretely: a doctor who has captured 1-4 undispatched Smart Rx pages and switches to another tab (e.g. to check Schedule) and back will find the capture queue silently emptied, with no "discard draft?" warning. This is not a correctness bug (no data is falsely reported as sent) but is a real doctor-facing paper-cut given Smart Rx is a multi-step capture workflow.

## Summary verdict

Core business-rule compliance (20% wallet math + disclosure line, KES/M-Pesa wording, KMPDC licensing, 4 clinical outcomes, no star ratings anywhere in these files, multi-image Rx cap, honest settlement-status labeling, PHI-never-cached history) is **solid and test-backed**. The two high-severity items are feature-completeness gaps (no real video calling yet; registration is a flat form, not the wizard other docs claim exists) rather than regressions in already-built features. Medium items are mostly prototype-parity gaps in Chat/Smart Rx quick actions and one schedule-cancellation gap. Static analysis and the full test suite are clean (0 analyzer issues, 49/49 tests passing) at the time of this audit.
