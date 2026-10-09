# Doctor Web prescription compression and upload progress

Verified 2026-10-07. Branch `main`, starting commit `e3739ae555f198c537dfa7f505fe506b0cd62df2`; existing uncommitted work preserved. Final production build: `6MUCqdkhsIXrtOLwiipGX`, preview `http://127.0.0.1:3001`.

## Current contract and implementation

Mobile `doctor_pages.dart` picks images at quality 85 and uploads each prescription page separately, with Dio send progress. The current backend `upload_security_service.rs` guarantees a 400 × 1024-byte image budget for each single-image Doctor submission. Patient multi-image intake has a different batch budget and was not changed.

Web now prepares photos on selection using the browser's existing image decoder and canvas encoder. Each page must be at most **409,600 image bytes** before dispatch. There is no oversized fallback. The JSON/base64 transport is naturally larger; the image limit does not mean a 400 KB JSON request.

The policy follows current server limits: input up to 10 MB, long edge at most 2,000 pixels, JPEG quality 80 down through 65/50/35/20/5/1, then halving dimensions and retrying. PNG remains PNG and uses resizing when necessary. WebP becomes JPEG because the current backend accepts JPEG/PNG, rather than WebP bytes. Native browser and Rust encoders are different, so byte-for-byte output parity is not claimed. Canvas output strips metadata; the server independently validates and sanitizes every accepted image. Pathological images may lose detail to fit the hard budget; the Doctor sees the actual prepared preview before dispatch.

Compression is asynchronous, shows an accessible preparing status, and disables conflicting controls. Corrupt/oversized source files produce actionable errors without discarding other valid pages. Compressed files and previews remain ephemeral; account/session boundaries clear them. No patient file or token is stored in browser storage.

Same-origin XMLHttpRequest posts the existing `base64_photo`, `doctor_notes`, `mime_type` DTO to `/api/proxy/consultations/:appointment_id/prescription`. HttpOnly BFF credentials, ownership rules and scoped idempotency remain unchanged. The progress bar sits next to Send Prescription and reports actual browser-to-BFF bytes for the active page. It does not claim visibility into BFF-to-backend/storage transfer. At 100% it explicitly waits for server confirmation. Only successful API responses set UPLOADED. Failed pages retain their compressed bytes, request key and original advice for retry; confirmed pages are not resent. Runtime invalidation aborts pending XHR and suppresses late results/progress.

## Evidence

| Check | Result |
| --- | --- |
| Typecheck, project lint, format checks | PASS |
| Unit tests | 18 PASS, including hard byte boundary, quality exhaustion/resizing, PNG, malformed encoder/size limits, session boundary, progress versus confirmation, partial success and retries |
| Production build | PASS |
| Installed Edge compression/API walkthrough | 8 PASS |
| Existing file-picker/retry/real login/MFA walkthrough | 7 PASS on final build |
| Responsive progress | Rendered at 1440, 1024, 390; no horizontal overflow; supplemental cold narrow-width capture avoids responsive transition artifacts |
| Browser JavaScript exceptions | 0 |
| Mobile and Rust source changes during this pass | NONE; protected-source hashes match prior baseline |

Generated synthetic high-entropy inputs, all initially above the budget, produced real outgoing image payloads:

| Input | Prepared format | Prepared image bytes |
| --- | --- | --- |
| JPEG | JPEG | 332,702 |
| PNG | PNG | 399,379 |
| WebP | JPEG | 332,359 |

Throttling via installed Edge's DevTools protocol demonstrated real intermediate progress, not timed animation. Retry payload/hash/key/advice equality, malformed file rejection, multiple-page preservation, actual backend errors, cancellation at the session boundary and absence of prescription rows after failure were checked. Synthetic accounts/photos only; no credentials or real patient content recorded. Existing approved PostgreSQL/Redis containers were read for fixture/count/rate-limit verification; no infrastructure was created or rate limit reset.

Evidence: [compression results](audit_evidence/doctor_web/rx-compression/results.json), [existing Rx checks](audit_evidence/doctor_web/rx-upload/results.json), [desktop progress screenshot](audit_evidence/doctor_web/rx-compression/actual-upload-progress-1440.png), [narrow progress screenshot](audit_evidence/doctor_web/rx-compression/actual-upload-progress-390.png), [supplemental narrow check](audit_evidence/doctor_web/rx-compression/narrow-visual.json). Tests: `rx-compression.test.ts`, `rx-upload.test.ts`, `rx_compression_regression.py`, `rx_compression_visual.py`.

## Remaining storage blocker

The final standard Rx walkthrough received real **409 RESOURCE_CONFLICT: Document storage is not configured. Upload is unavailable.** Repeated compression-test submissions also encountered the genuine **429 RATE_LIMIT_EXCEEDED** guard. Both remain failures with recoverable pages and no false server confirmation. Rate limiting was not bypassed; native-byte progress and compression were also checked with the actual storage rejection on the earlier build of the same compression implementation.

`STORAGE_CONFIGURATION_REQUIRED`: successful private storage persistence is still unverified. Compression and progress are complete; this does not change the prior whole-app verdict `DOCTOR_WEB_PARTIAL`. No MinIO, fake success or alternate storage was introduced.

## Host and asset integrity

HOST_SOFTWARE_INSTALLED = NONE

HOST_SOFTWARE_UNINSTALLED = NONE

GLOBAL_PACKAGES_INSTALLED = NONE

WINDOWS_SERVICES_CREATED = NONE

PATH_MODIFIED = NO

REGISTRY_MODIFIED = NO

FIREWALL_MODIFIED = NO

NEW_DOCKER_INFRASTRUCTURE = NONE

MINIO_REINTRODUCED = NO

BROWSER_INSTALLED = NO

PLAYWRIGHT_BROWSER_DOWNLOADED = NO

ANDROID_TOOLING_INSTALLED = NO

OPEN_SOURCE_ASSETS_DOWNLOADED = 0

No package/dependency changes. Only the authorized existing loopback preview on port 3001 was restarted with the new build. No unrelated Web screen, mobile or Rust implementation was modified. Stop this targeted scope.
