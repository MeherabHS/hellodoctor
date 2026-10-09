-- Widen the Patient pre-consultation intake image cap from 5 to 20 images per
-- consultation (appointment). Each image independently fits a fixed 40 KB
-- compression ceiling enforced in application code
-- (upload_security_service.rs::MAX_PRESCRIPTION_IMAGE_BYTES); this migration
-- only widens the database-level page-count CHECK constraint to match the
-- new application-level PATIENT_MAX_IMAGES_PER_CONSULTATION cap.
--
-- The Doctor's own Rx page_number constraint on the separate `prescriptions`
-- table (added in 20261014000001_rx_pages_document_authorization) is
-- deliberately NOT touched here — the Doctor's 5-page-per-appointment cap is
-- out of scope for this change.
ALTER TABLE prescription_intake_documents
    DROP CONSTRAINT prescription_intake_documents_page_number_check;

ALTER TABLE prescription_intake_documents
    ADD CONSTRAINT prescription_intake_documents_page_number_check
        CHECK (page_number >= 1 AND page_number <= 20);
