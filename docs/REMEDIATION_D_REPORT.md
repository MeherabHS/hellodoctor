# Remediation D — paused checkpoint

Status: **GREEN - Remediation D implementation and bounded regression closure verified.**

Resumed and closed on 2026-10-27 (local date). This is an application/remediation gate result, not a production-provider certification.

## Repository and safety

- Branch: `main`; HEAD: `47078ee0cafada469bdfab0b74baeca01abcfb82`.
- Remediations A/B/C and all D changes remain local and uncommitted. No staging, commits, pushes, resets, cleanups or deployments were performed.
- No Azure/Firebase credentials or production resources were configured. Docker containers and volumes were not removed.
- No test/build gate was still running when this checkpoint was recorded. The D gate and shared-fixture lock files were absent.
- Inspection snapshot: `scratch/remediation-d/git-before.txt`, `diff-before.bin`, `index-before.bin`, `protected-source-hashes.json`.
- Preservation inventory: `scratch/remediation-d/preservation.json`. Its index comparison passed; 373 of 380 inspection-protected files were unchanged. The seven changed entries are the intended backend API/repository/test files and Cargo.lock. Existing Admin source/tests and backend financial, erasure and notification-delivery service files were preserved.

## Initial findings and implementation

### Article discovery

The Patient Blogs page fetched the first 20 rows and searched them locally. Two widget reproductions failed before the correction: older articles could not be loaded, and searching did not query the archive.

Implemented bounded server search and cursor pagination in the existing unified published article feed. `GET /api/v1/articles` retains its `data` array and legacy limit/offset behavior, and adds optional `search`, `language`, `before_time`, `before_id`, and top-level `next_cursor`. Ordering remains publication timestamp descending, UUID ascending. Search is parameterized, capped at 160 characters, and uses complete Kiswahili content or English fallback. Page size is capped at 50, with one look-ahead row.

The query reuses the existing published Doctor/Admin projection and RLS. No draft/archived/unapproved projection was introduced. No migrations or policy changes were made. Existing publication indexes remain; large-archive substring-search performance has not been benchmarked.

The existing Patient Blogs tab now has localized Load More, retry, empty and end states. A screen-local controller handles debouncing, refresh, language changes, duplicate IDs, failed pages, obsolete responses and disposal. Existing article detail and approved optional quizzes remain in place.

### Notification destinations

Both mobile adapters previously refreshed the inbox on OS notification taps without routing using their payload. The original Doctor inbox code was reproduced in an isolated ignored test fixture: an appointment tap did not open an appointment destination in either language.

Implemented an owner-scoped `GET /api/v1/notifications/:id`, protected by normal authentication, ABAC and RLS, with an explicit user predicate and `private, no-store`. It does not mark a notification read or expose internal claim/lease fields.

Mobile routing trusts only a validated notification UUID from the OS, fetches the canonical owned notice, then independently authorizes the destination through existing backend APIs. Arbitrary URLs/routes are ignored.

| Supported canonical notice | Destination |
| --- | --- |
| Booking/consultation types related to an appointment | Focused current appointment-status page; fresh authenticated read, no automatic payment or video-room entry |
| `NEW_CHAT_MESSAGE` | Exact authorized conversation bound to the notice's appointment, then existing role-specific chat screen |
| Patient `NEW_ARTICLE_PUBLISHED` / `DOCTOR_ARTICLE` | Existing published article detail screen |
| Doctor `ARTICLE_STATUS_CHANGED` / `DOCTOR_ARTICLE` | Existing own-article editor after authorized lookup |
| General, unsupported or campaign notices | Inbox; campaigns currently have no approved article-link contract to reuse |

Signed-in navigation hosts handle in-memory cold-start handoff, duplicate callbacks, failures and disposal. Doctor navigation resets at the authentication boundary. Foreground receipt only refreshes; an authorized explicit open can mark a notice read using the existing endpoint. Denied destinations remain unread and show a localized message. No delivery worker, lease/fencing logic, FCM server transport, clinical trace acknowledgment or quiz-attempt history was changed.

Local notification launch/tap callbacks and iOS initialization were added. SDK tap listeners attach before optional token-registration work. Native execution remains unverified.

### Localization

Patient discovery, pagination, publisher identity, notification pagination, dates and destination errors use EN/SW localization. Existing localized quiz errors were retained.

Doctor gained ARB generation and a secure device locale preference for the affected inbox/settings/destination UI, with English fallback and protection against a stale preference read replacing a newer choice. Other Doctor screens were not comprehensively translated.

### CI and dependency security

Added `.github/workflows/application-checks.yml` for both Next.js apps and both Flutter apps: locked installs, types/lint/tests/build for web; generation/format/analyze/tests for Flutter; web dependency audits. Actions and tool versions are pinned. Extended backend CI with explicit compilation, rustfmt, runtime-generated test role credentials, owned Redis database 14 and a bounded serialized integration command. The existing strict Rust audit workflow and its sole existing RSA exception were retained.

Official actionlint 1.7.12 passed against the workflow definitions. These workflows have **not** run on GitHub; nothing was pushed.

Dependency changes:

- Existing fixes were already present: rustls-webpki 0.103.16, webpki-root-certs 1.0.10, webpki-roots 1.0.10 and Tokio 1.53.3. Tokio was not upgraded to 1.54.0.
- A fresh audit additionally found zstd vulnerabilities/unsoundness warnings. Targeted lock updates: zstd 0.14.0 → 0.14.3; zstd-safe 8.0.0 → 8.2.0; zstd-sys 2.1.0 → 2.1.2, retaining zstd 1.5.7.
- Doctor Next.js 15.5.27 → 15.5.28, addressing GHSA-2464-fhr4-w9xw and GHSA-p3m5-59mp-jv5r. Installed Sharp is 0.35.5. Added ESLint 10.12.0 / typescript-eslint 8.71.1 as development tooling; retained the existing source-safety checker.
- Admin postcss-selector-parser → 6.1.5, addressing GHSA-rj75-hqrm-r3gf. Admin Next.js remains 16.4.3.
- Doctor Flutter added SDK localization and its required direct intl 0.20.3 dependency.

Whole-app Dart format checks exposed 33 pre-existing formatting differences. Those were normalized only after verifying identical executable token hashes, normalizing optional formatter-owned trailing commas. Evidence and the exact patch are in `ci-formatting-token-equivalence.json` and `ci-formatting-only.patch`. These files include Agora/auth sources; their executable tokens were unchanged. No native Agora compatibility claim follows from that source check.

## Actual verification at checkpoint

Commands, native exit codes, durations and logs are retained in `scratch/remediation-d/`. Focused counts overlap full-suite counts and must not be added together.

| Check | Actual result |
| --- | --- |
| Rust formatting | PASS, exit 0; repeated after the queue-fixture correction |
| Rust Clippy all targets, warnings denied | PASS, exit 0; repeated after the queue-fixture correction |
| Rust check all targets | PASS, exit 0; repeated after the queue-fixture correction |
| Rust library | 91 passed, exit 0 |
| New D backend integration | 6 passed, exit 0 |
| CMS integration | 14 passed, exit 0 |
| Patient quiz backend integration | 10 passed, exit 0 |
| One serialized full backend run (initial checkpoint) | 443 passed, 2 failed, 4 ignored; exit 101; 1487.22 seconds test execution |
| Resumed bounded serialized full backend run | **445 passed, 0 failed, 4 ignored; exit 0; 1421.73 seconds test execution** |
| Patient Flutter full suite | **359 passed**, exit 0 |
| Doctor Flutter full suite | **290 passed**, exit 0 |
| Both Flutter full format/analyze gates | PASS; analysis reported no issues |
| Admin Web | **126 passed** (98 + 28), types/lint/build PASS |
| Doctor Web | **85 passed**, types/lint/build PASS |
| Cargo audit | PASS, exit 0: `--deny warnings --ignore RUSTSEC-2023-0071`; no new exceptions |
| npm audits | Both report zero vulnerabilities, exit 0 |
| Workflow validation | actionlint PASS, exit 0 |

The first Patient full run had 356 passes / 1 failure: the existing quiz test adapter overrode only the legacy `feed` method. The fixture was updated for `discover`, its acceptance assertions retained, and the full suite rerun successfully. Initial analyzer/format failures and fixture corrections are preserved in the evidence directory rather than deleted.

Admin lint retains one existing Campaign Manager hook warning (zero errors). Doctor Flutter retains an existing offscreen `Join` tap warning in its passing suite; it was not silenced.

## Resumption closure

The initial checkpoint run above is superseded for the final gate by the resumed bounded run recorded in `scratch/remediation-d/full-resume-20261027.json`. That run used the corrected calendar-boundary queue assertion and the same controlled PostgreSQL/Redis fixtures. It completed with **445 passed, 0 failed, 4 ignored**, exit 0, and an unchanged backend source digest. The campaign idempotency case passed in the full rerun, so the earlier isolated 503 did not recur. The corrected queue case also passed in the full rerun.

## Historical full backend failures at the paused checkpoint

1. `admin_campaigns::test_campaign_creation_is_idempotent_and_detects_content_mismatch`, `tests/cases/admin_campaigns.rs:304`: HTTP 503 instead of 200 during concurrent creation. The full log records approximately 361/372 ms response latency. The same unchanged case passed alone afterward: 1 passed, exit 0. The original response body was not captured, so the precise dependency/admission cause is **unconfirmed**; do not call it a proven database-acquisition timeout. Campaign code, production timeouts, pools and assertions were not changed.
2. `test_pre_e2e_queue_excludes_stale_and_terminal_but_preserves_history`, original `tests/api_tests.rs:17940`: expected two today's-queue IDs, received one. It reproduced alone. A read-only fixture clock check showed Nairobi 22:15, with the `now + 150 minutes` booking at 00:45 tomorrow. The API's today/schedule split was correct. Corrected the test to use PostgreSQL time and exact Nairobi calendar-based expected sets, retaining ordering, history, active-work and Doctor-isolation assertions. The corrected case passed alone: 1 passed, exit 0.

The initial run preceded the **test-only** calendar correction and also contained one transient campaign HTTP 503. Those historical failures are retained for auditability; they are superseded by the resumed bounded run below, which completed with 445 passed and zero failures.

## Cross-module acceptance evidence

| Gate | Evidence / limit |
| --- | --- |
| Local correctness | Archive with more than 20 mixed articles, server search, locale/fallback, cursor validation, widget pagination, routing allowlists, EN/SW and denied-destination feedback |
| System consistency | Published projection and version-bound quiz metadata reused; draft isolation and answer-key protection tested; owned notification lookup does not change delivery/read state; destination endpoints retain clinical ownership checks; Admin session/editor tests pass |
| Failure recovery | Stale search/page responses, retryable page failures, non-advancing cursors, refresh/disposal, delayed preferences, duplicate taps, account teardown and unauthorized destination fallback tested |

Existing quiz acceptance through the Blogs screen passed after fixing its test adapter. Notification receipt does not imply read/opened or delivered state. No patient quiz results were persisted. No historical financial or identity data reconciliation was performed.

Native Android/iOS foreground/background/terminated acceptance was not executed. `flutter devices` found only Windows and Edge. Real FCM/APNs setup, actual cloud covers/Azure connectivity, and GitHub workflow execution remain external/unverified. Backend/Flutter fixtures are not proof of those provider/device operations.

## Exact files and evidence

The D change inventory is retained in `scratch/remediation-d/d-files.json`; formatting-only paths are separately enumerated in `ci-formatting-token-equivalence.json`. The inventory includes generated Doctor localization files currently untracked; resolve their ignore/check-in convention consistently with Patient before eventual staging. All files remain on disk.

Important evidence:

- `full.json`, `full.log`: the sole full backend result and unchanged source digest during that run.
- `campaign-failure-diagnostic.json`: unchanged campaign case passes alone.
- `queue-calendar-reproduction.json`, `queue-calendar-clock.json`, `queue-calendar-corrected.json`: reproduction, server calendar evidence and corrected focused pass.
- `clippy-closure.json`, `check-closure.json`, `fmt-closure.json`: final Rust static gates.
- `patient-tests-closure.json`, `doctor-tests-complete.json`: final Flutter counts.
- `admin-test.json`, `doctor-web-tests.json` and corresponding type/lint/build metadata.
- `cargo-audit-final.json`, web audit metadata, `patched-dependencies.json`, `actionlint-final.log`.

## Follow-up limits

1. Preserve all A/B/C/D source changes and evidence; no staging, commit, push, reset or clean was performed.
2. Native Android/iOS notification delivery, Azure/FCM provider operations and GitHub workflow execution remain unverified external checks.
3. Do not start Azure, unified environment configuration, Firebase setup, another remediation or deployment under this checkpoint.

No outstanding development/test process needs to be left running overnight. This checkpoint does not authorize staging, committing or pushing.
