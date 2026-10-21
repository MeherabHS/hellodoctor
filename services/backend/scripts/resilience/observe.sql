\set QUIET on
-- Independent observer: grade committed tables, never worker exit/success counts.
SELECT set_config('resilience.phase', :'phase', false);
DO $$
DECLARE p text := current_setting('resilience.phase'); active_count int; confirmed int; settled int; events int; amount numeric; winner uuid;
BEGIN
 SELECT count(*) INTO active_count FROM transactions WHERE appointment_id='aaaaaaaa-4444-4444-4444-444444444444' AND payment_status<>'FAILED';
 IF active_count<>1 THEN RAISE EXCEPTION 'DEFENSE_FAIL phase=% active=%',p,active_count; END IF;
 IF (SELECT count(*) FROM transactions WHERE appointment_id='aaaaaaaa-4444-4444-4444-444444444444' AND payment_status='FAILED')<>2 THEN RAISE EXCEPTION 'Historical failed attempts lost'; END IF;
 SELECT count(*) INTO confirmed FROM financial_economic_effects WHERE effect_type='PAYMENT_CONFIRMED';
 SELECT count(*) INTO settled FROM financial_economic_effects WHERE effect_type='CONSULTATION_SETTLED';
 SELECT count(*) INTO events FROM payment_events;
 IF p IN ('confirmation','replay','settlement','approval','payout') AND (confirmed<>1 OR events<>100) THEN RAISE EXCEPTION 'DEFENSE_FAIL confirmations=% events=%',confirmed,events; END IF;
 IF p IN ('settlement','approval','payout') THEN
   SELECT lifetime_net INTO amount FROM doctor_wallets WHERE doctor_id='aaaaaaaa-2222-2222-2222-222222222222';
   IF settled<>1 OR amount IS DISTINCT FROM 80 THEN RAISE EXCEPTION 'DEFENSE_FAIL settlements=% wallet=%',settled,amount; END IF;
 END IF;
 IF p IN ('approval','payout') THEN
   SELECT approved_by INTO winner FROM disbursement_items WHERE id='aaaaaaaa-6666-6666-6666-666666666666';
   IF winner IS NULL OR winner NOT IN ('33333333-3333-3333-3333-333333333333'::uuid,'44444444-4444-4444-4444-444444444444'::uuid) THEN RAISE EXCEPTION 'Missing winning Admin identity'; END IF;
 END IF;
 IF p='payout' AND (SELECT count(*) FROM financial_economic_effects WHERE effect_type='PAYOUT_PAID')<>1 THEN RAISE EXCEPTION 'DEFENSE_FAIL payout effect count'; END IF;
 RAISE NOTICE 'case=% requests=100 active_obligations=% confirmations=% settlements=% provider_events=% winner=% verdict=DEFENSE_PASS',p,active_count,confirmed,settled,events,winner;
END $$;
