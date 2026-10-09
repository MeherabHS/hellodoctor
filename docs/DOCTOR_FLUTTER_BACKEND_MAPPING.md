> Current canonical ownership / Doctor Q&A correction (2026-10-08): [identity enforcement, audited Compliance trace and Q&A rendering evidence](USER_ISOLATION_AND_QNA.md); [all-table ownership inventory](USER_OWNERSHIP_INVENTORY.md). Migration 20261026000001 adds the database guards. Q&A real-device inspection remains pending until a device is connected; widget/golden renders are separate evidence. Earlier verification counts below are historical.

> Current public-exposure/session hardening and direct Doctor Articles (2026-10-08): [security controls and deployment requirements](PUBLIC_EXPOSURE_HARDENING.md), [Doctor Articles without Admin review](DOCTOR_ARTICLES.md). Earlier dated results below are historical snapshots. Slot approvals are explicitly excluded by product policy.

> Current Kenya professional-registration contract (2026-10-08): [explicit PENDING / VERIFIED / REJECTED model, KMPDC/KE, generic Admin routes, migration and evidence](KENYA_PROFESSIONAL_VERIFICATION.md). Earlier dated audit/results sections are historical snapshots and do not describe the current review model.

# Doctor Flutter Backend Mapping

This document is the frozen backend contract for the Doctor Flutter client.
All routes are under `/api/v1`, use the existing JWT/AuthContext middleware,
and retain ABAC plus PostgreSQL RLS enforcement.

| Doctor feature | Method and path | Auth/authorization | Data source and notes |
|---|---|---|---|
| Doctor login | `POST /auth/doctor/login` | Public step 1 | Existing MFA flow |
| Doctor registration | `POST /auth/doctor/register` | Public, rate-limited | Creates a `DOCTOR` account and profile with `professional_verification_status=PENDING` (derived compatibility `is_verified=false`); accepts only schema-backed onboarding fields and returns `PENDING_VERIFICATION`. ClinicalAdmin verification remains required. |
| Doctor MFA | `POST /auth/doctor/verify-otp` | Public step 2 | Existing brute-force protection |
| Refresh/logout | `POST /auth/refresh`, `POST /auth/logout` | Refresh family | Existing session rotation/revocation |
| Own profile | `GET /doctor/profile` | Doctor, `DoctorRead` | Public doctor profile fields |
| Edit own profile | `PATCH /doctor/profile` | Doctor, `DoctorManageProfile` | Explicit allow-listed field only: `full_name` (`deny_unknown_fields`); hospital/qualifications/verification remain ClinicalAdmin-only, see note below |
| Duty status | `GET /doctor/duty`, `PUT /doctor/duty` | Doctor, own record | Shared `DutyStatus` DTO: `{is_on_duty}` |
| Doctor queue | `GET /doctor/appointments` | Doctor, own appointments | SQL-scoped appointment/slot list; queue derives from existing states. **DOCTOR_QUEUE_UPCOMING_FIRST (2026-10-07)**: default ordering is now server-side — eligible upcoming consultations (`CONFIRMED`/`WAITING`/`IN_CONSULTATION`) always sort first by nearest `start_time`, with completed/cancelled/other items following newest-first; cancelled items never displace upcoming ones. Flutter must not re-sort this list. |
| Appointment history | `GET /doctor/appointments` | Doctor, own appointments | `status`, `limit`, `offset` filters |
| Own schedule | `GET /doctor/slots` | Doctor, own slots | SQL-scoped, paginated |
| Create slot | `POST /doctor/slots` | Doctor, own identity | Rejects invalid/overlapping slots |
| Cancel slot | `DELETE /doctor/slots/{slot_id}` | Doctor, own available slot | Booked/locked slots cannot be cancelled |
| Patient history | `GET /doctor/appointments/{current_appointment_id}/patient-history` | Current treating doctor; contextual authorization | Existing consultation episodes, summary-first |
| History detail | `GET /doctor/appointments/{current_appointment_id}/patient-history/{historical_consultation_id}` | Same current appointment context | Existing intake, prescription metadata, document metadata |
| Consultation summary | `GET /doctor/consultations/{appointment_id}/summary` | Assigned doctor | Existing appointment/intake/session fields |
| Agora token | `POST /consultations/{appointment_id}/rtc-token` | Existing consultation authorization | Backend-generated token |
| Telemetry | `POST /consultations/{appointment_id}/telemetry` and `/telemetry/events` | Existing consultation authorization | Existing DTOs |
| Complete consultation | `POST /consultations/{appointment_id}/complete` | Assigned doctor | Backend settlement remains authoritative |
| Doctor prescription | `POST /consultations/{appointment_id}/prescription` | Assigned doctor | Existing secure upload flow |
| No prescription | `POST /consultations/{appointment_id}/complete-no-rx` | Assigned doctor | Existing outcome flow |
| Document URL | `GET /appointments/{appointment_id}/documents/{document_id}` | Existing relationship/RLS | Short-lived authorized URL |
| Chat list | `GET /doctor/chats` | Doctor, own conversations | Paginated conversation summaries |
| Chat messages | `GET/POST /chat/{conversation_id}/messages` | Existing conversation authorization | HTTP chat remains unchanged |
| Wallet summary | `GET /doctor/wallet/summary` | Doctor, own wallet | Read-only financial summary |
| Wallet ledger | `GET /doctor/wallet/ledger` | Doctor, own wallet | Paginated settlement transactions; no withdrawal |
| Monthly payout statement | `GET /doctor/wallet/statements?month=YYYY-MM` | Doctor, own wallet | Implemented (2026-10-06): server-calculated month-bound statement, `FINALIZED`-snapshot vs `IN_PROGRESS`-estimate branches, pagination; verified by `test_financial_close_snapshot_adjustment_routing_and_approval` and the 2026-10-07 audit (docs/DOCTOR_SYSTEM_FINAL_MULTI_AGENT_AUDIT.md). This row previously said "Not yet available" — that was stale. |
| Provider payout binding | `GET/PUT /doctor/wallet/payout-method`, `POST /doctor/wallet/payout-methods/dev-bind` | Doctor, own wallet; write routes are `development`-only | DEV_SIMULATED binding only — a doctor can configure a payout destination in development, clearly labeled as simulated. Real provider (M-Pesa/bank/card) authorization and execution remain `DEFERRED_EXTERNAL_INTEGRATION`; no production route exists to bind a real destination yet. |
| Open Health Q&A list | `GET /doctor/qna/questions` | Eligible authenticated doctor | Open questions only, paginated |
| Open Health Q&A answer | `POST /doctor/qna/questions/{question_id}/answers` | Eligible authenticated doctor | One answer transitions question to `ANSWERED` |

The prototype withdrawal button is a `PROTOTYPE_BUSINESS_RULE_CONFLICT` and has
no backend endpoint. Finance Admin remains responsible for scheduled
disbursement.

Patient history is contextual and time-bounded: access is denied before
scheduled start minus 15 minutes; it remains available during a valid treatment
episode and until `ended_at + 24 hours`, then closes. Server UTC time enforces
this; pagination cannot affect authorization. Historical episodes are limited
to records preceding the current appointment's scheduled start (plus the
current doctor's own current episode), so later unrelated consultations are
never exposed. Access emits `PATIENT_HISTORY_VIEWED` or
`PATIENT_HISTORY_EPISODE_VIEWED` through the existing audit service.

The Q&A domain is separate from private consultation history. Q&A responses do
not grant access to the author's patient records. Doctor list/view/answer is
implemented; patient question creation is tracked as
`PATIENT_QNA_SUBMISSION_REQUIRED_FOR_PATIENT_PHASE` and is intentionally
deferred to the Patient backend/Flutter phase.

**DOCTOR_ENGAGEMENT notification domain (2026-10-07, supersedes the
`DOCTOR_NOTIFICATION_DELIVERY_CAPABILITY_REQUIRED` line above — the in-app
inbox/API side of that capability is now built and tested; see
`docs/CHECKPOINT.md` for the full pass):**

| Notification feature | Method and path | Auth/authorization | Notes |
|---|---|---|---|
| Own inbox | `GET /notifications` | Any authenticated user, own rows only | Paginated (`limit`/`offset`), RLS-enforced ownership |
| Unread count | `GET /notifications/unread-count` | Any authenticated user, own rows | — |
| Mark one read | `POST /notifications/{id}/read` | Any authenticated user, own rows | Silent no-op (not an error) if the id isn't owned by the caller |
| Mark all read | `POST /notifications/read-all` | Any authenticated user, own rows | — |
| Register push device | `POST /notifications/devices` | Any authenticated user | Stores `{platform, push_token}`; see `PUSH_PROVIDER_CONFIGURATION_REQUIRED` below |

Notification types implemented: `DAILY_CONSULTATION_SUMMARY` (configurable
Kenya-local time via `DOCTOR_DAILY_SUMMARY_TIME_LOCAL`, default `07:00`,
exactly once per Doctor per Kenya-local calendar day),
`CONSULTATION_REMINDER_15M` (created the moment an eligible appointment's
T-15 window opens; suppressed for cancelled appointments; immediate for a
near-term booking rather than a skipped/past-dated job),
`EARNING_CREDITED` (fired from the same transaction as the authoritative
wallet credit in `doctor_complete_consultation`, so a retried/duplicate
completion never double-notifies), `REGISTRATION_VERIFIED`/
`REGISTRATION_REJECTED` (Admin professional-registration verify/reject). `PAYOUT_PAID` exists and
is tested in isolation but has no live caller yet — real payout execution
remains `DEFERRED_EXTERNAL_PAYMENT_INTEGRATION`, and this must never be
wired to the `DEV_SIMULATED` path.

**`PUSH_PROVIDER_CONFIGURATION_REQUIRED`**: no FCM/APNs integration exists.
`notification_transport.rs`'s `NotificationTransport` trait + `NoopTransport`
is the complete adapter boundary and device-token contract; delivery is
never faked. Flutter notification-bell/inbox UI and deep-link wiring are a
separate, not-yet-started Flutter-side task (out of scope for the backend
pass that built the above).

`current_hospital` and `qualifications` are verified profile attributes and are
ClinicalAdmin-controlled. Doctor self-service profile updates cannot change
them, professional verification state/review metadata, `is_verified`, role, wallet, or other financial/security fields.


### Local storage cleanup ? 2026-10-07

At the user's request, local MinIO services/settings and Android emulator were removed. Document APIs retain encrypted external S3 support, but local development with no configured store returns an explicit conflict/unavailable message and never confirms upload or inserts successful prescription metadata. Backend tests use a clearly isolated in-memory object-store fixture; this is not evidence of a configured production storage service. Full device runtime verification is unavailable without a connected device.
