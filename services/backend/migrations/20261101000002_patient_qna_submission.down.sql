-- Reverses 20261101000002_patient_qna_submission.up.sql exactly.

DROP POLICY health_questions_patient_author_guard ON health_questions;
DROP POLICY health_questions_patient_insert ON health_questions;
REVOKE INSERT ON health_questions FROM hellodoctor_api;
