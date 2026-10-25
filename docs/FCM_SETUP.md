# Firebase Cloud Messaging setup

Stage D keeps FCM credentials and native application configuration outside the
repository. The Rust API accepts only server-side FCM HTTP v1 service-account
values (`FCM_PROJECT_ID`, `FCM_CLIENT_EMAIL`, and `FCM_PRIVATE_KEY_B64`). The
private key is never sent to either Flutter application.

The Gradle/Dart/client-side wiring described below (Milestone 0) is complete
and committed. What remains is entirely manual Firebase Console work plus
dropping the resulting files into place — none of it can be done from inside
this repository, since it requires real Firebase project credentials that must
not be invented or committed.

## What's already wired (do not redo)

- `android/settings.gradle.kts` (both apps) registers the Google Services
  Gradle plugin in the version catalog (`apply false`).
- `android/app/build.gradle.kts` (both apps) applies that plugin **only when
  `android/app/google-services.json` actually exists** on disk
  (`if (file("google-services.json").exists()) apply(plugin = "com.google.gms.google-services")`).
  This means: the build keeps succeeding today with zero Firebase
  configuration, and starts packaging real Firebase config the moment the
  file is dropped in — no further Gradle edits needed.
- `AndroidManifest.xml` (both apps) declares
  `com.google.firebase.messaging.default_notification_channel_id` =
  `GENERAL`, so an FCM message the OS auto-displays while the app is
  backgrounded/terminated lands on a channel that already exists (created at
  app startup by `LocalNotificationService.init()`), instead of being
  silently dropped by Android 8+'s channel requirement.
- `core/notifications/push_notification_service.dart` (both apps) already
  calls `Firebase.initializeApp()`, requests notification permission,
  registers the device token, listens for token refresh and re-registers,
  and bridges `onMessage`/`onMessageOpenedApp`/`getInitialMessage()` to an
  authenticated inbox refresh. A `Firebase.initializeApp()` failure (e.g. this
  file still being absent) is now reported through the existing sanitized
  client-diagnostics pipeline (`ClientErrorReporter`, operation `push_init`)
  instead of being silently swallowed — see `apps/*/test/push_notification_service_test.dart`.
- Device-token revocation (`NotificationRepository.revokeDevice`) is now
  called on logout in both apps, best-effort, so a signed-out device stops
  receiving push for that account.
- No `onBackgroundMessage` handler exists, deliberately: this app only ever
  sends a `notification` block (title/body) in its FCM payloads (see
  `services/backend/src/services/notification_transport.rs`), never a
  silent/data-only push, so the OS already displays it automatically while
  backgrounded or terminated with no app code involved. Adding a background
  isolate entry point would be unused background processing for a
  notification shape this system doesn't send.

## Android

Create a Firebase Android app for each package (`com.example.hellodoctor_mobile`
and `com.hellodoctor.patient` — these are each app's real, already-checked-in
`applicationId`, confirmed in `android/app/build.gradle.kts`). Download each
project's `google-services.json` into its matching `android/app/` directory
through the deployment secret/artifact process. **Do not commit those files**
— both apps' `.gitignore` already excludes them by exact path.

No further Gradle edits are needed: the plugin activates automatically once
the file is present (see above).

## iOS and APNs

Create matching Firebase iOS apps (bundle identifiers must match the Flutter
targets — confirm the real value in each app's `ios/Runner.xcodeproj` before
creating the Firebase app, rather than assuming one). Install the generated
`GoogleService-Info.plist` into each `ios/Runner/` directory through the
protected iOS build configuration (also `.gitignore`d by exact path).

Unlike Android's Gradle plugin, placing the file alone is **not** sufficient
on iOS — it must also be added to the Xcode project so it's copied into the
app bundle:

1. Open `ios/Runner.xcworkspace` (after `pod install`) in Xcode.
2. Drag `GoogleService-Info.plist` into the `Runner` group in the Project
   Navigator.
3. In the dialog, check **"Copy items if needed"** and ensure the `Runner`
   target's checkbox is ticked under **"Add to targets."**
4. Verify in the file inspector afterward that **Target Membership: Runner**
   is checked — this is the actual requirement; a file merely present on disk
   but not added to this build phase will not be bundled and Firebase will
   fail to initialize at runtime exactly as if the file were absent.

Then, in Firebase Console and the associated Apple Developer account:

5. Enable the **Push Notifications** and **Background Modes → Remote
   notifications** capabilities on the `Runner` target's signing &
   capabilities tab.
6. Generate an APNs authentication key (or certificate) in the Apple Developer
   portal and upload it under Firebase Console → Project Settings → Cloud
   Messaging → Apple app configuration.

These files and signing credentials stay outside Git, same as Android's.

## Verifying the setup actually works

`Firebase.initializeApp()` uses no explicit `FirebaseOptions` (no
`firebase_options.dart`/FlutterFire-CLI step is required here) — it reads the
native config file for the current platform automatically, which is why
dropping in the two files above is sufficient without any further Dart code
change. After placing the files:

1. Run `flutter pub get` (not strictly required for this, but harmless) and
   build/run the app on a real device or emulator with Google Play services
   (physical APNs testing requires a real iOS device, not the Simulator).
2. Confirm the app doesn't throw a native Firebase configuration error at
   startup and that `FirebaseMessaging.instance.getToken()` returns a real
   token (not null/empty) — this is what `register_device` in
   `notification_handlers.rs` persists.
3. Confirm the Rust backend reports `push_provider: "FCM_HTTP_V1"` (not
   `"NOOP_DEVELOPMENT"`) in the `register_device` response once its own
   `FCM_PROJECT_ID`/`FCM_CLIENT_EMAIL`/`FCM_PRIVATE_KEY_B64` are configured.
4. Send a real consultation/booking event and confirm the notification
   arrives both while the app is foregrounded (via the in-app bridge) and
   backgrounded/terminated (via the OS auto-display described above).

A successful automated Flutter test run is **not** evidence that real push
delivery works — the tests in this repository exercise the dedup/bridging
logic and the sanitized-diagnostics capture path only, never a real device
token or a real Firebase project (none exist in this repository). Final
acceptance requires a configured Firebase project and the on-device steps
above.

## Runtime behavior

The apps request notification permission without blocking authentication. FCM
foreground and opened-app callbacks only refresh the authenticated in-app
inbox; they do not authorize deep links or change appointment, consultation or
financial state. When native Firebase configuration is absent, the existing
in-app notification polling continues and no fake device token is registered.
