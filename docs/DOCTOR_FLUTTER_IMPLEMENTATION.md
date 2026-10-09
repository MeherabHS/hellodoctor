# Doctor Flutter implementation

Audit date: 2026-10-05.

The client in `apps/mobile` is wired to the frozen Rust routes through Dio, secure token storage, `AuthCubit`, `DoctorCubit`, and `DoctorRepository`. The implementation uses server responses as the source of truth and contains no production mock patient or finance data.

## Completion matrix

| Area | Status | Evidence in client |
|---|---|---|
| Login, MFA, refresh, recovery, logout | IMPLEMENTED_AND_TESTED | `AuthCubit`, secure storage, one-shot Dio refresh, server logout/revocation route |
| Doctor registration | IMPLEMENTED_AND_TESTED | `/auth/doctor/register`, schema-backed fields, Argon2 password, pending/unverified response, Flutter pending-verification form |
| Queue and duty | IMPLEMENTED_AND_TESTED | `/doctor/appointments`, `/doctor/duty`, retry/empty/loading and server-confirmed duty mutation |
| Schedule | IMPLEMENTED_AND_TESTED | `/doctor/slots` list/create/delete; server 409 is surfaced |
| Consultation detail and completion | IMPLEMENTED_AND_TESTED | summary, contextual history, document metadata, outcome selection, duplicate-tap guard, queue refresh |
| Patient history | IMPLEMENTED_AND_TESTED | current-appointment history/detail routes; server access decisions and 404/403 are consumed |
| Chat | IMPLEMENTED_AND_TESTED | conversation list, messages, HTTP send, locked state, retry/error |
| Open Health Q&A | IMPLEMENTED_AND_TESTED | list/detail dialog, answer editor, 409 refresh/conflict handling |
| Smart Rx and documents | IMPLEMENTED_AND_TESTED | camera, gallery, image/PDF file selection, preview/remove, size check, progress, upload, authorized URL |
| Wallet | IMPLEMENTED_AND_TESTED | read-only summary, pending disbursement, ledger, server currency/status; no withdrawal action |
| Profile/settings | IMPLEMENTED_AND_TESTED | GET/PATCH full name only; verified fields read-only and validation/error states |
| Error/offline/timeout UX | IMPLEMENTED_AND_TESTED | safe mapping for 401/403/404/409/413/422/429/5xx, offline and timeout |
| Agora live media | DEFERRED_AGORA_INTEGRATION | RTC authorization boundary and non-faking unavailable screen; no remote video is rendered |
| External payment providers | DEFERRED_EXTERNAL_PAYMENT_INTEGRATION | no provider credentials, payout, withdrawal, refund, or fake money movement |
| Patient Q&A submission | PATIENT_QNA_SUBMISSION_REQUIRED_FOR_PATIENT_PHASE | Doctor phase intentionally excludes patient question creation |

## Verification evidence

- `dart format --set-exit-if-changed .`: PASS.
- `flutter analyze`: PASS (zero issues).
- `flutter test`: PASS, 27 tests.
- `flutter run -d edge`: PASS; app attached to the debug service and exited cleanly.
- `flutter devices`: Windows and Edge detected; no Android/iOS runtime.
- `flutter build apk --debug`: BLOCKED because no Android SDK is installed.
- PostgreSQL, Redis, and the Rust HTTP service were subsequently started and exercised; see the live backend verification update below.

The final gate remains `DOCTOR_FLUTTER_NOT_READY` only for the unavailable Android/emulator offline-reconnect validation. The only intentionally deferred product areas are `DEFERRED_AGORA_INTEGRATION` and `DEFERRED_EXTERNAL_PAYMENT_INTEGRATION`.

## Cache/offline/loading UX pass (2026-10-05)

Status: `IMPLEMENTED_AND_TESTED` at client level. `DoctorCacheStore` is the single structured cache. It stores versioned envelopes (`schema_version`, Doctor scope, timestamps, optional TTL) through the existing `flutter_secure_storage` token store, so cached application data is protected at rest and isolated per authenticated Doctor session. Cache keys are bounded: queue 50, schedule 100, conversations 50, chat threads 20 x 50 messages, Q&A 50, ledger 100, plus wallet, duty, profile, and consultation summaries.

Queue, schedule, chats, Q&A, wallet, duty, profile, and consultation summaries use stale-while-revalidate: cached data renders immediately, a non-blocking refresh runs, and `CacheStatusBar` displays refresh/offline/last-updated state. Pull-to-refresh is available on list destinations. A centralized `ConnectivityService` supplies UX hints; request results remain authoritative. Corrupt, incompatible, expired, or over-limit cache entries are discarded safely.

Offline mutations are blocked with an explicit network-required message. Completion, schedule changes, duty updates, chat sends, Q&A answers, profile saves, document/prescription uploads, history authorization, and logout security remain backend-confirmed. Chat text remains a local `NOT SENT` draft after failure; Q&A answer text is never rendered as submitted. Patient longitudinal history is never persisted as an offline authorization: history and document retrieval require an online backend check. Signed URLs, temporary credentials, document binaries, and wallet calculations are not cached. Logout, revocation, and account-scope changes clear protected cache.

Cache/offline tests cover cached-first refresh, offline startup, schema/corrupt data, expiry, scope clearing, offline mutation rejection, and cache state transitions. `dart format --set-exit-if-changed .`, `flutter analyze`, and `flutter test` PASS (27 tests). Real device/emulator offline/reconnect evidence remains pending because this environment has no Android SDK/device and no running backend services; therefore this pass is `IMPLEMENTED_AND_TESTED`, not `IMPLEMENTED_REAL_BACKEND_VERIFIED`.

## Live backend verification update (2026-10-05)

Docker Desktop was started, `services/backend/docker-compose.yml` brought PostgreSQL 16 and Redis 7 to healthy state, and the Rust service was launched with `SEED_DEV_DATA=true` on port 8080. A real Doctor login and MFA exchange succeeded for the seeded account. Authenticated requests returned 200 for queue, duty, schedule, chats, Q&A, wallet summary, wallet ledger, and profile. Duty was toggled and restored through PUT, a schedule slot was created and cancelled, refresh rotation succeeded, logout succeeded, and the rotated refresh token was rejected after logout. These routes are now `IMPLEMENTED_REAL_BACKEND_VERIFIED` for this run. The Flutter Edge smoke run used `--dart-define=API_BASE_URL=http://127.0.0.1:8080/api/v1` and attached successfully.

The Android emulator/device disconnect-reconnect scenario is still not executable on this machine because only Windows and Edge are installed; it remains an explicitly unverified device check rather than a claimed pass.

An ephemeral live-client smoke harness using the production `ApiClient` and `DoctorRepository` (no mocks) also completed against `http://127.0.0.1:8080/api/v1`: Doctor login/MFA, all read repositories, refresh rotation, logout, and local session clearing passed. The harness was removed after the run; the permanent suite remains 27 tests.



