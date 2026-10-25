# Milestone 6F — Patient interactive article quizzes

## Initial read-only inspection

Current Patient BlogsPage/BlogDetailPage, DTO/repository, authenticated ApiClient,
locale controller/ARB generation, PatientTokens/PatientCard, existing tests and
repository instructions were inspected again. The article flow uses local Stateful
widgets and a plain repository, not Riverpod. No Patient quiz UI exists. Article
responses contain 6E quiz publication metadata but the DTO currently ignores it.
The generic ApiClient already handles authentication/session generation and does
not log request bodies; quiz selections must not be sent to ClientErrorReporter.

The 6E revision/question/option/publication tables are present. Private answer-key
tables FORCE RLS and permit only Admin contexts. Quiz publication binds article
content version and immutable approved quiz revision. Existing Admin publishing
locks the article first. No architectural conflict was found.

Plan: additive migration with tightly scoped SECURITY DEFINER functions using the
existing migration-owner pattern, fixed search path, explicit Patient identity
checks and restricted execution grants. Direct Patient table access stays denied.
Read/check operations lock the current published article and publication, validate
version compatibility, and expose only safe prompts/options or the selected
question's authorized feedback. No attempt/result tables or approval changes.

Minimum changes: new Rust Patient quiz handler/module/routes/migration marker and
focused tests; Flutter quiz models/repository/local controller/widgets, article DTO
metadata and BlogDetailPage attachment, EN/SW ARBs/generated localization and tests.
Admin authoring, Doctor article behavior, notifications, storage, payments, Agora
and Firebase remain outside scope.

Prior full-run evidence remains in `scratch/milestone6d-stabilization-backend-full.log`:
`admin_articles::test_cms_abandoned_media_cleanup_preserves_current_and_clinical_objects`,
`tests/cases/admin_articles.rs:781`, 250 ms database acquisition timeout; fixture role
pools have a 100 ms idle timeout. Previous outcome: 378 passed, 1 failed, 4 ignored.
Both production/fixture settings and assertions remain unchanged. After all other
checks finish, run one full serialized backend suite against owned fixtures with a
35-minute cap, retaining its exact result; do not repeat automatically.

Initial ports 3000/8080 were not listening. Azure stays intentionally deferred.
Implementation, exact files and actual verification are recorded below.

## Patient APIs and database boundary

- `GET /api/v1/articles/{id}/quiz`: published article/revision IDs, schema version,
  inclusion, count and ordered bilingual prompts/options. No correct IDs,
  explanations, author/reviewer fields or draft content.
- `POST /api/v1/articles/{id}/quiz/check`: accepts `revision_id`, `question_id` and
  `selected_option_id`. Returns server correctness, the selected question's correct
  option ID and bilingual explanation, with article/revision/question binding.

Both endpoints require canonical `PatientRead` authorization and normal authenticated
sessions/device proof. They reuse the existing API-role transaction context and
Redis limiter: reads 60/minute and checks 120/minute per Patient, separate scopes,
60-second TTLs. Rate keys contain no article/question/choice or score. The check body
is capped at 1 KiB, unknown fields are rejected, and successful responses use
`Cache-Control: private, no-store`. Existing global admission/IP protection remains.

Migration `20261129000001_patient_article_quizzes.{up,down}.sql` adds only functions:
private `lock_patient_article_quiz`, plus `read_patient_article_quiz` and
`check_patient_article_quiz`. All use SECURITY DEFINER, `pg_catalog,pg_temp` search
path and explicitly qualified platform tables. Execution is revoked from PUBLIC;
only the API role receives read/check execution, not the locking helper. Functions
also verify the transaction's Patient role, valid active Patient account and current
article/quiz publication and approval/version compatibility.

These functions are the deliberate narrow projection/evaluation boundary under the
controlled migration owner, following the existing privileged-function pattern.
`row_security=off` fails closed if that owner lacks the required bypass capability;
the API role receives no bypass or broader table privileges. All 6E FORCE RLS
policies and Admin-only private-row access remain unchanged. Direct Patient queries
still cannot read complete answer keys. The down migration removes only these three
functions, preserving articles, questions and policies.

Article and publication locks follow the existing parent-first order. Read/check
transactions serialize with unpublish, body edits and quiz replacement. Eligibility
is checked at the locked transaction snapshot. Missing, OFF, draft, unapproved,
archived or stale article-bound quizzes return 404; a superseded submitted revision
returns 409; a foreign/mismatched question or option returns 422 without revealing
private content. No automatic publication or editorial-rule changes.

Question fields contain EN/SW text to support immediate locale changes without new
answer submissions. A question's SW text/options are emitted only when its complete
SW question/options/explanation group exists; otherwise all fall back to English.
Explanations remain hidden until the Patient checks that question.

## Flutter experience and local session

BlogArticleDto parses quiz publication metadata only for Admin articles, schema 1,
inclusion ON, valid revision ID and 1–50 questions. Missing/OFF/invalid metadata and
Doctor articles add no quiz section or quiz request. The existing BlogDetailPage
appends ArticleQuizSection after article content/references; no new tab or dashboard.

Components: ArticleQuizSection, QuizQuestionCard, QuizAnswerOption, QuizFeedback and
QuizCompletionSummary. They use PatientTokens, PatientCard, Material controls and
generated AppLocalizations. Flow: introduction/count → Start → ordered questions →
selection → Submit → correctness/explanation → Next → completion → Restart.
The selected state is accessible to screen readers, controls are at least 48 pixels,
feedback is a live region, layout wraps at narrow widths and large text sizes, and
indeterminate loading animation is omitted when reduced motion is requested.

ArticleQuizRepository uses the unchanged canonical ApiClient; business logic is in
a screen-local ChangeNotifier, matching the existing lightweight Blog architecture.
Initial questions are fetched on Start. The returned revision/count must match the
article's publication metadata before any question is displayed. No client answer
key or correctness calculation is used: counts increment only on accepted server
feedback. Busy/feedback guards prevent duplicate submissions; failed requests retain
the selection without incrementing results. Async completions after disposal are
ignored.

Attempted/correct/incorrect counts and completion percentage exist only in memory.
Restart clears them and rereads the current quiz; leaving the screen disposes them.
No Patient attempt/profile/score tables, cache, token-store quiz state or analytics
events. Read/check code contains no answer-selection logging, audit payload or
notification operation. Authentication metadata and short-lived abuse counters are
the existing security mechanisms, not assessment histories.

EN/SW controls follow the existing Patient locale. Questions, choices and already
authorized feedback switch locally without duplicate checks; incomplete SW uses
complete English educational content. Superseded/withdrawn responses clear the local
quiz and scores. Refresh reloads the whole article, ensuring a replacement quiz is
not shown beneath an outdated article body. Offline, malformed, authorization,
rate-limit and answer failures have localized error/retry states.

## Exact files changed

- New migration pair: `services/backend/migrations/20261129000001_patient_article_quizzes.{up,down}.sql`.
- New handler: `services/backend/src/api/v1/patient_quiz_handlers.rs`.
- Registration: `services/backend/src/api/v1/mod.rs`, `src/api/router.rs`,
  `src/migration_status.rs` (under `services/backend`).
- Tests: new `services/backend/tests/cases/patient_quizzes.rs`; module registration
  in `tests/api_tests.rs`; only shared test-helper visibility in
  `tests/cases/admin_quizzes.rs` (no assertions weakened).
- New Flutter files under `apps/mobile_patient/lib/features/patient/blogs/`:
  `quiz_models.dart`, `quiz_repository.dart`, `quiz_controller.dart`,
  `quiz_widgets.dart`, `article_quiz_section.dart`.
- Existing Flutter files in that directory: `blogs_models.dart`,
  `blogs_repository.dart`, `blog_detail_page.dart`.
- `apps/mobile_patient/lib/l10n/app_en.arb`, `app_sw.arb` and generated
  `app_localizations.dart`, `app_localizations_en.dart`, `app_localizations_sw.dart`.
- New `apps/mobile_patient/test/patient_quiz_test.dart`.
- Documentation: this report, `docs/CHECKPOINT.md`, `docs/PROGRESS.md`.
- Safe local verification evidence/scripts: `scratch/milestone6f-*`.

No dependencies, Admin Web code/contracts, Doctor Flutter, payments, consultations,
Agora, Firebase, storage configuration or existing migration history were changed.
No shared-development migrations/reset or production deployment. Legitimate dirty
worktree changes were preserved; no Git stage/commit/push/reset/clean.

## Verification and acceptance

| Check | Completed result |
|---|---|
| Rust formatting | PASS, `cargo fmt --all -- --check`. |
| Rust Clippy | PASS, all targets with `-D warnings`. |
| Rust compilation | PASS, `cargo check --all-targets`. |
| Rust library | **91 passed, 0 failed**. |
| Patient quiz integration | **10 passed, 0 failed**. |
| Admin quiz regression | **9 passed, 0 failed**. |
| Article/CMS regression | **14 passed, 0 failed**. |
| Patient Dart formatting | PASS on changed files only. |
| Patient analysis | PASS, no issues. |
| Full Patient Flutter suite | **326 passed, 0 failed** (308 previous + 18 new), including the strengthened existing-Blogs-tab navigation and below-body position assertion. |
| Admin Web | Not rerun: no Admin source or authoring interface changed. Historical 93-pass result is not counted as a fresh 6F execution. |
| Full serialized backend | **398 passed, 0 failed, 4 ignored** (402 total), native exit **0**, one run in **1143.61 seconds**, below the 2100-second cap. |

Completed native exit-code evidence: `scratch/milestone6f-rust-gates.json`,
`milestone6f-backend-gates.json`, `milestone6f-flutter-acceptance-gates.json` and adjacent
logs. The initial Flutter compile/test/style findings were corrected: the test uses
the binding's surface-size API and waits for scroll layout before tapping; UTF-8
loading punctuation is preserved; new-file brace lints are fixed. None of these
changes weaken assertions or application timeouts.

The full-run wrapper `scratch/milestone6f-full-backend.py` checks completed
prerequisites, fixture labels/Redis marker, refuses overlapping integration
executables and a second attempt, and records its deadline/native exit code in
`milestone6f-full-backend.json`. Exact output is retained in
`milestone6f-full-backend.log`. The prior failing 6D full-suite evidence remains
intact; focused passes do not erase it. The full run completed once with no timeout.
The previous acquisition failure did not recur. Neither the 250 ms production
deadline nor 100 ms fixture idle setting was changed. The four ignored cases are
the existing explicit A/B admission-capacity benchmarks; they are not regular tests
silently skipped for this milestone. Focused integration counts are subsets of the
full suite and are not additional passes to sum into its total.

The final strengthened Flutter navigation test was run after the backend process
finished, avoiding competing load. Its native format/analyze/test exits are all 0;
log: `scratch/milestone6f-flutter-acceptance.log`. No further backend run was started.

Backend acceptance fixtures use real Admin article and quiz create/review/publish
handlers, including independent review for sensitive content, then real Patient
auth/HTTP handlers, SQL/RLS/functions and answer checking. Flutter tests verify
opening BlogDetailPage, quiz below article, selection/feedback/explanation, completion
and restart, plus OFF/Doctor behavior. The repository contract test exercises the
unchanged ApiClient with authenticated transport headers. This is layered automated
acceptance, not a claim of one live cross-app/device run on the same persisted row.

| Required acceptance step | Automated evidence |
|---|---|
| Admin publishes approved educational article | Real backend Admin lifecycle, including independent review in the sensitive-content acceptance fixture. |
| Admin publishes approved quiz, inclusion ON | Real Admin quiz lifecycle and immutable publication; Patient metadata confirms inclusion. |
| Patient opens article through existing Blogs tab | Existing navigation regression plus the 6F widget acceptance flow into BlogDetailPage. |
| Quiz appears beneath article | Real article-detail widget tree and below-body position assertion. |
| Patient selects/submits an answer | Widget selection/submit flow and canonical ApiClient contract test. |
| Backend checks the answer | Real authenticated SQL/RLS/function tests for correct/incorrect choices and membership/version checks. |
| Feedback/explanation | Backend bilingual feedback and localized Flutter rendering tests. |
| Completion summary | Flutter counts attempted/correct/incorrect and completion percentage from accepted server feedback. |
| Restart | Clears local state and revalidates current revision; no persisted attempt. |
| Inclusion OFF shows no quiz | Real backend 404 isolation and Flutter normal-article/no-fetch tests. |

Runtime limitation: no Android/iOS device was connected in discovery; only Windows
and Edge targets were detected and this app has no corresponding desktop/web runner.
No new runner, emulator, account or platform scaffold was created. Application ports
3000/8080 were initially stopped; no service availability is claimed without a fresh
check. Real Admin browser → persisted article/quiz → actual Patient runtime remains
unverified. Azure remains **DEFERRED — AWAITING AZURE BLOB INTEGRATION**; no cloud upload
acceptance, Azure provisioning/configuration or GitHub migration is included.

**Final verdict: GREEN for the required automated 6F checks.** Live cross-app/device
acceptance remains unverified as described above. No production readiness, physical
device verification, benchmark performance or cloud storage acceptance is claimed.
Local progress intentionally resets when leaving/restarting the quiz; no long-term
learning history is implemented. Previously reported tooling advisories remain
outside this milestone and were not re-audited. Stop after 6F.
