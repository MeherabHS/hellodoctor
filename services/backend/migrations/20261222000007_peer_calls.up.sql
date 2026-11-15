-- Peer Navigator, Phase 3b: voice/video support calls over Agora, with the
-- same timing rules as doctor video consultations (product owner,
-- 2026-11-14): the same join window around the slot, tokens that never
-- outlive it, presence from the same Agora webhook, and the same 180-second
-- empty-room grace, after which the call room ends and no new call token is
-- issued (the chat stays open). Voice and video share one session; the
-- client chooses whether to publish video.

CREATE TABLE peer_call_sessions (
    request_id UUID PRIMARY KEY REFERENCES peer_requests(id) ON DELETE RESTRICT,
    channel_name TEXT NOT NULL UNIQUE CHECK (channel_name LIKE 'peer\_%'),
    patient_user_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    peer_user_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    status TEXT NOT NULL DEFAULT 'INITIALIZED' CHECK (status IN ('INITIALIZED', 'ACTIVE', 'ENDED')),
    recovery_state TEXT NOT NULL DEFAULT 'NONE' CHECK (recovery_state IN ('NONE', 'GRACE_PERIOD', 'ENDED')),
    patient_present BOOLEAN NOT NULL DEFAULT false,
    peer_present BOOLEAN NOT NULL DEFAULT false,
    patient_presence_sequence BIGINT NOT NULL DEFAULT 0,
    peer_presence_sequence BIGINT NOT NULL DEFAULT 0,
    empty_room_deadline TIMESTAMPTZ,
    started_at TIMESTAMPTZ,
    ended_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
CREATE INDEX idx_peer_call_grace ON peer_call_sessions (empty_room_deadline)
    WHERE recovery_state = 'GRACE_PERIOD';

ALTER TABLE peer_call_sessions ENABLE ROW LEVEL SECURITY;
ALTER TABLE peer_call_sessions FORCE ROW LEVEL SECURITY;
CREATE POLICY peer_calls_parties_read ON peer_call_sessions FOR SELECT TO hellodoctor_api USING (
    NULLIF(current_setting('app.current_user_id', true), '')::uuid IN (patient_user_id, peer_user_id));
-- Created by either party when they first ask for a call token.
CREATE POLICY peer_calls_party_open ON peer_call_sessions FOR INSERT TO hellodoctor_api WITH CHECK (
    NULLIF(current_setting('app.current_user_id', true), '')::uuid IN (patient_user_id, peer_user_id)
    AND EXISTS (SELECT 1 FROM peer_requests r
                WHERE r.id = peer_call_sessions.request_id AND r.status = 'ACCEPTED'
                  AND r.patient_user_id = peer_call_sessions.patient_user_id
                  AND r.peer_user_id = peer_call_sessions.peer_user_id));
GRANT SELECT, INSERT ON peer_call_sessions TO hellodoctor_api;
-- Presence and the empty-room sweep run as the system worker.
CREATE POLICY peer_calls_system_worker ON peer_call_sessions TO hellodoctor_system_worker
    USING (true) WITH CHECK (true);
GRANT SELECT, UPDATE ON peer_call_sessions TO hellodoctor_system_worker;
