ALTER TABLE prescription_intake_documents
    DROP CONSTRAINT prescription_intake_documents_page_number_check;

ALTER TABLE prescription_intake_documents
    ADD CONSTRAINT prescription_intake_documents_page_number_check
        CHECK (page_number >= 1 AND page_number <= 5);
