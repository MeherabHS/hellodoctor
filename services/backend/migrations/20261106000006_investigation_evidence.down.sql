DROP FUNCTION session_related_to_consultation(UUID,UUID);
DROP TRIGGER chat_lock_transition ON chat_conversations;
DROP FUNCTION consultation_lock_transition();
DROP POLICY notifications_admin_evidence ON notifications;
