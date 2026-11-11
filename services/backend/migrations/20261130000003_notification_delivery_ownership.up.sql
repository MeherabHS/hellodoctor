-- Remediation C: additive worker ownership and explicit terminal suppression.
-- No historical notifications are rewritten or replayed. Legacy FAILED rows
-- without next_delivery_at are terminal under the corrected worker predicate.
ALTER TABLE notifications DROP CONSTRAINT notifications_delivery_status_check;
ALTER TABLE notifications ADD CONSTRAINT notifications_delivery_status_check
    CHECK (delivery_status IN ('CREATED','DUE','SENT_TO_PROVIDER','DELIVERED','FAILED','READ','SUPPRESSED'));

ALTER TABLE notifications
    ADD COLUMN delivery_claim_token UUID,
    ADD COLUMN delivery_lease_until TIMESTAMPTZ,
    ADD COLUMN delivery_uncertain BOOLEAN NOT NULL DEFAULT FALSE,
    ADD CONSTRAINT notifications_claim_pair CHECK
        ((delivery_claim_token IS NULL) = (delivery_lease_until IS NULL));
CREATE INDEX idx_notifications_expired_claim ON notifications(delivery_lease_until)
    WHERE delivery_status='DUE';

-- Cancellation may fence a previously attempted campaign row while retaining
-- uncertain provider outcomes. Never grants generic cross-user updates.
CREATE POLICY notifications_admin_cancel_campaign_update ON notifications
    FOR UPDATE TO hellodoctor_api
    USING (current_setting('app.current_role', true)='ADMIN'
       AND related_entity_type='CAMPAIGN'
       AND EXISTS (SELECT 1 FROM campaigns c
                   WHERE c.id=notifications.related_entity_id AND c.status='CANCELLED'))
    WITH CHECK (current_setting('app.current_role', true)='ADMIN'
       AND related_entity_type='CAMPAIGN' AND delivery_status='SUPPRESSED'
       AND EXISTS (SELECT 1 FROM campaigns c
                   WHERE c.id=notifications.related_entity_id AND c.status='CANCELLED'));
