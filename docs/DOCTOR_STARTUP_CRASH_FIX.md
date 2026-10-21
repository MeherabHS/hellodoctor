# Doctor Android launch crash — 2026-10-15

The installed release crashed before Flutter could render. This was reproduced
on the connected Moto g96 5G, which reports Android 16 / ARM64. The application
was version 1.0.1+2, package `com.example.hellodoctor_mobile`.

## Cause and fix

AndroidX Startup initializes WorkManager's Room database before the Activity
starts. Logcat reported `Unable to get provider
androidx.startup.InitializationProvider` and `Failed to create an instance of
class androidx.work.impl.WorkDatabase.canonicalName`.

The original R8 usage report explicitly lists the removed public no-argument
constructor of `androidx.work.impl.WorkDatabase_Impl`. Room 2.5's consumer rule
preserves database class names without preserving all reflectively used members.
The release artifact check independently failed on the original APK because
the constructor was absent from compiled DEX.

Doctor Android now loads an application ProGuard file that preserves the Room
base class, database subclass public no-argument constructors, and the
WorkManager Flutter background worker's reflective constructor/class name.
Release shrinking and the Vulkan/Impeller renderer remain enabled. No backend,
authentication, business rule, test assertion or app data was changed.

## Observed release/device checks

- Release build exited 0; version **1.0.1+3**.
- Compiled DEX constructor check: original APK **FAIL**, corrected APK **PASS**.
- Android 16 KB ZIP alignment check exited 0.
- `adb install -r` succeeded; application data was preserved.
- Three force-stop/cold-start attempts each remained alive and resumed after
  five seconds, without a native fatal exception in that process's log.
- Actual physical-device screenshot shows the Doctor login form rendering.
- `flutter analyze`: no issues, exit 0.
- `flutter test --machine`: **180 passed, 0 failed, 0 skipped**, exit 0.
- No existing tests or assertions were changed for this crash fix.

This verifies the reported immediate launch crash. It does not claim full
physical-device acceptance of login, consultations, payment, media calls or
background notifications.

## Artifacts and evidence

- [Updated Doctor APK](../releases/hellodoctor-doctor-arm64-startup-fix-20261015.apk)
- [Physical-device screenshot](audit_evidence/doctor_startup_crash/doctor_launch_fixed.png)
- [Cold-start results](audit_evidence/doctor_startup_crash/device_launch_results.json)
- [Original crash trace](audit_evidence/doctor_startup_crash/before_crash.log)
- [Original R8 removed-members report](audit_evidence/doctor_startup_crash/before_r8_usage.txt)
- [Compiled release constructor check](audit_evidence/doctor_startup_crash/check_release_constructor.py)

Recheck any future Doctor release with:

```powershell
python docs/audit_evidence/doctor_startup_crash/check_release_constructor.py apps/mobile/build/app/outputs/apk/release/app-release.apk
```

The APK remains a development-signed local-backend build using
`http://192.168.0.105:8080/api/v1`, with the existing development OTP setting.

APK size: 20,456,432 bytes. SHA-256:
`0b4a5aced2dadaec77d39a1654fc89a333a0a7cce9a8d495bf2f0b043c8271ed`.
