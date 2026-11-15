-- Admin Peer Directory (2026-11-15): who each Peer Navigator has supported
-- and how often, for Admins only. Admin roles get no direct read on the
-- peer tables (chat content and the topic a patient wrote must never reach
-- Admin: a topic can describe a health condition). These SECURITY DEFINER
-- functions return dates, statuses and counts only, and only when the
-- caller's session is an Admin.

CREATE FUNCTION fn_admin_peer_directory()
RETURNS TABLE (
    peer_id uuid,
    requests bigint,
    sessions_completed bigint,
    sessions_upcoming bigint,
    patients_supported bigint,
    chat_sessions bigint,
    call_sessions bigint,
    messages bigint,
    last_session_at timestamptz
)
LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public AS $$
    SELECT p.id,
           count(r.id),
           count(r.id) FILTER (WHERE r.status = 'COMPLETED'),
           count(r.id) FILTER (WHERE r.status = 'ACCEPTED'),
           count(DISTINCT r.patient_id) FILTER (WHERE r.status IN ('ACCEPTED', 'COMPLETED')),
           count(r.id) FILTER (WHERE r.status IN ('ACCEPTED', 'COMPLETED') AND r.support_type = 'CHAT'),
           count(r.id) FILTER (WHERE r.status IN ('ACCEPTED', 'COMPLETED') AND r.support_type = 'CALL'),
           COALESCE(sum(m.n), 0)::bigint,
           max(s.start_time) FILTER (WHERE r.status IN ('ACCEPTED', 'COMPLETED'))
    FROM public.peer_profiles p
    LEFT JOIN public.peer_requests r ON r.peer_id = p.id
    LEFT JOIN public.peer_slots s ON s.id = r.slot_id
    LEFT JOIN LATERAL (
        SELECT count(*) AS n FROM public.peer_conversations c
        JOIN public.peer_messages pm ON pm.conversation_id = c.id
        WHERE c.request_id = r.id
    ) m ON true
    WHERE current_setting('app.current_role', true) = 'ADMIN'
    GROUP BY p.id
$$;

CREATE FUNCTION fn_admin_peer_history(p_peer uuid)
RETURNS TABLE (
    request_id uuid,
    patient_id uuid,
    patient_name text,
    support_type text,
    status text,
    start_time timestamptz,
    end_time timestamptz,
    requested_at timestamptz,
    decided_at timestamptz,
    completed_at timestamptz,
    cancelled_at timestamptz,
    messages bigint,
    call_started_at timestamptz,
    call_ended_at timestamptz
)
LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public AS $$
    SELECT r.id, r.patient_id, pt.full_name, r.support_type, r.status,
           s.start_time, s.end_time, r.created_at, r.decided_at, r.completed_at,
           r.cancelled_at,
           (SELECT count(*) FROM public.peer_conversations c
              JOIN public.peer_messages pm ON pm.conversation_id = c.id
             WHERE c.request_id = r.id),
           cs.started_at, cs.ended_at
    FROM public.peer_requests r
    JOIN public.peer_slots s ON s.id = r.slot_id
    JOIN public.patient_profiles pt ON pt.id = r.patient_id
    LEFT JOIN public.peer_call_sessions cs ON cs.request_id = r.id
    WHERE r.peer_id = p_peer
      AND current_setting('app.current_role', true) = 'ADMIN'
    ORDER BY s.start_time DESC, r.created_at DESC
$$;

REVOKE ALL ON FUNCTION fn_admin_peer_directory() FROM PUBLIC;
REVOKE ALL ON FUNCTION fn_admin_peer_history(uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION fn_admin_peer_directory() TO hellodoctor_api;
GRANT EXECUTE ON FUNCTION fn_admin_peer_history(uuid) TO hellodoctor_api;
