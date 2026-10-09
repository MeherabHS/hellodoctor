-- Preserve existing completion timestamps; never manufacture a new grace window.
UPDATE consultation_sessions cs SET ended_at=a.completed_at
FROM appointments a WHERE a.id=cs.appointment_id AND a.status='COMPLETED'
AND cs.ended_at IS NULL AND a.completed_at IS NOT NULL;

CREATE FUNCTION doctor_history_context_allowed(p_appointment UUID, p_user UUID)
RETURNS BOOLEAN LANGUAGE sql SECURITY DEFINER SET search_path=public STABLE AS $$
 SELECT EXISTS (
   SELECT 1 FROM appointments a JOIN doctor_profiles d ON d.id=a.doctor_id
   JOIN users u ON u.id=d.user_id JOIN doctor_schedule_slots s ON s.id=a.slot_id
   LEFT JOIN consultation_sessions cs ON cs.appointment_id=a.id
   WHERE a.id=p_appointment AND d.user_id=p_user
   AND p_user=NULLIF(current_setting('app.current_user_id',true),'')::uuid
   AND d.deleted_at IS NULL AND d.is_verified AND u.is_active AND u.deleted_at IS NULL
   AND CURRENT_TIMESTAMP >= s.start_time - INTERVAL '15 minutes'
   AND ( (a.status IN ('CONFIRMED','WAITING','IN_CONSULTATION') AND cs.ended_at IS NULL)
      OR (a.status='COMPLETED' AND COALESCE(cs.ended_at,a.completed_at) IS NOT NULL
          AND CURRENT_TIMESTAMP < COALESCE(cs.ended_at,a.completed_at) + INTERVAL '24 hours') )
 )
$$;
REVOKE ALL ON FUNCTION doctor_history_context_allowed(UUID,UUID) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION doctor_history_context_allowed(UUID,UUID) TO hellodoctor_api;

CREATE OR REPLACE FUNCTION doctor_patient_history(
 p_current_appointment UUID, p_doctor_user UUID, p_limit BIGINT, p_offset BIGINT
) RETURNS SETOF JSONB LANGUAGE sql SECURITY DEFINER SET search_path=public STABLE AS $$
 SELECT jsonb_build_object('historical_appointment_id',a.id,'historical_consultation_id',cs.id,
 'date',s.start_time,'doctor_name',d.full_name,'specialty',d.primary_specialty,
 'presenting_problem',i.chief_complaint,'status',a.status,
 'prescription_available',EXISTS(SELECT 1 FROM prescriptions rx WHERE rx.appointment_id=a.id),
 'document_count',(SELECT COUNT(*) FROM prescription_intake_documents doc WHERE doc.appointment_id=a.id))
 FROM appointments current_a JOIN doctor_schedule_slots anchor ON anchor.id=current_a.slot_id
 JOIN appointments a ON a.patient_id=current_a.patient_id AND a.id<>current_a.id
 JOIN doctor_schedule_slots s ON s.id=a.slot_id AND s.start_time<anchor.start_time
 JOIN doctor_profiles d ON d.id=a.doctor_id
 LEFT JOIN consultation_sessions cs ON cs.appointment_id=a.id
 LEFT JOIN appointment_clinical_intake i ON i.appointment_id=a.id
 WHERE current_a.id=p_current_appointment
 AND doctor_history_context_allowed(p_current_appointment,p_doctor_user)
 ORDER BY s.start_time DESC,a.id LIMIT GREATEST(p_limit,1) OFFSET GREATEST(p_offset,0)
$$;

CREATE OR REPLACE FUNCTION doctor_patient_history_detail(
 p_current_appointment UUID,p_doctor_user UUID,p_historical_consultation UUID
) RETURNS JSONB LANGUAGE sql SECURITY DEFINER SET search_path=public STABLE AS $$
 SELECT jsonb_build_object('historical_appointment_id',a.id,'historical_consultation_id',cs.id,
 'date',s.start_time,'doctor_name',d.full_name,'specialty',d.primary_specialty,
 'presenting_problem',i.chief_complaint,'clinical_notes',i.clinical_notes,'status',a.status,
 'prescription',(SELECT to_jsonb(rx)-'integrity_verification_hash'-'rx_image_object_bucket'-'rx_image_object_key' FROM prescriptions rx WHERE rx.appointment_id=a.id),
 'documents',COALESCE((SELECT jsonb_agg(jsonb_build_object('id',doc.id,'page_number',doc.page_number,'mime_type',doc.mime_type,'security_status',doc.security_status,'created_at',doc.created_at) ORDER BY doc.page_number) FROM prescription_intake_documents doc WHERE doc.appointment_id=a.id),'[]'::jsonb))
 FROM appointments current_a JOIN doctor_schedule_slots anchor ON anchor.id=current_a.slot_id
 JOIN appointments a ON a.patient_id=current_a.patient_id
 JOIN doctor_schedule_slots s ON s.id=a.slot_id
 JOIN consultation_sessions cs ON cs.appointment_id=a.id AND cs.id=p_historical_consultation
 JOIN doctor_profiles d ON d.id=a.doctor_id
 LEFT JOIN appointment_clinical_intake i ON i.appointment_id=a.id
 WHERE current_a.id=p_current_appointment
 AND (a.id=current_a.id OR s.start_time<anchor.start_time)
 AND doctor_history_context_allowed(p_current_appointment,p_doctor_user) LIMIT 1
$$;
REVOKE ALL ON FUNCTION doctor_patient_history(UUID,UUID,BIGINT,BIGINT) FROM PUBLIC;
REVOKE ALL ON FUNCTION doctor_patient_history_detail(UUID,UUID,UUID) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION doctor_patient_history(UUID,UUID,BIGINT,BIGINT) TO hellodoctor_api;
GRANT EXECUTE ON FUNCTION doctor_patient_history_detail(UUID,UUID,UUID) TO hellodoctor_api;
