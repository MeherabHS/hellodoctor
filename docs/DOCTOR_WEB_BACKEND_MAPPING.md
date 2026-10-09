> Current canonical ownership / Doctor Q&A correction (2026-10-08): [identity enforcement, audited Compliance trace and Q&A rendering evidence](USER_ISOLATION_AND_QNA.md); [all-table ownership inventory](USER_OWNERSHIP_INVENTORY.md). Migration 20261026000001 adds the database guards. Q&A real-device inspection remains pending until a device is connected; widget/golden renders are separate evidence. Earlier verification counts below are historical.

> Current public-exposure/session hardening and direct Doctor Articles (2026-10-08): [security controls and deployment requirements](PUBLIC_EXPOSURE_HARDENING.md), [Doctor Articles without Admin review](DOCTOR_ARTICLES.md). Earlier dated results below are historical snapshots. Slot approvals are explicitly excluded by product policy.

> Current Kenya professional-registration contract (2026-10-08): [explicit PENDING / VERIFIED / REJECTED model, KMPDC/KE, generic Admin routes, migration and evidence](KENYA_PROFESSIONAL_VERIFICATION.md). Earlier dated audit/results sections are historical snapshots and do not describe the current review model.

> Latest functional fixes and user-requested removals: [follow-up report](DOCTOR_WEB_FUNCTIONAL_FOLLOWUP.md). Older results below retain their original scope.

# Doctor Web current backend mapping

Ground truth: branch `main`, commit `e3739ae555f198c537dfa7f505fe506b0cd62df2` with substantial pre-existing uncommitted changes. Doctor Web did not exist at preflight. One `apps/doctor-web` application follows the Admin Next/React convention with a server-side HttpOnly BFF. Next is patched to 15.5.24 and React remains 18.3.1. Flutter source is unchanged. A reproducible shared no-prescription completion defect required the narrow Rust service correction recorded below.

All endpoint paths below have prefix `/api/v1`. Rust router and current handlers, rather than historic documents, were read. Responses use `{success,data}`; errors use `{success:false,error:{code,message,details},meta:{request_id,timestamp}}`. Browser consumes a sanitized BFF response; tokens never leave the BFF. Clinical responses use `no-store`, ephemeral state, and fresh authorization.

| Screen | Action | Method / endpoint | Request DTO | Response DTO | Auth / ABAC | RLS / ownership | Errors / Web state | Mobile reference |
|---|---|---|---|---|---|---|---|---|
| Login | Password login | POST auth/doctor/login | email,password | session_token,expires_in,debug_otp_code (development only) | public; Redis abuse control | Doctor identity only | 401/429/503; MFA or error | auth_cubit.login |
| MFA | Verify | POST auth/doctor/verify-otp | session_token,otp_code | access_token,refresh_token | public challenge verification | challenge-scoped Doctor | 401/429/503; signed in/error | auth_cubit.verify |
| Registration | Submit | POST auth/doctor/register | full_name,license_number,phone_number,email,password,primary_specialty,experience_years,current_hospital,qualifications[],residential_address | doctor_id,status=PENDING_VERIFICATION | public; deny_unknown_fields | server sets DOCTOR and unverified; atomic onboarding | 409/422/429/503; pending/error | auth_cubit.registerDoctor |
| Session | Refresh | POST auth/refresh | refresh_token (server only) | access_token,refresh_token | active refresh session | token session revocation | 401; clear state | api_client.refreshSession |
| Session | Logout | POST auth/logout | refresh_token (server only) | success | refresh session | server revocation | cookie clear and tab broadcast even on failure | auth_cubit.logout |
| Profile | Read/edit | GET/PATCH doctor/profile | PATCH: full_name only | DoctorProfile / updated | DoctorRead / DoctorManageProfile | own ctx doctor_profile_id | 401/403/422; pending/verified/error | repository.profile/updateProfile |
| Duty | Read/set | GET/PUT doctor/duty | is_on_duty | is_on_duty | DoctorRead / DoctorManageDuty | own profile; verification for duty mutation | 403; server confirmation before display | repository.duty/setDuty |
| Queue | List | GET doctor/appointments | limit,offset,status? | id,appointment_number,patient_id,patient_display_name,modality,status,doctor_lifecycle,start_time,end_time,history_access_from,history_access_until,history_access_active,clinical_outcome | DoctorRead; verified profile | own doctor; begin_authed | loading/empty/error/refresh/paging | repository.queue |
| Schedule | List/create | GET/POST doctor/slots | POST start_time,end_time UTC | id,status,start_time,end_time,patient_display_name,appointment_id | DoctorRead / DoctorManageOwnSchedule | own Doctor; create locks profile, checks overlap and future time | 409/422; no optimistic success | repository.slots/createSlot |
| Schedule | Cancel | DELETE doctor/slots/:slot_id | none | slot_id,status | DoctorManageOwnSchedule | own available slots only; atomic WHERE | 409; refresh race; booked locked | repository.cancelSlot |
| Chat list | List | GET doctor/chats | limit,offset | id,appointment_id,patient_display_name,is_locked,expires_at,last_message_at | DoctorRead; verified profile | own conversations | empty/error; no invented online/triage tags | repository.chats |
| Chat thread | Read/send | GET/POST chat/:conversation_id/messages | POST content | Message[] / Message | AnyAuthenticated; ChatService participants | RLS participant; server expiry/lock | 403/409/422; read-only after expiry | repository.messages/sendMessage |
| Summary | Read | GET doctor/consultations/:appointment_id/summary | none | appointment_id,patient_id,status,chief_complaint,clinical_notes,clinical_outcome,completed_at,consultation_id,started_at,ended_at | DoctorRead; verified profile | doctor_history_context_allowed, own appointment | 403 outside window; fresh fetch only | repository.summary |
| History | List/detail | GET doctor/appointments/:current_appointment_id/patient-history[/:historical_consultation_id] | limit,offset | history JSON[] / episode JSON | DoctorRead | current appointment anchor; SQL security functions; audit | 404 absent/expired/foreign; clear old data | repository.history/historyDetail |
| Document viewer | Authorize | GET appointments/:appointment_id/documents/:document_id | current_appointment_id | download_url,expires_in=300 | AnyAuthenticated + Doctor context | doctor_authorized_document + CLEAN metadata | 404/409/503; never persist URL | repository.documentUrl |
| Document viewer | Download | GET documents/download | token (server grant only) | bytes,mime | short-lived Redis grant | ownership and access window rechecked on download | 404/503; no-store; BFF keeps grant out of browser URL | backend prescription_handlers.download_document |
| Smart Rx | Upload each page | POST consultations/:appointment_id/prescription | base64_photo,doctor_notes?,mime_type? | Prescription | UploadDoctorPrescription | own consultation, validation, storage, max five | 409/413/422/503; LOCAL/UPLOADING/UPLOADED/FAILED/RETRY | repository.uploadPrescription |
| Completion | With Rx / refer / escalate | POST consultations/:appointment_id/complete | outcome=COMPLETED_WITH_RX/REFERRED/ESCALATED | appointment_id,status | CompleteConsultation | own encounter; exactly-once financial completion | 409/422; lock submit; retain idempotency key | repository.complete |
| Completion | Without Rx | POST consultations/:appointment_id/complete-no-rx | reason | existing Prescription DTO, unchanged | UploadDoctorPrescription | own encounter; existing clinical-document window; no-Rx record and authoritative completion in one transaction | 403 outside clinical window; 409/422 conflicts/validation; reason required | repository.completeNoRx |
| RTC | Token | POST consultations/:appointment_id/rtc-token | none | channel_name,token,uid,app_id,expires_in | AnyAuthenticated + consultation service | participant and time window | DEFERRED_EXTERNAL_INTEGRATION: no installed Web Agora SDK | agora_page.dart / repository.rtcToken |
| Q&A | List / answer | GET doctor/qna/questions; POST doctor/qna/questions/:question_id/answers | status=ALL/OPEN/ANSWERED,limit,offset; answer_text | public question DTO[] / question_id,status | DoctorQna; verified doctor | public Q&A; no clinical history grants; one answer constraint | 409 already answered; refresh; 422 | repository.questions/answer |
| Wallet | Summary / ledger | GET doctor/wallet/summary; GET doctor/wallet/ledger | limit,offset | server KES gross,fee,net,unpaid,paid fields / transaction DTO[] | ViewOwnWallet | own doctor through ctx and RLS | no browser financial arithmetic | repository.wallet/ledger |
| Statement | Month | GET doctor/wallet/statements | month YYYY-MM,limit,offset | status,amount_label,gross_earnings,platform_charge,adjustments,payable,payout_status,destination,entries,next_offset | ViewOwnWallet | own month-scoped financial snapshot | finalized only from server; estimated for IN_PROGRESS | repository.monthlyStatement |
| Payout | Destinations | GET doctor/wallet/payout-methods | none | provider_code,method_type,currency,masked_destination,binding_status,verification_status,payout_capable,is_primary | ViewOwnWallet | own destinations | display simulation honestly; real binding deferred | repository.payoutMethod |

## Proven gaps / conflicts

- `GENUINE_BACKEND_GAP_DOCTOR_NOTIFICATIONS`: current router has no Doctor notification list/read/unread/mark-read contract. Prototype bell remains an honest unavailable surface, no fabricated badge or notifications.
- `GENUINE_BACKEND_GAP_DOCTOR_ARTICLES`: current router has no Doctor article CRUD/publication workflow. Secondary settings entry can explain availability; no static fake content.
- `GENUINE_BACKEND_GAP_DOCTOR_WEB_LAB_REPORTS`: current document authorization supports intake/Rx files, but no structured laboratory results, lab ordering or synthetic report contract. Preserve document controls and show supported authorized documents or truthful unavailable state.
- `GENUINE_BACKEND_GAP_DOCTOR_WEB_DASHBOARD_METRICS`: no server daily targets, averaged duration, safety alerts, adherence/upvote metrics, or mobile optical sync contract. Never preserve prototype claims as real data.
- `GENUINE_BACKEND_GAP_DOCTOR_WEB_PATIENT_DIRECTORY`: no arbitrary Doctor-accessible longitudinal patient directory. Display own appointment contexts; dossier always reauthorizes by appointment.
- `GENUINE_BACKEND_GAP_DOCTOR_WEB_CHAT_METADATA`: list response has no last-message content, urgent/follow-up/refill classification or online presence. Do not fetch every message thread just to invent previews; keep controls with availability explanation.
- `GENUINE_BACKEND_GAP_DOCTOR_WEB_FRONTEND_DIAGNOSTICS`: current system error architecture exposes Admin reads and consultation telemetry; no general frontend ingestion endpoint. Add sanitized local BFF diagnostics (category, release, route, request ID), document the ingestion gap, do not create a new business API.
- Queue current SQL is `ORDER BY s.start_time DESC`; it is not authoritative upcoming-first ordering. Web sorts the loaded owned DTOs by active/upcoming first and start ascending, retains all terminal statuses below. Pagination-wide earliest ordering requires reading all relevant pages; do not label the first descending page globally upcoming-first.
- `PROTOTYPE_BUSINESS_RULE_CONFLICT_RESOLVED`: Kenya/KES, KMPDC verification, provider authorization, Finance-scheduled disbursement, handwritten image Rx and honest RTC override historic merchant, self-withdrawal, fabricated optical sync/call/AI/prescription claims while preserving structure.

## Preflight

Available: Node 24.18.0, npm 11.16.0, Rust/Cargo 1.99.0, Flutter 3.47.6/Dart 3.13.5, Git 2.54.0, installed Edge, Python Playwright and Pillow. Missing: pnpm, yarn; neither required. Admin dependencies already installed. No Doctor Web test stack existed; use built-in Node test runner and existing Python Playwright with installed Edge. No browser download.

`NO_INSTALLATION_PERFORMED = YES`. Existing approved PostgreSQL/Redis containers were already running. Initial health returned 503 database unreachable; canonical ready/start tooling invoked before live verification. No migration, storage service or MinIO added.

## Reproducible shared-contract correction

Live browser testing returned success from `complete-no-rx`, but the controlled
appointment remained `CONFIRMED`. Current Flutter `completeNoRx` also calls only
this endpoint and reports server completion. The service previously inserted a
no-Rx decision without invoking the existing authoritative completion function.

`PrescriptionService::complete_without_prescription` now invokes the existing
`doctor_complete_consultation` SQL function in the same transaction as the
decision. `ConsultationService::complete_in_transaction` shares the existing
completion/error/audit path; it introduces no separate settlement logic.
Same-reason retries keep the response DTO compatible and do not credit twice.
The existing `doctor_history_context_allowed` rule protects access to the
decision; RLS, grants and history windows are not weakened. No migration or
Flutter source modification is required.

The meaningful API regression verifies before-T-15 denial without inserting a
decision, completion/status/session/outcome, the authoritative KES 640 earning,
same-reason retry with unchanged wallet values, and one decision row. The first
full API run exposed an early-fixture retry problem (52 passed, 1 failed).
After preserving the existing clinical authorization window, the targeted test
passes. The second full run passed 52 tests and exposed an older historical fixture
that completed before T-15; correcting its fixture timing makes the targeted
history regression pass. All 53 distinct tests have passed across these runs;
a single full run after the final test-only fixture correction is not claimed.
Canonical startup verifies source/binary/database at 20261019000001. The latest
real browser check confirms completion status and retry without double credit.

## Storage and external-provider truth

The actual unconfigured Rx storage response in this environment is HTTP 409,
`RESOURCE_CONFLICT`, "Document storage is not configured. Upload is unavailable."
The Web surfaces this response and retains individual failed pages for retry.
No successful persistence or literal server `STORAGE_UNAVAILABLE` code is
invented. `STORAGE_CONFIGURATION_REQUIRED` remains an environment limitation.
No MinIO or replacement storage has been introduced.


## Presentation correction - 2026-10-07

No Rust or Flutter files were edited by this correction. Flutter baseline hashes
are unchanged. Three notification/test Rust files changed separately in the
workspace; source-integrity-final.json records them without attributing them to
this UI work. Those changes were preserved.
All route/DTO/session/authorization mappings above remain. No Web-specific API,
migration, financial computation or alternative clinical authorization was added.
The earlier shared-contract and login recovery changes predate this correction.

Entry points changed only: queue is the Web Appointments surface; Rx/chat belong
to the Telehealth secondary flow; slots are reachable through calendar and
Appointments; earnings/statement/payout through Dashboard/profile. The Web EMR
panel now reads the same appointment-scoped history and existing HTTP chat
contracts, with contextual fetching and reauthorization. Profile duty awaits the
server and the test restores its original value.

Current real API regression and sanitized request outcomes are recorded in
`audit_evidence/doctor_web/correction/workflow-results.json`. Rx's storage failure
is the real 409 RESOURCE_CONFLICT, not a successful upload. Chat message creation
returns 201. Foreign summary/history requests remain denied. No live provider or
new completion mutation is claimed. See the correction report for exact scope.


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

| New Web action | Method / existing endpoint | Request | Response | Authorization / Web state |
|---|---|---|---|---|
| Inbox | GET /notifications?limit=25&offset=0 | limit, offset | Notification[] including notification_type/title/body/read_at | AuthContext user_id + own-inbox RLS; loading/error/empty/list |
| Unread badge | GET /notifications/unread-count | none | unread_count | Same ownership; no browser-generated notifications |
| Mark one read | POST /notifications/:id/read | no recipient/body | id, marked | Foreign id returns marked=false; UI awaits server |
| Mark all read | POST /notifications/read-all | none | marked_count | Current user's rows only |

No device-token API or arbitrary recipient parameter is exposed by the BFF.

## 2026-10-07 ? Dashboard / account / settings verified checkpoint

This targeted correction is complete before the dedicated Patient Chats routing follow-up. Build `lGgKwZC70Jn19ywa_XOwV`: production build PASS; typecheck, source lint, format checks and 11 unit tests PASS; 13 browser/API checks PASS with no JavaScript exceptions or unexpected 5xx. Installed Edge only. Evidence: `audit_evidence/doctor_web/dashboard-settings/results.json` and adjacent screenshots.

Dashboard: four supported summaries, one upcoming/active list preserving server order; duplicate sidebar queue and modality metrics removed. Instant Rx removed. Unsupported recent dispatch aggregation omitted (BACKEND_DATA_NOT_AVAILABLE). No developer-unavailable cards or emoji placeholders remain on Dashboard.

Account: compact keyboard-accessible popover, View profile opens the read-only Professional section, Settings is a full desktop page with Profile/Professional/Availability/Payout/Security sections. Display-name PATCH and server-confirmed duty tested then restored on the synthetic Doctor. Protected professional fields rejected by server on attempted mass assignment; only display name editable. Payout masking/empty state preserved; actual provider binding remains deferred. Wallet is a separate existing domain, not a profile action strip.

Prescription: camera removal/file picker/recoverable failed-page states separately PASS (7 browser checks). Current actual backend returns 409 RESOURCE_CONFLICT because private storage is unconfigured. STORAGE_CONFIGURATION_REQUIRED; successful server persistence has not passed.

Mobile and Rust source untouched by this pass; no installation or host/infrastructure change. Original whole-app acceptance remains DOCTOR_WEB_PARTIAL; this checkpoint is scoped to the user-authorized UX changes.

## 2026-10-07 ? Verified UX mapping changes

See the [current scoped mapping](DOCTOR_WEB_UX_CORRECTION_REPORT.md). Dashboard consumes the complete authorized appointment listing, wallet summary lifetime ledger values and status=OPEN Q&A (100+ label at the page cap). Unsupported Doctor-wide prescription-dispatch aggregate is omitted: BACKEND_DATA_NOT_AVAILABLE. No additional API or financial arithmetic added.

Profile PATCH permits full_name only and denies unknown fields. Professional values remain read-only; exact field/ownership matrix: [UX decisions](DOCTOR_WEB_UX_DECISIONS.md). Availability still uses own duty PUT; Payout still uses masked payout-methods GET. Unsupported password/MFA/session-management controls are not rendered.

GET doctor/chats provides identity, appointment_id, expiry, lock and last_message_at. It does not provide authoritative unread count, category or last-message preview; those UI fields/filters are omitted. Message POST has a 2,000-character contract. Existing ChatThread is shared by the dedicated inbox and contextual Telehealth thread. Two-Doctor list/read/send denial and backend expiry were verified against current APIs. No web-specific business API introduced.

Prescription upload keeps existing base64_photo/mime_type/doctor_notes and idempotency contracts. Running server returns 409 RESOURCE_CONFLICT / document storage not configured. Camera removed by explicit user request; file picker, local preview and recoverable per-page states verified. STORAGE_CONFIGURATION_REQUIRED; no server-success claim.


## 2026-10-07 ? Prescription compression and progress

Doctor Web preserves POST `/consultations/:appointment_id/prescription` with `base64_photo`, `doctor_notes`, `mime_type`, scoped idempotency and HttpOnly BFF auth. Current mobile uses one image per call; current Rust sanitizer allocates 400 ? 1024 bytes to that single-image submission. Web automatically enforces the same per-page byte cap before transfer. JPEG quality/resizing and PNG resizing use native canvas; WebP is converted to accepted JPEG. Native XHR progress covers browser-to-BFF bytes only, not persistence. No API/DTO/backend/mobile change. Actual 409 unconfigured storage and 429 rate limiting stay recoverable failures.

Detailed contract, evidence and host integrity: [DOCTOR_WEB_RX_UPLOAD_REPORT.md](DOCTOR_WEB_RX_UPLOAD_REPORT.md).


## 2026-10-07 ? Real slot blocking and semantic cards

Approved genuine scheduling gap resolved minimally: enum adds BLOCKED and PATCH `/doctor/slots/:slot_id` accepts only status BLOCKED or AVAILABLE, denies unknown fields, uses verified own Doctor context and atomic conditional UPDATE under existing RLS. AVAILABLE?BLOCKED / BLOCKED?AVAILABLE only; duplicate/lost-race/foreign/protected transitions return 409. Existing DELETE now cancels own AVAILABLE or BLOCKED unbooked rows, never booked/reserved rows. Patient directory/booking still requires AVAILABLE. Pagination is fully consumed before daily availability count. No web-specific business API. BFF adds only this PATCH allowlist entry.

Contract, tests, mobile exception, telemetry findings and full report: [DOCTOR_WEB_SCHEDULE_CORRECTION.md](DOCTOR_WEB_SCHEDULE_CORRECTION.md).
