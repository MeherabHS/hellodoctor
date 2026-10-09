# System telemetry and error tracker verification

Verified 7 October 2026 against the current working tree on `main`, base commit `e3739ae555f198c537dfa7f505fe506b0cd62df2`. Existing uncommitted work was preserved. This was a verification pass; no production implementation was changed.

**FULL_SYSTEM_TELEMETRY_REQUIREMENT = NOT_MET**

**CURRENT_TRACKER = PARTIAL**. A real backend incident service, database, Admin API and Admin screen implementation exist. They do not provide complete automatic client/server collection or complete incident retrieval. Backend existence is verified; an end-to-end Admin browser acceptance pass was blocked by the running Admin application's HTTP 500.

## Coverage and evidence

| Area | Result | Evidence / limit |
| --- | --- | --- |
| Central backend incident storage | VERIFIED | `src/system_error.rs`, `repository/system_error_repo.rs`; live Admin API returns the persisted PostgreSQL incidents. |
| Internal/database/dependency errors returned as AppError | IMPLEMENTED; unit/integration pieces tested | `src/error.rs` reports Internal, Database and ServiceUnavailable. This pass did not deliberately crash a live business endpoint. |
| Error codes, response reference IDs | IMPLEMENTED WITH LIMITS | AppError generates a UUID used in the response and first incident. Dedupe retains the first UUID and increments count; subsequent request IDs are not stored. No lookup-by-reference endpoint was found. Internal response code is `INTERNAL_SERVER_ERROR`, incident code is `INTERNAL_ERROR`. |
| Source location | VERIFIED | `#[track_caller]` tests pass for Internal, ServiceUnavailable and SQLx conversion. Stored `stack_trace` normally contains a file:line, not a full call stack or source snippet. |
| Endpoint, impacted user, route/build context | PARTIAL | Schema supports endpoint/user, but ordinary AppError reporting does not populate them. No request-context propagation into those events was found. Admin mapper supplies hardcoded device/version/network labels instead of measured incident metadata. |
| Deduplication, count, secret redaction | VERIFIED WITH LIMITS | Isolated tests prove grouping and bearer/key redaction. Only error-message redaction is performed; this is not a general PHI sanitizer. Occurrence details and later request IDs are not retained. |
| Admin list/detail/acknowledge/resolve | BACKEND VERIFIED | Access/lifecycle integration test passes. Real Admin MFA login and list API pass. |
| Admin authorization | VERIFIED FOR TESTED ROLES | Doctor and Patient cannot list incidents; Admin lifecycle passes. Existing ABAC allows PlatformAdmin/SecurityAdmin for incident operations. Not a new exhaustive audit of every Admin role. |
| Admin rendered screen | NOT VERIFIED / BLOCKED | Existing port 3000 `/login` returns HTTP 500 with `Cannot find module` / `webpack-runtime` markers. Browser attempt has one page exception and one 500. No visual pass claimed. |
| Admin complete retrieval/filtering/counts | INCOMPLETE | `getSystemErrors` requests page_size=200, no page traversal in `useBackendSync`; page filters/KPIs count that snapshot. Backend pagination and global totals exist but are not fully consumed by this screen. |
| Admin JSON export | MISSING | Button in `src/app/page.tsx` only calls `showToast('Logs Exported ...')`; no export/file-download operation. |
| Doctor Web browser/render/promise errors | LOCAL LOG ONLY | Event handlers call `/api/diagnostics`; endpoint writes a coarse category/reference to Next console. It does not forward to Rust. Live diagnostic POST succeeds and creates no database incident. |
| Doctor Web unexpected API/BFF failures | PARTIAL | Rust AppError incidents remain shared. Web diagnostics discard the failing request's code/reference/context; some BFF failures become local 503 responses without an Admin incident. |
| Admin Web client/BFF errors | NO CENTRAL CLIENT INGESTION FOUND | No automatic crash reporter/global browser error handler or diagnostics ingestion route found in current Admin implementation. Console/catch handling is not Admin incident ingestion. |
| Flutter uncaught errors | LOCAL PRESENTATION ONLY | `apps/mobile/lib/main.dart` uses zone/FlutterError handlers to display errors; they do not send to a central error endpoint. ApiClient preserves a backend request ID for display but does not ingest client crashes. |
| Consultation telemetry event backend | VERIFIED | Ownership-checked batch ingestion, PostgreSQL event table and Admin timeline API; isolated test verifies participant ownership and disconnect/reconnect reconstruction. |
| Automatic consultation telemetry producers | MISSING IN CURRENT CLIENTS | DoctorRepository has a `telemetry(id, events)` helper but no lib caller was found. No event submission in current Doctor Web. Live DB has zero aggregate and zero event rows. Empty tables alone do not prove a broken API; absence of client wiring is the source finding. |
| Telemetry measurement honesty | FAIL | Backend command handler defaults missing packet loss to 0.0, RTT to 24 ms and connection to Connected. Admin grievance mapper also supplies Connected/24 ms/zero defaults. These values are not measurements and must not be represented as evidence. |
| HTTP request counters and duration | LIVE VERIFIED | Existing `/metrics` responds with `http_requests_total` and `http_request_duration_seconds`. This is aggregate process telemetry, not an incident log. |
| Background failures / panics / middleware responses | INCOMPLETE | Notification scheduler errors only use tracing; no central report call. No centralized panic hook/catch layer found. Timeout/load-shed handler returns plain responses rather than reporting through AppError. |
| Incident delivery during storage outage | BEST EFFORT | In-memory fire-and-forget queue; persistence failures are logged and explicitly not retried. No durable retry/outbox in this writer. Database outage cannot be assumed visible in the database-backed Admin tracker. |
| Alert delivery, retention, external collection | NOT VERIFIED / NO COMPLETE REPO CONFIG FOUND | Prometheus exporter and Admin polling exist. No configured collector/alert delivery or incident retention job found in reviewed current project sources; production infrastructure outside this workspace was not inspected. |
| Administrative audit trail | EXISTS; SEPARATE | `audit_events` and `/admin/logs` are distinct from system incidents. Existing mutation-audit tests were previously run in the full regression; this pass did not conflate the audit trail with telemetry. |

## Checks performed

Existing installed Rust tooling, Python Playwright and Edge were used. Existing PostgreSQL/Redis and existing backend/Admin/Doctor Web listeners were used; nothing was started on a new port.

- `cargo test --test api_tests system_error -- --test-threads=1`: **3/3 PASS** (persistence/dedupe/redaction, ordinary rejection exclusion, Admin ownership/access/lifecycle).
- `cargo test --test api_tests test_telemetry_event_ingestion_ownership_and_timeline_reconstruction -- --test-threads=1`: **1/1 PASS**.
- `cargo test --lib error::tests`: **7/7 PASS**, including the three system-error unit tests (also run separately). **11 unique relevant tests passed** across these runs.
- Live probes: backend Admin login with MFA, system-error feed, command telemetry API, Doctor Web local diagnostic endpoint, Prometheus counters/duration all responded successfully.
- Live database at probe time: **2 incidents; 0 consultation aggregate telemetry rows; 0 consultation event rows**. No incident was injected into the live database in this pass.
- Browser attempt: **FAIL/BLOCKED**, existing Admin login route HTTP 500/missing compiled module; no rendered tracker acceptance claimed.
- No production build was run because this check did not change production code. Earlier application build results do not override the current Admin runtime failure.

The ordinary-rejection integration test actually exercises 401, 403, 409 and a 405 caused by calling a POST RTC route using GET. Its source comments mention additional statuses; those comments are not evidence for 404/422/429 coverage.

Machine-readable evidence and the reproducible probe scripts are in `docs/audit_evidence/system_telemetry/`. Credentials/tokens and patient records are never saved in those results.

## Work required before full acceptance

1. Restore the existing Admin build/runtime and verify its incident screen in the browser.
2. Provide a shared authenticated, bounded, privacy-safe ingestion contract and wire Web/Admin/Flutter automatic error collection into the existing incident service. Preserve trace/reference correlation and safe release/component/route metadata; do not collect request bodies, prescriptions, clinical messages or tokens.
3. Carry server request context and occurrence-level references through deduplication; expose reliable reference lookup.
4. Route real background/system failures into incidents with a defined outage/fallback delivery policy. Keep expected business rejections separate from crashes.
5. Replace unmeasured telemetry defaults with explicit unmeasured states; wire actual consultation event producers when the RTC integration can produce them.
6. Finish Admin pagination/server filtering/global counts and actual log export. Verify access, filtering, persistence, failed delivery, redaction and cross-client collection with permanent tests.
7. Define and verify alerting and retention requirements using the existing project infrastructure. External observability installations remain unauthorized.

This list records reproducible missing coverage; it is not a replacement architecture proposal. These items were not silently implemented during the user's request to check the system.

## Host integrity

HOST_SOFTWARE_INSTALLED = NONE; HOST_SOFTWARE_UNINSTALLED = NONE; GLOBAL_PACKAGES_INSTALLED = NONE; WINDOWS_SERVICES_CREATED = NONE; PATH_MODIFIED = NO; REGISTRY_MODIFIED = NO; FIREWALL_MODIFIED = NO; NEW_DOCKER_INFRASTRUCTURE = NONE; MINIO_REINTRODUCED = NO; BROWSER_INSTALLED = NO; PLAYWRIGHT_BROWSER_DOWNLOADED = NO; ANDROID_TOOLING_INSTALLED = NO.

Production backend, mobile, Admin and Doctor Web source files changed by this verification pass: **NONE**. Only verification evidence/documentation was added.
