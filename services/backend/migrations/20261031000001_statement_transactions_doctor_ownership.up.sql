-- ============================================================================
-- CRITICAL FIX: `monthly_statement_transactions` had FORCE ROW LEVEL SECURITY
-- enabled (20261012000001_financial_close_integrity) but its only SELECT
-- policy never checked caller identity against the statement's owning
-- doctor:
--
--   CREATE POLICY statement_transactions_read ON monthly_statement_transactions
--       FOR SELECT USING (EXISTS (SELECT 1 FROM monthly_payout_statements s
--                                  WHERE s.id = statement_id));
--
-- That `EXISTS` only proves the parent statement row exists — it is true for
-- EVERY statement, regardless of whose it is. Because the policy carries no
-- `TO <role>` clause, it is a permissive policy that applies to every role
-- sharing the table's GRANTs, including `hellodoctor_api` (GRANTed plain
-- SELECT in the same original migration). Confirmed directly (not just read
-- from the SQL): any `hellodoctor_api` connection — any authenticated
-- patient or doctor — authenticated as anyone, with any `app.current_role`,
-- could read another doctor's `gross_amount` / `platform_charge` /
-- `doctor_earning` line items out of `monthly_statement_transactions` by
-- supplying a `statement_id` that was not their own, with RLS alone doing
-- nothing to stop it. The one real caller of this table
-- (wallet_handlers.rs::get_monthly_statement) happens to resolve
-- `statement_id` itself from a `WHERE doctor_id = <the caller's own doctor
-- id>` lookup first, so the existing HTTP endpoint was not exploitable
-- through that one code path — but RLS, not applicaton-layer luck, is
-- supposed to be the backstop per this project's own defense-in-depth
-- design (see every sibling table in this migration file, which all encode
-- real ownership in their own policies: `statements_owner_or_finance`,
-- `adjustments_doctor_read`, etc.). Every other financial table in this
-- migration got this right; this one table was missed.
--
-- Fix: the plain SELECT policy now requires either an admin-visibility role
-- (matching the sibling `statements_owner_or_finance` pattern exactly) or
-- that the caller's own `doctor_profiles` row owns the parent statement.
-- `hellodoctor_finance_worker` gets its own explicit unconditional policy,
-- matching the house convention used for every other finance-sensitive
-- table in `20261003000001_rls_hardening` (e.g.
-- `transactions_finance_worker_all`, `doctor_wallets_finance_worker_all`)
-- rather than relying on `app.current_role` happening to be an admin role
-- inside a finance-worker transaction.
-- ============================================================================

DROP POLICY statement_transactions_read ON monthly_statement_transactions;

CREATE POLICY statement_transactions_owner_or_admin_read ON monthly_statement_transactions
    FOR SELECT TO hellodoctor_api
    USING (
        current_setting('app.current_role', true) IN ('PLATFORM_ADMIN', 'FINANCE_ADMIN', 'COMPLIANCE')
        OR EXISTS (
            SELECT 1 FROM monthly_payout_statements s
            JOIN doctor_profiles d ON d.id = s.doctor_id
            WHERE s.id = monthly_statement_transactions.statement_id
              AND d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
        )
    );

CREATE POLICY statement_transactions_finance_worker_select ON monthly_statement_transactions
    FOR SELECT TO hellodoctor_finance_worker USING (true);
