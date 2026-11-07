-- Phase 1B: durable, asynchronous transactional-email dispatch.
--
-- Prior state: a business handler rendered content and called
-- `email_delivery::send_templated_email` *after* its own transaction
-- committed, which both inserted the `email_outbox` row and attempted the
-- provider send inline, in the same awaited call. Two real gaps followed:
--   1. If the process crashed between "business transaction commits" and
--      "send_templated_email is called," no outbox row was ever created —
--      the event was silently lost, with nothing durable tying the business
--      event to a pending email intent for any later reconciliation to find.
--   2. The handler awaited the send (bounded by `ACS_REQUEST_TIMEOUT` and up
--      to two inline attempts), so a slow/degraded provider could delay an
--      otherwise-already-committed booking/approval/payment response.
--
-- This migration adds what's needed for the new model: a *thin* outbox row
-- (event type + dedup key + a reference to the business entity, never the
-- rendered content) is inserted in the SAME transaction as the business
-- event, on that transaction's own role/connection — true atomicity, no
-- separate statement, no window. A background worker (system role, already
-- broadly readable — see services/notification_scheduler.rs's existing
-- doctor/patient/appointment queries on the same pool) resolves fresh
-- recipient/content data and renders the template at dispatch time, then
-- claims/leases/retries exactly like the existing push-notification delivery
-- queue (notification_repo/notification_scheduler) — SELECT ... FOR UPDATE
-- SKIP LOCKED claims, a lease (`claimed_until`) so a crashed worker's claim
-- is reclaimable rather than stuck forever, and `next_attempt_at` for
-- exponential backoff between retries across ticks (not inline sleeps).
ALTER TABLE email_outbox
    ADD COLUMN related_entity_type VARCHAR(30),
    ADD COLUMN related_entity_id UUID,
    ADD COLUMN claimed_until TIMESTAMPTZ,
    ADD COLUMN next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now();

ALTER TABLE email_outbox DROP CONSTRAINT email_outbox_delivery_status_check;
ALTER TABLE email_outbox ADD CONSTRAINT email_outbox_delivery_status_check
    CHECK (delivery_status IN (
        'CREATED', 'SENT_TO_PROVIDER', 'FAILED', 'EXPIRED', 'SUPERSEDED'
    ));

-- Claim query: due, unclaimed-or-stale-leased, not-yet-terminal rows.
CREATE INDEX idx_email_outbox_claimable ON email_outbox(next_attempt_at)
    WHERE delivery_status = 'CREATED';

-- Supersession lookups scope by the business entity a row is about (e.g. "the
-- CONSULTATION_RESCHEDULED/CONSULTATION_CONFIRMED rows for appointment X").
CREATE INDEX idx_email_outbox_related_entity ON email_outbox(related_entity_type, related_entity_id)
    WHERE delivery_status = 'CREATED';

-- The business transactions that create these "thin" rows run on roles that
-- previously had no reason to touch email_outbox at all:
--   - hellodoctor_api: Doctor application approval/rejection
--     (api/v1/admin_handlers.rs), Doctor application received
--     (api/v1/auth_handlers.rs runs on AuthWorker already; unaffected).
--   - hellodoctor_payment_worker: consultation confirmed + payment confirmed
--     (services/appointment_service.rs), consultation rescheduled
--     (api/v1/trace_handlers.rs) — all three already run their business
--     transaction on this role.
--   - hellodoctor_finance_worker: refund processed
--     (services/grievance_service.rs), already running its business
--     transaction on this role.
-- Each gets exactly the same shape of policy/grant the existing two writer
-- roles already have on this operational-metadata-only table (no owner
-- SELECT policy, never a user-facing inbox — see migrations/20261205000001's
-- own doc comment).
CREATE POLICY email_outbox_api_worker ON email_outbox
    FOR ALL TO hellodoctor_api USING (true) WITH CHECK (true);
CREATE POLICY email_outbox_payment_worker ON email_outbox
    FOR ALL TO hellodoctor_payment_worker USING (true) WITH CHECK (true);
CREATE POLICY email_outbox_finance_worker ON email_outbox
    FOR ALL TO hellodoctor_finance_worker USING (true) WITH CHECK (true);

GRANT SELECT, INSERT ON email_outbox TO hellodoctor_api;
-- Also UPDATE: `trace_handlers.rs::reschedule` (payment-worker role) calls
-- `email_outbox_repo::supersede_pending` to mark an earlier, still-pending
-- CONFIRMED/RESCHEDULED row for the same appointment stale when a new
-- reschedule event supersedes it — see that function's own doc comment.
GRANT SELECT, INSERT, UPDATE ON email_outbox TO hellodoctor_payment_worker;
GRANT SELECT, INSERT ON email_outbox TO hellodoctor_finance_worker;

-- The dispatch worker (hellodoctor_system_worker) needs to UPDATE the new
-- claim/lease/backoff columns and SUPERSEDE rows; it already has SELECT,
-- UPDATE, INSERT from migrations/20261205000001 and 20261206000001.

-- The dispatch worker also needs read-only access to the three tables it
-- resolves fresh display content from at send time for payment/refund/
-- reschedule emails (consultation/doctor/patient data was already readable
-- — see migrations/20261020000001's existing system_worker grants, reused
-- as-is by services/appointment_service.rs::consultation_email_details).
-- Mirrors the existing `*_system_worker_all`/`doctor_worker_read` read-only
-- policy shape (migrations/20261003000001, 20261026000001): SELECT only,
-- never INSERT/UPDATE/DELETE — this worker only ever reads these three to
-- render an email about a transaction/refund/reschedule some other role
-- already fully owns.
CREATE POLICY transactions_system_worker_read ON transactions
    FOR SELECT TO hellodoctor_system_worker USING (true);
GRANT SELECT ON transactions TO hellodoctor_system_worker;

CREATE POLICY financial_refund_requests_system_worker_read ON financial_refund_requests
    FOR SELECT TO hellodoctor_system_worker USING (true);
GRANT SELECT ON financial_refund_requests TO hellodoctor_system_worker;

CREATE POLICY appointment_reschedules_system_worker_read ON appointment_reschedules
    FOR SELECT TO hellodoctor_system_worker USING (true);
GRANT SELECT ON appointment_reschedules TO hellodoctor_system_worker;

-- `appointment_service.rs`'s booking-confirmation transaction (payment-worker
-- role) now also resolves the confirmed recipient's email address, inside
-- that same transaction, to enqueue the two outbox rows above. It already
-- has SELECT on `patient_profiles` (migrations/20261118000002) but none at
-- all on `users` — mirrors `hellodoctor_api`'s own column-restricted grant
-- (migrations/20261026000001), narrowed further to just what this join
-- needs (never `password_hash` or anything else sensitive).
CREATE POLICY users_payment_worker_read ON users
    FOR SELECT TO hellodoctor_payment_worker USING (true);
GRANT SELECT (id, email) ON users TO hellodoctor_payment_worker;

-- `grievance_service.rs`'s refund-completion transaction (finance-worker
-- role) resolves the refunded patient's email the same way, inside that same
-- transaction. `users_finance_worker_language_read`
-- (migrations/20261122000001) already gives finance_worker an unconditional
-- SELECT policy on `users` — just not the `email` column; adding only that
-- column grant, no new policy needed. `patient_profiles` has no
-- finance_worker policy or grant at all yet (its existing `ctx` there is the
-- admin/ops caller approving the refund, not the patient themselves, so the
-- PUBLIC `patient_profiles_access` policy's self/guardian conditions don't
-- apply) — mirrors `patient_profiles_system_worker_read`'s identical
-- unconditional-read fix for the same underlying reason.
GRANT SELECT (email) ON users TO hellodoctor_finance_worker;
CREATE POLICY patient_profiles_finance_worker_read ON patient_profiles
    FOR SELECT TO hellodoctor_finance_worker USING (true);
GRANT SELECT ON patient_profiles TO hellodoctor_finance_worker;
