-- Milestone 8.0 critical-query audit (2026-10-31).
--
-- `consultation_trace::maintenance()` runs every 60 seconds, forever, in
-- production (services/notification_scheduler.rs's background loop). Two of
-- its three queries use `clock_timestamp()` (VOLATILE, re-evaluated per row,
-- opaque to the planner at plan time) as the bound of an inequality filter
-- against a column the query also sorts and limits by -- a shape where
-- PostgreSQL cannot fold the comparison value into a constant and therefore
-- falls back to a generic default selectivity guess instead of its real
-- column statistics. Verified live via EXPLAIN (ANALYZE, BUFFERS) against
-- real Configuration-A data before this migration was written:
--
-- 1. `cleanup_consultation_trace()`'s discovery SELECT against
--    `consultation_trace_events` (63,686 rows, 0 currently eligible): the
--    existing `consultation_trace_expiry` btree index on `expires_at` WAS
--    present but was used only to serve `ORDER BY expires_at` (scanning the
--    whole index start-to-finish and filtering row by row), not as a true
--    index condition -- 43,818 buffers, 26.6ms, "Filter:
--    (expires_at <= clock_timestamp())", "Rows Removed by Filter: 63686".
--    Replacing `clock_timestamp()` with `now()` in this one SELECT (not the
--    per-row DELETE recheck, which must stay volatile -- see below) let the
--    planner fold `now()` to a constant, estimate correctly against real
--    stats, and produced "Index Cond: (expires_at <= now())": 11 buffers,
--    0.124ms -- a ~3,983x / ~215x reduction.
-- 2. The chat-lock discovery SELECT against `chat_conversations` (24,815
--    rows) has the same `clock_timestamp()` shape but NO supporting index at
--    all, so it was already choosing a plain Seq Scan (387 buffers, 2.475ms)
--    -- not pathological today, but a cost that grows linearly with the
--    table forever, called every 60s in perpetuity. Tested empirically
--    before deciding: adding a new partial index on `(expires_at) WHERE NOT
--    is_locked` WITHOUT also fixing `clock_timestamp()` made this WORSE
--    (9,251 buffers, 3.554ms -- the exact same per-row-filter pathology as
--    (1), now against the new index), confirming indexes must not be added
--    reflexively. Both changes together: 2 buffers, 0.014ms ("Index Cond:
--    (expires_at <= now())") -- a ~175x reduction, and the cost no longer
--    scales with table size at all.
--
-- Neither change alters observable behavior: this is a batch maintenance
-- job that already re-checks true current time (`clock_timestamp()`,
-- intentionally left untouched) at the point it actually acts on each row
-- (the per-row DELETE in cleanup_consultation_trace, and the per-row UPDATE
-- lock in maintenance()) -- using the transaction's start time only to
-- DISCOVER candidates is semantically appropriate here (any row that
-- becomes eligible mid-scan is picked up on the very next 60-second cycle
-- regardless), and the existing per-row re-check already prevents acting on
-- a row prematurely. The source-side chat_conversations query change
-- (clock_timestamp() -> now() in services/consultation_trace.rs's
-- maintenance()) ships alongside this migration in the same commit.
CREATE OR REPLACE FUNCTION public.cleanup_consultation_trace(p_limit integer DEFAULT 500)
 RETURNS bigint
 LANGUAGE plpgsql
 SET search_path TO 'public'
AS $function$
DECLARE r RECORD; deleted BIGINT:=0; affected BIGINT;
BEGIN
 FOR r IN SELECT e.id,e.appointment_id FROM consultation_trace_events e WHERE e.expires_at<=now()
 AND NOT EXISTS(SELECT 1 FROM consultation_trace_holds h WHERE h.appointment_id=e.appointment_id AND h.release_at IS NULL)
 ORDER BY e.expires_at,e.id LIMIT LEAST(GREATEST(p_limit,1),1000) LOOP
  PERFORM pg_advisory_xact_lock(hashtextextended('consultation-trace:'||r.appointment_id::text,0));
  IF NOT EXISTS(SELECT 1 FROM consultation_trace_holds WHERE appointment_id=r.appointment_id AND release_at IS NULL) THEN
   DELETE FROM consultation_trace_events WHERE id=r.id AND expires_at<=clock_timestamp();
   GET DIAGNOSTICS affected=ROW_COUNT; deleted:=deleted+affected;
  END IF;
 END LOOP; RETURN deleted;
END $function$;

-- Supports the chat-lock discovery query's WHERE NOT is_locked AND
-- expires_at <= now() (services/consultation_trace.rs's maintenance()),
-- matching the shape of the existing idx_consultation_sessions_presence_grace
-- partial index already in this schema for an analogous "small live subset
-- of a permanently-growing table" access pattern. Write cost is bounded: a
-- conversation transitions is_locked false -> true exactly once in its
-- lifecycle, so each row is removed from this index at most once, never
-- repeatedly re-indexed.
CREATE INDEX idx_chat_conversations_expiry_unlocked ON chat_conversations(expires_at) WHERE NOT is_locked;
