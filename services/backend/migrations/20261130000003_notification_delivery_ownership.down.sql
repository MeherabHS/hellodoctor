-- Do not silently discard terminal dispositions, live claims or ambiguous sends.
DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM notifications WHERE delivery_status='SUPPRESSED'
               OR delivery_claim_token IS NOT NULL OR delivery_uncertain) THEN
        RAISE EXCEPTION 'Delivery ownership downgrade requires a reviewed drain/reconciliation plan';
    END IF;
END $$;
DROP POLICY notifications_admin_cancel_campaign_update ON notifications;
DROP INDEX idx_notifications_expired_claim;
ALTER TABLE notifications DROP CONSTRAINT notifications_claim_pair;
ALTER TABLE notifications DROP COLUMN delivery_claim_token,
    DROP COLUMN delivery_lease_until, DROP COLUMN delivery_uncertain;
ALTER TABLE notifications DROP CONSTRAINT notifications_delivery_status_check;
ALTER TABLE notifications ADD CONSTRAINT notifications_delivery_status_check
    CHECK (delivery_status IN ('CREATED','DUE','SENT_TO_PROVIDER','DELIVERED','FAILED','READ'));
