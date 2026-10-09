REVOKE INSERT ON notifications FROM hellodoctor_finance_worker;

DROP POLICY notifications_backend_insert ON notifications;
CREATE POLICY notifications_backend_insert ON notifications
    FOR INSERT
    TO hellodoctor_api, hellodoctor_system_worker
    WITH CHECK (true);
