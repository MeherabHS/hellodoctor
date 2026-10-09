DROP POLICY audit_events_select ON audit_events;
CREATE POLICY audit_events_select ON audit_events FOR SELECT USING (
    current_setting('app.current_role', true) IN
        ('PLATFORM_ADMIN', 'CLINICAL_ADMIN', 'FINANCE_ADMIN', 'SUPPORT', 'COMPLIANCE', 'SECURITY_ADMIN')
    OR current_user <> 'hellodoctor_api'
);

ALTER TABLE audit_events
    DROP COLUMN before_data,
    DROP COLUMN after_data,
    DROP COLUMN changed_fields;
