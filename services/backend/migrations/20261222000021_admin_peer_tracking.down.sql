-- Reverses 20261222000021_admin_peer_tracking.up.sql (read-only functions;
-- no data is touched).
DROP FUNCTION IF EXISTS fn_admin_peer_session_timeline(uuid);
DROP FUNCTION IF EXISTS fn_admin_peer_tracking(integer);
