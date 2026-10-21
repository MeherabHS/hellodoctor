use crate::domain::models::{Appointment, AppointmentClinicalIntake, AppointmentStatus};
use chrono::Utc;
use sqlx::PgExecutor;
use uuid::Uuid;

pub async fn insert<'e, E: PgExecutor<'e>>(
    executor: E,
    apt: &Appointment,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO appointments (id, appointment_number, patient_id, doctor_id, slot_id, modality, status, consultation_fee, clinical_outcome, completed_at, created_at, updated_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)",
    )
    .bind(apt.id)
    .bind(&apt.appointment_number)
    .bind(apt.patient_id)
    .bind(apt.doctor_id)
    .bind(apt.slot_id)
    .bind(apt.modality)
    .bind(apt.status)
    .bind(&apt.consultation_fee)
    .bind(&apt.clinical_outcome)
    .bind(apt.completed_at)
    .bind(apt.created_at)
    .bind(apt.updated_at)
    .execute(executor)
    .await?;
    Ok(())
}

/// Inserts a `PENDING_PAYMENT` booking whose reservation deadline is fixed at
/// creation from the policy in effect *now* (`hold_expires_at`), so a later
/// change to the configured window never alters an existing hold. Uses the
/// database clock, not the application clock, like every other deadline check.
pub async fn insert_pending_hold<'e, E: PgExecutor<'e>>(
    executor: E,
    apt: &Appointment,
    window_secs: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO appointments (id, appointment_number, patient_id, doctor_id, slot_id, modality, status, consultation_fee, clinical_outcome, completed_at, created_at, updated_at, hold_expires_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12, clock_timestamp() + make_interval(secs => $13))",
    )
    .bind(apt.id)
    .bind(&apt.appointment_number)
    .bind(apt.patient_id)
    .bind(apt.doctor_id)
    .bind(apt.slot_id)
    .bind(apt.modality)
    .bind(apt.status)
    .bind(&apt.consultation_fee)
    .bind(&apt.clinical_outcome)
    .bind(apt.completed_at)
    .bind(apt.created_at)
    .bind(apt.updated_at)
    .bind(window_secs as f64)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn find_by_id<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
) -> Result<Option<Appointment>, sqlx::Error> {
    sqlx::query_as::<_, Appointment>(
        "SELECT id, appointment_number, patient_id, doctor_id, slot_id, modality, status, consultation_fee, clinical_outcome, completed_at, created_at, updated_at \
         FROM appointments WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(executor)
    .await
}

/// Serializes concurrent callers on one appointment via a Postgres advisory
/// lock, scoped to the current transaction (auto-released at commit/
/// rollback) — the fix for a real race: two concurrent prescription-intake
/// uploads for the same appointment could each individually read a document
/// count under the 5-photo cap and both proceed to insert, together
/// exceeding it (see `PrescriptionService::upload_patient_intake_documents`,
/// the only caller).
///
/// Deliberately **not** `SELECT ... FOR UPDATE` on the `appointments` row:
/// that requires the `UPDATE` privilege in Postgres, which
/// `hellodoctor_api` intentionally does not have on this table (see
/// migrations/20261003000001_rls_hardening's own comment — appointment
/// lifecycle state is payment-worker-only). Confirmed by actually hitting
/// `permission denied for table appointments` when first tried, not
/// assumed. An advisory lock needs no table grant at all and is the
/// correct tool for "serialize on this logical key" regardless.
pub async fn lock_for_document_upload<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
) -> Result<(), sqlx::Error> {
    let key = i64::from_be_bytes(id.as_bytes()[0..8].try_into().unwrap());
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(key)
        .execute(executor)
        .await?;
    Ok(())
}

pub async fn list_all<'e, E: PgExecutor<'e>>(executor: E) -> Result<Vec<Appointment>, sqlx::Error> {
    sqlx::query_as::<_, Appointment>(
        "SELECT id, appointment_number, patient_id, doctor_id, slot_id, modality, status, consultation_fee, clinical_outcome, completed_at, created_at, updated_at \
         FROM appointments ORDER BY created_at DESC",
    )
    .fetch_all(executor)
    .await
}

/// Scoped alternative to `list_all` for handlers that only need a known,
/// typically-small set of appointments (e.g. the ones behind a handful of
/// *active* consultation sessions) — a whole-table fetch here scales with
/// total appointment history, not with the small amount of data actually
/// used, which is exactly backwards for an "active now" admin dashboard.
/// Confirmed as a real latency problem under a production-scale seeded
/// dataset (docs/CONSTRAINED_CAPACITY_TEST.md), not a hypothetical one.
pub async fn find_by_ids<'e, E: PgExecutor<'e>>(
    executor: E,
    ids: &[Uuid],
) -> Result<Vec<Appointment>, sqlx::Error> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    sqlx::query_as::<_, Appointment>(
        "SELECT id, appointment_number, patient_id, doctor_id, slot_id, modality, status, consultation_fee, clinical_outcome, completed_at, created_at, updated_at \
         FROM appointments WHERE id = ANY($1)",
    )
    .bind(ids)
    .fetch_all(executor)
    .await
}

pub async fn set_status<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    status: AppointmentStatus,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE appointments SET status = $2, updated_at = $3 WHERE id = $1")
        .bind(id)
        .bind(status)
        .bind(Utc::now())
        .execute(executor)
        .await?;
    Ok(())
}

/// h23 remediation: race-safe payment confirmation. Only transitions
/// PendingPayment->Confirmed; returns false if the row is no longer
/// PendingPayment (e.g. a concurrent `expire_abandoned_hold_for_slot` call
/// already won the race and moved it to Expired). The caller must not book
/// the slot when this returns false.
pub async fn confirm_payment_if_pending<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE appointments SET status='CONFIRMED', updated_at=clock_timestamp() WHERE id=$1 AND status='PENDING_PAYMENT'",
    )
    .bind(id)
    .execute(executor)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// A hold is due when its explicit deadline has passed. Legacy rows created
/// before `hold_expires_at` existed have no deadline and fall back to the
/// original `created_at + window` rule. Evaluated with `clock_timestamp()` so a
/// long-running transaction never uses a stale transaction-start time. The
/// predicate lives in queries only - never in an index/constraint predicate.
const HOLD_DUE: &str =
    "COALESCE(a.hold_expires_at, a.created_at + make_interval(secs => $WINDOW)) <= clock_timestamp()";

fn hold_due(window_param: u8) -> String {
    HOLD_DUE.replace("$WINDOW", &format!("${window_param}"))
}

/// Transitions one *locked* PENDING_PAYMENT appointment to EXPIRED and releases
/// its slot. Lock order is always appointment -> slot, the same order the
/// payment webhook uses, so reclaim and payment cannot deadlock. The status
/// compare-and-set makes it idempotent: a payment that won the row lock first
/// has already left PENDING_PAYMENT and is simply not touched.
async fn expire_locked_hold(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    appointment_id: Uuid,
    slot_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE appointments SET status='EXPIRED', updated_at=clock_timestamp() WHERE id=$1 AND status='PENDING_PAYMENT'",
    )
    .bind(appointment_id)
    .execute(&mut **tx)
    .await?;
    if result.rows_affected() != 1 {
        return Ok(false);
    }
    sqlx::query(
        "UPDATE doctor_schedule_slots SET status='AVAILABLE' WHERE id=$1 AND status='LOCKED_IN_PAYMENT' AND start_time > clock_timestamp()",
    )
    .bind(slot_id)
    .execute(&mut **tx)
    .await?;
    Ok(true)
}

/// Reclaims a due hold on `slot_id`, if any. Called by `book_appointment`
/// *before* it locks the slot (appointment -> slot lock order), inside the
/// booking transaction so reclaim and the following lock are atomic together.
/// Race-safe against a concurrent payment-success webhook purely through row
/// locking: `FOR UPDATE` waits for the webhook, then re-evaluates the
/// predicate, so a hold that was just confirmed is no longer returned.
pub async fn expire_abandoned_hold_for_slot(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    slot_id: Uuid,
    window_secs: i64,
) -> Result<Vec<Uuid>, sqlx::Error> {
    let sql = format!(
        "SELECT a.id FROM appointments a WHERE a.slot_id=$1 AND a.status='PENDING_PAYMENT' AND {} ORDER BY a.id FOR UPDATE OF a",
        hold_due(2)
    );
    let ids: Vec<Uuid> = sqlx::query_scalar(&sql)
        .bind(slot_id)
        .bind(window_secs as f64)
        .fetch_all(&mut **tx)
        .await?;
    let mut expired = Vec::new();
    for id in ids {
        if expire_locked_hold(tx, id, slot_id).await? {
            expired.push(id);
        }
    }
    Ok(expired)
}

/// Reclaims this Patient profile's own *due* holds whose interval overlaps the
/// slot being booked/rescheduled to. Without this, an abandoned checkout with a
/// different Doctor would keep blocking the Patient through
/// `appt_patient_no_overlap` until the next sweep. Only already-due holds are
/// touched: a live hold still blocks, as it must.
pub async fn expire_due_holds_overlapping_for_patient(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    patient_id: Uuid,
    slot_id: Uuid,
    window_secs: i64,
) -> Result<Vec<Uuid>, sqlx::Error> {
    let sql = format!(
        "SELECT a.id, a.slot_id FROM appointments a WHERE a.patient_id=$1 AND a.status='PENDING_PAYMENT' \
         AND a.slot_period && (SELECT tstzrange(s.start_time, s.end_time, '[)') FROM doctor_schedule_slots s WHERE s.id=$2) \
         AND {} ORDER BY a.id FOR UPDATE OF a",
        hold_due(3)
    );
    let rows: Vec<(Uuid, Uuid)> = sqlx::query_as(&sql)
        .bind(patient_id)
        .bind(slot_id)
        .bind(window_secs as f64)
        .fetch_all(&mut **tx)
        .await?;
    let mut expired = Vec::new();
    for (id, held_slot) in rows {
        if expire_locked_hold(tx, id, held_slot).await? {
            expired.push(id);
        }
    }
    Ok(expired)
}

/// Bounded, multi-instance-safe sweep: locks at most `limit` due holds with
/// `FOR UPDATE SKIP LOCKED`, so concurrent scheduler instances partition the
/// work instead of blocking or double-processing it. Returns the appointments
/// this call actually expired.
pub async fn expire_due_holds_batch(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    window_secs: i64,
    limit: i64,
) -> Result<Vec<Uuid>, sqlx::Error> {
    let sql = format!(
        "SELECT a.id, a.slot_id FROM appointments a WHERE a.status='PENDING_PAYMENT' AND {} \
         ORDER BY COALESCE(a.hold_expires_at, a.created_at), a.id LIMIT $2 FOR UPDATE OF a SKIP LOCKED",
        hold_due(1)
    );
    let rows: Vec<(Uuid, Uuid)> = sqlx::query_as(&sql)
        .bind(window_secs as f64)
        .bind(limit)
        .fetch_all(&mut **tx)
        .await?;
    let mut expired = Vec::new();
    for (id, slot_id) in rows {
        if expire_locked_hold(tx, id, slot_id).await? {
            expired.push(id);
        }
    }
    Ok(expired)
}

pub async fn complete<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    outcome: &str,
) -> Result<(), sqlx::Error> {
    let now = Utc::now();
    sqlx::query(
        "UPDATE appointments SET status = 'COMPLETED', clinical_outcome = $2, completed_at = COALESCE(completed_at, $3), updated_at = $3 WHERE id = $1",
    )
    .bind(id)
    .bind(outcome)
    .bind(now)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn insert_clinical_intake<'e, E: PgExecutor<'e>>(
    executor: E,
    intake: &AppointmentClinicalIntake,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO appointment_clinical_intake (appointment_id, patient_id, chief_complaint, clinical_notes, created_at, updated_at) \
         VALUES ($1,$2,$3,$4,$5,$6)",
    )
    .bind(intake.appointment_id)
    .bind(intake.patient_id)
    .bind(&intake.chief_complaint)
    .bind(&intake.clinical_notes)
    .bind(intake.created_at)
    .bind(intake.updated_at)
    .execute(executor)
    .await?;
    Ok(())
}
