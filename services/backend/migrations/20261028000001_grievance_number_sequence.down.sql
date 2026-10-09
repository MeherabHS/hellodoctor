ALTER TABLE grievance_reports ALTER COLUMN grievance_number DROP DEFAULT;
REVOKE USAGE ON SEQUENCE grievance_number_seq FROM hellodoctor_api;
DROP SEQUENCE grievance_number_seq;
