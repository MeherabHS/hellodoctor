-- Phase 2 follow-up: closes the issuance race `PasswordResetService::request_reset`
-- previously left open. `invalidate_all_for_user` (DELETE) and `insert`
-- (INSERT) ran as two separate statements, not one transaction — two
-- concurrent `request_reset` calls for the same account could each delete
-- zero/one rows and then each insert their own row, leaving two live
-- `password_reset_requests` rows for one user_id at once. The completion-side
-- fix (`complete_reset` now invalidating every row for the account inside its
-- own transaction) limited the blast radius but did not eliminate the
-- underlying race at issuance time.
--
-- A UNIQUE constraint on user_id makes "at most one live reset flow per
-- user" a database-enforced invariant rather than an application-level
-- convention two racing requests could violate: the request path moves to a
-- single atomic `INSERT ... ON CONFLICT (user_id) DO UPDATE`, so concurrent
-- callers for the same user serialize on this constraint instead of being
-- able to both succeed with two independent rows.
ALTER TABLE password_reset_requests
    ADD CONSTRAINT password_reset_requests_user_id_unique UNIQUE (user_id);
