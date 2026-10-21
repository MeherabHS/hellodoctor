-- Each attempt has a different session/reference; no request-key deduplication.
INSERT INTO transactions(transaction_number,appointment_id,payment_session_id,gateway,gross_amount,platform_fee_amount,net_amount,payment_status,currency)
VALUES('SYNTH-ATTEMPT-'||:attempt,'aaaaaaaa-4444-4444-4444-444444444444',gen_random_uuid(),'MPESA',100,20,80,'INITIATED','KES')
ON CONFLICT (appointment_id) WHERE payment_status <> 'FAILED' DO NOTHING;
