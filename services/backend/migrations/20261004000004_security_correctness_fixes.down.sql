DROP POLICY audit_events_select ON audit_events;
CREATE POLICY audit_events_select ON audit_events FOR SELECT USING (
    current_setting('app.current_role', true) = 'SECURITY_ADMIN'
    OR current_user <> 'hellodoctor_api'
);

ALTER TABLE prescriptions DROP COLUMN security_status;
ALTER TABLE prescription_intake_documents DROP COLUMN security_status;

ALTER TABLE pending_mfa_challenges
    DROP COLUMN attempt_count,
    DROP COLUMN max_attempts;
