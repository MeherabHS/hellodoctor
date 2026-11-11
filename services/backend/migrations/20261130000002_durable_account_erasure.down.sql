DO $$ BEGIN
 IF EXISTS(SELECT 1 FROM account_erasure_jobs WHERE state<>'COMPLETED') THEN
  RAISE EXCEPTION 'Pending erasures must be resolved before rollback';
 END IF;
END $$;
DROP POLICY patient_erasure_frozen ON patient_profiles;
DROP POLICY doctor_erasure_frozen ON doctor_profiles;
DROP TABLE account_erasure_jobs;
DROP FUNCTION protect_erasure_job();
