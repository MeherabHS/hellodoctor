DROP POLICY notifications_payment_worker_delete ON notifications;
DROP POLICY notifications_payment_worker_select ON notifications;
REVOKE SELECT, DELETE ON notifications FROM hellodoctor_payment_worker;
