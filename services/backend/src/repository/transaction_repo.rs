use crate::domain::ids::DoctorProfileId;
use crate::domain::models::{PaymentStatus, Transaction};
use chrono::Utc;
use sqlx::PgExecutor;
use uuid::Uuid;

pub async fn insert<'e, E: PgExecutor<'e>>(
    executor: E,
    txn: &Transaction,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO transactions (id, transaction_number, appointment_id, payment_session_id, gateway, gateway_reference, gross_amount, platform_fee_amount, net_amount, payment_status, created_at, settled_at, currency, payment_method_id) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)",
    )
    .bind(txn.id)
    .bind(&txn.transaction_number)
    .bind(txn.appointment_id)
    .bind(txn.payment_session_id)
    .bind(txn.gateway)
    .bind(&txn.gateway_reference)
    .bind(&txn.gross_amount)
    .bind(&txn.platform_fee_amount)
    .bind(&txn.net_amount)
    .bind(txn.payment_status)
    .bind(txn.created_at)
    .bind(txn.settled_at)
    .bind(&txn.currency)
    .bind(txn.payment_method_id)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn find_by_payment_session<'e, E: PgExecutor<'e>>(
    executor: E,
    payment_session_id: Uuid,
) -> Result<Option<Transaction>, sqlx::Error> {
    sqlx::query_as::<_, Transaction>(
        "SELECT id, transaction_number, appointment_id, payment_session_id, gateway, gateway_reference, gross_amount, platform_fee_amount, net_amount, payment_status, reconciliation_required, created_at, settled_at, currency, disbursement_item_id, payment_method_id \
         FROM transactions WHERE payment_session_id = $1",
    )
    .bind(payment_session_id)
    .fetch_optional(executor)
    .await
}

pub async fn find_by_appointment<'e, E: PgExecutor<'e>>(
    executor: E,
    appointment_id: Uuid,
) -> Result<Option<Transaction>, sqlx::Error> {
    sqlx::query_as::<_, Transaction>(
        "SELECT id, transaction_number, appointment_id, payment_session_id, gateway, gateway_reference, gross_amount, platform_fee_amount, net_amount, payment_status, reconciliation_required, created_at, settled_at, currency, disbursement_item_id, payment_method_id \
         FROM transactions WHERE appointment_id = $1 ORDER BY (payment_status <> 'FAILED') DESC, created_at DESC, id DESC LIMIT 1",
    )
    .bind(appointment_id)
    .fetch_optional(executor)
    .await
}

pub async fn list_all<'e, E: PgExecutor<'e>>(executor: E) -> Result<Vec<Transaction>, sqlx::Error> {
    sqlx::query_as::<_, Transaction>(
        "SELECT id, transaction_number, appointment_id, payment_session_id, gateway, gateway_reference, gross_amount, platform_fee_amount, net_amount, payment_status, reconciliation_required, created_at, settled_at, currency, disbursement_item_id, payment_method_id \
         FROM transactions ORDER BY created_at DESC",
    )
    .fetch_all(executor)
    .await
}

pub async fn mark_payment_held<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    gateway_reference: &str,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("UPDATE transactions SET payment_status = 'PAYMENT_HELD', gateway_reference = $2 WHERE id = $1 AND payment_status IN ('INITIATED','FAILED') AND gateway_reference IS NULL")
        .bind(id)
        .bind(gateway_reference)
        .execute(executor)
        .await?;
    Ok(result.rows_affected() == 1)
}

/// Conditional on the transaction still being `PAYMENT_HELD` — this is the
/// fix for a real double-credit bug: the caller previously read
/// `payment_status` once in an earlier, already-committed transaction, then
/// trusted that stale snapshot here. Two concurrent `complete_consultation`
/// calls for the same appointment (double-click, client retry) could both
/// pass that stale check and both credit the wallet. Making *this* UPDATE
/// itself the gate — and having the caller check `rows_affected()` before
/// ever crediting the wallet — makes the whole operation safe to attempt
/// twice: only the call that actually flips the status proceeds to credit.
pub async fn mark_settled<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE transactions SET payment_status = 'SETTLED_TO_DOCTOR', settled_at = $2 \
         WHERE id = $1 AND payment_status = 'PAYMENT_HELD'",
    )
    .bind(id)
    .bind(Utc::now())
    .execute(executor)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// Transactions already settled to a doctor's wallet but not yet included in a disbursement batch.
pub async fn list_settled_undisbursed_for_doctor<'e, E: PgExecutor<'e>>(
    executor: E,
    doctor_id: DoctorProfileId,
    period_start: chrono::NaiveDate,
    period_end: chrono::NaiveDate,
) -> Result<Vec<Transaction>, sqlx::Error> {
    sqlx::query_as::<_, Transaction>(
        "SELECT t.id, t.transaction_number, t.appointment_id, t.payment_session_id, t.gateway, t.gateway_reference, \
                t.gross_amount, t.platform_fee_amount, t.net_amount, t.payment_status, t.reconciliation_required, t.created_at, t.settled_at, t.currency, t.disbursement_item_id, t.payment_method_id \
         FROM transactions t \
         JOIN appointments a ON a.id = t.appointment_id \
         WHERE a.doctor_id = $1 AND t.payment_status = 'SETTLED_TO_DOCTOR' AND t.disbursement_item_id IS NULL \
           AND t.created_at >= $2::date AND t.created_at < ($3::date + INTERVAL '1 day')",
    )
    .bind(doctor_id)
    .bind(period_start)
    .bind(period_end)
    .fetch_all(executor)
    .await
}

/// Conditional claim: only transitions a transaction from
/// `SETTLED_TO_DOCTOR` to `DISBURSED` if it's still in that state — the fix
/// for a real double-payment bug. `list_settled_undisbursed_for_doctor`'s
/// `SELECT` is unlocked, so two concurrent "Initiate Disbursement" calls can
/// both read the exact same candidate rows before either commits; without
/// this guard, both would sum and pay out the same settled transactions.
/// Postgres's own row-level locking on `UPDATE` serializes concurrent
/// callers here — whichever commits first "wins" each row, and the second
/// caller's `WHERE` clause simply no longer matches once it proceeds.
/// Returns true if *this* call actually claimed the row (safe to include its
/// amounts in the current batch), false if something else already did.
pub async fn claim_for_disbursement<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE transactions SET payment_status = 'DISBURSED' \
         WHERE id = $1 AND payment_status = 'SETTLED_TO_DOCTOR'",
    )
    .bind(id)
    .execute(executor)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// Allocates a settled obligation to a prepared item without claiming that
/// money was paid. The unique transaction allocation is the exactly-once gate.
pub async fn assign_to_disbursement<'e, E: PgExecutor<'e>>(
    executor: E,
    transaction_id: Uuid,
    item_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE transactions SET disbursement_item_id = $2 WHERE id = $1 AND payment_status = 'SETTLED_TO_DOCTOR' AND disbursement_item_id IS NULL",
    )
    .bind(transaction_id)
    .bind(item_id)
    .execute(executor)
    .await?;
    Ok(result.rows_affected() == 1)
}

pub async fn mark_status<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    status: PaymentStatus,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE transactions SET payment_status = $2 WHERE id = $1")
        .bind(id)
        .bind(status)
        .execute(executor)
        .await?;
    Ok(())
}
