-- Milestone 8 Phase 5 efficiency investigation (2026-11-02).
--
-- Verified live via EXPLAIN (ANALYZE, BUFFERS) against the real
-- hellodoctor_api role under real RLS context, in an isolated pg_dump/restore
-- clone of the hellodoctor-m8-final-local benchmark database (never applied
-- directly against the shared dev database or any production target):
-- GET /api/v1/doctor/chats's list_own_chats query (doctor_handlers.rs)
-- computes MAX(m.sent_at) over chat_messages, joined BEFORE the LIMIT, to
-- determine each conversation's sort position. Because chat_messages has
-- FORCE ROW LEVEL SECURITY, scanning it re-evaluates chat_messages_access's
-- correlated EXISTS back into chat_conversations once per raw message row --
-- 3,591 separate executions for one doctor's 2,026 conversations in this
-- benchmark's seeded dataset -- not once per conversation. This measured
-- ~930ms of the endpoint's ~1,672ms (post-index-fix) execution time: removing
-- the chat_messages touch entirely (substituting c.created_at to measure the
-- floor) dropped execution to 744ms. See
-- docs/MILESTONE_8_PHASE5_EFFICIENCY_INVESTIGATION.md for the full
-- before/after plan evidence. fn_doctor_treats_patient and
-- doctor_history_context_allowed were independently measured in the same
-- investigation and confirmed NOT to be the bottleneck here (already correctly
-- deferred to only the ~20 paginated result rows by the Phase 3 fix) --
-- neither is touched by this migration.
--
-- Fix: denormalize the latest message timestamp onto chat_conversations
-- itself, maintained by a trigger on chat_messages INSERT, so the list/sort
-- query never touches chat_messages (and therefore never re-triggers its RLS
-- policy) at all. chat_messages remains the sole source of truth for message
-- content; this column is purely a derived, eventually-exact cache of a value
-- already computable by anyone with read access to the conversation (the
-- latest sent_at), not a new disclosure. Opening an actual chat thread still
-- reads chat_messages directly and is unaffected.
ALTER TABLE chat_conversations ADD COLUMN last_message_at TIMESTAMPTZ;

-- One-time backfill under the migrator's own (superuser) connection, not
-- subject to hellodoctor_api's restricted grants -- consistent with how every
-- other backfill in this migration history runs.
UPDATE chat_conversations c SET last_message_at = sub.max_sent
FROM (SELECT conversation_id, MAX(sent_at) AS max_sent FROM chat_messages GROUP BY conversation_id) sub
WHERE sub.conversation_id = c.id;

-- Plain (SECURITY INVOKER) trigger function, deliberately NOT SECURITY
-- DEFINER: it must run as the inserting role (hellodoctor_api, the sender's
-- own session) so FORCE RLS on chat_conversations is still enforced exactly
-- as for any other write that role performs -- the UPDATE below can only ever
-- touch the one conversation (NEW.conversation_id) that chat_sender_guard and
-- chat_messages_insert's own WITH CHECK clauses already proved the sender is
-- an authorized party of, via the existing chat_conversations_access USING
-- clause (doctor_id = fn_current_doctor_profile_id() OR
-- fn_patient_profile_owned_by_caller(...)), re-evaluated here with no
-- privilege escalation beyond the row this role already owns.
CREATE FUNCTION bump_chat_conversation_last_message() RETURNS trigger
LANGUAGE plpgsql SET search_path = pg_catalog, public AS $$
BEGIN
  UPDATE public.chat_conversations
    SET last_message_at = GREATEST(COALESCE(last_message_at, NEW.sent_at), NEW.sent_at)
    WHERE id = NEW.conversation_id;
  RETURN NEW;
END $$;
REVOKE ALL ON FUNCTION bump_chat_conversation_last_message() FROM PUBLIC;
CREATE TRIGGER chat_messages_bump_last_message AFTER INSERT ON chat_messages
  FOR EACH ROW EXECUTE FUNCTION bump_chat_conversation_last_message();

-- hellodoctor_api previously had read-only (SELECT) access to
-- chat_conversations -- table-level INSERT/UPDATE/DELETE belonged solely to
-- hellodoctor_system_worker. This grant is deliberately scoped to exactly the
-- one derived-cache column the trigger above writes, following this
-- project's existing precedent for a narrow, single-purpose write grant
-- (GRANT UPDATE(is_read) ON chat_messages TO hellodoctor_api). It does not
-- grant the ability to write any clinically or financially meaningful column
-- on chat_conversations, and FORCE RLS still restricts which rows any such
-- UPDATE can reach to ones the caller already owns.
GRANT UPDATE(last_message_at) ON chat_conversations TO hellodoctor_api;
