-- Peer Navigator, Phase 3a: chat between a patient and a peer. Opened when
-- the peer accepts a request and never locked (no 24-hour window, product
-- owner 2026-11-14): "Mark session complete" changes the request status only.
-- Same message rules as doctor chat (2,000 characters, idempotent sends,
-- image attachments stored encrypted). Admins have no access to content.

CREATE TABLE peer_conversations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    request_id UUID NOT NULL UNIQUE REFERENCES peer_requests(id) ON DELETE RESTRICT,
    patient_user_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    peer_user_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    last_message_at TIMESTAMPTZ
);
CREATE INDEX idx_peer_conversations_patient ON peer_conversations (patient_user_id, last_message_at DESC NULLS LAST);
CREATE INDEX idx_peer_conversations_peer ON peer_conversations (peer_user_id, last_message_at DESC NULLS LAST);

CREATE TABLE peer_messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    conversation_id UUID NOT NULL REFERENCES peer_conversations(id) ON DELETE CASCADE,
    sender_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    content TEXT NOT NULL CHECK (char_length(content) <= 2000),
    attachment_object_key VARCHAR(500),
    attachment_mime VARCHAR(50),
    attachment_size BIGINT,
    attachment_hash VARCHAR(128),
    is_read BOOLEAN NOT NULL DEFAULT false,
    sent_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    idempotency_key_hash TEXT,
    CONSTRAINT peer_message_has_body CHECK (content <> '' OR attachment_object_key IS NOT NULL)
);
CREATE INDEX idx_peer_messages_conversation ON peer_messages (conversation_id, sent_at, id);
CREATE UNIQUE INDEX peer_messages_idempotency ON peer_messages (conversation_id, sender_id, idempotency_key_hash)
    WHERE idempotency_key_hash IS NOT NULL;

-- The conversation's two parties, for policies (no per-row profile lookups).
CREATE FUNCTION fn_peer_conversation_party(p_conversation uuid) RETURNS boolean
LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, public AS $$
    SELECT EXISTS (
        SELECT 1 FROM public.peer_conversations c
        WHERE c.id = p_conversation
          AND NULLIF(current_setting('app.current_user_id', true), '')::uuid IN (c.patient_user_id, c.peer_user_id))
$$;
REVOKE ALL ON FUNCTION fn_peer_conversation_party(uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION fn_peer_conversation_party(uuid) TO hellodoctor_api;

ALTER TABLE peer_conversations ENABLE ROW LEVEL SECURITY;
ALTER TABLE peer_conversations FORCE ROW LEVEL SECURITY;
CREATE POLICY peer_conversations_parties_read ON peer_conversations FOR SELECT TO hellodoctor_api USING (
    NULLIF(current_setting('app.current_user_id', true), '')::uuid IN (patient_user_id, peer_user_id));
-- Opened by the peer when accepting their own request.
CREATE POLICY peer_conversations_peer_open ON peer_conversations FOR INSERT TO hellodoctor_api WITH CHECK (
    current_setting('app.current_role', true) = 'PEER'
    AND peer_user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
    AND EXISTS (SELECT 1 FROM peer_requests r
                WHERE r.id = peer_conversations.request_id AND r.status = 'ACCEPTED'
                  AND r.peer_user_id = peer_conversations.peer_user_id
                  AND r.patient_user_id = peer_conversations.patient_user_id));
CREATE POLICY peer_conversations_parties_touch ON peer_conversations FOR UPDATE TO hellodoctor_api
    USING (NULLIF(current_setting('app.current_user_id', true), '')::uuid IN (patient_user_id, peer_user_id))
    WITH CHECK (NULLIF(current_setting('app.current_user_id', true), '')::uuid IN (patient_user_id, peer_user_id));
GRANT SELECT, INSERT, UPDATE ON peer_conversations TO hellodoctor_api;

CREATE FUNCTION guard_peer_conversation_update() RETURNS trigger
LANGUAGE plpgsql SET search_path = pg_catalog, public AS $$
BEGIN
    IF (NEW.request_id, NEW.patient_user_id, NEW.peer_user_id, NEW.created_at)
       IS DISTINCT FROM (OLD.request_id, OLD.patient_user_id, OLD.peer_user_id, OLD.created_at) THEN
        RAISE EXCEPTION 'Peer conversation parties are immutable' USING ERRCODE = '42501';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER peer_conversation_update BEFORE UPDATE ON peer_conversations
    FOR EACH ROW EXECUTE FUNCTION guard_peer_conversation_update();

ALTER TABLE peer_messages ENABLE ROW LEVEL SECURITY;
ALTER TABLE peer_messages FORCE ROW LEVEL SECURITY;
CREATE POLICY peer_messages_parties_read ON peer_messages FOR SELECT TO hellodoctor_api
    USING (fn_peer_conversation_party(conversation_id));
CREATE POLICY peer_messages_party_send ON peer_messages FOR INSERT TO hellodoctor_api WITH CHECK (
    sender_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
    AND fn_peer_conversation_party(conversation_id));
CREATE POLICY peer_messages_recipient_read_receipt ON peer_messages FOR UPDATE TO hellodoctor_api
    USING (fn_peer_conversation_party(conversation_id)
           AND sender_id <> NULLIF(current_setting('app.current_user_id', true), '')::uuid)
    WITH CHECK (fn_peer_conversation_party(conversation_id));
GRANT SELECT, INSERT, UPDATE ON peer_messages TO hellodoctor_api;

-- Only the read flag may change, and only from unread to read.
CREATE FUNCTION guard_peer_message_update() RETURNS trigger
LANGUAGE plpgsql SET search_path = pg_catalog, public AS $$
BEGIN
    IF (NEW.conversation_id, NEW.sender_id, NEW.content, NEW.attachment_object_key, NEW.attachment_mime,
        NEW.attachment_size, NEW.attachment_hash, NEW.sent_at, NEW.idempotency_key_hash)
       IS DISTINCT FROM
       (OLD.conversation_id, OLD.sender_id, OLD.content, OLD.attachment_object_key, OLD.attachment_mime,
        OLD.attachment_size, OLD.attachment_hash, OLD.sent_at, OLD.idempotency_key_hash)
       OR (OLD.is_read AND NOT NEW.is_read) THEN
        RAISE EXCEPTION 'Peer messages are immutable' USING ERRCODE = '42501';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER peer_message_update BEFORE UPDATE ON peer_messages
    FOR EACH ROW EXECUTE FUNCTION guard_peer_message_update();
