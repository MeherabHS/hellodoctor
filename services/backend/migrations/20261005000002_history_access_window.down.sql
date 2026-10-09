-- Fail closed on rollback; the superseded indefinite-access policy is unsafe.
CREATE OR REPLACE FUNCTION doctor_patient_history(p_current_appointment UUID,p_doctor_user UUID,p_limit BIGINT,p_offset BIGINT)
RETURNS SETOF JSONB LANGUAGE sql AS 'SELECT NULL::jsonb WHERE false';
CREATE OR REPLACE FUNCTION doctor_patient_history_detail(p_current_appointment UUID,p_doctor_user UUID,p_historical_consultation UUID)
RETURNS JSONB LANGUAGE sql AS 'SELECT NULL::jsonb';
DROP FUNCTION doctor_history_context_allowed(UUID,UUID);
