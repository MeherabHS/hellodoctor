DROP TABLE patient_data_requests;
-- Acceptance evidence is immutable: retain referenced policy versions on rollback.
UPDATE legal_documents SET is_current=false WHERE document_type IN ('PATIENT_TERMS','PATIENT_PRIVACY_NOTICE','PATIENT_DATA_RIGHTS');
