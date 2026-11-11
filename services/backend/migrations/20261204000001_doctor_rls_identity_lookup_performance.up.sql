-- Milestone 8 Phase 3 RLS performance remediation (2026-11-02).
--
-- Root cause (verified live with EXPLAIN (ANALYZE, BUFFERS) against the real
-- hellodoctor_api role, real RLS context, and the seeded six-service
-- benchmark dataset -- not assumed): the general USING policies on
-- appointments, chat_conversations, chat_messages, and consultation_sessions
-- each resolve "is the caller the doctor who owns this row" via a CORRELATED
-- subquery back into doctor_profiles, e.g.
--   EXISTS (SELECT 1 FROM doctor_profiles d
--           WHERE d.id = <table>.doctor_id AND d.user_id = <caller>)
-- (chat_messages_access and consultation_sessions_access reach doctor_id via
-- an extra join to the relevant parent table first, making this two levels
-- deep). Because doctor_id varies per scanned row, PostgreSQL cannot treat
-- this as a single cacheable lookup and the planner showed it executed as a
-- genuine nested-loop subplan once per row: for a doctor with ~1,300-1,600
-- seeded rows (the GET /api/v1/doctor/chats and GET /api/v1/doctor/appointments
-- handlers in doctor_handlers.rs), this measured 3,074 separate subplan
-- executions in one request and a 23.3x slowdown versus the same query with
-- RLS bypassed (148ms vs 3,452ms for doctor/chats; 3,196ms for
-- doctor/appointments). See docs/MILESTONE_8_SIX_SERVICE_PERFORMANCE_REMEDIATION.md
-- Phase 3 for the full before/after evidence, including confirmation this
-- was the dominant cause of active-web-load degradation observed in the
-- six-service benchmark (not Next.js CPU, not primarily DB pool size).
--
-- Fix: fn_current_doctor_profile_id() resolves the SAME identity
-- (`doctor_profiles.id` for the row whose `user_id` equals the caller's
-- `app.current_user_id`) via a single STABLE SQL function call with NO
-- argument that varies per scanned row -- PostgreSQL can therefore evaluate
-- it once per statement (InitPlan) instead of once per row, exactly as it
-- already does for plain `current_setting(...)` calls elsewhere in these
-- same policies. Verified live in an isolated clone database
-- (hellodoctor_rls_clone, a pg_dump/restore copy -- never applied directly
-- against this project's long-running shared dev database or any production
-- target) before being written here:
--   - GET /api/v1/doctor/chats:        3,452ms -> 1,368ms combined with the
--     doctor_handlers.rs query restructuring in this same change (see that
--     file's own comment for why the two must be read together).
--   - GET /api/v1/doctor/appointments: 3,196ms ->   838ms, same pairing.
--   - Row-set equality confirmed identical before/after for every scanned
--     table, for a doctor with 1,613 appointments / 1,313 conversations, a
--     second doctor (cross-doctor isolation: 0 leaked rows either direction),
--     a patient (self-access preserved, 0 leaked rows for an unrelated
--     doctor), ADMIN (full visibility preserved, 63,442 rows), and an
--     unauthenticated/empty session context (fails closed, 0 rows).
--
-- This is a performance-only rewrite, not a security change: this function
-- answers the EXACT SAME question the correlated subquery already answered
-- (does a doctor_profiles row exist whose id matches this row's doctor_id
-- and whose user_id matches the caller?) -- same role branches (ADMIN /
-- patient-owned / doctor-owned) are preserved verbatim in every policy
-- below; only the doctor-owned branch's SQL shape changes. Sensitive,
-- richer per-row business checks that are NOT pure identity lookups --
-- patient_profiles_access's fn_doctor_treats_patient (verification status,
-- active-account, consultation-state/history-window checks) and the
-- directly-called doctor_history_context_allowed() -- are explicitly left
-- untouched by this migration; they are addressed instead by deferring their
-- evaluation to only the final paginated rows in doctor_handlers.rs, which
-- changes when they run, never what they're allowed to permit.
CREATE OR REPLACE FUNCTION public.fn_current_doctor_profile_id()
RETURNS uuid
LANGUAGE sql STABLE SECURITY DEFINER
SET search_path TO 'pg_catalog', 'public'
AS $function$
  SELECT d.id FROM public.doctor_profiles d
  WHERE d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
$function$;

DROP POLICY appointments_access ON appointments;
CREATE POLICY appointments_access ON appointments
    USING (
        current_setting('app.current_role', true) = ANY (ARRAY['ADMIN','ADMIN','ADMIN','ADMIN','ADMIN'])
        OR fn_patient_profile_owned_by_caller(patient_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid)
        OR doctor_id = fn_current_doctor_profile_id()
    );

DROP POLICY chat_conversations_access ON chat_conversations;
CREATE POLICY chat_conversations_access ON chat_conversations
    USING (
        current_setting('app.current_role', true) = ANY (ARRAY['ADMIN','ADMIN','ADMIN','ADMIN','ADMIN'])
        OR fn_patient_profile_owned_by_caller(patient_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid)
        OR doctor_id = fn_current_doctor_profile_id()
    );

DROP POLICY chat_messages_access ON chat_messages;
CREATE POLICY chat_messages_access ON chat_messages
    USING (
        current_setting('app.current_role', true) = ANY (ARRAY['ADMIN','ADMIN','ADMIN','ADMIN','ADMIN'])
        OR EXISTS (
            SELECT 1 FROM chat_conversations c WHERE c.id = chat_messages.conversation_id
                AND (
                    fn_patient_profile_owned_by_caller(c.patient_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid)
                    OR c.doctor_id = fn_current_doctor_profile_id()
                )
        )
    );

DROP POLICY consultation_sessions_access ON consultation_sessions;
CREATE POLICY consultation_sessions_access ON consultation_sessions
    USING (
        current_setting('app.current_role', true) = ANY (ARRAY['ADMIN','ADMIN','ADMIN','ADMIN','ADMIN'])
        OR EXISTS (
            SELECT 1 FROM appointments a WHERE a.id = consultation_sessions.appointment_id
                AND (
                    fn_patient_profile_owned_by_caller(a.patient_id, NULLIF(current_setting('app.current_user_id', true), '')::uuid)
                    OR a.doctor_id = fn_current_doctor_profile_id()
                )
        )
    );
