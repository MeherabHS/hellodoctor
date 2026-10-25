# Milestone 6D — Admin Blog/CMS

Date: 2026-10-25. Implemented Admin articles, editorial review, private cover media,
live Patient preview and integration with the existing Patient Blogs tab. All requested
regression suites passed. Final acceptance gate: **PARTIAL**, because a live cross-app
walkthrough and real external object storage remain unverified, and the existing
Next.js security advisory is unresolved.

> Subsequent stabilization: see [the current stabilization report](MILESTONE_6D_STABILIZATION_REPORT.md).
> Azure Blob is the approved permanent provider; earlier S3-compatible acceptance
> references below are historical and superseded. Cloud verification is
> **DEFERRED — AWAITING AZURE BLOB INTEGRATION**. Next.js dependency patches and
> independent-approval UX results are recorded in the newer report.

## Inspection and architecture assessment

Read-only inspection preceded all implementation. The worktree already contained
159 changed/untracked entries, including substantial legitimate Milestones 0–6C
work. No Git staging, commit, reset, cleanup, archive or deployment was performed.

Inspected current code rather than treating older milestone conclusions as current:

- Doctor article domain/repository/handlers/routes:
  `services/backend/src/domain/articles.rs`, `repository/article_repo.rs`,
  `api/v1/article_handlers.rs`, `api/router.rs`.
- Doctor schema/RLS: `20261025000001_doctor_articles`, canonical identity/admin
  migrations, `20261112000001_profile_pictures`, and the Milestones 4–5 worker grants.
  Doctor direct publication, archived terminal state, verification requirements,
  UUID routes and existing notification integration must remain unchanged.
- Patient: all four existing files in
  `apps/mobile_patient/lib/features/patient/blogs/`, existing navigation,
  `core/network/api_client.dart` and `core/l10n/locale_controller.dart`.
- Admin: `apps/admin-web/AGENTS.md`, installed Next.js route-handler documentation,
  existing sidebar, `src/lib/api.ts`, cookie/session/refresh middleware and JSON BFF,
  reusable panel conventions and Campaign Manager structure.
- Storage/upload: `DocumentStorageService`, `UploadSecurityService`, existing
  S3-compatible `AppState.object_storage`, image decoder limits, rate limiting,
  profile-photo byte transport and isolated in-memory storage test adapter.
- Scheduler/auditing/testing: `notification_scheduler.rs`, `AuditService`, ABAC,
  transaction-local role pools, reversible migrations and serialized SQLx tests.
- Repository instructions: `CLAUDE.md`, relevant authentication, schema, API,
  testing, architecture, roadmap and checkpoint documentation.

No saved 6A inspection artifact or newer recorded 6C full-suite result was found.
The latest documented full backend result was 349 passed/4 ignored, preceding the
current campaign code. The actual pre-CMS backend inventory was 369 tests, including
4 explicit ignored benchmarks; that inventory is not represented as a full test run.
With the required owned Redis fixture, pre-CMS Admin tests passed 66/66. The
previous documented Patient baseline was 295 tests; fresh results appear below.

There was no conflicting CMS. The approved separate `admin_articles` architecture
fits the repository. Missing capabilities were Admin editorial tables/APIs/UI,
bilingual categories, safe Markdown, private cover media, unified feed projection
and local live preview. No Doctor table or policy was changed. No external CMS,
second Patient tab, new scheduler, MCQ feature or campaign-delivery rewrite was added.

Minor adaptations:

- English content is required; Kiswahili title/body are optional together and fall
  back together to English. No duplicate summary field is stored for Admin articles.
- Returned/rejected articles use DRAFT plus an audited `return` action rather than
  introducing another state. Unpublishing also returns to DRAFT and clears approval.
- JPEG/PNG use existing supported decoders. WebP is deferred because the repository
  does not enable that decoder. Covers normalize to JPEG.
- Published article assets remain behind the existing authenticated Patient API.
  Draft assets are available only through canonical Admin authorization.
- A narrowly scoped binary BFF route handles covers; the existing JSON proxy remains
  unchanged. A five-second budget bounds the added scheduler pass.
- The current fixed Admin sidebar prevented a usable 390 px editor. A CMS-only
  small-screen shell class compacts the same navigation into a horizontal strip;
  other Admin areas, including Campaign Manager, keep their existing layout.

## Implementation and exact source files

New backend files:

1. `services/backend/migrations/20261127000001_admin_articles.up.sql`
2. `services/backend/migrations/20261127000001_admin_articles.down.sql`
3. `services/backend/src/domain/admin_articles.rs`
4. `services/backend/src/repository/admin_article_repo.rs`
5. `services/backend/src/api/v1/admin_article_handlers.rs`
6. `services/backend/src/services/article_media_service.rs`
7. `services/backend/tests/cases/admin_articles.rs`

Existing backend files extended:

1. `services/backend/src/abac.rs`
2. `services/backend/src/api/router.rs`
3. `services/backend/src/api/v1/article_handlers.rs`
4. `services/backend/src/api/v1/mod.rs`
5. `services/backend/src/domain/mod.rs`
6. `services/backend/src/repository/mod.rs`
7. `services/backend/src/services/mod.rs`
8. `services/backend/src/services/notification_scheduler.rs`
9. `services/backend/src/migration_status.rs`
10. `services/backend/tests/api_tests.rs`

New Admin Web files:

1. `apps/admin-web/src/components/blogs/BlogManager.tsx`
2. `apps/admin-web/src/components/blogs/CategoryManager.tsx`
3. `apps/admin-web/src/components/blogs/ArticleEditor.tsx`
4. `apps/admin-web/src/components/blogs/ArticlePreview.tsx`
5. `apps/admin-web/src/components/blogs/SafeMarkdown.tsx`
6. `apps/admin-web/src/components/blogs/blogs.css`
7. `apps/admin-web/src/lib/blogContent.ts`
8. `apps/admin-web/src/lib/blogApi.ts`
9. `apps/admin-web/src/app/api/article-media/[id]/route.ts`
10. `apps/admin-web/tests/blog-content.test.ts`
11. `apps/admin-web/tests/blog-preview.test.tsx`

Existing Admin Web files extended:

1. `apps/admin-web/src/app/page.tsx` — sidebar registration, composed view and CMS shell class.
2. `apps/admin-web/src/lib/api.ts` — exports the existing authenticated request helper.
3. `apps/admin-web/package.json`
4. `apps/admin-web/package-lock.json`

New Patient Flutter files:

1. `apps/mobile_patient/lib/features/patient/blogs/article_content.dart`
2. `apps/mobile_patient/test/admin_articles_test.dart`

Existing Patient Flutter files extended:

1. `apps/mobile_patient/lib/features/patient/blogs/blogs_models.dart`
2. `apps/mobile_patient/lib/features/patient/blogs/blogs_repository.dart`
3. `apps/mobile_patient/lib/features/patient/blogs/blogs_page.dart`
4. `apps/mobile_patient/lib/features/patient/blogs/blog_detail_page.dart`
5. `apps/mobile_patient/pubspec.yaml`
6. `apps/mobile_patient/pubspec.lock`

Documentation: this report and additive milestone entries in `docs/PROGRESS.md`
and `docs/CHECKPOINT.md`. Existing unrelated changes remain in the worktree.
Scratch logs, a synthetic SSR fixture, a local layout-check script, HTML, screenshots
and JSON layout evidence are under `scratch/milestone6d-*`.

## Database, RLS and transaction boundaries

Migration `20261127000001` adds `article_categories`, `admin_articles` and
`article_media`. All use ENABLE/FORCE RLS. Names have case-insensitive bilingual
unique indexes. Articles reference stable category UUIDs and Admin user UUIDs.
A composite foreign key ensures an attached cover belongs to that article.

Canonical ADMIN can manage articles and categories. Patient/Doctor reads require
PUBLISHED plus the exact approved content version. Patients cannot SELECT drafts
or write articles/media. Draft media is Admin-only; published media must be the
current ready cover of a visible published article. Storage keys/hashes never
appear in Patient JSON.

Worker permissions permit scheduled-to-published promotion and abandoned-media
cleanup. Cleanup row locking has a specific UPDATE policy with WITH CHECK(false),
which allows the required lock without permitting an actual worker media update.
The existing transaction-bound audit writer requires audit SELECT/INSERT grants.
Article publication, its audit row and optimistic version check share one transaction.

Triggers preserve article identity, guard lifecycle transitions and invalidate
approval/scheduling after substantive edits. A tightly scoped SECURITY DEFINER
insert trigger checks Doctor IDs without exposing Doctor drafts or changing their
RLS. Its search path is fixed and PUBLIC execution is revoked. Category review
classification is immutable; renaming remains supported, and referenced categories
cannot be deleted.

Indexes cover published feeds, Admin listing/status/category, scheduled publication
and old media cleanup. The down migration removes only the additive CMS structures
and grants. Rollback/reapply and Doctor-policy preservation are tested.

## Endpoints

All paths below are under `/api/v1`; existing authentication is retained.

| Method | Path | Behavior |
|---|---|---|
| GET/POST | `/admin/article-categories` | List/create bilingual categories |
| PATCH/DELETE | `/admin/article-categories/:id` | Rename/delete unused categories |
| GET/POST | `/admin/articles` | Search/filter/paginate or create draft |
| GET/PATCH | `/admin/articles/:id` | Read/edit with optimistic version |
| POST | `/admin/articles/:id/submit` | Submit DRAFT for review |
| POST | `/admin/articles/:id/approve` | Approve exact content version |
| POST | `/admin/articles/:id/return` | Return review to DRAFT |
| POST | `/admin/articles/:id/publish` | Publish APPROVED content |
| POST | `/admin/articles/:id/schedule` | Schedule APPROVED content in UTC |
| POST | `/admin/articles/:id/unpublish` | Return approved/scheduled/published article to DRAFT |
| POST | `/admin/articles/:id/archive` | Terminal archive |
| GET/POST | `/admin/articles/:id/cover` | Authorized draft cover read/upload |
| GET | `/articles`, `/articles/:id` | Existing endpoints, now unified published content |
| GET | `/articles/:id/cover` | Current published cover through authenticated API |

Creation accepts a client-generated UUID; a duplicate returns conflict and the UI
can recover the saved draft. Mutation requests carry `version`; schedules also carry
`scheduled_for`. Cover POST sends bounded image bytes and a `version` query parameter.
The Admin binary BFF is `/api/article-media/:id`, with the same HttpOnly session,
refresh coordination, origin protection and backend authorization conventions.

## Editorial workflow and scheduling

`DRAFT → PENDING_REVIEW → APPROVED → PUBLISHED`, with APPROVED → SCHEDULED →
PUBLISHED for future publication. Return/unpublish produces DRAFT; ARCHIVED is terminal.

Every mutation advances the optimistic row version. A substantive content, category,
tag, featured, sensitivity or cover change also advances `content_version`, returns
the article to draft and clears approval and scheduling. Approval records reviewer,
timestamp and exact content version. Unchanged saves preserve existing approval.

Article sensitivity and category review requirement default to true. Either requires
a reviewer different from the original author; a sole author cannot bypass the
pending-review state. Clinical classification is explicit editorial metadata, not an
automated medical-content classifier. Non-sensitive platform material can use an
explicitly non-sensitive category and article flag.

Schedules are stored as TIMESTAMPTZ UTC. The Admin picker interprets time in
Africa/Nairobi, UTC+3. The existing 60-second scheduler claims bounded due articles
with row locks/SKIP LOCKED and exact approval checks. Promotion/audit are atomic and
idempotent. A five-second CMS maintenance budget protects existing scheduler passes.
No second scheduler or queue is added.

Neither preview nor publication invokes NotificationService, creates a notification,
creates an announcement campaign or calls FCM. Existing Doctor publication and
Milestone 6C campaign behavior remain intact.

## Admin Manager and live preview

Blog Manager supports paginated listing, title search, status/category filtering,
language availability, author identity, dates, featured status and pending-review
indicators. Categories support bilingual create/rename/delete with duplicate and
foreign-key protection. Opening an article exposes the modular editor and its
review/publication controls.

The editor has EN/SW tabs, Markdown, category, tags, sensitivity, featured flag,
bounded cover input, draft saving, scheduling and version-aware lifecycle controls.
The phone preview renders local unsaved state immediately, with its own language
switcher, cover, category, publisher, publication metadata and formatted Markdown.
It clearly says preview, never saves per keystroke and does not create public records.
Archived articles are read-only. Save failures retain local text; stale versions can
reload the saved article. Panels stack below 1050 px.

The preview approximates the Patient detail screen; it is not claimed to be pixel-
perfect Flutter rendering. Installed-Edge static-render layout checks passed at
1440, 768 and 390 px, with screenshots and no horizontal overflow. These are layout
checks of actual components, not authenticated live-server E2E evidence.

A production build served only on `127.0.0.1` also passed ten interactive browser
checks with synthetic API responses and zero page exceptions: local typing/escaping,
language changes, exact save payloads, explicit review/publication actions, local
cover preview, explicit upload, the three viewport layouts and removal of CMS shell
styling when Campaign Manager opens. This verifies browser behavior, not real backend
authorization or cross-app persistence; real HTTP/RLS tests cover those backend rules.

## Markdown and cover safety

Admin uses `react-markdown` with raw HTML skipped, an explicit element allowlist,
no embedded images and safe HTTP(S)-only links. React escapes text; there is no
dangerouslySetInnerHTML, raw HTML plugin or unrestricted WYSIWYG editor.

Patient uses the maintained [`flutter_markdown_plus`](https://pub.dev/packages/flutter_markdown_plus)
renderer, which does not execute HTML. Embedded images are disabled, including
file/resource/data paths. Link callbacks accept only explicit HTTP(S), reject
credentials/control characters/backslashes and launch through the existing URL
launcher. Doctor article bodies remain plain text.

Covers use the existing private object provider and encryption configuration in a
separate `article-media/<generated UUID>.jpg` namespace. The clinical document
service, its 40 KB prescription limit and clinical read paths are unchanged.

- Maximum input 5 MiB; actual JPEG/PNG magic must match MIME.
- Header dimensions are checked before decode: at most 8192 per edge and 16 MP;
  decoder allocation is bounded at 128 MiB.
- Two bounded blocking decoder permits plus per-account upload rate/concurrency
  guards prevent unbounded processing. No DB transaction is held during decoding.
- Re-encoding strips image metadata, limits the long edge to 1600 px and optimizes
  toward a maximum 500 KiB output. Small images are not padded to 200 KiB.
- Object writes use create-only mode; UUID paths cannot traverse or overwrite
  arbitrary clinical objects. Encrypted bytes are checked against expected size/hash.
- A durable upload intent precedes the object write. Crashes, cancellation and stale
  saves leave cleanup candidates rather than untracked uploads. Cleanup retries old
  unreferenced media and preserves current covers and clinical objects.
- Responses are binary, `private, no-store`, JPEG and `nosniff`. No base64 image is
  embedded in article JSON; local file preview data stays only in the browser.

No real object-storage credentials were invented. Tests use the existing explicit
in-memory fixture; an unconfigured provider fails without confirming upload success.

## Patient compatibility

One existing Blogs tab lists both sources. Doctor UUIDs, routes, fields, references,
verification/RLS and existing limit/offset timestamp+ID ordering are retained. The
combined feed applies pagination once across both sources. New metadata is additive:
article type, content format, bilingual fields, category, tags, featured and cover ID.

The DTO handles legacy Doctor responses, attributes Admin content to AfyaMkononi
Editorial, selects whole EN/SW translations with English fallback and rejects draft
status responses defensively. Covers use existing authenticated `ApiClient.getBytes`
and generated article routes, not arbitrary response URLs or disk caches. Detail
rendering adds only the required Markdown/metadata/cover behavior. Existing navigation
and Doctor plain-text reading remain unchanged.

## Verification results

| Gate | Actual result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --all-targets -- -D warnings` | PASS, zero warnings |
| `cargo check --all-targets` | PASS |
| `cargo test --lib` | 87 passed, 0 failed |
| Focused real HTTP/SQL/RLS CMS integration | 14 passed, 0 failed |
| Full serialized backend integration | 379 passed, 0 failed, 4 existing benchmarks ignored; native exit 0 |
| Admin `tsc --noEmit` | PASS |
| Admin ESLint | 0 errors; 1 existing Campaign Manager warning; 0 CMS warnings |
| Admin tests | 76 passed: 72 native TypeScript tests + 4 component tests |
| Admin production build | PASS on retry; existing Next.js advisory/deprecations remain |
| Patient `flutter analyze` | PASS, 0 issues |
| Patient full `flutter test` | 308 passed, 0 failed; includes 13 new CMS tests |
| Installed-Edge static layout checks | 3 passed: 1440/768/390 px |
| Built Admin interactive browser checks | 10 passed, 0 page exceptions; synthetic transport |

CMS integration cases are included in the full backend suite, not additional unique
tests to add to its count. Rust library tests include three new cover-security cases.
No existing lockfile package was upgraded or removed when adding CMS dependencies.
The full backend run completed in 1228.15 seconds and includes all 16 existing
campaign tests. Its 383 cases replace the old baseline assumption: before the 14 CMS
cases were added, current source contained 369 cases, including the four benchmarks.
Expected negative tests emit audit-failure/pressure 5xx logs; the final failure count
is zero. Native exit codes were captured separately from PowerShell stderr records.

Evidence: `scratch/milestone6d-backend-full.log`,
`milestone6d-focused-backend-verified.log`, `milestone6d-rust-{format,clippy-final,check,lib}.log`,
`milestone6d-admin-{tests,typecheck,lint,build-responsive-final}.log`,
`milestone6d-patient-{analyze,tests,focused}.log`,
`milestone6d-browser-layout.json` and `milestone6d-browser-interactive.json`.
Native status evidence: `milestone6d-backend-exit-code.json`,
`milestone6d-rust-gate-codes.json` and `milestone6d-admin-test-exit-code.json`.

Owned isolated fixtures: `hd-6d-postgres` on loopback 55436 and `hd-6d-redis` on
16486, labelled `hd.fixture=milestone6d`. Redis DB 14 has the required exact ownership
marker. SQLx creates independent disposable databases, and integration tests run
serially. The initial fixture needed the repository's hardcoded `hellodoctor`
bootstrap database; creating it inside this owned container resolved migration setup.
Shared development PostgreSQL/Redis and their data were not reset or migrated.
After verification, only these two owned fixture containers were stopped; they were
not deleted. The temporary loopback preview server was stopped. Shared services
were left running.

## Defects found and fixed

1. Worker cleanup row locks required a narrowly scoped UPDATE privilege/policy even
   though the pass only deletes abandoned media. Added a locking-only policy.
2. Synthetic binary-upload requests lacked the test harness's peer extension and
   failed at the real perimeter guard. Fixed the fixture instead of weakening auth.
3. Existing Patient detail header overflowed at 320 px. Wrapped it in Expanded and
   removed redundant Editorial specialty text from Admin attribution.
4. Loaded Admin DTO metadata could leak into strict edit payloads. Explicit content
   projection now sends only editable fields, with a component assertion.
5. A guessed Doctor UUID could collide with an Admin-supplied UUID under caller RLS.
   Added a fixed-path, narrow identity-check trigger without changing Doctor policies.
6. New local lint issues were corrected. Existing Campaign Manager warning remains.
7. Replaced the initially considered discontinued Flutter Markdown package with its
   maintained continuation; only the latter remains in the final manifest/lockfile.
8. The standalone editor layout passed while the existing fixed Admin sidebar
   squeezed the integrated 390 px screen. Added only CMS-scoped responsive shell
   styles and verified navigation returns to its original layout outside CMS.

## Outstanding issues and acceptance boundary

- Existing Next.js 16.4.0 generated a security advisory and stopped the first build.
  Retrying completed the build; the dependency advisory remains. The install audit
  reported five affected dependency entries (three moderate, one high, one critical).
  A reviewed security patch upgrade is recommended separately; no broad framework
  or campaign rewrite was performed.
- The initial preview-server command was rejected by automatic approval review.
  Binding explicitly to loopback with the existing configuration succeeded; the
  resulting synthetic-transport browser walkthrough and static checks passed.
- Real configured S3/compatible storage, IAM/bucket configuration and a deployed
  authenticated browser-to-backend-to-device walkthrough remain external verification.
  Private-provider success is tested with the explicit in-memory adapter, not claimed
  against a real external object store.
- Full Admin-to-Patient E2E acceptance is not claimed from separate mocked component,
  real backend HTTP/SQL/RLS and Flutter widget tests. The exact unverified step is an
  authenticated live Admin browser publishing through its BFF followed by an actual
  Patient runtime reading that same persisted record and cover.
- No MCQs, advanced Patient browsing/search, learning progress or automatic campaigns
  are included. Existing shared backend processes were not restarted or deployed.

## End-to-end acceptance check and final gate

| Acceptance step | Verification |
|---|---|
| Admin creates an article | Real authenticated backend HTTP/SQL test |
| Admin saves/edits draft | Real backend test and browser/component payload checks |
| Admin previews unsaved content | Built browser checks: immediate local preview and no write per keystroke |
| Required independent approval | Real backend test: author's approval rejected; another Admin succeeds |
| Admin publishes exact approved content | Real backend lifecycle, concurrent-publish and audit-rollback tests |
| Patient sees and opens in existing Blogs tab | Real unified backend feed/detail checks plus Flutter mixed-feed/navigation widgets |
| One live Admin browser → BFF → persisted record → running Patient app | **UNVERIFIED** |
| External S3-compatible provider round trip | **UNVERIFIED**; explicit encrypted in-memory adapter round trip passed |

The complete sequence is implemented and covered across API, component, browser and
Flutter tests. Those checks do not prove a single live cross-app walkthrough against
the same persisted record. The remaining exact step is an authenticated Admin browser
publishing through the real BFF and an actual Patient runtime opening that record and
cover. Apply the additive migration/restart a reviewed target backend before that
walkthrough; this session did not migrate or restart shared development or production.

**Final gate: PARTIAL.** Automated regression gates passed. The live cross-app/provider
acceptance checks and existing dependency advisory remain open. Work stops at 6D;
Milestones 6E/6F and production deployment have not begun.

## Subsequent user-requested development startup — 2026-10-25

After the isolated implementation verification above, the user requested the live
Admin panel for testing. Started Next.js development on `http://127.0.0.1:3000`
and the Rust backend on `http://127.0.0.1:8080`, using existing development settings.
Normal backend startup additively applied the sole pending CMS migration to the local
development database. The canonical readiness script confirmed source, running binary
and database all match `20261127000001`. No database reset or production deployment.

Live smoke checks used no API mocks: login page HTTP 200, unauthenticated root redirect
to login, visible email/password fields, protected CMS BFF returning 401 and zero page
exceptions. Backend health is ready, and Node can reach the configured backend URL.
Evidence: `scratch/milestone6d-live-startup-smoke.json`. Both app servers were left
running for the user. Object storage remains unconfigured; authenticated CMS publication
and a real Patient walkthrough are still separate unverified acceptance steps.
