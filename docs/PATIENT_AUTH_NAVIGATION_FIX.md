# Patient OTP navigation and local backend recovery — 2026-10-15

## Patient redirect defect

The email OTP page was pushed onto the same MaterialApp Navigator as the
signed-out entry page. Successful verification saved tokens and emitted
`SignedIn`, correctly constructing the signed-in home underneath, but changing
MaterialApp's `home` did not remove the OTP route. Restarting appeared to fix
the issue because it created a fresh Navigator and restored the saved session.

The root MaterialApp now has a key based on the signed-in authentication
boundary. Successful verification replaces the signed-out Navigator, immediately
displaying the existing profile-completion gate or main menu. Logout/session
expiry replaces the protected Navigator as well. Busy/error/challenge states
retain the signed-out Navigator, so failed OTP verification stays on the form.
Ordinary signed-in rebuilds and language changes do not reset navigation.

The real-app widget test also exposed use of RepositoryProvider for the
ChangeNotifier-based locale controller. It now uses ChangeNotifierProvider,
with the already-resolved provider package declared directly. Home service
badges receive flexible width to fix the observed 412dp overflow. The bottom
menu respects Android system-navigation insets after a physical-device
screenshot exposed overlapping system buttons; success tests assert that inset.

An optional API factory at the composition root lets tests exercise the actual
application, repositories, secure storage, cubits, OTP route and main menu with
a controlled transport. It is unused in the normal application build. No API,
payment, authorization, schema or backend code was changed.

## Verification

- Four new real-app widget cases: registration success, login success,
  registration rejection and login rejection.
- Successful cases assert persisted tokens, visible PatientShell/bottom
  navigation, absence of the OTP/entry routes, and no route back to OTP.
- Both success cases also open a protected detail route, log out, and assert
  the route is removed and tokens cleared.
- Rejection cases assert no access token, no PatientShell and no protected
  Patient API request.
- Registration retains exact challenge/code and all four document/version
  submission assertions.
- Formatting check: exit 0. Analysis: no issues, exit 0.
- Full Patient suite: **185 passed, 0 failed, 0 skipped**, exit 0; previously 181.
- Existing test files/assertions remain unchanged; hashes recorded alongside
  [test results](audit_evidence/patient_auth_navigation/test_results.json).

Widget/API transport evidence does not claim a fresh physical-device
registration. The update is installed on the connected Moto g96 5G (Android
16), preserving its saved Patient test account. The app remains alive after
launch and renders the main menu. See [device/build observations](audit_evidence/patient_auth_navigation/device_build_results.json)
and [screenshot](audit_evidence/patient_auth_navigation/patient_updated_device.png).

Updated development APK: [Patient 1.0.1+4](../releases/hellodoctor-patient-arm64-auth-navigation-fix-20261015.apk).

## Backend recovery

No HTTP process was listening on 8080, while the existing PostgreSQL and Redis
containers were healthy. The existing backend binary was started as a hidden,
independent local process, retaining the current configuration and migrations.

Both `http://127.0.0.1:8080/healthz` and
`http://192.168.0.105:8080/healthz` returned HTTP 200, database ready and migrations
current. A readiness request originating from the connected phone also returned
HTTP 200; see [phone-to-backend check](audit_evidence/patient_auth_navigation/phone_backend_health.txt).
Doctor password authentication and development MFA verification both
returned HTTP 200 and issued a session; credentials/tokens were not published
in logs. Existing rate limits remain enabled.

This local test backend must be running for mobile login; it is not a hosted
service and does not automatically start when this computer reboots. If needed,
start the existing PostgreSQL/Redis containers and run from `services/backend`:

```powershell
.\target\debug\hellodoctor-backend.exe
```

The phone must be able to reach `192.168.0.105:8080` on the local network.
