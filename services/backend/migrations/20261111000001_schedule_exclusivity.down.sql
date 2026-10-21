-- Reverses 20261111000001_schedule_exclusivity. Removes only objects created by
-- the up migration; no appointment, slot or financial row is touched.
-- Rows written by new code remain valid under the old schema (the dropped
-- columns are additive); `hold_expires_at` deadlines are discarded, after which
-- expiry reverts to the legacy created_at + window rule.
DROP INDEX IF EXISTS idx_appointments_pending_hold_expiry;
ALTER TABLE appointments DROP CONSTRAINT IF EXISTS appt_patient_no_overlap;
ALTER TABLE appointments DROP CONSTRAINT IF EXISTS appt_doctor_no_overlap;
DROP TRIGGER IF EXISTS doctor_slots_propagate_period ON doctor_schedule_slots;
DROP FUNCTION IF EXISTS doctor_slots_propagate_period();
DROP TRIGGER IF EXISTS appointments_slot_period ON appointments;
DROP FUNCTION IF EXISTS appointments_sync_slot_period();
ALTER TABLE appointments DROP COLUMN IF EXISTS hold_expires_at;
ALTER TABLE appointments DROP COLUMN IF EXISTS slot_period;
