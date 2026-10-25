DELETE FROM notification_templates WHERE template_key IN (
    'registration_verified', 'registration_rejected', 'professional_verification_revoked',
    'earning_credited', 'payout_paid', 'article_published', 'article_archived',
    'new_chat_message_doctor', 'new_chat_message_patient',
    'booking_hold_created', 'booking_hold_one_minute', 'booking_hold_expired',
    'daily_consultation_summary', 'consultation_reminder_15m',
    'booking_confirmed_doctor', 'booking_confirmed_patient',
    'booking_rescheduled_doctor', 'booking_rescheduled_patient',
    'prescription_available', 'refund_approved', 'refund_processing', 'refund_completed',
    'consultation_ready', 'consultation_rejoin', 'consultation_interrupted',
    'consultation_needs_review', 'consultation_recovery_resolved_pending',
    'consultation_recovery_resolved_no_settlement', 'consultation_completed'
);

REVOKE SELECT (id, preferred_language) ON users FROM hellodoctor_finance_worker;
DROP POLICY users_finance_worker_language_read ON users;

REVOKE UPDATE (preferred_language) ON users FROM hellodoctor_api;
DROP POLICY users_api_update_own_language ON users;

DROP POLICY notification_type_registry_render_read ON notification_type_registry;
DROP POLICY notification_templates_render_read ON notification_templates;
REVOKE SELECT ON notification_templates, notification_type_registry
    FROM hellodoctor_payment_worker, hellodoctor_finance_worker;

DROP TABLE notification_preferences;

DROP FUNCTION appointment_notification_recipients(UUID);
CREATE FUNCTION appointment_notification_recipients(p_appointment UUID)
RETURNS TABLE(doctor_user_id UUID, patient_user_id UUID)
LANGUAGE sql SECURITY DEFINER SET search_path=public STABLE AS $$
 SELECT d.user_id, COALESCE(p.user_id, g.user_id)
   FROM appointments a
   JOIN doctor_profiles d ON d.id = a.doctor_id
   JOIN patient_profiles p ON p.id = a.patient_id
   LEFT JOIN patient_profiles g ON g.id = p.guardian_patient_id
  WHERE a.id = p_appointment AND consultation_participant_allowed(p_appointment)
$$;
REVOKE ALL ON FUNCTION appointment_notification_recipients(UUID) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION appointment_notification_recipients(UUID)
    TO hellodoctor_api, hellodoctor_payment_worker, hellodoctor_finance_worker;
