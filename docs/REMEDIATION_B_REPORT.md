# Remediation B: Admin session and editor correctness

**Verdict: GREEN for the scoped automated application gate.**

## Checkpoint and preservation

Started on `main`, commit `47078ee`, with 31 legitimate changed/untracked Remediation A files. Initial hashes were recorded under `scratch/remediation-b/initial-worktree.json`. Remediation A backend files, migrations, financial helpers and erasure behavior were preserved. Of its 31 initial changed files, 28 remain byte-for-byte identical; the existing page gained navigation integration and the two progress documents received appended entries. No staging, commits, push, reset, cleanup, deployment, Azure configuration, persistent-database mutation or dependency upgrades occurred.

The payment-provider requirement remains in `REMEDIATION_A_RECONCILIATION.md`: persist STARTED durably before contacting a real provider. Payment integration was not changed.

## Inspection and independent reproductions

| Finding | Actual path and reproduction | Root cause |
| --- | --- | --- |
| B1 | The production Next build returned 401 on JSON, private-cover and clinical-grant requests carrying a valid synthetic refresh cookie without access. | Each transport checked for access before entering its supported refresh branch. The noncredential shell marker also expired with access, despite refresh surviving seven days. |
| B2 | Through the real sidebar, editing article text then selecting Finance unmounted the editor without a dialog. | Sidebar and topbar selections set local page state directly. Editor Back and native unload protection did not govern this React navigation boundary. |
| B3 | A deferred import response replaced a newer local question edit and cleared its dirty flag. | Import busy state was local to QuizImport, while the parent could still edit/save and later applied the response unconditionally. |

The four initial browser assertions failed on these intended conditions. The delayed-import component assertion also failed before the fix. Additional refresh tests reproduced a late replica rotating an already-used cookie after the first leader released its lock, and an unhandled detached rejection after a transient failure.

Evidence: `browser-before.json`, `import-before.log`, `refresh-before.log` under `scratch/remediation-b/`.

## Session recovery

- `withAdminSession` in `session.ts` handles missing access and upstream 401 for JSON, cover, clinical-grant and diagnostic transports. A successful backend rotation sets cookies and retries the protected request once.
- The access lifetime remains 900 seconds and refresh remains seven days. HttpOnly, production Secure, SameSite=Strict and `/api` credential paths remain intact. Only the noncredential shell marker now follows the recoverable-session window.
- `/api/session` recovers an older/missing shell marker before redirecting to login. It validates access through an existing Admin-authorized backend read, then redirects to a fixed same-origin destination. It returns neither tokens nor upstream Admin data.
- Missing or authoritatively rejected refresh credentials return 401 and expire all cookies using their original security/path attributes. Backend rejection after rotation remains authoritative, including when a cached rotation is presented after logout.
- Backend/dependency failure and malformed refresh responses return a secret-free 502 without returning protected data or asserting revocation. A competing replica returns retryable 503 with a completion/race distinction rather than triggering the client's 401 login redirect.
- The existing same-process single flight and Redis fingerprint lock are retained. A short completion marker prevents a late replica reusing the old cookie immediately after release. The lock covers the bounded refresh request; configured Redis loss prevents independent rotation and can reconnect after recovery. An unconfigured single-instance development process retains local coordination.
- Rejected promises are handled without creating a detached rejected `finally` promise. The existing five-second result grace applies to transient errors too; retry recovery tests honor that window.
- CSRF/origin, ingress and backend authorization controls remain in place. Document-grant redemption remains single use; its bytes are streamed through the authenticated server path.

## Navigation safeguards

`NavigationGuard` is a reusable, in-memory boundary around the existing Admin page. Existing sidebar/topbar selections and editor Back use its request API. Article fields, category selection/tags, selected cover, schedule input, manual questions, quiz inclusion and unconfirmed import selection contribute unsafe state; category authoring also participates.

Stay preserves mounted editor state. Discard and Leave performs the requested navigation explicitly. During an unsafe pending operation, leaving is disabled until completion; a request that may already commit is not presented as safely cancelled. Normal draft save remains separate. Blank articles and reverted edits do not produce dirty warnings.

Native beforeunload handles reload/tab close. Same-document browser Back is intercepted through an opaque same-URL history checkpoint, then restored before confirmation. Regular same-window anchor navigation uses the same guard. The history marker contains an opaque identifier and existing router state, without article, question or credential data. Unsaved content stays in component memory and is not written to localStorage, sessionStorage, history or logs.

## Import concurrency

McqBuilder now owns a shared mutation gate spanning file read, preview, import, save, reload and review actions. The fieldset and callback guards block manual changes, inclusion changes, reordering/addition, duplicate confirmation, collapse and parallel saves while an operation is active.

QuizImport retains explicit preview/confirmation and file limits. Revision/article-generation checks reject obsolete preview/import responses and ignore completion after unmount. Changing the reviewed source invalidates its preview. Failed imports retain the selected file and existing questions, allowing manual edits afterward. Successful imports apply the intended server document, update the saved baseline and clear dirty state only after acknowledgement. Draft import selection can also be discarded explicitly.

Backend expected-version bodies, authoring schemas, approval rules, optional inclusion, immutable published revisions and Patient contracts are unchanged. No Patient quiz activity, publication or notification is created by preview/navigation.

## Mandatory cross-module gate

| Gate | Affected state and actual evidence |
| --- | --- |
| Local correctness | Missing/expired cookie recovery; valid/invalid/revoked refresh; form dirty tracking; explicit navigation consent; import validation/confirmation and response fencing. |
| System consistency | Backend-authority fixture, browser credential jar, shell marker and protected BFF responses agree; logout reaches the authority and cached rotation cannot restore it. Article and quiz local/saved baselines, import file/preview, pending state and root navigation agree. Existing review tests remain intact. |
| Failure recovery | Backend interruption, invalid payload, dependency loss/reconnection, concurrent/late refresh, revocation after rotation, failed draft save, delayed/failed import, duplicate mutations, obsolete response, unmount and native traversal/unload paths are exercised. |

Database historical reconciliation and financial ledger/outbox mutation are inapplicable here: no backend/database changes were made and no real session rows were repaired. Session/editor fixtures are synthetic; no real browser credentials were inspected. Backend RLS/ABAC, role pools, audit/outbox, encryption and production database timeouts remain unchanged.

## Verification actually executed

| Check | Result |
| --- | --- |
| Admin TypeScript `tsc --noEmit` | PASS |
| Admin ESLint | PASS: 0 errors; 1 existing Campaign warning |
| Full Admin automated suite | **126 passed, 0 failed**: 98 native tests + 28 component tests |
| Production `next build` | PASS |
| Production-build browser/BFF checks | **24 passed, 0 failed** in headless Microsoft Edge: 22 main checks + 2 focused authenticated-write/cover transport checks |
| Remediation A preservation | Initial backend/migration hashes match; its existing Admin tests remain in the passing suite |
| Rust/backend suite | Not rerun: no Rust/schema changes. Latest prior baseline remains 419 passed, 0 failed, 4 ignored; library 91 |
| Doctor/Patient apps | Unchanged; no new native/device test claim |

The two focused write checks additionally verify that a 401 retry preserves the JSON body, expected version and idempotency key with one authorized write, and that missing-access cover transport preserves its bounded binary payload. These use a synthetic HTTP authority/store and do not claim cloud upload or Rust image-decoder verification.

The browser harness uses the actual Next production build, cookie helpers, middleware, BFF handlers and rendered sidebar/editor, with a loopback synthetic HTTP backend and synthetic accounts. Coordinator tests use explicitly owned disposable Redis, including two independent coordinator instances and a separate process for loss/reconnection. They do not certify a deployed multi-replica topology or real Rust MFA/provider connectivity.

Intermediate evidence remains recorded. The first browser harness had invalid Playwright cookie arguments and an unsuitable dev-server loop; corrected reproductions used the production build. Component tests were updated to supply the new required navigation boundary and correct label queries, without weakening their assertions. Transient retries originally occurred inside the existing grace window; final tests verify recovery after it. The logout test was corrected to use browser fetch for Secure cookies on trusted loopback: APIRequestContext omits Secure cookies over plain HTTP and had not actually called backend logout. The final test explicitly requires that revocation call.

## Remaining limitations and operation

- Native unload prompts depend on browser support/user interaction. Edge traversal/tab-close paths pass; Safari, Firefox, mobile lifecycle and forced OS termination were not verified. Forced termination can lose local unsaved work because no persistent draft cache was introduced.
- An HTTP operation may complete after a forcibly closed tab. Reopen/reload the saved revision before retrying an uncertain mutation; existing backend optimistic concurrency remains authoritative.
- A losing replica gets a retryable 503 and must retry after the winning response updates browser cookies. No raw refresh tokens are shared through Redis. Losing a completed rotation response can require fresh sign-in; this task does not reconstruct missing credentials.
- Real deployment TLS/ingress, MFA, Azure, production Redis topology and device acceptance remain unverified. Browser checks use a production Next runtime with test exposure settings on loopback, including asserted Secure cookie attributes.
- Existing Campaign lint and framework deprecation warnings remain outside scope. Remediation C, Azure and GitHub actions were not started.

## Exact Remediation B files

- `apps/admin-web/src/app/api/article-media/[id]/route.ts`
- `apps/admin-web/src/app/api/diagnostics/route.ts`
- `apps/admin-web/src/app/api/documents/download/route.ts`
- `apps/admin-web/src/app/api/proxy/[...path]/route.ts`
- `apps/admin-web/src/app/api/session/route.ts`
- `apps/admin-web/src/app/page.tsx`
- `apps/admin-web/src/components/blogs/ArticleEditor.tsx`
- `apps/admin-web/src/components/blogs/BlogManager.tsx`
- `apps/admin-web/src/components/blogs/CategoryManager.tsx`
- `apps/admin-web/src/components/blogs/McqBuilder.tsx`
- `apps/admin-web/src/components/blogs/QuizImport.tsx`
- `apps/admin-web/src/components/navigation/NavigationGuard.tsx`
- `apps/admin-web/src/lib/refresh.ts`
- `apps/admin-web/src/lib/session.ts`
- `apps/admin-web/src/middleware.ts`
- `apps/admin-web/tests/blog-preview.test.tsx`
- `apps/admin-web/tests/browser/remediation_b.py`
- `apps/admin-web/tests/editor-concurrency.test.tsx`
- `apps/admin-web/tests/navigation-guard.test.tsx`
- `apps/admin-web/tests/refresh.test.ts`
- `docs/CHECKPOINT.md`
- `docs/PROGRESS.md`
- `docs/REMEDIATION_B_REPORT.md`
