DROP FUNCTION IF EXISTS admin_release_live_reservation(UUID,TEXT,UUID);

DROP POLICY IF EXISTS consultation_live_reservations_internal_admin_release ON consultation_live_reservations;
DROP POLICY IF EXISTS consultation_live_reservations_system_all ON consultation_live_reservations;
DROP POLICY IF EXISTS consultation_live_reservations_admin_read ON consultation_live_reservations;
DROP POLICY IF EXISTS consultation_live_reservations_self_all ON consultation_live_reservations;

ALTER TABLE consultation_live_reservations NO FORCE ROW LEVEL SECURITY;
ALTER TABLE consultation_live_reservations DISABLE ROW LEVEL SECURITY;

REVOKE ALL ON consultation_live_reservations FROM hellodoctor_system_worker;
REVOKE ALL ON consultation_live_reservations FROM hellodoctor_api;

DROP TABLE IF EXISTS consultation_live_reservations;
