CREATE TABLE health_questions (
    id UUID PRIMARY KEY,
    author_patient_id UUID REFERENCES patient_profiles(id) ON DELETE SET NULL,
    question_text TEXT NOT NULL CHECK (char_length(question_text) BETWEEN 1 AND 4000),
    category VARCHAR(80),
    status VARCHAR(20) NOT NULL DEFAULT 'OPEN' CHECK (status IN ('OPEN','ANSWERED','CLOSED')),
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE health_answers (
    id UUID PRIMARY KEY,
    question_id UUID NOT NULL REFERENCES health_questions(id) ON DELETE CASCADE,
    doctor_id UUID NOT NULL REFERENCES doctor_profiles(id) ON DELETE RESTRICT,
    answer_text TEXT NOT NULL CHECK (char_length(answer_text) BETWEEN 1 AND 4000),
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    UNIQUE (question_id)
);

CREATE INDEX idx_health_questions_status_created ON health_questions(status, created_at DESC);
CREATE INDEX idx_health_answers_question ON health_answers(question_id);

GRANT SELECT ON health_questions, health_answers TO hellodoctor_api;
GRANT INSERT, UPDATE ON health_answers TO hellodoctor_api;
GRANT UPDATE ON health_questions TO hellodoctor_api;

ALTER TABLE health_questions ENABLE ROW LEVEL SECURITY;
ALTER TABLE health_questions FORCE ROW LEVEL SECURITY;
ALTER TABLE health_answers ENABLE ROW LEVEL SECURITY;
ALTER TABLE health_answers FORCE ROW LEVEL SECURITY;

CREATE POLICY health_questions_read ON health_questions FOR SELECT USING (true);
CREATE POLICY health_answers_read ON health_answers FOR SELECT USING (true);
CREATE POLICY health_answers_doctor_write ON health_answers
    FOR INSERT TO hellodoctor_api
    WITH CHECK (doctor_id IN (SELECT id FROM doctor_profiles WHERE user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid));
CREATE POLICY health_questions_status_update ON health_questions
    FOR UPDATE TO hellodoctor_api
    USING (EXISTS (SELECT 1 FROM health_answers a WHERE a.question_id = health_questions.id AND a.doctor_id IN (SELECT id FROM doctor_profiles WHERE user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)))
    WITH CHECK (status IN ('OPEN','ANSWERED','CLOSED'));

-- History is authorized from the current appointment context inside the
-- database function. The function returns summaries only and never exposes
-- financial, audit, telemetry, or grievance data.
CREATE OR REPLACE FUNCTION doctor_patient_history(
    p_current_appointment UUID, p_doctor_user UUID, p_limit BIGINT, p_offset BIGINT
) RETURNS SETOF JSONB
LANGUAGE sql SECURITY DEFINER SET search_path = public STABLE AS $$
    SELECT jsonb_build_object(
        'historical_appointment_id', a.id,
        'historical_consultation_id', cs.id,
        'date', COALESCE(cs.created_at, a.created_at),
        'doctor_name', d.full_name,
        'specialty', d.primary_specialty,
        'presenting_problem', i.chief_complaint,
        'status', a.status,
        'prescription_available', EXISTS (SELECT 1 FROM prescriptions rx WHERE rx.appointment_id = a.id),
        'document_count', (SELECT COUNT(*) FROM prescription_intake_documents doc WHERE doc.appointment_id = a.id)
    )
    FROM appointments current_a
    JOIN appointments a ON a.patient_id = current_a.patient_id AND a.id <> current_a.id
    JOIN doctor_profiles d ON d.id = a.doctor_id
    LEFT JOIN consultation_sessions cs ON cs.appointment_id = a.id
    LEFT JOIN appointment_clinical_intake i ON i.appointment_id = a.id
    WHERE current_a.id = p_current_appointment
      AND current_a.doctor_id IN (SELECT id FROM doctor_profiles WHERE user_id = p_doctor_user)
      AND current_a.status IN ('CONFIRMED','WAITING','IN_CONSULTATION','COMPLETED')
    ORDER BY COALESCE(cs.created_at, a.created_at) DESC
    LIMIT GREATEST(p_limit, 1) OFFSET GREATEST(p_offset, 0)
$$;

CREATE OR REPLACE FUNCTION doctor_patient_history_detail(
    p_current_appointment UUID, p_doctor_user UUID, p_historical_consultation UUID
) RETURNS JSONB
LANGUAGE sql SECURITY DEFINER SET search_path = public STABLE AS $$
    SELECT jsonb_build_object(
        'historical_appointment_id', a.id,
        'historical_consultation_id', cs.id,
        'date', COALESCE(cs.created_at, a.created_at),
        'doctor_name', d.full_name,
        'specialty', d.primary_specialty,
        'presenting_problem', i.chief_complaint,
        'clinical_notes', i.clinical_notes,
        'status', a.status,
        'prescription', (SELECT to_jsonb(rx) - 'integrity_verification_hash' - 'rx_image_object_bucket' - 'rx_image_object_key' FROM prescriptions rx WHERE rx.appointment_id = a.id),
        'documents', COALESCE((SELECT jsonb_agg(jsonb_build_object('id', doc.id, 'page_number', doc.page_number, 'mime_type', doc.mime_type, 'security_status', doc.security_status, 'created_at', doc.created_at) ORDER BY doc.page_number) FROM prescription_intake_documents doc WHERE doc.appointment_id = a.id), '[]'::jsonb)
    )
    FROM appointments current_a
    JOIN appointments a ON a.patient_id = current_a.patient_id
    JOIN consultation_sessions cs ON cs.appointment_id = a.id AND cs.id = p_historical_consultation
    JOIN doctor_profiles d ON d.id = a.doctor_id
    LEFT JOIN appointment_clinical_intake i ON i.appointment_id = a.id
    WHERE current_a.id = p_current_appointment
      AND current_a.doctor_id IN (SELECT id FROM doctor_profiles WHERE user_id = p_doctor_user)
      AND current_a.status IN ('CONFIRMED','WAITING','IN_CONSULTATION','COMPLETED')
    LIMIT 1
$$;

GRANT EXECUTE ON FUNCTION doctor_patient_history(UUID, UUID, BIGINT, BIGINT) TO hellodoctor_api;
GRANT EXECUTE ON FUNCTION doctor_patient_history_detail(UUID, UUID, UUID) TO hellodoctor_api;
