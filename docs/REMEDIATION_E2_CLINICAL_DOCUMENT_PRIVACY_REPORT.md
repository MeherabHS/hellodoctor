# Remediation E2 — Secure Clinical Document Viewing

## Result

**GREEN for the scoped client-side application gate; native on-device PDF
rendering is explicitly NOT VERIFIED.** `flutter analyze`, `dart format
--set-exit-if-changed`, and the full existing test suite all pass for both
apps with zero regressions, and 17 new security regression tests (12 Doctor +
5 Patient) pass against real fakes of the repository layer. The one thing
this sandbox cannot prove is that `pdfx`'s native pdfium rendering actually
displays a real PDF correctly on a physical Android/iOS device or emulator —
no such device was available here. Everything else required by this task
(routing, authorization reuse, no external-application fallback, graceful
failure handling, dependency vetting) is verified, not assumed.

This is a client-only change. No backend code was touched — the existing
authenticated, single-redemption document-download-grant endpoint
(`get_document_download_url`/`download_document` in
`services/backend/src/api/v1/prescription_handlers.rs`) is reused exactly as
it was; no backend tests were run because no backend code changed.

No commits, stages, pushes, resets, or cleans were performed. No Docker
resources or databases were touched. Remediation E1's verified backend
checkpoint (452 passed/0 failed/4 ignored; `cargo test --lib` 108/108) is
undisturbed.

## Phase 1 — reproduced behavior (re-verified against current source, not just cited)

**Doctor app (`apps/mobile`).** `DocumentButton`/`_DocumentButtonState`
(`lib/features/doctor/doctor_pages.dart`, formerly lines 4658-4687) had no
mime-type branch at all: every tap of "Open authorized document" — for a
clinical attachment or Rx page, image or PDF, at both call sites (the live
appointment's "Clinical attachments and prescriptions" list and
`openHistoryDetail` for historical episodes) — called
`launchUrl(Uri.parse(url), mode: LaunchMode.externalApplication)`
unconditionally. `mime_type` was present in the backend's document JSON and
was even inspected — `findDocumentId`/`findDocumentIds` checked
`node['mime_type'] != null` as an inclusion filter — then discarded; it never
reached `DocumentButton`. The app already had a safe, shipped in-app image
viewer, `ClinicalImageViewer` (`lib/features/doctor/clinical_image_viewer.dart`),
but it was wired only to the 24-hour clinical chat attachment feature
(`ChatThreadPage`'s `ClinicalMessageBubble.onViewAttachment`), a separate
data path from appointment-level documents.

**Patient app (`apps/mobile_patient`).** `health_vault_page.dart`'s `_open`
already routed `image/jpeg`/`image/png` to an in-app `_ImageViewerPage`, but
every other mime type — in practice, PDF — went to the same
`launchUrl(url, mode: LaunchMode.externalApplication)`. This was confirmed
real, not dead code: `services/backend/src/services/upload_security_service.rs::validated_format`
explicitly accepts `application/pdf` (magic-byte-sniffed), doctor-authored Rx
pages can be PDF (`prescription_service.rs::upload_doctor_prescription_photo`),
and patients can attach real PDF lab reports via a dedicated `_pickPdf()`
picker in `pre_consult_intake_modal.dart`.

**Document retrieval, confirmed unchanged and correctly authorized.** Both
paths resolve a short-lived (300s), single-redemption (Redis `GETDEL`)
signed download grant before fetching bytes
(`prescription_handlers.rs::get_document_download_url`/`download_document`).
Ownership/authorization is checked server-side before the grant is minted
(`authorized_document`), not by the client. The server also returns an
authoritative `Content-Type` header on redemption — a fact this fix uses (see
Phase 2).

## Phase 2 — secure image viewing

- Added `DocumentRef {id, mimeType}` and changed `findDocumentIds` to return
  `List<DocumentRef>` instead of discarding the mime type, threading it
  through to both `DocumentButton` call sites.
- `DocumentButton` now branches on the declared mime type: `image/jpeg` or
  `image/png` pushes the existing `ClinicalImageViewer` unchanged (no
  duplicate image viewer introduced) with a `load` callback that calls a new
  `DoctorRepository.fetchDocumentBytes()` — mints a fresh grant via the
  existing `documentUrl()` and redeems it via the authenticated client, never
  `launchUrl`.
- **Cross-check, not blind trust:** a new `ApiClient.getBytesWithType()` (both
  apps) returns the bytes *and* the server's real `Content-Type` response
  header. `DocumentButton`/`health_vault_page.dart`'s `_open` both compare
  this against the declared mime type from the listing and refuse to render
  (safe error, not a guess) on any mismatch — satisfying "don't trust a
  client-side value alone" with an actual second, authoritative source, not
  just by relying on the listing.
- The Patient app's existing image path (`_ImageViewerPage`, `Image.network`)
  is untouched — confirmed still working by a new regression test, per the
  original scope ("unchanged safe path still works").
- No clinical-file encryption-at-rest code was touched (out of scope, and
  unnecessary — this change is entirely about what the already-decrypted
  bytes are displayed with, not storage).

## Phase 3 — secure PDF viewing

**Dependency decision, verified live, not guessed.** No existing dependency
in either `pubspec.lock` could render a PDF. This sandbox had real network
access (confirmed before adding anything), so the choice below was verified
against the actual current pub.dev/GitHub state today rather than stale
training knowledge:

- **Package:** `pdfx: ^2.11.0` (added to both apps' `pubspec.yaml`; `flutter
  pub get` succeeded for both, 7 new transitive packages each, no conflicts).
- **License:** MIT (fetched and read the actual `LICENSE` file from
  `github.com/ScerIO/packages.flutter` — OSI-approved, commercial-safe).
- **Publisher/maintenance signal, fetched from `pub.dev/api/packages/pdfx/score`:**
  160/160 pub points (max score), verified publisher `serge.software`, 528
  likes, ~490k downloads/30 days.
- **Known vulnerabilities:** queried `api.osv.dev` directly for `{"package":
  {"name": "pdfx", "ecosystem": "Pub"}}` — empty result, no advisories found.
- **SDK compatibility:** package requires Flutter `>=3.29.0`; this sandbox
  runs Flutter 3.47.6 — compatible, confirmed by the successful `pub get`
  and clean `flutter analyze`/`flutter test` runs below.
- **Temp-file behavior — read from the actual plugin source, not assumed.**
  Fetched `pdfx`'s real source from GitHub
  (`lib/src/renderer/interfaces/document.dart`,
  `lib/src/viewer/simple/pdf_view.dart`,
  `lib/src/viewer/simple/pdf_controller.dart`): `PdfDocument.openData(Uint8List)`
  opens a document directly from memory — no file path parameter exists in
  that API at all. Page rendering (`PdfPage.render()`) returns an in-memory
  `PdfPageImage` over the platform channel, consumed as an ordinary
  `ImageProvider` (the same category of in-memory, non-persistent caching
  already accepted for `Image.network`/`ClinicalImageViewer` in this
  codebase). **Conclusion: this implementation never writes a PDF temp file
  to disk at all** — a stronger answer than "a short-lived temp file in an
  app-private directory," and it makes the startup-sweep-for-stale-files
  requirement moot (there is nothing to sweep).
- **Resource cleanup:** `PdfController.dispose()` (verified from source)
  does **not** close the underlying native `PdfDocument` — a real gap in the
  plugin's own API ergonomics. `ClinicalPdfViewer` (new file, both apps)
  works around this by holding its own reference to the resolved
  `PdfDocument` and calling `.close()` explicitly in `dispose()` and before
  every retry, so native resources are actually released.
- **No external launch, no sharing affordance:** `ClinicalPdfViewer` renders
  entirely via `PdfView` inside the app's own `Scaffold`; no "share"/"open
  in" action was added.
- **No raw errors/URLs in logs:** the widget never calls `print`/`debugPrint`
  on the fetched URL, token, or raw plugin exception — only a fixed, generic
  "Unable to open this document." string, consistent with this codebase's
  existing `client_error_reporter.dart` discipline.

## Two real bugs found and fixed while writing tests

Writing regression tests against this change surfaced two genuine defects —
reported here rather than silently patched, since they affect reliability of
the exact code paths this remediation now routes far more traffic through:

1. **Pre-existing bug in `ClinicalImageViewer`** (`apps/mobile`, not written
   by this pass): its Retry button was `onPressed: () =>
   setState(() => _image = widget.load())`. Because the inner arrow
   function's body is an assignment expression, it returns the assigned
   `Future` value; Flutter's `State.setState` asserts at runtime that its
   callback never returns a `Future` (to catch exactly this class of
   accidental-async-in-setState mistake) and throws. This had never been
   exercised by a test before. Fixed by giving the callback a block body
   (`{ _image = widget.load(); }`) so it returns `void`. In scope because
   `DocumentButton` now sends substantially more traffic through this exact
   retry path than before.
2. **New bug introduced during this pass, caught before landing:**
   `ClinicalPdfViewer`'s first draft used
   `documentFuture.then((doc) => _openDocument = doc, onError: (_) {})`. Dart
   infers the `.then()` call's type parameter from the `onValue` callback's
   return type (`PdfDocument`, from the assignment expression), but the
   `onError` callback `(_) {}` returns `void` — a type mismatch that throws
   `ArgumentError` at runtime on *every* open, success or failure. Fixed by
   splitting into `.then((doc) { _openDocument = doc; }).catchError((_) {})`.
   Caught entirely by the new test suite before this ever reached a device.

## Phase 4 — security regression tests

**Doctor app** (`apps/mobile/test/doctor_document_viewer_test.dart`, 12 new
tests, all passing):
`image/jpeg` and `image/png` route to `ClinicalImageViewer`; `application/pdf`
routes to `ClinicalPdfViewer`; an unrecognized mime type (`application/zip`)
and a missing mime type are both refused with a message and **never reach the
network fetch** (asserted via a fetch-call counter); expired/revoked auth
(401) and a missing document (404) both show a graceful in-viewer error, not
a crash; a server `Content-Type` that disagrees with the declared mime type
is rejected rather than rendered; retry mints a fresh grant (asserted via the
fetch counter incrementing, proving the one-time grant isn't reused); a
malformed PDF and a PDF fetch failure both fail without throwing; disposing
after a failed open does not throw.

**Patient app**
(`apps/mobile_patient/test/health_vault_document_viewer_test.dart`, 5 new
tests, all passing): a PDF document now routes to `ClinicalPdfViewer` (not
`launchUrl`) and actually calls the authenticated fetch; an image document is
confirmed **unchanged** — it still never calls the new bytes-fetch method,
proving the existing safe path wasn't disturbed; an unsupported mime type
shows a safe snack-bar message with zero network calls; a mismatched
server/declared content-type and an expired-auth fetch both fail gracefully.

**Cross-account document access:** not independently client-testable — there
is no client-side authorization logic to bypass. Confirmed by code trace
(Phase 1) that both apps' new `fetchDocumentBytes()` always goes through the
real `documentUrl()`/`resolveDownloadUrl()` call, which always hits the
backend's own ownership check (`authorized_document`) before a grant is
minted; nothing in this change touches or could bypass that.

**No accidental external launch, structurally:** `url_launcher`'s import was
removed entirely from `doctor_pages.dart` and `health_vault_page.dart` (the
Patient app still legitimately uses it elsewhere — blog links, consultation
status links, the emergency-contact call banner — none of which are clinical
documents). In the Doctor app, `url_launcher` is now unreferenced in source
entirely; the dependency itself was left in `pubspec.yaml` rather than
removed, to keep this change scoped to the security fix (flagged below as
optional follow-up cleanup, not a defect).

## Files changed

**Doctor app (`apps/mobile`):**
- `lib/core/network/api_client.dart` — new `TypedBytes`, `getBytesWithType()`.
- `lib/features/doctor/doctor_repository.dart` — new `fetchDocumentBytes()`.
- `lib/features/doctor/doctor_pages.dart` — `DocumentRef`, `findDocumentIds`
  signature change, `DocumentButton` routing rewrite, `url_launcher` import
  removed.
- `lib/features/doctor/clinical_image_viewer.dart` — one-line Retry bug fix
  (see above).
- `lib/features/doctor/clinical_pdf_viewer.dart` (new).
- `pubspec.yaml`/`pubspec.lock` — `pdfx: ^2.11.0` added.
- `test/doctor_document_viewer_test.dart` (new, 12 tests).

**Patient app (`apps/mobile_patient`):**
- `lib/core/network/api_client.dart` — identical `TypedBytes`/`getBytesWithType()`.
- `lib/features/patient/vault/vault_repository.dart` — new `fetchDocumentBytes()`.
- `lib/features/patient/vault/health_vault_page.dart` — `_open` rewrite for
  the PDF branch, new `_loadVerifiedBytes`, `url_launcher` import removed.
- `lib/features/patient/vault/clinical_pdf_viewer.dart` (new).
- `pubspec.yaml`/`pubspec.lock` — `pdfx: ^2.11.0` added.
- `test/health_vault_document_viewer_test.dart` (new, 5 tests).

**Backend:** no files changed.

## Cache and temporary-file security

No temporary file is created by this change in either app, for either image
or PDF viewing — confirmed from `pdfx`'s actual source (Phase 3), not
assumed. All document bytes live only in process memory for the lifetime of
the viewer route:

- Images: `MemoryImage`/`Image.network`'s standard in-memory image cache,
  evicted on dispose (images) or subject to Flutter's normal in-memory LRU
  (unchanged Patient path) — the same pattern already accepted elsewhere in
  this codebase.
- PDFs: `pdfx` renders pages to in-memory bitmaps over a platform channel;
  the native `PdfDocument` handle is explicitly closed in `dispose()` and
  before every retry.
- No document URL, token, or raw plugin error is ever logged.

## Dependencies introduced

| Package | Version | Ecosystem | License | Confidence | Reasoning |
|---|---|---|---|---|---|
| `pdfx` | `^2.11.0` | pub.dev | MIT | High — verified live against pub.dev's API and the package's actual GitHub source during this pass, not recalled from training data | Only in-memory PDF-open/render API found; verified publisher, max pub score, no known OSV advisories |

No other new dependencies were added.

## Tests and exact exit codes

| Check | App | Command | Result |
|---|---|---|---|
| New regression tests | Doctor | `flutter test test/doctor_document_viewer_test.dart` | PASS — 12/12 |
| New regression tests | Patient | `flutter test test/health_vault_document_viewer_test.dart` | PASS — 5/5 |
| Full test suite | Doctor | `flutter test` | PASS — 302/302, exit 0 |
| Full test suite | Patient | `flutter test` | PASS — 364/364, exit 0 |
| Static analysis | Doctor | `flutter analyze` | PASS — "No issues found!", exit 0 |
| Static analysis | Patient | `flutter analyze` | PASS — "No issues found!", exit 0 |
| Formatting | Doctor | `dart format --set-exit-if-changed lib test` | PASS — 0 changed, exit 0 |
| Formatting | Patient | `dart format --set-exit-if-changed lib test` | PASS — 0 changed, exit 0 |
| Dependency advisory check | — | `api.osv.dev` query for `pdfx`/Pub | PASS — no advisories found |
| Backend tests | — | not run | No backend code changed |

## Platform-specific limitations (explicit, not glossed over)

- **Real on-device/emulator PDF rendering is NOT VERIFIED.** This sandbox has
  no Android emulator, no iOS simulator, and no physical device. All PDF
  tests here verify *routing* (the right viewer opens) and *graceful failure*
  (no crash, no external-application fallback) against a harness with no
  native pdfium binary — not that a real PDF visually renders correctly, or
  that paging/zoom/scroll feel right on-screen. This must be run on an actual
  Android and iOS device/emulator before this is treated as fully verified,
  per the task's own instruction not to claim physical-device behavior from
  widget tests alone.
- Android/iOS native build compatibility for `pdfx` (Gradle/CocoaPods
  resolution, and in particular any repeat of this codebase's own documented
  `agora_rtc_engine`-vs-`file_picker` AndroidX version conflict) was not
  checked — no Android SDK/Gradle or Xcode toolchain was exercised in this
  sandbox, only `flutter pub get`/`analyze`/`test`. A real `flutter build
  apk`/`flutter build ios` should be run before release.
- Web/desktop PDF rendering (pdfx also supports Web/Windows/macOS) was not
  considered — out of scope, since this project targets Android/iOS only.

## Remaining security risks / judgment calls

- **Invalid/unsupported mime type → safe refusal, not a fallback.** Both apps
  now show a plain "cannot preview"/snack-bar message for any mime type that
  isn't a recognized image or PDF, including `null`. This is a deliberate,
  conservative choice per the task's own instruction ("fail safely, not
  silently external-launch as a fallback") — it is a narrowing of prior
  behavior: any document type besides JPEG/PNG/PDF that could previously be
  opened externally can no longer be opened in-app at all. Given this
  codebase's own upload validators only ever accept JPEG/PNG/PDF for clinical
  documents, this should not affect any genuine real-world document, but is
  worth confirming against production data before relying on it.
- `url_launcher` is now unused in the Doctor app's source but was left in
  `pubspec.yaml` to keep this change scoped; removing it is a reasonable,
  separate, zero-risk cleanup.
- `pdfx`'s own `PdfController.dispose()` not closing the underlying document
  is an upstream API sharp edge, not a bug in this codebase — worked around
  explicitly here, but worth remembering if `PdfController` is ever used
  directly elsewhere in the future without the same explicit `.close()`.

## Known limitations (restated for visibility)

1. Native on-device PDF rendering is unverified in this sandbox.
2. Native Android/iOS build compatibility for the new dependency is unverified.
3. The safe-refusal behavior for unrecognized mime types is a narrowing of
   prior (insecure) behavior, not a feature regression against any
   currently-valid document type, but should be confirmed against real
   production document mime-type distribution.
