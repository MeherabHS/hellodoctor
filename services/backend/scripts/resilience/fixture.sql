-- Synthetic-only principals, no password/provider credentials, no PHI.
INSERT INTO users(id,phone_number,email,role) VALUES
('11111111-1111-1111-1111-111111111111','+254799000001','patient@example.invalid','PATIENT'),
('22222222-2222-2222-2222-222222222222','+254799000002','doctor@example.invalid','DOCTOR'),
('33333333-3333-3333-3333-333333333333','+254799000003','admin-a@example.invalid','ADMIN'),
('44444444-4444-4444-4444-444444444444','+254799000004','admin-b@example.invalid','ADMIN');
INSERT INTO patient_profiles(id,user_id,full_name,date_of_birth,gender) VALUES
('aaaaaaaa-1111-1111-1111-111111111111','11111111-1111-1111-1111-111111111111','Synthetic Patient','1990-01-01','OTHER');
INSERT INTO doctor_profiles(id,user_id,full_name,license_number,license_authority,license_country,primary_specialty,experience_years,current_hospital,consultation_fee_video,consultation_fee_chat,residential_address,verified_phone,is_verified,professional_verification_status)
VALUES('aaaaaaaa-2222-2222-2222-222222222222','22222222-2222-2222-2222-222222222222','Synthetic Doctor','SYNTH-RESILIENCE','KMPDC','KE','General',1,'Synthetic Facility',100,100,'Synthetic','+254799000002',true,'VERIFIED');
INSERT INTO doctor_schedule_slots(id,doctor_id,start_time,end_time,status) VALUES
('aaaaaaaa-3333-3333-3333-333333333333','aaaaaaaa-2222-2222-2222-222222222222',now()+interval '1 hour',now()+interval '2 hour','LOCKED_IN_PAYMENT');
INSERT INTO appointments(id,appointment_number,patient_id,doctor_id,slot_id,modality,status,consultation_fee) VALUES
('aaaaaaaa-4444-4444-4444-444444444444','SYNTHETIC-LOCAL-ONLY','aaaaaaaa-1111-1111-1111-111111111111','aaaaaaaa-2222-2222-2222-222222222222','aaaaaaaa-3333-3333-3333-333333333333','CHAT','PENDING_PAYMENT',100);
INSERT INTO transactions(transaction_number,appointment_id,payment_session_id,gateway,gross_amount,platform_fee_amount,net_amount,payment_status,currency)
SELECT 'SYNTH-HISTORY-'||n,'aaaaaaaa-4444-4444-4444-444444444444',gen_random_uuid(),'MPESA',100,20,80,'FAILED','KES' FROM generate_series(1,2)n;
INSERT INTO disbursement_batches(id,batch_number,initiated_by,period_start,period_end,total_doctors,total_gross,total_platform_fee,total_net_disbursed,status) VALUES
('aaaaaaaa-5555-5555-5555-555555555555','SYNTHETIC-BATCH','33333333-3333-3333-3333-333333333333','2026-01-01','2026-01-31',1,100,20,80,'PENDING_APPROVAL');
INSERT INTO disbursement_items(id,batch_id,doctor_id,gross_amount,platform_fee,net_amount,gateway,status,currency,provider_code) VALUES
('aaaaaaaa-6666-6666-6666-666666666666','aaaaaaaa-5555-5555-5555-555555555555','aaaaaaaa-2222-2222-2222-222222222222',100,20,80,'MPESA','PENDING_APPROVAL','KES','SYNTHETIC_LOCAL');
