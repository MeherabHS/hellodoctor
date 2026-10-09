-- Restrict clinical intake to the existing authorized history window.
DROP POLICY appointment_clinical_intake_access ON appointment_clinical_intake;
CREATE POLICY appointment_clinical_intake_access ON appointment_clinical_intake
USING (
    current_setting('app.current_role',true) IN ('PLATFORM_ADMIN','CLINICAL_ADMIN','COMPLIANCE')
    OR EXISTS (SELECT 1 FROM appointments a JOIN patient_profiles p ON p.id=a.patient_id
        WHERE a.id=appointment_clinical_intake.appointment_id
        AND p.user_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid)
    OR doctor_history_context_allowed(appointment_id,NULLIF(current_setting('app.current_user_id',true),'')::uuid)
);
DROP POLICY health_answers_doctor_write ON health_answers;
CREATE POLICY health_answers_doctor_write ON health_answers FOR INSERT TO hellodoctor_api
    WITH CHECK (current_setting('app.current_role',true)='DOCTOR' AND doctor_id IN
        (SELECT id FROM doctor_profiles WHERE user_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid AND is_verified));

-- Ownership must agree across the statement, destination and obligation.
ALTER TABLE doctor_payout_destinations ADD CONSTRAINT payout_destination_owner_identity UNIQUE(id,doctor_id,currency);
CREATE UNIQUE INDEX unique_bound_provider_reference ON doctor_payout_destinations(provider_code,provider_reference);
ALTER TABLE monthly_payout_statements ADD CONSTRAINT statement_owner_identity UNIQUE(id,doctor_id,currency);
ALTER TABLE disbursement_items ADD CONSTRAINT disbursement_destination_owner
    FOREIGN KEY (payout_destination_id,doctor_id,currency) REFERENCES doctor_payout_destinations(id,doctor_id,currency);
ALTER TABLE disbursement_items ADD CONSTRAINT disbursement_statement_owner
    FOREIGN KEY (statement_id,doctor_id,currency) REFERENCES monthly_payout_statements(id,doctor_id,currency);
ALTER TABLE disbursement_items ADD CONSTRAINT disbursement_kenya_currency CHECK (currency='KES');

CREATE FUNCTION protect_finalized_period() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.status='FINALIZED' THEN
        RAISE EXCEPTION 'A finalized accounting period cannot be silently changed' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER finalized_period_immutable BEFORE UPDATE OR DELETE ON financial_periods
    FOR EACH ROW EXECUTE FUNCTION protect_finalized_period();
ALTER TABLE financial_periods ADD CONSTRAINT financial_period_valid_status CHECK(status IN ('OPEN','FINALIZING','FINALIZED'));
ALTER TABLE financial_periods ADD CONSTRAINT finalized_period_evidence CHECK(status<>'FINALIZED' OR (finalized_at IS NOT NULL AND finalized_by IS NOT NULL));
ALTER TABLE monthly_payout_statements ADD CONSTRAINT statement_balanced_totals CHECK(final_payable=gross_eligible-platform_charge+adjustments);
ALTER TABLE disbursement_items ADD CONSTRAINT disbursement_paid_evidence CHECK(status NOT IN ('PAID','CONFIRMED','DISBURSED') OR
    (paid_at IS NOT NULL AND gateway_reference IS NOT NULL AND provider_code IS NOT NULL AND provider_code<>'DEV_SIMULATED'));
