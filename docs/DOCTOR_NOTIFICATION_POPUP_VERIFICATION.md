# Doctor upcoming-consultation notification — 2026-10-15

**2026-10-16 follow-up:** Doctor 1.0.1+6 retains both reminder receivers and release reflection rules; its compiled APK/Room constructor and signature are checked. The user requested continuing without physical verification. No reminder pop-up is claimed from that build. See [latest correction artifacts](audit_evidence/existing_features_device_e2e/correction_release_artifacts.json).

Implementation and release build are ready. Physical pop-up verification is pending: after the user reconnected the Moto g96 5G, Windows detected the phone, but ADB briefly reported `offline` and then no debugging device. The updated APK has not been installed and no new pop-up screenshot is claimed.

## Existing reminder contract

The backend scheduler runs every 60 seconds. A CONFIRMED or WAITING consultation becomes eligible during the 15 minutes before its scheduled start. It inserts a deduplicated `CONSULTATION_REMINDER_15M` event with the privacy-safe title **Consultation starting soon** and body **Your consultation starts in 15 minutes.** The reminder does not require video/Agora; the device demonstration will use a real test CHAT consultation.

The foreground Doctor client polls every 20 seconds. Its first successful fetch establishes a baseline rather than replaying historical notifications. Subsequent new events use the high-importance `CONSULTATION_REMINDERS` Android channel. Separately, the existing schedule reconciler creates local, inexact alarms from authorized consultations. Background work and inexact alarms are best-effort; this is not guaranteed instantaneous push delivery.

## Fix

- Added the scheduled-notification and boot/update receivers required by the installed `flutter_local_notifications` plugin to the Doctor Android manifest.
- Added release rules preserving the plugin's reflection-dependent models and Gson TypeToken signatures. Existing WorkManager/Room startup rules remain in place.
- Added one platform-channel regression test for initialization, all six channels, and exact reminder title/body/payload/high priority. No existing test assertions were changed by this notification fix.

## Observed checks

Doctor formatting and analysis pass. The full Doctor test run has **190 passed, 0 failed, 0 skipped**, up from 189. Release assembly completed successfully. Inspection of the compiled APK manifest confirms both receivers, notification and reboot permissions, and version **1.0.1+5**. The compiled Room public-constructor check also passes.

APK: [Doctor ARM64 notification fix](../releases/hellodoctor-doctor-arm64-notifications-fix-20261015.apk), 20,456,616 bytes. Hash, compiled-manifest evidence and parsed test results: [notification_fix_build.json](audit_evidence/existing_features_device_e2e/notification_fix_build.json). Build, analysis and machine test logs are in the same evidence directory.

Before disconnection, the device notification permission was granted, Do Not Disturb was off, and the consultation-reminder channel already had HIGH importance. These observations do not prove delivery after the release fix.

## Pending physical demonstration

1. Restore an authorized ADB connection and install the APK in place, preserving app data.
2. Sign in to Doctor, allow its initial notification baseline to load, and keep the test phone available.
3. Book a real near-term CHAT consultation for the synthetic Patient and Doctor through existing APIs. Use only the existing signed development-payment callback; no real-money success is implied.
4. Allow the existing reminder scheduler/client or reconciled alarm to deliver it. Capture the actual heads-up banner and verify the Doctor notification inbox/tray against the same appointment.

No notification rows or fake system notifications have been inserted for this demonstration. No new reminder-test booking was created during the disconnected attempt. Video/Agora implementation remains DEFERRED and outside this task.

| Status | Result |
| --- | --- |
| DOCTOR_NOTIFICATION_RELEASE_FIX_IMPLEMENTED | YES |
| DOCTOR_NOTIFICATION_AUTOMATED_REGRESSION | 190/190 PASS |
| DOCTOR_NOTIFICATION_FIX_INSTALLED_ON_DEVICE | PENDING_ADB_CONNECTION |
| DOCTOR_UPCOMING_CONSULTATION_POPUP_PHYSICALLY_VERIFIED | NOT_VERIFIED |
