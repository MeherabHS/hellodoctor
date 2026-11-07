REVOKE UPDATE(last_message_at) ON chat_conversations FROM hellodoctor_api;
DROP TRIGGER IF EXISTS chat_messages_bump_last_message ON chat_messages;
DROP FUNCTION IF EXISTS bump_chat_conversation_last_message();
ALTER TABLE chat_conversations DROP COLUMN IF EXISTS last_message_at;
