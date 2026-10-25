# Milestone 6E — optional Admin article MCQs

## Inspection before implementation

Current source was inspected again, including the dirty worktree and repository
instructions. No quiz/MCQ implementation exists; patient/doctor Q&A is unrelated.
Reusable: `ArticleEditor.tsx`, `ArticlePreview.tsx`, CMS styles, `blogApi.ts`, the
authenticated same-origin BFF, `admin_article_handlers.rs`, `admin_article_repo.rs`,
the 6D migration, canonical `ManageArticles` ABAC, transaction-local RLS context and
transaction-bound audit writer. Category sensitivity is immutable. Article edits
invalidate approval and return the article to DRAFT; Doctor publishing is separate.

MCQ edits therefore use separate draft/approved/immutable published quiz revisions
under the existing article UUID. They do not edit the article row or remove a live
article. Review binds quiz content version and article content version, including
independent Admin approval for sensitive content. Explicit quiz publication swaps
only the safe publication pointer/metadata. Inclusion defaults OFF; absent quiz
records and incomplete excluded question drafts never block article publication.

Planned additive schema: quiz revisions, questions, options and safe publication
metadata, with FORCE RLS, foreign keys, ordering/size constraints and immutable
published revisions. Admin-only question/answer-key APIs use the existing BFF.
Patient article projections expose only approved inclusion/revision/count metadata;
Flutter interaction and scoring remain 6F. No Patient Dart or Azure changes.

Planned files: migration `20261128000001_article_quizzes.{up,down}.sql`; Rust domain,
repository and Admin handlers for article quizzes plus module/router registration;
MCQ content/API helpers and modular Builder, QuestionEditor, Import and Preview
components; existing ArticleEditor/ArticlePreview/styles; focused Rust/Admin tests;
migration-status marker and milestone/checkpoint documentation.

Prior timeout inspected: `admin_articles::test_cms_abandoned_media_cleanup_preserves_current_and_clinical_objects`,
`tests/cases/admin_articles.rs:781`, the existing 250 ms DB acquisition deadline.
Fixture role pools reap idle connections after 100 ms. First full run was 378 passed,
1 timeout, 4 ignored; the individual retry passed and a subsequent repeat was stopped
at the user's request. Evidence remains in `scratch/milestone6d-stabilization-*`.
No new full-suite run is authorized by this milestone's focused verification plan.
Initial runtime check found neither port 3000 nor 8080 listening.

No architectural conflict requiring unrelated changes was found. Implementation
and verification results are recorded below.

## Implemented architecture and editorial workflow

MCQs are optional and default OFF. No quiz row is required to create, review,
schedule or publish an article. Excluded/incomplete question drafts are preserved.
Question saving does not update `admin_articles`, its status or its approval.

Each article has at most one editable quiz revision and any number of immutable
published quiz revisions. Revisions follow DRAFT → PENDING_REVIEW → APPROVED →
PUBLISHED, with return-to-draft and edit invalidation. A save increments the quiz's
optimistic version and content version, clears approval and binds the current
article content version. Sensitive-category/article quizzes require another Admin
who is neither the revision creator nor its latest substantive editor. Existing
article-body review independently preserves its original author restriction.

Quiz approval records reviewer, time and exact quiz content version. Publication
requires the parent article to be published and the article content version to
match. Enabling/disabling inclusion requires this same review path. Publication
atomically replaces only the quiz publication pointer/metadata. The previous
approved quiz stays visible while a replacement is drafted/reviewed. Parent locks
and expected revision/version checks serialize competing changes; published
questions/options cannot be mutated even through direct database writes.

Changing the article body preserves the original 6D lifecycle. An older quiz bound
to a different article content version is excluded from Patient metadata until
saved/reviewed/published against the replacement version. Article scheduling remains
unchanged; it never silently publishes a quiz. Quiz publication is an explicit
Admin action after the article is published. No notifications are sent.

## Database and RLS

Migration `20261128000001_article_quizzes.up.sql` adds:

- `article_quiz_revisions`: article FK, inclusion, creator/editor FKs, optimistic
  and content versions, bound article content version, review/publication metadata.
- `article_quiz_questions`: ordered bilingual text/explanations and one nullable
  correct-option reference (nullable only permits incomplete drafts).
- `article_quiz_options`: ordered bilingual choices; IDs unique within a revision.
- `article_quiz_publications`: current approved revision ID, article-version binding,
  inclusion and safe question count. No prompts, answers or explanations.

All four tables FORCE RLS. Only the existing canonical ADMIN context can author or
read the private answer-key tables. Patient/Doctor contexts may read only safe
publication metadata for a currently published, matching article revision.
No new production roles or system-worker quiz privileges. Composite FKs prevent
cross-question answer references and cross-article publication pointers. Deferred
constraints enforce contiguous order and 50-question/6-option caps. BEFORE triggers
lock the revision and reject changes outside DRAFT, protecting review/publication
against concurrent child changes. Published revisions are immutable. The paired
down migration removes only the new quiz objects; Doctor policies are untouched.

API operations audit in the same transaction using existing `AuditService`.
Events use `ARTICLE_QUIZ_SAVE`, `ARTICLE_QUIZ_IMPORT`, `ARTICLE_QUIZ_SUBMIT`,
`ARTICLE_QUIZ_APPROVE`, `ARTICLE_QUIZ_RETURN` and `ARTICLE_QUIZ_PUBLISH`; audit
snapshots contain IDs, versions, state, inclusion and counts, not answer keys.

## Backend APIs

All routes require authenticated ADMIN/`ManageArticles`, use transaction-local RLS
and the existing same-origin BFF. Base: `/api/v1/admin/articles/{article_id}/quiz`.

| Method/path | Behavior |
|---|---|
| `GET` base | Versioned `{schema_version:1,draft,published}` authoring document. |
| `PATCH` base | Atomic bounded replacement of a draft's questions/settings; creates a new draft when based on an immutable published revision. |
| `POST /import-preview` | Read-only parsing, row validation, duplicates against saved content and import preview; no content write. |
| `POST /import` | Reparse/revalidate and atomically append to the expected saved revision; inclusion is preserved and nothing publishes. |
| `POST /{revision_id}/{action}` | Explicit `submit`, `approve`, `return`, `publish`, with expected revision version. |

Save/confirm requests identify the source `revision_id` and `version`; both are null
for the first draft. Every revision lookup is scoped to its article. Request bodies
are capped at 1 MiB (1 KiB for transitions); file text at 128 KiB; total quiz text
at 256 KiB; prompts at 2000 characters, explanations at 4000, option text at 500.
Unknown fields, malformed UUIDs/choice references, duplicate IDs/questions, invalid
ordering and count violations are rejected. Excluded drafts may be incomplete;
included review submissions require 1–50 complete questions, 2–6 choices, one answer
and an explanation. Each question supports complete SW translation or EN fallback.

## Builder, import and preview

`McqBuilder`, `QuestionEditor`, `QuizImport` and `QuizPreview` extend the existing
Article Editor. There is no new dashboard/navigation or main `page.tsx` expansion.
The optional builder loads its private authoring document on demand. It supports
inclusion, question add/edit/delete/reorder, answer options/correct choice,
explanations, EN/SW switching, explicit draft saves and review/publication controls.
Unsaved question edits participate in the editor's leave-page warning.

The existing phone preview appends the optional simulated quiz after the article
body. Selection, correct/incorrect feedback, post-answer explanation, next-question
progress and completion/restart are local React state. Content edits reset the
simulation; language switching uses per-question fallback. No scoring/attempt,
notification or publication API is called by preview interactions. Prompt/choice/
explanation strings render as escaped text, with no raw HTML or media execution.

Import accepts strict UTF-8 CSV/JSON. Client file checks precede transport; backend
parsing and validation are authoritative. CSV supports BOM, CRLF, quoted commas,
quoted newlines and doubled quotes. Required columns: `text_en`, `option_a_en`,
`option_b_en`, `correct_option`, `explanation_en`; optional `text_sw`,
`explanation_sw`, and `option_{a..f}_{en,sw}`. CSV answer is A–F; records identify
logical row numbers including the header. Unknown/duplicate headers are rejected.
JSON uses `{schema_version:1,questions:[...]}`; each item has text/explanation fields,
an options array of text fields, and zero-based `correct_option`. The server
generates IDs. No IDs, quiz status or approval can be imported from a file.

Preview displays parsed questions and row/field errors. Import is all-or-nothing,
requires a separate confirmation, appends to saved content, and refuses stale bases.
Manual changes must be saved before import. Fixed CSV/JSON templates are provided;
CSV escaping also neutralizes leading spreadsheet-formula markers. No PDF/Word
extraction, question export, cloud upload or external parser dependency was added.

## Safe 6F contract and explicit limits

Ordinary published Admin article responses add only a `quiz` summary:

```json
{"schema_version":1,"included":true,"revision_id":"approved-revision-uuid","question_count":5}
```

Absent/excluded/stale quizzes resolve to no included quiz. Doctor responses remain
unchanged. No Patient response contains `correct_option_id`, explanations or the
private question document. The future 6F question-reading contract must return only
revision-bound prompt/choice IDs and localized text; server-authoritative evaluation
must bind article, revision, question and selected-option IDs. Answer/explanation
release belongs to that future evaluation flow, not ordinary article reads.

This milestone implements Admin authoring and simulated preview, not the Patient
Flutter renderer. **An actual Patient interactive quiz below the article requires
6F**. No Patient Dart, attempt/submission/scoring/progress records, sensitive-health
profiling, Azure implementation or campaign changes are included. Existing three
moderate tooling advisories remain outside this milestone. No cloud success or
live cross-app/device acceptance is claimed.

## Files added/modified

- New migration pair: `services/backend/migrations/20261128000001_article_quizzes.{up,down}.sql`.
- New Rust: `src/domain/article_quizzes.rs`, `src/repository/article_quiz_repo.rs`,
  `src/api/v1/admin_quiz_handlers.rs`, `tests/cases/admin_quizzes.rs` (under `services/backend`).
- Rust registrations/integration: `src/domain/mod.rs`, `src/repository/mod.rs`,
  `src/api/v1/mod.rs`, `src/api/router.rs`, `src/repository/admin_article_repo.rs`,
  `src/migration_status.rs`, `tests/api_tests.rs`.
- New Admin helpers: `apps/admin-web/src/lib/quizContent.ts`, `quizApi.ts`.
- New Admin components: `src/components/blogs/McqBuilder.tsx`, `QuestionEditor.tsx`,
  `QuizImport.tsx`, `QuizPreview.tsx` (under `apps/admin-web`).
- Existing Admin components: `ArticleEditor.tsx`, `ArticlePreview.tsx`, `blogs.css`.
- New Admin tests: `apps/admin-web/tests/quiz-content.test.ts`, `quiz-components.test.tsx`.
- Documentation: this report, `docs/CHECKPOINT.md`, `docs/PROGRESS.md`.
- Local render/layout harness and evidence: `scratch/milestone6e-*`.

No dependencies, actual storage configuration, existing migration history or Patient
source were changed. The worktree's prior legitimate changes remain preserved.

## Verification results

| Check | Actual result |
|---|---|
| Rust format | PASS, `cargo fmt --all -- --check`. |
| Rust Clippy | PASS, `cargo clippy --all-targets -- -D warnings`. |
| Rust compilation | PASS, `cargo check --all-targets`. |
| Rust library | **91 passed, 0 failed** (87 existing + 4 MCQ validation/parser cases). |
| Focused MCQ integration | **9 passed, 0 failed**; final strengthened direct-Admin immutability check uses the same nine-case module. |
| Existing CMS regression | **14 passed, 0 failed**, including Doctor/unified feed, approval, scheduling, media cleanup and migration compatibility. |
| Admin Web suite | **93 passed, 0 failed**: 79 native cases + 14 React component cases. |
| Admin typecheck | PASS. |
| Admin ESLint | PASS, 0 errors / 1 existing Campaign Manager warning. |
| Admin production build | PASS. Existing React 18/middleware deprecation notices remain. |
| Responsive actual-component layouts | **3 passed**, 1440/768/390 px; no document overflow and expected column stacking. |
| Patient Flutter | Not changed or rerun in 6E; prior 308-pass result remains historical. |
| Full backend | Not rerun; no claim of a GREEN complete regression. |

Native exit-code evidence: `scratch/milestone6e-rust-gates.json`,
`milestone6e-backend-gates.json`, `milestone6e-focused-verified-exit.json`,
`milestone6e-admin-gates.json`; corresponding logs are retained beside them.
Final focused log: `scratch/milestone6e-focused-verified.log`; CMS regression:
`milestone6e-cms-regression.log`; library: `milestone6e-library.log`;
Admin suite: `milestone6e-admin-tests.log`; layout evidence:
`milestone6e-layout.json` and three screenshots.

Development test corrections were resolved before the final verification: the audit
failure fixture now targets `resource_type`, producing its intended failure and
transaction rollback; the child-write guard explicitly rejects non-Admin contexts
before reading private revisions. The immutability test uses an authenticated
Admin transaction, ensuring denial comes from published-content protection rather
than missing authorization. Production acquisition deadlines, connection pools and
the existing 6D assertions were not weakened.

The historic full-backend result remains **378 passed, 1 DB timeout, 4 ignored**;
the stopped repeat is not a passing full regression. No new full-suite run was
started. A coordinated full-backend regression remains a separate verification gate.
The exact previously failed cleanup case passed in the current 14-case CMS run;
this does not substitute for a complete backend regression.

Only existing ownership-verified `hd-6d-postgres` / `hd-6d-redis` fixtures were used
(loopback 55436 / 16486, marked fixture Redis DB 14). SQLx databases were isolated
and module runs serialized; no shared development reset, migration application or
database destruction. Docker was initially unavailable, then its existing local
installation and owned fixtures became available; no infrastructure was installed.
No package/dependency installation or upgrades were performed.

**6E readiness:** Admin authoring/import/preview and approved publication metadata
are ready for the separately authorized 6F implementation. Full-backend release
regression remains pending coordination. Azure status remains **DEFERRED — AWAITING
AZURE BLOB INTEGRATION**. No actual cloud upload, Patient interactive runtime or
cross-app quiz acceptance is claimed. Local Admin/backend application ports 3000
and 8080 were not listening at final availability inspection; no claim that either
development server was started. Stop at 6E; do not begin 6F or Azure deployment.
