-- Forward-only fix: the admin reschedule endpoint
-- (trace_handlers::reschedule) now cancels a stale T-15 reminder for the
-- appointment's old time inside its own existing hellodoctor_payment_worker
-- transaction, so the cancellation is atomic with the reschedule itself
-- (never committed independently, never left half-done). That role was
-- never granted DELETE on notifications, and notifications is FORCE ROW
-- LEVEL SECURITY with no DELETE policy applicable to it at all -- confirmed
-- live: the reschedule endpoint 500'd with "permission denied for table
-- notifications" the moment this call was added.
--
-- Scoped exactly like the existing hellodoctor_system_worker UPDATE/SELECT
-- policies on this table (unconditional USING(true)): the application code
-- path is the real boundary here
-- (notification_repo::cancel_pending_reminders_for_appointment only ever
-- deletes rows matching a T-15-reminder dedup-key prefix for one specific
-- appointment, never an arbitrary notification), not a row-level predicate.
--
-- SELECT is required alongside DELETE, not optional: Postgres needs read
-- access to any column referenced in a DELETE statement's WHERE clause
-- (here, dedup_key and delivery_status) to evaluate it at all -- confirmed
-- live, DELETE-only still 500'd with the identical "permission denied"
-- error until this was added too.
GRANT SELECT, DELETE ON notifications TO hellodoctor_payment_worker;
CREATE POLICY notifications_payment_worker_select ON notifications
    FOR SELECT
    TO hellodoctor_payment_worker
    USING (true);
CREATE POLICY notifications_payment_worker_delete ON notifications
    FOR DELETE
    TO hellodoctor_payment_worker
    USING (true);
