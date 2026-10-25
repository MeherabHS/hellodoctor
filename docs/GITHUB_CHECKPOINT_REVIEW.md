# Git checkpoint awaiting user review

No staging, commits or push are authorized until the user completes the requested review. This document proposes the checkpoint; it does not perform it.

## Proposed scope and destination

- Current branch: `main`; proposed destination: `origin/main` at `https://github.com/MeherabHS/hellodoctor.git`. No force push.
- Cached remote comparison: five local commits ahead, zero behind. The remote has not been freshly fetched, so current remote synchronization is unverified.
- Proposed checkpoint: all existing pending development listed below, plus this review document, preserving existing deletions. This includes older legitimate work outside Milestones 6D?6F; review that broader scope before approval.
- Proposed commit title: `feat: checkpoint notification platform and educational article quizzes through milestone 6F`.
- After approval: refresh remote state, stage the reviewed scope, inspect the staged diff, create the checkpoint commit, then push if the remote permits a normal fast-forward. Stop and report conflicts rather than overwriting remote work.

## Existing commits that would also be pushed

```text
9dc0210 fix: cross-codebase bug-hunt findings (telemetry, double-tap, statement disclosure, CI)
e517028 docs: backfill audit evidence, FCM setup guide, and repo hygiene
4e1d04e feat(admin-web): grievance investigation UI wiring
f1c123f feat(mobile): FCM push notifications and consultation-recovery client wiring
29dde6f feat(backend): FCM push notifications, booking-hold cleanup, consultation recovery oversight
```

## Verification and limitations

- Existing recorded 6F results: backend integration 398 passed, 0 failed, 4 explicitly ignored benchmarks; Rust library 91 passed; Patient Flutter 326 passed. Rust format/Clippy/compilation and Flutter format/analyze passed.
- Focused backend results: Patient quiz 10, Admin quiz 9, CMS 14 passed. These are subsets of the full backend count.
- Admin Web historical baseline is 93 passed; it was unchanged and not rerun in 6F. The entire broader pending worktree has not been freshly revalidated during Git preparation.
- The earlier backend acquisition timeout remains documented; it did not recur in the single bounded 6F full run. See `MILESTONE_6F_REPORT.md`.
- Live cross-app Android/iOS acceptance remains unverified. Azure Blob integration and real cloud-upload acceptance remain intentionally deferred. Historical moderate tooling advisories remain outstanding.
- Initial `git diff --check` passed. No tests were rerun solely to produce this Git inventory.

## Credential and artifact review

- Live environment files, native Firebase configuration, signing keys, bearer-token exports and local scratch outputs are excluded by current ignore rules. No tracked native Firebase configuration was found.
- A limited local pattern scan of tracked and candidate text files found fixture/example credential URLs and explicitly documented public synthetic ECDSA test keys. No live credential was identified by those patterns. This is not a comprehensive history or binary secret audit. Matched values are omitted from this report.
- 188 existing tracked files also match ignore rules, predominantly historic scratch material. Ignoring a file does not remove it from Git. These have not been deleted or removed from the index.
- An existing tracked Android build report remains at `apps/mobile/android/build/reports/problems/problems-report.html`. It is unchanged and outside the proposed new diff. Historical artifact cleanup should be separately reviewed; no cleanup is bundled here.
- Existing tracked large files include the design source `Hello Doctor [B].fig` (25,005,368 bytes) and audit evidence. No binary/history migration is proposed.
- Review `services/backend/examples/reset_admin_password.rs` explicitly: it is an existing untracked development/test utility and has not been executed during preparation.

## Pending inventory

Initial inventory: 219 paths: 123 modified tracked, 2 deleted tracked, 94 untracked. This new review document adds one untracked path (220 total if no concurrent edits occur). All remain unstaged.

`M` = modified tracked; `D` = deleted tracked; `?` = new/untracked.

| Status | Path |
|---|---|
| M | `apps/admin-web/package-lock.json` |
| M | `apps/admin-web/package.json` |
| M | `apps/admin-web/src/app/layout.tsx` |
| M | `apps/admin-web/src/app/login/page.tsx` |
| M | `apps/admin-web/src/app/page.tsx` |
| M | `apps/admin-web/src/lib/api.ts` |
| M | `apps/admin-web/src/middleware.ts` |
| M | `apps/doctor-web/src/app/layout.tsx` |
| M | `apps/doctor-web/src/components/Auth.tsx` |
| M | `apps/doctor-web/src/components/Desktop.tsx` |
| M | `apps/doctor-web/src/components/DoctorApplication.tsx` |
| M | `apps/doctor-web/src/components/Queue.tsx` |
| M | `apps/doctor-web/src/components/Settings.tsx` |
| M | `apps/doctor-web/src/generated/templates.json` |
| M | `apps/doctor-web/src/lib/contracts.ts` |
| M | `apps/doctor-web/src/middleware.ts` |
| M | `apps/doctor-web/tests/consultation-actions.test.ts` |
| M | `apps/mobile/.gitignore` |
| M | `apps/mobile/android/app/build.gradle.kts` |
| M | `apps/mobile/android/app/src/main/AndroidManifest.xml` |
| M | `apps/mobile/android/app/src/main/kotlin/com/example/hellodoctor_mobile/MainActivity.kt` |
| M | `apps/mobile/android/app/src/main/res/xml/network_security_config.xml` |
| M | `apps/mobile/android/settings.gradle.kts` |
| M | `apps/mobile/ios/Runner/AppDelegate.swift` |
| M | `apps/mobile/ios/Runner/Info.plist` |
| M | `apps/mobile/lib/app.dart` |
| M | `apps/mobile/lib/core/network/api_client.dart` |
| M | `apps/mobile/lib/core/network/client_error_reporter.dart` |
| M | `apps/mobile/lib/core/notifications/background_sync_worker.dart` |
| M | `apps/mobile/lib/core/notifications/local_notification_service.dart` |
| M | `apps/mobile/lib/core/notifications/notification_ids.dart` |
| M | `apps/mobile/lib/core/notifications/push_notification_service.dart` |
| D | `apps/mobile/lib/core/notifications/schedule_notification_reconciler.dart` |
| M | `apps/mobile/lib/core/video/call_controller.dart` |
| M | `apps/mobile/lib/core/widgets/startup_error_page.dart` |
| M | `apps/mobile/lib/features/auth/auth_cubit.dart` |
| M | `apps/mobile/lib/features/doctor/doctor_models.dart` |
| M | `apps/mobile/lib/features/doctor/doctor_pages.dart` |
| M | `apps/mobile/lib/features/notifications/notification_cubit.dart` |
| M | `apps/mobile/lib/features/notifications/notification_models.dart` |
| M | `apps/mobile/lib/features/notifications/notification_repository.dart` |
| M | `apps/mobile/lib/features/settings/settings_pages.dart` |
| M | `apps/mobile/test/api_client_host_policy_test.dart` |
| M | `apps/mobile/test/background_sync_worker_test.dart` |
| M | `apps/mobile/test/consultation_actions_test.dart` |
| M | `apps/mobile/test/core/video/call_controller_test.dart` |
| M | `apps/mobile/test/notification_cubit_test.dart` |
| M | `apps/mobile/test/notification_repository_test.dart` |
| D | `apps/mobile/test/schedule_notification_reconciler_test.dart` |
| M | `apps/mobile_patient/.gitignore` |
| M | `apps/mobile_patient/android/app/build.gradle.kts` |
| M | `apps/mobile_patient/android/app/src/main/AndroidManifest.xml` |
| M | `apps/mobile_patient/android/app/src/main/kotlin/com/hellodoctor/patient/MainActivity.kt` |
| M | `apps/mobile_patient/android/app/src/main/res/xml/network_security_config.xml` |
| M | `apps/mobile_patient/android/settings.gradle.kts` |
| M | `apps/mobile_patient/ios/Runner/AppDelegate.swift` |
| M | `apps/mobile_patient/ios/Runner/Info.plist` |
| M | `apps/mobile_patient/lib/app.dart` |
| M | `apps/mobile_patient/lib/core/l10n/locale_controller.dart` |
| M | `apps/mobile_patient/lib/core/network/api_client.dart` |
| M | `apps/mobile_patient/lib/core/network/client_error_reporter.dart` |
| M | `apps/mobile_patient/lib/core/notifications/local_notification_service.dart` |
| M | `apps/mobile_patient/lib/core/notifications/notification_ids.dart` |
| M | `apps/mobile_patient/lib/core/notifications/push_notification_service.dart` |
| M | `apps/mobile_patient/lib/core/video/call_controller.dart` |
| M | `apps/mobile_patient/lib/core/widgets/startup_error_page.dart` |
| M | `apps/mobile_patient/lib/features/auth/auth_cubit.dart` |
| M | `apps/mobile_patient/lib/features/auth/auth_entry_page.dart` |
| M | `apps/mobile_patient/lib/features/notifications/notification_cubit.dart` |
| M | `apps/mobile_patient/lib/features/notifications/notification_models.dart` |
| M | `apps/mobile_patient/lib/features/notifications/notification_repository.dart` |
| M | `apps/mobile_patient/lib/features/patient/blogs/blog_detail_page.dart` |
| M | `apps/mobile_patient/lib/features/patient/blogs/blogs_models.dart` |
| M | `apps/mobile_patient/lib/features/patient/blogs/blogs_page.dart` |
| M | `apps/mobile_patient/lib/features/patient/blogs/blogs_repository.dart` |
| M | `apps/mobile_patient/lib/features/patient/chat/consultation_status_page.dart` |
| M | `apps/mobile_patient/lib/features/patient/chat/patient_chat_list_page.dart` |
| M | `apps/mobile_patient/lib/features/patient/profile/patient_profile_page.dart` |
| M | `apps/mobile_patient/lib/l10n/app_en.arb` |
| M | `apps/mobile_patient/lib/l10n/app_sw.arb` |
| M | `apps/mobile_patient/pubspec.lock` |
| M | `apps/mobile_patient/pubspec.yaml` |
| M | `apps/mobile_patient/test/app_boot_test.dart` |
| M | `apps/mobile_patient/test/consultation_booking_test.dart` |
| M | `apps/mobile_patient/test/core/video/call_controller_test.dart` |
| M | `apps/mobile_patient/test/locale_controller_test.dart` |
| M | `docs/CHECKPOINT.md` |
| M | `docs/FCM_SETUP.md` |
| M | `docs/PROGRESS.md` |
| M | `index.html` |
| M | `prototype/index.html` |
| M | `services/backend/.env.example` |
| M | `services/backend/Cargo.lock` |
| M | `services/backend/Cargo.toml` |
| M | `services/backend/src/abac.rs` |
| M | `services/backend/src/api/router.rs` |
| M | `services/backend/src/api/v1/admin_handlers.rs` |
| M | `services/backend/src/api/v1/article_handlers.rs` |
| M | `services/backend/src/api/v1/auth_handlers.rs` |
| M | `services/backend/src/api/v1/mod.rs` |
| M | `services/backend/src/api/v1/notification_handlers.rs` |
| M | `services/backend/src/api/v1/patient_handlers.rs` |
| M | `services/backend/src/api/v1/trace_handlers.rs` |
| M | `services/backend/src/config.rs` |
| M | `services/backend/src/domain/mod.rs` |
| M | `services/backend/src/email_delivery.rs` |
| M | `services/backend/src/migration_status.rs` |
| M | `services/backend/src/repository/mod.rs` |
| M | `services/backend/src/repository/notification_repo.rs` |
| M | `services/backend/src/repository/user_repo.rs` |
| M | `services/backend/src/services/appointment_service.rs` |
| M | `services/backend/src/services/auth_service.rs` |
| M | `services/backend/src/services/chat_service.rs` |
| M | `services/backend/src/services/consultation_presence_service.rs` |
| M | `services/backend/src/services/consultation_service.rs` |
| M | `services/backend/src/services/disbursement_service.rs` |
| M | `services/backend/src/services/grievance_service.rs` |
| M | `services/backend/src/services/mod.rs` |
| M | `services/backend/src/services/notification_scheduler.rs` |
| M | `services/backend/src/services/notification_service.rs` |
| M | `services/backend/src/services/payment_provider_verification.rs` |
| M | `services/backend/src/services/prescription_service.rs` |
| M | `services/backend/tests/api_tests.rs` |
| M | `services/backend/tests/cases/consultation_recovery.rs` |
| M | `services/backend/tests/cases/push_notification_delivery.rs` |
| ? | `apps/admin-web/src/app/api/article-media/[id]/route.ts` |
| ? | `apps/admin-web/src/components/ConsoleGuard.tsx` |
| ? | `apps/admin-web/src/components/blogs/ArticleEditor.tsx` |
| ? | `apps/admin-web/src/components/blogs/ArticlePreview.tsx` |
| ? | `apps/admin-web/src/components/blogs/BlogManager.tsx` |
| ? | `apps/admin-web/src/components/blogs/CategoryManager.tsx` |
| ? | `apps/admin-web/src/components/blogs/McqBuilder.tsx` |
| ? | `apps/admin-web/src/components/blogs/QuestionEditor.tsx` |
| ? | `apps/admin-web/src/components/blogs/QuizImport.tsx` |
| ? | `apps/admin-web/src/components/blogs/QuizPreview.tsx` |
| ? | `apps/admin-web/src/components/blogs/SafeMarkdown.tsx` |
| ? | `apps/admin-web/src/components/blogs/blogs.css` |
| ? | `apps/admin-web/src/components/campaigns/CampaignManager.tsx` |
| ? | `apps/admin-web/src/components/campaigns/CampaignNotificationPreview.tsx` |
| ? | `apps/admin-web/src/lib/blogApi.ts` |
| ? | `apps/admin-web/src/lib/blogContent.ts` |
| ? | `apps/admin-web/src/lib/blogErrors.ts` |
| ? | `apps/admin-web/src/lib/campaignValidation.ts` |
| ? | `apps/admin-web/src/lib/notificationPreviewText.ts` |
| ? | `apps/admin-web/src/lib/quizApi.ts` |
| ? | `apps/admin-web/src/lib/quizContent.ts` |
| ? | `apps/admin-web/src/lib/sideDoorGuard.ts` |
| ? | `apps/admin-web/tests/blog-content.test.ts` |
| ? | `apps/admin-web/tests/blog-errors.test.ts` |
| ? | `apps/admin-web/tests/blog-preview.test.tsx` |
| ? | `apps/admin-web/tests/campaign-preview-text.test.ts` |
| ? | `apps/admin-web/tests/campaign-validation.test.ts` |
| ? | `apps/admin-web/tests/quiz-components.test.tsx` |
| ? | `apps/admin-web/tests/quiz-content.test.ts` |
| ? | `apps/admin-web/tests/side-door-guard.test.ts` |
| ? | `apps/doctor-web/src/components/ConsoleGuard.tsx` |
| ? | `apps/doctor-web/src/lib/sideDoorGuard.ts` |
| ? | `apps/doctor-web/tests/side-door-guard.test.ts` |
| ? | `apps/mobile/android/app/src/debug/res/xml/network_security_config.xml` |
| ? | `apps/mobile/lib/features/settings/notification_settings_page.dart` |
| ? | `apps/mobile/test/push_notification_service_test.dart` |
| ? | `apps/mobile_patient/lib/features/notifications/notification_settings_page.dart` |
| ? | `apps/mobile_patient/lib/features/patient/blogs/article_content.dart` |
| ? | `apps/mobile_patient/lib/features/patient/blogs/article_quiz_section.dart` |
| ? | `apps/mobile_patient/lib/features/patient/blogs/quiz_controller.dart` |
| ? | `apps/mobile_patient/lib/features/patient/blogs/quiz_models.dart` |
| ? | `apps/mobile_patient/lib/features/patient/blogs/quiz_repository.dart` |
| ? | `apps/mobile_patient/lib/features/patient/blogs/quiz_widgets.dart` |
| ? | `apps/mobile_patient/test/admin_articles_test.dart` |
| ? | `apps/mobile_patient/test/notification_repository_test.dart` |
| ? | `apps/mobile_patient/test/patient_quiz_test.dart` |
| ? | `apps/mobile_patient/test/push_notification_service_test.dart` |
| ? | `docs/AZURE_BLOB_INTEGRATION_PLAN.md` |
| ? | `docs/MILESTONE_6D_REPORT.md` |
| ? | `docs/MILESTONE_6D_STABILIZATION_REPORT.md` |
| ? | `docs/MILESTONE_6E_REPORT.md` |
| ? | `docs/MILESTONE_6F_REPORT.md` |
| ? | `services/backend/examples/reset_admin_password.rs` |
| ? | `services/backend/migrations/20261119000001_payment_worker_cancel_t15_reminders.down.sql` |
| ? | `services/backend/migrations/20261119000001_payment_worker_cancel_t15_reminders.up.sql` |
| ? | `services/backend/migrations/20261120000001_milestone1_transactional_notifications.down.sql` |
| ? | `services/backend/migrations/20261120000001_milestone1_transactional_notifications.up.sql` |
| ? | `services/backend/migrations/20261121000001_milestone2_consultation_lifecycle_notifications.down.sql` |
| ? | `services/backend/migrations/20261121000001_milestone2_consultation_lifecycle_notifications.up.sql` |
| ? | `services/backend/migrations/20261122000001_milestone3_localization_preferences.down.sql` |
| ? | `services/backend/migrations/20261122000001_milestone3_localization_preferences.up.sql` |
| ? | `services/backend/migrations/20261124000001_milestone4_5_engagement_and_holidays.down.sql` |
| ? | `services/backend/migrations/20261124000001_milestone4_5_engagement_and_holidays.up.sql` |
| ? | `services/backend/migrations/20261124000002_rebrand_afyamkononi.down.sql` |
| ? | `services/backend/migrations/20261124000002_rebrand_afyamkononi.up.sql` |
| ? | `services/backend/migrations/20261125000001_admin_campaigns.down.sql` |
| ? | `services/backend/migrations/20261125000001_admin_campaigns.up.sql` |
| ? | `services/backend/migrations/20261126000001_campaign_status_new_values.down.sql` |
| ? | `services/backend/migrations/20261126000001_campaign_status_new_values.up.sql` |
| ? | `services/backend/migrations/20261126000002_campaign_scheduling_and_delivery.down.sql` |
| ? | `services/backend/migrations/20261126000002_campaign_scheduling_and_delivery.up.sql` |
| ? | `services/backend/migrations/20261127000001_admin_articles.down.sql` |
| ? | `services/backend/migrations/20261127000001_admin_articles.up.sql` |
| ? | `services/backend/migrations/20261128000001_article_quizzes.down.sql` |
| ? | `services/backend/migrations/20261128000001_article_quizzes.up.sql` |
| ? | `services/backend/migrations/20261129000001_patient_article_quizzes.down.sql` |
| ? | `services/backend/migrations/20261129000001_patient_article_quizzes.up.sql` |
| ? | `services/backend/src/api/v1/admin_article_handlers.rs` |
| ? | `services/backend/src/api/v1/admin_quiz_handlers.rs` |
| ? | `services/backend/src/api/v1/campaign_handlers.rs` |
| ? | `services/backend/src/api/v1/patient_quiz_handlers.rs` |
| ? | `services/backend/src/domain/admin_articles.rs` |
| ? | `services/backend/src/domain/article_quizzes.rs` |
| ? | `services/backend/src/domain/campaigns.rs` |
| ? | `services/backend/src/repository/admin_article_repo.rs` |
| ? | `services/backend/src/repository/article_quiz_repo.rs` |
| ? | `services/backend/src/repository/campaign_repo.rs` |
| ? | `services/backend/src/services/article_media_service.rs` |
| ? | `services/backend/tests/cases/admin_articles.rs` |
| ? | `services/backend/tests/cases/admin_campaigns.rs` |
| ? | `services/backend/tests/cases/admin_quizzes.rs` |
| ? | `services/backend/tests/cases/engagement_and_holiday_notifications.rs` |
| ? | `services/backend/tests/cases/notification_localization_preferences.rs` |
| ? | `services/backend/tests/cases/patient_quizzes.rs` |
| ? | `docs/GITHUB_CHECKPOINT_REVIEW.md` |

## Review decision

The user requested review before staging, committing or pushing. Await explicit approval of the scope and destination above. No Git index changes or remote writes have been made.
