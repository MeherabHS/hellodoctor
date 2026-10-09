-- Client incidents reuse the existing operational incident table and lifecycle.
ALTER TABLE system_error_logs ADD COLUMN client_context JSONB;
