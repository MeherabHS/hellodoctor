use crate::domain::models::{PaymentStatus, Transaction};
use chrono::Utc;
use sqlx::PgExecutor;
use uuid::Uuid;

pub async fn insert<'e, E: PgExecutor<'e>>(executor: E, txn: &Transaction) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO transactions (id, transaction_number, appointment_id, payment_session_id, gateway, gateway_reference, gross_amount, platform_fee_amount, net_amount, payment_status, created_at, settled_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)",
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
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn find_by_payment_session<'e, E: PgExecutor<'e>>(executor: E, payment_session_id: Uuid) -> Result<Option<Transaction>, sqlx::Error> {
    sqlx::query_as::<_, Transaction>(
        "SELECT id, transaction_number, appointment_id, payment_session_id, gateway, gateway_reference, gross_amount, platform_fee_amount, net_amount, payment_status, created_at, settled_at \
         FROM transactions WHERE payment_session_id = $1",
    )
    .bind(payment_session_id)
    .fetch_optional(executor)
    .await
}

pub async fn find_by_appointment<'e, E: PgExecutor<'e>>(executor: E, appointment_id: Uuid) -> Result<Option<Transaction>, sqlx::Error> {
    sqlx::query_as::<_, Transaction>(
        "SELECT id, transaction_number, appointment_id, payment_session_id, gateway, gateway_reference, gross_amount, platform_fee_amount, net_amount, payment_status, created_at, settled_at \
         FROM transactions WHERE appointment_id = $1",
    )
    .bind(appointment_id)
    .fetch_optional(executor)
    .await
}

pub async fn list_all<'e, E: PgExecutor<'e>>(executor: E) -> Result<Vec<Transaction>, sqlx::Error> {
    sqlx::query_as::<_, Transaction>(
        "SELECT id, transaction_number, appointment_id, payment_session_id, gateway, gateway_reference, gross_amount, platform_fee_amount, net_amount, payment_status, created_at, settled_at \
         FROM transactions ORDER BY created_at DESC",
    )
    .fetch_all(executor)
    .await
}

pub async fn mark_payment_held<'e, E: PgExecutor<'e>>(executor: E, id: Uuid, gateway_reference: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE transactions SET payment_status = 'PAYMENT_HELD', gateway_reference = $2 WHERE id = $1")
        .bind(id)
        .bind(gateway_reference)
        .execute(executor)
        .await?;
    Ok(())
}

pub async fn mark_settled<'e, E: PgExecutor<'e>>(executor: E, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE transactions SET payment_status = 'SETTLED_TO_DOCTOR', settled_at = $2 WHERE id = $1")
        .bind(id)
        .bind(Utc::now())
        .execute(executor)
        .await?;
    Ok(())
}

/// Transactions already settled to a doctor's wallet but not yet included in a disbursement batch.
pub async fn list_settled_undisbursed_for_doctor<'e, E: PgExecutor<'e>>(executor: E, doctor_id: Uuid) -> Result<Vec<Transaction>, sqlx::Error> {
    sqlx::query_as::<_, Transaction>(
        "SELECT t.id, t.transaction_number, t.appointment_id, t.payment_session_id, t.gateway, t.gateway_reference, \
                t.gross_amount, t.platform_fee_amount, t.net_amount, t.payment_status, t.created_at, t.settled_at \
         FROM transactions t \
         JOIN appointments a ON a.id = t.appointment_id \
         WHERE a.doctor_id = $1 AND t.payment_status = 'SETTLED_TO_DOCTOR'",
    )
    .bind(doctor_id)
    .fetch_all(executor)
    .await
}

pub async fn mark_status<'e, E: PgExecutor<'e>>(executor: E, id: Uuid, status: PaymentStatus) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE transactions SET payment_status = $2 WHERE id = $1")
        .bind(id)
        .bind(status)
        .execute(executor)
        .await?;
    Ok(())
}
