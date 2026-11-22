-- Reverses 20261222000020_admin_profile_pictures.up.sql. Drops only the
-- photo references; the stored objects become unreferenced and are not read
-- again.
DROP TABLE IF EXISTS admin_profile_pictures;
