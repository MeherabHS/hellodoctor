ALTER TABLE prescriptions DROP CONSTRAINT prescriptions_appointment_id_key;
ALTER TABLE prescriptions ADD COLUMN page_number SMALLINT NOT NULL DEFAULT 1 CHECK (page_number BETWEEN 1 AND 5);
ALTER TABLE prescriptions ADD COLUMN mime_type VARCHAR(40);
CREATE UNIQUE INDEX uq_prescription_page ON prescriptions(appointment_id,page_number);

-- Finance and Support have no access to clinical document contents.
DROP POLICY prescriptions_access ON prescriptions;
CREATE POLICY prescriptions_access ON prescriptions USING (
    current_setting('app.current_role',true) IN ('PLATFORM_ADMIN','CLINICAL_ADMIN','COMPLIANCE')
    OR EXISTS (SELECT 1 FROM appointments a LEFT JOIN patient_profiles p ON p.id=a.patient_id
        LEFT JOIN doctor_profiles d ON d.id=a.doctor_id WHERE a.id=prescriptions.appointment_id
        AND (p.user_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid
            OR d.user_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid))
);
DROP POLICY prescription_intake_documents_access ON prescription_intake_documents;
CREATE POLICY prescription_intake_documents_access ON prescription_intake_documents USING (
    current_setting('app.current_role',true) IN ('PLATFORM_ADMIN','CLINICAL_ADMIN','COMPLIANCE')
    OR EXISTS (SELECT 1 FROM appointments a LEFT JOIN patient_profiles p ON p.id=a.patient_id
        LEFT JOIN doctor_profiles d ON d.id=a.doctor_id WHERE a.id=prescription_intake_documents.appointment_id
        AND (p.user_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid
            OR d.user_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid))
);
CREATE POLICY prescriptions_doctor_window ON prescriptions AS RESTRICTIVE FOR SELECT TO hellodoctor_api
    USING (current_setting('app.current_role',true) <> 'DOCTOR' OR doctor_history_context_allowed(appointment_id,NULLIF(current_setting('app.current_user_id',true),'')::uuid));
CREATE POLICY intake_documents_doctor_window ON prescription_intake_documents AS RESTRICTIVE FOR SELECT TO hellodoctor_api
    USING (current_setting('app.current_role',true) <> 'DOCTOR' OR doctor_history_context_allowed(appointment_id,NULLIF(current_setting('app.current_user_id',true),'')::uuid));

-- Historical documents need a current authorized appointment context. This
-- function bypasses ownership only after validating that context and chronology.
CREATE FUNCTION doctor_authorized_document(p_current UUID,p_document_appointment UUID,p_document UUID,p_user UUID)
RETURNS JSONB LANGUAGE sql SECURITY DEFINER SET search_path=public STABLE AS $$
    SELECT document FROM (
      SELECT jsonb_build_object('bucket',doc.object_bucket,'key',doc.object_key,'hash',doc.content_hash,
          'mime',doc.mime_type,'security_status',doc.security_status) document
        FROM prescription_intake_documents doc WHERE doc.id=p_document AND doc.appointment_id=p_document_appointment
      UNION ALL
      SELECT jsonb_build_object('bucket',rx.rx_image_object_bucket,'key',rx.rx_image_object_key,
          'hash',rx.integrity_verification_hash,'mime',rx.mime_type,'security_status',rx.security_status)
        FROM prescriptions rx WHERE rx.id=p_document AND rx.appointment_id=p_document_appointment AND NOT rx.is_no_rx_required
    ) contents
    WHERE current_setting('app.current_role',true)='DOCTOR'
      AND doctor_history_context_allowed(p_current,p_user)
      AND EXISTS (SELECT 1 FROM appointments current_a JOIN doctor_schedule_slots anchor ON anchor.id=current_a.slot_id
        JOIN appointments historical_a ON historical_a.patient_id=current_a.patient_id
        JOIN doctor_schedule_slots historical_s ON historical_s.id=historical_a.slot_id
        WHERE current_a.id=p_current AND historical_a.id=p_document_appointment
          AND (historical_a.id=current_a.id OR historical_s.start_time < anchor.start_time))
    LIMIT 1
$$;
REVOKE ALL ON FUNCTION doctor_authorized_document(UUID,UUID,UUID,UUID) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION doctor_authorized_document(UUID,UUID,UUID,UUID) TO hellodoctor_api;

-- The existing history-detail function expects one prescription subquery.
-- Keep that compatibility field deterministic and expose all pages separately.
DO $$
DECLARE function_sql TEXT;
BEGIN
    SELECT pg_get_functiondef('doctor_patient_history_detail(uuid,uuid,uuid)'::regprocedure) INTO function_sql;
    function_sql := replace(function_sql,
      'FROM prescriptions rx WHERE rx.appointment_id=a.id)',
      'FROM prescriptions rx WHERE rx.appointment_id=a.id ORDER BY rx.page_number LIMIT 1)');
    function_sql := replace(function_sql, '''documents'',COALESCE',
      '''prescription_pages'',COALESCE((SELECT jsonb_agg(to_jsonb(rx)-''integrity_verification_hash''-''rx_image_object_bucket''-''rx_image_object_key'' ORDER BY rx.page_number) FROM prescriptions rx WHERE rx.appointment_id=a.id),''[]''::jsonb),''documents'',COALESCE');
    EXECUTE function_sql;
END $$;
