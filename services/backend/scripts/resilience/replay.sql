BEGIN;
INSERT INTO payment_events(transaction_id,gateway,provider_event_id,provider_event_type,event_type)
SELECT id,gateway,'SYNTH-EVENT-1','PAYMENT_SUCCESS','PAYMENT_SUCCESS' FROM transactions
WHERE appointment_id='aaaaaaaa-4444-4444-4444-444444444444' AND payment_status<>'FAILED'
ON CONFLICT (gateway,provider_event_id) DO NOTHING;
UPDATE transactions SET payment_status='PAYMENT_HELD',gateway_reference='SYNTHETIC-PROVIDER-REFERENCE'
WHERE appointment_id='aaaaaaaa-4444-4444-4444-444444444444' AND payment_status='INITIATED' AND gateway_reference IS NULL;
COMMIT;
