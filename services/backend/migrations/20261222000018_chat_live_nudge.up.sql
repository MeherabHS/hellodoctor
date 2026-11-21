-- Live chat nudge (product owner, 2026-11-21).
--
-- Every new chat message already inserts a NEW_CHAT_MESSAGE notification for
-- its recipient (consultation chat and peer chat). This trigger additionally
-- emits a Postgres NOTIFY on channel `chat_live` so connected apps can fetch
-- the conversation at once instead of waiting for their next poll.
--
-- NOTIFY is transactional: it is delivered only after the message's
-- transaction commits, and never for a rolled-back one. The payload carries
-- identifiers only (recipient, conversation, kind) and never message text,
-- titles or other health information; apps still read messages over the
-- normal authenticated HTTP endpoints.

CREATE OR REPLACE FUNCTION notify_chat_live() RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog
AS $$
BEGIN
    IF NEW.related_entity_id IS NOT NULL THEN
        PERFORM pg_notify(
            'chat_live',
            json_build_object(
                'u', NEW.user_id,
                'c', NEW.related_entity_id,
                'k', COALESCE(NEW.related_entity_type, '')
            )::text
        );
    END IF;
    RETURN NULL;
END;
$$;

CREATE TRIGGER notifications_chat_live
    AFTER INSERT ON notifications
    FOR EACH ROW
    WHEN (NEW.type = 'NEW_CHAT_MESSAGE')
    EXECUTE FUNCTION notify_chat_live();
