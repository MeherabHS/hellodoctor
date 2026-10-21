# Consultation trace and grievance investigation

Implementation milestone, 2026-10-16. Automated and live Web/API acceptance passed; Android physical acceptance remains pending. See the current gate and device record in [PHYSICAL_DEVICE_EXISTING_FEATURES_E2E.md](PHYSICAL_DEVICE_EXISTING_FEATURES_E2E.md).

## Data separation and retention

`consultation_trace_events` is a separate, high-resolution ledger. Its default retention is five days, configurable with `CONSULTATION_TRACE_RETENTION_DAYS` (only 3, 4 or 5 accepted). The INSERT trigger replaces caller-supplied event time, expiry, actor, session and request identifiers with transaction context and database `clock_timestamp()`. Ordering is `(event_time_server, id)`.

This TTL never deletes appointments, consultation sessions, chat messages, prescriptions, clinical documents, transactions, financial ledgers, grievances, adjudications or permanent audit events. Temporary trace metadata contains references and bounded operational states, never message bodies, medical narrative, document bytes, tokens or signed URLs. Metadata keys are allowlisted at the database boundary. Server-side producers determine event names, result and reason; clients can submit only a bounded acknowledgement DTO, not arbitrary trace rows.

The existing notification scheduler invokes maintenance. It locks expired conversations in batches of 100 and deletes expired, unheld trace rows in batches of 500. DB cleanup clamps each requested batch to 1..1000. System-worker access is separate from human ADMIN. No new scheduler, service or external push provider is introduced.

## Preservation and concurrency

A successful grievance INSERT creates a consultation preservation hold in the same transaction. Grievance creation and cleanup take the same appointment advisory lock; cleanup rechecks the hold after acquiring it. Existing grievances are backfilled during migration. A hold preserves the consultation's existing and subsequent temporary trace while active. No automatic release duration or ordinary Admin deletion endpoint exists. Release requires a separately approved policy/action. Rows already purged before a later grievance cannot be reconstructed.

Application roles and ADMIN through the API have SELECT/INSERT only where authorized, no trace UPDATE/DELETE. The system maintenance capability alone has trace DELETE and cleanup EXECUTE. Trace and hold tables use FORCE RLS. Rollback refuses to destroy trace tables while an active preservation hold exists.

## Event producers

| Source | Evidence |
| --- | --- |
| Booking transaction | APPOINTMENT_CREATED, PAYMENT_INITIATED (transaction created; not provider acceptance) |
| Authorized payment initiation denied without an adapter | PAYMENT_INITIATION_REJECTED / PROVIDER_UNAVAILABLE; no financial success |
| Signed authoritative payment callback | PAYMENT_CONFIRMED |
| Conversation activation | CONSULTATION_ACTIVATED; retry does not create a second activation |
| Authenticated client acknowledgement | PATIENT_CONSULTATION_OPENED, DOCTOR_CONSULTATION_OPENED, CHAT_OPENED |
| Message transaction | FIRST_CHAT_MESSAGE_SENT, CHAT_MESSAGE_SENT, NEW_CHAT_NOTIFICATION_CREATED |
| Rejected visible-thread mutation | CHAT_MESSAGE_REJECTED with safe reason |
| Per-user/message first display acknowledgement | CHAT_READ_ACKNOWLEDGED; first timestamp is retained |
| Clinical image flow | ATTACHMENT_UPLOAD_STARTED/AUTHORIZED/COMPLETED/REJECTED |
| Authorized retrieval | ATTACHMENT_VIEW_AUTHORIZED/VIEWED, RX_VIEW_AUTHORIZED/VIEWED |
| Intake/Rx creation | ATTACHMENT_UPLOAD_COMPLETED, RX_CREATED |
| Completion | CONSULTATION_COMPLETED |
| Expiry maintenance | CHAT_LOCKED, with persisted locked_at and reason |
| Grievance creation | GRIEVANCE_CREATED and preservation hold |
| Admin sensitive reads/mutations | Permanent actor-specific audit; investigation, history, attachments, Rx, reschedule, adjudication |
| Logout/revocation | Permanent SESSION_REVOKED, linked to related consultation session families |
| Correlated infrastructure failure | CONSULTATION_REQUEST_FAILED references the sanitized system-error group and each original request |
| Client refresh recovery | CLIENT_REPORTED/CHAT_REFRESH acknowledgements where observable; never proof that a Doctor ignored a message |

Payment failure/reconciliation facts remain sourced from the existing financial event/state records. No external-provider request/response event is manufactured while no adapter is connected. Actual download evidence means the backend retrieved authorized bytes; it cannot prove that a human interpreted them. Client acknowledgement likewise cannot prove comprehension. CHAT_LOCKED records the persisted transition observation; `expires_at` separately provides the exact send boundary.

## Request and account attribution

Patient Flutter, Doctor Flutter and both Web BFFs forward safe client type/build and a UUID request ID. Backend middleware normalizes the ID once and carries it through AuthContext, DB context, audit, trace, response and system-error capture. Authenticated sender/account identity is derived server-side. Deduplicated system errors retain a per-request consultation trace reference so grouping does not erase individual request correlation.

Client timestamps, when supplied, are explicitly non-authoritative metadata. Client type/build is self-reported, not device attestation. Sessions identify authenticated accounts; evidence does not identify the physical person holding a shared phone.

## Admin grievance section

`GET /admin/grievances/:id/timeline` is ADMIN-only and records the specific viewing Admin account. The grievance detail embeds **Consultation investigation timeline**. It displays every returned trace and permanent audit event, server time, actor role/account, session, request/trace ID, resource, result, reason, safe metadata, client/build and normal expiry. Every event also exposes its ID and an expandable complete safe record, preventing fields such as received_at from disappearing in the readable summary. Ordering preserves database microseconds rather than truncating to JavaScript milliseconds. It also displays all linked record-reference arrays, preservation holds and separately labelled existing telemetry.

Facts resolve relationally to appointment, consultation, Doctor, Patient, modality, schedule, transaction and conversation. Record references cover messages (no body), notifications, read acknowledgements, payments, documents, prescriptions, reschedules, adjudications and sanitized error references. A human-readable grievance number is never substituted for an API UUID. Decision notes come from the individual Admin; preset clinical/telemetry conclusions have been removed.

## Migrations and implementation

Forward/reverse SQLx migrations `20261106000001` through `20261106000006` implement the canonical role, trace, holds, historical encounter access, immutable rescheduling, narrow chat helpers and investigation references. Existing applied migration files and `_sqlx_migrations` are not rewritten. The B-01 readiness guard remains in place.

Primary sources: `services/backend/src/services/consultation_trace.rs`, `api/v1/trace_handlers.rs`, `api/middleware/request_context.rs`, `system_error.rs`, and `apps/admin-web/src/components/GrievanceInvestigation.tsx`.

## Limitations

No real external payment, Azure storage, device delivery or physical-device acceptance is claimed. Video/Agora remains deferred. High-resolution evidence expires by design; permanent business records survive. Offline observations cannot reach the backend while disconnected and are not guaranteed to survive process death. No audio/video is recorded. An uncertain attachment COMMIT preserves encrypted private bytes for safe retry/reconciliation instead of risking deletion of a committed attachment.

The investigation joins permanent attachment reads through message IDs as well as intake/Rx IDs, and includes related session-revocation/role-mismatch evidence. Adjudication records include the actual recorded action, notes, remedy, actor and server time. The focused 21-test rerun verifies actual Patient/Admin attachment retrieval events and their presence in the grievance timeline, canonical Admin attribution, preservation, pagination, concurrency, exact expiry and historical isolation.


## Subsequent API traffic correction (2026-10-16)

See [API_TRAFFIC_CACHE_AND_POLLING_VERIFICATION.md](API_TRAFFIC_CACHE_AND_POLLING_VERIFICATION.md) for route/background polling, in-flight coalescing, bounded pagination, selected-day Schedule reads and the separate post-milestone test/device evidence. Earlier counts and screenshots above remain historical evidence for their recorded revision. No new physical-device claim is implied.
