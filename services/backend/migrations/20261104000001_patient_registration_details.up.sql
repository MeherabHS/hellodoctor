ALTER TABLE patient_auth_challenges ADD COLUMN registration_profile JSONB;
ALTER TYPE legal_document_type_enum ADD VALUE IF NOT EXISTS 'PATIENT_TERMS';
ALTER TYPE legal_document_type_enum ADD VALUE IF NOT EXISTS 'PATIENT_PRIVACY_NOTICE';
ALTER TYPE legal_document_type_enum ADD VALUE IF NOT EXISTS 'PATIENT_DATA_RIGHTS';
