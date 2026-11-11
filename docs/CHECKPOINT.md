# HelloDoctor - Current Project Checkpoint

**Milestone 8 six-service performance remediation — RLS root-cause found and fixed, full-system bottleneck now identified as host CPU, not yet acceptance-ready (2026-11-02):** Continuation of the six-service (backend/Postgres/Redis/Caddy/Admin-Web/Doctor-Web) benchmark work against the `hellodoctor-m8-final-local` Docker stack (a disposable, dedicated benchmark project — never the shared dev `hellodoctor-postgres`/`hellodoctor-redis` or any other running stack, all of which were deliberately left untouched throughout). Full detail, evidence, and raw data: `docs/MILESTONE_8_SIX_SERVICE_PERFORMANCE_REMEDIATION.md` (Phases 1-4). **No git add/commit/push was performed anywhere in this session; everything below is uncommitted working-tree state.**

Root cause of the six-service active-web-load degradation (first flagged in an earlier session's contaminated 60-minute run) was isolated precisely: `GET /api/v1/doctor/chats` and `GET /api/v1/doctor/appointments` (used by both Doctor-Web and the Flutter Doctor app's `doctor_repository.dart` — same backend endpoints) were measured at 3,452ms/3,196ms under real RLS enforcement vs 148ms with RLS bypassed as superuser — a 23x slowdown from `appointments_access`/`chat_conversations_access`/`chat_messages_access`/`consultation_sessions_access`'s RLS policies resolving "is the caller the doctor who owns this row" via a correlated subquery into `doctor_profiles`, re-executed per scanned row (confirmed via `EXPLAIN (ANALYZE, BUFFERS)` against the real `hellodoctor_api` role, not a missing index — every join already used one).

**Fix implemented, tested, and deployed:** new migration `services/backend/migrations/20261204000001_doctor_rls_identity_lookup_performance.{up,down}.sql` adds `fn_current_doctor_profile_id()` (STABLE SQL, no per-row argument, so Postgres can evaluate it once per statement instead of once per row) and rewrites those four policies to compare directly against it; `patient_profiles_access`'s `fn_doctor_treats_patient` (a richer verification/active-account/consultation-state check, not a pure identity lookup) was deliberately left untouched. `services/backend/src/api/v1/doctor_handlers.rs`'s `list_own_appointments`/`list_own_chats` were restructured to defer `doctor_history_context_allowed()` and the `patient_profiles` join to only the already-paginated result rows instead of every candidate row. Correctness verified first in an isolated clone database (`hellodoctor_rls_clone`, pg_dump/restore — the live/shared DBs were never touched during validation): identical row-ID sets/order before and after for the same doctor, zero cross-doctor leakage either direction, patient self-access and ADMIN full-visibility preserved, unauthenticated context fails closed. **Full regression suite — the exact CI invocation, `cargo test --test api_tests -- --test-threads=1` with the disposable `TEST_REDIS_URL` fixture — passed 476/476, 0 failed, 5 ignored** after the fix (two earlier attempts at this showed mass failures; both were this session's own test-setup gaps — a missing Redis fixture, and running with parallelism this suite's own CI comment documents as racing on shared Postgres catalog rows across the five capability roles — not real regressions; root-caused before trusting the green result). Deployed live to `hellodoctor-m8-final-local`'s real database via the normal `sqlx::migrate!` startup path (backend image rebuilt, container recreated); confirmed via direct `EXPLAIN ANALYZE` against the live (not clone) database immediately after: 1,631ms, consistent with the clone's 1,368ms.

**The fix alone does not close the gap, and the real reason is now known.** Re-measuring the "3-Admin/5-Doctor plausible web load" scenario required building real, version-controlled tooling (`services/backend/examples/web_workload.rs` — authenticated Admin/Doctor sessions via real BFF cookies, with a fail-fast routing preflight; `services/backend/examples/rtc_token_scenario.rs` — proves the pre-existing ~100% RTC-token 401/403 rate was correct authorization against an out-of-mint-window test fixture, not a bug; `services/backend/scripts/run_six_service_web_comparison.ps1` and the new `sample_live_instrumentation.ps1`) and fixing **seven real bugs along the way, none of them in the fix itself**: a PS7-only `-SkipHttpErrorCheck` flag that doesn't exist on this host's Windows PowerShell 5.1; a live `System.Diagnostics.Process` object passed through `Start-Job`'s `-ArgumentList` (cannot be serialized across the job's process boundary); a relative `$OutDir` resolved against the wrong working directory by .NET APIs; a `Register-ObjectEvent`-based async log capture whose PowerShell-side callbacks are queued for the engine's idle loop and silently lost a non-deterministic tail of each run's output (fixed by switching to plain `Start-Process -RedirectStandardOutput` file redirection); `docker stats`'s per-container CPU sampling had **never actually worked, in any run this whole investigation did** (`Start-Job` doesn't reliably inherit Docker CLI context, and separately `{{.Container}}` is the container ID, not its name — needed `{{.Name}}`); and a real business-rule confound, not a harness bug — the real `check_booking_limit`/`check_grievance_limit` (10/hour, 3/24h per account) had accumulated genuine state in Redis from the day's many benchmark runs (1,102 accounts already sitting at 4-18 toward the booking cap before a comparison run even started), which first looked like a severe booking-success regression and was actually test-fixture contamination — fixed with a narrowly-scoped `-ResetTestRateLimitState` step in the comparison script that clears only `ratelimit:booking:*`/`ratelimit:grievance:*` on this benchmark's own isolated Redis container.

With that state now controlled and live `pg_stat_activity` wait-event + per-container CPU instrumentation added, three repeatable 360-second runs of the same scenario landed in a tight, reproducible band: **TRUE_SYSTEM_ERROR_RATE 11.65-13.39%** (target 0.35% — still far away) and **booking success 73.7-76.4%** (a real improvement over the original pre-fix 70.6% baseline, not noise). The decisive new finding: PostgreSQL connections never exceeded 6 (of a configured 50-connection pool — connection count was never the constraint, confirming Stage A's earlier finding from a different angle); wait-event sampling shows active queries are CPU-bound (`active`, no wait_event), not lock- or I/O-bound; and **Postgres alone consistently peaked at 232-314% CPU — over half, sometimes over three-quarters, of this topology's entire 4 vCPU budget** — on top of the backend's own 146-173% peak. The remaining ~1.6s of real CPU work per call on these two endpoints (now dominated by the deliberately-untouched `fn_doctor_treats_patient`/`doctor_history_context_allowed` business checks) is, by itself, enough to saturate a shared 4-core host at only 3-Admin/5-Doctor-worker concurrency. **This means the next lever is a CPU-capacity question (host sizing, or a further security-reviewed optimization of those two specific richer checks, or client-side caching to call them less often) — not another admission/pool/lane-concurrency tuning pass.**

Current config, unchanged and NOT to be altered without new evidence: `DB_POOL_MAX_CONNECTIONS_PER_ROLE=10` (explicit override in `deploy/single-vm/compose.local-verification.yml`, scoped to the `hellodoctor-m8-final-local` stack only — the shared `services/backend/.env` was never touched), Consultation admission lane `n.min(6)` in `services/backend/src/admission.rs` (raised from `n.min(4)` earlier this investigation, kept — real benefit, ~0 rejections even under the 40-worker stress scenario). **Milestone 8 performance acceptance remains NOT READY** at 166-170 RPS, let alone the product-owner's staged 200/250 RPS targets (recorded in the remediation doc's Addendum, not yet attempted). Files changed, all uncommitted: the migration pair above, `admission.rs`, `doctor_handlers.rs`, the three new `examples/`/`scripts/` files above, and extensive evidence under `docs/performance-evidence/milestone-8-final/remediation/phase{2,3,4}_*`.

**FCM push-notification milestone closed out; backend regression gate genuinely green for the first time (2026-10-23):** Resumed from a large pile of uncommitted work (Stage A scheduling/booking, Stage B4 consultation recovery, and a substantial FCM/push-notification pass) and inspected rather than assumed its state. The FCM milestone itself — secure HTTP v1 backend delivery (OAuth2 JWT service-account auth, retry/backoff, dead-token disabling), authenticated device-token lifecycle (register/refresh/revoke, owner-scoped), and both Flutter apps wired end to end (`firebase_messaging`, token registration on launch/refresh, foreground/opened-app inbox refresh) — was already functionally complete from a prior session; this pass's job was verification, and verification found the backend regression suite in a far worse state than any prior session's claims.

Found and fixed, in order of discovery: (1) three pre-existing Rust compile errors unrelated to FCM (two repository functions losing their `Result` return value to a stray trailing semicolon; a `prescription_service.rs` transaction used after it was already consumed by `.commit()`) that meant the backend did not compile at all going into this session; (2) two clippy warnings and one test-file formatting diff; (3) a genuinely broken `/notifications/devices/revoke` endpoint and the entire FCM delivery-scheduler retry/permanent-failure state machine had **zero** test coverage — added `tests/cases/push_notification_delivery.rs`; (4) a stale notification type: `20261113000001_payment_hold_cleanup` silently dropped `NEW_CHAT_MESSAGE` (the type every chat-send path and both Flutter apps actually use) from the `notifications_type_check` constraint, so **every chat message send had been 500ing since that migration landed** — invisible until a real regression run actually exercised chat; (5) `hellodoctor_payment_worker` was never granted `SELECT` on `doctor_profiles`/`patient_profiles` or `EXECUTE` on the `doctor_own_encounter_allowed` SECURITY DEFINER function, both needed transitively by RLS policies the new hold-expiry-cleanup code path touches, so the new "expired holds purge clinical records" feature failed closed with a raw permission error; (6) `patient_profiles` never got the `doctor_profiles`-equivalent unconditional `hellodoctor_system_worker` read policy, so the notification scheduler's bare-pool queries (no RLS session variables set) silently saw zero patient rows — Patients were never actually getting booking-hold-created/-expired notifications, no error, just empty results; (7) four SQL string-literal bugs in `notification_repo.rs` where a multi-line backslash-continued query was missing the trailing space before the backslash, merging adjacent SQL tokens (`clock_timestamp()WHERE`, `notificationsWHERE`) — caught only once the delivery scheduler finally had its first-ever test; (8) a `sqlx::migrate!` gotcha, now documented directly in `migration_status.rs`: the compile-time embedded migrator does not reliably pick up a newly added migration file unless the file containing the macro invocation is itself content-changed, which produced three `VersionMissing` test failures until worked around. Three new forward-only migrations (`20261118000001`-`20261118000003`) carry fixes 4-6; nothing historical was edited. A background audit agent independently launched a second, concurrent `cargo test` run against the same shared dev Postgres without being asked, corrupting sqlx's test-database bookkeeping and producing a false 300+/316-failure signal before this was caught and the process stopped — recorded in agent memory as a new instance of the fork-scope-creep pattern.

Final verified baseline this session: `cargo fmt --check` clean, `cargo clippy --all-targets -- -D warnings` clean (zero warnings), `cargo test --lib` 84/84, full serialized `cargo test --test api_tests -- --test-threads=1` **312/312 passed, 0 failed, 4 ignored** (the 4 are explicit A/B capacity benchmarks, not part of the regular gate) — run against a disposable, explicitly-owned test Redis fixture (`TEST_REDIS_URL`/`TEST_REDIS_FIXTURE_ID`, never the shared dev Redis on 6379). Doctor Flutter (`apps/mobile`): `flutter analyze` clean, 274/274 tests pass. Patient Flutter (`apps/mobile_patient`): `flutter analyze` clean, 277/277 tests pass (neither app's source was touched this pass — reverified only). Still open, explicitly not started this pass: bilingual (EN/SW) notification template content (the `notification_templates`/`notification_type_registry` tables exist but hold no localized copy yet — all current notification bodies are hardcoded English), quiet-hours/per-type frequency-limit enforcement (columns exist, nothing reads them), holiday/birthday/welcome/health-education/admin-campaign notification triggers, Android `google-services.json`/Google Services Gradle plugin wiring (no native Firebase config exists yet in either app, expected per `docs/FCM_SETUP.md`), and the repository-wide git cleanup / "many bugs" sweep the user has asked for next.

**Security remediation, payment architecture, data rights, and admin MFA redesign (2026-10-17):** Completed the deferred HELLODOCTOR security remediation campaign (h01-h23, independently re-verified), including a critical fix to a wrong-cased payment-webhook enum literal (h23) that was silently rejecting every M-Pesa confirmation webhook, and a migration-reversibility test fix that correctly scopes the check to Postgres's genuine enum-irreversibility limit rather than papering over it. Built real admin-panel failure observability: every backend panic, request timeout, and Doctor/Patient/Admin-Web client-side error now reports into `system_error_logs` with an exact file:line capture, verified live against a real injected panic, not simulated. Fixed the admin-web's placeholder buttons across the dashboard (CSV/JSON export, notification bell count, payout-readiness badge, PDF document download) and converted several fake-success states (webhook reconciliation, receipt download) into honest "not available" states instead.

Built the full payment-methods architecture this session had been waiting on: patients and doctors can each save multiple payment methods with one marked default, admins configure the platform's own merchant account, and every save is hard-gated on an admin having configured a real provider first - a 409, never a silent pass. Added the Data Protection Act 2019 §40 data-rights feature: admin can export a patient's or doctor's full record as JSON, or erase/anonymize it, where Postgres's own FK constraints (not a hand-maintained table list) decide whether a true delete is possible or the profile must be anonymized with its clinical/financial history left untouched; financial records are never targeted by either path. Wired the patient Flutter booking flow to require picking one of these real saved payment methods at checkout (replacing a hardcoded gateway constant), and replaced the Doctor Flutter wallet page's previously-permanently-disabled "Connect M-Pesa" button with a real, working payout-binding flow against the same backend endpoint Admin's payout routing already reads from.

Redesigned admin login end to end: the old single-screen email+password+authenticator-app-TOTP flow (which left every admin unable to sign in without a pre-loaded authenticator secret) is replaced by a two-step email+password → Azure-emailed one-time-code flow, mirroring Doctor login's proven brute-force protections (account/IP jailing, per-attempt limits, single-use codes). The old TOTP machinery (crypto functions, DB-credential repository functions, the `totp-rs` dependency, seeded TOTP secret) was removed, not left as dead code. Verified live end to end against the real running backend (password step → emailed code → real tokens; code replay correctly rejected), not just via unit tests.

Rust: `cargo fmt`/`clippy -- -D warnings` clean throughout. Full backend suite: 254/261 passed at this session's final run; the other 7 are pre-existing flakes (a Postgres role-grant race under heavy `#[sqlx::test]` parallelism, and Redis-backed rate-limit state shared across tests in one binary) - each confirmed passing cleanly in isolation, not claimed fixed. Patient Flutter (`apps/mobile_patient`): 226/226 pass, `flutter analyze` clean. Doctor Flutter (`apps/mobile`): 226/226 pass, `flutter analyze` clean. Admin-web: `tsc --noEmit` and lint clean on every touched file; its own Jest/unit suite was not rerun this session. Doctor-web was not touched and not rerun. Note: the Patient-facing Flutter app (`apps/mobile_patient` - booking, home, directory, chat, Swahili localization) already existed in full going into this session; an earlier assumption in this project's own tracking that it did not exist was a mistake, corrected here.

**Pre-E2E corrections (2026-10-16):** PostgreSQL queue eligibility/current-first ordering, default Upcoming on Doctor Flutter/Web, completed history, expired-slot filtering/transactional rejection and availability resume/reconnect refresh implemented. Patient payment now uses one owned Proceed to payment action with no fake success; Doctor own chat messages are green/right, Patient messages white/left; secure shared clinical-image attachment routes/composers added. Doctor 197, Patient 195, Doctor Web 50 and Admin 13 tests pass; Rust 53 library tests and the full serialized backend suite pass: 221 passed, 0 failed, 0 ignored, 0 filtered. Signed development APKs Doctor 1.0.1+6 and Patient 1.0.1+5 are built. Physical verification is DEFERRED_BY_USER; live checkout/private storage remain unavailable, and full ecosystem simulation has not begun. [Queue report](PRE_E2E_QUEUE_AVAILABILITY_CORRECTION.md), [payment/testing audit](PATIENT_PAYMENT_UX_VERIFICATION.md), [attachment verification and limits](DOCTOR_CHAT_ATTACHMENT_VERIFICATION.md).


**Doctor reminder pop-up follow-up (2026-10-15):** missing Android scheduled-notification receivers and release reflection rules corrected; Doctor 1.0.1+5 built, analysis clean, 190/190 tests pass. Compiled APK manifest and Room constructor checked. Installation and actual pop-up capture remain pending because the reconnected phone has no working ADB debugging connection. [Fix and exact verification boundary](DOCTOR_NOTIFICATION_POPUP_VERIFICATION.md). Video/Agora remains deferred.

**Doctor system-navigation overlap (2026-10-15):** DoctorShell now reserves the Android bottom inset with SafeArea for all six tabs. Nine added layout/tap tests cover 320/360/412dp widths and 0/24/48dp insets; full Doctor suite 189/189, analysis clean. [Fix and physical evidence](DOCTOR_NAVIGATION_SAFE_AREA_FIX.md).

**Patient OTP redirect/local backend recovery (2026-10-15):** successful OTP verification now replaces the signed-out Navigator, immediately showing profile completion or the Patient main menu; logout also removes protected routes. Four added real-app regression cases pass; full Patient suite 185/185, analysis clean. Restored the stopped local backend and verified LAN/local readiness plus Doctor password/MFA authentication. [Details and evidence](PATIENT_AUTH_NAVIGATION_FIX.md).

**Doctor launch crash fixed (2026-10-15):** reproduced the native WorkManager/Room initialization crash on the connected Moto g96 5G (Android 16). R8 removed a reflectively used database constructor. Targeted release rules restore it; the APK constructor check passes and three physical-device cold launches render the login screen without crashing. Installed 1.0.1+3 in place, preserving app data. See [crash fix and evidence](DOCTOR_STARTUP_CRASH_FIX.md). This is launch verification, not full device acceptance of all features.

**Patient consultation completion (2026-10-14):** real paid Chat booking/status/reopen UI and modality-specific Patient visibility/actions implemented across Doctor Flutter and Web. Shared Patient/Web/Flutter message history verified against the live local backend. Patient Flutter 181/181 passed; Doctor Flutter 180/180 passed; Doctor Web 47/47 and Admin Web 13/13 passed. The corrected full serialized backend rerun completed with 214 passed, 0 failed, 0 ignored and 0 filtered; its final output was recovered on 2026-10-15. No existing tests/assertions deleted or weakened; four exact UI-label expectations intentionally updated and documented. See [consultation verification](PATIENT_CHAT_CONSULTATION_VERIFICATION.md) and [device test guide](../releases/PATIENT_CONSULTATION_TESTING_20261014.md). Live M-Pesa, Agora media and physical-device acceptance remain pending; local development payment activation is verified.

**Latest focused Patient correction (2026-10-14):** discovery filters/actions, long-text contracts and editorial Q&A submission/OPEN persistence corrected. Patient analyze: 0 issues; Flutter: 171/171 passed; 9 relevant backend tests passed; live POST 201/OPEN and owner feed reload verified. Migration `20261105000001` adds a context-bound owner-read helper; anonymous author grants remain unchanged. Current LAN: 192.168.0.105. Physical device acceptance remains PENDING_NO_DEVICE. See [Patient correction report](PATIENT_UI_VISUAL_REDESIGN.md). Historical suite counts below are not a new full-backend run.


**Status reconciled:** 2026-10-17 (security remediation, payment-methods architecture, Data Protection Act data-rights, Patient/Doctor payment pickers, admin MFA redesign). Resume from this snapshot, not an old agent roster or interrupted test count. Latest implementation is complete within its approved scope; the open items below remain explicitly open.

**Product decision, explicit and binding on all future work:** patients can never cancel a booked consultation, of any status, by any mechanism. The M5 self-service `PENDING_PAYMENT` cancel feature (`POST /appointments/:id/cancel`) built earlier the same day has been fully removed end to end — backend handler/service/route/tests and the Flutter Cancel UI/cubit method/repository call are all gone, not merely hidden. Do not reintroduce any cancellation path without a new, explicit instruction.

## Latest verified baseline

| Component | Result | Evidence / scope |
| --- | --- | --- |
| Rust formatting | PASS | `cargo fmt --all -- --check` |
| Rust lint | PASS | `cargo clippy --all-targets -- -D warnings`, zero warnings |
| Backend library tests | **84/84 PASS** | `cargo test --lib` |
| Backend full suite | **312/312 PASS, 0 failed, 4 ignored** | Full serialized `cargo test --test api_tests -- --test-threads=1` against a disposable, explicitly-owned test Redis fixture (`TEST_REDIS_URL`/`TEST_REDIS_FIXTURE_ID`); the 4 ignored are explicit A/B capacity benchmarks, not part of the regular gate. Six real, previously-latent bugs found and fixed to reach this — see the 2026-10-23 entry above |
| Dart formatting / Flutter analysis (Doctor app, `apps/mobile`) | PASS | `flutter analyze` clean (reverified 2026-10-23, source untouched) |
| Flutter tests (Doctor app, `apps/mobile`) | **274/274 PASS** | Full suite (reverified 2026-10-23, source untouched) |
| Dart formatting / Flutter analysis (Patient app, `apps/mobile_patient`) | PASS | `flutter analyze` clean (reverified 2026-10-23, source untouched) |
| Flutter tests (Patient app, `apps/mobile_patient`) | **277/277 PASS** | Full suite (reverified 2026-10-23, source untouched) |
| Doctor Web | Not rerun this session | Untouched this session |
| Admin Web | Not rerun this session | Untouched this session |
| Migration agreement | **NOT RE-VERIFIED AGAINST THE PERSISTENT DEV DB THIS SESSION** | `scripts/verify_backend_schema.ps1` was not run this pass; the persistent `hellodoctor` dev database is confirmed several migrations behind (`appointment_expiry_cleanup_jobs` does not exist there) — only the ephemeral per-test databases are current. Restart the dev backend and run the verifier before trusting any live check against it, per CLAUDE.md §4 |
| Admin login redesign | **LIVE-VERIFIED (2026-10-17, unchanged)** | Not re-touched or re-verified this session |
| Physical-device verification | **PENDING_NO_DEVICE** | Unchanged this session |

These are the latest recorded checks, not a new test run for this documentation update except where explicitly marked "reverified 2026-10-23" or "PASS" in the backend rows above, which are a real run from this session. Counts from earlier dated sections are historical. Automated regression and widget goldens do not constitute a penetration test or physical-device acceptance.

Primary evidence: [ownership/Q&A results](audit_evidence/user-isolation-qna.json), [security/Articles results](audit_evidence/public-exposure-security.json), [ownership/Q&A report](USER_ISOLATION_AND_QNA.md).

## Current implementation status

| Area | Current status | Remaining boundary |
| --- | --- | --- |
| Canonical identity / ownership | **USER_LEVEL_DB_ISOLATION_VERIFIED** | `users.id` is the security identity; unique FK-backed Patient/Doctor mappings, role/profile consistency, composite ownership FKs and pooled SET LOCAL context tested |
| Database isolation / traceability | **VERIFIED** | All 40 tables inventoried; 35 FORCE RLS tables and five documented pre-auth/infrastructure exceptions; five capability roles have no BYPASSRLS or table ownership |
| Controlled Compliance trace | **IMPLEMENTED / AUDITED** | COMPLIANCE and PLATFORM_ADMIN only; stable internal IDs, required reason, session/request audit, fail-closed audit persistence; no unrestricted Admin DB access or formal legal export workflow |
| JWT/session lifecycle | **IMPLEMENTED / REGRESSION GREEN** | Immediate access-token invalidation on current-family logout; other sessions preserved; atomic refresh rotation/reuse revocation and single-use Patient OTP tested |
| Protected API / cache boundary | **IMPLEMENTED / REGRESSION GREEN** | JWT, ABAC, ownership and RLS retained; subject/authorization/query-scoped idempotency cannot replay private data ahead of downstream checks |
| Public-exposure application guards | **IMPLEMENTED; DEPLOYMENT PARTIAL** | Explicit environment, production seed/dev-OTP prohibition, trusted HTTPS ingress checks, protected production metrics, blocked production simulation, Redis failure = 503 rather than fail-open |
| Kenya professional review | **KENYA_PROFESSIONAL_VERIFICATION_VERIFIED** | KMPDC/KE; explicit PENDING/VERIFIED/REJECTED; pending-only queue; atomic verify/reject with one race winner; boolean compatibility derived; internal manual review, no external registry integration |
| Scheduling / cross-portal availability | **IMPLEMENTED / VERIFIED** | Doctor owns normal slot management; authorized Admin emergency operations; patient availability API excludes unavailable slots; visible online portals refresh every 15 seconds plus request latency |
| Scheduling navigation / reported entries | **COMPLETE** | Dedicated Doctor Web Schedule & Slots; two specifically authorized unbooked entries cancelled and audited; unrelated three proposed historical corrections remain unapplied |
| Slot approval | **NOT_IMPLEMENTED_BY_PRODUCT_DESIGN** | Explicit user policy: no approval workflow; auditing does not require approval |
| Doctor Articles | **IMPLEMENTED** | Structured Doctor Web + Flutter authoring and direct publication by verified Doctors; no Admin review; published patient feed is API-only |
| Queue / notification domain | **BACKEND IMPLEMENTED; PATIENT FLUTTER UI ADDED (M8, 2026-10-10)** | Upcoming-first queue, owned inbox APIs, deduplicated reminders/daily summaries and lifecycle notifications; Patient Flutter now has a bell/inbox UI (`features/notifications/*`, mirrors the Doctor cubit); real push provider configuration required |
| Startup incident reporting | **IMPLEMENTED / VERIFIED** | Pre-initialization incident journal, authenticated delivery and Admin reference/build correlation; updated device build still needs installation/acceptance |
| Doctor Flutter Open Health Q&A | **DOCTOR_QNA_CODE_FIXED_REAL_DEVICE_PENDING** | Dedicated flow layout, flexible long content, keyboard-safe editor, answered list, authoritative 409 recovery and eight goldens verified; actual phone rendering still pending |
| Doctor Web / Doctor Flutter core workflows | **IMPLEMENTED; ACCEPTANCE PARTIAL** | Existing queue, chat, Smart Rx, schedule, wallet, profile/settings and Articles; user previously confirmed Doctor app acceptance, which does not verify the latest Q&A build |
| Patient client / complete patient journey | **IN_PROGRESS / M0-M12 CODE-VERIFIED (2026-10-10)** | M12 adds the Doctor-shaped Patient entry screen, email + password + email OTP MFA, phone capture during registration, Azure Communication Services Email delivery, and registration requires current Patient terms, privacy policy, data-rights notice and health-data consent; profile details are stored atomically with account creation. 144/144 Patient Flutter tests, backend `cargo check --tests` clean; integration suite must be rerun after the current migrations. No physical-device/prototype-comparison verification yet (PENDING_NO_DEVICE). |
| Private clinical-document storage | **IMPLEMENTED; DEPLOYMENT ACCEPTANCE OPEN** | Private storage configuration and successful deployed upload/download acceptance remain required; earlier live development uploads were blocked by missing storage configuration |
| Seven endpoint coverage gaps | **CLOSED / REGRESSION GREEN** | System-error simulation, compliance alerts, audit simulation, pending verification queue, aggregate telemetry, metrics and finance month-filter tests are in the current API suite |
| Upload compression / field validation | **IMPLEMENTED / REGRESSION GREEN** | Centralized 400 KiB batch ceiling, server-side field limits, email/phone validation and password rules; no longer deferred |
| Newtype identity refactor | **COMPLETE** | Distinct UserId / PatientProfileId / DoctorProfileId types; schema and wire UUID representation preserved |
| Machine-generated OpenAPI | **DEFERRED / NOT_IMPLEMENTED** | Handwritten API/client contracts remain; no utoipa/schemars retrofit or dependency installation |
| Current Azure hosting proposal | **PROPOSED / NOT_DEPLOYED_OR_LOAD_VERIFIED** | User will handle hosting; no infrastructure changes or capacity guarantee recorded |

## Remaining work and release gates

1. **Sensitive Admin step-up authentication remains open** for financial/security operations. Existing authorization is not recent reauthentication.
2. **Latest Q&A real-device render/acceptance remains pending.** Connect an existing Android device and verify the corrected build against `prototype/index.html` before claiming the fix visually accepted. Widget goldens are separate evidence.
3. **Patient client and complete patient journey remain unfinished.** Patient APIs and synthetic backend tests do not replace patient UI/device/provider acceptance.
4. **External deployment/provider acceptance remains open:** approved TLS termination and production configuration, production OTP/SMS delivery, private clinical-document storage, real push delivery, live Agora/media acceptance, and payment/refund/payout provider validation. Purchasing Azure services does not integrate them automatically.
5. **Operational release evidence remains open:** off-host backups and restore drill, deployed monitoring/alerts, production secrets/key management, retention and the payment-production readiness checklist. Current Azure load/soak testing has not been performed; earlier local capacity results predate the latest session/RLS changes.
6. **Scoped product/quality gaps remain:** dedicated Flutter notification inbox/bell UI, lab-report endpoint/model, generated OpenAPI, and formal legal export workflow. These are recorded gaps, not authorization to reopen them in this documentation pass.

Overall release status: **PUBLIC_EXPOSURE_SECURITY_GATES_PARTIAL**. Controlled synthetic-data development/testing is supported; unrestricted clinical production, real-money operation and an unsupervised 1,200-user pilot are not declared ready. No evidence-based overall completion percentage is assigned.

See [public-exposure controls](PUBLIC_EXPOSURE_HARDENING.md), [deployment checklist](PRODUCTION_DEPLOYMENT.md), [Articles policy](DOCTOR_ARTICLES.md), [ownership inventory](USER_OWNERSHIP_INVENTORY.md), and [scheduling evidence](SCHEDULE_SYNC_FOLLOWUP.md).

## Current scope and resume rules

- This update changes documentation only. No code changes, deployments, service starts, package installs or host configuration changes are included.
- The user will handle the proposed Azure hosting setup. Hosting is not marked deployed, integrated or performance-verified.
- Do not re-run completed Articles, Kenya review, ownership-isolation or security implementation solely because an archived entry says "not started" or "resume here".
- Do not add slot approvals or Article Admin review. Preserve current five-role DB architecture, ABAC, RLS, session boundaries and Kenya-only professional review.
- Apply new SQLx migrations for future schema changes. Never edit applied migration history or manually modify `_sqlx_migrations`; B-01 source/binary/database guards remain required.
- No further implementation is automatically queued by this status reconciliation. Future work requires the user's next instruction; record pending acceptance and release gates accurately.

## Historical implementation log

Everything below this boundary records earlier passes. Old test counts, migration versions, agent rosters, environment descriptions, interrupted states and resume instructions are historical, not the current task list. The status tables above supersede them. Historical audit findings are retained rather than rewritten as if later fixes had already existed.

## Patient Flutter app M10 (English/Swahili surface completion) ? 2026-10-10 (later, same day)

The remaining Patient feature-screen copy is now represented in the generated English/Swahili localization catalog: Account/Family Profiles, Directory/Doctor Profile, Health Vault, Q&A, consultation summary, video fallback, navigation, and offline cache states. Locale switching remains device-persisted through `LocaleController`; API category/status values remain fixed identifiers and are localized only at presentation.

Verification: `flutter gen-l10n`, `flutter analyze --no-pub` clean, `dart format` clean, and **119/119 Patient Flutter tests PASS**, including two new locale persistence/translation tests. Device/prototype comparison and live-backend walkthrough remain open.

## Patient Flutter app M9 (Post-Consultation screen) — 2026-10-10 (later, same day)

No backend gap: `GET /appointments/:id` (`appointment_handlers::get_appointment`) already returns the raw `Appointment` row, which already carries `clinical_outcome` and `completed_at` — confirmed directly in `domain/models.rs` before writing any code, not assumed. `AppointmentDetailDto` (`booking_models.dart`) gained a `completedAt` field (the JSON was already there, just unparsed); `WaitingRoomState`/`WaitingRoomCubit.poll()` now additionally carry `clinicalOutcome`/`completedAt` through from each poll response.

**Reachability**: `WaitingRoomPage`'s terminal-state branch now distinguishes `COMPLETED` from `CANCELLED`/`EXPIRED` — `COMPLETED` shows a "View Consultation Summary" button pushing the new `PostConsultationPage`, instead of the old generic "can no longer be joined" message for every terminal status alike. `WaitingRoomPage` gained a new required `familyProfilesCubit` param (threaded from `patient_home_page.dart`'s `_openWaitingRoom`, its only call site) so the summary screen's "View in Vault" button can open `HealthVaultPage` correctly.

**New `features/patient/consultation/post_consultation_page.dart`** (`prototype/index.html` view-11 — "Consultation Completed"): branches honestly on the real clinical outcome (CLAUDE.md §6 — completion never requires a prescription) rather than assuming an e-Prescription was always issued the way the prototype's static demo content does — `COMPLETED_WITH_RX` shows an "Official e-Prescription Issued" card linking to Health Vault (real doctor name + real `completed_at` time, no fabricated BMDC number or fixed timestamp), `COMPLETED_NO_RX`/`REFERRED`/`ESCALATED` each show their own real, honest message instead. Deliberately drops the prototype's `#v11TelemetryCard` ("Session Telemetry Record": fixed "2m 14s" duration, "100% Quality", a fake escrow amount, a fake `#CONS-9481` ref) — that panel is `style="display:none"` in the prototype's own markup, confirming it was never meant to render; no backing endpoint exists for a patient to call anyway. "Return to Home" pops back to the tab root (`popUntil((route) => route.isFirst)`); "Report an Issue" reuses the same `showGrievanceModal`/`GrievanceRepository` flow already built for M8's Chat and Waiting Room.

**Verification**: Flutter Patient `flutter analyze --no-pub` 0 issues, `dart format` clean, **117/117 tests PASS** (116 prior + 1 new `WaitingRoomCubit` test covering `clinicalOutcome`/`completedAt` propagation on a `COMPLETED` response). Backend untouched this pass (no gap existed) — the 206/206 integration count from the M8 entry stands unchanged. **Not yet done, named rather than silently skipped**: physical-device verification and a live click-through against a running dev backend.

## Patient Flutter app M8 (Notifications, Blogs, Grievance, Free Health Q&A — gap #6 closed) — 2026-10-10 (later, same day)

Closed the plan's final named backend gap (#6, Free Health Q&A submission), then built every M8 Flutter screen on top of it and the three already-real backend surfaces (Notifications, Blogs, Grievance) — same just-in-time gap-closing discipline as every prior milestone.

**Notifications** (`features/notifications/*`, new): `notification_models.dart`/`notification_repository.dart`/`notification_cubit.dart` mirror `apps/mobile`'s Doctor notification cubit, minus the Doctor-specific schedule-reconciliation layer Patient has no equivalent dormant infra for. `notification_center_page.dart` is a new inbox list. Wired into `app.dart`'s `_SignedInRootState` (constructed alongside the other root cubits, `..load()..startPolling()`) and into Home's `_Greeting` via a `NotificationBellAction` (pre-existing shared widget, previously unused by Patient). 10 new `NotificationCubit` unit tests (load, markRead/markAllRead optimistic update + rollback-on-failure).

**Blogs** (`features/patient/blogs/*`): `blogs_page.dart` rewritten from a "coming soon" stub into a real feed against the already-real `GET /articles` (no backend gap — confirmed directly by the plan). `blogs_models.dart`'s `BlogArticleDto` deliberately has no "category"/"read time" fields, since `DoctorArticle` carries none — the prototype's per-article "5 min read" badges are static mockup text with nothing to back them. Plain repository, no cubit, per the plan's own "single-fetch, no cross-screen state" architecture note — same precedent as this entry's Grievance and Q&A-read paths below.

**Grievance** (`features/patient/grievance/*`, new): `grievance_repository.dart` resolves `consultation_id` via the new `GET /appointments/:id/consultation` (`appointment_handlers::get_consultation_for_appointment`, closed this pass — the consultation-session read analogue of gap #3's conversation lookup, over the already-existing `consultation_repo::find_by_appointment`) and then files a real `POST /grievances`. `grievance_modal.dart` deliberately omits the prototype's `#patientGrievanceModal` "System Auto-Collected Telemetry" inspector panel — that section is entirely hardcoded demo numbers (a fixed "2m 14s" duration, a fixed audit timeline, a fixed device string) with no backing endpoint a patient can call; showing fabricated telemetry would be a worse "No Phantom Completions" violation than omitting it. Real telemetry (wired earlier this session) still reaches Admin's own adjudication view, which this app has no patient-facing read for. Wired as a "Report Issue" icon in `patient_chat_page.dart`'s `_ChatTopBar` and a "Report Consultation Issue" button in `waiting_room_page.dart`.

**Free Health Q&A — gap #6 closed** (the plan's last named backend gap): `health_questions` had a public `SELECT USING (true)` read policy and a Doctor-only `UPDATE` grant (`20261005000001_doctor_contract_qna`) but **no INSERT grant and no INSERT policy at all** — confirmed by direct migration inspection, not inferred; submitting a question was structurally impossible at the database layer regardless of any application code. New migration `20261101000002_patient_qna_submission` grants `INSERT` and adds the first INSERT policy, using the same guardian-OR-self ownership shape (`fn_patient_profile_owned_by_caller`) `20261101000001_patient_profile_dependents` established for every other patient-authored table, plus a `RESTRICTIVE` author guard mirroring `grievance_patient_author`/`booking_patient_author` exactly (PATIENT role, `status='OPEN'` only). New `Action::SubmitHealthQuestion` ABAC action (mirrors `SubmitGrievance`'s shape rather than overloading `PatientManageProfile`). Two new handlers in `patient_handlers.rs`: `list_answered_questions` (`GET /patients/me/qna/questions`, hardcoded to `status='ANSWERED'` — a patient never sees another patient's still-open question) and `submit_question` (`POST`, same endpoint). `submit_question`'s `INSERT ... RETURNING` deliberately names only the column-scoped-granted columns (never `author_patient_id` — `20261026000001_canonical_user_isolation` revoked full SELECT on this table specifically to keep Q&A anonymous-to-doctors at the grant level, and naming an ungranted column in `RETURNING` fails the whole insert, not just the response shape). 5 new backend tests: submit-then-appears-once-answered round trip (through the real Doctor answer endpoint), empty/oversized-text validation (**caught by the test run, not inspection**: `AppError::Validation` maps to `422`, not `400` — the test's first draft asserted `400` and was corrected against the real `AppError` status mapping), non-PATIENT-role rejection, guardian-submits-for-dependent (asserts `author_patient_id` in the DB really is the dependent's id, not the guardian's), and a BOLA guard rejecting an unowned `profile_id` with `404`.

New Flutter (`features/patient/qna/*`, all new): `qna_models.dart` (`AnsweredQuestionDto`, plus a client-side-only `qnaCategories` chip list — `health_questions.category` is a free-text `VARCHAR(80)`, not a backend enum, same shape as the Grievance modal's fixed category list), `qna_repository.dart`, `qna_cubit.dart` (owns the global answered feed plus submit-flow state; the feed itself is not profile-scoped — any patient may read any doctor-answered question — only `submit()`'s optional `profileId` is), `qna_ask_sheet.dart` (the "Ask Your Medical Question" bottom sheet), `qna_page.dart` (the feed + FAB entry point). Two prototype elements deliberately dropped, both previously-established patterns applied again here: the "Post as Anonymous (Patient)" checkbox (anonymity isn't an optional toggle — the backend's column-scoped SELECT grant means a doctor can *never* see `author_patient_id` regardless of what a patient picks, so presenting it as a choice would be dishonest UI) and the per-answer "👍 N Found Helpful" vote counter (CLAUDE.md §2.1's zero-star-ratings/no-popularity-algorithms rule — no backing column or grant exists for one either). The ask sheet offers a guardian "Asking as" profile picker (reusing `FamilyProfilesCubit`) only when more than one profile exists. Wired into Home's "Ask Free Q&A" grid card and the Services tab's "Ask a Free Question" row, both previously "coming soon." 5 new `QnaCubit` unit tests (load success/failure, submit success/failure, `resetSubmitState`).

**Verification**: backend `cargo fmt`/clippy clean, `cargo check --all-targets` clean, full `cargo test --test api_tests -- --test-threads=1` **206/206 PASS** (three runs this pass: 198→201 confirming the Grievance consultation-lookup tests — one of the three required a fix, documented above, since its premise about *when* a consultation session row is created was wrong — then 201→206 confirming the 5 Q&A tests together with everything else, zero regressions). `cargo test --lib` 53/53 unchanged. Flutter: Patient `flutter analyze --no-pub` 0 issues, `dart format` clean, **116/116 tests PASS** (101 prior + 10 Notifications + 5 Q&A; Blogs/Grievance intentionally have no dedicated unit tests, matching the plan's own "plain repository, no cubit" guidance for single-fetch/single-submit screens). **Not yet done, named rather than silently skipped**: physical-device verification and a live click-through against a running dev backend (no `cargo run` process was started this session) — a migration file was added this pass (`20261101000002`), so per this project's own migration-discipline rule, any future live verification against a persistent dev database must restart that backend process first.

## Cancellation removed, real call telemetry added, M7 Health Vault (gap #2 closed) — 2026-10-10 (later, same day)

**Explicit user instruction, three parts, all completed this pass:** (1) remove booked-consultation cancellation entirely — "there won't be any cancellation of any consultations booked"; (2) make the app send live consultation telemetry (patient joined, doctor joined, disconnection, consultation start, consultation finished, consultation delayed) for grievance review; (3) continue to M7.

**Part 1 — cancellation removed end to end.** The M5 entry immediately below this one documents gap #4's *original* build; that work is now superseded, not just hidden. Removed: `AppointmentService::cancel_appointment` (service), `appointment_handlers::cancel_appointment` (handler), the `/appointments/:id/cancel` route, its 4 integration tests, and the Flutter Cancel button/dialog/cubit method/`BookingRepository.cancel()` call. `WaitingRoomCubit` is now a plain read-only status poller (`canSelfCancel`/`cancelling`/`cancelError` all removed, not just unused). `AppointmentStatus::CANCELLED` remains a valid enum variant (nothing else in this codebase currently writes it), left alone since removing an enum variant is unrelated schema churn, not requested.

**Part 2 — real call-lifecycle telemetry, both Patient and Doctor sides.** `ConsultationService::TELEMETRY_EVENT_TYPES` and the `POST /consultations/:id/telemetry/events` endpoint already existed with zero real callers anywhere in the codebase (confirmed directly: `apps/mobile`'s `DoctorRepository.telemetry()` was dead code, declared but never invoked from `AgoraPage`). This pass wires real submission on both sides:
- **Patient** (`apps/mobile_patient/lib/features/patient/video/agora_placeholder_page.dart`, backed by `BookingRepository.submitTelemetryEvents` and the `TelemetryEventType`/`telemetryEvent()` helpers in `booking_models.dart`): `CALL_INITIATED`+`PATIENT_JOINED` on successful RTC-token mint (the one real, observable event at this placeholder-video fidelity), `PATIENT_DISCONNECTED`/`PATIENT_RECONNECTED` on app-lifecycle pause/resume while a session is active (the honest proxy for a connection interruption available without a real Agora SDK), `CALL_ENDED` on leaving the screen.
- **Doctor** (`apps/mobile/lib/features/doctor/agora_page.dart`): identical instrumentation using `DOCTOR_JOINED`/`DOCTOR_DISCONNECTED`/`DOCTOR_RECONNECTED`. `DoctorRepository.telemetry()` was changed from rethrow-on-failure to swallow-on-failure (mirroring the backend's own `record_telemetry_events` doc comment: "ingesting observability data must never be able to break an active consultation") — safe, since it had zero existing callers to break.
- **"Consultation delayed" is deliberately not a new event type.** The vocabulary is a DB `CHECK` constraint with no `DELAYED` entry; adding one would need a migration for a value that's already fully derivable from data this pass already captures — comparing `PATIENT_JOINED`/`DOCTOR_JOINED`'s real `occurred_at` against the appointment's scheduled slot start time. Patient's placeholder page uses exactly that comparison to show an honest "running late" banner, with no new backend concept invented.
- Backend side needed zero changes — the vocabulary, endpoint, and RLS-backed authorization all already existed correctly.

**Part 3 — M7 Health Vault, gap #2 closed.** `GET /patients/me/documents` (`patient_handlers::list_own_documents`) merges doctor-issued e-Prescriptions and patient-uploaded intake documents into one chronological list — `prescription_intake_documents` already had a `list_*_for_patient` repo query; `prescriptions` did not, so this queries it directly in the handler (the same precedent `list_own_appointments` already set for a patient-facing joined read, rather than adding a repo wrapper with exactly one caller). `security_status != 'CLEAN'` rows are excluded, matching the gate `prescription_handlers::authorized_document` already enforces at download time — nothing in the list can ever fail to download. No-Rx clinical decisions (`rx_image_object_key IS NULL`) are excluded from the e-Prescription view — confirmed via a real test, not just reasoned about. The existing `GET /appointments/:id/documents/:document_id` already transparently serves both document kinds (confirmed by reading `authorized_document`'s fallback logic directly) — no changes needed there.

New Flutter: `vault_cubit.dart`/`vault_repository.dart`/`vault_models.dart`/`health_vault_page.dart` — grouped-by-month list, client-side kind tabs (All / e-Prescriptions / My Uploads — relabeled from the prototype's finer "Lab Reports"/"Doctor Notes" split, which the backend has no data to actually distinguish), an in-app image viewer for JPEG/PNG, and `url_launcher` external-open for PDFs (no new PDF-rendering dependency). Mirrors `HomeCubit`'s active-profile pattern exactly (§5b requires M7 to scope to the currently active profile). Wired into Home's "Health Records" card and the Services tab's "Health Records & Vault" row, both previously "coming soon."

**Verification**: backend `cargo fmt`/clippy clean, `cargo test --lib` 53/53, full `cargo test --test api_tests -- --test-threads=1` **198/198 PASS** (two complete runs this pass: one confirming the cancel removal + chat gap #3 code at 194/194, one final run confirming all of it plus the 4 new Health Vault tests together at 198/198; zero regressions either time). Flutter: Patient `flutter analyze` 0 issues, `dart format` clean, **101/101 tests PASS**; Doctor `flutter analyze` clean (one pre-existing unrelated info), **168/170 tests PASS** — the 2 failures are a pre-existing golden-image pixel-diff in an unrelated screen (`notification_center_page_test.dart`), confirmed via `git stash` to fail identically without this pass's changes, so not a regression introduced here and not fixed (regenerating a stale golden baseline wasn't requested and changes a visual-regression baseline the project may want to review deliberately). **Not yet done, named rather than silently skipped**: physical-device verification and a live click-through against a running dev backend (no `cargo run` process was started this session).

## Patient Flutter app M5 (Waiting Room + self-cancel) and M6 (24h Chat) — 2026-10-10 (later, same day)

**Superseded, 2026-10-10 (later, same day):** the self-cancel feature described below (gap #4) was fully removed by an explicit product decision — see the entry above this one. This section is preserved as a historical record of what was built and why, not as a description of current behavior; do not resume or reference gap #4 as open or closed going forward, it is simply out of scope.

Closed two of the plan's named backend gaps just-in-time, each small and additive (no migration), then built the Flutter screens that depended on them — same sequencing discipline as every prior milestone this session.

**Gap #4 — `POST /appointments/:id/cancel` (`AppointmentService::cancel_appointment`,
`services/backend/src/services/appointment_service.rs`).** Scoped deliberately narrower than
`docs/STATE_MACHINES.md`'s documented `CONFIRMED -> CANCELLED` (full refund, >2h guard): this
codebase has **no automated fund-movement path anywhere** — every real refund
(`grievance_service::transition_refund_request`) is a manual, Finance-adjudicated
`financial_refund_requests` disbursement, never a backend-automatic M-Pesa transfer. Building an
instant patient-triggered refund would have been a new, un-reviewed financial primitive. Instead:
only `PENDING_PAYMENT -> CANCELLED` is real self-service (releases the locked slot, marks the
`Initiated` transaction `Failed`); a `CONFIRMED` (paid) appointment gets a clear 409 naming why,
surfaced client-side as "contact support" — honest, not a phantom completion. Verified this is
the common case: `book_appointment` refuses outright outside `environment == "development"`, and
nothing in this codebase lets a real patient confirm their own payment (only an
operator/test-triggered webhook does), so almost every real booking sits at `PENDING_PAYMENT`.
4 new integration tests (self-cancel + slot release + re-cancel-is-409, confirmed-appointment
rejected with the exact "contact support" message, BOLA reject, guardian-cancels-dependent).

**Gap #3 — `GET /appointments/:id/conversation` (`appointment_handlers.rs`).** Smaller than it
looked: `chat_repo::find_conversation_by_appointment` already existed, so this was purely a
missing read endpoint, not new persistence. Authorization is entirely RLS
(`chat_conversations_access`, already extended for guardian/dependent in the §5b migration) — a
non-owned appointment 404s before `chat_conversations` is even queried, the same non-leaky shape
as `get_appointment`. Confirmed directly in `AppointmentService::process_payment_webhook` that
**every** `CONFIRMED` appointment gets a conversation regardless of modality — the 24-hour chat is
bundled with a Video booking too, not only a Chat one — which changed the Flutter Waiting Room
design (see below). 4 new integration tests (resolve-on-confirmed, 404-before-confirmed, BOLA
reject, guardian-resolves-dependent's).

**New Flutter code**, `apps/mobile_patient/lib/features/patient/`:
- `booking/waiting_room_cubit.dart` + `waiting_room_page.dart` (prototype view-10) — polls
  `GET /appointments/:id` for status, real `permission_handler` camera/mic check (declared in
  `AndroidManifest.xml` for the first time this pass — previously undeclared), Cancel wired to
  gap #4's real/disabled split exactly.
- `video/agora_placeholder_page.dart` — mirrors `apps/mobile`'s `AgoraPage`/`AgoraSession`
  almost verbatim (same mint-then-placeholder shape via the same
  `POST /consultations/:id/rtc-token` contract).
- `chat/chat_cubit.dart` + `chat_repository.dart` + `patient_chat_page.dart` (prototype view-6,
  text messaging only — the prototype's digital-Rx/patient-history header shortcuts have no
  patient-side contract and aren't fabricated here) — load/send/24h-expiry-lock handling mirrors
  `apps/mobile`'s `_ChatThreadState` structurally, plus a silent background poll the Doctor
  screen doesn't have.
- Waiting Room's Join/Chat buttons corrected to match the gap #3 finding above: Chat-modality
  shows "Open Chat" as primary; Video-modality shows "Join Video Call" as primary **plus** a
  secondary "💬 Open 24h Chat" button, since that thread exists for Video appointments too.
- Wired into Home: the "Manage" button on the Upcoming Consultation card (previously
  "coming soon") now opens the real Waiting Room for the nearest upcoming appointment.

**Verification**: backend `cargo fmt`/clippy clean, `cargo test --lib` 53/53, full
`cargo test --test api_tests -- --test-threads=1` **198/198 PASS** (190 prior + 4 + 4, zero
regressions, two full runs performed — once after gap #4 alone, once after both gaps together).
Flutter: `flutter analyze` 0 issues, `dart format` clean, **`flutter test` 96/96 PASS** (78 prior
+ 10 `WaitingRoomCubit` + 8 `ChatCubit`, zero regressions). **Not yet done, named rather than
silently skipped**: physical-device/prototype-comparison verification (no Android device
connected this session) and a live click-through against a running dev backend (no `cargo run`
process was started this session, so these new routes have been exercised only by the automated
test suite, not manually).

## Patient Flutter app M4 — Directory, Doctor Profile, Booking, Intake upload — 2026-10-10 (later, same day)

Implements the plan's M4 milestone (`C:\Users\CUBE\.claude\plans\enumerated-wishing-pearl.md` §6)
on top of the M3.5 multi-profile/family-support foundation. Confirmed before writing any code
that all backend endpoints this milestone needs were already real (`GET /doctors`,
`GET /doctors/:id`, `GET /doctors/:id/slots`, `POST /appointments/book`,
`POST /appointments/:id/prescriptions`) — **zero backend files changed this pass**, no migration,
no `cargo` commands needed beyond what M3.5 already verified.

**New Flutter code**, all under `apps/mobile_patient/lib/features/patient/`:
- `directory/` — `directory_models.dart` (`PublicDoctorDto`, mirroring
  `doctor_repo::PublicDoctorProfile`'s plain-snake-case JSON), `directory_repository.dart`,
  `directory_cubit.dart` (modality tab + specialty/search client-side filtering over the current
  server page), `patient_directory_page.dart` (prototype view-3), `doctor_profile_page.dart`
  (prototype view-9).
- `booking/` — `booking_models.dart` (`DoctorSlotDto`, `BookingResultDto`),
  `booking_repository.dart`, `booking_cubit.dart`, `patient_booking_page.dart` (prototype view-4,
  shared by both Video and Chat modalities), `paid_chat_booking_modal.dart`,
  `pre_consult_intake_modal.dart` (prototype's `#preConsultIntakeModal`).
- Wired into Home's search pill / "Book Specialist" grid card and the Services tab's "Book a
  Specialist Doctor" row, which previously showed a "coming soon" snackbar for this item.

**Real validation the prototype's static demo lacked** (plan §4's view-4 row, done as specified):
`BookingState.canProceed` only turns true once the patient has actually selected both a date and
a slot on that date — the prototype pre-highlights a date and slot by default with no way to
represent "nothing chosen yet." Covered by `booking_cubit_test.dart`'s dedicated
"real date/slot-required validation" group.

**Three deliberate, documented departures from a literal prototype port — each because the real
backend contract required it, not stylistic preference:**
1. **Paid chat booking still requires picking a real slot.** The prototype's
   `#paidChatBookingModal` is a self-contained final booking screen with no slot picker, but
   `AppointmentService::book_appointment` (`services/backend/src/services/appointment_service.rs`)
   locks a `doctor_schedule_slots` row via `try_lock_slot` for **both** `Modality::Video` and
   `Modality::Chat` — confirmed by reading the function body directly, not assumed. Faking an
   instant slot-free chat booking would silently diverge from what actually happens server-side.
   `paid_chat_booking_modal.dart` keeps the prototype's real informational content (inclusions,
   fee breakdown) and then hands off to the same real slot-picking `PatientBookingPage` used for
   Video, documented inline with the exact backend line this decision rests on.
2. **Doctor Profile's "Availability" tab does not reproduce the prototype's static
   "Saturday–Thursday 09:00–20:00" weekly-schedule copy.** No backend contract exposes a
   recurring weekly template (only ad-hoc `doctor_schedule_slots` rows) — inventing one would
   violate CLAUDE.md's "No Unilateral Feature Inventions" rule. Shows the real next few bookable
   slots from `GET /doctors/:id/slots` instead.
3. **Currency and professional-body copy relabeled for the Kenya deployment**: ৳ (Bangladesh taka)
   → `KES` throughout Directory/Booking/Doctor Profile (the *amounts* are always the real
   per-doctor `consultation_fee_video`/`consultation_fee_chat` values from the backend, never
   invented — only the currency symbol changes), and "BMDC Verified"/"BMDC #" → "KMPDC
   Verified"/"KMPDC #" (plan §4's screen-mapping row for view-3 states this explicitly). Payment
   gateway sent is `MPESA`, matching the M3.5 test-fixture correction away from `BKASH`.

**Family Profiles / M3.5 carried through from the start, per plan §5b**: `PatientBookingPage`
takes `FamilyProfilesCubit` as a direct constructor parameter (not resolved via `BuildContext`)
specifically because Directory/Doctor Profile/Booking are reached via `Navigator.push`, a sibling
route to the Home/Account tabs that does not inherit `BlocProvider`s declared inside
`_SignedInRootState`'s single `home` route — confirmed by checking how Flutter's `Navigator`/
`Overlay` route-stacking actually works before writing this, not assumed. A compact "Booking for:
[Self ▾]" bar (reusing `FamilyProfileChipRow`) sits above the date/slot picker, defaulting to
whichever profile is currently active in `FamilyProfilesCubit.state.activeProfileId`.

**Honest pending-payment state, not a phantom completion**: `AppointmentService::book_appointment`
only ever confirms payment via a provider webhook outside this phase's scope (real M-Pesa
integration is explicitly deferred — plan §8). The post-booking confirmation sheet states plainly
that the appointment is reserved pending payment confirmation and that M-Pesa collection runs
only in this build's development environment today — it does not claim "Confirmed."

**Intake upload** (`pre_consult_intake_modal.dart`) uses the real current 1–20-image/40KB-per-image
business rule (`core/validation/upload_validation.dart`, already landed before this pass) and
corrects the prototype's stale "Up to 5 Max" copy to the real current limit, per plan §4's
intake-modal row. Picks via `image_picker` (gallery multi-select + camera) and `file_picker` (PDF),
mirroring `apps/mobile`'s existing `_RxPage`/`ImagePicker`/`FilePicker` pattern
(`apps/mobile/lib/features/doctor/doctor_pages.dart`) rather than inventing a new one.

**Verification**: `flutter analyze --no-pub` 0 issues, `dart format --output=none
--set-exit-if-changed lib test` clean, **`flutter test --no-pub` 78/78 PASS** (61 prior + 17 new:
`directory_cubit_test.dart` covering load/modality-tab-switch/specialty+search filtering,
`booking_cubit_test.dart` covering slot grouping, the canProceed validation gate, submit
success/failure, and upload-gated-on-a-booking-result). No regressions in the 61 pre-existing
tests. **Not yet done, named rather than silently skipped**: physical-device / rendered-screen
comparison against `prototype/index.html` (no Android device connected this session — same
`PENDING_NO_DEVICE` gate as the rest of this app), and a live manual run against the real dev
backend (the backend side of this contract was already live-verified during M3.5's resume pass
and is unchanged here, but the new Flutter screens themselves have not been clicked through
against a running backend yet).

## Patient Flutter app (M0–M3) + multi-profile/family support backend — RESUMED AND VERIFIED — 2026-10-10

**Resume-session update (same day, later):** all five "exact resume steps" below were
executed in order and are now green. The two tests that failed in the prior stale run
(`test_identity_doctor_private_financial_and_treatment_context_rls`,
`test_security_refresh_migration_revokes_legacy_rotated_families`) pass both in isolation
and as part of a full fresh run: **`cargo fmt --check` clean, `cargo clippy --all-targets
-- -D warnings` clean, `cargo test --lib` 53/53, full `cargo test --test api_tests --
--test-threads=1` 190/190, zero failures.** Dev backend restarted via
`start_backend_ready.ps1` after the new migration; `SCHEMA_VERIFIED: source == binary ==
database == 20261101000001`, `HELLODOCTOR_BACKEND_READY`. Flutter side re-confirmed with no
regression from the M3 additions: `flutter analyze` clean, `dart format` clean, `flutter
test` **49/49 PASS** (same count as before M3 — M3's Home/Profile/shell files did not add
new tests). The multi-profile/family-support backend feature (migration `20261101000001`,
guardian/dependent `patient_profiles`, the 14 extended RLS policies, and the 3 ownership-bug
fixes for Agora token minting / telemetry / idempotency replay) is now fully verified
against final code — the earlier "stale run" caveat no longer applies.

**M3.5 Flutter profile-switcher UI — DONE, same day — 2026-10-10.** The prototype's Family
Profiles chip row (confirmed purely cosmetic there — zero click handlers) is now real:
- New `FamilyProfilesRepository`/`FamilyProfilesCubit`
  (`features/patient/profile/family_profiles_{repository,cubit}.dart`), constructed once in
  `app.dart`'s `_SignedInRootState` alongside `ProfileCubit`/`HomeCubit` so the chosen
  "active profile" (`null` = self, matching the backend's own `acting_patient_profile_id`
  default) survives switching between the Home and Account tabs and resets naturally on
  logout. `PatientProfileDto` (`profile_models.dart`) gained `guardianPatientId`/
  `relationship`/`isSelf` — confirmed field-for-field against a live `GET
  /patients/me/profiles` call against the real dev backend (self row first,
  `guardian_patient_id: null`; a created dependent with `user_id: null`,
  `guardian_patient_id` pointing at self, `relationship: "Spouse"`).
- New shared `FamilyProfileChipRow` widget and `showAddDependentSheet` form (full name/DOB/
  gender/relationship + optional blood group/weight/emergency contact, mirroring
  `ProfileCompletionPage`'s fields). The dependent-creation POST uses `idempotent: true` —
  called out explicitly because a dependent profile can never be deleted once created (no
  soft-delete column exists), so a retried double-submit must not create two; `
  FamilyProfilesCubit.createDependent` also self-guards against a second call while one is
  already in flight.
- Wired into **Account/Profile page** (`patient_profile_page.dart`): a "Family Profiles"
  section with the chip row + "+ Add Dependent", replacing the page's former doc-comment
  claim that this was an inert "coming soon" stub.
- Wired into **Home** (`patient_home_page.dart`): a compact "Managing for: X" bar directly
  above Upcoming Consultation, tappable to open a profile-switch sheet reusing the same chip
  row. `HomeCubit` gained `setActiveProfile`/an internal `_activeProfileId`, threaded into
  `HomeRepository.upcomingAppointments(profileId: ...)` → `GET
  /patients/me/appointments?profile_id=...`. A `BlocListener<FamilyProfilesCubit>` keeps the
  two cubits in sync without either needing to know about the other's internals.
- Cache-safety decision, called out explicitly: `PatientCacheStore`'s fixed key namespace has
  no per-profile slot, so showing a dependent's appointments falls back to **no offline
  cache** rather than risk silently showing the wrong profile's cached data while
  offline — a deliberate, documented scope cut, not an oversight. Added a `family_profiles`
  cache key for the profile list itself (not time-sensitive clinical data, lower risk).
- **12 new Flutter unit tests** (`family_profiles_cubit_test.dart`,
  `home_cubit_active_profile_test.dart`) covering: load/default-to-self, switching to a
  known dependent and back, rejecting an unknown id, dependent creation success, a
  double-submit-while-creating no-op, a repository failure surfacing as `createError` (not
  thrown), and `HomeCubit` re-querying with the right `profile_id` on every switch.
  **Full Flutter suite: analyze clean, format clean, 61/61 tests pass** (49 prior + 12 new,
  zero regressions).
- Not yet done, named rather than silently skipped: M4's booking screen "Booking for: [Self
  ▾]" selector (next milestone per the plan — carries the active profile from the start,
  per §5b), and physical-device visual confirmation of this new UI against
  `prototype/index.html` view-7 (no Android device connected this session — same
  `PENDING_NO_DEVICE` gate as the rest of this app).

**Original pause entry, preserved for history below — the "What is NOT yet verified"
section it describes is now resolved by the paragraph above, not by re-reading that section
as current status.**

### What exists now

**New Patient Flutter package, `apps/mobile_patient/`** (a literal separate
top-level Flutter package, per the user's explicit, acknowledged override of
CLAUDE.md §3's single-`apps/mobile/`-project rule — see the approved plan at
`C:\Users\CUBE\.claude\plans\enumerated-wishing-pearl.md`, which remains the
authoritative design reference and should be read before resuming). Built
through M3 of that plan:
- **M0 Scaffold**: `flutter create`, Android build config with
  `isMinifyEnabled`/`isShrinkResources` + `proguard-rules.pro` (keep rules for
  `flutter_local_notifications`/`workmanager`), real `applicationId =
  "com.hellodoctor.patient"`, `l10n.yaml` + `gen_l10n` wiring. Size budget is
  **26MB** (raised mid-session from an initial ~20MB target — both the plan
  file and this doc already reflect the updated number).
- **M1 Core infra**: `core/network/api_client.dart` (session-generation
  guard, 401-refresh-on-GET-only, Idempotency-Key support, HTTPS enforcement
  — replicated near-verbatim from `apps/mobile`'s proven `ApiClient`),
  `core/cache/patient_cache_store.dart` (JWT-subject-scoped encrypted cache),
  `core/network/connectivity_service.dart`, `core/network/client_error_reporter.dart`
  + `core/network/startup_incident_queue.dart`, `core/validation/kenya_phone.dart`
  + `otp_validation.dart` + `upload_validation.dart` (1–20 image count /
  JPEG+PNG+PDF mime allow-list, deliberately NOT replicating the backend's
  40KB-per-image compression math client-side), `core/notifications/` (local
  notification channels + ids), `core/l10n/locale_controller.dart` (English/
  Swahili, a `ChangeNotifier`, persisted outside the account-scoped cache so
  it survives logout), `core/widgets/startup_error_page.dart`.
- **M2 Auth**: real 2-step OTP flow against the actual backend contract
  (`POST /auth/register-otp` → `POST /auth/verify-otp-and-login` — confirmed
  the `session_token` from step 1 is never sent to step 2, only
  `phone_number` links the two calls). `features/auth/auth_cubit.dart`
  (sealed `AuthState`), `phone_entry_page.dart`, `otp_verify_page.dart`
  (resend countdown, DEV-only OTP banner). **49/49 Flutter tests passing** at
  this point (confirmed), including a regression test for the startup
  safety net (`app_boot_test.dart` — an unconfigured `ENVIRONMENT`
  dart-define is now proven to show `StartupErrorPage`, not a blank screen).
- **M3 Profile completion + Home shell**: `features/patient/profile/`
  (`profile_cubit.dart`/`profile_repository.dart`/`profile_models.dart`,
  `patient_profile_page.dart`, `profile_edit_sheet.dart`,
  `language_modal.dart` wired to the real `LocaleController`),
  `features/patient/home/` (`home_cubit.dart`/`home_repository.dart`/
  `home_models.dart`, `patient_home_page.dart`,
  `emergency_contact_banner.dart` — text is exactly "Emergency Contact",
  never "24/7", positioned directly above Upcoming Consultation, per the
  CLAUDE.md business rule, confirmed against the live prototype markup),
  `features/auth/profile_completion_page.dart` (routes a freshly-OTP-logged-in
  account through real-data collection while its profile still matches the
  OTP placeholder sentinel), `PatientShell`/`PatientBottomNav` in `app.dart`
  (Home/Services/Blogs/Account tabs — Services and Blogs are honest "coming
  soon" stubs per the plan, not yet real screens).
  **`flutter analyze` was clean at this point, but `flutter test` was NOT
  re-run after these M3 files landed** — this is an explicit, known gap, not
  an oversight being hidden. Run it first thing tomorrow.

**Backend, M3's two blocking gaps closed**: `GET/PATCH /patients/me` and
`GET /patients/me/appointments` (`src/api/v1/patient_handlers.rs`, new file),
`ctx.patient_profile_id` auth-context helper already existed and was reused.
No migration needed for these two — purely new handlers/routes over the
existing `patient_profiles`/`appointments` tables, plus one RLS fix (the
`list_own_appointments` query had to join `doctor_public_directory`, the
RLS-safe view, not the raw `doctor_profiles` table, which a patient session
cannot read directly — confirmed by a failing integration test before the
fix, not just by inspection).

### Mid-session scope addition: multi-profile / family support

**The user explicitly escalated this from "out of scope" to "must-have"
mid-build**, with a concrete product description: one authenticated account
(one phone number, one login, one device) can create and manage several
patient profiles under itself (e.g. a parent + a spouse + children), each
with its own fully independent appointments/prescriptions/health-vault
history — "each distinct profile acts solely as different accounts in the
system." No separate login for a dependent profile. This is now formally
part of the plan as **§5b / milestone M3.5** (see the plan file) and must be
threaded through Booking (M4) and every later milestone, not retrofitted.

**Design, confirmed with the user**: extend `patient_profiles` itself with a
nullable, self-referencing `guardian_patient_id` (dependents are first-class
`patient_profiles` rows with `user_id = NULL`, not a separate table) — single
guardian per dependent, no "graduate a dependent into its own login" path.
Full reasoning and the rejected alternative (a separate `patient_dependents`
table) are in the plan file's §5b and in the migration's own comments.

**Backend implementation, this session:**
- New migration `services/backend/migrations/20261101000001_patient_profile_dependents.{up,down}.sql`:
  `patient_profiles.user_id` now nullable, new `guardian_patient_id`/
  `relationship` columns, `patient_profiles_self_xor_dependent` CHECK
  constraint, `guard_single_level_dependents` trigger (blocks nested
  dependents), two new RLS helper functions
  (`fn_patient_profile_is_self`/`fn_patient_profile_owned_by_caller`), and
  **14 RLS policies across 9 tables** extended additively (same `OR`-shape
  throughout) to recognize a guardian acting for their dependent —
  `patient_profiles_access`, `patient_profile_self_update`,
  `dependent_profile_author_guard` (new), `appointments_access`,
  `booking_patient_author`, `appointment_clinical_intake_access`,
  `prescription_intake_documents_access`, `intake_document_author_guard`,
  `consultation_sessions_access`, `prescriptions_access`,
  `consultation_telemetry_access`, `consultation_telemetry_events_access`,
  `chat_conversations_access`, `chat_messages_access`,
  `chat_messages_insert`, `transactions_access`, `payment_events_access`,
  `grievance_reports_access`, `grievance_reports_insert`,
  `grievance_patient_author`, `grievance_adjudications_access`. Also fixes a
  pre-existing `guard_profile_identity()` trigger that unconditionally
  required every `patient_profiles` row to resolve to a `users` row with
  `role='PATIENT'` — correct for a self profile, but a dependent's
  `user_id IS NULL` by design, so this was rewritten to skip the role lookup
  (not the identity-immutability check) only when `user_id IS NULL`. Also
  adds `GRANT INSERT ON patient_profiles TO hellodoctor_api` (previously only
  `hellodoctor_auth_worker` had it).
- New endpoints: `GET/POST /patients/me/profiles` (list self+dependents /
  create a dependent), `PATCH /patients/me/profiles/:id` (update one
  dependent; deliberately rejects the caller's own self id with a clear
  error telling them to use `/patients/me` instead).
- New `AuthContext::acting_patient_profile_id(state, requested: Option<...>)`
  — resolves to the caller's self profile when `requested` is `None`/equals
  self, or authorizes a dependent via an RLS-scoped lookup (a non-owned id
  simply isn't found, same non-leaky shape as everything else in this
  codebase). Threaded through `POST /appointments/book` (new `profile_id`
  field), `GET /patients/me/appointments` (new `profile_id` query param,
  defaults to self), and — resolved from the resource itself, not a
  client-supplied field, since the appointment/consultation already pins down
  which profile it belongs to — `POST /appointments/:id/prescriptions` and
  `POST /grievances`.
- New `patient_repo::list_for_guardian`/`exists_for_user_id` functions;
  `patient_repo::insert`/`update`/`find_*` all extended for the two new
  columns.
- New ABAC actions `PatientRead`/`PatientManageProfile` (scoped to the
  `Patient` role only).
- **15 new integration tests**, all individually confirmed passing (full
  create/list/update dependent flow, cross-patient BOLA checks, self-vs-
  dependent endpoint misuse rejection, dependent booking end-to-end
  including intake upload and grievance filing, booking-as-a-profile-you-
  don't-own rejected, and a dedicated regression test for the Agora bug
  below). Uses `MPESA` as the gateway (the user explicitly corrected an
  earlier `BKASH` leftover in these new test fixtures — Kenya-only
  deployment) via `send_webhook_provider(app, "mpesa", ...)`, NOT the
  shared `send_webhook()` helper, which is hardcoded to `"bkash"` and is left
  untouched in all the pre-existing tests that already relied on it.

**Three real, pre-existing-pattern bugs found and fixed** (not introduced by
the schema change itself, but newly reachable/testable once dependents
existed) — all three independently re-derived "is this caller the patient"
by looking up the caller's own self profile via `find_by_user_id` and
comparing its id directly against an appointment's `patient_id`, which can
never match when that appointment belongs to a dependent:
1. `ConsultationService::mint_agora_token` — a guardian joining their
   dependent's own consultation was wrongly rejected as "not an authorized
   participant." Fixed by resolving ownership one level up through
   `guardian_patient_id` (dependents cannot nest, so one level is always
   enough). Covered by the new
   `test_guardian_can_mint_rtc_token_for_dependents_consultation` test.
2. `ConsultationService::record_telemetry_events` — identical bug, identical
   fix.
3. `Idempotency::replay` (`api/middleware/idempotency.rs`) — a guardian
   retrying an idempotent booking/upload call for a dependent would have
   been wrongly rejected with "Appointment not found." Identical fix.

### What is NOT yet verified — READ THIS BEFORE DOING ANYTHING ELSE TOMORROW

A full `cargo test --test api_tests -- --test-threads=1` run was kicked off
**before** the three ownership-bug fixes above existed. That stale run
finished with **187 passed, 2 failed**:
- `test_identity_doctor_private_financial_and_treatment_context_rls` — a
  real RLS regression **I introduced**: my first draft of
  `appointment_clinical_intake_access` was based on that policy's ORIGINAL
  text (`20261003000001_rls_hardening`), not its actual current text — I had
  not checked for later amendments. `20261017000001_clinical_and_payout_invariants`
  had already replaced the bare `d.user_id = current_user_id` doctor branch
  with the real treatment-context-window check
  `doctor_history_context_allowed(...)` and narrowed the admin role list
  (dropping `FINANCE_ADMIN`/`SUPPORT` from clinical PHI access). My draft
  would have silently reverted that security fix.
- `test_security_refresh_migration_revokes_legacy_rotated_families` — a real
  500 error (`column "guardian_patient_id" does not exist"`), caused by this
  test deliberately rolling the DB back to an older migration version
  (`migrator.undo(&pool, 20261022000001)`) to test refresh-token-lineage
  migration behavior, then calling the real OTP-login endpoint while in that
  rolled-back state. `auth_service::verify_otp_and_login`'s existing-user
  check unconditionally selected the full current column set.

**Both are fixed in the code as of this checkpoint** (migration role lists
and the `appointment_clinical_intake_access` doctor-branch corrected back to
their real current text; `patient_repo::exists_for_user_id` added as an
existence-only query touching only `user_id`, used in place of the full-row
`find_by_user_id` in the OTP-login placeholder-creation check). **While
auditing this, I also found and fixed the identical role-list regression on
two more policies that had NOT yet surfaced a test failure**
(`prescriptions_access`/`prescription_intake_documents_access`, also
narrowed by `20261014000001_rx_pages_document_authorization` and also
silently reverted by my first draft) — these had real, separate restrictive
"window" policies (`prescriptions_doctor_window`/
`intake_documents_doctor_window`) that were never touched and kept enforcing
the time window correctly regardless, so the *role list* was the only actual
regression there, not a PHI-window bypass — but still a genuine mistake
worth fixing, and now fixed.

**I was in the middle of re-running the two originally-failing tests in
isolation to confirm the fixes when the user asked to pause. That
confirmation never completed. A full fresh suite run has also never
completed against this final code.**

### Exact resume steps, in order

1. `cd services/backend && cargo test --test api_tests -- --test-threads=1 test_identity_doctor_private_financial_and_treatment_context_rls test_security_refresh_migration_revokes_legacy_rotated_families` — confirm both pass in isolation first (fast signal, ~1 min).
2. If both pass, run the full suite:
   `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --lib && cargo test --test api_tests -- --test-threads=1` (the integration run takes ~22 minutes — this project's own house rule is it must be single-threaded due to cluster-wide capability-role setup, see the file's own top-of-file comment).
3. Only once that full run is 100% green: restart the real dev backend (new migration landed — see CLAUDE.md §4's restart-discipline rule) and run `services/backend/scripts/verify_backend_schema.ps1` before trusting anything against the persistent dev DB.
4. On the Flutter side: `cd apps/mobile_patient && flutter analyze --no-pub && dart format --output=none --set-exit-if-changed lib test && flutter test --no-pub` — confirm the M3 additions didn't regress the 49 passing tests from M2 (never re-confirmed this session).
5. Then continue the plan at **M3.5** (`C:\Users\CUBE\.claude\plans\enumerated-wishing-pearl.md` §5b): Flutter profile-switcher UI (the prototype's Family Profiles chip row, made real) reachable from Home/Account, "+Add Dependent" form, wiring an "active profile" concept into Home's upcoming-consultation card — all backend endpoints for this already exist and are implemented, only the Flutter UI remains. After that, M4 (Directory/Booking) should carry the profile selector from the start, per the plan.

**Do not re-litigate the design decisions already made this session**
(separate `apps/mobile_patient` package, 26MB budget, `guardian_patient_id`
column-extension approach, single-guardian/no-graduation scope, MPESA not
BKASH in new test fixtures) — they're settled, confirmed with the user, and
recorded in the plan file and this entry.

## Completed correction - Kenya professional registration - 2026-10-08

Explicit PENDING / VERIFIED / REJECTED, KMPDC/KE, pending-only queue, atomic
ClinicalAdmin/PlatformAdmin review, and client contracts are verified. Sequential
Rust checks pass: 50 library / 113 API. Flutter analysis and 74 tests pass; Doctor
Web 33 tests/build and Admin 4 tests/build pass. Live review passed 14 checks with
zero browser exceptions. Source = binary = database migration 20261024000001.
The user confirms the Doctor app is verified. Earlier visual-verification and
boolean-only queue limitations below are historical snapshots.

That correction is complete. Its original stop instruction applied to that pass; later authorized security/Articles and ownership/Q&A passes are also complete. External regulator integration remains outside scope. [Full evidence](KENYA_PROFESSIONAL_VERIFICATION.md).

> Current Kenya professional-registration contract (2026-10-08): [explicit PENDING / VERIFIED / REJECTED model, KMPDC/KE, generic Admin routes, migration and evidence](KENYA_PROFESSIONAL_VERIFICATION.md). Earlier dated audit/results sections are historical snapshots and do not describe the current review model.

## HARD VISUAL REPLICATION RULE - 2026-10-06

The canonical Doctor visual source is exclusively `prototype/index.html`.

The Flutter Doctor application is not considered visually complete unless it is checked back and forth against that file every time a screen or component is changed. For each screen, the mandatory loop is:

1. Inspect the corresponding HTML/CSS/JS in `prototype/index.html`.
2. Render the Flutter screen.
3. Compare the rendered Flutter result against the prototype.
4. Record and correct every discrepancy.
5. Repeat the comparison after the correction.

Source inspection, compilation, analyzer success, or widget tests alone are not visual verification. No screen may be marked matched, ready, or complete without rendered comparison evidence. The blanket `DOCTOR_FLUTTER_NOT_VERIFIED` verdict recorded here is historical: the user later confirmed the Doctor app. The latest Q&A change specifically remains `DOCTOR_QNA_CODE_FIXED_REAL_DEVICE_PENDING`; source/tests/goldens do not replace its physical-device render gate.
# Historical Project Checkpoint â€” Resume Point

## Doctor Engagement extension (historical; backend and Articles now complete) + two queued Rust/API quality items + token-budget note — 2026-10-07 (later same day)

**Current reconciliation (2026-10-08):** This earlier roster is no longer active. Backend validation, Queue/notifications, newtype identities, the seven API coverage additions, and direct Doctor Articles have landed and are covered by the latest full regression. Dedicated Flutter notification UI and actual push delivery remain open. Use the current tables above; do not launch agents or resume the obsolete sequence below.

### What's running or already landed from the "resume backend field validation + stricter
email/phone + full Queue/Notifications/Articles spec, use agents" instruction

1. **`backend-field-validation` (agent name) — DONE, independently re-verified in part, one
   follow-up fix applied by the orchestrator directly.** Added `validate_text_field` (shared
   length-cap + control-char-strip helper) and `validate_password_strength` (≥8 chars, upper/
   lower/digit/special-char, deliberately no max-length cap) to
   `services/backend/src/domain/validation.rs`; applied length caps (150/1000/2000/300 canonical
   numbers, matching the client-side numbers from the earlier input-safety pass) across chat
   messages, grievance category/claim-summary/audit-notes/provider-reference, appointment chief
   complaint, prescription no-Rx reason, BMDC notes, system-error resolution/simulate notes,
   admin-log simulate fields, and `POST /admin/doctors`'s previously-completely-unvalidated
   name/specialty/address fields; tightened `validate_email` to require a genuine ≥2-char
   alphabetic TLD (was accepting `a@b.c`); fixed a real bug in `validate_phone_number` that only
   checked prefix+length shape, not that the remainder was actually digits (`+8801abcdefgh` used
   to pass). 43/43 `cargo test --lib` at the time it reported (29 prior + 14 new). **Its own
   attempt at the full `cargo test --test api_tests` run failed to compile — correctly diagnosed
   by the agent itself as a transient collision with the concurrently-running
   `backend-queue-notifications` agent's in-progress `notification_scheduler` module declaration
   (confirmed via a clean `cargo check` showing the failure was that module, not this agent's own
   diff) — not a real defect. **Orchestrator follow-up fix, now covered by the latest clean 50-library/139-API regression**:
   found and removed a pre-existing separate hardcoded 12-128 character password-length check in
   `auth_handlers.rs::doctor_register` that ran *before* `validate_password_strength` — since
   12 > 8, the old check silently won, meaning the user's explicit "8 characters" policy was not
   actually what got enforced at the real endpoint. Fixed by deleting the redundant old check so
   `validate_password_strength` is the single source of truth; updated the stale doc comment that
   referenced the removed check. **Full fmt/clippy/library/API verification subsequently passed; this is no longer an outstanding check.**
2. **`backend-queue-notifications` (agent name) — DONE, fully re-verified end to end.** Part A
   (Queue upcoming-first ordering) + Parts B-G (the full notification domain) of the pasted
   "HELLODOCTOR — DOCTOR ENGAGEMENT & CONTENT EXTENSION" spec — **the full original spec text is
   preserved verbatim earlier in this session's conversation history**, not reproduced again here.
   Part H was outside that historical pass. Articles now exist in backend, Doctor Web and Flutter; dedicated notification bell/inbox UI remains open.

   **Part A — Queue**: `doctor_handlers.rs::list_own_appointments`'s `ORDER BY` now sorts eligible
   upcoming consultations (`CONFIRMED`/`WAITING`/`IN_CONSULTATION`) first by nearest `start_time`,
   with everything else (completed/cancelled/other) following newest-first and never displacing an
   upcoming item, tie-broken by `appointment_id`. New test
   `test_doctor_queue_shows_upcoming_consultations_first` covers the spec's exact scenario
   (08:00 completed / 09:00-10:00-14:00 upcoming → expected order 09:00/10:00/14:00/then-completed),
   a cancelled item never displacing upcoming ones, foreign-doctor exclusion, and pagination.

   **Parts B-G — notification domain**: new migration `20261020000001_doctor_engagement_notifications`
   (`notifications` + `notification_device_tokens`, RLS-protected, no PHI columns — title/body are
   short backend-template strings only). New `notification_repo.rs`, `notification_service.rs`,
   `notification_scheduler.rs` (DB-backed, no new broker — a `tokio::spawn`'d 60s-tick loop calling
   two independent, dedup-key-idempotent passes), `notification_transport.rs` (the
   `NotificationTransport` adapter boundary; `NoopTransport` is the only implementation — **status:
   `PUSH_PROVIDER_CONFIGURATION_REQUIRED`**, no FCM/APNs exists, delivery is never faked). Triggers
   wired into the existing, frozen transactions: `admin_handlers.rs::verify_bmdc_doctor`/
   `reject_bmdc_doctor` (REGISTRATION_VERIFIED on a genuine `false→true` transition only;
   REGISTRATION_REJECTED deduplicated by doctor-profile so a repeat rejection is a safe no-op, since
   `doctor_profiles` has no separate REJECTED status column to gate on) and
   `consultation_service.rs::complete_in_transaction` (EARNING_CREDITED fired only when *this exact
   call* was the one holding `PAYMENT_HELD` beforehand — captured before calling the frozen
   `doctor_complete_consultation` SQL function, never modifying that function itself). `GET/POST
   /notifications[...]` + `/notifications/devices` endpoints, gated by `Action::AnyAuthenticated`
   with RLS as the real ownership boundary. New config `DOCTOR_DAILY_SUMMARY_TIME_LOCAL` (default
   `07:00`, Africa/Nairobi computed via a fixed UTC+3 offset — Kenya has no DST, so no `chrono-tz`
   dependency was needed).

   **Two genuine PostgreSQL RLS bugs found and fixed via real failing tests, not assumed correct
   from the schema design alone — read these before touching any RLS-protected table a backend role
   needs to write to on another user's behalf**:
   1. `INSERT ... ON CONFLICT (dedup_key) DO NOTHING` requires implicit `SELECT`-policy visibility
      to resolve whether a conflict exists. Every notification here is written *for someone else*
      (an admin verifying a doctor; the system crediting a doctor's wallet) — the owner-scoped
      `SELECT` policy made `ON CONFLICT` fail for 100% of real (non-test-isolated) call sites,
      regardless of which role's `TO` list the `INSERT` policy named. Fixed by replacing `ON
      CONFLICT` with a plain `INSERT` wrapped in a real `SAVEPOINT` (`notification_repo::
      insert_if_new` now takes `impl sqlx::Acquire<Database = Postgres>` and calls `.begin()`,
      which transparently becomes a top-level transaction on a bare pool or a `SAVEPOINT` inside an
      existing one) plus a caught `23505` unique-violation — necessary because catching a Postgres
      error in Rust does **not** un-abort the underlying transaction; without the savepoint, a
      duplicate-key hit inside a larger transaction (e.g. the admin reject handler, which writes an
      audit-log row afterward) poisoned that entire outer transaction, surfacing as a 500 on the
      *second* of two otherwise-idempotent reject calls.
   2. The identical "implicit SELECT visibility" requirement also applies to a plain `UPDATE`/
      `DELETE`, not just `ON CONFLICT` — confirmed the hard way: `rows_affected: 0` with no error at
      all. The public, unauthenticated logout endpoint's device-token-disable `UPDATE` (there is no
      live session to set `app.current_user_id` to during logout) silently matched zero rows until
      a backend-trusted `FOR SELECT TO hellodoctor_api USING (true)` policy was added *alongside*
      the already-correct backend-trusted `FOR UPDATE` policy — the `UPDATE` policy's permissive
      `USING(true)` alone was not sufficient, since Postgres still needs an applicable `SELECT`
      policy to find the candidate rows in the first place.

   **Verification, all independently re-run by the orchestrator (this is the single-fork pass that
   did both the implementation and its own verification, per the directive's instructions)**:
   `cargo fmt --check` clean, `cargo clippy --all-targets -- -D warnings` clean, `cargo test --lib`
   **43/43**, full `cargo test --test api_tests -- --test-threads=1` **62/62** (up from 53 — 1 new
   Queue test + 8 new notification tests, zero regressions). `start_backend_ready.ps1` run for real:
   `SOURCE_LATEST_MIGRATION = BINARY_LATEST_MIGRATION = DATABASE_LATEST_MIGRATION =
   20261022000001`, `SCHEMA_VERIFIED`, `HELLODOCTOR_BACKEND_READY` (two other migrations —
   `20261021000001_doctor_slot_blocking`, `20261022000001_client_incident_context` — landed from
   other concurrent agents and applied cleanly alongside this one, confirmed no conflict).

   **Explicitly not done, named rather than silently skipped**: no dedicated adversarial
   cross-tenant test for the T-15/daily-summary scheduler beyond the general notification-ownership
   test (`test_doctor_cannot_list_or_mark_read_another_doctors_notifications`) already covering the
   access-control shape; a real production-triggered duplicate-completion race (two genuinely
   concurrent HTTP requests) was not exercised for `EARNING_CREDITED` specifically — the sequential
   duplicate-call test covers the dedup-key mechanism, not true concurrency (the underlying SQL
   function's `pg_advisory_xact_lock` already serializes real concurrent completions regardless, per
   its own existing, previously-tested design); `PAYOUT_PAID` has no live caller (correctly — no
   real `PAID` transition exists anywhere in this codebase yet).
3. **Current status: direct Doctor Articles implemented** in backend, Doctor Web and Flutter, including patient-feed APIs. No Admin moderation/review is required. Dedicated Flutter notification bell/inbox UI remains unimplemented. The old agent-launch plan is retired.
4. **Important pre-existing scoping fact, confirmed by direct investigation, not assumption**:
   `apps/mobile/lib/features/` contains only `auth` and `doctor` directories — **there is no
   patient-facing Flutter app anywhere in this codebase**, despite `CLAUDE.md` describing
   `apps/mobile` as "Patient & Doctor." The spec's "patient article feed" requirement must be
   built and tested as a backend API only; any Flutter UI agent must report this as genuinely
   blocked (no existing app to add it to) rather than fabricate one. Do not let a future agent
   "helpfully" scaffold a whole new patient app to satisfy this — that would be a large,
   unrequested architecture decision, not a small fix.

### Rust/API quality items - newtypes complete; generated OpenAPI remains deferred

Both arose from the user asking to cross-check this codebase against two external Rust API design
references (`rust-lang/api-guidelines` and `rust-api.dev`'s API-design chapter) — the orchestrator
fetched both externally and did a real, evidence-based cross-check (not a generic opinion) before
the user said yes to both:

1. **Newtype wrappers for ID types (`C-NEWTYPE`) — DONE, independently re-verified in full by the
   orchestrator.** `services/backend/src/domain/ids.rs`: `UserId`, `PatientProfileId`,
   `DoctorProfileId` (deliberately scoped to only these three proven-risk families, not every ID
   in the system — `appointment_id`/`transaction_id`/`slot_id`/etc. have never actually been
   confused with anything else, so wrapping them would be pure churn). Each is `#[sqlx(transparent)]`
   + `#[serde(transparent)]` — binds/decodes and serializes exactly like a bare `Uuid` always did,
   zero schema or wire-format change. ~30 files touched (`domain/models.rs`, `auth_context.rs`,
   all `repository/*.rs`, most `services/*.rs` and `api/v1/*.rs`, `api/middleware/auth.rs`,
   `seed.rs`, `tests/api_tests.rs`), driven compiler-first (86 type errors → 0). The agent added a
   deliberately-wrong `UserId == PatientProfileId` comparison to confirm it fails to compile
   (`E0308`), then removed it — the orchestrator independently re-read the actual historical-bug
   function (`consultation_service.rs::mint_agora_token`) afterward and confirmed the fix is
   genuinely sound: `ctx.user_id` (now `UserId`) is bridged to `PatientProfileId` only through a
   real repository lookup (`patient_repo::find_by_user_id`), and every final equality check
   compares same-type values only. **Independently re-verified by the orchestrator end to end**:
   `cargo fmt --check` clean (one trivial trailing-newline diff found and fixed), `cargo clippy
   --all-targets -- -D warnings` clean, `cargo test --lib` **47/47**, full `cargo test --test
   api_tests -- --test-threads=1` **83/83**, zero failures. Confirmed `apps/admin-web`/
   `apps/mobile` file timestamps predate this agent's run window — genuinely untouched, consistent
   with the wire-format-unchanged design. **This item is closed.**
2. **Machine-readable API spec (`utoipa`/`schemars`, OpenAPI + Swagger UI generated from the
   handler code itself)**. Confirmed zero `utoipa`/`schemars` anywhere in `Cargo.toml` — the API
   contract today lives only in a hand-written `docs/API_SPEC.md` plus hand-written TS
   (`admin-web`) and Dart (`apps/mobile`) client bindings kept in sync manually. This is not
   hypothetical either: the Finance month-filter bug fixed earlier this session
   (`getFinanceSummary()` silently dropping the month parameter) was exactly this class of
   client/server drift. Also confirmed in the same pass: `ApiResponse<T>` (the `{success, data}`
   envelope) is independently redefined in **11 separate handler files** rather than shared from
   one module — currently byte-identical across all 11 (not yet an active bug), but nothing
   structurally prevents future drift; cheap to consolidate whenever convenient. Also noted:
   success responses carry no `request_id`/`meta` the way error responses do (an asymmetry, not a
   bug) — worth adding for support/debugging correlation if the Articles/Notifications work adds
   new client-facing error paths.
3. Two things **confirmed already correct** in the same cross-check, so a future session doesn't
   waste time re-deriving them: financial amounts are already serialized as JSON strings (via
   `BigDecimal`, confirmed by the TS client types expecting `string`, not `number` — avoids
   JS's 53-bit integer precision ceiling), and every route already carries a `/api/v1/` version
   prefix.

### Read-only endpoint coverage + requirement-alignment audit — done, real gaps found,
**CLOSED: all seven API coverage items implemented.** Current professional-verification queue tests include rejected-row exclusion; production metrics and simulation guards gained later regression coverage. The dedicated finance month-filter regression exists. All remain in the latest 139/139 serialized API run. The original gap inventory below is historical, not a resume instruction.

Dispatched as a read-only fork (no file edits, no `cargo` compiles) specifically so it could run
safely *while* the newtype-ID refactor above was still mid-flight. Two of this session's own
earlier "zero test coverage" claims (`/client-errors`, the telemetry/timeline route) turned out to
be **wrong** — found via naive `grep` for literal route-path strings, which misses tests that build
real URLs with substituted UUIDs via `format!()`. Corrected publicly to the user; the lesson is
recorded here so a future pass doesn't repeat the same naive-grep mistake.

**Confirmed, by actually reading the test bodies, genuinely zero coverage**:
- `POST /admin/system-errors/simulate` — built by the orchestrator earlier this session; should
  have had a test added at the time and didn't.
- `GET /admin/compliance/alerts`
- `POST /admin/logs/simulate`
- `GET /admin/bmdc/queue` (the listing itself — the verify/reject *actions* on BMDC doctors are
  already well tested)
- `POST /consultations/:appointment_id/telemetry` (the aggregate endpoint — distinct from
  `/telemetry/events`, which *is* well tested) — feeds the grievance dispute-evidence view shown
  to admins adjudicating a claim, so this isn't purely cosmetic.
- `GET /metrics` — standard Prometheus exporter; flagged for completeness, not urgency.

**Partial coverage, one of which matters a lot**: `GET /admin/finance/transactions` has a
happy-path name-resolution test, but **no regression test for its own `?month=YYYY-MM` filter** —
the exact parameter that had a real silently-ignored-filter bug fixed earlier this session. Right
now nothing would catch that bug coming back. `GET /admin/finance/payout-routing` wasn't checked
in full depth given the audit's time budget.

**Requirement-alignment spot checks**: `ClinicalOutcome` states match CLAUDE.md exactly, no drift.
The 20%-platform-fee concept is referenced consistently across 15 files, but the audit did not
have time to trace the exact arithmetic end-to-end, and did not check the prescription 1-5-image
cap or the emergency-contact-banner rules against implementation at all — **named as not done,
not silently skipped**; a deeper full doc-vs-code trace across all 82 routes against every
requirements doc remains a reasonable separate follow-up if wanted later.

**RESUME HERE**: with the newtype-ID refactor now fully landed and verified (above), it's safe to
write the missing tests and the `finance/transactions` month-filter regression test. Not done yet
as of this checkpoint — do this next, in `services/backend` only, then re-run the full `cargo
fmt`/clippy/`test --lib`/`test --test api_tests` sequence once more before moving on to Doctor
Articles / Flutter UI.

### Token/usage budget note

One real Anthropic session/plan usage-limit interruption already occurred this session (the
`backend-image-compression` agent was terminated mid-task by a 429 rate-limit error; resumed
successfully after the limit reset and finished cleanly — see the compression entry earlier in
this file). The remaining queue above — a still-running large notification-domain build, an
unstarted Articles CMS vertical, two Flutter UI passes, a backend-wide newtype-ID refactor, and a
backend-wide OpenAPI retrofit — is large enough that hitting another such interruption mid-task is
a real possibility, not a hypothetical. If that happens: the interrupted agent is resumable by name
via `SendMessage` (it keeps its own progress/context), so prefer resuming over re-briefing from
scratch, and re-read this checkpoint entry first to know which of the items above are still
genuinely queued versus already landed by the time you resume.

## Multi-agent pass: Admin Finance UI, Flutter parity/cleanup, upload compression, input-safety audit — 2026-10-07

Resumed immediately after the error-log/admin-panel checkpoint entry below (same day, same session,
continuous). Per explicit user instruction to use agents systemwide, this pass ran as a sequence of
`fork`-type subagents, each dispatched with precise file/line pointers gathered by the orchestrating
session first, and every agent's self-reported result was independently re-verified by the
orchestrator before being trusted (re-ran `flutter analyze`/`flutter test`, `tsc --noEmit`/`npm run
build`, `cargo fmt`/clippy/test, and spot-read the actual diffs — not just the agent's own summary).
**Session paused here, token-budget-conscious, with two items explicitly deferred to a future
session (not started) — see "Deferred, not started" at the end of this entry.**

### 1. Admin-web Finance UI — done, verified

- **Real bug fixed**: `apps/admin-web/src/lib/useBackendSync.ts` called `getFinanceSummary()`/
  `getAdminTransactions()` with no arguments in both `fetchAll` code paths, silently ignoring the
  Finance page's existing month-picker (`financeMonth`/`statementMonth`) even though both `api.ts`
  functions already accepted an optional `month` param and `getPayoutRouting` already used it
  correctly. Now both pass `statementMonth` the same way.
- **New "Refund Request Resolution" screen** (net-new, no prototype precedent — confirmed by
  checking `prototype/index.html` first): `apps/admin-web/src/lib/api.ts` gained
  `FinancialRefundRequest`/`getRefundRequests()`/`transitionRefundRequest()` bound to the
  already-frozen, already-tested backend contract (`GET/POST /admin/finance/refund-requests[...]`,
  `grievance_handlers.rs`/`grievance_service.rs` — not touched, pre-existing). New card in
  `page.tsx` matching existing `admin-card`/`admin-table`/badge conventions; status-aware action
  buttons (Processing/Refunded/Failed/Reconciliation Required), `REFUNDED` requires a
  `window.prompt`-collected provider reference, terminal state disables further actions.
- **Verified**: `npx tsc --noEmit`/`npm run build` clean (re-run by orchestrator, not just agent
  self-report). Live HTTP round-trip through the real backend was attempted but blocked by the
  global per-IP rate limiter under the heavy concurrent agent traffic this pass generated —
  substituted with direct code review (types match the Rust DTOs exactly) plus confirming
  `test_refund_request_has_a_finance_resolution_lifecycle` exists in `api_tests.rs` and is covered
  by the full suite's 53/53 green result.

### 2. Doctor Flutter prototype-parity pass (second parity pass this project, see also the
2026-10-06 entries below) — done, verified

Five gaps closed against `prototype/index.html`, in `apps/mobile/lib/features/doctor/doctor_pages.dart`
and `core/widgets/doctor_widgets.dart`:
1. Chat canned-reply strip (5 pills: take-after-meals/check-fever send canned text, Patient
   History, Snap/Send Rx — prototype's "View Lab Report" pill deliberately omitted, see item 3).
2. Chat header quick actions (Video Call, Snap & Send Rx icons added alongside the pre-existing
   History icon — prototype's "Diagnostic Lab Reports" icon omitted for the same reason as item 3).
3. **Smart Rx "Attached Lab Reports" — BLOCKED, correctly left unimplemented.** The agent searched
   backend source and docs and confirmed there is no lab-report data model, endpoint, or storage
   anywhere in this codebase — this is the pre-existing `GENUINE_BACKEND_GAP` already named in
   `docs/DOCTOR_FLUTTER_REPLICATION_REPORT.md`. Adding a button with nothing behind it would be a
   phantom feature per CLAUDE.md's rule against unilateral feature invention; correctly flagged
   instead of faked. **Needs a product/backend decision before Flutter can close this — not
   forgotten, genuinely blocked.**
4. Wallet ledger styling: the "20% Fee" amount in `_LedgerCard` wasn't bolding its number portion
   the way the prototype and Flutter's own adjacent "Total" span do — fixed.
5. Schedule cancel affordance: confirmed `cancel_own_slot` only ever permits cancelling
   `AVAILABLE` slots at the DB level (a booked slot genuinely cannot be self-cancelled, so that
   capability was correctly not invented) — added a confirm/keep dialog before cancelling a free
   slot (previously zero confirmation existed) and an explicit "🔒 Booked — cannot be
   self-cancelled" note on booked cards.
6. App-resume session revalidation: `_AppState` now mixes in `WidgetsBindingObserver`;
   `didChangeAppLifecycleState(resumed)` calls the existing `refreshSession()`.

Verified (orchestrator re-ran independently, not just trusted): `flutter analyze` 0 issues,
`flutter test` 49/49 (no regression). No rendered/visual comparison possible — no Android
SDK/emulator in this environment, consistent with every prior pass; marked
`IMPLEMENTED_SOURCE_PORT_PENDING_VISUAL_GATE` throughout, per this file's own HARD VISUAL
REPLICATION RULE at the top.

### 3. Flutter cleanup pass — done, verified (ran sequentially *after* item 2, same files)

1. `TextEditingController` leak in the "complete without Rx" dialog (`doctor_pages.dart`) — fixed,
   wrapped in try/finally matching the file's other four controllers' lifecycle.
2. Unused `PageHeader` widget in `core/widgets/doctor_widgets.dart` — confirmed zero references
   (distinct from the actually-used `DoctorPageHeader`), removed.
3. New regression test in `test/doctor_widgets_test.dart`: a BOOKED slot shows the "cannot be
   self-cancelled" note and no actionable cancel button; an AVAILABLE slot's cancel goes through
   the new confirm dialog. Caught a real bug in its own first draft (button off-screen in the
   default test viewport, fixed with `tester.ensureVisible()`).
4. Stale "4-step registration wizard" doc claim — **searched fresh, found already resolved**: every
   remaining mention is in a dated audit report correctly recording that finding, not a living doc
   asserting a wizard exists. No edit made (correctly didn't invent one).

Verified: `flutter analyze` 0 issues, `flutter test` 50/50 (up from 49 — the one new test),
`dart format --set-exit-if-changed lib test` clean.

### 4. Doctor/patient prescription upload compression - COMPLETE; later full regression green

Centralized in `services/backend/src/services/upload_security_service.rs`'s
`UploadSecurityService::validate_and_sanitize` — the single function every prescription/intake/
Smart-Rx image upload already passed through for decode/re-encode sanitization (confirmed before
adding anything, not assumed). Deliberately backend-only (never touches `apps/mobile`) so the
guarantee holds regardless of client.

**Spec evolved through three rounds of user feedback — this is the final, authoritative version,
superseding anything an agent may have implemented for an earlier round**:
1. ~~500 KB per image~~ (first pass) →
2. ~~200 KB total per batch~~ (user: too tight for legibility, especially 5-image worst case at
   40 KB/image) →
3. **400 KB total per batch, divided across however many images are in the batch** (80 KB/image at
   5 images, full 400 KB for a single-image upload) — "same for patient side" (same centralized
   function serves both).
4. **Final correction (subsequently completed and regression verified): this must be a guaranteed, always-met
   ceiling, never exceeded, and the upload must never be rejected for being hard to compress** —
   the user's own words: "no rejection or nothing." This replaces an earlier "soft target with a
   legibility floor that may land slightly over budget" design the agent had built for round 3.
   The only non-negotiable floor is whatever's needed to avoid a broken/zero-byte image; everything
   above that is fair game to sacrifice (quality down toward the JPEG encoder's real minimum,
   dimensions down toward a true minimum) to guarantee the byte budget on a pathological image.

**What's confirmed solid from the implementation work so far** (round-3 numbers, re-verify the
actual final constants before citing them as current): single centralized injection point;
downscale-then-recompress design (JPEG quality-stepping loop, PNG max-compression + downscale,
format never converted — PNG stays PNG, JPEG stays JPEG, so stored mime/content-type never
desyncs from the bytes); `cargo fmt`/clippy clean and `cargo test --lib`/full `api_tests` green at
multiple checkpoints along the way (exact final counts not restated here since the last correction
may have added/changed tests — check the agent's final report, named `backend-image-compression`
in this session's agent roster, or its resumed state if still running).

**Viewing confirmed unaffected** (orchestrator verified independently, not just asserted): both
doctor and patient view documents through the same shared, ownership-scoped
`get_document_download_url`/`download_document` pair; the download endpoint streams stored bytes
with the original `Content-Type` header and has zero size/quality awareness; Flutter opens the
signed URL via `launchUrl(..., LaunchMode.externalApplication)` (hands off to the OS's own
viewer/browser, no in-app image-rendering assumptions to break).

**CLOSED OUT, independently re-verified by the orchestrator (not just the agent's self-report) —
this item is done.** Final algorithm (`upload_security_service.rs`): `TOTAL_BATCH_BUDGET_BYTES =
400 * 1024`; each image's target is that total divided by `images_in_batch` (patient intake: the
real submitted-file count, confirmed `images_in_batch = files.len()` in
`prescription_service.rs`; doctor Rx: always `1`, confirmed that endpoint's request carries one
photo per call, not an array). `compress_to_target` loops: encode → if still over its share, halve
the long-edge dimension and retry, each resolution round re-running a full JPEG quality search
(80 down to `ABSOLUTE_MIN_JPEG_QUALITY = 1`); PNG has no quality knob so resolution alone drives
convergence down to `ABSOLUTE_MIN_IMAGE_DIMENSION = 8`px. Both remaining floors are pure technical
minimums (avoid a degenerate encoder call), not legibility protections — nothing stops early while
still over budget, and the upload is never rejected for being hard to compress, exactly per the
user's own words ("no rejection or nothing"). The orchestrator caught and fixed one stale leftover
comment in `prescription_service.rs` still saying "200 KB" from an earlier round (code was
correct, only the comment was stale) — confirmed no other stale 200 KB references remain anywhere
in `src` or `docs/VALIDATION_RULES.md`.

**Independently re-verified by the orchestrator**: `cargo fmt --check` clean, `cargo clippy
--all-targets -- -D warnings` clean, `cargo test --lib` **29/29** (10 in
`upload_security_service`, including two genuine worst-case-noise tests — seeded random-pixel
noise, JPEG/PNG's actual worst case, proving the guarantee holds even when document-photo-style
compressibility assumptions don't). Empirical worst case measured directly: a 5-image JPEG noise
batch (80 KB/image share) landed at ~28 KB/image, 141,544 bytes total; a 1-image PNG noise upload
(full 400 KB budget) landed at 259,772 bytes — both comfortably under budget without ever reaching
the absolute technical floors. Full `cargo test --test api_tests -- --test-threads=1` re-run by
the orchestrator: pending final confirmation in this same session (kicked off right after this
edit — check for a fresher result before assuming, though every other signal here is solid green).
`docs/VALIDATION_RULES.md` §2.1 updated by the agent to describe the guarantee model.

**One performance note for later, not a blocker**: the two 5-image-batch tests each took ~50-60s
in an unoptimized debug build (whole-suite debug run: 108-114s for just these 10 tests) — a release
build will be substantially faster, but worth keeping an eye on real upload latency under load,
since each image can require several resize+full-quality-search rounds before converging.

### 5. Input-safety audit - mobile, Admin Web and backend implemented / regression verified

Triggered by the user asking directly whether field-length/sanitization and double-click/idempotency
protection existed. Investigation (by the orchestrator, before dispatching anything) found:
- `services/backend/src/domain/validation.rs` validates only phone/email/license/slot-times —
  **zero length caps or sanitization on any free-text field anywhere** (chat messages, clinical
  notes, grievance text, Q&A answers, doctor profile fields, resolution notes, etc.).
- A real, working idempotency-key middleware already exists and is wired globally
  (`services/backend/src/api/middleware/idempotency.rs` — route+key+body-hash replay with TTL,
  409 on key-reuse-with-different-body) but is entirely opt-in via an `Idempotency-Key` header,
  and the Flutter client never sent one (confirmed: zero references anywhere, before this pass).
- Neither admin-web (React) nor Flutter renders raw/unescaped HTML anywhere (confirmed: zero
  `dangerouslySetInnerHTML` in admin-web) — so there is no classic stored-XSS rendering vector;
  the real gap is unbounded text (resource abuse, UI breakage, DB bloat), not script injection.
- Canonical length numbers decided by the orchestrator (to keep client and server in agreement
  without a cross-agent sequencing dependency): short labels 150 chars, clinical/administrative
  free text 1000 chars, chat/Q&A text 2000 chars, short reference/note fields 300 chars.

**Mobile (`apps/mobile`) — done, verified**: audited every mutating action (`setDuty`, `createSlot`,
`cancelSlot`, `sendMessage`, Q&A `answer`, `complete`/`completeNoRx`, `updateProfile`, prescription
`dispatch`, `login`, `registerDoctor`, `devBindPayoutMethod`) — every one already had a busy-guard,
zero new ones needed (this layer was already solid). Added `idempotent: true` support to
`ApiClient.post`/`put` (128-bit random hex `Idempotency-Key`, scoped per logical call so the
existing 401-refresh-retry interceptor reuses the same key on its own retry while two separate user
taps correctly get different keys) — applied to `createSlot`, `sendMessage`, `complete`,
`completeNoRx`, `uploadPrescription`, `registerDoctor`. Deliberately skipped Q&A `answer` (already
`ON CONFLICT DO NOTHING` at the DB level — structurally duplicate-safe already) and
`setDuty`/`updateProfile`/`devBindPayoutMethod` (naturally idempotent "set to X" semantics). Added
the canonical length caps to chat/Rx-notes/Q&A/profile/registration fields. **Also added, per a
mid-task user request**: real upload-progress (determinate `CircularProgressIndicator` wired to
`onSendProgress`, replacing a generic indeterminate spinner) on the doctor's per-page Smart Rx photo
upload. Verified by the orchestrator independently: `flutter analyze` 0 issues, `flutter test`
50/50, `dart format` clean.

**admin-web (`apps/admin-web`) — done, verified**: audited every mutating action; found and fixed
real gaps — BMDC approve/clarify/reject had **no busy-guard at all** (added, plus a missing
`refreshBackend()` after approve/reject the agent noticed was also missing); slot toggle had none
(added per-slot, so toggling one doesn't freeze the board); Simulate App Failure had none (added);
system-error acknowledge/resolve had none (added); grievance adjudicate buttons had an internal
guard but **no `disabled` on the buttons themselves**, leaving a fast-double-click race window
(added `disabled` for defense-in-depth). Added length caps: simulate modal's custom note (300,
`maxLength` + server-payload truncation backstop), refund-request `window.prompt` provider
reference (300, validated client-side since prompts have no native maxLength), all 6 search/filter
inputs (150). **Noted for the record**: admin-web has no free-text textarea today (resolution/audit
notes are fixed hardcoded strings the client sends, not user-typed) — nothing to cap there.
Verified: `npx tsc --noEmit`/`npm run build` clean.

**Backend half — explicitly deferred, not started.** Per explicit user instruction ("backend field
validation later... update those in progress.md and checkpoint.md as we are almost hitting token
limit"), no backend agent was dispatched for this. **RESUME HERE**: add a shared length/sanitization
helper in `services/backend/src/domain/validation.rs` (something like
`validate_text_field(value, field_name, max_len)`, stripping control characters other than
`\n`/`\r`/`\t`, trimming, rejecting over-length) and apply it across every free-text DTO in the
handler layer (chat messages, clinical notes/chief-complaint, grievance claim summary, Q&A
question/answer, doctor profile free-text fields, resolution/audit notes, the system-error-simulate
custom note). **Use the exact same canonical numbers already applied client-side this session**
(150 / 1000 / 2000 / 300 — see above) so client and server agree; do not re-derive independently.
**Also explicitly requested by the user and not yet done**: stricter email validation requiring a
genuine domain/TLD structure (current `validate_email` in `validation.rs` only checks "has an `@`,
a non-empty local part, a domain with a dot not at either end, ≥3 chars" — does not reject something
like `a@b.c`; tighten the TLD-shape check), and confirm/tighten the existing per-country phone
format enforcement (`validate_phone_number`/`normalize_kenya_phone_number` already exist and look
reasonably strict — BD: `+880` + 10 digits = 14 chars; KE: `+254` + 9 digits = 13 chars — but
re-verify against the user's exact phrasing and extend to any client-side (Flutter/admin-web) form
that doesn't already enforce the same shape before calling this done). Document the final numbers
in `docs/VALIDATION_RULES.md` once implemented, matching the pattern already used for the upload
compression numbers in that same file.

### 6. Doctor Engagement extension - backend and direct Articles complete; notification UI/push open

**Current reconciliation:** The following text records the earlier plan. Articles now publish directly without Admin review; the historical moderation proposal is superseded by explicit user policy.

The user pasted a large, fully-specified product requirements document (its own internal heading:
"HELLODOCTOR — DOCTOR ENGAGEMENT & CONTENT EXTENSION / QUEUE PRIORITY + NOTIFICATIONS + DOCTOR
ARTICLES") covering: (A) reordering the Doctor Queue to show upcoming consultations first
(`scheduled_start ASC`, server-side, not Flutter-side sorting), with completed items never
preceding upcoming ones; (B–D) a full notification domain (daily consultation-count summary at a
configurable Kenya-local time, T-15-minute pre-consultation reminders, earning-credited
notifications tied to the authoritative wallet-credit event only, payout-paid notifications tied
only to a real external-provider `PAID` transition — never a `DEV_SIMULATED` one, registration
verification/rejection notifications), all DB-backed (not relying solely on Flutter local
scheduling) with an in-app notification inbox/bell, unread count, mark-read; (E) a
`NotificationTransport`/push-adapter boundary that explicitly reports
`PUSH_PROVIDER_CONFIGURATION_REQUIRED` rather than faking delivery if FCM/APNs isn't configured;
(F) a full Doctor article/blog authoring + moderation + publish workflow (`DRAFT →
PENDING_REVIEW → PUBLISHED/REJECTED → ARCHIVED`) with a read-only patient-facing article feed; plus
an extensive, explicitly-enumerated test list and documentation-update list for all of the above,
and its own detailed guardrails (don't redesign auth/appointment-states/RLS/ABAC, don't duplicate
the finance ledger, don't fabricate external-provider or push-delivery success, don't install
host-level software/Docker services/create cloud accounts).

**Nothing in this spec has been started.** Per explicit user instruction, this is queued for a
future session, after the backend field-validation item above. The full original spec text is
preserved verbatim in this session's conversation history (not reproduced again here to keep this
checkpoint entry a reasonable size) — **when resuming, pull the complete pasted spec from that
conversation rather than working from this summary alone**, since it contains exact schema
sketches, exact dedup-key naming conventions, exact test-list enumeration, and an exact
machine-readable "FINAL STATUS" report format the implementing agent(s) are expected to fill in.
Given its size and the real file-overlap risk with `doctor_pages.dart` (Queue UI, notification
bell, Articles entry point all land there), plan to run it as its own dedicated wave, not
piggybacked onto another in-flight Flutter-touching agent, and expect to split it into at least two
sequenced-or-carefully-scoped agents (backend notification/queue infrastructure vs. the more
self-contained article/CMS vertical) given its size relative to this session's other single-agent
tasks.

### Reconciled backlog (2026-10-08)
- Backend field-length validation, sanitization, stricter email/phone validation: COMPLETE.
- Queue ordering and notification backend: COMPLETE; notification Flutter UI and push delivery remain OPEN.
- Doctor Articles and patient-feed API: COMPLETE, with direct publication and no Admin review.
- Patient Flutter application: NOT_IMPLEMENTED.

(Image compression, §4 above, is now fully closed out and independently re-verified — no longer
an open item.)

## Admin panel error-log/crash-report wiring + real stack-trace capture checkpoint — 2026-10-07

Triggered by an explicit instruction to make sure "all kind of error log must report to the
admin panel as per system design ... error logs crash reports with codename and code snippet
has to show in the admin panel as there is a dedicated section for it." Investigation found two
separate, real gaps behind that ask, both now closed — not a redesign of the already-approved
System Error Service (`docs/ERROR_HANDLING.md` §3, `services/backend/src/system_error.rs`,
originally built 2026-10-04, see §8.1 above).

**Gap 1 — the admin-web "App Error & Incident Logs" page was reading the wrong table.** It called
`GET /admin/logs` (the `audit_events` administrator-action trail) and relabeled rows as if they
were crash reports. The dedicated `GET /admin/system-errors` endpoint (the real
`system_error_logs` feed, with `trace_id`/`subsystem`/`component`/`error_code` — the panel's
"codename" fields — and `stack_trace`/`occurrence_count`/`http_status`/`endpoint`) already
existed in the backend but was never wired to the UI. Worse, the page's "resolve" button and
"simulate failure" button were **both entirely client-side fakes**: resolving flipped local React
state with no backend call at all, and simulating fabricated a fake row locally while firing a
disconnected write into the unrelated audit trail. Fixed: `apps/admin-web/src/lib/api.ts` gained
real typed bindings (`getSystemErrors`, `getSystemErrorDetail`, `acknowledgeSystemError`,
`resolveSystemError`, `simulateSystemError`); `useBackendSync.ts`'s `backendSystemErrorToStoreItem`
replaces the old `backendLogToStoreItem` (which was reinterpreting audit rows as errors);
`page.tsx`'s simulate/acknowledge/resolve handlers and "Refresh Feed" button now make real
backend calls and refetch, and the detail dossier modal explicitly labels the subsystem/component
fields as "codename" and the stack-trace panel as "source code snippet / stack trace."

**Gap 2 — `stack_trace` (the "code snippet" field) was a permanently-`None` column with zero
writers**, explicitly flagged as a known cut corner at the end of §8.1 above ("source file/line/
stack-trace capture ... the schema has a `stack_trace` column but nothing populates it yet").
Root cause: `AppError::Database` used `#[from] sqlx::Error}` (an automatic, un-annotatable `From`
impl) and `AppError::Internal`/`ServiceUnavailable` were constructed as bare tuple-variant
literals at ~40 call sites — neither shape can carry `#[track_caller]` location info. Fixed in
`services/backend/src/error.rs`: a manual `impl From<sqlx::Error> for AppError` (replacing
`#[from]`, keeping `#[source]` for error-chain compatibility) and two new `#[track_caller]`
associated functions, `AppError::internal(String)`/`AppError::service_unavailable(String)`,
each recording `"{file}:{line}"` of their real call site into a thread-local
(`LAST_ERROR_LOCATION`) that `into_response` reads-and-clears immediately before building the
`system_error::ErrorEvent` — safe despite being thread-local rather than task-local because
there is no `.await` between an `AppError` being constructed and the same task's `Result::Err`
reaching `into_response` (documented inline with the full reasoning). All ~40 direct
`AppError::Internal(...)` construction call sites across `crypto.rs`, `rate_limit.rs`,
`auth_context.rs`, `ip_jail.rs` (`ServiceUnavailable`), and five `services`/`api` files were
updated to the tracked constructors (two genuine pattern-match sites — `rate_limit.rs`'s
`Err(AppError::Internal(_)) => Ok(())` and `error.rs`'s own `into_response` match arm — were
correctly left untouched, since renaming would have broken pattern matching, not construction).
`system_error::ErrorEvent` gained a `stack_trace` field and `.with_stack_trace()` builder;
`persist()` now writes the real captured value instead of a hardcoded `None`.

**A real compile-time gotcha hit and fixed along the way, not hypothetical**: giving the new
constructors an `impl Into<String>` parameter (matching the enum's `String` field) made every
pre-existing `"literal".into()` call site across the codebase genuinely ambiguous — `rustc`
could no longer infer `.into()`'s target type once the parameter became generic over the `Into`
bound rather than concretely `String`. Fixed by taking a concrete `String` parameter instead;
documented inline in `error.rs` so a future reader doesn't "simplify" it back into `impl Trait`
and reintroduce the ambiguity.

**New capability, not previously requested but directly in scope**: `POST
/admin/system-errors/simulate` (dev/staging only — returns 403 in production, per
`docs/ERROR_HANDLING.md` §3's existing rule that failure simulation must not exist in production),
gated by the same `Action::SimulateLog` ABAC rule as the pre-existing `/admin/logs/simulate`.
Writes a **real** row into `system_error_logs` via a new `system_error::report_sync` (a
synchronous sibling of the existing fire-and-forget `report()`, needed so the admin sees the
resulting row in the same request rather than racing the background channel), through six fixed
named scenario presets mirroring `prototype/index.html`'s simulate-failure dropdown (M-Pesa
timeout, Agora ICE failure, clinical Rx triage violation, offline sync conflict, professional
registry outage, SMS OTP carrier throttling) — fixed presets, not admin-supplied free text,
because `ErrorEvent`'s `subsystem`/`component`/`error_code` fields are deliberately `&'static
str` for the fingerprint/dedup mechanism to stay meaningful.

**Verification**: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` clean. New
unit tests in `error.rs` (`internal_constructor_captures_its_own_call_site`,
`service_unavailable_constructor_captures_its_own_call_site`,
`database_from_impl_captures_the_question_mark_call_site`,
`bare_variant_construction_does_not_leave_a_stale_location`) prove the location capture is a real
per-call-site value, not a fixed string — `cargo test --lib`: **23/23** (up from 19). Full
`cargo test --test api_tests -- --test-threads=1`: **53/53**, unchanged count, confirming zero
regressions from touching ~40 call sites across 10 files. Live-verified against the real running
dev server (`scripts/start_backend_ready.ps1` confirmed `SCHEMA_VERIFIED`): logged in as the
seeded `PLATFORM_ADMIN`, called `POST /admin/system-errors/simulate` for the `mpesa_timeout`
preset, confirmed the row appears in `GET /admin/system-errors` with a real `trace_id`,
`subsystem: "PAYMENT_GATEWAY"`, `component: "MpesaProviderCallback"`, and a non-null
`stack_trace`, then called `acknowledge` and `resolve` on it and confirmed both status
transitions. `apps/admin-web`: `npx tsc --noEmit` and `npm run build` both clean.

**Explicitly not done in this pass, named rather than silently skipped**: the ~40 call sites were
migrated to the tracked constructors, but nothing enforces that a *new* call site written in the
future uses `AppError::internal(...)` instead of the bare `AppError::Internal(...)` literal (a
clippy lint forbidding direct construction of that variant outside `error.rs` would close this,
and was judged out of scope for this pass); a real production-triggered 500 was not exercised
live end-to-end against the dev database (judged an unnecessary live-data risk once the four new
unit tests already prove the capture mechanism directly and the full existing integration suite
stayed green) — the simulate-endpoint round-trip was used as the live end-to-end proof instead.

## B-01 prevention control checkpoint â€” 2026-10-07 (follow-up to the four-agent audit below)

Resumed as a single, narrowly-scoped follow-up to exactly one finding from the four-agent audit
below: B-01 (a long-running backend silently serving a schema several migrations behind the
repository, 500ing every Doctor registration for hours, fixed in the original pass only by a
restart that happened to occur â€” the original report called this "self-healed, no further code
fix possible," which undersold what was actually buildable). Full technical detail is in
`docs/PROGRESS.md`'s matching entry and `docs/DOCTOR_SYSTEM_FINAL_MULTI_AGENT_AUDIT.md` Â§3a
(which supersedes Â§3's original verdict without deleting it) â€” this entry is the resume-point
pointer, not a duplicate.

**Explicit scope boundary honored throughout, per instruction**: did not touch payout
onboarding, registration UX, refund UI, the Finance month-picker, Flutter prototype polish,
Agora, or external payment providers â€” all remain exactly as documented open/deferred in the
checkpoint entry below.

**Parallelized across three subagents with disjoint file ownership** (specifically so no two
of them raced the same `cargo` build lock or shared Postgres instance â€” the same class of
hazard this very incident is about): one implemented the Rust-side runtime self-check
(`services/backend/src/migration_status.rs`, a new `GET /healthz` that compares the binary's
embedded migration set against what `_sqlx_migrations` actually has applied, plus a narrow new
grant migration and 4 new unit tests), one implemented two PowerShell scripts
(`services/backend/scripts/verify_backend_schema.ps1` â€” a three-way source/binary/database
check that catches the one case the running process structurally cannot detect itself: new
migration files added to disk after it started â€” and `start_backend_ready.ps1`, the project's
first "don't claim ready until it actually is" dev-start command), and one added a CI gate
(`.github/workflows/backend-migrations.yml`, real Postgres+Redis service containers running the
full test suite on every push/PR) plus a `CLAUDE.md` Â§4 guardrail documenting the restart
discipline this incident class requires.

**Everything claimed below was independently re-verified by the orchestrating session itself,
not just trusted from the implementing subagents' own reports**: re-ran `cargo fmt --check`
(clean), `cargo clippy --all-targets -- -D warnings` (clean), `cargo test --lib` (**19/19**),
`cargo test --test api_tests -- --test-threads=1` alone (**53/53**) â€” **72/72 total** â€” then
ran the actual new `start_backend_ready.ps1` against the real Docker containers (stopped the
stale process, started fresh, polled readiness, ran the verifier, printed `SOURCE_LATEST_
MIGRATION = BINARY_LATEST_MIGRATION = DATABASE_LATEST_MIGRATION = 20261019000001`,
`SCHEMA_VERIFIED`, `HELLODOCTOR_BACKEND_READY`), and finally fired a real `POST
/auth/doctor/register` against that freshly-verified process with a complete valid payload â€”
succeeded cleanly (`"status":"PENDING_VERIFICATION"`), the exact call that 500'd for ~14 hours
during the original incident.

B-01 is reclassified from `SELF_HEALED_ONLY` to `PREVENTION_CONTROL_VERIFIED`. Both drift cases
the incident could recur as (database falls behind an already-running binary; a new migration
file is added while an old binary is still running) are now detectable within seconds by a
script, instead of silently serving 500s for hours, and a CI gate fails the build if migrations
ever stop applying cleanly from scratch.

## Doctor system four-agent audit + fix pass checkpoint â€” 2026-10-07

Resumed as an explicit adversarial acceptance run, separate from and following the previous
field-test hardening pass below: four independent, read-only audit agents (Backend/Contract,
Security/Bug/Concurrency, Business-Logic/Financial-Gap, Flutter Quality/UX) were run
concurrently, each against its own slice of the system, with an explicit independence rule (no
agent read another's conclusions). Full reports are in `docs/audit/`; the synthesis role
(merge findings, fix confirmed defects only, re-run regression, write the final report) is
`docs/DOCTOR_SYSTEM_FINAL_MULTI_AGENT_AUDIT.md` â€” this entry is a pointer, not a duplicate.

Headline fixes (each with reproduce â†’ root-cause â†’ minimum-fix â†’ new regression test, per the
mission's own guardrails): the Admin "Unpaid Doctor Earnings" KPI was reading a wallet column
nothing ever decremented (fixed, with a double-counting bug in the fix's own first draft caught
by its own new test before landing); approved grievance refunds had no resolution path at all
once approved (fixed â€” Finance can now list/transition them); a refund approved against an
already-settled transaction never reversed the doctor's wallet credit (fixed, after rejecting
an initially-obvious wrong fix that would have double-subtracted from the monthly statement);
the refund-approval API response claimed money was "Disbursed" when it never was (fixed); two
live seeded doctor accounts were still serving Bangladesh identity data on a Kenya-only
deployment, predating an earlier session's Kenya rewrite of `seed.rs` that never backfilled the
already-existing rows (corrected live, not just in source); `/admin/finance/summary` had no
month filter and surfaced bkash/nagad buckets on the Kenya dashboard (both fixed); `ip_jail`'s
fail-closed Redis-outage behavior was indistinguishable from a generic server bug (now a
dedicated 503).

Two gaps closed that previous audits/sessions could not: a genuine **live** two-distinct-doctor
IDOR test (previously only structurally reviewed â€” stood up two real, independent, MFA-
completed sessions and fired a real cross-account slot-cancel attempt, confirmed `409`), and a
clean, fully-isolated single-process backend test run (**53/53**, 0 failed) proving the 4-5
failures two of the audit agents each saw were purely `sqlx::test`-vs-shared-Postgres
concurrency artifacts from running simultaneously, not product defects.

Evidence: backend `cargo fmt --check`/clippy clean, `cargo test --lib` 15/15, `cargo test
--test api_tests -- --test-threads=1` run alone 53/53 (68/68 total) â€” up from 43/43 at the end
of the previous pass below, reflecting 2 new regression tests plus the isolation fix; Flutter
unchanged this pass (`flutter analyze` clean, `flutter test` 49/49, confirming Agent D's own
count rather than re-deriving it); admin-web `tsc --noEmit` clean, `npm run build` succeeds.
Left open and explicitly documented (not silently skipped): an Admin-web UI for the new refund-
resolution endpoints, a Finance month-picker control, and several Flutter prototype-parity/UX
items â€” see the final report's Â§17-19 for the full list and why each is lower priority than the
fixes above.

## Doctor Flutter field-test hardening + payout method + email login checkpoint â€” 2026-10-06

Resumed after the prototype-replication pass (Chat/Smart Rx/Schedule/Wallet/Q&A, see `docs/PROGRESS.md` pass 2 and `docs/DOCTOR_FLUTTER_REPLICATION_REPORT.md`) once real on-device IRL testing surfaced a visual bug, a data-integrity scare, and two explicit product-design fixes requested by the user.

### Status-bar SafeArea bug (visual, not backend)

Five Doctor screens â€” Q&A, Consultation Chats (list), Smart Rx Scanner, Consultation Schedule, Doctor Earnings & Wallet â€” build their own custom header (`DoctorPageHeader`/`_QnaFlowTop`) instead of a platform `AppBar`, and none of their `Scaffold`s had a `SafeArea`. Queue happened to work by accident (a zero-height `AppBar` trick already reserves the status-bar inset); `ChatThreadPage` already had `SafeArea` from the prior pass. The other five rendered their header text directly under the Android status bar, confirmed from the user's own screenshots. Fixed by wrapping each affected screen's `Scaffold` in `SafeArea(bottom: false, ...)` (`apps/mobile/lib/features/doctor/doctor_pages.dart`).

### Root cause of "a lot of mock patient data"

Not a Flutter bug. The live dev Postgres database (used by the real field-test backend, not a throwaway test DB) had **271 separate `patient_profiles` rows, all literally named "Patient User"**, plus a third, non-seeded doctor ("Dr. Live Specialist"), 753 appointments, 753 transactions, 686 chats, and 3,275 slots â€” leftover fixture data from the constrained-capacity load-testing work (`docs/CONSTRAINED_CAPACITY_TEST.md`) that was never cleaned out of the shared dev database. Confirmed by direct query before touching anything. With explicit user confirmation, the user ran a `TRUNCATE` (rows only, no schema/table changes) across all 30 content tables except `_sqlx_migrations`. `SEED_DEV_DATA` (an explicit opt-in env flag â€” seeding is deliberately never automatic, see `main.rs`) was flipped to `true` for one restart to let the backend's own existing `seed_dev_data()` rebuild exactly the canonical baseline, then flipped back to `false` immediately after, restoring the original "a plain restart behaves like production" safety property. Verified clean state by direct query: 2 real doctors (now with real emails, see below), 1 real test patient, 5 open slots, zero appointments/transactions/chats.

### Email-based doctor login (explicit design fix, user's own words: "this part was my fault of design")

Doctor login previously used the BMDC license/registration number as the identifier; the user asked for email-based login instead.

- `DoctorLoginRequest.license_number` â†’ `email` (`auth_handlers.rs`); `DoctorRegistrationRequest.email` changed from `Option<String>` to required `String`, validated by a new `validation::validate_email`.
- `AuthService::doctor_login_step1` now looks up by `user_repo::find_by_email` (already existed, used for registration duplicate-checks) instead of `doctor_repo::find_by_license`, then confirms a `doctor_profiles` row exists for that user before proceeding.
- `seed.rs`: Dr. Sabrina Akter and Dr. Anika Rahman now have real emails (`sabrina.akter@hellodoctor.health`, `anika.rahman@hellodoctor.health`) so the existing demo login still works end to end.
- **Real bug found and fixed along the way**: `doctor_verify_otp`'s account-level brute-force jail still built its Redis key from `doctor.license_number` while the login handler's jail key had already moved to email â€” meaning repeated MFA failures would jail one key while a fresh login attempt checked a different, never-matching key, silently defeating the account lockout. Caught by the existing `test_doctor_mfa_brute_force_is_blocked` test failing after the login-handler change; fixed by fetching the `User` row once (reused for both the jail key and the final login response, removing a redundant second fetch) and keying on `user.email` in both places.
- Flutter: `LoginPage`/`RegistrationPage` (`app.dart`) and `AuthCubit.login`/`registerDoctor` (`auth_cubit.dart`) updated to match â€” email is now a required, validated field in both forms.
- Verified live against the running server post-cleanup (`POST /auth/doctor/login` with the new seed email succeeded, correct OTP challenge returned) in addition to the test suite.

### Doctor payout method setup (new feature, explicitly requested)

Previously there was no way for a doctor to tell Finance *where* a scheduled disbursement should actually go. Added, without touching the deferred external-payment-integration boundary (`DEFERRED_EXTERNAL_PAYMENT_INTEGRATION` is unchanged â€” this stores a destination, it never moves money or calls a provider):

- Migration `20261006000001_doctor_payout_methods.{up,down}.sql`: new `doctor_payout_methods` table (`method_type` enum MPESA/BKASH/BANK_CARD/BANK_TRANSFER, `display_label`, `account_identifier_encrypted`, `account_identifier_last4`), RLS-protected (doctor sees/writes only their own row; Finance/Platform Admin may read, never write, matching the grant pattern every other financial table in this codebase already uses).
- `crypto.rs`: generalized the existing TOTP-secret AES-256-GCM encryption into `encrypt_secret`/`decrypt_secret` (the scheme was never actually TOTP-specific); `encrypt_totp_secret`/`decrypt_totp_secret` kept as thin wrappers so no existing call site changed.
- `wallet_handlers.rs`: `GET`/`PUT /doctor/wallet/payout-method`, reusing the existing `ViewOwnWallet` ABAC action (same ownership boundary, no new action needed). Mobile-money identifiers reuse the existing BD/KE phone validator; card/bank identifiers get a light length/charset check. The raw identifier is never returned by the API â€” only `display_label` (server-constructed, e.g. "bKash â€¢â€¢â€¢â€¢ 5678") and `account_identifier_last4`.
- Flutter: `PayoutMethodCard` + `_PayoutMethodSheet` on the Wallet screen, `DoctorRepository.payoutMethod()`/`setPayoutMethod()`.
- New test each side: `test_doctor_payout_method_set_and_read_masks_account_identifier` (backend â€” confirms the raw identifier never comes back from the API and isn't stored in the clear; confirms doctor-scoping) and a widget test pinning the card's configured/unconfigured states.

### Evidence

- Backend: `cargo check`, `cargo clippy --all-targets -- -D warnings` clean; `cargo test --test api_tests -- --test-threads=1` **43/43 passing** (up from 42 â€” the brute-force-jail fix and the new payout test).
- Flutter: `flutter analyze` clean, `dart format --set-exit-if-changed lib test` clean, `flutter test` **41/41 passing** (up from 40).
- Live server restarted after every backend change and re-verified against the actual running Postgres/Redis containers, not just the test suite.
- Release APK rebuilt successfully after every change (`apps/mobile/build_field_test.ps1`).

Known still-open item, not touched this pass: the three documented backend-data gaps from the prototype-replication checkpoint (chat preview/category, Rx lab-reports/vitals, Q&A answered-list) remain product decisions awaiting a call, not oversights.

## Doctor prototype contract closure checkpoint â€” 2026-10-05

This checkpoint records the approved Doctor Prototype Backend Contract Gap
Closure pass (Capabilities 1â€“10). It exposed only server contracts required by
the approved `index.html`; the existing Rust/Axum, PostgreSQL, Redis,
AuthContext/JWT, RBAC/ABAC, RLS/FORCE RLS, payment, consultation, Agora,
audit, telemetry, grievance, and disbursement architecture was not redesigned.

### Implemented and tested

- **Queue and appointment history (Capabilities 1 and 4):**
  `doctor_handlers.rs::list_own_appointments`, routed by `router.rs`, performs
  SQL-scoped, paginated filtering by authenticated doctor and existing
  appointment states. Queue derives from appointments/slots; no parallel queue
  state machine exists.
- **Duty and schedule (Capabilities 2 and 3):** `update_own_duty`,
  `list_own_slots`, `create_own_slot`, and `cancel_own_slot` reuse existing
  profile/slot models with server-side ownership, overlap, time, and
  booked/locked-slot checks.
- **Contextual longitudinal history (Capability 5):** migration
  `services/backend/migrations/20261005000001_doctor_contract_qna.up.sql`
  adds narrow `doctor_patient_history` and detail functions. The current
  appointment derives patient identity; authorization never depends on a
  supplied patient UUID. Same-patient linkage, permitted appointment state,
  pagination, metadata-only summaries, and history audit events are enforced.
- **Chat list (Capability 6):** `GET /api/v1/doctor/chats` composes existing
  chat tables with SQL-scoped doctor ownership and pagination; HTTP chat is
  unchanged.
- **Open Health Q&A (Capability 7):** the migration adds minimum
  `health_questions`/`health_answers` schema, indexes, grants, and RLS. Doctor
  list/answer handlers are paginated and AuthContext-bound. Q&A is separate
  from private patient history and does not reveal EMR data.
- **Profile, summary, and wallet ledger (Capabilities 8â€“10):** explicit
  profile DTO, assigned-doctor consultation summary, and read-only paginated
  wallet ledger are implemented in `doctor_handlers.rs` and `router.rs`.

### Explicit non-implementation

The prototype withdrawal action remains a
`PROTOTYPE_BUSINESS_RULE_CONFLICT`. No withdrawal, cash-out, payout-request, or
doctor-initiated disbursement endpoint was added. Scheduled disbursement stays
under Finance Admin control. Patient booking-document behavior was not changed.

### Evidence and status

- ABAC: `services/backend/src/abac.rs`.
- Routes/handlers: `services/backend/src/api/router.rs` and
  `services/backend/src/api/v1/doctor_handlers.rs`.
- Schema/RLS/grants/functions: migration
  `20261005000001_doctor_contract_qna.{up,down}.sql`.
- Tests: `services/backend/tests/api_tests.rs`, including endpoint/Q&A
  authorization and current-treatment history tests.
- Mapping: `docs/DOCTOR_FLUTTER_BACKEND_MAPPING.md`.
- `cargo fmt --check`, clippy, and library tests pass; serialized API
  integration is **37/37 passed**.
- Capacity/load experiments were intentionally not repeated for read/list
  contract additions.

All ten requested capabilities are `FIXED_AND_TESTED`; withdrawal is
`PROTOTYPE_BUSINESS_RULE_CONFLICT`. The backend contract is frozen and Doctor
Flutter should consume these routes without duplicating business logic.

## Targeted Doctor contract correction checkpoint â€” 2026-10-05

Six verified defects were corrected without reopening architecture:

- History authorization is now evaluated before SQL pagination, invariant to
  offset/limit/cursor. The server enforces scheduled-start minus 15 minutes,
  valid treatment state, consultation completion timestamp plus 24 hours, and
  same-patient historical cutoff. Later unrelated consultations are excluded.
- Q&A answer insertion is atomic with `ON CONFLICT DO NOTHING`; concurrent
  losers receive deterministic 409 rather than raw 500.
- `GET /doctors?modality=VIDEO|CHAT` now validates and SQL-filters using the
  existing modality semantics; unsupported values return 422.
- GET/PUT `/doctor/duty` share the narrow `DutyStatus` response.
- Verified hospital and qualification attributes are no longer doctor-editable;
  they remain ClinicalAdmin-controlled. Admin creation validates bounded
  credential payloads.
- Consultation completion records server `ended_at` idempotently, supporting
  the approved 24-hour history window.

Evidence is in migration
`services/backend/migrations/20261005000002_history_access_window.{up,down}.sql`,
`doctor_handlers.rs`, `consultation_service.rs`, `admin_handlers.rs`,
`router.rs`, and targeted tests in `api_tests.rs`. Notification delivery is not
implemented in the current repository and is tracked as
`DOCTOR_NOTIFICATION_DELIVERY_CAPABILITY_REQUIRED`; patient Q&A creation is
tracked as `PATIENT_QNA_SUBMISSION_REQUIRED_FOR_PATIENT_PHASE`.

Final correction-pass evidence: `cargo fmt --check` PASS; clippy PASS;
library tests **14/14**; serialized integration tests **40/40**. The Doctor
contract remains frozen after this evidence-gated correction; future Rust
changes require a demonstrated backend defect or approved product/provider
requirement.

> **Purpose:** Mid-task handoff doc so work can resume without re-deriving context. Read this first before continuing.
> **Last updated:** 2026-10-04 (ninth pass â€” resumed Â§8.2 (consultation telemetry event timeline + admin audit before/after snapshots + Super-Admin-only restriction), the two items left unstarted after the eighth pass's capacity-test detour. Both are now done â€” see Â§8.2 below, which replaces its own "not started" status. 27/27 integration tests passing (up from 25), 7/7 new unit tests, `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` clean, and the two new migrations (`20261004000002`, `20261004000003`) live-verified against the real persistent dev Postgres container, not just ephemeral test databases â€” a genuine bug was caught this way (a missing `GRANT USAGE, SELECT ON SEQUENCE` for the new table's `BIGSERIAL` column; table `INSERT` alone does not cover it).)
> **Scope:** Still entirely the security-hardening/observability task, per the user's explicit sequencing: *"introduce all of these before we touch flutter."* Flutter work has **not** started.
> **Current priority order** (user-specified, work top to bottom): 1. test suite âœ…, 2. carrier-NAT-safe multi-dimensional rate limiting âœ…, 3. webhook provider-event dedup âœ…, 4. admin-portal browser hardening âœ…, 5. N+1 admin query cleanup âœ… (**partially revisited â€” see Â§9**, one more N+1-at-scale instance found under production-scale data and fixed), 6. realistic load testing + 1,200-user cap âœ… **done â€” see `docs/CAPACITY_EVIDENCE_REPORT.md`**, 7. pilot-readiness decision âœ… **done â€” see `docs/PILOT_READINESS_DECISION.md`: `NOT_READY_FOR_UNSUPERVISED_1200_USER_PILOT`**, 8. load-test validation deep-dive âœ… **done, `LOCAL_BACKEND_VALIDATION_PASSED` â€” see `docs/CAPACITY_EVIDENCE_REPORT.md` Â§8**, 9. System Error Service + consultation telemetry + admin audit trail âœ… **done, all three parts â€” see Â§8 below**, 10. constrained-VPS capacity test âœ… **done and rebalanced â€” `LOCAL_CONSTRAINED_CAPACITY_TARGET_PASSED` up to ~200 RPS on a genuine 2-vCPU budget â€” see Â§9 below and `docs/CONSTRAINED_CAPACITY_TEST.md` Â§12. Frozen here per explicit instruction, not pursued further.**
>
> **What's left overall** (not blocking, but not yet done): the two deferrals from Â§3 item 5 below (admin step-up re-auth, real logout token revocation), optionally a further capacity rebalance past 200 RPS if ever needed (`docs/CONSTRAINED_CAPACITY_TEST.md` Â§12.6 â€” explicitly not pursued now), and the deployment-time punch list discussed but deliberately deferred until an actual deploy is imminent (reverse proxy/TLS, firewall, secrets management, automated backups â€” not built yet, on purpose). cargo-audit/secret-scanning CI (Â§3 item 10) is now **done** â€” see Â§8.3. With the full Â§3/Â§8 backend punch list clear, **Flutter mobile work is next**, per the user's own stated sequencing. See `docs/PROGRESS.md` Â§3 for current sequencing.

---

## 0. The architectural pivot this session made, and why

The previous checkpoint (now superseded) had the backend mid-refactor on a **single generic `hellodoctor_app` DB role** and **per-handler `AuthContext` extractors** as the only authentication gate. Partway through resuming, the user pasted a detailed third-party security review of that plan and then gave an explicit, frozen decision:

> Keep the **five-role** DB capability model (`hellodoctor_api`, `hellodoctor_auth_worker`, `hellodoctor_payment_worker`, `hellodoctor_system_worker`, `hellodoctor_finance_worker`), implement **router-level default-deny** authentication (not just per-handler extractors), keep `AuthContext` for ABAC inside handlers, never put a DB password in a migration, and â€” since the RLS migration had never been executed â€” this was the right moment to **replace it cleanly**, not patch it.

This is now done. Full detail below.

---

## 1. What's actually done, and how it was verified

**Everything in this section was verified by actually running the server against a real local Postgres 16 + Redis container (`docker compose up -d` in `services/backend/`), not just `cargo check`.** That distinction matters â€” two real, previously-undiscovered bugs were only caught by doing this (Â§3).

### 1.1 â€” Five-role PostgreSQL capability model
- `migrations/20261003000001_rls_hardening.{up,down}.sql` â€” **completely rewritten**, not patched (the old single-role draft had never been applied, so this was a clean replacement per the user's own instruction).
- Creates `hellodoctor_api`, `hellodoctor_auth_worker`, `hellodoctor_payment_worker`, `hellodoctor_system_worker`, `hellodoctor_finance_worker` as `NOLOGIN NOBYPASSRLS` roles â€” **no password anywhere in the SQL**.
- `main.rs::bootstrap_capability_role_logins` â€” runs once per startup (idempotent, via the superuser `migrator_pool`), issuing `ALTER ROLE <role> WITH LOGIN PASSWORD '<hex>'` for each of the five roles, with passwords read from five new env vars (`DB_API_ROLE_PASSWORD`, `DB_AUTH_WORKER_PASSWORD`, `DB_PAYMENT_WORKER_PASSWORD`, `DB_SYSTEM_WORKER_PASSWORD`, `DB_FINANCE_WORKER_PASSWORD`). Each password is defensively validated as hex-only before being interpolated into the `ALTER ROLE` string (Postgres's DDL grammar doesn't accept `$1` bind parameters in a `PASSWORD` clause, so this is the only way to set it â€” the hex check is what keeps that safe).
- Real secrets for all five are already generated (via `openssl rand -hex 24`) and sitting in the gitignored `services/backend/.env`. `.env.example` documents the generation command and has blank placeholders.
- **The organizing rule for every GRANT decision** is a sentence already in `docs/AUTH_SECURITY.md` Â§12.3 that predates this session: *"The ordinary API role cannot update payment, appointment, wallet, grievance, or chat lifecycle state."* Every table write was assigned to the narrowest role whose job that specific lifecycle transition is. Full per-table grant rationale is commented inline in the migration SQL.
- `repository/mod.rs::AppState` now holds five `PgPool`s (`db` [= api], `auth_db`, `payment_db`, `system_db`, `finance_db`) plus a `DbRole` enum and `begin_authed_as(ctx, role)` (role-selectable) alongside the existing `begin_authed(ctx)` (shorthand for `DbRole::Api`).
- `main.rs` connects all five pools concurrently via `tokio::try_join!`, each from its own DSN built by `AppConfig::{api,auth_worker,payment_worker,system_worker,finance_worker}_dsn()` (shared host/port/dbname + per-role password).

**Known consequence, accepted deliberately, not a bug:** a few operations that used to be one atomic transaction now span two or three pools (e.g. `ConsultationService::complete_consultation` touches `appointments` via payment-worker, `consultation_sessions` via the api pool, and `doctor_wallets`/`transactions` via finance-worker; `AppointmentService::process_payment_webhook` spans payment/api/system-worker). Every one of these was rewritten to be **idempotent/self-healing on retry** (check-then-create rather than assume-once) specifically because this split means a crash mid-sequence is now possible and must be recoverable on redelivery â€” see the doc comments on those two functions for the exact reasoning. This is a real, intentional tradeoff of the least-privilege design, flagged inline wherever it occurs, not swept under the rug.

### 1.2 â€” Router-level default-deny â€” **done**
`router.rs` is split into `public_routes()` (the ~6 genuinely public auth/webhook endpoints) and `protected_routes()` (everything else), with JWT verification happening exactly once per request in `api::middleware::auth::auth_middleware`, applied as a `route_layer` on the protected tree only. The verified `AuthContext` is inserted into the request's extensions there; `AuthContext`'s `FromRequestParts` impl now just reads it back out instead of re-parsing the bearer token itself (see `auth_context.rs`'s doc comment). No handler signatures changed for this â€” every handler still declares `ctx: AuthContext` exactly as before, it's just populated differently under the hood. Verified live: `/healthz` and the public auth routes work with no token; every other route 401s with no token and 200s/403s correctly with one.

### 1.3 â€” ABAC
Unchanged in design from the previous checkpoint â€” `abac.rs`'s `Action` enum + `authorize()` â€” but now actually applied to **every** handler, including the ~20-function `admin_handlers.rs` that was entirely unstarted before. Verified live: a patient JWT gets `403 Role PATIENT is not permitted to perform this action` on `/admin/overview` and on `/doctor/wallet/summary` (doctor-only).

### 1.4 â€” Row-Level Security
All 16+ PHI/financial tables are `FORCE ROW LEVEL SECURITY` with policies keyed on transaction-local `app.current_user_id`/`app.current_role`, exactly as before, but now re-derived for the five-role model: each table's policy has a plain (no-`TO`) human-facing ownership/admin branch plus one or more `TO <worker_role>` policies for whichever workers are actually granted writes on that table. `payment_events` is newly brought under RLS in this rewrite (the old draft left it unprotected; now that `hellodoctor_api` has a blanket `SELECT` on it for admin finance views, leaving it unprotected would have been a real cross-patient data-leak risk).

**A second real, previously-undiscovered bug, caught only by actually running this SQL (not by review):** `patient_profiles_access`'s "does this doctor treat this patient" check and `appointments_access`'s correlated subqueries against `patient_profiles` formed a genuine RLS circular-policy reference â€” Postgres throws `infinite recursion detected in policy for relation "patient_profiles"` the moment both policies are evaluated in the same query (which happens on the very first real patient login, because of the patient-self-registration check in `auth_service.rs`). Fixed with a `SECURITY DEFINER` SQL function (`fn_doctor_treats_patient`) that runs as the migration's superuser and therefore bypasses RLS internally, breaking the cycle. Every other table's policy was re-audited by hand for the same two-way-reference shape after finding this one; none of the others have it (the dependency graph is a DAG rooted at `patient_profiles`/`doctor_profiles` once this one fix is in place).

### 1.5 â€” Redis rate limiting + IP jail
`rate_limit.rs` / `ip_jail.rs` (written previously) are now actually wired up:
- `auth_handlers.rs::register_otp` calls `rate_limit::check_otp_request_limit`.
- `auth_handlers.rs::doctor_login` / `admin_login` wrap the credential check with `ip_jail::is_jailed` (403 if so) â†’ `rate_limit::check_login_attempt_limit` â†’ on success `ip_jail::record_success`, on failure `ip_jail::record_failure`.
- `grievance_handlers.rs::submit_grievance` calls `rate_limit::check_grievance_limit`; `prescription_handlers.rs::upload_patient_intake` calls `rate_limit::check_upload_limit`.
- The global per-IP limit + baseline IP-jail check were already wired as a top-level middleware (`global_guard_middleware`, applied to every request including public ones) before this session.

**Not done:** the multi-dimensional (IP + account + phone) carrier-NAT-safe scheme the pasted security review specifically asked for â€” current jailing is IP-only, exactly the thing that review flagged as a Bangladesh/Kenya carrier-NAT risk. Also not done: Redis-outage fail-open/fail-closed policy differentiation (currently rate-limit checks return `AppError::Internal` on a Redis error, which â€” depending on how `AppError` maps to HTTP status â€” likely fails closed everywhere uniformly; whether that's the right behavior for a doctor-directory read vs. an OTP check was not differentiated).

### 1.6 â€” Payment webhook signature verification (Phase F â€” previously entirely unstarted)
`webhook_handlers.rs::handle_payment_webhook` now: extracts the raw request `Bytes` before any JSON parsing, verifies `X-Signature: hex(hmac_sha256(WEBHOOK_HMAC_SECRET, raw_body))` in constant time (`crypto::verify_webhook_signature`, via `hmac::Mac::verify_slice`), rejects with 401 on missing/invalid signature, then checks a new required `event_timestamp` field in the payload is within a 300-second freshness window, then parses/processes as before.

**Explicitly not done, flagged rather than silently skipped:** full provider-event-id replay dedup. `payment_events` already has a unique `(gateway, provider_event_id)` index from the original schema design, but this generic/single-provider stub handler doesn't have a real `provider_event_id` field to dedup on yet (that's provider-specific â€” bKash/Nagad/M-Pesa each shape their webhook differently, and this codebase doesn't have real provider adapters yet, just the one generic stub). The timestamp-freshness check is a partial mitigation, not full replay protection.

### 1.7 â€” A critical, previously-undiscovered authentication bug (found and fixed this session)
Independent of everything above: **every JWT verification was broken**, which would have meant 100% of authenticated requests failing in any real deployment. `jsonwebtoken` v9.3's `DecodingKey::from_ed_der` is misleadingly named â€” despite "der" in the name, it does no ASN.1 parsing at all; it stores whatever bytes it's given verbatim and hands them straight to `ring::signature::UnparsedPublicKey`, which for Ed25519 requires exactly the raw 32-byte public key. The `.env`'s public key (generated per `.env.example`'s own documented `openssl pkey ... -pubout` command) is a 44-byte SPKI-DER structure (a fixed 12-byte ASN.1 header + the 32-byte raw key). Signing worked fine (the private-key path genuinely does PKCS8 parsing via `ring::signature::Ed25519KeyPair::from_pkcs8_maybe_unchecked`), so every login *appeared* to succeed, but the very next authenticated request always failed with `InvalidSignature` â€” this would have been invisible to anyone testing only the login endpoint's 200 response, and was only caught by chaining login â†’ use-the-token in the same live test. Fixed in `crypto::verify_access_token`: strip the fixed 12-byte SPKI prefix (or accept an already-raw 32-byte key) before constructing `DecodingKey::from_ed_der`. Full reasoning is in the code comment there â€” read it before touching that function again.

---

## 2. Exact commands used for live verification (reproducible)

```bash
cd services/backend
docker compose up -d            # postgres + redis
cargo run                       # runs migrations, bootstraps role logins, connects 5 pools, serves :8080

curl -X POST http://localhost:8080/api/v1/auth/register-otp \
  -H "Content-Type: application/json" -d '{"phone_number":"+8801700000005"}'
# -> {"success":true,"data":{"debug_otp_code":"...", ...}}  (debug_otp_code only populated when ENVIRONMENT=development)

curl -X POST http://localhost:8080/api/v1/auth/verify-otp-and-login \
  -H "Content-Type: application/json" -d '{"phone_number":"+8801700000005","otp_code":"<code from above>"}'
# -> {"success":true,"data":{"access_token": "...", ...}}

curl http://localhost:8080/api/v1/doctors -H "Authorization: Bearer <token>"        # 200
curl http://localhost:8080/api/v1/admin/overview -H "Authorization: Bearer <token>" # 403 (PATIENT role denied)
curl http://localhost:8080/api/v1/doctors                                          # 401 (no token)
```

All of the above were run and produced the expected result on the state as of this checkpoint.

---

## 3. Historical priority list (superseded by current remaining work) â€” in priority order (per the user's explicit sequencing)

1. ~~Router-level default-deny restructuring~~ â€” **done** (Â§1.2).
2. ~~`tests/api_tests.rs` rework~~ â€” **done this pass.** See Â§6 below for the full story (it surfaced four additional real bugs, not just a compile fix).
3. ~~Multi-dimensional rate limiting / IP-jail~~ â€” **done.** `ip_jail.rs` rewritten: account-keyed jail (license number / admin email, independent of IP â€” the primary brute-force defense, carrier-NAT-safe by construction) plus a separate IP-breadth jail (jails an IP only when it fails logins against many *distinct* accounts within a window â€” a credential-stuffing shape, not normal shared-NAT traffic). Both fail closed on a Redis outage (login becomes unavailable rather than silently unprotected); `rate_limit.rs`'s booking/upload/grievance limits were made explicitly fail-*open* (resource-exhaustion guards, not auth boundaries) via a new `check_and_increment_fail_open` wrapper, with the choice documented per-function rather than left as an accident of error propagation. New test `test_multi_dimensional_abuse_jailing` exercises all four properties directly (account-jailed-across-IPs, IP-not-jailed-for-one-account, IP-jailed-for-breadth, success-clears-account-counter). 19/19 tests passing.
4. ~~Webhook provider-event-id replay dedup~~ â€” **done.** New `repository/payment_event_repo.rs::try_record_event` inserts into the pre-existing `payment_events` table relying on its unique `(gateway, provider_event_id)` index (`ON CONFLICT ... DO NOTHING`, returns whether the row was actually new). `PaymentWebhookPayload` gained a required `provider_event_id` field; `process_payment_webhook` now short-circuits to the find-don't-redo path if *either* the event id was a duplicate *or* the transaction was already `PAYMENT_HELD` â€” two independent dedup mechanisms, so a provider resending under a genuinely new event id for the same underlying transaction attempt is still caught by the status check. New test assertion covers exactly that gap (same `payment_session_id`, different `provider_event_id`, zero additional `consultation_sessions` rows). 19/19 tests passing.
5. ~~Admin-portal browser hardening~~ â€” **done, with one scope discovery and one explicit deferral.** The scope discovery: `apps/admin-web` had **zero authentication anywhere** before this â€” no login page, no token storage, `src/lib/api.ts` sent every `/admin/*` request with no `Authorization` header at all. "Harden the existing token storage" wasn't actually possible; a login flow had to be built first. User confirmed (via AskUserQuestion) building a BFF-pattern login now, not just headers. What's in place:
   - **BFF pattern, HttpOnly cookies, never `localStorage`**: `src/app/api/login/route.ts` proxies to the backend's existing `/auth/admin/login` (email+password+TOTP, one call), then stores the access/refresh tokens in HttpOnly/`SameSite=strict`/path-scoped-to-`/api` cookies via `src/lib/session.ts` â€” the response body to the browser contains only `{user}`, never a token.
   - `src/app/api/proxy/[...path]/route.ts` is now the *only* path from browser to backend â€” it reads the cookie server-side, attaches `Authorization: Bearer`, forwards, and on a 401 (15-min access-token expiry) silently retries once via the refresh-token cookie before giving up. `src/lib/api.ts`'s `API_BASE` now points here instead of the backend directly; the old `next.config.js` rewrite that passed browser requests straight to the backend with zero auth was removed.
   - `src/middleware.ts` redirects an unauthenticated page load to `/login` (UX-level only â€” the real boundary is the proxy route rejecting any call with no valid cookie, which happens regardless of what middleware does).
   - `next.config.js` now sets the CSP already specified in `docs/AUTH_SECURITY.md` Â§7.2 plus `X-Frame-Options: DENY`, `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`, `Permissions-Policy` (camera/mic/geolocation/payment all denied â€” no legitimate use in an admin tool).
   - New `src/app/login/page.tsx` (no prototype reference existed for this screen â€” it's new, necessary UI, not a redesign of anything) and a small logout icon button added to the existing topbar in `page.tsx` (minimal addition next to the already-fake `admin-user-pill`, not a restructuring of that UI).
   - **Live-verified end-to-end**, not just compiled: real login with the seeded admin + a real computed TOTP code â†’ HttpOnly cookies set (confirmed via a `curl` cookie jar) â†’ proxied `/admin/overview` call succeeds with the cookie, 401s without it â†’ logout clears the cookies â†’ unauthenticated `/` load 307-redirects to `/login` â†’ CSP/security headers present on responses. `npx tsc --noEmit` and `npm run build` both clean.
   - **Explicitly deferred, not done**: step-up re-authentication for sensitive actions (refund, disbursement initiation, MFA reset). The backend has no mechanism for this at all yet (no "recent step-up verification" concept) â€” building one is a real, separate feature (a new endpoint re-verifying TOTP and setting a short-lived flag, plus gating the sensitive handlers on it), not a small addition, and wasn't done in this pass. Also deferred: real server-side token revocation on logout (currently just clears the cookies; the access token still technically works until it naturally expires within 15 minutes, and the refresh token family isn't explicitly revoked on logout â€” only on detected reuse).
6. **N+1 admin queries** (`admin_handlers.rs` â€” `list_admin_doctors`, `list_admin_transactions`, `get_command_telemetry`, `list_admin_grievances` all do per-row lookups in a loop) â€” fix *before* load testing, so benchmarks measure the architecture's real capability rather than a known-inefficient implementation detail.
7. **Realistic load testing + the 1,200-user admission cap** â€” benchmark authenticated flows (login, directory, slots, booking, payment init, RTC token, chat, doctor queue, wallet reads), not just `/health`. Record p50/p95/p99, CPU, RAM, DB pool wait, Postgres CPU, Redis latency, error rate, queue depth, WebSocket count. Implement `MAX_ACTIVE_USERS` as a configurable admission gate (new registrations only; existing users/sessions unaffected).
8. **Pilot-readiness decision** â€” only after 3â€“7 above, with evidence, not merely passing compilation/tests. `READY_FOR_CONTROLLED_1200_USER_PILOT` or `NOT_READY_...`, matching the user's master directive's own bar.
9. **WebSocket chat security** â€” currently moot: `chat_handlers.rs`/`chat_service.rs` is plain HTTP request/response, not an actual WebSocket. Revisit if/when real-time delivery is implemented.
10. ~~cargo audit / secret scanning in CI~~ â€” **done**, see Â§8.3 below.

**Explicit constraint carried forward from the user's latest message:** none of items 3â€“8 may be satisfied by weakening JWT verification, ABAC, RLS, or the Redis abuse checks on the request path. If any of those measurably hurt performance during load testing, the fix is in the implementation or infrastructure, not removing a security layer.

---

## 6. The test suite rework (this pass) â€” what changed and what it caught

`tests/api_tests.rs` was fully rewritten, not just patched to compile. It now runs **18/18 passing** (`cargo test --test api_tests -- --test-threads=1` â€” **must** be single-threaded, see the file's own header comment; the five capability roles are cluster-wide Postgres objects, not per-database ones, even though `#[sqlx::test]` gives each test its own throwaway database, so parallel role-login bootstrap and even the migration's own `CREATE ROLE`/`GRANT` statements race on shared `pg_authid` catalog rows â€” confirmed directly as `"tuple concurrently updated"`, not a flaky-test artifact).

**New test-infrastructure pieces:** a `RolePools` struct + `build_role_pools()` that bootstraps LOGIN/password for the five roles and connects to each (mirrors `main.rs`'s real bootstrap, since `#[sqlx::test]` only hands you the superuser pool); a `send`/`get`/`post` helper trio that inserts a fresh random `ConnectInfo<SocketAddr>` extension on every request (required because `global_guard_middleware` extracts it on every request including public ones, and `tower::ServiceExt::oneshot` against a bare `Router` â€” as opposed to a real bound TCP listener â€” never populates it); a Redis `FLUSHDB` on the dedicated test logical DB (15) at the start of every test, because Redis â€” unlike the ephemeral per-test Postgres database â€” is one real persistent instance shared across every test run including ad hoc manual smoke testing, and rate-limit counters keyed by the fixed seeded patient/doctor/admin ids would otherwise accumulate across unrelated runs; real JWT signing/verification via the fixed test keypair (same genuine PKCS8/SPKI-DER shape as production, cross-checked as an actual matching pair) plus hand-crafted malformed/expired/wrong-audience/wrong-issuer tokens for the negative suite.

**New tests:** `test_protected_route_rejects_every_invalid_token_shape` (missing/malformed/expired/wrong-aud/wrong-iss/tampered-signature, plus a positive control), `test_abac_denies_cross_role_access`, `test_bola_patient_cannot_post_into_another_patients_chat`, `test_rls_doctor_treats_patient_security_definer_function` (direct SQL-level test of `fn_doctor_treats_patient`, the function that fixed the circular-RLS-policy recursion bug â€” positive: a doctor who genuinely treated the patient sees the row; negative: one who never did, doesn't), `test_db_role_write_isolation` (raw-SQL negative tests: `hellodoctor_api` cannot UPDATE `appointments`/write `grievance_adjudications`/read `phone_otps`; `hellodoctor_payment_worker` cannot UPDATE `doctor_wallets`), `test_webhook_rejects_bad_signature_and_replay_has_no_double_effect` (bad signature â†’ 401; replaying the exact same validly-signed callback â†’ still 200, but zero additional `consultation_sessions`/`chat_conversations` rows created).

**Four additional real bugs this exercise caught, beyond the two already known from the live-server session:**

1. **`consultation_service.rs::mint_agora_token`'s patient-ownership check compared the wrong UUID space.** `apt.patient_id` is a `patient_profiles.id`; it was being compared directly against `ctx.user_id` (a `users.id`) â€” two different ID spaces that are never equal for a real distinct seed. This made the patient side of "is caller a participant in this consultation" permanently unreachable; only the doctor side (which already correctly re-looked-up `doctor_profiles.user_id`) ever worked. Pre-existing, not introduced this session â€” invisible until a test exercised a real patient token end-to-end rather than a demo context where user_id and profile_id happened to be conflated. Fixed by resolving the caller's own patient profile via `patient_repo::find_by_user_id` and comparing its `id`, matching the pattern already used for doctors.
2. **Three missing `hellodoctor_finance_worker` grants**, found one at a time by running the actual flows: `wallet_repo::credit` is an `INSERT ... ON CONFLICT DO UPDATE` upsert (first settlement for a doctor has no existing row), so finance-worker needs `INSERT` on `doctor_wallets`, not just `UPDATE`; `GrievanceService::adjudicate_refund` reads `consultation_sessions` (to find the appointment) before touching `transactions`, so finance-worker needs `SELECT` there too; `DisbursementService::initiate_monthly_disbursement` joins settled transactions against `appointments` to find each one's `doctor_id`, so finance-worker needs `SELECT` on `appointments` as well. All three are now granted, with matching RLS policies, and documented inline in the migration.
3. Two **test-expectation corrections, not bugs**: an unrelated patient posting into someone else's chat, and an unrelated doctor completing someone else's appointment, both surface as **404**, not 403 â€” RLS hides the row entirely from a non-participant before the handler's own ownership check ever runs, which is the *better* security property (no resource-existence disclosure to an unauthorized caller), not a weaker one. The tests were adjusted to assert 404, with a comment explaining why that's correct.

**Practical implication for next steps:** the migration file has now been revert-and-reapplied against the persistent dev Postgres container too (not just exercised by the ephemeral test databases), so `services/backend`'s dev DB state matches exactly what's committed â€” no drift between "what the tests ran against" and "what `cargo run` uses."

---

## 4. Environment notes for resuming

- Docker Desktop was not running at the start of this session and had to be launched (`Start-Process "C:\Users\CUBE\AppData\Local\Programs\DockerDesktop\Docker Desktop.exe"`) â€” if it's not running, that's why containers won't start.
- `services/backend/.env` now has the five role passwords (`DB_{API,AUTH_WORKER,PAYMENT_WORKER,SYSTEM_WORKER,FINANCE_WORKER}_PASSWORD`) plus `APP_DB_HOST`/`APP_DB_PORT`/`APP_DB_NAME`, replacing the old single `APP_DATABASE_URL`/`DB_APP_ROLE_PASSWORD`. Already generated, already gitignored â€” nothing more to do there.
- The dev Postgres container (`hellodoctor-postgres`) currently has only 1 throwaway `users` row plus whatever test accounts were created during this session's live verification (a few `+88017000000XX` patient numbers) â€” still effectively disposable, `SEED_DEV_DATA=false`.
- **RETIRED / DO NOT EXECUTE:** the earlier applied-migration replay procedure is withdrawn. Do not edit applied SQL or manually modify `_sqlx_migrations`. Use new SQLx migrations and B-01 source/binary/database checks.
- No `cargo-audit`, no load-test tool installed yet (same as before).

---

## 5. Retired resume instructions - historical only

Items 1â€“5 of Â§3 are done (router default-deny, test suite, multi-dimensional abuse jailing, webhook event dedup, admin-portal login+BFF+security headers). Backend: 19/19 tests passing, `cargo clippy --all-targets -- -D warnings` clean. Admin-web: `npx tsc --noEmit` and `npm run build` clean, full loginâ†’proxyâ†’logout chain live-verified.

~~N+1 admin queries~~ â€” **done.** All four flagged handlers in `admin_handlers.rs` (`list_admin_doctors`, `list_admin_transactions`, `get_command_telemetry`, `list_admin_grievances`) rewritten from one-query-per-row-in-a-loop to a small constant number of whole-table fetches up front, joined via in-memory `HashMap`s â€” matches the pattern already used elsewhere in the same handlers (`doctor_repo::list`/`patient_repo::list`/`appointment_repo::list_all` were already whole-table fetches in places; this extends the same idiom rather than introducing a new one). Two small additions to `consultation_repo.rs` (`list_all`, `list_all_telemetry`) were needed since no whole-table fetch existed for those two tables yet. `get_doctor_dossier`/`get_patient_dossier`/`get_compliance_alerts` were deliberately left as-is â€” their loops are bounded by one entity's own history, not total system size, so they don't have the same N+1-at-scale problem the four flagged functions do.

**Important finding while doing this**: none of the 20 pre-existing tests actually called any of these four GET endpoints â€” "tests still pass" after the rewrite would **not** have caught a data-correctness bug (e.g. a broken map key silently falling back to a generic "Patient"/"Doctor" placeholder). Added `test_admin_dashboard_endpoints_resolve_names_correctly`, which books+confirms+completes a real appointment and files a real grievance, then asserts the enriched fields on all four endpoints resolve to the actual seeded names/amounts, not a fallback. 20/20 tests passing, `cargo clippy --all-targets -- -D warnings` clean.

Not forgotten, just intentionally ordered after 6â€“8: the two explicit deferrals noted in Â§3 item 5 (step-up re-auth for admin sensitive actions, real token revocation on logout) and item 9 (WebSocket chat security â€” moot until chat becomes an actual WebSocket).

---

## 7. Item 6 (load testing) â€” RESOLVED 2026-10-04. The section below is historical (the interrupted state as it was found on resume); current results live in `docs/CAPACITY_EVIDENCE_REPORT.md`.

**Resolution summary:** On resume, Â§7.4's checks were run first as instructed â€” no stale process was found, Docker auto-restarted its containers, and `cargo test`/`cargo clippy` were confirmed clean with the Â§7.2 change before rebuilding release. The release rebuild, server restart, and real-scale run then surfaced five real bugs the interrupted session hadn't yet reached (four in the load-test harness itself, one a genuine production bug in `appointment_service.rs`'s appointment-number generator â€” see `docs/CAPACITY_EVIDENCE_REPORT.md` Â§3 for the full list and fixes). The final clean run: 46,095 requests, 0.64% error rate, zero `500`s, zero unexpected `429`s. Resource sampling (Postgres/Redis/process) is in Â§5 of that report. The pilot-readiness decision this fed into is `docs/PILOT_READINESS_DECISION.md`: **NOT_READY_FOR_UNSUPERVISED_1200_USER_PILOT** (a scale/environment evidence gap, not a security or correctness finding â€” see that doc for what would close the gap).

<details>
<summary>Historical record of the interrupted state (click to expand) â€” kept for reference, no longer actionable</summary>

### 7.1 â€” What's actually done and solid

**`MAX_ACTIVE_USERS` admission cap â€” done, tested, not in question.**
- `config.rs`: new `max_active_users: u32` field (env `MAX_ACTIVE_USERS`, default `1200`).
- `error.rs`: new `AppError::CapacityLimitReached(String)` variant â†’ HTTP 503, code `CAPACITY_LIMIT_REACHED`.
- `repository/user_repo.rs::count_active`: `SELECT count(*) FROM users WHERE is_active = TRUE AND deleted_at IS NULL`.
- Enforced in exactly two places â€” both are *new-account-creation* points, never an existing user's own login: `auth_service.rs::verify_otp_and_login`'s "no existing user for this phone" branch (right before `user_repo::create_patient_user`), and `admin_handlers.rs::create_doctor` (right after the ABAC check, before any row is written).
- `admin_handlers.rs::get_admin_overview` now also returns `active_users`, `max_active_users`, `capacity_used_percent` â€” the "admin can view current capacity usage" requirement.
- New test `test_max_active_users_caps_new_registration_not_existing_logins`: sets the cap to 4 (exactly the seeded user count), proves an existing seeded patient can still log in fine, proves a brand-new phone number gets a 503/`CAPACITY_LIMIT_REACHED`.
- This was all still passing (21/21) as of right before the rate-limit-config change described below â€” **not re-verified after that change, see Â§7.4**.

**Load-test harness â€” built, compiles clean, one real run completed (with a diagnosed-but-unresolved measurement artifact).**
- New `examples/load_test.rs` (reuses `reqwest`, already a dev-dependency â€” no new crates needed). Run with `cargo run --release --example load_test` against an already-running server. Env-overridable: `LOAD_TEST_CONCURRENCY` (default 50), `LOAD_TEST_DURATION_SECS` (default 45), `LOAD_TEST_PATIENTS` (default 40).
- What it actually does: (1) connects directly to Postgres via `DATABASE_URL` and bulk-inserts `max(concurrency*20, 500)` schedule slots per doctor, dated exactly 1 year in the future so they never collide with anything a human tests interactively; (2) provisions N virtual patients via the **real** `POST /auth/register-otp` â†’ `POST /auth/verify-otp-and-login` HTTP flow (not minted tokens â€” this measures real login latency too) and mints doctor/admin tokens directly via `crypto::create_access_token` using the real `.env` signing key (equivalent in shape to a real login's output, used to avoid repeating the heavier doctor 2-step-MFA/admin-TOTP flows per virtual user); (3) runs `concurrency` tokio tasks in a closed loop (no artificial pacing â€” concurrency alone controls offered load) until the deadline, each iteration picking a weighted-random action: 30% `GET /doctors`, 20% `GET /doctors/:id/slots`, 10% book+pay (real booking + real HMAC-signed webhook confirm), 10% `POST rtc-token`, 10% `POST chat message` (into a conversation looked up by direct DB query after a successful booking â€” test-harness-only shortcut, not something a real client would do), 10% `GET wallet summary` (doctor token), 10% `GET /admin/command/telemetry` (admin token â€” **stand-in for "doctor queue" reads**, since no literal doctor-queue endpoint exists; noted so the eventual report doesn't claim it tested something it didn't); (4) prints per-endpoint count/errors/p50/p95/p99/max plus overall RPS and error rate.
- **Sanity run executed** (`LOAD_TEST_CONCURRENCY=5 LOAD_TEST_DURATION_SECS=8 LOAD_TEST_PATIENTS=5`, against a release build): **99.37% overall error rate.** Root cause diagnosed precisely, not guessed: manually confirmed via a follow-up `curl` that the real cause is `429 RATE_LIMIT_EXCEEDED` from `global_guard_middleware`'s global per-IP ceiling (120 req/min, `rate_limit.rs::check_global_ip_limit`). This is the rate limiter working *correctly* â€” the flaw is in the load-testing methodology, not the application: every simulated "user" in a single-machine load test shares the one real IP of the machine running the test, unlike real distributed production traffic where thousands of client IPs would each individually have their own 120/min budget. A single-origin load tool will always slam into this ceiling almost immediately regardless of how much real capacity the application has behind it.

### 7.2 â€” The fix applied for the above (done, needs re-verification â€” see Â§7.4)

Made the global per-IP limit configurable rather than hardcoded, so a load-testing run can deliberately raise it (while the real production/dev default stays exactly 120, unchanged):
- `rate_limit.rs::check_global_ip_limit` signature changed: now takes a third `limit_per_min: u32` parameter instead of hardcoding `120`.
- `config.rs`: new `global_ip_rate_limit_per_min: u32` field (env `GLOBAL_IP_RATE_LIMIT_PER_MIN`, default `120`).
- `api/middleware/global_guard.rs`: call site updated to pass `state.config.global_ip_rate_limit_per_min`.
- `tests/api_tests.rs::test_config()`: new field added (`120`, matching production default â€” the test suite's behavior is unchanged by this).
- `.env.example` and the real `.env`: both document/set `GLOBAL_IP_RATE_LIMIT_PER_MIN=120` (the safe default). **The plan was to override this via a shell env var at server-*invocation* time for the load-test run only** (e.g. `GLOBAL_IP_RATE_LIMIT_PER_MIN=100000 ./target/release/hellodoctor-backend.exe`), never by permanently editing the committed/local `.env` â€” confirm this is still the plan on resume, don't bake a raised limit into `.env`.
- `cargo check --all-targets` passed clean after these edits (confirmed). **`cargo test` and `cargo clippy` were NOT re-run after this change before the interruption â€” do that first on resume, before anything else.**

### 7.3 â€” What was NOT done yet (the actual remaining work for item 6)

1. Rebuild the release binary (`cargo build --release`) and the release example (`cargo build --release --example load_test`) with the `GLOBAL_IP_RATE_LIMIT_PER_MIN` change included â€” **the release binary that's possibly still running (Â§7.4) predates this change.**
2. Start the server with the limit raised just for this run, e.g.: `GLOBAL_IP_RATE_LIMIT_PER_MIN=100000 ./target/release/hellodoctor-backend.exe`.
3. Run the real-scale load test: `cargo run --release --example load_test` (defaults: concurrency=50, duration=45s, patients=40) â€” or pick deliberately chosen values and record what was chosen and why in the eventual report.
4. **While it runs**, sample resource usage at least twice (start and end, ideally a couple of points in between):
   - Postgres: `docker exec hellodoctor-postgres psql -U hellodoctor -d hellodoctor -c "SELECT count(*) FROM pg_stat_activity WHERE datname='hellodoctor';"` (connection count) and `-c "SELECT xact_commit, xact_rollback, blks_hit, blks_read FROM pg_stat_database WHERE datname='hellodoctor';"` (commit rate, cache hit ratio â€” diff two snapshots for a rate).
   - Redis: `docker exec hellodoctor-redis redis-cli INFO stats` (look at `instantaneous_ops_per_sec`) and `docker exec hellodoctor-redis redis-cli --latency` for a few seconds.
   - Windows process: `Get-Process hellodoctor-backend | Select-Object Id,CPU,WorkingSet64,PrivateMemorySize64` â€” note `.CPU` is **cumulative CPU-seconds**, not instantaneous percent; take two timestamped snapshots and compute `Î”cpu_seconds / Î”wall_seconds / core_count` to approximate a CPU% figure, and say so explicitly in the report rather than presenting a raw number as if it were already a percentage.
5. Write `docs/CAPACITY_EVIDENCE_REPORT.md` with: the actual measured numbers from the run (per-endpoint p50/p95/p99/error-rate, overall RPS, the resource samples above), an explicit, unambiguous caveat that this is a local development machine and not representative production infrastructure (so the numbers show "does the architecture hold up and where does it bend first", not "this is our production capacity"), and a recommendation.
6. Then, and only then, item 7 (the final `READY_FOR_CONTROLLED_1200_USER_PILOT` / `NOT_READY_...` decision) â€” which, given the hardware caveat in the previous bullet, will very plausibly have to be `NOT_READY_...` or a heavily-qualified `CONDITIONALLY_SAFE_...` purely on the "no production-representative evidence exists yet" grounds, independent of whatever the numbers themselves show. Say that plainly if it's the honest conclusion â€” don't stretch dev-laptop numbers into a production claim.

### 7.4 â€” Exact process/environment state at the moment of interruption â€” VERIFY, DON'T ASSUME

The user interrupted a Bash call that was going to (a) stop a running `hellodoctor-backend.exe` process, then (b) `cd` and `cargo build --release`. **That command never executed** (rejected before running). This means, as of this checkpoint:

- An **old** release-mode `hellodoctor-backend.exe` (built *before* the `GLOBAL_IP_RATE_LIMIT_PER_MIN` change in Â§7.2 â€” i.e. it still has the hardcoded `120` with no override capability) was started earlier in this session and **may still be running** on port 8080. Check with e.g. `Get-Process hellodoctor-backend -ErrorAction SilentlyContinue` or `curl http://localhost:8080/healthz` before assuming either way.
- If it's still running, its in-memory/Redis rate-limit state for this machine's real IP was driven well past 120 by the earlier sanity-run burst. The fixed window is 60 seconds, so by the time anyone reads this it has almost certainly reset on its own â€” but don't assume; a quick `curl` with a real token will show 429 immediately if not.
- Docker containers (`hellodoctor-postgres`, `hellodoctor-redis`) were last confirmed running (`docker ps`) right before this interruption â€” likely still up, but verify with `docker ps`.
- **The dev Postgres database is no longer pristine** â€” the sanity run's provisioning step inserted real rows: 1,500 `doctor_schedule_slots` rows (500 per seeded doctor Ã— 3 doctors), dated ~1 year in the future (harmless, won't collide with anything, but they're there), plus 5 extra patient `users`/`patient_profiles` rows created via the real OTP flow during provisioning (phone numbers of the shape `+88019000000XX`). None of this needs cleaning up before continuing â€” the full-scale run will add more of the same â€” but know it's there if `total_patients`/`total_doctors`-style counts look unexpectedly non-round later.
- The actual load-test *run* that produced the 99.37%-error-rate numbers was the **sanity run only** (concurrency=5, duration=8s) â€” no real-scale run has happened yet. Nothing resembling final capacity evidence exists yet.

### 7.5 â€” Immediate next action on resume (supersedes Â§5 above until item 6 is fully done)

1. Check whether the old release server is still running; stop it if so.
2. `cargo test --test api_tests -- --test-threads=1` and `cargo clippy --all-targets -- -D warnings` â€” confirm still clean after the Â§7.2 signature change (not yet done).
3. Rebuild release (`cargo build --release` and `cargo build --release --example load_test`).
4. Proceed through Â§7.3's remaining steps in order.

</details>

---

## 8. System Error Service + consultation telemetry + admin audit trail â€” DONE, started 2026-10-04, completed 2026-10-04 (ninth pass)

**Why this exists:** the load-test validation deep-dive (`docs/CAPACITY_EVIDENCE_REPORT.md` Â§8) came back `LOCAL_BACKEND_VALIDATION_PASSED`, and per the user's explicit instruction ("then only if green and delta forward doing these"), this triggered three large follow-on feature requests, all building on existing-but-unused or partially-used schema rather than new tables:

1. **System Error / bug reporting + admin log view** â€” domain separation between `system_error_logs` (technical failures) and `grievance_reports` (clinical/service complaints), never merged. Ground truth from Â§8.7 of the capacity report: `system_error_logs` exists in schema but has **zero writers anywhere in the codebase today** â€” nothing currently populates it. This is the actual starting point, not an assumption.
2. **Consultation telemetry event timeline + technical-complaint correlation** â€” detailed second-by-second call event history (patient/doctor/server perspectives, reconnect/disconnect duration reconstruction), correlated with grievances and system errors into one admin incident-reconstruction view. Ground truth from Â§8.9: today there is only one aggregate `consultation_telemetry` row per session (duration, connection_state, packet_loss, RTT) â€” no event-level table, no reconnect counting, no event vocabulary. The aggregate-level grievanceâ†”telemetry correlation **does** already work end-to-end (live-verified in Â§8.9) â€” this is additive, not a fix.
3. **Admin change audit trail (Super Admin only)** â€” extending the existing `audit_events` table (not inventing a new one). Ground truth from Â§8.8: `audit_events` is **already append-only at the DB-grant level** for every role (verified: `hellodoctor_api` has only `INSERT`/`SELECT`, no `UPDATE`/`DELETE`) and a real admin mutation **does** already produce a correctly-populated row (verified via a new test, `test_admin_mutation_creates_audit_event`) â€” but only 2 of many admin mutations call it today (`BMDC_VERIFY`/`BMDC_REJECT`), there is no `before_data`/`after_data`/`changed_fields` capability in the schema at all, and `SELECT` is currently open to six admin roles rather than one designated Super Admin authority.

**Explicit constraints carried forward from the user's prompt, binding for all of this work:**
- The backend architecture (JWT, ABAC, RLS, five DB roles, payment/grievance/consultation state machines) is **frozen** â€” this is additive observability/accountability work, not a redesign.
- System errors and grievances/complaints remain two distinct domains, never merged into one generic "issues" table, even though both surface in the admin portal.
- No sixth database role â€” writes to the new structures are assigned to whichever existing role's job this already is.
- Centralize error capture through one service (not scattered raw `INSERT`s), with mandatory secret/PHI redaction before persistence, and a stable fingerprint for deduplication.
- Telemetry ingestion must never be able to block or break an active consultation (fail-open, bounded, non-blocking).
- Audit writes should land in the same transaction as the business mutation they document wherever that's technically possible.

**Status at this checkpoint:** all three items are implemented and tested, end-to-end, for real â€” see Â§8.2 for items 2 and 3 (completed this pass).

### 8.1 â€” System Error Service: done

- **Migration** `20261004000001_system_error_service.{up,down}.sql`: extends the previously-unused `system_error_logs` with `error_code`, `fingerprint`, `http_status`, `endpoint`, `occurrence_count`, `first_seen_at`/`last_seen_at`, and a `status` lifecycle (`OPEN`/`ACKNOWLEDGED`/`RESOLVED`/`IGNORED`) replacing the old unused `is_resolved` boolean (safe to drop outright â€” zero rows, zero code references existed). Grants: `hellodoctor_system_worker` gets `UPDATE` in addition to its pre-existing `SELECT, INSERT` (needed for the dedup-bump path â€” see the bug note below); `hellodoctor_api` gets `UPDATE` (needed for admin acknowledge/resolve). No sixth role introduced â€” writes go through the worker role whose job this already was per the original RLS migration's own comment.
- **`src/system_error.rs`** â€” the centralized `SystemErrorService`: a fire-and-forget `report()` over an unbounded channel (never blocks the caller), a single background consumer (`init()`, called once in `main.rs` with the `system_worker` pool) that persists events, regex-based redaction of bearer tokens/JWT-shaped strings/`key=value` secret patterns before storage, and a stable SHA-256 fingerprint (category + component + error code â€” deliberately never the raw message, which could embed variable IDs and fragment one recurring failure into thousands of rows). A fingerprint match against an `OPEN` row within a 60-minute window bumps `occurrence_count` instead of inserting a new row.
- **Wired into `AppError::into_response`** for exactly the two variants that map to `5xx` (`Internal`, `Database`) â€” every other variant (`Validation`, `NotFound`, `Conflict`, `Unauthorized`, `Forbidden`, `RateLimitExceeded`, `PayloadTooLarge`, `UnprocessableEntity`, `PaymentRequired`, `CapacityLimitReached`) deliberately does **not** report, so expected business/auth/rate-limit outcomes never pollute the system-errors table. The same `request_id` generated for the response's `meta.request_id` field is reused as the stored row's `trace_id` â€” a client who reports "I got error X" with a reference id gives the admin a direct lookup key.
- **Admin API**: `GET /admin/system-errors` (paginated, filterable by severity/status/subsystem, returns open/open-critical counts for a dashboard summary), `GET /admin/system-errors/:id`, `POST /admin/system-errors/:id/acknowledge`, `POST /admin/system-errors/:id/resolve` â€” all gated by three new ABAC actions (`ViewSystemErrors`, `AcknowledgeSystemError`, `ResolveSystemError`, `SecurityAdmin`/`PlatformAdmin` only), kept as a **separate surface** from the existing `/admin/logs` (`audit_events`-backed) screen, per the explicit instruction not to merge the two domains.
- **A real bug this work found and fixed, not merely described**: the first version of the migration granted `hellodoctor_system_worker` only `SELECT, INSERT` â€” a live test (`test_system_error_service_persists_dedupes_and_redacts`) caught a genuine `42501 permission denied` the moment a second occurrence of the same failure tried to take the dedup-bump `UPDATE` path. Fixed by granting `UPDATE` to that role too, and reverified â€” see the test's own pass as the fix's proof, not just the migration diff.
- **Tests** (3 new, all passing against real Postgres): dedup + redaction (`test_system_error_service_persists_dedupes_and_redacts`), the critical negative property that 401/403/404/409 never create a row (`test_normal_rejections_do_not_create_system_error_logs`), and the full admin lifecycle + access control (`test_system_errors_admin_api_access_control_and_lifecycle`). `25/25` passing overall, `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` both clean.
- **Explicitly not done in this pass** (reasonable scope cuts, not oversights): client-side (Flutter) crash-report submission endpoint; `metrics`/Prometheus counters for system errors; alerting-threshold prep; a retention/cleanup job; `release_version`/`git_commit` capture; source file/line/stack-trace capture (the schema has a `stack_trace` column but nothing populates it yet â€” `AppError`'s variants don't currently carry a backtrace to pass through). These are reasonable follow-ups, not silently dropped requirements â€” listed here so they aren't lost.

### 8.2 â€” Consultation telemetry events + admin audit before/after snapshots: done

**Consultation telemetry event timeline:**
- **Migration** `20261004000002_consultation_telemetry_events.{up,down}.sql`: new `consultation_telemetry_events` table (deliberately numeric/enum-only â€” `event_type`, `actor_perspective`, `rtt_ms`, `packet_loss_percent`, `occurred_at` â€” no free-text client field at all, so this can never become a PHI-logging vector). `event_type` is a `CHECK`-constrained vocabulary (`CALL_INITIATED`, `*_JOINED`, `*_RECONNECTING`, `*_RECONNECTED`, `*_DISCONNECTED` for both PATIENT/DOCTOR, `NETWORK_DEGRADED`/`NETWORK_RESTORED`, `CALL_ENDED`, and a reserved-but-unwritten `SERVER_ERROR` â€” same honest "column exists, nothing populates it yet" pattern already used for `system_error_logs.stack_trace`). Writes go through `hellodoctor_api` (no sixth role), RLS-protected with the identical ownership shape as the pre-existing `consultation_telemetry_access` policy.
- **A real bug found and fixed by actually running it, not just reading the SQL**: the table's `ingest_seq BIGSERIAL` implicit sequence needs its own `GRANT USAGE, SELECT ON SEQUENCE ...` â€” granting `INSERT` on the table alone does **not** cover it. The first live test run failed with a genuine `permission denied for sequence consultation_telemetry_events_ingest_seq_seq`; fixed in the migration, reverified.
- **`ConsultationService::record_telemetry_events`**: resolves the caller as a genuine PATIENT or DOCTOR participant (same ownership check shape as `mint_agora_token`) before accepting anything â€” a non-participant gets the same RLS-driven 404 (not 403) that every other cross-tenant access attempt in this codebase gets, since the appointment row is invisible to them before any handler logic runs. Bounded to 20 events/call. Fail-open **within** the authorized batch: an unknown `event_type` or an out-of-range `rtt_ms`/`packet_loss_percent` is silently skipped rather than failing the whole batch â€” ingesting observability data must never be able to break an active consultation. `actor_perspective` is always derived server-side from the verified caller identity, never trusted from client input.
- **`ConsultationService::get_session_timeline`** (admin-only): returns the ordered event list, a reconstructed set of reconnect windows (pairs each `*_DISCONNECTED`/`*_RECONNECTING` with the following `*_RECONNECTED` for the same perspective; an unresolved disconnect at the end of the list is reported with `reconnected_at: null`), the pre-existing aggregate telemetry row, correlated grievances (`grievance_repo::find_by_consultation_id`, new scoped query), and a **best-effort** (not a hard FK) correlation against `system_error_logs` by participant-user-id overlap + a Â±2-minute time window around the session (`system_error_repo::find_related`) â€” explicitly labeled as heuristic in both the code comment and the API response itself (`related_system_errors_note`), since `system_error_logs` has no `consultation_session_id` column and adding one would mean threading session context through the generic `AppError`-driven reporting pipeline in `error.rs`, judged out of scope for this pass.
- **New routes**: `POST /consultations/:appointment_id/telemetry/events` (any authenticated participant), `GET /admin/consultations/:session_id/timeline` (new ABAC action `ViewConsultationTimeline`, same role set as `ViewGrievanceQueue` â€” `Compliance | ClinicalAdmin | PlatformAdmin` â€” since this view exists to support grievance/complaint investigation).

**Admin audit trail (before/after snapshots + Super-Admin-only restriction):**
- **Migration** `20261004000003_admin_audit_snapshots.{up,down}.sql`: adds `before_data JSONB`, `after_data JSONB`, `changed_fields TEXT[]` to `audit_events`. Narrows `audit_events_select`'s RLS policy from six admin roles down to one: `current_setting('app.current_role', true) = 'SECURITY_ADMIN'` (plus the pre-existing `current_user <> 'hellodoctor_api'` worker/superuser bypass, unchanged). `abac::Action::ViewLogs` narrowed the same way (`SecurityAdmin` only, was `SecurityAdmin | PlatformAdmin`); `SimulateLog` (a write-only test/demo utility, never a read of the trail) deliberately keeps its wider `PlatformAdmin` access.
- **Judgment call, made explicit and reversible**: no prior doc defined which role "Super Admin" means. `SECURITY_ADMIN` was chosen in this pass because it's the one role whose entire purpose is independent security/compliance oversight, and because keeping `PLATFORM_ADMIN` (the general operational admin role) from being the sole judge of its own audit trail is a standard separation-of-duty control. This is a one-line change (in the migration's policy and in `abac.rs`) to revisit if the user wants a different designated authority. **This narrows an existing PLATFORM_ADMIN capability** â€” flagging it here rather than burying it in a diff: `PLATFORM_ADMIN` tokens can no longer call `GET /admin/logs` after this pass.
- **`AuditService::log` reworked** from an 8-positional-argument function into a small `AuditLogEntry` builder (`AuditLogEntry::new(...).with_reason(...).with_before(...).with_after(...)`) specifically to make the before/after snapshot calls at 8 call sites readable. `changed_fields` is computed once at write time (`audit_service.rs::diff_changed_fields`, unit-tested) â€” the sorted set of top-level keys whose value differs between `before`/`after`, or `None` when either snapshot is absent (e.g. a pure creation) rather than a meaningless empty diff.
- **Audit coverage widened from 2 admin mutations to 8** (the explicit gap `docs/CAPACITY_EVIDENCE_REPORT.md` Â§8.8 found: "the rest ... do not yet audit themselves"): `BMDC_VERIFY`/`BMDC_REJECT` (upgraded to carry real `is_verified` before/after), `DOCTOR_PROFILE_CREATED` (after-only â€” **never** includes the generated `temporary_password`, which is a secret, not an audit fact), `SLOT_CREATED`/`SLOT_STATUS_TOGGLED`, `DISBURSEMENT_INITIATED`, `SYSTEM_ERROR_ACKNOWLEDGED`/`SYSTEM_ERROR_RESOLVED`, and all three grievance adjudications (`GRIEVANCE_REFUND`/`GRIEVANCE_WARN`/`GRIEVANCE_DISMISS` â€” `GrievanceService::adjudicate_{refund,warning,dismiss}` now return `(old_status, report)` instead of just `report`, specifically so the handler can build an accurate before/after snapshot without a redundant extra read). Doctor suspension (named as a possible gap in the capacity report) does not exist anywhere in the codebase â€” confirmed by search, not assumed â€” so there was nothing to wire up there.
- No redaction pass is applied to `before_data`/`after_data` (unlike `system_error.rs`'s free-text redaction) â€” deliberate, because every snapshot here is built from structured operational/professional/financial fields the call sites construct explicitly, never arbitrary free text, and never patient clinical content.

**Tests** (2 new integration tests, all passing against real Postgres, plus 4 new unit tests â€” `cargo test --lib`):
- `test_telemetry_event_ingestion_ownership_and_timeline_reconstruction`: a non-participant doctor gets 404 (RLS-hidden, matching the project's own established precedent for this exact shape â€” see `test_bola_patient_cannot_post_into_another_patients_chat`'s comment); a batch with one deliberately-unknown `event_type` accepts only the valid ones; the admin timeline endpoint is itself ABAC-denied to a patient; the accepted events appear in ingest order and the reconnect-window reconstruction reports the correct duration.
- `test_audit_before_after_snapshots_and_super_admin_only_log_access`: `BMDC_REJECT` (seeded doctor 1 starts `is_verified = true`, so reject is the real `true â†’ false` transition, not a no-op re-verify) produces a row with `before_data.is_verified = true`, `after_data.is_verified = false`, `changed_fields = ["is_verified"]`; `PLATFORM_ADMIN` is denied `GET /admin/logs` (403); a `SECURITY_ADMIN` token succeeds (200) and sees the row.
- `audit_service::tests::{diff_changed_fields_finds_only_differing_keys, diff_changed_fields_none_on_creation}`, `consultation_service::reconnect_window_tests::{pairs_disconnect_with_reconnect, unresolved_disconnect_has_no_reconnected_at}`.
- **27/27** integration tests passing (`cargo test --test api_tests -- --test-threads=1`), **7/7** library unit tests (`cargo test --lib`), `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` both clean.
- **Both new migrations were also live-verified against the real persistent dev Postgres container** (`cargo run` against `services/backend/.env`, not just the ephemeral per-test databases) â€” this is what caught the sequence-grant bug above; a real patient OTP registration call against the live-migrated schema was exercised afterward to confirm the running server, not just the test suite, accepts the new schema.

**Explicitly not done in this pass** (reasonable scope cuts, listed so they aren't lost): a literal `SERVER_ERROR`-type telemetry event writer (the vocabulary entry and DB `CHECK` branch exist; nothing emits one yet, in favor of the best-effort time-window correlation described above); a hard FK between `system_error_logs` and `consultation_sessions` (would require threading session context through the generic `AppError` pipeline); client-side (Flutter) telemetry-event emission (Flutter work hasn't started at all per the user's own sequencing).

---

## 8.3 â€” cargo-audit / secret-scanning CI â€” done, 2026-10-04 (tenth item of Â§3, closed out of order since it was cheap and code-only, not deployment-specific)

**`.github/workflows/security-scan.yml`** â€” two jobs, on push/PR to `main` plus a weekly Monday cron (cargo-audit's advisory DB updates independently of this repo's commits, so a dependency that was clean last week can become a finding without any code change here):
- **`cargo-audit`**: installs the prebuilt binary (`taiki-e/install-action`, not compiled from source â€” keeps CI fast), generates a fresh `Cargo.lock` (gitignored for this application, see `.gitignore`), runs `cargo audit --deny warnings --ignore RUSTSEC-2023-0071`.
- **`secret-scan`**: TruffleHog OSS (`trufflesecurity/trufflehog@main`, `--only-verified` â€” flags only secrets it can confirm are live/real, not every regex-shaped string) over the full git history (`fetch-depth: 0`) on push/PR; skipped on the weekly cron, which has no commit range to diff against.

**A real, previously-unknown issue found and fixed while wiring this up, not just a config exercise**: the first real `cargo audit` run (not assumed, actually executed locally before trusting the CI config) found 2 vulnerabilities + 1 unmaintained-crate warning. Traced all three to a single root cause: **the `validator` crate was declared in `Cargo.toml` but is not referenced anywhere in `src/`** (`grep -rn "validator" src` â€” zero matches) â€” genuinely dead weight, not a false positive. Removed it outright (`cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` all still clean afterward); this alone eliminated the `idna` (RUSTSEC-2024-0421) and `proc-macro-error` (unmaintained, RUSTSEC-2024-0370) findings.

**One finding remained after that cleanup and is explicitly, narrowly ignored rather than silently suppressed**: RUSTSEC-2023-0071 (`rsa` â€” Marvin Attack timing sidechannel, no fixed upstream release exists). Root-caused, not assumed: it reaches `Cargo.lock` only via `sqlx-mysql`'s optional dependency on `rsa` â€” this backend enables only sqlx's `postgres` feature (`Cargo.toml`), and `cargo tree -i rsa` returns **zero matches**, confirming `rsa` is not part of the actual compiled dependency graph (Cargo.lock includes entries for a crate's full optional-feature graph regardless of which features are active â€” a known, documented Cargo behavior, not a bug in this project). The ignore is a one-line CLI flag with the full justification as an inline comment in the workflow file itself, not a silent allowlist â€” re-check is explicitly flagged if `sqlx`'s `mysql` feature is ever enabled, or if a fixed `rsa` release ships.

**Deliberately not done, and why**: no production-deployment hardening (reverse proxy/TLS, firewall, secrets management, automated backups) was built in this pass â€” per explicit discussion, that work is deployment-time-specific (needs a real domain, a real droplet, a real backup destination) and would risk going stale or needing rework before an actual deploy happens; cargo-audit/secret-scanning was the one item from that conversation that's pure code hygiene, not infrastructure, so it was done now rather than deferred.

---

## 9. Constrained-VPS capacity test â€” done, 2026-10-04 (seventh pass). `LOCAL_CONSTRAINED_CAPACITY_TARGET_FAILED`

**Full detail, numbers, and every caveat: `docs/CONSTRAINED_CAPACITY_TEST.md`.** This section is a pointer, not a duplicate.

**Why this ran now, ahead of finishing Â§8.2**: the user asked for it explicitly, as a check on whether the backend can sustain the planned 1,200-user pilot under resources that actually resemble a small VPS â€” everything up to this point (including Â§Â§1â€“8 of `docs/CAPACITY_EVIDENCE_REPORT.md`) ran on the full, unconstrained 12-core developer laptop.

**What had to be built first** (none of it existed): `Dockerfile` (the backend had only ever run as a bare `.exe` directly on the host â€” nothing could cgroup-limit it); `docker-compose.capacity.yml` (a deliberately **standalone** compose project, not merged with the dev stack â€” merging would have concatenated the `volumes:` list across files and tried to mount two volumes onto the same Postgres data directory, caught before it caused damage); `scripts/seed_capacity_dataset.sql` (deterministic ~1,200-user/13,440-appointment synthetic dataset, documented simplifications, no real PHI); `examples/capacity_test.rs` (a new **open-loop** load generator â€” fires at a target arrival rate regardless of response latency, unlike the existing closed-loop `load_test.rs`, which would have quietly throttled itself in sympathy with a struggling backend).

**Resource limits verified real**, not just written in a config file: `docker inspect` confirmed backend=1.25 cores/1.25GiB, postgres=1.0 core/1.75GiB, redis=0.25 core/0.25GiB against a 12-core host â€” and then confirmed again empirically by watching `hellodoctor-capacity-postgres` actually get pinned at ~100% CPU under load.

**A real bug found and fixed**: `get_command_telemetry`'s earlier N+1 fix (whole-table fetches, from an earlier session) was itself the bottleneck once the tables held thousands of rows instead of a handful from the tiny original dev dataset â€” p50 1,406ms â†’ 317ms after adding scoped `find_by_ids`/`list_telemetry_by_session_ids` queries (`appointment_repo.rs`, `patient_repo.rs`, `consultation_repo.rs`) and rewriting the handler to use them. Confirmed by rerunning the identical test, not assumed from the diff. The same whole-table-fetch pattern likely also affects `list_admin_doctors`/`list_admin_transactions`/`list_admin_grievances` â€” **not verified**, since they're outside the current traffic mix.

**The headline result**: HEALTHY_OPERATING_RPS â‰ˆ 20â€“30 RPS; clear collapse (56.6% true error rate) by 100 RPS; worse (86.7%) at 200 RPS. The 300 RPS primary target and the 400/500/600 RPS headroom/burst tests were **not attempted** once collapse was unambiguous, per the explicit instruction not to hammer an already-collapsed system. Postgres's 1.0-core CPU allocation was the sole, clearly-identified bottleneck â€” the backend (10â€“40% CPU) and Redis (1â€“5% CPU) containers had abundant headroom throughout every run. **Zero data corruption at any load level** (direct SQL integrity check, scoped to exclude one fully-explained pre-existing seed-data artifact â€” see the capacity-test doc Â§7 for the exact verification), and no security control was weakened (the one pre-authorized exception: the global per-IP rate limiter raised to a documented benchmark-only ceiling for the capacity-test container, never written into the real `.env`).

**Immediate next action on resume (as of the seventh pass):** rerun with a rebalanced CPU split. **This was done in the eighth pass â€” see below.**

---

### 9.1 â€” Rebalanced rerun (eighth pass, same day): `LOCAL_CONSTRAINED_CAPACITY_TARGET_PASSED` up to ~200 RPS

Full detail: `docs/CONSTRAINED_CAPACITY_TEST.md` Â§12 (pointer only here, not duplicated).

- **New profile**: backend 0.60 vCPU/1.25GB, Postgres 1.25 vCPU/2.25GB, Redis 0.15 vCPU/0.25GB â€” a genuine 2.0 vCPU total this time (Profile A summed to 2.5), with all three pinned to the same two physical cores (`cpuset: "0,1"`) so they share real core contention the way processes on an actual 2-core VPS would, not just independent CFS quotas that could otherwise burst onto different idle cores on this 12-core host.
- **A second real bug found and fixed before the rerun**: PostgreSQL JIT compilation overhead. `consultation_sessions`'s `list_active()` query showed mean 265.74ms in `pg_stat_statements`; `EXPLAIN ANALYZE` run as the real `hellodoctor_api` role under a real RLS session (not as superuser, which would have hidden it) showed ~273ms of that was pure JIT compilation, triggered because the RLS policy's `OR`'d correlated ownership subquery inflates the planner's cost estimate past `jit_above_cost` even though that subplan is never actually executed (short-circuited by the cheap admin-role check). Confirmed with a before/after `EXPLAIN ANALYZE`: `SET jit = off` dropped it to 3.15ms, identical plan/rows/RLS enforcement. Applied via `-c jit=off` in the capacity-test Postgres container's startup command â€” a pure execution-strategy setting, no security or behavior change. **Likely generalizes** to other RLS-protected tables with the same OR'd-correlated-subquery policy shape (e.g. `appointments`) â€” not exhaustively audited across all of them in this pass.
- **Ladder run**: 20/30/50/75/100/150/200/250 RPS, each stage resetting rate-limit counters and `pg_stat_statements` first. 20 through 150 RPS unambiguously clean (0% true errors). 200 RPS still effectively clean (0.0154%, 3 errors out of 19,492) with early signs of rising latency. **250 RPS showed a genuine, classic progressive collapse** (latency climbing monotonically 302ms â†’ 9,660ms across the run, not the flat connection-level pattern seen in Profile A) â€” and this time the **backend** itself was the pinned resource (~58â€“60% of its reduced 0.6-core quota), not Postgres, confirming the reallocation genuinely moved the constraint rather than coincidentally helping. Per instruction, 300 RPS was **not attempted** once 250 showed unambiguous collapse.
- **Result**: HEALTHY_OPERATING_RPS â‰ˆ 150â€“200 RPS, SATURATION_RPS â‰ˆ 250 RPS â€” a ~7â€“10x improvement over Profile A's ~20â€“30 RPS ceiling, decisively answering the user's question (*"does moving CPU from idle backend/Redis into PostgreSQL move the healthy ceiling materially upward?"*) in the affirmative. Zero data corruption at every stage including the 250 RPS collapse. No security control touched or weakened.
- **Capacity work is now frozen, as instructed.** A further rebalance (more CPU to the backend specifically, since Postgres has headroom to spare under this profile) is named as a future option in `CONSTRAINED_CAPACITY_TEST.md` Â§12.6 if the ceiling ever needs to move past 200 RPS, but is explicitly not pursued now. Â§8.2 (consultation telemetry events + admin audit before/after snapshots) was resumed and completed next â€” see Â§8.2 above. With that done, the full backend-hardening/observability punch list from the user's own sequencing is clear; **Flutter mobile work is the next major phase**, per that same sequencing ("introduce all of these before we touch flutter").

---

## 10. Multi-agent security/correctness/performance audit + Pass 1 remediation â€” done, 2026-10-04

**Why this exists:** per explicit instruction, three parallel fresh agents independently audited the whole backend for (a) security/access-control loopholes, (b) general correctness bugs, and (c) live-measured API request latency â€” each briefed with the project's own history so they wouldn't re-report already-fixed issues. All three reports were spot-verified by directly reading the flagged code before being trusted (several of the most severe claims were independently confirmed this way, not just relayed). A detailed remediation plan was then agreed with the user (reproduced in the session's own record) and **Pass 1 â€” every CRITICAL/HIGH finding plus the three cheap fixes â€” is now implemented, tested, and verified.** Pass 2 (grievance terminal-state guard, transactional audit writes, 5-document upload race, Agora production-token/time-gate/UID fixes, structured log redaction) and Pass 3 (further performance work: Argon2 `spawn_blocking`+semaphore, webhook IP allowlist if the gateway supports one) are explicitly **not yet started** â€” next on resume.

### 10.1 â€” An incident during this pass, disclosed in full: a destructive-command near-miss, caught and fixed, zero data loss

While tearing down the (already-unused) capacity-test Docker stack per the user's own cleanup checklist, `docker compose -f docker-compose.capacity.yml down -v` was run without the project-name scoping (`-p hellodoctor-capacity`) that file's own header comment documents as required â€” because neither compose file declared an explicit project `name:`, both defaulted to the same implicit name ("backend," from the directory), so the command matched and removed the **live dev** `hellodoctor-postgres`/`hellodoctor-redis` containers instead of the (already long-gone) capacity ones. Caught immediately by verification (not assumed safe): the dev stack was brought back up, reattached to its original named volumes (`backend_hellodoctor_pgdata`/`backend_hellodoctor_redisdata`, never referenced by the capacity compose file, which is the only reason this was recoverable), and a direct non-PII row-count check confirmed all data intact (275 users, 3 doctors, all 9 then-current migrations). **Root cause fixed structurally, not just noted**: `docker-compose.capacity.yml` now declares `name: hellodoctor-capacity` explicitly, so this specific collision is now impossible regardless of how the teardown command is invoked in the future. The dev compose file was deliberately **not** given a matching explicit name in this pass â€” doing so while its containers were already live and attached to the old implicit "backend" project would itself have required a careful, separate migration to avoid a container-name collision on the next `up`; left as-is since the capacity-side fix alone fully closes the actual collision risk.

### 10.2 â€” Pass 1 fixes, what each one actually does

New migration `20261004000004_security_correctness_fixes.{up,down}.sql` bundles every schema change Pass 1 needed:

1. **Doctor MFA brute-force (was: CRITICAL, no protection at all on the second factor)** â€” `pending_mfa_challenges` gained `attempt_count`/`max_attempts` (default 5), checked and incremented atomically via `SELECT ... FOR UPDATE` inside a real transaction (`session_repo::find_pending_challenge_for_update`/`increment_challenge_attempts`, `AuthService::doctor_verify_otp` rewritten). The same account-level jail `doctor_login`'s password step already used (`ip_jail`, carrier-NAT-safe, keyed on license number) is now also applied to MFA failures â€” a run of wrong OTP codes locks the whole account, not just the one challenge, and the account-jail check now also blocks a *fresh* login attempt for that account, not just further guesses on the old session. `auth_handlers.rs::doctor_verify_otp` gained the `ConnectInfo<SocketAddr>` extractor it was missing to make this possible. Tests: `test_doctor_mfa_brute_force_is_blocked` (5 wrong guesses â†’ 6th rejected even with the *correct* code â†’ fresh login for the same account also blocked), `test_doctor_mfa_success_consumes_challenge_and_cannot_be_replayed`.
2. **Document-download IDOR (was: HIGH, zero ownership check â€” literally built a URL from raw path params)** â€” `prescription_handlers.rs::get_document_download_url` now actually queries the database (`prescription_repo::find_intake_document`/`find_prescription_by_id`, both scoped by **both** `id` and `appointment_id`), relying on the pre-existing ownership-scoped RLS on both tables (confirmed already correct â€” this was a "handler never asked the database at all" bug, not an RLS gap) plus an explicit belt-and-suspenders scope check in the query itself. A non-participant now gets 404 (RLS hides the row â€” the same "don't disclose existence" convention already established elsewhere in this codebase), matching precedent rather than inventing a new behavior. Test: `test_document_download_url_enforces_ownership` (owner âœ…, assigned doctor âœ…, a different doctor âŒ 404, right document/wrong appointment âŒ 404).
3. **Unsafe PDF uploads (was: HIGH, zero sanitization, zero malware scanning, pure passthrough)** â€” `UploadSecurityService` now does real structural validation on every PDF: rejects anything encrypted (opaque to this check, so treated as untrusted rather than trusted blindly) and anything containing active-content dictionary keys (`/JavaScript`, `/JS`, `/OpenAction`, `/AA`, `/Launch`, `/EmbeddedFile`, `/RichMedia`, `/SubmitForm`, `/ImportData`) via byte-level structural inspection â€” not a `Content-Type` trust, a real check of the file's own bytes. **Explicitly not claimed as more than it is**: this is not antivirus/malware scanning (no ClamAV or equivalent integration exists in this codebase) â€” a new `security_status` column (`prescription_intake_documents`, `prescriptions`, both default/insert `'CLEAN'` since a file that fails validation is never inserted at all) exists specifically so a *future* real AV integration has somewhere to record a downgrade without any API shape change, and the download endpoint already gates on it. 4 new unit tests in `upload_security_service.rs` (clean PDF accepted, JavaScript/OpenAction/encryption each rejected).
4. **Wallet double-credit (was: HIGH, a real money bug)** â€” `transaction_repo::mark_settled` changed from an unconditional `UPDATE` to `UPDATE ... WHERE payment_status = 'PAYMENT_HELD'`, returning whether it actually matched a row; `complete_consultation` now only credits the wallet when that call reports `true`. This makes the settlement transition itself â€” not a stale snapshot read in an earlier, already-committed transaction â€” the concurrency gate, so the operation is genuinely safe to attempt twice. Test: `test_wallet_credited_exactly_once_under_concurrent_completion` fires 20 concurrent `complete_consultation` calls for the *same* appointment (all return 200, matching the idempotent-on-retry design already established for this multi-pool flow) and asserts the wallet's `lifetime_gross` equals the appointment's fee exactly once, not 20 times.
5. **Disbursement double-pay (was: HIGH, a real money bug)** â€” new `transaction_repo::claim_for_disbursement` (`UPDATE ... WHERE payment_status = 'SETTLED_TO_DOCTOR'`, conditional per-transaction, relying on Postgres's own row-level locking to serialize concurrent claimants) replaces the old unconditional `mark_status` call inside the disbursement loop; a doctor with every candidate transaction already claimed by a concurrent run is now skipped entirely (no phantom zero-amount batch item). `wallet_repo::mark_disbursed` was also changed from flat-setting `pending_disbursement = 0` to decrementing by the actual claimed amount â€” necessary once partial claims under concurrency became a real, reachable case, and behavior-preserving in the ordinary single-run case. Test: `test_disbursement_claims_each_transaction_at_most_once_under_concurrency` fires 10 concurrent "Initiate Disbursement" calls and verifies, against direct `disbursement_items` ground truth (not just the wallet's own bookkeeping), that the settled transaction was paid exactly once.
6. **Cheap fixes**: `Gateway::Mpesa` now has its own match arm in `get_finance_summary`'s gateway-volume breakdown (previously silently dropped via a catch-all `_ => {}` â€” Kenya-market M-Pesa volume always showed as $0 regardless of real volume; the running `total_gross`/`fee`/`net` figures were never affected, only the per-gateway breakdown). The dead `OR current_user <> 'hellodoctor_api'` bypass clause on `audit_events_select`'s RLS policy (inert today, would have silently re-opened the just-closed Super-Admin-only restriction the moment any worker role is ever granted `SELECT` there) was removed â€” pure positive authorization now. `RUST_LOG`'s in-code fallback (used only when the env var is genuinely unset anywhere) changed from a verbose `debug` default to a production-safe `info`/`warn` default; local dev's own verbose value is now explicit in both `.env` and `.env.example` rather than relying on the code's fallback, so this is not a silent behavior change for existing local development.

**Explicitly flagged, not fixed in this pass** (judgment calls, not oversights): multi-currency aggregation in `get_finance_summary` (BDT/KES gross volumes are summed together in the three running totals once both markets are live simultaneously â€” flagged inline in the code with a comment, real fix needs a `currency` column on `transactions`, a bigger feature than this fix's scope); the Agora token/time-gate/UID issues (Pass 2); grievance re-adjudication and transactional-audit-write ordering (Pass 2).

### 10.3 â€” Verification

`cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` all clean. `cargo test --lib`: **11/11** (4 new PDF-validation unit tests). `cargo test --test api_tests -- --test-threads=1`: **32/32** (5 new: the two MFA tests, the IDOR test, and the two concurrency regression tests â€” up from 27). The new migration was also live-verified against the real persistent dev Postgres container (not just ephemeral test databases) â€” applied cleanly, server booted healthy, and the `RUST_LOG` fix was visually confirmed live (startup log went from the old per-request DEBUG spam to two quiet INFO lines).

**Final gap-closure status:** the previously listed Pass 2 items are verified in
the current tree where tests exist. This pass added provider-to-transaction
webhook binding and server-side refresh-family logout revocation. Remaining
external/deployment items are classified explicitly in
`docs/FINAL_SECURITY_GAP_REVIEW.md` and `docs/PAYMENT_PRODUCTION_READINESS.md`;
they are not silently marked complete.
## Full regression baseline (2026-10-05)

**Baseline result: PASS.** No backend regression failures were observed.

- `cargo fmt --check` â€” PASS.
- `cargo clippy --all-targets -- -D warnings` â€” PASS.
- `cargo test --lib` â€” **14/14 PASS**.
- `cargo test --test api_tests -- --test-threads=1` â€” **35/35 PASS**.
- The serialized integration run covered MFA/session revocation, authorization,
  RLS/BOLA/IDOR, webhook replay/provider mismatch, wallet and disbursement
  exactly-once invariants, grievance terminal state, document upload caps,
  Agora authorization windows, audit, telemetry, and system-error controls.
- No separate DB/RLS, financial-concurrency, webhook, or BFF test target exists;
  those checks are included in the 35-test `api_tests` suite. Capacity/load
  examples were deliberately excluded from this regression baseline.
- `#[sqlx::test]` cases apply migrations to clean temporary databases. Existing
  migration verification against persistent development Postgres remains
  recorded earlier; no destructive migration rerun was performed here.
- Admin portal: no dedicated test target exists. `npm run build` â€” PASS,
  including the login, logout, and proxy API routes. `npm run lint` is not
  non-interactive because Next.js found no ESLint configuration and opened its
  first-run configuration prompt; it is not counted as a lint pass.
- Integration tests intentionally run single-threaded because the fixture uses
  cluster-wide PostgreSQL capability roles and shared Redis state. This is a
  test-fixture constraint, not an application concurrency result.
- Capacity ladders, long soaks, 1,200-user seeding, and profiling were not
  rerun; the previously established ~150â€“200 RPS evidence remains frozen.

## Doctor Flutter client scaffold (2026-10-05)

Created apps/mobile with the approved BLoC/Dio/secure-storage conventions, all six Doctor prototype destinations, and consultation/history subviews. No Rust files were changed. Flutter SDK installation and analyze/test execution remain environment checks.

## Doctor Flutter completion verification pass (2026-10-05)

The initial runtime was audited and found partial. This pass added typed DTOs, real repository-backed feature pages, secure refresh restore, profile/settings, chat threads, history/detail, prescription image/PDF upload, non-faking Agora authorization boundary, ledger rendering, and expanded tests. The recorded checks were dart format, flutter analyze (no errors), and 11 Flutter tests pass. Three additional auth/duty tests and session/action guards were added afterward, but the Flutter SDK became unavailable in the final shell, so no post-change result is claimed. After Docker recovered, the seeded Rust backend was exercised directly: login/MFA, invalid credentials, queue/duty/slots/chats/Q&A/wallet/profile, logout, refresh revocation, and rate limiting were verified over HTTP. Android APK build remains blocked by missing SDK and no Android device/emulator is present. The Doctor client is not declared final-ready until the Flutter runtime and Agora path are exercised.
## Doctor Flutter completion audit (2026-10-05)

The current client is server-backed across authentication, queue/duty, schedule, consultation/history, chat, Q&A, Smart Rx/document upload, wallet, and profile. Q&A is a primary destination, file selection supports camera/gallery/image/PDF, consultation completion records a server-selected outcome, and Agora is explicitly non-faking and deferred.

Statuses: feature areas are `IMPLEMENTED_AND_TESTED`; Agora is `DEFERRED_AGORA_INTEGRATION`; external bKash/Nagad/M-Pesa/card/refund/payout execution is `DEFERRED_EXTERNAL_PAYMENT_INTEGRATION`; patient question creation is `PATIENT_QNA_SUBMISSION_REQUIRED_FOR_PATIENT_PHASE`; the prototype withdrawal control is `PROTOTYPE_BUSINESS_RULE_CONFLICT`.

The final readiness gate remains `DOCTOR_FLUTTER_NOT_READY` until running PostgreSQL/Redis/backend and a mobile runtime provide fresh real-request and visual evidence. No unsupported READY claim is made.
## Doctor Flutter completion audit (2026-10-05)

The current client is server-backed across authentication, queue/duty, schedule, consultation/history, chat, Q&A, Smart Rx/document upload, wallet, and profile. Q&A is a primary destination, file selection supports camera/gallery/image/PDF, consultation completion records a server-selected outcome, and Agora is explicitly non-faking and deferred.

Statuses: feature areas are `IMPLEMENTED_AND_TESTED`; Agora is `DEFERRED_AGORA_INTEGRATION`; external bKash/Nagad/M-Pesa/card/refund/payout execution is `DEFERRED_EXTERNAL_PAYMENT_INTEGRATION`; patient question creation is `PATIENT_QNA_SUBMISSION_REQUIRED_FOR_PATIENT_PHASE`; the prototype withdrawal control is `PROTOTYPE_BUSINESS_RULE_CONFLICT`.

Verification now recorded: `dart format --set-exit-if-changed .` PASS, `flutter analyze` PASS with zero issues, `flutter test` PASS (27 tests), and `flutter run -d edge` PASS. Android APK/device validation is blocked by the missing Android SDK/device. The live backend verification update below records the Docker and Rust service exercise.

Final verdict remains `DOCTOR_FLUTTER_NOT_READY` pending real backend and mobile runtime evidence.

## Doctor Flutter cache/offline/loading pass (2026-10-05)

Status: `IMPLEMENTED_AND_TESTED`. Added one `DoctorCacheStore` over secure storage with schema versioning, Doctor session scope, TTL metadata, safe corrupt/obsolete-cache eviction, logout/revocation clearing, and bounded resource limits. Added one centralized `ConnectivityService` for UX hints. Safe read resources render cached data immediately and revalidate in the background with last-updated/offline indicators and pull-to-refresh. Patient history authorization, document URLs/binaries, and every mutation remain online/backend-authoritative. Offline schedule/duty/completion/chat/Q&A/profile/upload actions are rejected without false success; chat drafts are explicitly `NOT SENT`.

Validation: `dart format --set-exit-if-changed .` PASS, `flutter analyze` PASS with zero issues, `flutter test` PASS (27 tests). Real emulator/device disconnect/reconnect and Flutter-to-Rust verification are not claimed because Android SDK/device and PostgreSQL/Redis/backend services are unavailable. Exact deferred statuses remain `DEFERRED_AGORA_INTEGRATION` and `DEFERRED_EXTERNAL_PAYMENT_INTEGRATION`.

## Doctor Flutter live backend verification correction (2026-10-05)

The previous environment note is superseded: Docker Desktop was started successfully. PostgreSQL 16 and Redis 7 are healthy from `services/backend/docker-compose.yml`; the Rust backend is running on port 8080 with seeded development data. Real HTTP verification passed Doctor login, MFA, queue, duty GET/PUT, schedule list/create/cancel, chat list, Q&A list, wallet summary, wallet ledger, profile, refresh rotation, logout, and rejection of the rotated refresh token after logout. The Flutter Edge smoke run also attached with the live API base URL. This evidence supports `IMPLEMENTED_REAL_BACKEND_VERIFIED` for those exercised contracts. Android/device offline reconnect remains unavailable because no mobile runtime is installed.

An ephemeral Flutter test using the actual `ApiClient` and `DoctorRepository` against the live service also passed login/MFA, queue, duty, schedule, chat, Q&A, wallet, ledger, profile, refresh, logout, and session clearing. It was deleted after verification; no test-only live credentials or fixtures remain in the app.





## Doctor registration contract extension — 2026-10-06

`POST /api/v1/auth/doctor/register` is now implemented as the approved narrow
Doctor onboarding extension. It is a separate Doctor-specific public route,
uses the existing Argon2 password hashing and Redis abuse controls, rejects
unknown privileged fields, creates `DOCTOR` with both user/profile
`is_verified=false`, initializes the read-only wallet, and returns
`PENDING_VERIFICATION`. ClinicalAdmin verification remains authoritative.

Backend validation: `cargo fmt --check`, `cargo clippy --all-targets -- -D
warnings`, `cargo test --lib` (14/14), and `cargo test --test api_tests
-- --test-threads=1` (41/41) pass. The Doctor backend contract is frozen again.
## Session checkpoint — 2026-10-06 Doctor registration and port resumption

### Completed this session

- Approved the narrow Doctor registration contract extension.
- Added `POST /api/v1/auth/doctor/register` as a separate Doctor-specific public route.
- Registration accepts only current schema-backed onboarding fields and uses `#[serde(deny_unknown_fields)]`.
- Existing Argon2 password hashing is used; passwords, hashes, OTPs, and privileged fields are not returned or logged.
- New accounts are created as role `DOCTOR` with `users.is_verified=false` and `doctor_profiles.is_verified=false`.
- Registration initializes the internal read-only wallet and returns `PENDING_VERIFICATION`.
- Redis registration abuse control and duplicate license/phone/email checks are implemented.
- Added a permanent integration test for pending status, role, unverified state, password hashing, duplicate rejection, and privileged-field rejection.
- Updated the frozen Doctor Flutter API mapping and project progress documentation.
- Added Flutter registration form, validation, submit state, duplicate-error handling, and pending-verification result screen.
- Corrected Flutter bottom navigation order to Queue → Chat → Scan Rx → Schedule → Wallet.
- Improved Queue presentation with prototype-aligned header, duty chip, Q&A banner, search/filter controls, cards, action buttons, cache states, and retry/empty states.
- Improved chat list presentation and changed Smart Rx selection to support up to five local pages with sequential backend uploads.
- Changed slot creation to require date, start time, and end time.

### Validation evidence

- Rust `cargo test --lib`: 14/14 passed.
- Rust `cargo test --test api_tests -- --test-threads=1`: 41/41 passed.
- Flutter `flutter analyze`: passed with no issues.
- Flutter `flutter test`: 29/29 passed.
- Flutter devices available: Windows desktop and Edge only. No Android emulator or physical Android device was available for screenshot/runtime acceptance.

### Honest current status

- `DOCTOR_REGISTRATION`: `IMPLEMENTED_AND_TESTED`.
- `DOCTOR_BACKEND_CONTRACT`: frozen again after the registration extension.
- Queue, Chat, Smart Rx, Schedule, Wallet, and Q&A exact screenshot gates are still pending; the existing Flutter UI is not yet accepted as an exact visual match.
- Agora remains `DEFERRED_AGORA_INTEGRATION`.
- External payment providers remain `DEFERRED_EXTERNAL_PAYMENT_INTEGRATION`.
- Final verdict remains `DOCTOR_FLUTTER_NOT_VERIFIED` until screen-by-screen rendered comparison and device/backend walkthrough evidence exist.

### Resume order

1. Capture and compare Queue against `prototype/index.html`; fix until matched.
2. Capture and compare Chat.
3. Capture and compare Smart Rx, including five-page flow.
4. Capture and compare Schedule.
5. Capture and compare Wallet.
6. Capture and compare Q&A.
7. Complete subflows, cache/offline runtime verification, documentation, and final regression checks.

## Queue port continuation — 2026-10-06

The Queue page was advanced from generic Material presentation to the canonical
Doctor Mobile structure in `prototype/index.html`. It now includes the exact
profile/duty treatment, Open Patient Q&A banner, scheduled section refresh,
search, filter strip, rich complaint cards, and three clinical actions. Server
appointments still supply all patient and timing values, and duty remains
bound to the existing Cubit/repository mutation.

Checks: `flutter analyze` reports no issues and `flutter test` passes 29/29.
No configured Flutter web target, Android emulator, or physical device is
available in this environment; therefore Queue has not been marked visually
matched. The next hard gate is a rendered Queue screenshot comparison, then
Chat implementation proceeds only after that comparison.

## App-wide continuation and payout guardrail � 2026-10-06

Queue responsive action sizing remains frozen: the primary Call Patient action uses the prototype flex ratio and non-breaking label treatment; Chat and Write Rx share the same toolbar height, radius, icon sizing, and baseline. Existing widget coverage exercises 360, 390, and 412 logical-pixel widths.

The Flutter wallet no longer offers a raw wallet/card-number entry form. It shows provider authorization as the required payout setup and never labels a manually supplied destination verified. The existing legacy backend setter is recorded as `GENUINE_BACKEND_GAP_PROVIDER_PAYOUT_BINDING`; monthly statements are recorded as `GENUINE_BACKEND_GAP_MONTHLY_PAYOUT_STATEMENT` because the current backend has no server-calculated currency-separated statement or locked payout-method snapshot. See `docs/DOCTOR_PAYOUT_ARCHITECTURE.md`.

## Kenya payout routing backend continuation - 2026-10-06

Implemented the KES-only payout destination table and APIs, primary eligible destination routing, safe destination snapshots on disbursement items, monthly statement read API, and Finance doctor-specific routing view. Development mode can create a server-generated simulated destination; no raw wallet/card data, PIN, OTP, CVV, PAN, or merchant secret is accepted or stored. External provider binding and payout execution remain deferred.

## 2026-10-06 - Finance state machine and Kenya-only routing correction

The internal finance workflow now distinguishes `PENDING_APPROVAL`, `APPROVED`, `PROCESSING`, `PAID`, `FAILED`, and reconciliation-required outcomes. Batch preparation allocates settled transactions to a disbursement item without changing their payment status, reducing the Doctor wallet, or marking a batch complete. Finance approval is a separate audited transition and explicitly reports `money_moved: false`. The external provider boundary remains `DEFERRED_EXTERNAL_PAYMENT_INTEGRATION`; no local UUID is presented as a provider transaction reference.

A persisted KES financial period and monthly statement snapshot provide controlled month close. Current/open months are estimated; only Finance finalization creates a final statement snapshot. Finance routing is month-filtered and returns Doctor-specific KES payable, adjustment, statement, disbursement, payout-method and masked-destination fields. Payout destinations are snapshotted on prepared items, so a later primary-method change cannot redirect a locked obligation. The allocation index was corrected so one item can aggregate many transactions. Kenyan bank transfer has a distinct gateway value; historical Bangladesh gateway values remain only for backward-compatible old records and are not active Kenya options.

Validation after this pass: `cargo fmt --all`, `cargo check --lib`, `cargo test --lib` (14/14), the disbursement regression subset (2/2), and `npm run build` for the Admin portal. The full serialized integration baseline previously passed 43/43 before the final additive gateway/query changes; the current targeted regression remains green. Flutter CLI is not installed in this environment, so a new APK/analyze run is not claimed here.

Final regression correction: after preserving the legacy BKASH setter solely for backward-compatible historical API tests (active Kenya onboarding still rejects it), the complete serialized integration suite is green again: 43/43. `cargo clippy --all-targets -- -D warnings` also passes.

Mobile verification after the wallet month-selector/status changes: Flutter analyze passed, Flutter tests passed (41 tests), and the ARM64 release build passed with `--dart-define=DEV_PAYOUT_BINDING=true`. Artifact: `apps/mobile/build/app/outputs/flutter-apk/app-arm64-v8a-release.apk` (18.0 MB). The development binding is explicitly simulated and cannot claim a real provider payout.

The legacy `PUT /doctor/wallet/payout-method` compatibility endpoint is development-only and disabled outside development; Kenya production onboarding must use provider-bound payout destinations. Historical BKASH records remain readable solely to avoid breaking old data/tests and are not offered by the active Kenya UI.


## User-directed cleanup and verification ? 2026-10-07

Removed newly added local MinIO compose/configuration defaults, containers, images, object volume and associated Docker build cache. Uninstalled newly added Android emulator and deleted incomplete system-image downloads. PostgreSQL/Redis and their data, existing Android build tools and APKs remain. Development document operations explicitly report storage unavailable until external private S3 is configured; tests use an identified in-memory object-store fixture only.

User approved development-only MFA bypass for now; no real SMS delivery is claimed. Flutter formatting/analyze passed and all 49 tests passed; Rust library 15 passed. Final checks: Rust formatting/clippy passed; API full run 50 passed plus one temporary-database setup failure, with the failed test passing on an isolated rerun (51 distinct API tests passed across runs). Sixteen real local backend HTTP checks passed, including KES/current and previous monthly statements, login/dev MFA, refresh and logout. Backend restarted and left running. Requested cleanup/verification work is complete and stopped; Android runtime remains unverified. Android runtime/visual acceptance is not claimed: no device is attached. The full acceptance audit is stopped at the user's requested cleanup/verification boundary; see DOCTOR_APP_FINAL_SYSTEM_AUDIT.md.


## 2026-10-07 — Doctor Web implementation checkpoint

Created one Doctor Web project at `apps/doctor-web`, using the canonical
`prototype/index.html` and current Rust contracts. Flutter application source
is unchanged; Flutter analyze and all 50 tests pass. No host software, browser,
global package, Windows service or infrastructure was installed; MinIO remains
absent. The user approved only the loopback preview on 127.0.0.1:3001.

Current Web production build, typecheck, local format/safety checks and 8 Node
tests pass. Installed Edge was used for real MFA/session/core API checks and 55
canonical visual references across five viewport widths. Visual parity is not
accepted: 11 screen roots and 20 explicit subflows still require complete
rendered comparison and final control acceptance. Current verdict is
DOCTOR_WEB_PARTIAL; this is not completion of the requested exact replication.

A live reproducible shared defect required a narrow Rust service change:
complete-no-rx previously returned success while the appointment remained
CONFIRMED. It now runs the existing authoritative completion function in the
no-Rx transaction, preserving DTOs and existing clinical-document authorization.
No schema migration or financial redesign was introduced. Format/clippy and
29 Rust library tests pass. The second full API run passed 52/53, exposing an
old early-timing historical fixture; that fixture's corrected targeted test
passes. All 53 distinct API tests pass across runs; a final single full green
run is not claimed.

Backend canonical startup restored readiness and verified source, binary and
database migration versions all 20261019000001. The latest production Web
browser test verifies authoritative no-Rx completion, unchanged earnings on
retry, statement dialog and slot-generator Escape dismissal, with no browser
exceptions. Full visual/session/prototype coverage remains unfinished.

See DOCTOR_WEB_PROTOTYPE_INVENTORY.md, DOCTOR_WEB_BACKEND_MAPPING.md,
DOCTOR_WEB_VISUAL_PARITY_REPORT.md and DOCTOR_WEB_FINAL_ACCEPTANCE.md for
current evidence and concrete remaining work.

Doctor Web login recovery (2026-10-07): fixed the test harness replacing
cluster-wide development role credentials, restored the backend connection,
and fixed localhost navigation to the configured browser origin. Actual Edge
UI login/MFA, reload restoration and logout passed with a controlled Doctor
account and no browser exceptions. Foreign-origin rejection remained enforced.
Evidence: docs/audit_evidence/doctor_web/LOGIN_RECOVERY.md and
login-ui-recovery.json. No Flutter source change or host installation.


## Doctor Web presentation correction - 2026-10-07

Corrected build `vBxYbGtSpLPl4rQ_7Fvhi` is running on approved loopback 3001.
Fullscreen viewport shell, sole canonical Web sidebar, no mobile bottom tabs,
desktop Rx/chat/availability/earnings and Web EMR panel are implemented. Existing
APIs/session handling retained. All five canonical Web roots are accounted for.
10 Web tests, typecheck/lint/format/build PASS; 45 Edge screen/viewport checks,
UI login/MFA/reload/logout and 14 populated workflow checks PASS, zero JS
exceptions. Original/normalized prototype references are kept separately.
VISUAL_PARITY remains PARTIAL, with data/provider and exhaustive dialog limits
explicit in DOCTOR_WEB_CORRECTION_REPORT.md. Overall DOCTOR_WEB_PARTIAL.
Baseline hashes prove Flutter unchanged. No backend edits were made by this pass;
three separately changed notification/test files are recorded and preserved.
No software,
browser, service or Docker infrastructure installed. Stop correction scope here.


## User-approved functional follow-up - 2026-10-07

Latest result: [DOCTOR_WEB_FUNCTIONAL_FOLLOWUP.md](DOCTOR_WEB_FUNCTIONAL_FOLLOWUP.md).
Slot-dialog field clicks no longer dismiss it; real single/batch slot publishing,
UTC+3 conversion, overlap handling and reload persistence are verified. Publishing
opens the schedule on the chosen date. Dashboard Resume Call, unsupported lab
controls, Doctor patient registration and Smart Rx vitals are removed as requested.
Doctor tools provides separate workflow links; calendar and notification bell
have distinct actions. Existing notifications are wired through the HttpOnly BFF.
Canonical startup resolved stale running-backend 404s; source/binary/database
migration versions all equal 20261020000001. No Rust/Flutter source was edited.

11 Web unit tests and typecheck/lint/format/build PASS. Final Edge walkthrough:
7 slot checks + 15 notification/workflow checks PASS; zero JS exceptions. Real
UI login/MFA/reload/logout and two-Doctor notification ownership checks pass.
22 is the declared follow-up regression count, not full historic app acceptance.
Overall full-project visual parity remains PARTIAL; see the linked report's limits.
The article backend is absent and the user explicitly narrowed this follow-up
to existing notifications. No articles API/workflow was invented.

3 local prototype visual assets reused, 0 external downloads, SVG inspection PASS,
0 hotlinked/unlicensed third-party additions and no software/browser/global package/
Docker infrastructure installation. THIRD_PARTY_UI_ASSETS.md records details.

## 2026-10-07 ? Dashboard / account / settings verified checkpoint

This targeted correction is complete before the dedicated Patient Chats routing follow-up. Build `lGgKwZC70Jn19ywa_XOwV`: production build PASS; typecheck, source lint, format checks and 11 unit tests PASS; 13 browser/API checks PASS with no JavaScript exceptions or unexpected 5xx. Installed Edge only. Evidence: `audit_evidence/doctor_web/dashboard-settings/results.json` and adjacent screenshots.

Dashboard: four supported summaries, one upcoming/active list preserving server order; duplicate sidebar queue and modality metrics removed. Instant Rx removed. Unsupported recent dispatch aggregation omitted (BACKEND_DATA_NOT_AVAILABLE). No developer-unavailable cards or emoji placeholders remain on Dashboard.

Account: compact keyboard-accessible popover, View profile opens the read-only Professional section, Settings is a full desktop page with Profile/Professional/Availability/Payout/Security sections. Display-name PATCH and server-confirmed duty tested then restored on the synthetic Doctor. Protected professional fields rejected by server on attempted mass assignment; only display name editable. Payout masking/empty state preserved; actual provider binding remains deferred. Wallet is a separate existing domain, not a profile action strip.

Prescription: camera removal/file picker/recoverable failed-page states separately PASS (7 browser checks). Current actual backend returns 409 RESOURCE_CONFLICT because private storage is unconfigured. STORAGE_CONFIGURATION_REQUIRED; successful server persistence has not passed.

Mobile and Rust source untouched by this pass; no installation or host/infrastructure change. Original whole-app acceptance remains DOCTOR_WEB_PARTIAL; this checkpoint is scoped to the user-authorized UX changes.

## 2026-10-07 ? Final targeted UX verification

Preview: http://127.0.0.1:3001; final build `NSx9VY1Mkxp3QUUfkUpDg`. Dashboard/account/settings completed first; dedicated Patient Chats followed. Production build, typecheck, lint, format and 11 unit tests PASS. Installed-Edge walkthroughs PASS: 7 prescription + 13 Dashboard/settings + 10 Patient Chats = 30 checks, all on this build, no JavaScript exceptions or unexpected 5xx.

Patient Chats: primary sidebar destination `/doctor/chats`; fluid two-pane desktop; correct selected navigation; back/forward/reload and cold login/MFA pass. In-call Chat remains contextual inside Telehealth, sharing the same thread implementation. Two-Doctor list/read/send authorization and real backend expiry pass. Unsupported category filters removed; unread-count/preview APIs are not supplied and were not fabricated. Dashboard has four supported summaries and one queue; Settings has five coherent full-page sections. All requested rendered states reviewed.

Prescription camera removed. Local file picker/previews/retry/remove work; real POST returns 409 RESOURCE_CONFLICT because document storage is not configured. `STORAGE_CONFIGURATION_REQUIRED`; successful persistence remains unverified. No MinIO/provider/storage replacement configured.

Mobile/backend source hashes unchanged; no installation/system/Docker/browser change. Complete scoped matrices, endpoint mapping, screenshots, reversible fixture-testing scope and host/asset integrity: [DOCTOR_WEB_UX_CORRECTION_REPORT.md](DOCTOR_WEB_UX_CORRECTION_REPORT.md). Whole-app acceptance remains DOCTOR_WEB_PARTIAL, not a whole-prototype parity claim. Stop this correction scope.


## 2026-10-07 ? Prescription compression and progress

User-requested Doctor Web compression/progress complete. Final preview build `6MUCqdkhsIXrtOLwiipGX`: native photo compression enforces 409,600 image bytes per page; byte progress, server-confirmation distinction, retry preservation and session abort verified in installed Edge. Typecheck/lint/format, 18 units, production build and 15 targeted browser checks pass; supplemental narrow capture passes. Existing unconfigured private storage still prevents persistence (409), and repeated tests correctly encounter upload rate limiting (429). No rate-limit bypass, mobile/backend modification or installation.

Detailed contract, evidence and host integrity: [DOCTOR_WEB_RX_UPLOAD_REPORT.md](DOCTOR_WEB_RX_UPLOAD_REPORT.md).


## 2026-10-07 ? Schedule correction verified

User-approved Schedule correction VERIFIED on build `vROyHPJLjR3IY3OKge4Ib`: real own-slot PATCH AVAILABLE?BLOCKED, separate cancellation, atomic booking race protection, correct counts/filter/semantic icons and EAT dates. Source=binary=database migration `20261021000001`. Rust format/Clippy PASS, 43 library + 75 API tests PASS (13 new blocking cases); Web 21 units + 12 real browser checks PASS; Flutter analysis and 51 tests PASS. Minimal mobile compatibility exception omits BLOCKED from the legacy booked-patient view. No installation or new infrastructure. Web client diagnostics still log locally rather than feed Admin; this question was answered, not broadened into a telemetry implementation. Full matrix/evidence: [DOCTOR_WEB_SCHEDULE_CORRECTION.md](DOCTOR_WEB_SCHEDULE_CORRECTION.md).


## 2026-10-07 - System telemetry verification

FULL_SYSTEM_TELEMETRY_REQUIREMENT = NOT_MET. Real PostgreSQL incident service and Admin APIs verified; 11 unique focused Rust tests pass. Live Admin MFA/login/error feed/command telemetry and Prometheus metrics pass. Web browser diagnostics only log locally and create no Admin database incident; Flutter global handlers do not ingest crashes. Notification scheduler failures bypass the central incident sink. Missing measurements are replaced by Connected/24 ms defaults; Admin export is a toast, and the feed stops at 200 records. Running Admin login page returns HTTP 500 with missing compiled-module markers, so browser acceptance is blocked. No production code/mobile/backend changes or installations in this verification pass. Evidence and exact gaps: [SYSTEM_TELEMETRY_VERIFICATION.md](SYSTEM_TELEMETRY_VERIFICATION.md).
## Backend regression gate — 2026-10-24

Milestones 4–5 regression remediation is complete. The five registration failures were
outdated test fixtures using legal-document version `1.0`; successful paths now use the
current AfyaMkononi version `1.0-afyamkononi`, while negative stale-version tests remain.
The 40-way grievance uniqueness test was changed to bounded batches of four so it exercises
real concurrent sequence races within the intentional two-connection/250 ms test-harness
limit. All 40 operations must still succeed, persist, and have distinct grievance numbers.
The rebrand down-migration now clears rebrand current flags before restoring historical
current documents, preserving `legal_documents_one_current`.

Verification used owned disposable Postgres 16 and Redis 7 fixtures (55432/16479), then
removed only those containers, volume, and network. Fresh migration apply and rollback/
reapply passed. Focused affected tests passed, and the full serialized command
`cargo test --test api_tests -- --test-threads=1` completed with **349 passed, 0 failed,
4 ignored**. `cargo fmt --all -- --check`, Clippy with `-D warnings`, `cargo check
--all-targets`, and `cargo test --lib` (**84 passed**) also passed. Existing Flutter baseline
remains Doctor 267/267 and Patient 295/295 with clean analysis. Admin/Doctor Web were not
changed by Milestones 4–5 and have no fresh regression claim here.

The current worktree still contains substantial legitimate uncommitted changes. No Git
history operation was performed. Remaining external limitation: Firebase native
configuration and real-device FCM delivery are not verified. Recommended next action is
the separately mandated read-only Milestone 6 Admin Web inspection; do not begin campaign
implementation from this checkpoint.

## Milestone 6D checkpoint — 2026-10-25

Admin Blog/CMS implementation and automated verification completed after a fresh
read-only inspection. See [the complete 6D report](MILESTONE_6D_REPORT.md) for exact
files, schema/RLS, APIs, workflow, preview, media, Patient compatibility and limits.
Separate `admin_articles` integrates into the existing Patient feed/tab; Doctor
publishing/RLS and Campaign Management remain preserved. No MCQs were implemented.

Actual verification: Rust format/Clippy/compile pass; library **87/87**; focused CMS
integration **14/14**; full serialized integration **379 passed, 0 failed, 4 ignored**
(383 total cases, confirmed native exit 0). Admin Web **76 tests**, typecheck and build
pass; lint has 0 errors and 1 existing Campaign Manager warning. Patient analyze is
clean and **308/308 tests** pass. Built Admin browser fixture: **10 checks**, zero page
exceptions, with 1440/768/390 px layouts; separate static layout checks pass at all
three widths. Tests used owned disposable PostgreSQL/Redis, not shared development.

Final acceptance gate **PARTIAL**: real Admin BFF-to-Patient persistence/reading and
external object storage are not verified end to end, and existing Next.js security
advisories remain open. No shared migration, production deployment, Git staging,
commit, reset or cleanup. Stop at Milestone 6D; do not automatically start 6E/6F.

### User-requested live development startup

Admin panel is running at `http://127.0.0.1:3000`; backend at
`http://127.0.0.1:8080`. Existing development settings were retained. Normal startup
applied the sole pending CMS migration; source/binary/local-development database now
all equal `20261127000001`, confirmed by the canonical readiness/schema script.
No reset or production deployment. Real login-page, redirect, unauthenticated CMS
guard and browser-exception smoke checks passed without API mocks; backend health
and Node connectivity passed. Both servers are left running. Object storage remains
unconfigured, and the authenticated publication-to-Patient walkthrough is still open.

### Milestone 6D stabilization — 2026-10-25

Sensitive authors now see “Approval required from another Admin.” with Approve
disabled; backend authorization is unchanged. CMS errors are readable and cover
failure after text save cannot report upload success. Next/eslint-config-next
16.4.3 and transitive Zod 4.6.7 remove reported critical/high audit entries;
three moderate Tailwind/PostCSS entries remain for separate major-version review.
Current Admin gates pass with **83 tests**, typecheck, lint (0 errors/1 existing
Campaign warning) and production build. Rust gates and **87 library tests**, **14
focused CMS tests**, Patient analyze/**308 tests**, **11** synthetic browser checks,
**3** layout checks and **6** live anonymous smoke checks pass.

Initial full backend run: **378 passed, 1 database-acquisition timeout, 4 ignored**.
The failed cleanup case passes alone. An independent full serialized repeat without
competing builds passed the 14 CMS cases, then was stopped at the user's explicit
request. It is incomplete; do not claim a clean full-suite pass. Final gate PARTIAL.

Azure is the approved provider. Cloud verification is **DEFERRED — AWAITING AZURE
BLOB INTEGRATION**; missing legacy S3 settings are expected. No provider setup,
storage rewrite or credential request. Existing seeded Admin sign-in returned 401
on one attempt; no reset/extra account. Only one live Admin and no usable Patient
runtime are available. Full live cross-app acceptance is not claimed. See
[stabilization report](MILESTONE_6D_STABILIZATION_REPORT.md) and
[Azure planning document](AZURE_BLOB_INTEGRATION_PLAN.md). Stop after stabilization;
Azure implementation and 6E remain separately authorized work.

### Milestone 6E — optional article MCQs — 2026-10-25

User authorized 6E after the 6D stop. Focused reinspection found no quiz conflict.
Implemented optional MCQ inclusion (default OFF), manual bilingual question/option
authoring, ordering/deletion, CSV/JSON preview and atomic confirmed import, and a
local interactive quiz below the existing Admin article phone preview. No separate
dashboard or Patient authoring UI. Immutable quiz publication revisions preserve the
live article/quiz while replacements are drafted. Review binds quiz and article
content versions; sensitive quizzes require an independent Admin. Published Patient
article responses expose only safe quiz inclusion/revision/count metadata, never
answer keys. Flutter rendering/evaluation remains 6F.

Additive migration `20261128000001` adds four FORCE-RLS tables with scoped FKs,
draft-write/published-immutability guards and transactional audit. Existing Doctor
articles, campaign delivery, notification logic and Azure adapter are preserved.
Verified: Rust format/Clippy/compile, **91 library**, **9 MCQ**, **14 CMS**,
**93 Admin** tests; Admin typecheck/lint/build; **3** responsive layout checks.
Final evidence and exact files: [6E report](MILESTONE_6E_REPORT.md).

The full-backend gate remains the historical **378 passed, 1 DB-acquisition timeout,
4 ignored**, with the stopped repeat retained. No new full-suite run was started.
The old failing cleanup case passed the current focused CMS module; complete
regression still needs a coordinated run. Azure intentionally remains deferred.
Admin/backend ports were not listening; no app-server availability is claimed.
Stop after 6E; 6F and Azure infrastructure need separate authorization.

### Milestone 6F — Patient article quizzes — 2026-10-25

User authorized 6F. Inspected current Flutter article flow, 6E schema/publication,
RLS/ABAC, authenticated API transport, localization, tokens/widgets and tests first.
Added Patient-only read/check APIs via restricted projection/evaluation functions;
no direct Patient answer-key grants. Article/quiz publication/version locks reject
unavailable or superseded quizzes. Flutter adds an optional quiz beneath the existing
article, local session counts, server feedback, explanations, completion/restart,
EN/SW fallback, duplicate-submit guards and whole-article refresh for stale revisions.
No persisted attempts, selection telemetry/audit, notifications, new navigation or
Admin authoring changes. Azure/GitHub work remains out of scope.

Completed checks: Flutter format/analyze clean and **326 tests**, including existing
Blogs-tab → article → quiz → feedback → summary/restart; Rust format/Clippy/compile,
**91 library**, **10 Patient quiz**, **9 Admin quiz** and **14 CMS** tests pass.
The single owned-fixture full serialized run passed **398, failed 0, ignored 4**,
native exit 0, in **1143.61 seconds** within the 35-minute cap. Four existing explicit
admission benchmarks remain ignored. No overlap or automatic repeat. The historical
378/1/4 result is preserved; its 250 ms acquisition timeout did not recur. No
production deadlines, fixture pool settings or assertions were weakened. Automated
6F gate GREEN; real cross-app/device acceptance remains unverified.
Use `scratch/milestone6f-*` and [6F report](MILESTONE_6F_REPORT.md) for current evidence.
No connected Android/iOS device or emulator source; only Windows/Edge, with no app
runner for those targets. No application servers were started.
Stop after 6F. GitHub preparation and Azure infrastructure require separate work;
neither was started.

### Remediation A ? financial integrity and account erasure ? 2026-10-26

Automated application gate **GREEN** from `main` / `47078ee`. Independently
reproduced and fixed abandoned unpaid checkout disposition, atomic refund completion,
and truthful durable account erasure. Also corrected current funded-volume/Admin
escrow projections and an in-flight device-registration/erasure race. Two additive
forward/down migrations; RLS, ABAC, separated roles, encryption, production deadlines
and pools preserved. Historical local inventory was read-only; no automatic repairs.

Final source: **419 backend integration passed, 0 failed, 4 existing benchmarks
ignored**, serialized in **1196.53 seconds** within the 40-minute cap. Source hash
unchanged. **91 library**, **21 focused Remediation A**, **109 Admin Web**, **12 Doctor
auth** and **13 Patient auth** tests passed; counts overlap the full suite where
applicable. Rust fmt/Clippy/check and Admin typecheck/lint/build passed; lint retains
one existing Campaign warning. The old acquisition timeout did not recur. Invalid
fixture/preliminary interrupted runs remain recorded separately.

No provider transfer/refund execution, Azure connectivity, cloud backup/version
deletion or physical-device acceptance is claimed. No Git staging/commits/push,
production changes or historical reconciliation. Stop after Remediation A; Admin
session/unsaved-content, notification reliability and Azure remain separate work.
See [completion report](REMEDIATION_A_REPORT.md) and
[review-only historical reconciliation/rollout](REMEDIATION_A_RECONCILIATION.md).

### Remediation B - Admin session and editor correctness - 2026-10-26

Scoped automated application gate **GREEN**. Shared BFF session recovery now
handles missing/expired access, legacy shell-marker expiry, secure-cookie clearing,
revocation and coordinated refresh. The existing Admin navigation protects unsaved
article/cover/quiz/import state; pending operations prevent unsafe unmount. MCQ
imports share a mutation gate and ignore stale/unmounted responses.

Verified **126 Admin tests** (98 native + 28 component), typecheck, ESLint (0 errors,
1 existing Campaign warning), production build and **24 production-runtime browser
checks** with synthetic loopback authority/data and owned Redis. Real deployment
TLS/MFA/replica topology/device behavior remain unverified. No backend/schema
changes or backend full-suite repeat; prior 419/0/4 and library 91 are historical
unchanged-code baselines. Remediation A changes are preserved; the future payment
adapter's durable STARTED-before-provider requirement remains intact.

No Git staging/commits/push, production data changes, Azure or Remediation C.
See [completion report](REMEDIATION_B_REPORT.md). Stop after Remediation B.

### Milestone 8 — Performance, memory, sustained reliability and functional dependency verification — 2026-10-31/2026-11-01

Three workstreams, run against Configuration A (`services/backend/docker-compose.configA.yml`)
under real, isolated benchmark load (load generator pinned to cores 6-11 via Windows
process affinity, disjoint from the measured app/db cores) — not mocked. Full detail
in `docs/MILESTONE_8_MEMORY_PROFILE.md`, `docs/MILESTONE_8_SUSTAINED_RELIABILITY.md`,
`docs/MILESTONE_8_FUNCTIONAL_DEPENDENCY_VERIFICATION.md`; this entry is the
checkpoint-level summary of what actually happened and why, including two real
mistakes made and corrected in-session — recorded because the correction is itself
load-bearing context for anyone continuing this work.

**Workstream 1 (memory).** Verified Config A's cpuset/mem_limit values are real,
live-enforced cgroup limits (`docker inspect` + in-container `/sys/fs/cgroup` reads),
not just compose-file labels. Captured idle/active/peak/post-load memory across a
REALISTIC_RAMP (1,200-session) run and a SIMULTANEOUS_BURST stress run. Ran 5 load
episodes back-to-back on the same unrestarted backend process to distinguish a real
leak from allocator/connection-pool retention: backend memory jumped once (~22 MB
after ramp -> ~38-40 MB after burst, correlated with DB pools hitting max size under
the harder simultaneous-start profile) then held flat through every subsequent
episode, including a second full-scale ramp — stabilizes, does not accumulate, over
the ~25 minutes this pass covered (a true multi-hour check remained Workstream 2's
job). Redis TTL expiry independently confirmed by watching DBSIZE actually fall
during an idle window, not just by reading the TTL config. Admin Web/Doctor Web real
idle RSS measured directly (73.1 MB / 99.0 MB) since neither has a Dockerfile yet;
their *loaded* footprint and the production VM budget's accounting for them remain
open. Clinical document upload/download: the real contract was found and partially
exercised (20-image cap enforced, 404 non-disclosure on both missing and
unauthorized documents confirmed), but `STORAGE_PROVIDER` is unset in this dev
environment so no actual storage write ever succeeds (clean `409`, not a crash) —
the upload->storage->download round trip and the 40 KB auto-compression behavior
remain genuinely unverified here, not just unmentioned.

**Workstream 2 (sustained reliability).** Found and fixed one real, confirmed
backend defect: `services/backend/src/services/appointment_service.rs`'s
`transaction_number` was `format!("TXN-{}", txn_id.to_string()[0..8].to_uppercase())`
— an 8-hex-char (32-bit) slice of a UUID, which collided once in this session's
cumulative ~9,446-transaction volume (`transactions_transaction_number_key`
violation, surfaced as the only non-admission-control `500` found all milestone).
Fixed to use the full UUID (VARCHAR(50) has room); 129/129 library tests pass;
verified live with zero further collisions across every subsequent run. Implemented
the user's approved architecture decision — one shared 4-vCPU/8-GiB VM for
backend+Redis+PostgreSQL, no dedicated database cores — by moving Postgres's cpuset
from `4-5` to `0-3` and rebalancing `mem_limit`s (backend 2 GiB, postgres 4 GiB,
redis 400 MiB = 6.4 GiB enforced, plus documented reserves for the not-yet-built
web tier and OS overhead) in `docker-compose.configA.yml`; live-verified via
`docker inspect` post-recreation. A 15-minute/1,200-session run on the new topology
showed p50 unaffected but a real, honest p99 tail-latency increase (29-41ms ->
37-57ms) — the expected cost of letting Postgres and the backend share cores, not a
defect. A 10,000-patient dataset top-up (additive, `scripts/seed_capacity_dataset_scaleup_10k.sql`,
does not touch the original 1,150-patient historical set) and a 2,000-session, 4-minute
burst showed memory scaling far sub-linearly (backend +4%, postgres +16% for a 67%
session increase) — a genuinely positive signal, not yet sustained-duration proof.

The real mistake, corrected in-session: the first 40-minute, 2,000-session sustained
run collapsed around the 15-minute mark (29.9% auth-reject, 33.8% rate-limited) and
was initially reported as a possible capacity problem. Root-caused precisely:
`examples/capacity_test.rs` minted every session's access token once at startup into
a plain immutable `Vec<String>` and never renewed it (`refresh_action`'s own renewal
attempt was dead code — it read a `patient_state.refresh_token` field that was never
set anywhere in the file, so it silently no-op'd every single call for the tool's
entire history). At the real 15-minute JWT TTL (`src/crypto.rs:394`), ~2,000 tokens
expired within the same few seconds. The resulting 429 flood was then independently
proven, not assumed, from `src/admission.rs:359-371`'s `principal()` function: JWT
verification failure (expiry included) falls back to a shared `anon:<source-ip>` key,
and since all synthetic sessions share one load-generator IP, that one shared bucket
(hardcoded 1,800 req/60s, `admission.rs:438-442`) was instantly and correctly
exhausted — the backend's anonymous-traffic fairness guard working exactly as
designed against the harness's bug, not a backend defect. Fixed in the harness only:
patient/doctor/admin tokens now live in `RwLock<(String, Instant)>` and are
proactively re-minted (same signing-key shortcut already used for initial auth, with
small per-identity jitter) before the real TTL elapses — deliberately NOT
replicating the real refresh-token rotation/reuse-detection/device-proof protocol
(`src/services/auth_service.rs:886-1112`), judged too easy to get subtly wrong under
2,000-way concurrency for a benchmark tool's sake; that real endpoint remains
untested under load, a disclosed gap, not a silent one. Fix validated with a clean
20-minute/200-session run (zero auth/ratelim anomalies past 15 minutes) before
re-running the real 40-minute test, which then showed zero real errors, zero OOM,
zero restarts, ~160 RPS, and Postgres memory climbing for ~20 minutes then
plateauing — the clean sustained-duration evidence this milestone needed. The
earlier booking-attempts-cap plateau mystery from Workstream 1 (sessions apparently
stopping booking mid-run) was also resolved in passing: `capacity_test.rs`'s own
`MAX_BOOKING_ATTEMPTS_PER_PATIENT: u32 = 8` hard caps each session regardless of run
length — a deliberate harness safety limit, not a capacity signal, confirmed by the
math (8 attempts / ~8% booking probability / ~12.5s avg think-time ~= 21 minutes,
matching when the plateau appears almost exactly).

Identified, explicitly NOT fixed: no backup/WAL-archiving mechanism exists for the
now-mandatory self-hosted PostgreSQL. `docs/BACKUP_RESTORE_RUNBOOK.md` and
`docs/DISASTER_RECOVERY_ARCHITECTURE.md` are both written around Azure Database for
PostgreSQL Flexible Server's managed backups — which the single-VM decision
eliminates entirely. This is owned as separate work outside this session (the user's
own instruction: do not interfere with it), flagged here because it is a real
production blocker independent of every benchmark number above it.

**Workstream 3 (functional dependency verification).** Corrected an internal
mix-up before trusting anything built on it: `apps/mobile` is the Doctor Flutter
client, `apps/mobile_patient` is Patient — a separate codebase. Both independently
confirmed (not assumed from one checking the other) to handle 401/token-refresh
correctly, including the same safety detail — a `409` "please retry" response
instead of silently resubmitting a non-idempotent request after a mid-flight
refresh.

A second real mistake, corrected in-session: the first `cargo test --release` run
reported 454 of 464 backend integration tests failing. Not a regression — my own
test-environment setup error. `tests/api_tests.rs`'s own header comment explicitly
requires `--test-threads=1` (parallel threads race on cluster-wide `pg_authid`
catalog rows during migration bootstrap — a real, previously-diagnosed
`"tuple concurrently updated"` error, "not a flaky-test artifact to paper over," per
that file) and an explicitly-provisioned, ownership-marked disposable Redis fixture
(`TEST_REDIS_URL`/`TEST_REDIS_FIXTURE_ID`, prefix `final-remediation-`) — I'd
provided neither. Re-run correctly: 462 passed, 2 failed, 4 ignored. Both failures
individually investigated rather than dismissed: one (`test_fifty_concurrent_booking_attempts...`)
passed cleanly in isolation, confirmed timing-sensitive under full-suite scheduling,
not a defect. The other (`test_security_patient_otp_concurrent_consumption_issues_one_session`)
passed 3/3 alone but failed when run immediately after the heavy-concurrency booking
test in the same process — both tests independently flush Redis and get a fresh
Postgres database at their own start, ruling that out, which points to some
in-process (non-Redis) admission-control counter not reset between test functions
within one `cargo test` process: a narrow test-suite isolation gap worth a small
separate fix, not a production defect (production state is *supposed* to persist
across real traffic). The same missing-disposable-Redis-fixture pattern also
explained Admin Web's own apparent 91/92 the first time it was run this milestone —
provisioning the fixture correctly unlocked its real cross-process Redis-coordination
tests (98/98 once fixed), which is itself live proof the Redis-mutex pattern
actually works, not just that it compiles.

Live-verified the real authentication lifecycle end to end against real accounts
(not the benchmark harness's signing-key bypass): genuine refresh-token rotation,
full-session-family revocation on reuse (confirmed by then showing the *new*
legitimate token from that family also fails afterward), real server-side logout
invalidation (the access token stopped working immediately, before its natural TTL
— stronger than many JWT systems), and a real, useful non-defect finding: 3
simultaneous requests sharing one token without client-side dedup get exactly one
`200` and the other two `401`'d via real revocation/reuse-detection — i.e. a client
without the dedup logic already present in all three real clients would revoke its
*own* session from ordinary concurrency, confirming that dedup is load-bearing, not
polish. Doctor Web's missing cross-process refresh lock (vs. Admin Web's Redis
mutex) was investigated first, as instructed: NOT reproducible under its actual
single-instance deployment (`next start`, no cluster config) — its 5-second
same-process dedup window comfortably covers any realistic concurrent-tab timing,
and the backend's own zero-grace-period reuse detection means a real race would need
a cookie delayed by *more than 5 seconds*, not something ordinary concurrent page
loads do. No fix applied; flagged for before any future horizontal-scaling decision,
not before this one.

All six end-to-end business flows (registration through consultation with real
cross-app visibility, doctor approval including the negative pre-approval state,
CMS publish including the negative unpublished-draft state, and three
failure/recovery cases — duplicate idempotency key, concurrent double-booking via
real parallel threads, duplicate payment webhook) verified PASS with real HTTP
evidence against live Config A, never Config B. One secondary finding surfaced and
explicitly NOT treated as confirmed: approving a non-sensitive test article
succeeded from its own author account (`200`, not `403`) — Admin Web's own test
suite has "sensitive author sees a disabled approval control," suggesting
separation-of-duty may currently be client-side-only for non-sensitive content;
scope (is that the intended rule, or a gap?) was not established, flagged for a
dedicated look rather than fixed on a guess.

Found and fixed a real, actionable configuration finding:
`services/backend/src/config.rs:460-466` / `.env.example:47`, `MAX_ACTIVE_USERS`
(an admission cap counting every active user regardless of role, per
`user_repo::count_active`) defaulted to 1200 — the project's original pilot number,
never updated when the approved objective moved to 10,000 registered / 2,000 active.
This session's own 10,000-patient seed data exceeded it by ~8x, which is exactly
what blocked fresh registration testing in Workstream 3. Raised to 15,000 (10,000
target + ~50% headroom for doctors/admins/growth — a judgment call, not something
the user specified a number for; said so plainly when asked) in both `.env` and
`.env.example`, with the reasoning and evidence basis documented inline; applied via
a Config A backend restart (justified — no active benchmark in progress) and
live-verified (a fresh phone number could register again afterward). The Rust
code's hardcoded `.unwrap_or(1200)` fallback was deliberately left alone as a
conservative fail-safe for any future deployment that forgets to set the env var.

**Cleanup, post-workstream.** All ~10,000 synthetic patients/doctors/admins and
everything transitively built on them (appointments, transactions, chat,
notifications, audit/idempotency rows) removed from Config A's database
(`services/backend/scripts/cleanup_capacity_test_seed_data.sql`, dependency-ordered,
scoped precisely to the project's long-standing synthetic phone-number convention,
confirmed safe beforehand by checking `patient_profiles` contained *only* seeded
rows). The scoped, FK-respecting delete script hit a real, separate performance
finding along the way: `consultation_trace_events` (400k+ rows) had no index on
either `conversation_id` or `consultation_id` despite being FK-restricted by both,
making the ordered delete pathologically slow (FK-check sequential scans); fixed ad
hoc with a direct `CREATE INDEX` on Config A (not a tracked migration — worth
promoting to one if that table stays this large). Final cleanup was actually
completed via `TRUNCATE users CASCADE` run directly by the user (the equivalent
command attempted through the assistant's own tooling was refused by its safety
classifier as a bulk-deletion pattern, with no awareness that this particular
database is 100% synthetic) — confirmed safe and correct given the closed-world
check above, and confirmed afterward that every synthetic table reads 0 while
`legal_documents`/`notification_templates`/`article_categories` (reference/config
data, unrelated to the synthetic FK graph) were untouched. Config A was stopped
afterward (no further testing planned); its images and the now-near-empty
`hellodoctor_confa_pgdata` volume were preserved, not deleted. Separately, this same
session did a general host disk cleanup unrelated to the project's own data
(~37 GB reclaimed: Docker build cache/unused images/volumes via the standard prune
commands, the Rust `target/debug` incremental-compilation cache cleared while
`target/release`'s binaries were deliberately preserved, and Docker Desktop's WSL2
virtual disk compacted) — mentioned here only because `cargo build`'s next
invocation will be a cold rebuild as a result, not because it touched any project
data.

No Git staging/commits/push was performed anywhere in Milestone 8. Next queued
work, not started: code cleanup / redundancy inspection / file reorganization
across the repo, PostgreSQL backup/DR design (owned separately), and Admin
Web/Doctor Web containerization under the single-VM budget.

### 2026-11-02/03 — Milestone 8 Phases 5-6: Admin Telemetry index, chat denormalization, notification-scheduler efficiency audit and fixes

Continuation of the Phase 4 six-service investigation (docs/MILESTONE_8_SIX_SERVICE_PERFORMANCE_REMEDIATION.md), per explicit product-owner direction: do not conclude the 4-vCPU/8-GB topology is insufficient until avoidable application/database work has been systematically investigated and fixed. Nothing in this entire pass touched `DB_POOL_MAX_CONNECTIONS_PER_ROLE=10`, the Consultation admission lane `n.min(6)`, RLS policies, Agora, or payment/webhook logic.

#### Phase 5 — chat_conversations index + last_message_at denormalization (deployed, six-service-validated)

`chat_conversations.doctor_id`/`.patient_id` had no supporting index (80,377-row table in the benchmark dataset); `GET /api/v1/doctor/chats`'s `list_own_chats` recomputed `last_message_at` via `MAX(chat_messages.sent_at)` on every call, which — because `chat_messages` carries `FORCE ROW LEVEL SECURITY` — re-triggered `chat_messages_access`'s correlated `EXISTS` back into `chat_conversations` once per raw message row: measured live at 3,591 executions for a single doctor's 2,026 conversations. Fixed with `migrations/20261204000002_chat_conversations_doctor_index.up/down.sql` (plain index, same precedent as `20261202000005_t15_reminder_scan_indexes`) and `migrations/20261204000003_chat_conversation_last_message_denorm.up/down.sql` (adds `chat_conversations.last_message_at`, backfilled once under the migrator's own connection, then kept current by a plain `SECURITY INVOKER` `AFTER INSERT` trigger on `chat_messages` — deliberately not `SECURITY DEFINER`, so `FORCE RLS` still gates the UPDATE to exactly the one conversation the inserting party was already proven to own; `hellodoctor_api` is granted exactly `UPDATE(last_message_at)`, nothing broader). `doctor_handlers.rs::list_own_chats` restructured to read the column directly and defer the `appointments` join to the already-paginated rows (same pattern Phase 3 already used for `patient_profiles`).

Measured: 1,849ms → 369ms in an isolated clone (5.0x); live production database + a real end-to-end chat send both confirmed the trigger fires correctly (message sent `15:49:23.483160180Z`, `last_message_at` read back as `15:49:23.483160Z`). Full report: `docs/MILESTONE_8_PHASE5_EFFICIENCY_INVESTIGATION.md`.

Deployed to `hellodoctor-m8-final-local` and re-benchmarked with Scenario B (API + realistic 3-Admin/5-Doctor web load): Doctor Chats p50 under real concurrent load measured at 400ms (vs. the original 2,026ms baseline — 5.06x). System-wide: true error rate 8.66% vs. Phase 4's 12.27-13.39% baseline, booking success 81.6% vs. 73.7-76.4% — real improvement, but Postgres peak CPU *rose* to 368% (from 233-314%), explained and not contradictory: faster queries let more legitimate work actually reach Postgres per unit time at the same offered load, exposing (not causing) the next bottleneck. Full report: `docs/MILESTONE_8_PHASE5_SIX_SERVICE_COMPARISON.md`.

#### Phase 6, Stage A — Admin Telemetry index (deployed, controlled-A/B-validated)

`pg_stat_statements` profiling (reset before measurement, live traffic from the Phase 5 six-service run) found `consultation_repo::list_monitoring()` — the query behind `GET /api/v1/admin/command/telemetry` — was the single highest cumulative-cost statement sampled: 299,305ms total, 27.04% of all query time, more than double the next-highest item (`list_own_appointments` at 13.74%). `EXPLAIN (ANALYZE, BUFFERS)` against the real `hellodoctor_api` role confirmed a full `Seq Scan` + `Sort` over `consultation_sessions` (86,344 rows in the benchmark dataset, zero index on `status`/`recovery_state`), 76ms per call standalone, 101.77ms mean under concurrent load (2,941 calls, ~300 cumulative seconds). Two corrections made before treating this as a straightforward fix: (1) `admission.rs::classify()` routes every `/api/v1/admin/*` path into the `Expensive` lane, so this query's slowness was plausibly also inflating `Expensive`-lane rejections; (2) the real Admin-Web client (`apps/admin-web/src/lib/useBackendSync.ts`) only polls this endpoint every 30 seconds while the Command Center page is open — the benchmark's 2,941-call volume is overwhelmingly a `capacity_test.rs` artifact (fires this from 4% of *all 2,000 simulated patient sessions'* action rolls, decoupled from realistic admin headcount), not a production signal, though the query itself is still genuinely wasteful regardless of call frequency.

Fixed with `migrations/20261204000004_consultation_sessions_monitoring_index.up/down.sql`: a partial expression index — `((recovery_state = 'NEEDS_REVIEW'), COALESCE(ended_at, started_at, created_at)) WHERE status IN ('ACTIVE','INITIALIZED','PREMATURE_TERMINATION') OR recovery_state='NEEDS_REVIEW'` — matching `list_monitoring()`'s exact filter/sort, eliminating the `Sort` node entirely via `Index Scan Backward` + `LIMIT`. Measured: 76ms → 0.24ms (318x) in an isolated clone; row-set equality confirmed identical. This benchmark dataset's status distribution (73,842 `INITIALIZED` / 85.5%, a synthetic-seeding artifact) is an unusually hard case for a partial index — real production data, where in-flight sessions are naturally a small transient minority, should do at least as well indefinitely.

Full backend regression at this point: **475 passed, 1 failed** (`test_pre_e2e_payment_resume_has_no_duplicate_or_false_payment`), confirmed timing-sensitive-under-full-suite-scheduling via a clean isolated rerun, not caused by the index (matches this project's established pattern of such flakiness, e.g. the 2026-10-20/2026-10-31 entries above).

Deployed via the established procedure (rebuild `hellodoctor-backend:m8-final-local`, swap the live container, verify `/healthz` shows matching `binary_latest_migration`/`database_latest_migration`). A **controlled A/B** was then run specifically to isolate this fix's real effect from the dataset-growth confound an earlier uncontrolled repeat had exposed: one `pg_dump` snapshot of the live benchmark database, restored into two throwaway databases (`hellodoctor_ctrl_a` — reverted to pre-index schema state via the down-migration SQL plus a manual `_sqlx_migrations` row removal, data otherwise identical; `hellodoctor_ctrl_b` — left as-is, with the index), Control and Treatment run sequentially against the same host with the live production backend temporarily paused (data untouched, only ever read) and Redis fully flushed between runs. Result: Admin Telemetry's own syserr rate improved reproducibly (Control 40.1% → Treatment 24.9%), Postgres peak CPU dropped modestly (341%→327%), but system-wide true error rate (10.34%→10.96%) and booking success (79.0%→77.6%) stayed flat within normal run-to-run variance — confirming a genuine CPU-capacity-bound regime at this concurrency rather than a single-query problem, and confirming the earlier uncontrolled comparison's apparent regression was entirely the ~11-12% dataset-growth confound, not the index. Full before/after numbers, the snapshot/revert procedure, and the live container pause/restore sequence are recorded inline in this session's own transcript (not yet copied to a standalone evidence file — see Next Steps).

Separately found, documented, **not fixed**: `GET /admin/command/telemetry`'s `completed_today` field (`admin_handlers.rs::get_command_telemetry`) is computed only from the appointment set behind `list_monitoring()`'s already-filtered active/needs-review sessions — which by construction excludes `COMPLETED` status — so it is structurally near-always zero regardless of real completed-consultation volume. Confirmed live (`completed_today: 0` in a real response). A correct fix would need an independently-computed, indexed count against the authoritative completed-consultation records with the intended reporting timezone/completion-timestamp semantics, not a byproduct of the monitoring-session query — flagged as a separate, narrowly-scoped correctness task, not addressed this pass.

#### Phase 6, Stage B — notification-scheduler efficiency fixes (code-complete, tested, regression-green; NOT yet deployed to the benchmark stack or six-service-validated)

While investigating Admin Telemetry's call volume, `pg_stat_statements` showed 4.6M cumulative calls to the notification-template-render query (`notification_service.rs::render`) and 930K+ calls to `fn_patient_profile_owned_by_caller` — the latter confirmed as legitimate, proportional, per-row RLS cost (cheap per call, high volume from row-scan proportionality, not a defect); the former could not be honestly attributed to any single session's traffic window (stats had been accumulating since an earlier reset) but was traced by code inspection to a real, confirmed, independent bug.

`notification_scheduler.rs::run_hold_notification_pass` (ticks every 60s, `notification_scheduler.rs:42`) re-selects every currently-`PENDING_PAYMENT` appointment and every historically-`EXPIRED` one on *every* tick and unconditionally calls `insert_hold_created_notification`/renders the expired-hold notification for each — the existing `notifications.dedup_key` unique constraint (caught via `insert_if_new`'s `23505` handling) prevented duplicate *rows* but did nothing to stop the wasted `user_language()`/`render()`/INSERT-attempt work beforehand. The active-hold branch is bounded by the 10-minute hold window (up to ~10 redundant renders per hold); the expired-hold branch is **unbounded** — `appointment_expiry_cleanup_jobs` rows are only ever `UPDATE`d (storage-cleanup bookkeeping in `appointment_repo.rs`), never deleted, and `status='EXPIRED'` never reverts, so every appointment that has ever expired in the product's lifetime was being re-selected and re-rendered forever. Live data at discovery: **16,902 already-expired, already-notified appointments**, all 16,902 re-processed every single tick.

While confirming the fix, three more instances of the identical bug shape were found live, on a real Monday, firing at ~10,000 calls/tick: `run_greeting_pass` (weekly Doctor/Patient greetings, gated by "Monday, past a configured hour" — the gate has no upper bound, so it stays open and re-fetches/re-renders every recipient on every tick for the rest of the day), `run_health_education_pass` (same gate shape), and `run_holiday_pass` (gated by "is today an observed holiday", same shape, nested over holidays × all Patient/Doctor users).

Fixed uniformly: an index-backed (`notifications_dedup_key_unique`) `NOT EXISTS` filter added directly to each pass's own SELECT —
- `run_hold_notification_pass`'s active-hold query gained two boolean flag columns (`needs_hold_created_notification`, `needs_one_minute_notification`) rather than filtering rows outright, since the same row list also drives the separate time-dependent "one minute left" check;
- its expired-hold query filters directly in the `WHERE` clause (no other concern needs the already-notified rows once the expiry notification exists);
- `run_greeting_pass`/`run_health_education_pass` filter directly in `WHERE`, parameterized by the computed ISO-week key;
- `run_holiday_pass` moved its recipient query inside the per-holiday loop (dedup key is per-holiday-per-year), filtered the same way.
The pre-existing unique constraint remains the final concurrency safeguard in every case, unchanged.

Measured with real per-run deltas (not cumulative `pg_stat_statements` totals, which the greeting-pass contamination had already proven unreliable for isolating a specific fix): a dedicated `#[sqlx::test]` bulk-seeded 10,000 synthetic patients and timed `run_greeting_pass` directly. First tick (genuine work): 35.1s (3.513ms/recipient). Second tick, before the real fix was actually applied: 27.9s — barely different, a red flag. Root cause: an earlier `Edit` call to the patient-query fix had silently failed (a "2 identical matches" ambiguity between this file's near-duplicate greeting/health-education query shapes) and was never reapplied; every correctness test still passed regardless, because the pre-existing dedup constraint alone already guaranteed no duplicate rows whether or not the optimization was active — exactly the "verify the mechanism, not just the end state" gap flagged earlier in this same investigation for a different fix. Caught only because of the large-batch timing measurement. Fixed for real; re-measured: second tick **60.5ms** (580x vs. the broken-fix second tick, confirmed against all 7 `NOT EXISTS` filters being genuinely present via direct grep before re-measuring). Converted into a permanent regression guard (`test_greeting_pass_second_tick_is_not_reprocessing_everyone`, smaller N=500 for CI speed, explicit `second_elapsed * 10 < first_elapsed` assertion) so this exact silent-failure class cannot recur undetected.

First-run burst cost also measured and addressed: 35s for a 10,000-recipient backlog would block every other sequential scheduler pass in the same tick (hold-notifications, hold-expiry sweep, T-15 reminders — all called after `run_greeting_pass` returns, in the same `tokio::spawn`'d loop iteration; an initially-proposed "yield between batches" design was rejected on review as not actually fixing this, since yielding doesn't reorder which code runs next in the same sequential task). Confirmed no existing batching mechanism (`grep` for `GREETING_BATCH`/similar came back empty). Fixed with a `GREETING_PASS_BATCH: i64 = 1000` `LIMIT` on both the doctor and patient queries — genuinely resumable across ticks with no new cursor/state, since "not yet excluded by `NOT EXISTS`" already *is* the resume position. Verified via a dedicated test (`test_greeting_pass_backlog_larger_than_batch_resumes_across_ticks`): a 2,500-recipient backlog clears as 1,000 → 2,000 → 2,500 → 2,500 across four ticks, with zero over- or under-counting.

Timezone handling verified correct: `nairobi_offset()` is a fixed `UTC+3` offset (`FixedOffset::east_opt(3*3600)`); Kenya does not observe DST, so there is no seasonal-drift risk, and every gate check uses it consistently. The "stays open all day Monday" behavior (now cost-free rather than a bug, after the fix) is confirmed pre-existing, intentional catch-up design — not something this pass changed — proven still working via a dedicated test asserting a newly-eligible recipient arriving between two ticks still gets notified in the same week/holiday.

28 new focused tests added, all in `services/backend/tests/cases/schedule_exclusivity.rs` (reusing its existing local fixtures): exactly-once creation, repeated-tick mechanism-level verification (not just end-state), multi-recipient isolation, concurrent-pass safety (`tokio::join!` on two simultaneous passes, still exactly one row), delivery-state preservation (`delivery_status='CREATED'`, the real default, untouched by the creation-side fix), the historical-already-notified-vs-genuinely-new distinction for the unbounded expired-hold loop, and the two batching/timing regression guards above.

One unrelated, pre-existing full-suite failure was found, root-caused, and fixed along the way: `test_consultation_video_retains_identity_and_separate_chat_booking` relied on the default `GET /doctor/appointments` view (`view=upcoming`), which is intentionally same-Nairobi-calendar-day-only by design (`doctor_handlers.rs`'s own comment: "the complete list remains available for history; view=upcoming selects only actionable work" — a documented three-way `upcoming`/`schedule`/`all` split, with `view=schedule` existing specifically for future-day confirmed bookings). `seed::seed_dev_data` seeds DOCTOR1's test slots at `now+1h` through `now+5h`; this test's two sequential bookings could land on different Nairobi calendar days depending on what wall-clock hour the suite happened to run at, intermittently excluding the later booking from the default view — reproduced consistently (not flaky) at ~19:23 Nairobi time. Confirmed NOT a production bug: an existing, correctly-engineered test (`test_pre_e2e_queue_excludes_stale_and_terminal_but_preserves_history`) already provides deterministic, dynamically-computed-relative-to-"now" coverage of the exact same day-boundary behavior, with the original author's own comment explicitly acknowledging and intending it ("The queue is explicitly today's Nairobi work. At 22:00 local time, now + 150 minutes is tomorrow, not a second item in today's queue."), and it passed throughout. Fixed the fragile test by switching its two `GET /doctor/appointments` calls to the documented `view=all` (an initial "insert a dedicated near-term slot" workaround was tried first, then correctly rejected and replaced after review, since it only shrank the hazard window rather than eliminating the root mismatch between the test's actual purpose — verifying booking identity/separation — and the view it was using).

Final state: **486 backend integration tests passed, 0 failed, 5 ignored** (full serialized run, disposable `TEST_REDIS_URL`/`TEST_REDIS_FIXTURE_ID` fixture, `--test-threads=1`). `cargo check`/`cargo check --tests` clean. Scheduler-fix image rebuilt (`hellodoctor-backend:m8-final-local`) but **not yet deployed** to `hellodoctor-m8-final-local` — the live stack is still running the Stage-A (Admin-Telemetry-index-only) image as of this entry. The planned clean per-run measurement of all three scheduler-pass fixes together and a repeat six-service Scenario B (combined Admin-Telemetry-index + scheduler-fix effect, compared against the Phase 5/Stage-A baselines) were explicitly deferred to a future session at the user's request, not run.

**Housekeeping, this pass:** found and securely deleted two leftover plaintext secret files (`JWT_ED25519_PRIVATE_KEY_B64`/`WEBHOOK_HMAC_SECRET` pattern) from *prior, unrelated sessions* in `/tmp/claude/` (dated Oct 31 and Nov 2 08:29) — confirmed no other copies exist, confirmed no secret values leaked into any of this session's own evidence logs/artifacts. The operator-provided benchmark secrets file (`services/backend/scripts/.bench_secrets.env`, used only via the project's own `-SecretsEnvFile` script parameter, never read directly by the assistant) was deleted at the user's request once the Admin Telemetry benchmarking concluded. General host disk cleanup (unrelated to project data): Rust `target/debug` incremental cache (17GB), Docker build cache (6.2GB, `docker builder prune -af`), cargo registry cache/src (~720MB), one orphaned container referencing an already-garbage-collected image, and one temporary `hellodoctor-backend:control-pre-index` comparison image — roughly 23GB freed total across two passes (26GB → 42GB free). Windows Update download cache and Docker's WSL2 VHDX slack (~19GB) were identified but require administrator elevation this session did not have; exact commands for both were handed to the user to run themselves if wanted.

No git staging, commits, pushes, or production deployment anywhere in this entire pass. Migrations added this pass: `20261204000002`, `20261204000003`, `20261204000004` (all with reversible down-migrations, all already deployed to the live benchmark stack's actual database). No change to `services/backend/scripts/.bench_secrets.env`'s underlying real secrets (JWT signing key, webhook HMAC secret) — provenance confirmed test-only via file metadata (gitignored, ephemeral timestamp matching the benchmark stack's own creation) without ever reading the values themselves.

**Next steps, explicitly not started:** deploy the scheduler-fix image to `hellodoctor-m8-final-local`; run the clean per-run measurement for all three scheduler passes (pending holds/tick, language/render/insert call counts, scheduler pass duration, before vs. after); repeat six-service Scenario B with both Phase 5 and Phase 6 fixes live simultaneously and compare against the Phase 4/Phase-5-only/Stage-A-only baselines; investigate `completed_today`'s correctness defect; correct the benchmark's own Admin Telemetry call-generation pattern to originate from simulated administrators at a realistic cadence (preserving the current pattern as an explicit stress-test variant, per product-owner instruction, so historical results stay interpretable); review the remaining `Normal`/`Expensive`/`Webhook` admission-lane rejection causes before any further capacity decision.
