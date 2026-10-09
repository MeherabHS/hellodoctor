> Kenya correction, 2026-10-08: [current professional-registration contract](KENYA_PROFESSIONAL_VERIFICATION.md) supersedes legacy regulator/status terminology below. Original dated/prototype evidence is retained as history. The user confirms the Doctor app is verified.

# Doctor Flutter Final Verification Report

This is an honest, evidence-gated verification pass against the 46-phase audit
brief. It states plainly what was actually run and what could not be, rather
than inferring "works" from code that merely compiles. Two environment
constraints limited scope and are called out explicitly rather than papered
over; everything else was driven against real code and, for the backend, a
real running stack.

## A. Environment

- Flutter: 3.47.6 (stable), Dart 3.13.5 — SDK found at
  `C:\Users\CUBE\.codex\tools\flutter` (not on `PATH`; invoked by prepending it
  per-command). Not installed anywhere else on this machine.
- `flutter doctor` results: Windows toolchain present; **no Android SDK**
  despite `ANDROID_HOME` pointing at a path that does not exist on disk; **no
  Chrome**; **no Visual Studio C++ workload**. Result: `flutter devices` only
  lists `windows` (desktop) and `edge` (web), and neither can actually be
  *built* without Visual Studio / a working Chrome install. **No emulator, no
  physical device, no buildable running target was available in this
  environment.**
- `apps/mobile` is untracked in git (never committed), so there is no commit
  hash for the Flutter side; changes in this pass are working-tree edits only.
- Backend commit: `e3739ae5` (2026-10-03).
- Database/migrations: `hellodoctor-postgres` (healthy) and `hellodoctor-redis`
  (healthy) Docker containers, already running at session start. Migrations
  current as of `20261005000002_history_access_window`.
- A second AI coding session (Codex CLI) was found actively editing the same
  `apps/mobile` files concurrently partway through this pass. Work was paused
  and re-synced with the user before continuing; the concurrent changes found
  on resume (an improved `AuthCubit.restore()` offline/online distinction, a
  `SessionRestoring` regression test, a new `cache_store_test.dart`) were
  inspected and are sound, so they were kept rather than reverted.

**Consequence for scope:** Phases that require an actual running app on a
device/emulator/browser (runtime walkthrough, visual replication against
`index.html`, screenshot/background exposure, small/large-screen layout,
app start/resume/terminate lifecycle) **could not be executed** and are marked
`NOT_TESTED` below, not silently assumed to pass. Everything achievable via
static code review, the Dart/Flutter toolchain (format/analyze/test), and the
real backend stack (Postgres+Redis+Rust, driven with `cargo test`/curl) was
actually executed.

## B. Test Results

**Flutter** (`apps/mobile`, after fixes in this pass):
- `dart format --output=none --set-exit-if-changed .`: clean (0 files would
  change) at the end of this pass.
- `flutter analyze`: **No issues found.** (Started this pass at 1 compile
  **error** + 46 info/warning issues — see Findings.)
- `flutter test`: **28/28 passed**, 0 failed, 0 skipped (7 test files; one new
  file added this pass, one new test added to an existing file).

**Backend** (`services/backend`, re-run fresh in this pass, not assumed from
prior docs):
- `cargo fmt --check`: clean.
- `cargo clippy --all-targets -- -D warnings`: clean.
- `cargo test --lib`: **14/14 passed**.
- `cargo test --test api_tests -- --test-threads=1`: **40/40 passed**.

These numbers match `docs/CHECKPOINT.md`'s claimed baseline, confirmed by
actually re-running the suite in this session rather than trusting the
document.

## C. Real Backend Matrix

The backend's own integration suite already exercises the Doctor contract
end-to-end (auth, MFA, duty, slots, history access window, Q&A concurrency,
wallet, webhooks, RLS/ABAC/BOLA) against a real Postgres+Redis, and that suite
was re-run clean in this pass (see B). What is reported below is specifically
*the Flutter client's* real-backend status, which is a different question than
"does the backend work."

| Feature | Status | Notes |
|---|---|---|
| AUTH (login request shape) | `VERIFIED_BY_CODE_REVIEW` | `AuthCubit.login` posts to `/auth/doctor/login` exactly per contract; see D for why live curl failed |
| AUTH (live request/response) | `BLOCKED` | See Critical-path note below: the long-running dev backend process currently 500s on **every** login (doctor and admin) |
| MFA | `VERIFIED_BY_CODE_REVIEW` | Posts `session_token`+`otp_code` to `/auth/doctor/verify-otp`, matches contract |
| REFRESH | `VERIFIED_BY_CODE_REVIEW` | Single-flight `refreshSession()`, correct 401-retry-once, no refresh storm (traced by hand, see Findings) |
| LOGOUT | `VERIFIED_BY_CODE_REVIEW` | Clears tokens+cache unconditionally in `finally`, even if the server call fails offline |
| QUEUE | `VERIFIED_BY_CODE_REVIEW` | `GET /doctor/appointments`, matches mapping doc |
| DUTY | `VERIFIED_BY_CODE_REVIEW` | `GET`/`PUT /doctor/duty`, `requireOnline()` blocks offline mutation |
| SCHEDULE | `VERIFIED_BY_CODE_REVIEW` | create/cancel both `requireOnline()`-guarded; 409 surfaces via `ApiException.userMessage` |
| CONSULTATION DETAIL | `VERIFIED_BY_CODE_REVIEW` | `GET /doctor/consultations/{id}/summary`, server-driven, no fabricated fields |
| PATIENT HISTORY | `VERIFIED_BY_CODE_REVIEW` (backend window logic independently `VERIFIED_REAL_BACKEND` via `cargo test`) | Client calls the contextual-appointment endpoint only, never a bare patient id |
| HISTORICAL DETAIL | `VERIFIED_BY_CODE_REVIEW` | Uses `c.mounted` correctly already (pre-existing, no fix needed) |
| CHAT | `VERIFIED_BY_CODE_REVIEW` | HTTP only, matches "no WebSocket" constraint |
| Q&A | `VERIFIED_BY_CODE_REVIEW` (409 handling), backend atomicity `VERIFIED_REAL_BACKEND` | Client maps 409 to "already answered", not generic error — confirmed in code and via backend's `ON CONFLICT DO NOTHING` + test |
| SMART RX / UPLOAD | `VERIFIED_BY_CODE_REVIEW` | Single-file only (no multi-image cap applies — that CLAUDE.md rule is patient-intake-scoped, not doctor Rx) |
| DOCUMENT ACCESS | `VERIFIED_BY_CODE_REVIEW` | Fetches a server-issued URL per request; not persisted, not constructed client-side |
| COMPLETION | `VERIFIED_BY_CODE_REVIEW` | Double-tap guarded by `completing` flag; 409 maps to "already completed" |
| WALLET | `VERIFIED_BY_CODE_REVIEW` | 20%-withheld disclosure text is server-sourced and always rendered (see Findings) |
| PROFILE | `VERIFIED_BY_CODE_REVIEW` + backend `VERIFIED_REAL_BACKEND` | Server uses `#[serde(deny_unknown_fields)]` with only `full_name` — mass assignment is backend-enforced, not just UI-hidden |
| AGORA | `DEFERRED_AGORA_INTEGRATION` | Mints a real token via the real endpoint, shows an honest "temporarily unavailable" state, no fake call UI |
| EXTERNAL PAYMENT | `DEFERRED_EXTERNAL_PAYMENT_INTEGRATION` | No payment/payout UI in Doctor app; wallet is read-only |

**`VERIFIED_BY_CODE_REVIEW`** here means: the Dio/repository code was read
line-by-line and traced against the real backend handler/contract (also read),
including error paths — not "it looks plausible." It is **not** the same as
`VERIFIED_REAL_BACKEND` (an actual client request observed to succeed), which
the brief requires and which this environment could not fully produce for the
Flutter app specifically.

**Why live client-to-server curl testing stopped**: the long-running dev
backend process (PID 31588, started earlier, not by this session) returns
`500 INTERNAL_SERVER_ERROR` on **both** doctor and admin login. Postgres logs
show `password authentication failed for user "hellodoctor_auth_worker"` and
`"hellodoctor_system_worker"` — consistent with a stale process whose
in-memory DB role passwords no longer match Postgres's (most likely: Postgres
roles were re-bootstrapped by a different, newer backend start since this one
began). `/healthz` still returns 200 because it doesn't need those pools. This
is an **environment/process staleness issue, not a code defect** — restarting
the backend process would almost certainly resolve it, since `cargo test`
(which bootstraps its own role logins fresh every run) passes 40/40 against
the same migrations. The harness's safety layer blocked this session from
stopping that process ("Interfere With Workloads"), since it cannot
distinguish a stale broken process from another session's live work. This
needs the user (or a fresh terminal) to restart
`cargo run`/`target/debug/hellodoctor-backend.exe` before further live
curl/device verification can happen.

## D. Visual Replication

`NOT_TESTED`. No buildable device/emulator/browser target existed in this
environment (see Environment). The prototype's Doctor screens and the Flutter
`doctor_pages.dart` layout were compared by reading both sources, which caught
the mojibake/em-dash defect below, but this is not a substitute for an
on-screen comparison and is not reported as one.

## E. Offline/Cache

Verified by code review and by the existing + new automated tests
(`cache_store_test.dart`, `auth_security_test.dart`):
- Cache is scoped per authenticated identity (fingerprint of the access/refresh
  token), schema-versioned, and stored only via `flutter_secure_storage`
  (never `SharedPreferences`/Hive/sqflite — confirmed by grep, zero hits).
- Corrupt JSON, wrong schema version, and scope mismatches are all discarded
  safely (tested) rather than crashing or leaking.
- Logout and account-expiry (`onSessionExpired`) both call `cache.clear()`
  unconditionally; a cross-doctor cache-isolation test (`doctor-a` cannot read
  `doctor-b`'s cached wallet) passes.
- `restore()` now correctly distinguishes "server rejected the session" (401/
  403 → clear cache, sign out) from "could not reach the server at all"
  (→ stay signed in, rely on the existing per-screen offline/cache UI) — this
  was a concurrent fix found on resume (see Environment) and independently
  verified sound by tracing `ApiClient._rotate()`'s error paths.

What was **not** tested: actually toggling OS-level airplane mode against a
running app (requires a device), so this is code-level verification of the
cache/offline logic, not an observed runtime offline/online transition.

## F. Security Findings

| ID | Severity | Description | Repro | Root Cause | Fix | Test | Status |
|---|---|---|---|---|---|---|---|
| SEC-1 | LOW | Doctor Flutter release build is signed with the **debug** keystore (`android/app/build.gradle.kts`: `signingConfig = signingConfigs.getByName("debug")`), and both Android `applicationId` and iOS `PRODUCT_BUNDLE_IDENTIFIER` are still the Flutter scaffold default `com.example.*`. | Inspect `android/app/build.gradle.kts` and `ios/Runner.xcodeproj/project.pbxproj`. | Never customized past `flutter create` defaults. | Not fixed — requires a real release keystore (secret, team-owned) and a deliberate product decision on the real bundle identifier; fabricating either would be worse than leaving it documented. | — | Open (pre-launch blocker, not pre-acceptance) |
| SEC-2 | INFO (confirmed non-issue) | Checked for PHI/secret logging (Dio `LogInterceptor`, `print`, `debugPrint`, `Logger`) across all of `lib/`. | `grep` across `apps/mobile/lib` | N/A | N/A — zero logging calls exist anywhere in the app, so there is nothing to leak. | — | No action needed |

No CRITICAL or HIGH security findings were identified in what this environment
could inspect (static code, and the backend's own real, passing security test
suite covering BOLA, RLS, ABAC, webhook signature/replay, DB role isolation).

## G. Functional Bugs (found and fixed this pass)

| ID | Severity | Summary | Root Cause | Fix | Test | Status |
|---|---|---|---|---|---|---|
| BUG-1 | CRITICAL (build-breaking) | `doctor_cubit.dart` referenced `ApiException` without importing it — `flutter analyze` reported a hard compile error (`type_test_with_undefined_name`), meaning the app would not build at all. | Missing `import '../../core/network/api_client.dart';` | Added the import. | Covered by `flutter analyze` passing clean + existing `doctor_cubit_test.dart`. | Fixed |
| BUG-2 | MEDIUM | Five strings in `doctor_pages.dart` rendered mojibake (`Ã¢â‚¬â€œ`/`Ã¢â‚¬â€`) instead of an en-dash/em-dash — visible garbled text on the appointment list, slot list, wallet "no value" placeholder, and chat last-activity line. | Re-encoding corruption (likely a copy/paste round-trip through the wrong encoding) baked into the source file. | Replaced with literal `\u2013`/`\u2014`. | Visual-only; confirmed no remaining non-ASCII mojibake via byte-level grep. | Fixed |
| BUG-3 | MEDIUM | Cold start with a valid saved session briefly showed the **login form** before flipping to the Doctor Shell — `AuthCubit`'s initial state was `SignedOut()`, and `restore()`'s first line awaited secure storage before emitting anything, so the very first frame rendered as signed-out. | `restore()`'s busy/token check combined `_busy` and an `await` in one expression, so nothing could be emitted synchronously before the await. | Added a `SessionRestoring` state, emitted synchronously (before any `await`) at the top of `restore()`; `app.dart` now shows a splash spinner for that state instead of the login form. | New test in `auth_state_test.dart` asserts `auth.state is SessionRestoring` immediately after calling `restore()`, before awaiting it. | Fixed |
| BUG-4 | MEDIUM | Several handlers used `BuildContext`/`context.read<DoctorCubit>()` **after** an async gap (time picker, image/file picker, dialogs) with no `mounted` guard — `createSlot`, `cancelSlot`, `pick`, `pickFile`, `answerQuestion` — risking a thrown exception (and, worse, a silently dropped mutation if "fixed" with a naive early-return) if the user navigated away mid-operation. | Missing lifecycle guards flagged by `use_build_context_synchronously`. | Captured the `DoctorCubit`/repo reference *before* the async gap so the mutation always completes even if the widget is later disposed; guarded only the subsequent UI feedback (`setState`/`showError`) with `mounted`/`c.mounted`. | Covered by `flutter analyze` (0 remaining `use_build_context_synchronously` warnings) + existing widget tests still passing. | Fixed |
| BUG-5 | MEDIUM (business-rule gap) | CLAUDE.md Business Rule 6 requires doctors be able to complete a consultation with outcome `ESCALATED` (in addition to `COMPLETED_WITH_RX`/`COMPLETED_NO_RX`/`REFERRED`). The backend's `ClinicalOutcome` enum and `/consultations/{id}/complete` handler already support it, but the Doctor app's completion dialog only offered the other three — `ESCALATED` was unreachable from the UI. | Dialog action list simply omitted it; no payload difference needed (same `{"outcome": "..."}` shape as `REFERRED`). | Added an "Escalate" button to `chooseCompletion()`'s dialog, wired to the existing `complete('ESCALATED')` path. | New widget test `consultation_completion_test.dart` asserts all four outcome buttons render. | Fixed |
| BUG-6 | LOW (unused import / lint) | `doctor_cache_store.dart` imported `flutter_secure_storage` directly but only used the `TokenStore` abstraction from `api_client.dart`. | Leftover import. | Removed. | `flutter analyze` clean. | Fixed |
| BUG-7 | LOW (doc defect) | `docs/DOCTOR_FLUTTER_BACKEND_MAPPING.md` row 13 said `PATCH /doctor/profile` accepts "name, hospital, qualifications" as editable, contradicting both the backend code (`DoctorProfileUpdate { full_name: Option<String> }` with `#[serde(deny_unknown_fields)]`) and the same document's own closing paragraph. | Stale table row, never updated after the backend's mass-assignment fix (per `CHECKPOINT.md`). | Corrected the row to say only `full_name` is editable. | N/A (doc-only). | Fixed |

## H. Deferred Items (approved, not treated as bugs)

- `DEFERRED_AGORA_INTEGRATION` — real Agora media wiring. The app correctly
  mints a real RTC token and shows an honest "temporarily unavailable" screen;
  no fake call state is rendered.
- `DEFERRED_EXTERNAL_PAYMENT_INTEGRATION` — no payment-provider UI exists in
  the Doctor app; wallet remains read-only, no withdraw/payout control found
  anywhere in `doctor_pages.dart` (confirmed by grep).
- `PATIENT_QNA_SUBMISSION_REQUIRED_FOR_PATIENT_PHASE`,
  `DOCTOR_NOTIFICATION_DELIVERY_CAPABILITY_REQUIRED` — not applicable to
  Doctor Flutter; not flagged as defects.
- Android/iOS app identifiers and release signing (SEC-1) — pre-launch
  deployment configuration, not an acceptance blocker for feature
  verification, but must be resolved before any real build is shipped.

## I. Final Open Findings

## Registration contract extension — 2026-10-06

`POST /api/v1/auth/doctor/register` is implemented and tested as a separate
Doctor-specific public route. It creates an unverified `DOCTOR` account/profile,
uses the existing Argon2 credential path, rejects unknown privileged fields,
and returns `PENDING_VERIFICATION`. Backend regression is green at 14/14
library and 41/41 API integration tests. Flutter registration validation and
pending-verification UI are covered by the Flutter suite.

```
CRITICAL_OPEN = 0
HIGH_OPEN     = 0
MEDIUM_OPEN   = 0   (4 found this pass, all fixed: BUG-2, BUG-3, BUG-4, BUG-5)
LOW_OPEN      = 1   (SEC-1: debug-signed release build + com.example.* identifiers —
                      requires real deployment secrets/decisions this session cannot supply)
```

## Final Acceptance Rule

`DOCTOR_FLUTTER_NOT_VERIFIED`

Not because of any open CRITICAL/HIGH defect — none were found, and every
confirmed in-scope defect this pass could find was fixed with a regression
test. The brief's acceptance bar explicitly requires a runtime walkthrough and
screen-by-screen visual replication review, and requires every feature to be
demonstrated `VERIFIED_REAL_BACKEND` end-to-end. This environment had no
buildable device/emulator/browser target (no Android SDK, no Chrome, no Visual
Studio) and the one live backend process available stopped answering login
requests partway through this pass for reasons unrelated to the Doctor Flutter
code (see Section C). Those two gaps mean the required evidence does not yet
exist, independent of code quality — reported honestly rather than inferring
a pass from static review and a formerly-passing backend test suite.

**To close the gap:** (1) restart the backend process (`cargo run` in
`services/backend`, or the existing `target/debug/hellodoctor-backend.exe`,
after confirming no one else is using it) and re-run live curl/API checks
against the Doctor contract; (2) install the Android SDK + an AVD (or Visual
Studio's "Desktop development with C++" workload for a Windows build target)
so the app can actually run, for the device walkthrough and visual
replication phases this report could not perform.
