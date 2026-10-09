-- ============================================================================
-- PAYOUT_PAID wiring: Finance Worker needs to create a notification in the
-- same transaction as a disbursement item's authoritative PAID transition.
--
-- `DisbursementService::confirm_disbursement_item_paid` runs under
-- `hellodoctor_finance_worker` (the only role with UPDATE on
-- `disbursement_items` — see migrations/20261003000001_rls_hardening) and
-- calls `NotificationService::notify_payout_paid` in that same transaction,
-- mirroring how `EARNING_CREDITED` is fired from the same transaction as its
-- wallet credit. But `notifications_backend_insert`
-- (migrations/20261020000001_doctor_engagement_notifications) only names
-- `hellodoctor_api, hellodoctor_system_worker` — confirmed by a real failing
-- test ("permission denied for table notifications"), not assumed: Finance
-- Worker had no INSERT grant on `notifications` at all.
-- ============================================================================

DROP POLICY notifications_backend_insert ON notifications;
CREATE POLICY notifications_backend_insert ON notifications
    FOR INSERT
    TO hellodoctor_api, hellodoctor_system_worker, hellodoctor_finance_worker
    WITH CHECK (true);

GRANT INSERT ON notifications TO hellodoctor_finance_worker;
