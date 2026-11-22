-- Admin dashboard peer tracking (product owner, 2026-11-22): peer chat and
-- call sessions are tracked on the main dashboard next to doctor
-- consultations, for the same N-day window as the consultation trace, with a
-- per-session timeline.
--
-- Same rule as 20261222000009_admin_peer_history: Admin roles get no direct
-- read on the peer tables, because the topic a patient wrote and the chat
-- content can describe a health condition. These SECURITY DEFINER functions
-- return times, statuses, counts and who acted (patient or peer) only, and
-- only when the caller's session is an Admin. Never the topic, never message
-- text or attachments.

-- Every peer session (a request the peer accepted at some point) with
-- activity in the last p_days days. A session drops off by itself p_days
-- after its last activity.
CREATE FUNCTION fn_admin_peer_tracking(p_days integer)
RETURNS TABLE (
    request_id uuid,
    peer_id uuid,
    peer_name text,
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
    last_message_at timestamptz,
    call_status text,
    call_started_at timestamptz,
    call_ended_at timestamptz,
    patient_present boolean,
    peer_present boolean,
    last_activity_at timestamptz
)
LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public AS $$
    SELECT * FROM (
        SELECT r.id AS request_id, r.peer_id AS peer_id, pp.full_name AS peer_name,
               pt.full_name AS patient_name, r.support_type AS support_type, r.status AS status,
               s.start_time AS start_time, s.end_time AS end_time, r.created_at AS requested_at,
               r.decided_at AS decided_at, r.completed_at AS completed_at, r.cancelled_at AS cancelled_at,
               COALESCE(m.n, 0)::bigint AS messages, m.last_at AS last_message_at,
               cs.status AS call_status, cs.started_at AS call_started_at, cs.ended_at AS call_ended_at,
               COALESCE(cs.patient_present, false) AS patient_present,
               COALESCE(cs.peer_present, false) AS peer_present,
               GREATEST(r.created_at, r.decided_at, r.completed_at, r.cancelled_at, m.last_at,
                        cs.started_at, cs.ended_at,
                        CASE WHEN s.end_time <= clock_timestamp() THEN s.end_time END) AS last_activity_at
        FROM public.peer_requests r
        JOIN public.peer_slots s ON s.id = r.slot_id
        JOIN public.peer_profiles pp ON pp.id = r.peer_id
        JOIN public.patient_profiles pt ON pt.id = r.patient_id
        LEFT JOIN public.peer_call_sessions cs ON cs.request_id = r.id
        LEFT JOIN LATERAL (
            SELECT count(*) AS n, max(pm.sent_at) AS last_at
            FROM public.peer_conversations c
            JOIN public.peer_messages pm ON pm.conversation_id = c.id
            WHERE c.request_id = r.id
        ) m ON true
        WHERE r.decided_at IS NOT NULL
          AND r.status IN ('ACCEPTED', 'COMPLETED', 'CANCELLED')
          AND current_setting('app.current_role', true) = 'ADMIN'
    ) t
    WHERE t.last_activity_at > clock_timestamp() - make_interval(days => GREATEST(p_days, 1))
    ORDER BY (t.call_status = 'ACTIVE') DESC NULLS LAST, t.last_activity_at DESC
    LIMIT 200
$$;

-- One peer session's timeline, derived from the rows themselves: requested,
-- accepted or declined, chat opened, each message (who and when only), call
-- room opened, call started and ended, cancelled, completed.
CREATE FUNCTION fn_admin_peer_session_timeline(p_request uuid)
RETURNS TABLE (at timestamptz, event text, actor text)
LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public AS $$
    SELECT e.at, e.event, e.actor FROM (
        SELECT r.created_at AS at, 'REQUESTED' AS event, 'PATIENT' AS actor FROM public.peer_requests r WHERE r.id = p_request
        UNION ALL
        SELECT r.decided_at, CASE WHEN r.status = 'DECLINED' THEN 'DECLINED' ELSE 'ACCEPTED' END, 'PEER'
          FROM public.peer_requests r WHERE r.id = p_request AND r.decided_at IS NOT NULL
        UNION ALL
        SELECT c.created_at, 'CHAT_OPENED', 'PEER' FROM public.peer_conversations c WHERE c.request_id = p_request
        UNION ALL
        SELECT * FROM (
            SELECT pm.sent_at, CASE WHEN pm.attachment_object_key IS NOT NULL THEN 'ATTACHMENT_SENT' ELSE 'MESSAGE_SENT' END,
                   CASE WHEN pm.sender_id = c.peer_user_id THEN 'PEER' ELSE 'PATIENT' END
              FROM public.peer_conversations c
              JOIN public.peer_messages pm ON pm.conversation_id = c.id
             WHERE c.request_id = p_request
             ORDER BY pm.sent_at, pm.id
             LIMIT 500
        ) msgs
        UNION ALL
        SELECT cs.created_at, 'CALL_ROOM_OPENED', 'SYSTEM' FROM public.peer_call_sessions cs WHERE cs.request_id = p_request
        UNION ALL
        SELECT cs.started_at, 'CALL_STARTED', 'SYSTEM' FROM public.peer_call_sessions cs WHERE cs.request_id = p_request AND cs.started_at IS NOT NULL
        UNION ALL
        SELECT cs.ended_at, 'CALL_ENDED', 'SYSTEM' FROM public.peer_call_sessions cs WHERE cs.request_id = p_request AND cs.ended_at IS NOT NULL
        UNION ALL
        SELECT r.cancelled_at, 'CANCELLED', 'PATIENT' FROM public.peer_requests r WHERE r.id = p_request AND r.cancelled_at IS NOT NULL
        UNION ALL
        SELECT r.completed_at, 'COMPLETED', 'SYSTEM' FROM public.peer_requests r WHERE r.id = p_request AND r.completed_at IS NOT NULL
    ) e
    WHERE current_setting('app.current_role', true) = 'ADMIN'
    ORDER BY e.at
$$;

REVOKE ALL ON FUNCTION fn_admin_peer_tracking(integer) FROM PUBLIC;
REVOKE ALL ON FUNCTION fn_admin_peer_session_timeline(uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION fn_admin_peer_tracking(integer) TO hellodoctor_api;
GRANT EXECUTE ON FUNCTION fn_admin_peer_session_timeline(uuid) TO hellodoctor_api;
