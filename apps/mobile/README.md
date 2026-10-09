# HelloDoctor Doctor app

Use only the repository's existing Flutter/Android tooling. No patient app is present here.

Development LAN builds require explicit defines:

```
flutter run --no-pub --dart-define=ENVIRONMENT=development --dart-define=ALLOW_INSECURE_HTTP=true --dart-define=DEV_AUTH_OTP_ENABLED=true --dart-define=API_BASE_URL=http://YOUR_PRIVATE_LAN_IP:8080/api/v1
```

Use synthetic data only. The backend must also explicitly enable development OTP. Production/staging require their explicit environment and an HTTPS API_BASE_URL; omit all development flags. API redirects are disabled. Build/reinstall after source updates; an existing phone installation is not updated by source tests.

Doctor Articles are under Profile & Settings ? My Articles. Draft/save/publish/edit/archive belong to the authenticated Doctor, with no Admin review. See [article contract](../../docs/DOCTOR_ARTICLES.md) and [public exposure requirements](../../docs/PUBLIC_EXPOSURE_HARDENING.md).

Validation: `flutter analyze --no-pub` and `flutter test --no-pub` use existing dependencies. This pass does not install anything or automatically install an APK on a device.
