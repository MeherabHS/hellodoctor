# Sensitive ownership inventory

This inventories all 40 current public tables, including infrastructure and
pre-authentication exceptions. `P` means patient_profiles.id → UNIQUE user_id →
users.id; `D` means doctor_profiles.id → UNIQUE user_id → users.id; `A` means
appointments.id → P and D; `C` means consultation_sessions → A. All arrows are
FK-backed, except deliberately polymorphic audit resource references. A stable
actor/subject FK remains the canonical anchor for those events.

R/F means ENABLE + FORCE RLS. A capability grant is still required in addition
to RLS. Admin labels below describe the existing policy/service boundary, not
a new unrestricted permission. The controlled trace primitive returns IDs only
and independently authorizes/audits Compliance or Platform access.

| Table | Primary subject / ownership path to users.id | Doctor relationship | Patient relationship | Appointment/consultation | RLS / FORCE | Explicit exception / traceability |
|---|---|---|---|---|---|---|
| users | id, canonical security identity | D.user_id | P.user_id | via profiles | R/F | Auth bootstrap/session validation; scheduler reads; API own/explicit Admin roles; password-hash SELECT denied |
| patient_profiles | UNIQUE user_id FK | authorized treatment only | P | A.patient_id | R/F | Clinical/Compliance/Platform/Support demographics; auth bootstrap INSERT; immutable identity |
| doctor_profiles | UNIQUE user_id FK | D, private own profile | no private Patient access | A.doctor_id | R/F | Clinical/Finance/Compliance/Platform; Auth/System/Finance worker grants; public projection excludes private fields |
| doctor_schedule_slots | doctor_id → D | owner D | public availability, not PHI | A.slot_id + matching Doctor composite FK | R/F | Public slot SELECT; own Doctor/Platform/Support writes; Payment/System lifecycle; slot mutations audited |
| appointments | patient_id → P, doctor_id → D | assigned D | owning P | A, slot must match D | R/F | Existing authorized Admin roles; Payment/System transitions, Finance read; booking INSERT own Patient only |
| appointment_clinical_intake | appointment_id → A, matching patient_id → P | approved clinical window | owner P | A | R/F | Clinical/Compliance/Platform; clinical content excluded from audit snapshots |
| prescription_intake_documents | appointment_id → A + matching patient_id → P | approved clinical window | owner P/uploader | A | R/F | Clinical/Compliance/Platform; own Patient INSERT; object metadata + immutable parent chain |
| prescriptions | appointment_id → A + matching patient_id and doctor_id | author D, approved window | owner P | A | R/F | Clinical/Compliance/Platform; Doctor-author INSERT; completion audited; no orphan clinical record |
| consultation_sessions | appointment_id → A | assigned D | owner P | C | R/F | Existing operational Admin roles; System/Finance capabilities; session identity links timeline |
| consultation_telemetry | session_id → C → A | participant D | participant P | C/A | R/F | Existing timeline/Admin policy; System lifecycle; no independent owner copy |
| consultation_telemetry_events | session_id → C → A | participant D | participant P | C/A | R/F | Existing timeline/Admin policy; event timestamps and session chain traceable |
| chat_conversations | appointment_id → A + matching Patient/Doctor pair | participant D | participant P | A | R/F | Existing authorized operational roles/System expiry; participant FKs cannot disagree |
| chat_messages | conversation_id → chat → A; sender_id → users | participant D | participant P | A via chat | R/F | Sender must equal current user; locked/expired/nonparticipant INSERT denied; UPDATE limited to read receipts; content not put into audit snapshots |
| health_questions | optional author_patient_id → P | no private treatment relationship | internal author only when account-originated | none | R/F | Anonymous public question content; internal author SELECT denied to API role; known author retained |
| health_answers | doctor_id → D, question_id → question | author D | optional question author linkage internal | none | R/F | Public community answer; verified Doctor-only INSERT; author relationship traceable |
| doctor_wallets | doctor_id PK → D | owner D | none | earnings through A/transactions | R/F | Platform/Finance, Finance worker; Auth empty-wallet bootstrap; own/authorized INSERT |
| doctor_payout_methods | doctor_id PK → D | owner D | none | indirect earnings | R/F | Existing encrypted destination fields; Finance/Platform read; canonical Doctor key |
| doctor_payout_destinations | doctor_id → D | owner D | none | statements/disbursements | R/F | Finance/Platform, narrow Finance capability; provider identifiers never returned by trace |
| transactions | appointment_id → A | assigned D | owner P | A, optional disbursement item | R/F | Platform/Finance/Compliance; Payment/Finance workers; provider events + financial audits |
| payment_events | transaction_id → transaction → A | via A | via A | A | R/F | Payment/Finance capabilities; provider-event lineage, no separate client-controlled owner |
| disbursement_batches | initiated_by → users | through items D | through item transactions A | indirect | R/F | Finance/Platform/Finance worker; initiated actor and financial audit |
| disbursement_items | doctor_id → D; batch_id; approved_by → users | owner D | through allocated transactions | indirect | R/F | Finance/Platform/Finance worker; destination/statement composite integrity; approvals audited |
| financial_adjustments | doctor_id → D, created_by → users | owner D | none | period-level financial correction | R/F | Finance/Platform/Finance worker; created actor and reason |
| financial_refund_requests | grievance_id → grievance → C/A; transaction_id → A; approved_by → users | via A | via A | A/C | R/F | Finance worker + existing authorized Finance/Compliance workflow; resolution audited |
| financial_periods | global currency/month accounting state; optional finalized_by → users | via statements | none directly | indirect | R/F | Finance capability finalization; global row is not a Patient-owned record |
| monthly_payout_statements | doctor_id → D, period_id, destination_id | owner D | indirect allocated transactions | indirect | R/F | Finance capability/authorized reads; immutable finalization snapshot and allocation FKs |
| monthly_statement_transactions | statement_id → statement → D; transaction_id → A | both paths agree through finalization | via A | A | R/F | Finance-only insertion; FK-backed allocation lineage |
| grievance_reports | patient_id → P, consultation_id → C/A | assigned D through C/A | submitting P must match C/A | C/A | R/F | Clinical/Compliance/Platform + narrow Finance; Patient-only INSERT; parent/child subject checks |
| grievance_adjudications | grievance_id → grievance → C/A; admin_user_id → users | via C/A | via C/A | C/A | R/F | Authorized adjudication + Finance capability; responsible Admin FK/audit |
| notifications | user_id → users | recipient D mapping | recipient P mapping | optional event reference | R/F | Own recipient reads; backend/System writes; canonical recipient and originating event |
| notification_device_tokens | user_id → users, device_id PK | recipient D mapping | recipient P mapping | none | R/F | Own registration/update; refresh-authorized Auth Worker logout; trace returns device UUID, never push token |
| doctor_articles | author_id → D | author D | public readers have no private owner linkage | none | R/F | Public PUBLISHED projection, own drafts/edits; publication/archive audit; no Admin review |
| audit_events | actor_id → users; session_id → auth_sessions; resource_id polymorphic | actor or resource chain | actor or resource chain | resource-specific | R/F | API/Finance append only; Security read; audited Compliance IDs-only primitive; no UPDATE/DELETE grants |
| system_error_logs | optional impacted_user_id → users; ack/resolution actors → users | known impacted D mapping | known impacted P mapping | correlated reference where available | R/F | Security/Platform + System worker; genuine global process errors may have no user; no identity invented |
| idempotency_records | user_id → users; response_body_ref → appointment | authenticated replay context | authenticated replay context | A authorization rechecked | R/F | Own subject only; current auth/ABAC/resource checks before private replay; context-bound INSERT |
| auth_sessions | user_id → users; token_family_id identifies session lineage | login D mapping | login P mapping | no clinical records | no/no | Auth worker only; pre-login and JWT revocation authority; API has no table grant; audit session FK |
| mfa_credentials | user_id → users | applicable account mapping | applicable account mapping | none | no/no | Auth worker only; encrypted credentials never exposed by trace/API |
| pending_mfa_challenges | user_id → users | challenged Doctor/Admin | none for Patient OTP | none | no/no | Auth worker only; row-locked single-use challenge; ephemeral retention |
| phone_otps | pre-account phone challenge, deliberately no user yet | no Doctor login identity substitution | consumed before atomic Patient bootstrap | none | no/no | Auth worker only; ephemeral pre-identity exception, no medical/financial ownership; one-time consumption |
| _sqlx_migrations | migration version/checksum metadata | none | none | none | no/no | Migrator authority only; application readiness SELECT; never manually edited |

## Retention and exceptions

No account deletion/cascade policy was broadened. Existing clinical, financial
and audit parent FKs generally RESTRICT deletion; existing bounded ephemeral
auth/session/device and dependent event retention semantics remain in place.
Known anonymous-Q&A author links now RESTRICT Patient deletion. Public Q&A
without an author and global financial-period/system-error rows are deliberate
non-user subjects, not guessed ownership. Audit resource references are
polymorphic; the actor and optional impacted subject remain FK-backed, and the
controlled trace resolves resource relationships rather than treating arbitrary
resource UUIDs as authorization evidence.

No PHI/financial table relies on email, phone, display name or registration
number as an ownership key. A phone is used only for the pre-account OTP
challenge/account lookup; ownership becomes users.id after authentication.

## Permanent regression mapping

`tests/api_tests.rs` adds these grouped tests:

- `test_identity_unique_profiles_role_consistency_and_immutable_mapping`: Patient/Doctor uniqueness, role mismatch and immutable identity (requirements 1–3).
- `test_identity_patient_direct_rls_read_update_delete_and_documents`: cross-Patient SELECT/UPDATE/DELETE and document insertion/access (4, 9).
- `test_identity_doctor_private_financial_and_treatment_context_rls`: private Doctor/financial separation, unrelated Patient, caller-bound definer helper, current clinical window, expiring profile follow-up and cross-Doctor appointment attack (5–8).
- `test_identity_wrong_doctor_writes_and_chat_sender_spoofing_denied`: prescription, slot and chat INSERT/UPDATE/DELETE and sender identity (10–11).
- `test_identity_pool_context_resets_for_patients_and_doctors`: proven reuse of one pooled connection, commit and rollback (12).
- `test_identity_compliance_trace_scoped_authorized_and_audited`: authorized/rejected roles, canonical linkage, scoped case, audit metadata, not-found access (13–17).
- `test_identity_rls_capabilities_append_only_audit_and_public_privacy`: five non-owner/non-BYPASS roles, FORCE RLS, immutable audit grants and public Doctor/Q&A privacy (18).
- `test_identity_patient_bootstrap_atomic_and_doctor_phone_never_becomes_patient`: one matching Patient profile, no Doctor role confusion, no orphan account after forced profile failure.
- `test_identity_composite_parent_ownership_cannot_disagree_even_without_rls`: parent/child FK consistency even under a privileged fixture connection.
- `test_identity_patient_booking_cannot_substitute_another_patient`: supplied Patient UUID cannot choose a booking owner; direct wrong-subject INSERT rejected.

Existing treatment-history, session, authorization, financial and scheduling
regressions remain required; this matrix supplements them.
