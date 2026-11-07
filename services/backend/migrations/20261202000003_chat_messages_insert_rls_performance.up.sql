-- Milestone 8.0 database performance remediation (2026-10-31).
--
-- Root cause (verified live with EXPLAIN (ANALYZE, BUFFERS) against the
-- real hellodoctor_api role and realistic data volume before this change
-- was written, not assumed): chat_messages_insert's WITH CHECK used an
-- UNCORRELATED subquery shape -- `conversation_id IN (SELECT id FROM
-- chat_conversations WHERE <ownership-or-doctor> AND NOT is_locked AND
-- expires_at > now())` -- which forces PostgreSQL to materialize that whole
-- WHERE clause's matching set via a full Seq Scan of chat_conversations
-- (22,910 rows in the measured dataset; 476-480ms per INSERT, 48.9% of all
-- Postgres CPU time in a 1,200-session load test, confirmed via
-- `pg_stat_statements`), evaluating fn_patient_profile_owned_by_caller()
-- once per scanned row (5.7 million calls against ~660 inserts in that same
-- run) before the outer IN membership test can even run.
--
-- A plain btree index on chat_conversations(patient_id) or (doctor_id) was
-- considered and explicitly verified NOT to fix this: the Seq Scan's filter
-- wraps patient_id inside fn_patient_profile_owned_by_caller(patient_id,
-- ...), a function call, not a direct comparison -- PostgreSQL cannot use a
-- plain index to answer "which rows satisfy this opaque boolean function of
-- a column" without evaluating the function per candidate row regardless of
-- whether an index exists on that column.
--
-- The fix needs no new index at all. This table's own sibling policies
-- (chat_sender_guard, added in 20261107000003_chat_rls_backstop, and
-- chat_messages_access's USING clause, from 20261003000001_rls_hardening)
-- already encode the IDENTICAL ownership/lock/expiry condition as a
-- CORRELATED `EXISTS (SELECT 1 FROM chat_conversations c WHERE c.id =
-- conversation_id AND ...)` -- which lets the planner use the existing
-- chat_conversations_pkey index to fetch exactly the one relevant row
-- before evaluating the ownership function on it, instead of scanning the
-- whole table. Verified live: the identical rewrite measured at 1.1ms via
-- EXPLAIN (ANALYZE, BUFFERS) against the same role/data (a ~430x reduction
-- from the 476-480ms uncorrelated shape), using
-- "Index Scan using chat_conversations_pkey", not a Seq Scan.
--
-- This migration ONLY restructures chat_messages_insert's WITH CHECK
-- expression into that same correlated-EXISTS shape. The authorization
-- semantics are unchanged -- EXISTS (SELECT 1 FROM T WHERE id = X AND P(id))
-- is logically identical to X IN (SELECT id FROM T WHERE P(id)) when id is
-- T's primary key (chat_conversations.id is), so this is a performance-only
-- rewrite, not a security change: sender must still equal the caller; the
-- caller must still either own the conversation's patient profile
-- (including the guardian/dependent case, via the same
-- fn_patient_profile_owned_by_caller) or be the conversation's doctor with
-- live VERIFIED, non-deleted status; the conversation must still be
-- unlocked and unexpired. chat_sender_guard (RESTRICTIVE) and
-- chat_messages_access (the general USING policy) are untouched -- both
-- already used the efficient shape and were never part of this bug.
DROP POLICY chat_messages_insert ON chat_messages;
CREATE POLICY chat_messages_insert ON chat_messages
    FOR INSERT TO hellodoctor_api WITH CHECK (
        sender_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
        AND EXISTS (
            SELECT 1 FROM chat_conversations c WHERE c.id = conversation_id
                AND (
                    fn_patient_profile_owned_by_caller(c.patient_id, sender_id)
                    OR c.doctor_id IN (SELECT id FROM doctor_profiles WHERE user_id = sender_id
                        AND professional_verification_status = 'VERIFIED' AND deleted_at IS NULL)
                )
                AND c.is_locked = FALSE
                AND c.expires_at > CURRENT_TIMESTAMP
        )
    );
