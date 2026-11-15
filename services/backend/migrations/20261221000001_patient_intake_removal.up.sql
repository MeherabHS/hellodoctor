-- Patients may remove one of their own intake photos (for example the wrong
-- photo) and upload again, but only while the consultation can still take
-- uploads: a live payment hold, or CONFIRMED / WAITING / IN_CONSULTATION.
-- After that the upload is part of the clinical record and stays.
-- Product-owner instruction, 2026-11-14.

-- 1. The API may delete intake rows, restricted to the owning patient
--    (self or guardianed dependent) inside the upload window.
GRANT DELETE ON prescription_intake_documents TO hellodoctor_api;
CREATE POLICY intake_document_patient_remove ON prescription_intake_documents
    AS RESTRICTIVE FOR DELETE TO hellodoctor_api USING (
    current_setting('app.current_role', true) = 'PATIENT'
    AND fn_patient_profile_owned_by_caller(patient_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    AND EXISTS (
        SELECT 1 FROM appointments a
        WHERE a.id = prescription_intake_documents.appointment_id
          AND (a.status IN ('CONFIRMED', 'WAITING', 'IN_CONSULTATION')
               OR (a.status = 'PENDING_PAYMENT' AND a.hold_expires_at > clock_timestamp()))
    )
);

-- 2. Durable queue for deleting the removed photo's stored object. Written in
--    the same transaction as the row delete; drained by the system worker.
CREATE TABLE intake_document_removal_jobs (
    document_id UUID PRIMARY KEY,
    appointment_id UUID NOT NULL REFERENCES appointments(id) ON DELETE RESTRICT,
    object_bucket VARCHAR(100) NOT NULL,
    object_key VARCHAR(500) NOT NULL,
    state VARCHAR(20) NOT NULL DEFAULT 'PENDING'
        CHECK (state IN ('PENDING', 'RUNNING', 'COMPLETED', 'FAILED')),
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    lease_until TIMESTAMPTZ,
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    completed_at TIMESTAMPTZ
);
CREATE INDEX idx_intake_document_removal_due
    ON intake_document_removal_jobs (next_attempt_at, document_id)
    WHERE state <> 'COMPLETED';

ALTER TABLE intake_document_removal_jobs ENABLE ROW LEVEL SECURITY;
ALTER TABLE intake_document_removal_jobs FORCE ROW LEVEL SECURITY;
CREATE POLICY intake_document_removal_jobs_patient_insert ON intake_document_removal_jobs
    FOR INSERT TO hellodoctor_api WITH CHECK (
    current_setting('app.current_role', true) = 'PATIENT'
    AND EXISTS (
        SELECT 1 FROM appointments a
        WHERE a.id = intake_document_removal_jobs.appointment_id
          AND fn_patient_profile_owned_by_caller(a.patient_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    )
);
GRANT INSERT ON intake_document_removal_jobs TO hellodoctor_api;
CREATE POLICY intake_document_removal_jobs_system_worker ON intake_document_removal_jobs
    FOR ALL TO hellodoctor_system_worker USING (true) WITH CHECK (true);
GRANT SELECT, UPDATE ON intake_document_removal_jobs TO hellodoctor_system_worker;
