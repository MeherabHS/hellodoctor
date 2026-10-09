-- Add a reversible availability state; cancellation remains a separate removal.
-- Existing exclusion/index/RLS policies already include non-CANCELLED slots and
-- expose only AVAILABLE slots to patient booking. No policy/grant changes needed.
ALTER TYPE slot_status_enum ADD VALUE IF NOT EXISTS 'BLOCKED' AFTER 'AVAILABLE';
