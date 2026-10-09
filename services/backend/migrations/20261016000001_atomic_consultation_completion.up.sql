-- A narrowly authorized operation keeps clinical completion, settlement and
-- exactly-once wallet credit atomic without broadening any role's table grants.
CREATE FUNCTION doctor_complete_consultation(p_appointment UUID,p_outcome TEXT)
RETURNS TEXT LANGUAGE plpgsql SECURITY DEFINER SET search_path=public AS $$
DECLARE appointment_row appointments%ROWTYPE;
        payment_row transactions%ROWTYPE;
        actor UUID := NULLIF(current_setting('app.current_user_id',true),'')::uuid;
        completed_time TIMESTAMPTZ;
BEGIN
    IF current_setting('app.current_role',true) <> 'DOCTOR' OR actor IS NULL THEN RETURN 'FORBIDDEN'; END IF;
    IF p_outcome NOT IN ('COMPLETED_WITH_RX','COMPLETED_NO_RX','REFERRED','ESCALATED') THEN RETURN 'INVALID_OUTCOME'; END IF;
    SELECT a.* INTO appointment_row FROM appointments a JOIN doctor_profiles d ON d.id=a.doctor_id WHERE a.id=p_appointment AND d.user_id=actor;
    IF NOT FOUND THEN RETURN 'NOT_FOUND'; END IF;
    IF NOT EXISTS(SELECT 1 FROM doctor_profiles d JOIN users u ON u.id=d.user_id
        WHERE d.id=appointment_row.doctor_id AND d.is_verified AND u.is_active AND u.deleted_at IS NULL) THEN RETURN 'FORBIDDEN'; END IF;
    SELECT * INTO payment_row FROM transactions WHERE appointment_id=p_appointment;
    IF NOT FOUND THEN RETURN 'PAYMENT_NOT_CONFIRMED'; END IF;
    PERFORM pg_advisory_xact_lock(61012,EXTRACT(YEAR FROM payment_row.created_at)::integer*100+EXTRACT(MONTH FROM payment_row.created_at)::integer);
    SELECT * INTO appointment_row FROM appointments WHERE id=p_appointment FOR UPDATE;
    SELECT * INTO payment_row FROM transactions WHERE appointment_id=p_appointment FOR UPDATE;
    IF appointment_row.status NOT IN ('CONFIRMED','IN_CONSULTATION','COMPLETED') THEN RETURN 'INVALID_STATE'; END IF;
    IF payment_row.payment_status NOT IN ('PAYMENT_HELD','SETTLED_TO_DOCTOR','DISBURSED','REFUND_PENDING','REFUNDED_TO_PATIENT') THEN RETURN 'PAYMENT_NOT_CONFIRMED'; END IF;
    IF appointment_row.status='COMPLETED' AND appointment_row.clinical_outcome IS DISTINCT FROM p_outcome THEN RETURN 'INVALID_STATE'; END IF;
    IF payment_row.payment_status='PAYMENT_HELD' AND EXISTS(SELECT 1 FROM financial_periods WHERE currency=payment_row.currency
        AND period_year=EXTRACT(YEAR FROM payment_row.created_at) AND period_month=EXTRACT(MONTH FROM payment_row.created_at) AND status='FINALIZED') THEN RETURN 'PERIOD_CLOSED'; END IF;
    completed_time := COALESCE(appointment_row.completed_at,NOW());
    UPDATE appointments SET status='COMPLETED',clinical_outcome=p_outcome,completed_at=completed_time,updated_at=NOW() WHERE id=p_appointment;
    UPDATE consultation_sessions SET status='COMPLETED',ended_at=COALESCE(ended_at,completed_time) WHERE appointment_id=p_appointment;
    IF payment_row.payment_status='PAYMENT_HELD' THEN
        UPDATE transactions SET payment_status='SETTLED_TO_DOCTOR',settled_at=NOW() WHERE id=payment_row.id;
        INSERT INTO doctor_wallets (doctor_id,lifetime_gross,lifetime_fee_withheld,lifetime_net,pending_disbursement,updated_at)
          VALUES (appointment_row.doctor_id,payment_row.gross_amount,payment_row.platform_fee_amount,payment_row.net_amount,payment_row.net_amount,NOW())
          ON CONFLICT (doctor_id) DO UPDATE SET lifetime_gross=doctor_wallets.lifetime_gross+EXCLUDED.lifetime_gross,
              lifetime_fee_withheld=doctor_wallets.lifetime_fee_withheld+EXCLUDED.lifetime_fee_withheld,
              lifetime_net=doctor_wallets.lifetime_net+EXCLUDED.lifetime_net,
              pending_disbursement=doctor_wallets.pending_disbursement+EXCLUDED.pending_disbursement,updated_at=NOW();
    END IF;
    RETURN 'COMPLETED';
END $$;
REVOKE ALL ON FUNCTION doctor_complete_consultation(UUID,TEXT) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION doctor_complete_consultation(UUID,TEXT) TO hellodoctor_api;
