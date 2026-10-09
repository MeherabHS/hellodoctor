DROP POLICY statement_transactions_finance_worker_select ON monthly_statement_transactions;
DROP POLICY statement_transactions_owner_or_admin_read ON monthly_statement_transactions;

CREATE POLICY statement_transactions_read ON monthly_statement_transactions FOR SELECT
    USING (EXISTS (SELECT 1 FROM monthly_payout_statements s WHERE s.id = statement_id));
