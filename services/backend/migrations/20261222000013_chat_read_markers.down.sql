-- Reverses 20261222000013_chat_read_markers.up.sql. Read receipts are
-- derived state (no clinical content); dropping them loses only the ticks.
DROP TABLE IF EXISTS chat_read_markers;
