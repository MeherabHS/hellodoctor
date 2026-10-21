# Lifecycle milestone acceptance record

Status: AUTOMATED GATES PASSED; LIVE WEB/API ACCEPTANCE PASSED; PHYSICAL DEVICE PENDING. This record covers the current consultation trace/Queue/history/Admin milestone, not earlier APK acceptance.

## Observed automated gates

| Gate | Frozen prior observed count | Current result |
| --- | ---: | --- |
| Rust library | 53 | 53 passed, 0 failed, exit 0 |
| Backend integration | 221 | 235 passed, 0 failed/ignored/filtered, exit 0 (1872.30 seconds) |
| Patient Flutter | 195 | 205 passed, exit 0; analyze/format passed |
| Doctor Flutter | 197 | 208 passed, exit 0; analyze/format passed |
| Doctor Web | 50 | 53 passed, 0 failed/skip, exit 0; lint/typecheck/build passed |
| Admin Web | 13 | 16 passed, 0 failed/skip, exit 0; lint/typecheck/build passed |

Rust format and offline Clippy all-targets with warnings denied exited 0. Admin lint exited 0 with 10 existing warnings, not a warning-free claim. Current development ARM64 APKs are release-built and signature-verified; their hashes and build configuration are in `release_artifacts.json`. They have not been installed on a phone for this milestone.


The frozen baseline is from last observed complete correction gates, not a freshly rerun pre-edit baseline. Source/test snapshots are in `audit_evidence/lifecycle_milestone/baseline.json`. An earlier unfiltered integration run passed 235/235, followed by four additional production boundary corrections. All four affected cases then passed. The final full rerun passed all 235 tests on the unchanged recorded backend source manifest; the previous full pass is retained as historical evidence.

## Test integrity

No existing test files or test functions were removed. No skip/ignore markers or failure-swallowing runners were added. Exact intentional assertion replacements, approved contract reasons and full diffs are retained in `test_integrity_audit.json` and `test_diffs/`. The audit documents 23 exact assertion replacements across approved canonical ADMIN authorization, retained own-encounter access, today's Queue/separate History and the exact payment CTA. The original broader private-profile cutoff and foreign-encounter denials remain. Financial, BOLA, database postcondition and concurrency checks are preserved.

Failed intermediate attempts are not represented as passes: History SQL initially referenced an unsupported enum and was corrected in production; new fixtures needed valid reference length/non-overlapping slots and transport peer metadata; the compose-route test needed an exact AppBar locator. Existing short-delay client diagnostic tests had transient failures under concurrent compilation and later passed unchanged. Added 320dp shell checks found real Home/auth overflows, which were fixed in presentation code.

## Physical and live acceptance

ADB currently lists no physical phone. No emulator was installed/launched. This milestone has no new physical-device screenshots, APK installation, on-phone Patient/Doctor cross-app send/receive, keyboard, bottom-system-inset or local notification acceptance. Desktop Web/API acceptance is separately recorded below. Earlier screenshots and APK evidence are not reused as current verification.

Required next device run: login, Home/discovery/profile, only future slots, payment-unavailable truth, current/today Queue, future Schedule, separate History, authoritative Chat send/reply/reopen, expired read-only history, paperclip/Rx, Notification Centers, Q&A, records, wallet/settings, article shortcut, keyboard and 0/gesture/three-button safe area. Admin grievance must visibly expose every recorded tracked event and preserve the hold. No video/media scenario is included.

REAL_EXTERNAL_PAYMENT_PROVIDER_VERIFIED = NO
REAL_AZURE_BLOB_VERIFIED = NO
VIDEO_AGORA_IMPLEMENTATION = DEFERRED
RTC_MEDIA_VERIFIED = NO
PHYSICAL_DEVICE_EXISTING_FEATURES_ACCEPTANCE = PENDING

## Acceptance semantics

`AUTOMATED_VERIFIED` below means observed regression evidence on current source; it does not mean physical-device acceptance, external-provider acceptance, or a whole-system production certification. The full backend and live Web/API gates passed; the detailed results are in the gate record. No test runner was modified to hide a failure.

| Required area | Implementation and automated evidence | Physical/live limitation |
| --- | --- | --- |
| Living trace, DB time, append-only, 5-day default, bounded purge | Implemented; real application-role/RLS tests exercise append-only and unheld expiry | Live deployment readiness is checked separately |
| Grievance preservation | Transactional hold, existing-grievance backfill, cleanup race lock and preserved clinical records tested | Held evidence remains until separately approved release policy |
| First Patient message, Doctor Chat open/reply/read acknowledgement | Message and bounded authenticated acknowledgement records with account/session/request linkage tested | Acknowledgement cannot prove human comprehension |
| Attachment/Rx retrieval evidence | Authorized backend byte retrieval traced; revoked/missing-session grants and unrelated users denied | No Azure/provider download claim |
| Latest 50 / 200-message pagination | Tuple cursor, newer catch-up, concurrent older-page merge and microsecond ordering tested | Physical long transcript pending |
| Idempotency | Sender/conversation/key serializes one logical message and notification, including text/image cross-type conflicts | No text-only deduplication |
| Exact Chat lock / own retained history | DB wall clock at INSERT, delayed transaction crossing expiry, own-encounter reads and new-upload denial tested | Device read-only screenshots pending |
| Same Patient / multiple Doctors | Own encounter allowed, other Doctor Chat/doc/Rx denied; broader private-profile cutoff remains intact | Admin is individually audited privileged exception |
| Queue / Schedule / History | Current and remaining today only, active first; future Schedule and separate retained History tested | Phone resume/notification reconciliation pending |
| Patient availability | Full future instant filter, direct old-slot denial, Nairobi boundary and crossing row-lock wait tested | Real Web/API publication sync separately recorded |
| Canonical ADMIN | All six legacy roles, revoked sessions, two individual accounts, rollback/reapply and specific-account audit tested | Live canonical login separately recorded |
| Grievance investigation | Authoritative facts, every safe trace/audit field, references/holds, actual reschedule/adjudication records; no hardcoded clinical verdict | Live UI screenshot separately recorded |
| Payment CTA | Exact `Proceed to payment method`, pending/failed/unknown handling and no fake provider success tested | Real external provider not connected/verified |
| Chat UI | Persisted sender controls own green/right and incoming white/left; paperclip reuses bounded private upload pipeline | Real keyboard/Android screenshots pending |
| Article entry | Accessible home compose shortcut routes to existing editor; existing ownership lifecycle retained | Phone screenshot pending |
| System insets | Patient and Doctor geometry checks at bottom insets 0/24/34/48; narrow Home/auth layout defects fixed | Three-button/gesture/keyboard device acceptance pending |

The final audit contains **23 intentional exact assertion replacements** for the explicitly approved new role/Queue/history/payment contracts. No existing tests were removed; unchanged security/business/concurrency assertions remain. Full old/new assertion text, test names and reasons are in [TEST_INTEGRITY_AUDIT.md](audit_evidence/lifecycle_milestone/TEST_INTEGRITY_AUDIT.md).

The separate legacy Admin finance/Patient Vault prototype content is outside this grievance-screen correction. It is not evidence of a production-complete financial or clinical administration workflow. The existing signed development payment callback is suitable for synthetic activation acceptance only, never a real provider-payment claim.

## Final observed test inventory

BASELINE_TEST_COUNTS = Rust 53; backend integration 221; Patient Flutter 195; Doctor Flutter 197; Doctor Web 50; Admin Web 13. Baseline total: 729.

FINAL_TEST_COUNTS = Rust 53; backend integration 235; Patient Flutter 205; Doctor Flutter 208; Doctor Web 53; Admin Web 16. Final total: 770. NEW_TESTS_ADDED = 41.

EXISTING_TESTS_DELETED = 0

EXISTING_ASSERTIONS_DELETED = 0 (standalone deletions; approved replacements recorded separately)

EXISTING_ASSERTIONS_DELETED_WITHOUT_APPROVED_REPLACEMENT = 0

TEST_ASSERTIONS_WEAKENED = 0

TEST_SKIP_OR_IGNORE_ADDED = 0

TEST_RUNNER_FAILURES_SWALLOWED = 0

EXISTING_ASSERTIONS_INTENTIONALLY_CHANGED = 23 (each old/new assertion and approved reason recorded in the audit).

All final suites have zero failed/ignored/skipped cases. The full backend run has zero filtered cases; filtered focused runs are identified as supplemental evidence only. Native successful exits were observed, not inferred from a truncated log. The backend source manifest remained unchanged throughout the final serialized gate.

An additional live rerun correctly stopped at the unchanged five-per-IP/15-minute login guard (HTTP 429). Its nonzero exit and failure record are retained in `live_web_rate_limit_attempt.*`; no Redis limiter state or threshold was altered. After natural expiry, the final browser acceptance exited 0 with all checks passed, including final Admin labels and both real notification records. New Queue test development initially needed the correct synchronous Flutter binding API and real inactive/hidden lifecycle transitions; no existing test/assertion was changed.

## Live Web/API acceptance

Observed native exit 0 in `live_web_acceptance.log`, using existing Edge and production Web builds. The backend readiness guard confirmed source/binary/database migration 20261106000006. The synthetic scenario published a new Doctor slot via API, fetched it through Patient availability, booked CHAT at the authoritative price, observed provider-unavailable without fake success, then activated through the existing signed DEVELOPMENT callback.

The Patient API message appeared in Doctor Web; Doctor Web replied through the actual BFF, and Patient API loaded that same reply. Reload retained the same conversation. Each direction produced one privacy-safe notification record. Browser assertions verified persisted sender ownership and exact green/white across all three Web themes. Admin logged in as canonical ADMIN, opened the actual grievance and displayed every returned trace/audit event and every safe event field, with account/session/request attribution and active hold. No page errors were observed.

Evidence screenshots:

- [Doctor authoritative Chat](audit_evidence/lifecycle_milestone/live_doctor_chat.png)
- [Admin investigation timeline and preservation hold](audit_evidence/lifecycle_milestone/live_admin_grievance_timeline.png)
- [Admin authoritative consultation facts](audit_evidence/lifecycle_milestone/live_admin_grievance_facts.png)
- [Admin record references](audit_evidence/lifecycle_milestone/live_admin_grievance_records.png)

These are desktop Web screenshots, not physical Android screenshots. The successful synthetic callback is not external-payment-provider verification. No RTC event or media participation is simulated.

## Required milestone status flags

YES below means implementation plus observed automated evidence, supplemented by the described real local Web/API scenario. It is not an Android physical-device claim. Full feature-by-feature physical acceptance remains PENDING.

### Trace

| Status | Result |
| --- | --- |
| CONSULTATION_LIVING_TRACE_IMPLEMENTED | YES (automated) |
| CONSULTATION_TRACE_SERVER_TIME_VERIFIED | YES (automated) |
| CONSULTATION_TRACE_APPEND_ONLY_VERIFIED | YES (automated) |
| CONSULTATION_TRACE_5_DAY_RETENTION_CONFIGURED | YES (automated) |
| CONSULTATION_TRACE_CLEANUP_VERIFIED | YES (automated) |
| GRIEVANCE_TRACE_PRESERVATION_HOLD_VERIFIED | YES (automated) |
| CHAT_FIRST_PATIENT_MESSAGE_TRACEABLE | YES (automated) |
| DOCTOR_FIRST_CHAT_OPEN_TRACEABLE | YES (automated) |
| DOCTOR_FIRST_REPLY_TRACEABLE | YES (automated) |
| CHAT_READ_ACK_TRACEABLE | YES (automated) |
| ATTACHMENT_VIEW_TRACEABLE | YES (automated) |
| RX_VIEW_TRACEABLE | YES (automated) |

### Chat and history

| Status | Result |
| --- | --- |
| CHAT_LATEST_MESSAGE_PAGINATION_VERIFIED | YES (automated) |
| CHAT_OVER_50_MESSAGES_VERIFIED | YES (automated) |
| CHAT_IDEMPOTENT_RETRY_VERIFIED | YES (automated) |
| CHAT_24H_DB_WALL_TIME_LOCK_VERIFIED | YES (automated) |
| CHAT_HISTORY_READABLE_AFTER_LOCK | YES (automated) |
| CHAT_ATTACHMENT_READABLE_AFTER_LOCK | YES (automated) |
| RX_HISTORY_READABLE_AFTER_LOCK | YES (automated) |
| DOCTOR_OWN_HISTORICAL_CONSULTATION_READABLE | YES (automated) |
| UNRELATED_DOCTOR_HISTORICAL_ACCESS_DENIED | YES (automated) |
| SAME_PATIENT_MULTI_DOCTOR_BOLA_VERIFIED | YES (automated) |
| IDOR_OBJECT_SUBSTITUTION_DENIED | YES (automated) |

### Queue and slots

| Status | Result |
| --- | --- |
| DOCTOR_QUEUE_CURRENT_AND_TODAY_ONLY_VERIFIED | YES (automated) |
| DOCTOR_FUTURE_APPOINTMENTS_IN_SCHEDULE_VERIFIED | YES (automated) |
| DOCTOR_HISTORY_SEPARATED_FROM_QUEUE | YES (automated) |
| DOCTOR_QUEUE_RESUME_REFRESH_VERIFIED | YES (automated) |
| PATIENT_YESTERDAY_SLOTS_HIDDEN | YES (automated) |
| PATIENT_EARLIER_TODAY_SLOTS_HIDDEN | YES (automated) |
| PATIENT_FUTURE_TODAY_SLOT_VISIBLE | YES (automated) |
| PATIENT_FUTURE_DATE_SLOT_VISIBLE | YES (automated) |
| DIRECT_PAST_SLOT_BOOKING_REJECTED | YES (automated) |
| LATEST_DOCTOR_SLOT_SYNC_VERIFIED | YES (automated) |

### Admin

| Status | Result |
| --- | --- |
| SINGLE_HUMAN_ADMIN_ROLE_IMPLEMENTED | YES (automated) |
| MULTIPLE_ADMIN_ACCOUNTS_VERIFIED | YES (automated) |
| ADMIN_SPECIFIC_ACCOUNT_ATTRIBUTION_VERIFIED | YES (automated) |
| ADMIN_SENSITIVE_READ_AUDIT_VERIFIED | YES (automated) |
| LEGACY_ADMIN_ROLE_MIGRATION_VERIFIED | YES (automated) |
| ADMIN_GRIEVANCE_TIMELINE_VERIFIED | YES (automated) |
| ADMIN_TIMELINE_HARDCODED_FACTS_REMOVED | YES (automated) |

### UX

| Status | Result |
| --- | --- |
| PATIENT_PROCEED_TO_PAYMENT_METHOD_UX_VERIFIED | YES (automated) |
| NO_FAKE_PAYMENT_SUCCESS_VERIFIED | YES (automated) |
| DOCTOR_OWN_MESSAGE_GREEN_VERIFIED | YES (automated) |
| PATIENT_MESSAGE_WHITE_IN_DOCTOR_CHAT_VERIFIED | YES (automated) |
| DOCTOR_CHAT_ATTACHMENT_ACTION_VERIFIED | YES (automated) |
| DOCTOR_ARTICLE_COMPOSE_SHORTCUT_VERIFIED | YES (automated) |
| PATIENT_BOTTOM_NAV_SYSTEM_INSET_VERIFIED | YES (automated) |
| DOCTOR_BOTTOM_NAV_SYSTEM_INSET_VERIFIED | YES (automated) |



## Subsequent API traffic correction (2026-10-16)

See [API_TRAFFIC_CACHE_AND_POLLING_VERIFICATION.md](API_TRAFFIC_CACHE_AND_POLLING_VERIFICATION.md) for route/background polling, in-flight coalescing, bounded pagination, selected-day Schedule reads and the separate post-milestone test/device evidence. Earlier counts and screenshots above remain historical evidence for their recorded revision. No new physical-device claim is implied.
