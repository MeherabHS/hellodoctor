# Milestone 6D stabilization and acceptance closure

Scope: stabilization of the implemented CMS, following the corrected Azure storage
decision. No CMS rebuild, Campaign Management rewrite, MCQs, deployment, account
creation, Git staging/commit/reset or shared-database reset.

## A. Application defects fixed

Sensitive-content authors now see **“Approval required from another Admin.”**
with Approve disabled. Authenticated Admin article responses project `can_approve`
and `approval_block_reason` from the current database transaction context, article
state and article/category sensitivity. Independent reviewers retain approval
controls. Missing capability data fails closed. These fields do not enter Patient
responses. Backend independent-review authorization is unchanged and remains
authoritative; a direct prohibited request returns 403 and preserves pending state
and version.

CMS screens translate existing API error envelopes into readable messages. The
specific self-approval 403 becomes the editorial instruction; generic permission,
session and server failures do not expose raw JSON or server diagnostics. This
translation is CMS-scoped and does not alter campaign or authentication transport.

A cover failure after a successful text save now reports **“Text draft saved”**
and the failure, refreshes the saved article list, retains the selected file and
does not show a successful-save/upload notice. Missing storage is described as
**“Cover upload deferred — awaiting Azure Blob integration. No cover was uploaded.”**
The existing article/version upload association, create-only keys, encryption,
ownership, published-read gating and abandoned-media cleanup remain unchanged.

### Exact application/configuration files changed in this stabilization

| File | Change |
|---|---|
| `services/backend/src/repository/admin_article_repo.rs` | Authenticated approval capability projection; no schema change. |
| `services/backend/tests/cases/admin_articles.rs` | Strengthened existing sensitive-review test for author/reviewer capabilities, rejected-request state and Patient response isolation. |
| `apps/admin-web/src/lib/blogContent.ts` | Types for the two Admin-only response fields. |
| `apps/admin-web/src/lib/blogErrors.ts` | New scoped readable-error helper. |
| `apps/admin-web/src/components/blogs/ArticleEditor.tsx` | Approval controls/messages, readable errors and truthful partial-save handling. |
| `apps/admin-web/src/components/blogs/BlogManager.tsx` | Readable CMS errors. |
| `apps/admin-web/src/components/blogs/CategoryManager.tsx` | Readable CMS errors. |
| `apps/admin-web/tests/blog-errors.test.ts` | Three new error-translation cases. |
| `apps/admin-web/tests/blog-preview.test.tsx` | Capability fixture plus four new component cases, including cover failure after text save. |
| `apps/admin-web/package.json` | Compatible Next and matching ESLint-config patch pins. |
| `apps/admin-web/package-lock.json` | Matching resolved dependencies and compatible transitive Zod patch. |
| `services/backend/.env.example` | Comments explaining intentional legacy-storage deferral and linking the Azure plan; no actual credentials/configuration changed. |

No new migration or endpoint. Existing Doctor article policies, unified Patient
feed, approval/content versions, scheduling and notification behavior are preserved.
There are no Patient Dart changes in stabilization.

Documentation added: this report and `docs/AZURE_BLOB_INTEGRATION_PLAN.md`.
`docs/MILESTONE_6D_REPORT.md`, `docs/CHECKPOINT.md` and `docs/PROGRESS.md` receive
stabilization pointers; their earlier implementation results remain historical.
Reproducible local harnesses/evidence are under `scratch/milestone6d-stabilization-*`;
the existing `scratch/milestone6d-browser-interactive.py` and render/layout harnesses
were updated/re-executed against the current components.

## B. Security dependency remediation

| Installed before | Installed after | Result |
|---|---|---|
| Next `16.4.0` | `16.4.3` | Compatible patch; reported critical/high Next advisories removed. |
| `eslint-config-next` `16.4.0` | `16.4.3` | Matched to framework patch. |
| Transitive Zod `4.6.5` | `4.6.7` | Compatible patch in the existing ESLint hooks dependency chain; reported high advisory removed. |

The reported Next advisories concern
[SVG image optimization](https://github.com/advisories/GHSA-2464-fhr4-w9xw),
[App Router Server Actions](https://github.com/advisories/GHSA-p3m5-59mp-jv5r) and
[`use cache` typed-array results](https://github.com/advisories/GHSA-673q-mjm6-r5xr).
The source inspection found no application imports of `next/image`, `use server`
or `use cache`; that limits demonstrated feature use, not a claim that the old
framework was safe to retain. The patched framework passes production build.
The [Zod advisory](https://github.com/advisories/GHSA-mrhx-c7xc-m3w5) affected the
installed tooling dependency and is removed by the resolved patch.

`npm audit`: **before 1 critical, 1 high, 3 moderate; after 0 critical, 0 high,
3 moderate**. Audit totals count affected package entries, not distinct advisories.
The remaining chain is `tailwindcss@3.4.19` → `postcss-nested@6.2.0` /
`postcss-selector-parser@6.1.4`, with
[selector-parsing CPU exhaustion](https://github.com/advisories/GHSA-rj75-hqrm-r3gf).
The indicated parser fix is `>=7.1.6`; the installed parents require the older
major. npm proposes Tailwind `4.3.3`, a major migration. No Tailwind `3.4.20` was
available when checked. No unsupported parser override or forced framework/tooling
upgrade was applied. This remaining tooling remediation requires separate review.
CMS content is Markdown, not user-supplied CSS, but the dependency finding remains
open. Existing ESLint peer warnings and the existing Campaign Manager lint warning
were not hidden or changed.

Evidence: `scratch/milestone6d-stabilization-audit-{before,after}.json` and the
resolved dependency tree (`npm ls`).

## C. Actual regression results

| Gate | Current execution |
|---|---|
| Rust format | PASS, `cargo fmt --all -- --check`. |
| Rust Clippy | PASS, all targets with `-D warnings`. |
| Rust compile | PASS, `cargo check --all-targets`. |
| Rust library | **87 passed, 0 failed**. |
| Focused CMS integration | **14 passed, 0 failed**; existing tests strengthened, not counted twice. |
| Full serialized backend | First run: **378 passed, 1 failed, 4 ignored**. Independent repeat was **stopped at the user's request**; it is not a completed passing suite. |
| Cleanup case targeted retry | **1 passed, 0 failed**; separate retry, not a clean full-suite result. |
| Admin Web suite | **83 passed, 0 failed**: 75 native tests and 8 component tests. |
| Admin typecheck / ESLint / production build | PASS; lint has **0 errors, 1 existing Campaign Manager warning**. Next build uses `16.4.3`. |
| Patient Flutter analyze | PASS, no issues. |
| Patient Flutter tests | **308 passed, 0 failed**. |
| Current Admin interactive browser fixture | **11 passed**, zero page errors; synthetic API transport, not live authenticated acceptance. |
| Static actual-editor layouts | **3 passed**, 1440/768/390 px; no overflow, expected side-by-side/stacking. |
| Live unauthenticated smoke | **6 passed**, zero page errors, no API mocks. |
| Real seeded Admin sign-in | **Blocked: HTTP 401**; one attempt, no reset, extra account or authentication change. |

PostgreSQL/Redis regressions use owned loopback-only disposable fixtures
`hd-6d-postgres` / `hd-6d-redis`, confirmed by their `hd.fixture=milestone6d`
labels and Redis ownership marker. SQLx creates isolated databases; integration
tests run with `--test-threads=1`. No shared development database is flushed/reset.
In-memory encrypted object storage tests cover real service/HTTP/database behavior
without claiming a cloud provider round trip.

The first full run's sole failure was the cleanup test at
`tests/cases/admin_articles.rs:781`: `ServiceUnavailable("Database operation is
temporarily unavailable.")`. This error is emitted by the existing 250 ms
`begin_authed_as` connection-acquisition deadline, before cleanup assertions.
The fixture reaps role-pool connections after 100 ms idle. The initial suite
overlapped Rust library/image checks and build work; that resource-load explanation
is an inference, not a demonstrated cleanup defect. The case passed both in the
initial focused 14-test run and a separate retry after other builds stopped.
Production deadlines and storage cleanup code were not relaxed or changed.
Both the initial full-run failure and the independent full-repeat outcome are
retained in the evidence rather than replacing the failing log.
The independent repeat passed the 14 CMS cases, then was interrupted on the user's
explicit request to stop waiting. Its abnormal process exit records that intentional
termination, not a completed test verdict. A clean full-suite repeat remains unverified.

Logs and explicit native exit codes:
`scratch/milestone6d-stabilization-{rust-gates,focused-exit,full-exit,admin-gates,admin-exit,patient-gates}.json`
and adjacent `.log` files. The cleanup retry and independent full repeat use
`scratch/milestone6d-stabilization-cleanup-retry-exit.json` and
`scratch/milestone6d-stabilization-isolated-exit.json`. Browser evidence:
`scratch/milestone6d-stabilization-browser-interactive.json`,
`scratch/milestone6d-browser-layout.json`,
`scratch/milestone6d-stabilization-smoke.json`.

## D. Azure Blob integration — intentionally pending

**DEFERRED — AWAITING AZURE BLOB INTEGRATION.** This is the approved infrastructure
sequence, not a CMS implementation failure. No S3/MinIO workaround, provider
configuration, credential request, resource provisioning, Azure dependency or
storage-layer rewrite was performed. No real upload-to-cloud success is claimed.

The existing private storage adapter remains unconfigured. Article-media isolation
and authorized Patient cover reads pass in-memory regression fixtures. Live upload,
private-container permissions, managed identity and cloud cleanup still require
separate Azure implementation and staging verification.

See [the planning-only Azure document](AZURE_BLOB_INTEGRATION_PLAN.md) for SDK/REST
and existing `object_store` adapter evaluation; shared service boundaries; private
clinical/article containers; backend reads; encryption/identity; conditional writes;
ownership/deletion/cleanup; Azurite; environment requirements; and migration gates.
Local read-only inventory found zero references in the five checked media-key
columns and zero `article_media` rows. This does not establish that all environments
or historical external storage are empty.

## E. Real cross-app acceptance and remaining blockers

| Required sequence | What is actually verified / remaining |
|---|---|
| Admin signs in through actual Web/BFF | Login page and anonymous guards pass; approved seeded account sign-in returned 401. No retry/reset/bypass. |
| Create → save draft → local preview | Covered by backend, components and browser fixtures. Live authenticated walkthrough could not pass sign-in; no new live article/category records were created. |
| Upload cover | Validation, encrypted round trip, ownership and visibility pass isolated fixtures. Actual cloud step is deferred to Azure. UI truthfully reports unavailable upload. |
| Independent reviewer approves sensitive article | Real isolated HTTP/database test uses a separate authorized fixture reviewer; author remains pending on 403. Live development has only one active Admin. No additional live Admin was created. |
| Publish exact approved version | Covered by isolated lifecycle, concurrent-publication, version, scheduler and audit tests; not claimed from a live authenticated browser. |
| Same article appears and opens in running Patient Blogs tab | Existing Doctor/Admin feed/navigation/detail and EN/SW/cover behavior pass backend and Flutter tests. **No actual Patient runtime was available**, so same-record cross-app acceptance remains unverified. |
| Unpublished content inaccessible | Isolated API/RLS/media tests pass; unauthenticated live article/cover routes return 401. These are separate from a logged-in Patient runtime walkthrough. |

Flutter device inspection found no Android/iOS device/emulator. The project has no
web/Windows runner, so detected desktop/browser targets do not establish a usable
Patient runtime. No new platform scaffold was added and no device verification is
claimed. Real sign-in rejection and unavailable independent reviewer are additional
live-environment blockers; neither was silently attributed to Azure.

Admin development remains available at `http://127.0.0.1:3000`, backend at
`http://127.0.0.1:8080`. Canonical readiness confirmed source, running binary and
local development schema `20261127000001`. Production was not deployed.

**Acceptance gate: PARTIAL — stopped at the user's request.** The final regression outcome is recorded above;
complete real Admin → BFF → same persisted record → Patient runtime → cloud cover
acceptance is not claimed. Azure integration, live authentication/reviewer access,
Patient runtime availability and the remaining moderate tooling advisory are the
outstanding items. Work stops after authorized 6D stabilization.
