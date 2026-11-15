-- Admin patient record (2026-11-15): the patient's Peer Navigator sessions
-- beside their doctor consultations. Same rules as fn_admin_peer_history:
-- Admin sessions only, dates, statuses and counts only, never the topic the
-- patient wrote or any message.
CREATE FUNCTION fn_admin_patient_peer_sessions(p_patient uuid)
RETURNS TABLE (
    request_id uuid,
    peer_id uuid,
    peer_name text,
    support_type text,
    status text,
    start_time timestamptz,
    requested_at timestamptz,
    completed_at timestamptz,
    messages bigint
)
LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public AS $$
    SELECT r.id, r.peer_id, p.full_name, r.support_type, r.status,
           s.start_time, r.created_at, r.completed_at,
           (SELECT count(*) FROM public.peer_conversations c
              JOIN public.peer_messages pm ON pm.conversation_id = c.id
             WHERE c.request_id = r.id)
    FROM public.peer_requests r
    JOIN public.peer_slots s ON s.id = r.slot_id
    JOIN public.peer_profiles p ON p.id = r.peer_id
    WHERE r.patient_id = p_patient
      AND current_setting('app.current_role', true) = 'ADMIN'
    ORDER BY s.start_time DESC, r.created_at DESC
$$;
REVOKE ALL ON FUNCTION fn_admin_patient_peer_sessions(uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION fn_admin_patient_peer_sessions(uuid) TO hellodoctor_api;
