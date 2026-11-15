# Patient UI — Android Emulator Workflow

Status on 2026-11-12: **emulator run, baseline partial.** At the first inspection no emulator existed (section 1 records that state). Later the same day a `Pixel_9` AVD was installed by the product owner; the app was built from source in debug, launched on it against the local backend, and four signed-out screens were captured (section 2A). The signed-in baseline was stopped at the product owner's instruction, who directed that functionality be verified from tests and backend logs instead. Sections 3–6 are the procedure for the per-batch loop.

Scope: `apps/mobile_patient` only. No release APK is built anywhere in this workflow.

---

## 1. Environment as first inspected (before the AVD was installed)

| Item | Value | Source |
|---|---|---|
| Host | Windows 11 Pro 25H2 (10.0.26200), AMD Ryzen 5 5600, 23.9 GB RAM, 55 GB free on C: | `flutter doctor -v`, CIM |
| Flutter | 3.47.6 stable (framework 5fc346839b, engine 692136cb65), at `C:\Users\CUBE\.codex\tools\flutter` | `flutter --version` |
| Dart | 3.13.5, DevTools 2.60.0 | same |
| Java | OpenJDK 21.0.10 (Android Studio bundled JBR) | `flutter doctor -v` |
| Android Studio | Installed at `C:\Program Files\Android\Android Studio` | file check |
| Android SDK | `C:\Users\CUBE\AppData\Local\Android\Sdk`; platforms 31, 33, 34, 35, 36; build-tools 28.0.3, 35.0.0, 36.0.0; platform-tools (adb) present | directory listing |
| SDK `emulator` package | **Not installed** (no `emulator\` directory) | directory listing |
| SDK system images | **None** (`system-images\` is empty) | directory listing |
| AVDs | **None** (`%USERPROFILE%\.android\avd` does not exist; `flutter emulators` reports "Unable to find any emulator sources") | `flutter emulators` |
| `sdkmanager` / `avdmanager` | **Not present** (`cmdline-tools\latest\bin\` has neither) | file check |
| SDK licences | Licence files exist, but `flutter doctor` reports "Android license status unknown" because it cannot run `sdkmanager` | `flutter doctor -v` |
| Hardware acceleration | Available: firmware virtualization enabled, hypervisor present, Windows Hypervisor Platform + Virtual Machine Platform + Hyper-V features all enabled | CIM `Win32_Processor`, `Win32_OptionalFeature` |
| Connected devices | None over adb. Flutter lists only `windows` and `edge` | `adb devices`, `flutter doctor -v` |
| Other doctor findings | Chrome and Visual Studio missing. Irrelevant to Android work | `flutter doctor -v` |

Project state:

| Item | Value |
|---|---|
| Package / applicationId | `com.hellodoctor.patient`, label "AfyaMkononi" |
| `pubspec.lock` | Tracked, present |
| Run configurations | Only the IDE default `apps/mobile_patient/.idea/runConfigurations/main_dart.xml` (plain `lib/main.dart`, no dart-defines). No `.vscode/launch.json`, no flavors |
| `integration_test/` | Does not exist |
| `google-services.json` | Present locally at `android/app/`, git-ignored |
| Local backend | Not running: nothing listens on port 8080. Docker containers `hellodoctor-postgres` (5432) and `hellodoctor-redis` (6379) are up |
| Screenshot blocking | `FLAG_SECURE` is **not** set (`MainActivity.kt` records that it was removed at the product owner's request), so `adb screencap` will capture real pixels |

---

## 2. Original blocker and minimum action (resolved for one device)

**Blocker at first inspection:** the Android SDK has no emulator binary, no system image and no virtual device. There is also no `sdkmanager`, so these cannot be installed from the command line as the SDK stands.

**Minimum action (about 10 minutes plus a 2–3 GB download), in Android Studio:**

1. *Settings → Languages & Frameworks → Android SDK → SDK Tools*: tick **Android Emulator** and **Android SDK Command-line Tools (latest)**, then Apply.
2. *SDK Platforms* tab, "Show Package Details": under Android 15 (API 35) tick **Google APIs Intel x86_64 Atom System Image**, then Apply. The CPU is AMD; x86_64 images run on it through Windows Hypervisor Platform, which is already enabled.
3. *Device Manager → Create Virtual Device*, twice:
   - `afya_compact_360`: a 360 × 800 dp class phone ("Small Phone", or a custom 720 × 1600 px, 320 dpi profile).
   - `afya_large_412`: Pixel 7 or Pixel 8 (412 dp wide).
4. Run `flutter doctor --android-licenses` once and accept.

After that, `flutter emulators` should list both devices and the rest of this document applies.

Installing SDK packages and accepting licences changes the machine and needs your consent, so I did not attempt it.

---

## 2A. What was actually run on 2026-11-12

| Item | Value |
|---|---|
| AVD | `Pixel_9` (created through Android Studio by the product owner) |
| System image | Android 17, API 37, `google_apis_playstore` x86_64; model `sdk_gphone64_x86_64` |
| Display | 1080 × 2424 px at 420 dpi, which is about 411 × 923 dp |
| Font scale | 1.0 |
| Acceleration | `emulator -accel-check`: "WHPX(10.0.26200) is installed and usable" |
| Build | `flutter run` debug from `apps/mobile_patient` at `3c09124` with the four defines in section 3.2 plus `DEV_AUTH_OTP_ENABLED=true`. First Gradle build took 834 s |
| Backend | local `cargo run` on port 8080 against the local Docker Postgres (`ENVIRONMENT=development`) |
| Result | App launched and rendered the sign-in page. No startup error |

Screenshots saved in `docs/patient-ui/screenshots/baseline/pixel_9_api37/`:

| File | Screen |
|---|---|
| `03-sign-in.png` | Sign in |
| `04-register-empty.png` | Create your account, empty |
| `04b-register-date-picker.png` | Date-of-birth picker |
| `06a-forgot-password.png` | Forgot your password |

A second pass the same evening, after the product owner signed in with a test account, added 27 signed-in captures (31 files in total). Navigation only; nothing was booked, submitted or saved.

| Files | Screens |
|---|---|
| `08-home-no-upcoming`, `08b-home-profile-switcher-sheet` | Home tab, profile switcher |
| `09-services` | Services tab |
| `10-blogs-empty` | Blogs tab, empty state |
| `12-account`, `12b-account-coming-soon-snackbar` | Account tab, "coming soon" snackbar |
| `13a`–`13d` | Profile edit sheet, photo sheet, add-dependent sheet, language modal |
| `14a-notification-settings-error`, `14b-data-rights` | Notification settings (showed "Unable to load notification settings"), data rights |
| `15-notification-center-empty` | Notification center |
| `16-doctor-directory-empty`, `16b-directory-specialty-sheet` | Doctor directory, specialty filter |
| `20-chat-list-empty` | Chat consultations list |
| `26-health-vault-empty` | Health Vault |
| `27-qna-feed-empty`, `27b-qna-ask-question` | Q&A feed, ask form |
| `*-sw` | Home, Services, Account and the language modal in Swahili |
| `*-sw-ts13` | The same at text scale 1.3 |

Observed while capturing:

- At text scale 1.3 the Family Profiles chips on the Account tab overflow ("BOTTOM OVERFLOWED BY 28 PIXELS"), visible in `12-account-sw-ts13-overflow.png`.
- Notification settings rendered its error state against the local backend. The cause was not investigated.
- The language modal stays open after a language is chosen; the change applies behind it.

Still not captured, because the local database has no doctors, articles, appointments or documents: doctor profile, booking, consultation status, waiting room, call, post-consultation, chat thread, article reader and quiz, populated lists, OTP and password-reset follow-on pages, splash and startup error. Seeding data was out of scope (no database changes). A synthetic registration (`@example.test`) was started earlier and never submitted, so this work created no account. Language and text scale were restored to English and 1.0.

Notes for the next run:

- Only one AVD exists. A compact 360 dp device is still needed for the two-width checks.
- The Play Store image offers Google autofill, which put a real email address into the registration form once. That screenshot was deleted. Use a non-Play "Google APIs" image, or turn autofill off, before capturing.
- The app source at `3c09124` is identical under `apps/mobile_patient` to commit `966e85a`, the commit the distributed test APK was built from (`git diff 966e85a HEAD -- apps/mobile_patient` is empty). The debug build therefore shows the same UI as that APK, but talks to the local backend instead of the deployed one. The APK itself was not installed.
- Typing into forms through `adb shell input` is unreliable while the on-screen keyboard is visible. For signed-in captures, sign in by hand and capture with `adb`, or add an `integration_test` screenshot harness.

---

## 3. Running the Patient app

### 3.1 The app will not start without dart-defines

`ApiClient` (`lib/core/network/api_client.dart:188-200`) calls `validateMobileDeployment(release: true, …)` whenever it builds its own Dio, including in debug. Consequences, read from the code:

- With no `ENVIRONMENT` define it throws `StateError('An explicit valid ENVIRONMENT is required.')`, and the app renders `StartupErrorPage`.
- The default base URL is `http://10.0.2.2:8080/api/v1`. Plain HTTP is accepted only when `ENVIRONMENT=development`, `ALLOW_INSECURE_HTTP=true`, and the host is loopback, `10.0.2.2` or an RFC 1918 address.
- `ALLOW_INSECURE_HTTP` and `DEV_AUTH_OTP_ENABLED` are rejected for any environment other than `development`.
- Android's `network_security_config.xml` independently allows cleartext only to `localhost`, `127.0.0.1` and `10.0.2.2` (the debug variant is broader).

So the plain IDE run configuration fails at startup by design. That is a security control. Do not weaken it.

### 3.2 Local backend (lowest-risk target for screenshots)

The emulator reaches the host's `localhost:8080` as `10.0.2.2:8080`. This keeps all traffic on the machine and away from production.

```powershell
# terminal 1: backend (Postgres and Redis containers are already running)
cd services\backend
cargo run
# after any migration change, restart and run scripts\verify_backend_schema.ps1 (CLAUDE.md §4)

# terminal 2: patient app
cd apps\mobile_patient
flutter pub get
flutter emulators --launch afya_compact_360
flutter devices
flutter run -d emulator-5554 `
  --dart-define=ENVIRONMENT=development `
  --dart-define=ALLOW_INSECURE_HTTP=true `
  --dart-define=API_BASE_URL=http://10.0.2.2:8080/api/v1
```

Whether `DEV_AUTH_OTP_ENABLED=true` is also needed depends on how the local backend delivers the email OTP; see the discovery report, section F, for what the docs say about dev OTP and seeded accounts. Credentials are not reproduced in these documents.

Use `r` for hot reload and `R` for hot restart. Dart-define values are compile-time constants, so changing one needs a full re-run.

### 3.3 Staging or production backend

Not recommended for screenshot work. A debug build pointed at an HTTPS host needs `--dart-define=ENVIRONMENT=staging` (or `production`) with no development flags. Booking, chat and grievance actions on such a build mutate real data. Use it only with an approved test account and only for the specific regression gates that need it.

---

## 4. Capturing screenshots

PowerShell's `>` redirection can corrupt binary output, so capture on the device and pull:

```powershell
$dev = "emulator-5554"
$out = "docs\patient-ui\screenshots\baseline\afya_compact_360"
New-Item -ItemType Directory -Force $out | Out-Null
adb -s $dev shell screencap -p /sdcard/shot.png
adb -s $dev pull /sdcard/shot.png "$out\01-splash.png"
adb -s $dev shell rm /sdcard/shot.png
```

Record once per device and store beside the screenshots as `device.txt`:

```powershell
adb -s $dev shell getprop ro.build.version.release   # Android version
adb -s $dev shell getprop ro.build.version.sdk       # API level
adb -s $dev shell wm size                            # physical resolution
adb -s $dev shell wm density                         # dpi; logical width = px * 160 / dpi
adb -s $dev shell settings get system font_scale     # text scaling
adb -s $dev shell getprop persist.sys.locale
```

Text-scale and locale passes:

```powershell
adb -s $dev shell settings put system font_scale 1.3   # then recapture; restore with 1.0
```

Language is switched inside the app (Profile → Language), which is the path real users take.

Naming convention: `docs/patient-ui/screenshots/<baseline|batch-N>/<avd>/<NN>-<screen>[-<state>][-sw][-ts13].png`.

No screenshot may contain a real patient's data, an access token, an OTP, or an Agora token. Use synthetic accounts only.

---

## 5. Baseline capture list

Captured on `Pixel_9`: rows 03, 04 (empty and date picker), 06 (first page only), 08 (no-upcoming state), 09, 10 (empty), 12, 13, 14, 15 (empty), 16 (empty and one filter), 20 (empty), 26 (empty), 27 (empty and ask form). Rows 01, 02, 05, 07, 11, 17–19, 21–25 and all populated states are **not captured**. Order follows the navigation tree in `PATIENT_UI_SCREEN_INVENTORY.md`.

| # | Screen | State(s) to capture |
|---|---|---|
| 01 | Splash (`_SplashPage`) | session restoring |
| 02 | Startup error (`StartupErrorPage`) | run once without dart-defines |
| 03 | Sign in (`AuthEntryPage`) | empty, validation error, busy |
| 04 | Register (`AuthEntryPage`, register mode) | empty, filled, keyboard open |
| 05 | Email OTP (`EmailOtpVerifyPage`) | empty, wrong code |
| 06 | Forgot password → reset OTP → new password → success | each page |
| 07 | Profile completion | placeholder-profile account only |
| 08 | Home tab | no upcoming, upcoming consultation, loading skeleton, error |
| 09 | Services tab | list; "coming soon" snackbar |
| 10 | Blogs tab (current third tab) | loading, list, empty search, error |
| 11 | Article reader, with quiz | article, quiz in progress, quiz result |
| 12 | Account tab | profile, family chips, loading, error |
| 13 | Profile edit sheet, photo sheet, add dependent sheet, language modal | each, keyboard open |
| 14 | Notification settings, Data rights | each |
| 15 | Notification center | list, empty |
| 16 | Doctor directory | list, filters open, empty search, error |
| 17 | Doctor profile | about tab, availability tab |
| 18 | Booking page | slot grid, summary step, error |
| 19 | Paid chat booking modal, add payment method sheet | each |
| 20 | Chat consultations list | active, past, empty, error |
| 21 | Consultation status page, pre-consult intake modal | each |
| 22 | Chat thread | populated, empty, keyboard open, locked or expired |
| 23 | Waiting room | too early (locked), countdown, join open |
| 24 | Agora call page | connecting and in-call, test accounts only |
| 25 | Post-consultation, grievance modal | each |
| 26 | Health Vault | list, each tab, empty, PDF viewer, image viewer |
| 27 | Q&A feed, ask-question page, full answer | each, empty |

Both AVDs, English at text scale 1.0, are the minimum. Home, Services, the chat list and thread, and Account are also captured in Swahili and at text scale 1.3.

---

## 6. Per-batch loop (after discovery is approved)

1. Capture "before" PNGs for the routes the batch touches.
2. Implement one batch. Iterate with hot reload.
3. Tap through every visible control and every state on the emulator.
4. Capture "after" PNGs on both AVDs. Place each beside its reference panel crop.
5. `flutter analyze` and `flutter test` from `apps/mobile_patient`.
6. Report: files changed, device and API level, before/after pairs, remaining deviations, test results.
7. Wait for approval before the next batch.

One signed APK is produced only after all batches are approved, with the same applicationId and keystore as today.
