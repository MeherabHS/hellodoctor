# Doctor client error reporting verification

Date: 2026-10-08. Scope: Doctor Flutter and Doctor Web incidents entering the existing backend System Error Service and existing Admin diagnostics. This is an observability verification, not a production-security approval.

## Implementation

Source inspection found no suitable authenticated client-incident ingestion endpoint. Added one: `POST /api/v1/client-errors`. It calls the existing `system_error::report_sync` pipeline and persists in the existing `system_error_logs` table. The only schema addition is nullable `client_context` on that table; both migration directions exist. No second table, monitoring provider, Admin page, or infrastructure was introduced.

The backend permits authenticated Doctors only. It derives the impacted user from `AuthContext`, assigns ERROR severity and controlled client subsystem/component names, generates trace/fingerprint values, and retains the existing incident lifecycle. Unknown fields, including client-supplied identity, severity, environment, status, and fingerprint, are rejected. Payloads are capped at 16 KiB, messages at 2,000 bytes, and stack traces at 6,000 bytes. The endpoint enforces 12 submissions per Doctor per minute. Concurrent client reports group atomically under a database advisory transaction lock; grouping includes Doctor, build, platform, error type, screen, and operation. Existing backend fingerprint behavior is preserved.

Privacy is enforced before transmission and again at ingestion: arbitrary exception prose is omitted rather than attempting to recognize every clinical phrase. Only bounded source-file locations survive stack processing; exception headers, argument values, arbitrary function names, URL hosts/query strings, and unrecognized screen/operation strings are removed. This preserves source locations while deliberately sacrificing potentially sensitive diagnostic prose. Release identifiers remain client-reported metadata, not security assertions.

Flutter uses one reporter from the existing FlutterError, PlatformDispatcher, and guarded-zone boundaries. Repository boundaries report unexpected parsing/invariant failures even when UI code catches them. The existing Dio/session infrastructure supplies the current session snapshot. Unexpected Dio JSON decoding failures also enter the reporter. Diagnostic requests do not refresh or expire sessions. Native Android/iOS metadata hooks read installed version/build values; desktop/test fallbacks match the current pubspec. Native metadata behavior has unit coverage, but native builds and device execution were not performed.

Doctor Web uses its existing same-origin `/api/diagnostics` BFF, now forwarding sanitized incidents with the server-held session token. It records render/browser/unhandled-promise failures, caught unexpected exceptions, malformed API responses, and HTTP 5xx system failures through the shared transport, including document/upload paths. Generic operations cover message sending, scheduling/booking vocabulary, login, prescriptions, RTC token requests, and telemetry submissions. Backend reference IDs are retained when available. The BFF supplies the package version and actual Next build ID.

Both reporters are best effort, bounded, and protected against recursion. Expected business 4xx responses and normal offline/cancellation/timeout states are excluded. There is no persistent incident queue. Pre-authentication frontend errors are not uploaded; existing backend failures during login still use backend reporting. Busy reporters, rate limiting, session changes, offline devices, or ingestion outages can drop reports. “Every failure” therefore means coverage of the central unexpected-failure paths, not a guarantee of delivery for every occurrence.

## Existing Admin diagnostics

The existing 15-second polling/list/detail APIs are reused. Client subsystem, Doctor association, app version/build, platform, screen/operation, and sanitized source frames appear in the existing incident dialog. Two existing runtime blockers were fixed to make the Admin login work: Next bootstrap scripts now receive a per-request CSP nonce, and the page-routing gate uses a separate noncredential session marker because access cookies are scoped to `/api`. Backend JWT authorization remains the data-access boundary. Missing telemetry/build/network measurements are displayed as unmeasured/not captured instead of invented successful values.

## Verification evidence

- Live results: [client-delivery-results.json](audit_evidence/system_telemetry/client-delivery-results.json).
- Repeatable development-only probe: [verify_client_delivery.py](audit_evidence/system_telemetry/verify_client_delivery.py).
- Rendered Doctor Web exception: [screenshot](audit_evidence/system_telemetry/doctor-web-client-incident.png).
- Rendered simulated web system response: [screenshot](audit_evidence/system_telemetry/doctor-web-api-failure.png).
- Rendered Flutter framework incident: [screenshot](audit_evidence/system_telemetry/flutter-framework-client-incident.png).
- Rendered existing backend service simulation: [screenshot](audit_evidence/system_telemetry/backend-system-service-simulation.png).
- Flutter explicit live-network harness: `apps/mobile/test/dev/client_incident_live.dart`. Credentials are passed only in the child process environment; the saved result contains an incident UUID and delivery status.

The browser probe uses real Doctor MFA and Admin TOTP sessions. A controlled browser exception traverses the actual browser listener and BFF. HTTP 202 acceptance is independently correlated with PostgreSQL and the exact Admin detail row; Chromium's fire-and-forget response-body capture sometimes requires a PostgreSQL lookup. The row is located after existing Admin polling and opened in the rendered dialog. Controlled duty-request responses are intercepted before any clinical mutation: 409/422 create no incidents, and 503 appears in Admin. Secret/PHI fixtures are absent from persisted/detail payloads.

The Flutter framework probe uses the real Flutter error hook, real Dio, a real authenticated backend session, PostgreSQL, and the rendered Admin dialog. Its source frame is explicitly synthetic. This proves framework/network/backend/Admin delivery, not Android/iOS device delivery. No mobile device or emulator was available; only Windows and existing Edge were detected.

Backend verification includes a test-only HTTP handler returning an actual internal backend error, followed by retrieval through the existing Admin API. Live backend verification uses the existing development-only System Error Service simulator, not a newly exposed crash endpoint. Synthetic incidents are resolved through the existing Admin API. Repeated local fixture logins required resetting only the development loopback login bucket; the blanket API rate limit was preserved and observed through bounded backoff.

Checks completed on the shared tree after the concurrent ID changes: backend all-target check, clippy with warnings denied, Rust formatting, source/binary/database schema agreement at `20261022000001`, Flutter analysis, and Doctor Web/Admin production builds. Current targeted backend tests: 16/16 (8 client integrations, 3 existing system-error integrations, 1 telemetry/grievance integration, 4 relevant unit tests). Flutter full suite: 65/65, including 14 reporter tests; explicit real-network framework probe: 1/1. Doctor Web full suite: 30/30. These counts do not claim a fresh full backend-suite run or native-device acceptance.

## Calls and patient grievances

Existing endpoints support RTC token issuance, aggregate telemetry, and bounded lifecycle-event batches. Participant ownership is checked, actor perspective is derived from authentication, and event vocabulary/metrics are bounded. Admin session timelines retrieve ordered events, reconnect windows, aggregate telemetry, and grievances referencing the same consultation. The integration test submits a patient grievance and confirms it accompanies the same session's four accepted events. System-error correlation remains explicitly heuristic by participant and time window.

Doctor Web now permits the existing RTC/telemetry routes through its restricted BFF and provides a tested callback adapter. No event is fabricated merely because a page opens or a token is issued. Agora SDK callbacks are not integrated, so actual call delivery, packet-loss/RTT measurements, and causation are not verified. Participant-reported event names, measurements, and timestamps should be treated as claims, not independent evidence for grievance adjudication.

## Final status

```text
BACKEND_ERROR_REPORTING_EXISTING = VERIFIED
ADMIN_SYSTEM_ERROR_SYNC_EXISTING = VERIFIED
DOCTOR_FLUTTER_LOCAL_HANDLER = VERIFIED
CLIENT_ERROR_INGESTION_ENDPOINT = IMPLEMENTED
FLUTTER_TO_BACKEND_ERROR_WIRING = VERIFIED
FLUTTER_ERROR_TO_SYSTEM_ERROR_LOG = PASS (real framework/network probe)
SYSTEM_ERROR_LOG_TO_ADMIN = PASS
END_TO_END_DOCTOR_EXCEPTION_TO_ADMIN = NOT_LIVE_VERIFIED (mobile device)
DOCTOR_WEB_EXCEPTION_TO_ADMIN = PASS
NORMAL_409_NOT_REPORTED = PASS
NORMAL_422_NOT_REPORTED = PASS
SECRET_REDACTION = PASS
PHI_REDACTION = PASS
RECURSION_GUARD = PASS
APP_VERSION_CORRELATION = PASS
BACKEND_TESTS = 16/16 targeted
FLUTTER_TESTS = 65/65; live framework probe 1/1
DOCTOR_WEB_TESTS = 30/30
CALL_TELEMETRY_API = VERIFIED (integration)
PATIENT_GRIEVANCE_TELEMETRY_LINK = PASS (integration)
LIVE_AGORA_CALLBACK_DELIVERY = NOT_LIVE_VERIFIED
LIVE_DEVICE_DELIVERY = NOT_LIVE_VERIFIED
HOST_SOFTWARE_INSTALLED = NONE
HOST_SOFTWARE_UNINSTALLED = NONE
ANDROID_EMULATOR_INSTALLED = NO
BROWSER_INSTALLED = NO
GLOBAL_PACKAGES_INSTALLED = NONE
NEW_DOCKER_INFRASTRUCTURE = NONE
MINIO_REINTRODUCED = NO
PATH_MODIFIED = NO
VERDICT = DOCTOR_FLUTTER_ERROR_REPORTING_CODE_COMPLETE_LIVE_DEVICE_PENDING
```

Host flags describe changes made by this task. Existing Edge, Flutter, Rust, Node, PostgreSQL, and Redis were reused. Unrelated security findings from the separate read-only assessment are not marked fixed by this observability work.
