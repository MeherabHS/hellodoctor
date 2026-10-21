DO $$
DECLARE rejected boolean := false;
BEGIN
 BEGIN
   UPDATE transactions SET payment_status='FAILED' WHERE appointment_id='aaaaaaaa-4444-4444-4444-444444444444' AND payment_status='SETTLED_TO_DOCTOR';
 EXCEPTION WHEN check_violation THEN rejected := true; END;
 IF NOT rejected THEN RAISE EXCEPTION 'DEFENSE_FAIL settled payment regressed'; END IF;
 rejected := false;
 BEGIN
   UPDATE transactions SET gateway_reference='ATTACKER-CHANGED' WHERE appointment_id='aaaaaaaa-4444-4444-4444-444444444444' AND payment_status='SETTLED_TO_DOCTOR';
 EXCEPTION WHEN check_violation THEN rejected := true; END;
 IF NOT rejected THEN RAISE EXCEPTION 'DEFENSE_FAIL provider reference changed'; END IF;
 RAISE NOTICE 'case=settled_regression_and_reference_change verdict=DEFENSE_PASS';
END $$;
