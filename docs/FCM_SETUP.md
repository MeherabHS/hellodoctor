# Firebase Cloud Messaging setup

Stage D keeps FCM credentials and native application configuration outside the
repository. The Rust API accepts only server-side FCM HTTP v1 service-account
values (`FCM_PROJECT_ID`, `FCM_CLIENT_EMAIL`, and `FCM_PRIVATE_KEY_B64`). The
private key is never sent to either Flutter application.

## Android

Create a Firebase Android app for each package (`com.example.hellodoctor_mobile`
and `com.hellodoctor.patient`). Download each `google-services.json` into its
matching `android/app/` directory through the deployment secret/artifact
process. Do not commit those files. Apply the Google Services Gradle plugin in
the Android build only in the deployment variant that supplies the file.

## iOS and APNs

Create matching Firebase iOS apps and install the generated
`GoogleService-Info.plist` into each Runner target through the protected iOS
build configuration. Enable the Push Notifications and Background Modes
(Remote notifications) capabilities, upload the APNs authentication key or
certificate in Firebase Console, and verify the bundle identifiers match the
Flutter targets. These files and signing credentials stay outside Git.

## Runtime behavior

The apps request notification permission without blocking authentication. FCM
foreground and opened-app callbacks only refresh the authenticated in-app
inbox; they do not authorize deep links or change appointment, consultation or
financial state. When native Firebase configuration is absent, the existing
in-app notification polling continues and no fake device token is registered.
