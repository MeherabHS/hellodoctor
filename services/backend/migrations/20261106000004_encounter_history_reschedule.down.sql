DROP TABLE appointment_reschedules;
DO $$ DECLARE r RECORD; BEGIN
 FOR r IN SELECT * FROM encounter_policy_rollback WHERE kind='POLICY' LOOP EXECUTE 'DROP POLICY '||r.identity;EXECUTE r.definition;END LOOP;
 FOR r IN SELECT * FROM encounter_policy_rollback WHERE kind='FUNCTION' LOOP EXECUTE r.definition;END LOOP;
END $$;
DROP TABLE encounter_policy_rollback;
DROP FUNCTION doctor_encounter_patient_name(UUID);
DROP FUNCTION doctor_own_encounter_allowed(UUID,UUID);
