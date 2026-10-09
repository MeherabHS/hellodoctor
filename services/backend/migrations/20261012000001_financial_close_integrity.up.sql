-- Audit correction: enforce financial ownership, immutable closes and statement allocation.
ALTER TABLE financial_periods FORCE ROW LEVEL SECURITY;
ALTER TABLE monthly_payout_statements FORCE ROW LEVEL SECURITY;
ALTER TABLE financial_adjustments FORCE ROW LEVEL SECURITY;
ALTER TABLE doctor_payout_destinations FORCE ROW LEVEL SECURITY;

CREATE POLICY periods_doctor_read ON financial_periods FOR SELECT TO hellodoctor_api
    USING (current_setting('app.current_role', true) = 'DOCTOR');
CREATE POLICY periods_finance_write ON financial_periods FOR ALL TO hellodoctor_finance_worker
    USING (current_setting('app.current_role', true) IN ('FINANCE_ADMIN','PLATFORM_ADMIN'))
    WITH CHECK (current_setting('app.current_role', true) IN ('FINANCE_ADMIN','PLATFORM_ADMIN'));
CREATE POLICY statements_finance_write ON monthly_payout_statements FOR ALL TO hellodoctor_finance_worker
    USING (current_setting('app.current_role', true) IN ('FINANCE_ADMIN','PLATFORM_ADMIN'))
    WITH CHECK (current_setting('app.current_role', true) IN ('FINANCE_ADMIN','PLATFORM_ADMIN'));
CREATE POLICY adjustments_doctor_read ON financial_adjustments FOR SELECT TO hellodoctor_api
    USING (doctor_id IN (SELECT id FROM doctor_profiles WHERE user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid));
CREATE POLICY adjustments_finance_insert ON financial_adjustments FOR INSERT TO hellodoctor_finance_worker
    WITH CHECK (current_setting('app.current_role', true) IN ('FINANCE_ADMIN','PLATFORM_ADMIN') AND created_by = NULLIF(current_setting('app.current_user_id', true), '')::uuid);
GRANT SELECT ON financial_adjustments TO hellodoctor_api;
REVOKE UPDATE ON financial_adjustments FROM hellodoctor_finance_worker;

-- Source membership is frozen along with the statement, not recomputed at batch preparation.
CREATE TABLE monthly_statement_transactions (
    statement_id UUID NOT NULL REFERENCES monthly_payout_statements(id) ON DELETE RESTRICT,
    transaction_id UUID NOT NULL UNIQUE REFERENCES transactions(id) ON DELETE RESTRICT,
    gross_amount NUMERIC(12,2) NOT NULL CHECK (gross_amount >= 0),
    platform_charge NUMERIC(12,2) NOT NULL CHECK (platform_charge >= 0),
    doctor_earning NUMERIC(12,2) NOT NULL CHECK (doctor_earning >= 0),
    PRIMARY KEY (statement_id, transaction_id),
    CHECK (gross_amount = platform_charge + doctor_earning)
);
ALTER TABLE monthly_statement_transactions ENABLE ROW LEVEL SECURITY;
ALTER TABLE monthly_statement_transactions FORCE ROW LEVEL SECURITY;
CREATE POLICY statement_transactions_read ON monthly_statement_transactions FOR SELECT
    USING (EXISTS (SELECT 1 FROM monthly_payout_statements s WHERE s.id=statement_id));
CREATE POLICY statement_transactions_insert ON monthly_statement_transactions FOR INSERT TO hellodoctor_finance_worker
    WITH CHECK (current_setting('app.current_role', true) IN ('FINANCE_ADMIN','PLATFORM_ADMIN'));
GRANT SELECT ON monthly_statement_transactions TO hellodoctor_api, hellodoctor_finance_worker;
GRANT INSERT ON monthly_statement_transactions TO hellodoctor_finance_worker;

ALTER TABLE disbursement_items ADD COLUMN statement_id UUID REFERENCES monthly_payout_statements(id);
ALTER TABLE disbursement_items ADD COLUMN adjustments NUMERIC(12,2) NOT NULL DEFAULT 0;
ALTER TABLE disbursement_items ADD COLUMN provider_code VARCHAR(40);
CREATE UNIQUE INDEX uq_active_statement_disbursement ON disbursement_items(statement_id)
    WHERE statement_id IS NOT NULL AND status <> 'CANCELLED';
ALTER TABLE disbursement_items ADD CONSTRAINT statement_destination_ownership
    CHECK (statement_id IS NULL OR (currency='KES' AND net_amount >= 0));

-- A finalized statement cannot be overwritten, even by a future service regression.
CREATE FUNCTION protect_finalized_statement() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.status='FINALIZED' THEN
        RAISE EXCEPTION 'Finalized statement is immutable' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER immutable_finalized_statement BEFORE UPDATE OR DELETE ON monthly_payout_statements
    FOR EACH ROW EXECUTE FUNCTION protect_finalized_statement();

-- Insert and close share the same transaction lock; late adjustments cannot change an issued month.
CREATE FUNCTION protect_closed_adjustment_period() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    PERFORM pg_advisory_xact_lock(61012, NEW.period_year * 100 + NEW.period_month);
    IF EXISTS (SELECT 1 FROM financial_periods WHERE currency=NEW.currency
        AND period_year=NEW.period_year AND period_month=NEW.period_month AND status='FINALIZED') THEN
        RAISE EXCEPTION 'Adjustments require an open financial period' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER adjustment_open_period BEFORE INSERT ON financial_adjustments
    FOR EACH ROW EXECUTE FUNCTION protect_closed_adjustment_period();

-- Freeze Doctor, amounts and destination as soon as the prepared item exists.
CREATE FUNCTION protect_disbursement_snapshot() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF ROW(OLD.doctor_id, OLD.statement_id, OLD.currency, OLD.gross_amount, OLD.platform_fee,
        OLD.net_amount, OLD.adjustments, OLD.payout_destination_id, OLD.provider_code,
        OLD.provider_reference_snapshot, OLD.masked_destination_snapshot)
       IS DISTINCT FROM ROW(NEW.doctor_id, NEW.statement_id, NEW.currency, NEW.gross_amount,
        NEW.platform_fee, NEW.net_amount, NEW.adjustments, NEW.payout_destination_id,
        NEW.provider_code, NEW.provider_reference_snapshot, NEW.masked_destination_snapshot) THEN
        RAISE EXCEPTION 'Prepared disbursement snapshot is immutable' USING ERRCODE='23514';
    END IF;
    IF NEW.status IN ('PAID','CONFIRMED','DISBURSED') AND
       (NEW.gateway_reference IS NULL OR NEW.paid_at IS NULL OR NEW.provider_code='DEV_SIMULATED') THEN
        RAISE EXCEPTION 'Authoritative provider confirmation is required' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER immutable_disbursement_snapshot BEFORE UPDATE ON disbursement_items
    FOR EACH ROW EXECUTE FUNCTION protect_disbursement_snapshot();
