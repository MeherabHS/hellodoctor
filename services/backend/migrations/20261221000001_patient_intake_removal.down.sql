REVOKE SELECT, UPDATE ON intake_document_removal_jobs FROM hellodoctor_system_worker;
REVOKE INSERT ON intake_document_removal_jobs FROM hellodoctor_api;
DROP TABLE intake_document_removal_jobs;

DROP POLICY IF EXISTS intake_document_patient_remove ON prescription_intake_documents;
REVOKE DELETE ON prescription_intake_documents FROM hellodoctor_api;
