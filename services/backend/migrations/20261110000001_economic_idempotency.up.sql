-- Fail migration on conflicting history; never delete/deduplicate money rows.
-- FAILED history remains permissible; a paid/refunded obligation stays occupied.
CREATE UNIQUE INDEX uq_appointment_payable_obligation ON transactions(appointment_id)
    WHERE payment_status <> 'FAILED';
CREATE INDEX idx_transactions_appointment_history ON transactions
    (appointment_id,(payment_status <> 'FAILED') DESC,created_at DESC,id DESC);
CREATE UNIQUE INDEX uq_disbursement_provider_payment ON disbursement_items(provider_code,gateway_reference)
    WHERE gateway_reference IS NOT NULL;

-- Append-only economic evidence. Existing historical counters are not backfilled
-- into invented events. New transitions commit/rollback with their unique event.
CREATE TABLE financial_economic_effects (
    effect_type TEXT NOT NULL CHECK (effect_type IN
        ('PAYMENT_CONFIRMED','CONSULTATION_SETTLED','REFUND_APPROVED','PAYOUT_PAID')),
    business_id UUID NOT NULL,
    transaction_id UUID REFERENCES transactions(id),
    disbursement_item_id UUID REFERENCES disbursement_items(id),
    currency CHAR(3) NOT NULL,
    gross_amount NUMERIC(12,2) NOT NULL,
    platform_fee_amount NUMERIC(12,2) NOT NULL,
    net_amount NUMERIC(12,2) NOT NULL,
    actor_id UUID,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY(effect_type,business_id),
    CHECK ((transaction_id IS NOT NULL) <> (disbursement_item_id IS NOT NULL))
);
ALTER TABLE financial_economic_effects ENABLE ROW LEVEL SECURITY;
ALTER TABLE financial_economic_effects FORCE ROW LEVEL SECURITY;
CREATE POLICY economic_effects_finance_read ON financial_economic_effects FOR SELECT
    USING (current_setting('app.current_role',true)='ADMIN');
-- FORCE RLS also applies to a non-BYPASSRLS migration owner. Only the fixed
-- SECURITY DEFINER triggers run as that owner; application roles get no INSERT.
CREATE POLICY economic_effects_internal_insert ON financial_economic_effects FOR INSERT
    TO CURRENT_USER WITH CHECK (true);
GRANT SELECT ON financial_economic_effects TO hellodoctor_api,hellodoctor_finance_worker;

CREATE FUNCTION record_transaction_economic_effect() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,public AS $$
DECLARE economic_kind TEXT;
BEGIN
    IF OLD.appointment_id IS DISTINCT FROM NEW.appointment_id
       OR (OLD.gateway_reference IS NOT NULL AND OLD.gateway_reference IS DISTINCT FROM NEW.gateway_reference)
       OR OLD.gateway IS DISTINCT FROM NEW.gateway
       OR OLD.payment_session_id IS DISTINCT FROM NEW.payment_session_id
       OR ROW(OLD.currency,OLD.gross_amount,OLD.platform_fee_amount,OLD.net_amount)
          IS DISTINCT FROM ROW(NEW.currency,NEW.gross_amount,NEW.platform_fee_amount,NEW.net_amount) THEN
        RAISE EXCEPTION 'Payment business/provider identity is immutable' USING ERRCODE='23514';
    END IF;
    IF OLD.payment_status=NEW.payment_status THEN RETURN NEW; END IF;
    IF OLD.payment_status NOT IN ('INITIATED','FAILED') AND NEW.payment_status IN ('INITIATED','FAILED','PAYMENT_HELD') THEN
        RAISE EXCEPTION 'Confirmed payment cannot return to a payable state' USING ERRCODE='23514';
    END IF;
    economic_kind := CASE NEW.payment_status
        WHEN 'PAYMENT_HELD' THEN 'PAYMENT_CONFIRMED'
        WHEN 'SETTLED_TO_DOCTOR' THEN 'CONSULTATION_SETTLED'
        WHEN 'REFUND_PENDING' THEN 'REFUND_APPROVED'
        ELSE NULL END;
    IF economic_kind IS NOT NULL THEN
        INSERT INTO public.financial_economic_effects(effect_type,business_id,transaction_id,currency,
            gross_amount,platform_fee_amount,net_amount,actor_id)
        VALUES(economic_kind,NEW.appointment_id,NEW.id,NEW.currency,NEW.gross_amount,
            NEW.platform_fee_amount,NEW.net_amount,NULLIF(current_setting('app.current_user_id',true),'')::uuid);
    END IF;
    RETURN NEW;
END $$;
REVOKE ALL ON FUNCTION record_transaction_economic_effect() FROM PUBLIC;
CREATE TRIGGER transaction_economic_identity BEFORE UPDATE ON transactions
    FOR EACH ROW EXECUTE FUNCTION record_transaction_economic_effect();

CREATE FUNCTION record_disbursement_economic_effect() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,public AS $$
BEGIN
    IF OLD.status='PAID' AND ROW(NEW.status,NEW.gateway_reference,NEW.paid_at)
        IS DISTINCT FROM ROW(OLD.status,OLD.gateway_reference,OLD.paid_at) THEN
        RAISE EXCEPTION 'Paid disbursement is final' USING ERRCODE='23514';
    END IF;
    IF NEW.status='PAID' AND OLD.status<>'PAID' THEN
        INSERT INTO public.financial_economic_effects(effect_type,business_id,disbursement_item_id,
            currency,gross_amount,platform_fee_amount,net_amount,actor_id)
        VALUES('PAYOUT_PAID',NEW.id,NEW.id,NEW.currency,NEW.gross_amount,NEW.platform_fee,NEW.net_amount,
            NULLIF(current_setting('app.current_user_id',true),'')::uuid);
    END IF;
    RETURN NEW;
END $$;
REVOKE ALL ON FUNCTION record_disbursement_economic_effect() FROM PUBLIC;
CREATE TRIGGER disbursement_economic_identity BEFORE UPDATE ON disbursement_items
    FOR EACH ROW EXECUTE FUNCTION record_disbursement_economic_effect();
-- A narrowly authorized operation keeps clinical completion, settlement and
-- exactly-once wallet credit atomic without broadening any role's table grants.
CREATE OR REPLACE FUNCTION doctor_complete_consultation(p_appointment UUID,p_outcome TEXT)
RETURNS TEXT LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,public AS $$
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
    SELECT * INTO payment_row FROM transactions WHERE appointment_id=p_appointment AND payment_status <> 'FAILED';
    IF NOT FOUND THEN RETURN 'PAYMENT_NOT_CONFIRMED'; END IF;
    PERFORM pg_advisory_xact_lock(61012,EXTRACT(YEAR FROM payment_row.created_at)::integer*100+EXTRACT(MONTH FROM payment_row.created_at)::integer);
    SELECT * INTO appointment_row FROM appointments WHERE id=p_appointment FOR UPDATE;
    SELECT * INTO payment_row FROM transactions WHERE appointment_id=p_appointment AND payment_status <> 'FAILED' FOR UPDATE;
    IF appointment_row.status NOT IN ('CONFIRMED','IN_CONSULTATION','COMPLETED') THEN RETURN 'INVALID_STATE'; END IF;
    IF payment_row.payment_status NOT IN ('PAYMENT_HELD','SETTLED_TO_DOCTOR','DISBURSED','REFUND_PENDING','REFUNDED_TO_PATIENT') THEN RETURN 'PAYMENT_NOT_CONFIRMED'; END IF;
    IF appointment_row.status='COMPLETED' AND appointment_row.clinical_outcome IS DISTINCT FROM p_outcome THEN RETURN 'INVALID_STATE'; END IF;
    IF payment_row.payment_status='PAYMENT_HELD' AND EXISTS(SELECT 1 FROM financial_periods WHERE currency=payment_row.currency
        AND period_year=EXTRACT(YEAR FROM payment_row.created_at) AND period_month=EXTRACT(MONTH FROM payment_row.created_at) AND status='FINALIZED') THEN RETURN 'PERIOD_CLOSED'; END IF;
    IF appointment_row.status='COMPLETED' AND payment_row.payment_status<>'PAYMENT_HELD' THEN RETURN 'ALREADY_COMPLETED'; END IF;
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
