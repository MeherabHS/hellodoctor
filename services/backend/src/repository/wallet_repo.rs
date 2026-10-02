use crate::domain::models::{DisbursementBatch, DisbursementItem, DoctorWallet};
use bigdecimal::BigDecimal;
use chrono::Utc;
use sqlx::PgExecutor;
use uuid::Uuid;

pub async fn get_or_default<'e, E: PgExecutor<'e>>(executor: E, doctor_id: Uuid) -> Result<DoctorWallet, sqlx::Error> {
    let found = sqlx::query_as::<_, DoctorWallet>(
        "SELECT doctor_id, lifetime_gross, lifetime_fee_withheld, lifetime_net, pending_disbursement, last_disbursement_at, last_disbursement_amount, updated_at \
         FROM doctor_wallets WHERE doctor_id = $1",
    )
    .bind(doctor_id)
    .fetch_optional(executor)
    .await?;

    Ok(found.unwrap_or(DoctorWallet {
        doctor_id,
        lifetime_gross: BigDecimal::from(0),
        lifetime_fee_withheld: BigDecimal::from(0),
        lifetime_net: BigDecimal::from(0),
        pending_disbursement: BigDecimal::from(0),
        last_disbursement_at: None,
        last_disbursement_amount: BigDecimal::from(0),
        updated_at: Utc::now(),
    }))
}

pub async fn list_all<'e, E: PgExecutor<'e>>(executor: E) -> Result<Vec<DoctorWallet>, sqlx::Error> {
    sqlx::query_as::<_, DoctorWallet>(
        "SELECT doctor_id, lifetime_gross, lifetime_fee_withheld, lifetime_net, pending_disbursement, last_disbursement_at, last_disbursement_amount, updated_at FROM doctor_wallets",
    )
    .fetch_all(executor)
    .await
}

pub async fn insert_default<'e, E: PgExecutor<'e>>(executor: E, doctor_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO doctor_wallets (doctor_id, lifetime_gross, lifetime_fee_withheld, lifetime_net, pending_disbursement, last_disbursement_amount, updated_at) \
         VALUES ($1, 0, 0, 0, 0, 0, $2) ON CONFLICT (doctor_id) DO NOTHING",
    )
    .bind(doctor_id)
    .bind(Utc::now())
    .execute(executor)
    .await?;
    Ok(())
}

/// Credits a settled transaction's gross/fee/net into the doctor's running wallet totals.
pub async fn credit<'e, E: PgExecutor<'e>>(
    executor: E,
    doctor_id: Uuid,
    gross: &BigDecimal,
    fee: &BigDecimal,
    net: &BigDecimal,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO doctor_wallets (doctor_id, lifetime_gross, lifetime_fee_withheld, lifetime_net, pending_disbursement, last_disbursement_amount, updated_at) \
         VALUES ($1, $2, $3, $4, $4, 0, $5) \
         ON CONFLICT (doctor_id) DO UPDATE SET \
         lifetime_gross = doctor_wallets.lifetime_gross + EXCLUDED.lifetime_gross, \
         lifetime_fee_withheld = doctor_wallets.lifetime_fee_withheld + EXCLUDED.lifetime_fee_withheld, \
         lifetime_net = doctor_wallets.lifetime_net + EXCLUDED.lifetime_net, \
         pending_disbursement = doctor_wallets.pending_disbursement + EXCLUDED.lifetime_net, \
         updated_at = EXCLUDED.updated_at",
    )
    .bind(doctor_id)
    .bind(gross)
    .bind(fee)
    .bind(net)
    .bind(Utc::now())
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn mark_disbursed<'e, E: PgExecutor<'e>>(executor: E, doctor_id: Uuid, amount: &BigDecimal) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE doctor_wallets SET pending_disbursement = 0, last_disbursement_at = $2, last_disbursement_amount = $3, updated_at = $2 WHERE doctor_id = $1",
    )
    .bind(doctor_id)
    .bind(Utc::now())
    .bind(amount)
    .execute(executor)
    .await?;
    Ok(())
}

// ---- Disbursement batches/items ----

pub async fn insert_batch<'e, E: PgExecutor<'e>>(executor: E, batch: &DisbursementBatch) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO disbursement_batches (id, batch_number, initiated_by, period_start, period_end, total_doctors, total_gross, total_platform_fee, total_net_disbursed, status, created_at, completed_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)",
    )
    .bind(batch.id)
    .bind(&batch.batch_number)
    .bind(batch.initiated_by)
    .bind(batch.period_start)
    .bind(batch.period_end)
    .bind(batch.total_doctors)
    .bind(&batch.total_gross)
    .bind(&batch.total_platform_fee)
    .bind(&batch.total_net_disbursed)
    .bind(&batch.status)
    .bind(batch.created_at)
    .bind(batch.completed_at)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn insert_item<'e, E: PgExecutor<'e>>(executor: E, item: &DisbursementItem) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO disbursement_items (id, batch_id, doctor_id, gross_amount, platform_fee, net_amount, gateway, gateway_reference, status, created_at, confirmed_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)",
    )
    .bind(item.id)
    .bind(item.batch_id)
    .bind(item.doctor_id)
    .bind(&item.gross_amount)
    .bind(&item.platform_fee)
    .bind(&item.net_amount)
    .bind(item.gateway)
    .bind(&item.gateway_reference)
    .bind(&item.status)
    .bind(item.created_at)
    .bind(item.confirmed_at)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn list_items_for_doctor<'e, E: PgExecutor<'e>>(executor: E, doctor_id: Uuid) -> Result<Vec<DisbursementItem>, sqlx::Error> {
    sqlx::query_as::<_, DisbursementItem>(
        "SELECT id, batch_id, doctor_id, gross_amount, platform_fee, net_amount, gateway, gateway_reference, status, created_at, confirmed_at \
         FROM disbursement_items WHERE doctor_id = $1 ORDER BY created_at DESC",
    )
    .bind(doctor_id)
    .fetch_all(executor)
    .await
}
