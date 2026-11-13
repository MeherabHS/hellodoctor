# Doctor UI — Android Emulator Workflow

Date: 2026-11-13. Scope: `apps/mobile`. No release APK is built in this workflow.

## 1. Environment (checked)

| Item | Value |
|---|---|
| Flutter / Dart | 3.47.6 stable / 3.13.5, at `C:\Users\CUBE\.codex\tools\flutter` |
| Android SDK | `C:\Users\CUBE\AppData\Local\Android\Sdk`, platform android-37, build-tools 36.0.0, emulator 37.2.13 |
| `flutter doctor` | Flutter, Windows, connected device, network: pass. Android toolchain: warning. Chrome and Visual Studio missing (not needed) |
| AVD | `Pixel_9`, running as `emulator-5554`, 1080 × 2424 at 420 dpi (about 411 dp wide), Android 17 |
| Doctor app id | `com.example.hellodoctor_mobile` (Patient is `com.hellodoctor.patient`; both can be installed together) |
| Local backend | Listening on port 8080; Postgres and Redis containers up |
| Local data | 2 patients, 1 admin, **0 doctors, 0 slots, 0 appointments** |
| `adb` | Not on `PATH`; use `…\Android\Sdk\platform-tools\adb.exe` |

## 2. Two ways to run

**A. Design preview (default for UI work).** Opens the signed-in screens directly with labelled sample data; no login, no code, no approval, no backend.

```
cd apps/mobile
flutter run --no-pub -d emulator-5554 -t lib/dev/design_preview_main.dart
```

Add `--dart-define=PREVIEW_VERIFICATION=PENDING` (or `REJECTED`) to see the approval gate. Reads return fixtures, unknown reads return "not available", every write is refused, and a "PREVIEW DATA" ribbon is drawn. It refuses to start in a release build. Fixtures live in that one file; extend them there as screens are built.

**B. Real app against the local backend** (for behaviour checks, when a doctor account exists):

```
flutter run --no-pub -d emulator-5554 --dart-define=ENVIRONMENT=development --dart-define=ALLOW_INSECURE_HTTP=true --dart-define=DEV_AUTH_OTP_ENABLED=true --dart-define=API_BASE_URL=http://10.0.2.2:8080/api/v1
```

Signed-in screens this way need a doctor registered and then verified by an admin, plus slots and a patient booking. None of that was created during discovery.

## 3. Capturing

```
adb exec-out screencap -p > docs/doctor-ui/screenshots/<set>/<name>.png
```

Use hot reload (`r`) and hot restart (`R`) in an attached `flutter run`. Android's notification permission prompt can be pre-granted with `adb shell pm grant com.example.hellodoctor_mobile android.permission.POST_NOTIFICATIONS`.

## 4. Baseline captured on 2026-11-13

In `screenshots/baseline/`. 01–04 are the real app signed out; 05 and 10–22 are the design preview.

| File | Screen |
|---|---|
| 01-login | Login, password step |
| 02-forgot-password | Forgot password |
| 03-register-top, 04-register-bottom | Doctor registration form |
| 05-pending-approval | Verification gate (pending) |
| 10-queue | Queue tab |
| 11-chat-list | Chat tab |
| 12-scan-rx | Scan Rx tab |
| 13-schedule, 14-schedule-slots | Schedule tab |
| 15-wallet-deferred | Wallet tab, finance deferred |
| 16-settings | Settings tab |
| 17-notifications | Notification centre |
| 18-consultation-history | Consultation history |
| 19-patient-qna | Patient Q&A sheet |
| 20-chat-thread | Chat thread |
| 21-profile | Profile |
| 22-security | Security |

Not captured: emailed-code step, reset code, new password, reset success, legal agreements, registration submitted, rejected gate, video call states, patient history sheet, encounter detail, notification settings, article list and editor, payout sheet, document viewers. Video call needs camera permission and a real Agora token; the others can be added through the preview when their batch starts.

## 5. Per-batch loop

1. Crop the board panel for the screen; measure against the 9:41 status-bar frame; sample colours.
2. Build the screen; hot reload in the preview.
3. Capture at 411 dp; place beside the panel for approval.
4. Also capture at 360 dp, in Swahili, and at text scale 1.3 (`adb shell wm size 945x2121` or a second AVD for 360 dp; `adb shell settings put system font_scale 1.3`; reset both afterwards).
5. `flutter analyze --no-pub` and `flutter test --no-pub`. On this Windows machine ten golden tests already fail from platform font drift (see the discovery report); judge goldens on CI or regenerate deliberately when a screen changes.
6. Behaviour check for anything touching sign-in, joining, slots, Rx or completion: by tests, and in the real app when fixtures exist.
