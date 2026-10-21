BEGIN;
SELECT set_config('app.current_role','DOCTOR',true),set_config('app.current_user_id','22222222-2222-2222-2222-222222222222',true);
-- The preceding synthetic callback has already activated the appointment.
SELECT doctor_complete_consultation('aaaaaaaa-4444-4444-4444-444444444444','COMPLETED_NO_RX');
COMMIT;
