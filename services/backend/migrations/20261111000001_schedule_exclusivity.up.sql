-- ============================================================================
-- Stage A: appointment interval exclusivity + explicit payment-hold deadline.
--
-- Invariants enforced by PostgreSQL itself (not by application checks):
--   INV-BOOK-01  One Doctor cannot hold two active appointments whose
--                half-open [start,end) intervals overlap.
--   INV-BOOK-02  One Patient *profile* (the person receiving care) cannot hold
--                two active appointments with overlapping intervals, even with
--                two different Doctors.
--   INV-BOOK-03  Payment holds carry an explicit `hold_expires_at`, so expiry
--                never needs a clock inside an index predicate.
--
-- Patient identity: the key is `appointments.patient_id`, i.e. the beneficiary
-- profile. A guardian account that manages several dependents may therefore
-- hold simultaneous appointments for two different dependents (two different
-- people). The same person can never be double-booked. Account-level
-- exclusivity for *live calls* is a separate, later control.
--
-- Design: `appointments.slot_period` is a denormalised copy of the slot's
-- [start_time, end_time) interval. Two triggers are its synchronisation
-- invariant (copy on insert / slot_id change; propagate when the slot's times
-- change), so the copy can never drift from the slot.
--
-- The "active" predicate has no clock:
--   PENDING_PAYMENT, CONFIRMED, WAITING, IN_CONSULTATION
-- A PENDING_PAYMENT row stops blocking only after it has been transitioned to
-- EXPIRED (by the sweep or by a booking that finds it expired).
--
-- Safe to roll forward under a rolling deploy: all new columns are nullable or
-- trigger-populated, so a not-yet-upgraded instance's INSERT/UPDATE statements
-- keep working. A not-yet-upgraded instance will see an overlapping booking as
-- a 500 instead of a 409 until it is upgraded; it can never create an overlap.
-- ============================================================================

-- ---------------------------------------------------------------------------
-- 0. Fail-closed preflight. Reads only; never modifies or deletes a row.
--    The same checks are available as a standalone read-only report in
--    scripts/preflight_schedule_exclusivity.sql for operators to run first.
-- ---------------------------------------------------------------------------
DO $$
DECLARE
    patient_conflicts bigint;
    doctor_conflicts  bigint;
    orphaned          bigint;
BEGIN
    SELECT count(*) INTO orphaned
      FROM appointments a
      LEFT JOIN doctor_schedule_slots s ON s.id = a.slot_id
     WHERE s.id IS NULL;

    SELECT count(*) INTO patient_conflicts
      FROM appointments a
      JOIN doctor_schedule_slots sa ON sa.id = a.slot_id
      JOIN appointments b ON b.patient_id = a.patient_id AND b.id > a.id
      JOIN doctor_schedule_slots sb ON sb.id = b.slot_id
     WHERE a.status IN ('PENDING_PAYMENT','CONFIRMED','WAITING','IN_CONSULTATION')
       AND b.status IN ('PENDING_PAYMENT','CONFIRMED','WAITING','IN_CONSULTATION')
       AND tstzrange(sa.start_time, sa.end_time, '[)') && tstzrange(sb.start_time, sb.end_time, '[)');

    SELECT count(*) INTO doctor_conflicts
      FROM appointments a
      JOIN doctor_schedule_slots sa ON sa.id = a.slot_id
      JOIN appointments b ON b.doctor_id = a.doctor_id AND b.id > a.id
      JOIN doctor_schedule_slots sb ON sb.id = b.slot_id
     WHERE a.status IN ('PENDING_PAYMENT','CONFIRMED','WAITING','IN_CONSULTATION')
       AND b.status IN ('PENDING_PAYMENT','CONFIRMED','WAITING','IN_CONSULTATION')
       AND tstzrange(sa.start_time, sa.end_time, '[)') && tstzrange(sb.start_time, sb.end_time, '[)');

    IF orphaned > 0 OR patient_conflicts > 0 OR doctor_conflicts > 0 THEN
        RAISE EXCEPTION
            'schedule exclusivity preflight failed: % orphaned appointment(s), % overlapping Patient pair(s), % overlapping Doctor pair(s). No data was changed. Run scripts/preflight_schedule_exclusivity.sql and obtain an operator-approved remediation plan; paid appointments must never be auto-cancelled or deleted.',
            orphaned, patient_conflicts, doctor_conflicts
            USING ERRCODE = 'check_violation';
    END IF;
END $$;

-- ---------------------------------------------------------------------------
-- 1. Columns
-- ---------------------------------------------------------------------------
ALTER TABLE appointments ADD COLUMN slot_period tstzrange;
ALTER TABLE appointments ADD COLUMN hold_expires_at timestamptz;

COMMENT ON COLUMN appointments.slot_period IS
    'Synchronised copy of the slot''s half-open [start_time,end_time) interval. Maintained only by triggers.';
COMMENT ON COLUMN appointments.hold_expires_at IS
    'Deadline of a PENDING_PAYMENT hold, set at booking from the configured reservation window. NULL on legacy rows: the sweep then falls back to created_at + the configured window.';

-- ---------------------------------------------------------------------------
-- 2. Synchronisation triggers. SECURITY DEFINER so the copy works for every
--    capability role regardless of its table grants.
-- ---------------------------------------------------------------------------
CREATE FUNCTION appointments_sync_slot_period() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, public AS $$
BEGIN
    SELECT tstzrange(start_time, end_time, '[)') INTO NEW.slot_period
      FROM public.doctor_schedule_slots WHERE id = NEW.slot_id;
    IF NEW.slot_period IS NULL THEN
        RAISE EXCEPTION 'appointment slot does not exist' USING ERRCODE = 'foreign_key_violation';
    END IF;
    RETURN NEW;
END $$;

CREATE TRIGGER appointments_slot_period
    BEFORE INSERT OR UPDATE OF slot_id ON appointments
    FOR EACH ROW EXECUTE FUNCTION appointments_sync_slot_period();

CREATE FUNCTION doctor_slots_propagate_period() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, public AS $$
BEGIN
    -- If the new interval would create an overlap for an active appointment,
    -- the exclusion constraints below raise 23P01 and the slot edit is rejected.
    UPDATE public.appointments
       SET slot_period = tstzrange(NEW.start_time, NEW.end_time, '[)')
     WHERE slot_id = NEW.id;
    RETURN NULL;
END $$;

CREATE TRIGGER doctor_slots_propagate_period
    AFTER UPDATE OF start_time, end_time ON doctor_schedule_slots
    FOR EACH ROW
    WHEN (OLD.start_time IS DISTINCT FROM NEW.start_time OR OLD.end_time IS DISTINCT FROM NEW.end_time)
    EXECUTE FUNCTION doctor_slots_propagate_period();

-- ---------------------------------------------------------------------------
-- 3. Backfill, then require the column.
-- ---------------------------------------------------------------------------
UPDATE appointments a
   SET slot_period = tstzrange(s.start_time, s.end_time, '[)')
  FROM doctor_schedule_slots s
 WHERE s.id = a.slot_id;

ALTER TABLE appointments ALTER COLUMN slot_period SET NOT NULL;

-- ---------------------------------------------------------------------------
-- 4. The two exclusion constraints (btree_gist is installed by the initial
--    schema). `[)` ranges let 10:00-10:30 and 10:30-11:00 coexist.
-- ---------------------------------------------------------------------------
ALTER TABLE appointments ADD CONSTRAINT appt_doctor_no_overlap EXCLUDE USING gist (
    doctor_id WITH =,
    slot_period WITH &&
) WHERE (status IN ('PENDING_PAYMENT','CONFIRMED','WAITING','IN_CONSULTATION'));

ALTER TABLE appointments ADD CONSTRAINT appt_patient_no_overlap EXCLUDE USING gist (
    patient_id WITH =,
    slot_period WITH &&
) WHERE (status IN ('PENDING_PAYMENT','CONFIRMED','WAITING','IN_CONSULTATION'));

-- ---------------------------------------------------------------------------
-- 5. Sweep support: the set of unpaid holds is small; keep it cheap to scan.
-- ---------------------------------------------------------------------------
CREATE INDEX idx_appointments_pending_hold_expiry
    ON appointments (hold_expires_at, created_at)
    WHERE status = 'PENDING_PAYMENT';
