use crate::domain::ids::DoctorProfileId;
use crate::domain::models::{
    DisbursementBatch, DisbursementItem, DoctorPayoutDestination, DoctorPayoutMethod, DoctorWallet,
    PayoutMethodType,
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use sqlx::PgExecutor;
use std::collections::HashMap;
use uuid::Uuid;

pub async fn get_or_default<'e, E: PgExecutor<'e>>(
    executor: E,
    doctor_id: DoctorProfileId,
) -> Result<DoctorWallet, sqlx::Error> {
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

pub async fn list_payout_destinations<'e, E: PgExecutor<'e>>(
    executor: E,
    doctor_id: DoctorProfileId,
) -> Result<Vec<DoctorPayoutDestination>, sqlx::Error> {
    sqlx::query_as::<_, DoctorPayoutDestination>(
        "SELECT id, doctor_id, provider_code, method_type, currency, provider_reference, masked_destination, binding_status, verification_status, payout_capable, platform_payment_method_id, is_primary, created_at, updated_at FROM doctor_payout_destinations WHERE doctor_id = $1 ORDER BY is_primary DESC, created_at DESC",
    )
    .bind(doctor_id)
    .fetch_all(executor)
    .await
}

pub async fn get_primary_payout_destination<'e, E: PgExecutor<'e>>(
    executor: E,
    doctor_id: DoctorProfileId,
) -> Result<Option<DoctorPayoutDestination>, sqlx::Error> {
    sqlx::query_as::<_, DoctorPayoutDestination>(
        "SELECT id, doctor_id, provider_code, method_type, currency, provider_reference, masked_destination, binding_status, verification_status, payout_capable, platform_payment_method_id, is_primary, created_at, updated_at FROM doctor_payout_destinations WHERE doctor_id = $1 AND currency = 'KES' AND is_primary = TRUE AND binding_status = 'ACTIVE' AND payout_capable = TRUE LIMIT 1",
    )
    .bind(doctor_id)
    .fetch_optional(executor)
    .await
}

/// Development-only binding fixture. It creates a server-generated reference;
/// no wallet/card number, PIN, OTP, or provider secret is accepted or stored.
pub async fn create_dev_payout_destination(
    connection: &mut sqlx::PgConnection,
    doctor_id: DoctorProfileId,
    method_type: &str,
    masked_destination: &str,
) -> Result<DoctorPayoutDestination, sqlx::Error> {
    // Serialize primary changes per Doctor. An unreferenced modifying CTE does
    // not guarantee the UPDATE runs before INSERT checks the partial unique index.
    sqlx::query("SELECT id FROM doctor_profiles WHERE id=$1 FOR UPDATE")
        .bind(doctor_id)
        .execute(&mut *connection)
        .await?;
    sqlx::query("UPDATE doctor_payout_destinations SET is_primary=FALSE, updated_at=NOW() WHERE doctor_id=$1 AND currency='KES'")
        .bind(doctor_id).execute(&mut *connection).await?;
    sqlx::query_as::<_, DoctorPayoutDestination>(
        "INSERT INTO doctor_payout_destinations (doctor_id, provider_code, method_type, currency, provider_reference, masked_destination, binding_status, verification_status, payout_capable, is_primary)
         VALUES ($1, 'DEV_SIMULATED', $2, 'KES', 'dev-payout-' || gen_random_uuid()::text, $3, 'ACTIVE', 'DEV_SIMULATED', TRUE, TRUE)
         RETURNING id, doctor_id, provider_code, method_type, currency, provider_reference, masked_destination, binding_status, verification_status, payout_capable, platform_payment_method_id, is_primary, created_at, updated_at",
    )
    .bind(doctor_id)
    .bind(method_type)
    .bind(masked_destination)
    .fetch_one(connection)
    .await
}

/// The production-grade binding this codebase's own `set_payout_method`
/// error message refers to ("use the configured Kenya provider binding
/// flow") but which never existed — confirmed by grepping the entire
/// codebase for any such flow before writing this. Deliberately targets
/// `doctor_payout_destinations` (the table `list_payout_routing` and the
/// whole disbursement pipeline actually read), NOT the separate legacy
/// `doctor_payout_methods` table `set_payout_method` writes to — that
/// table has no live reader at all, confirmed directly, which is the real
/// reason a doctor's saved legacy method never appeared in Admin's payout
/// routing view regardless of any readiness-badge bug on the frontend.
///
/// `verification_status` is deliberately `AWAITING_PROVIDER_INTEGRATION`,
/// not `DEV_SIMULATED` — this accepts the doctor's real, caller-supplied
/// reference (encrypted, same as every other secret in this codebase), it
/// is simply not yet verified against a live provider API because none is
/// wired up ("the body waiting for the heart"). `payout_capable=TRUE`
/// matches how every other payout-destination row already behaves in this
/// pre-real-provider system — real money movement remains simulated
/// platform-wide regardless of this flag, not specifically gated by it.
#[allow(clippy::too_many_arguments)]
pub async fn bind_payout_destination(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    doctor_id: DoctorProfileId,
    provider_code: &str,
    method_type: &str,
    provider_reference_encrypted: &str,
    masked_destination: &str,
    verification_status: &str,
    platform_payment_method_id: Uuid,
    make_primary: bool,
) -> Result<DoctorPayoutDestination, sqlx::Error> {
    sqlx::query("SELECT id FROM doctor_profiles WHERE id=$1 FOR UPDATE")
        .bind(doctor_id)
        .execute(&mut **tx)
        .await?;
    if make_primary {
        sqlx::query("UPDATE doctor_payout_destinations SET is_primary=FALSE, updated_at=NOW() WHERE doctor_id=$1 AND currency='KES'")
            .bind(doctor_id).execute(&mut **tx).await?;
    }
    sqlx::query_as::<_, DoctorPayoutDestination>(
        "INSERT INTO doctor_payout_destinations (doctor_id, provider_code, method_type, currency, provider_reference, masked_destination, binding_status, verification_status, payout_capable, platform_payment_method_id, is_primary)
         VALUES ($1, $2, $3, 'KES', $4, $5, 'ACTIVE', $6, TRUE, $7, $8)
         RETURNING id, doctor_id, provider_code, method_type, currency, provider_reference, masked_destination, binding_status, verification_status, payout_capable, platform_payment_method_id, is_primary, created_at, updated_at",
    )
    .bind(doctor_id)
    .bind(provider_code)
    .bind(method_type)
    .bind(provider_reference_encrypted)
    .bind(masked_destination)
    .bind(verification_status)
    .bind(platform_payment_method_id)
    .bind(make_primary)
    .fetch_one(&mut **tx)
    .await
}

pub async fn set_primary_payout_destination(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    doctor_id: DoctorProfileId,
    destination_id: Uuid,
) -> Result<bool, sqlx::Error> {
    sqlx::query("SELECT id FROM doctor_profiles WHERE id=$1 FOR UPDATE")
        .bind(doctor_id)
        .execute(&mut **tx)
        .await?;
    sqlx::query("UPDATE doctor_payout_destinations SET is_primary=FALSE, updated_at=NOW() WHERE doctor_id=$1 AND currency='KES'")
        .bind(doctor_id)
        .execute(&mut **tx)
        .await?;
    let result = sqlx::query(
        "UPDATE doctor_payout_destinations SET is_primary=TRUE, updated_at=NOW() \
         WHERE id=$1 AND doctor_id=$2 AND binding_status='ACTIVE'",
    )
    .bind(destination_id)
    .bind(doctor_id)
    .execute(&mut **tx)
    .await?;
    Ok(result.rows_affected() == 1)
}

pub async fn list_all<'e, E: PgExecutor<'e>>(
    executor: E,
) -> Result<Vec<DoctorWallet>, sqlx::Error> {
    sqlx::query_as::<_, DoctorWallet>(
        "SELECT doctor_id, lifetime_gross, lifetime_fee_withheld, lifetime_net, pending_disbursement, last_disbursement_at, last_disbursement_amount, updated_at FROM doctor_wallets",
    )
    .fetch_all(executor)
    .await
}

pub async fn insert_default<'e, E: PgExecutor<'e>>(
    executor: E,
    doctor_id: DoctorProfileId,
) -> Result<(), sqlx::Error> {
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
    doctor_id: DoctorProfileId,
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

/// Reverses exactly what an earlier `credit()` call for this same
/// transaction added — used when a refund is approved for a transaction that
/// had already been settled into the doctor's wallet, so the doctor's
/// lifetime totals don't permanently overstate true earnings once the
/// underlying payment is refunded (see GAP-C02 in
/// docs/audit/DOCTOR_BUSINESS_LOGIC_GAP_AUDIT.md). The authoritative monthly
/// statement does not need a matching adjustment: the refunded transaction's
/// `payment_status` leaving `SETTLED_TO_DOCTOR` already excludes it from the
/// next period-close's gross/net aggregation.
pub async fn debit<'e, E: PgExecutor<'e>>(
    executor: E,
    doctor_id: DoctorProfileId,
    gross: &BigDecimal,
    fee: &BigDecimal,
    net: &BigDecimal,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE doctor_wallets SET \
         lifetime_gross = lifetime_gross - $2, \
         lifetime_fee_withheld = lifetime_fee_withheld - $3, \
         lifetime_net = lifetime_net - $4, \
         pending_disbursement = pending_disbursement - $4, \
         updated_at = $5 \
         WHERE doctor_id = $1",
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

/// Decrements `pending_disbursement` by exactly `amount` rather than
/// flat-setting it to 0 — matters now that `disbursement_service.rs` claims
/// transactions one at a time (`transaction_repo::claim_for_disbursement`):
/// two concurrent disbursement runs for the same doctor can each legitimately
/// claim a *partial* share of the pending total, and each must only
/// subtract its own share, not assume it was the only disbursement in
/// flight. In the common (single-run, no contention) case `amount` equals
/// the full `pending_disbursement`, so this is behavior-preserving there.
pub async fn mark_disbursed<'e, E: PgExecutor<'e>>(
    executor: E,
    doctor_id: DoctorProfileId,
    amount: &BigDecimal,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE doctor_wallets SET pending_disbursement = pending_disbursement - $3, last_disbursement_at = $2, last_disbursement_amount = $3, updated_at = $2 WHERE doctor_id = $1",
    )
    .bind(doctor_id)
    .bind(Utc::now())
    .bind(amount)
    .execute(executor)
    .await?;
    Ok(())
}

/// Every doctor's true "earned, not yet provider-confirmed paid" total — the
/// same `finalized_unpaid + scheduled_unpaid` definition already used by
/// `DisbursementService::get_doctor_wallet_summary`, computed for every
/// doctor in one query. Deliberately **not** `doctor_wallets.pending_disbursement`:
/// that column is only ever incremented (by `credit`), never decremented by
/// any reachable code path (`mark_disbursed`'s only caller was removed from
/// the disbursement architecture), so it misrepresents "unpaid" as "lifetime
/// gross ever earned" — see docs/audit/DOCTOR_BUSINESS_LOGIC_GAP_AUDIT.md
/// GAP-C01. This is the source Admin's "unpaid earnings" views must use.
pub async fn list_true_unpaid_totals<'e, E: PgExecutor<'e>>(
    executor: E,
) -> Result<HashMap<DoctorProfileId, BigDecimal>, sqlx::Error> {
    let rows: Vec<(DoctorProfileId, BigDecimal)> = sqlx::query_as(
        "SELECT doctor_id, SUM(unpaid) FROM (
           SELECT s.doctor_id, s.final_payable AS unpaid
             FROM monthly_payout_statements s
            WHERE s.status = 'FINALIZED'
              AND NOT EXISTS (SELECT 1 FROM disbursement_items i WHERE i.statement_id = s.id AND i.status <> 'CANCELLED')
           UNION ALL
           SELECT i.doctor_id, i.net_amount AS unpaid
             FROM disbursement_items i
            WHERE i.status IN ('PENDING_APPROVAL','APPROVED','PROCESSING','RECONCILIATION_REQUIRED')
         ) combined GROUP BY doctor_id",
    )
    .fetch_all(executor)
    .await?;
    Ok(rows.into_iter().collect())
}

/// The doctor's most recent authoritative (provider-confirmed) payout, read
/// straight from `disbursement_items` rather than
/// `doctor_wallets.last_disbursement_*` (which nothing ever writes — see
/// `list_true_unpaid_totals` above). Populates
/// `WalletDto.last_disbursement_at/last_disbursement_amount`, which the
/// Flutter client already parses but the backend previously never sent (see
/// addendum A2 in docs/audit/DOCTOR_BACKEND_CONTRACT_AUDIT.md).
pub async fn last_paid_disbursement<'e, E: PgExecutor<'e>>(
    executor: E,
    doctor_id: DoctorProfileId,
) -> Result<Option<(DateTime<Utc>, BigDecimal)>, sqlx::Error> {
    sqlx::query_as(
        "SELECT paid_at, net_amount FROM disbursement_items \
         WHERE doctor_id = $1 AND status = 'PAID' AND paid_at IS NOT NULL AND provider_code <> 'DEV_SIMULATED' \
         ORDER BY paid_at DESC LIMIT 1",
    )
    .bind(doctor_id)
    .fetch_optional(executor)
    .await
}

// ---- Disbursement batches/items ----

pub async fn insert_batch<'e, E: PgExecutor<'e>>(
    executor: E,
    batch: &DisbursementBatch,
) -> Result<(), sqlx::Error> {
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

pub async fn insert_item<'e, E: PgExecutor<'e>>(
    executor: E,
    item: &DisbursementItem,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO disbursement_items (id, batch_id, doctor_id, gross_amount, platform_fee, net_amount, gateway, gateway_reference, status, created_at, confirmed_at, payout_destination_id, currency, provider_reference_snapshot, masked_destination_snapshot, reconciliation_state, statement_id, adjustments, provider_code) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19)",
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
    .bind(item.payout_destination_id)
    .bind(&item.currency)
    .bind(&item.provider_reference_snapshot)
    .bind(&item.masked_destination_snapshot)
    .bind(&item.reconciliation_state)
    .bind(item.statement_id)
    .bind(&item.adjustments)
    .bind(&item.provider_code)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn list_items_for_doctor<'e, E: PgExecutor<'e>>(
    executor: E,
    doctor_id: DoctorProfileId,
) -> Result<Vec<DisbursementItem>, sqlx::Error> {
    sqlx::query_as::<_, DisbursementItem>(
        "SELECT id, batch_id, doctor_id, gross_amount, platform_fee, net_amount, gateway, gateway_reference, status, created_at, confirmed_at, payout_destination_id, currency, provider_reference_snapshot, masked_destination_snapshot, approved_by, approved_at, paid_at, reconciliation_state, statement_id, adjustments, provider_code \
         FROM disbursement_items WHERE doctor_id = $1 ORDER BY created_at DESC",
    )
    .bind(doctor_id)
    .fetch_all(executor)
    .await
}

pub async fn approve_disbursement_item<'e, E: PgExecutor<'e>>(
    executor: E,
    item_id: Uuid,
    admin_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("UPDATE disbursement_items SET status='APPROVED', approved_by=$2, approved_at=NOW() WHERE id=$1 AND status='PENDING_APPROVAL'")
        .bind(item_id).bind(admin_id).execute(executor).await?;
    Ok(result.rows_affected() == 1)
}

/// Transitions a disbursement item to the authoritative, provider-confirmed
/// `PAID` state. Only ever moves an item out of `APPROVED`/`PROCESSING` — a
/// `PENDING_APPROVAL` item must be approved first, and an already-`PAID` or
/// `CANCELLED` item never moves again (the `WHERE status IN (...)` makes a
/// repeat call a no-op: `Ok(None)`, not a duplicate transition). A
/// `DEV_SIMULATED`/missing `provider_code` destination is rejected outright —
/// no transition, no caller-visible row — matching
/// `NotificationService::notify_payout_paid`'s "never call this from a
/// `DEV_SIMULATED` path" contract and the DB's own
/// `disbursement_paid_evidence` CHECK / `protect_disbursement_snapshot`
/// trigger (migrations/20261012000001, 20261017000001), which would reject
/// the same write at the constraint layer if this guard were ever removed.
pub async fn confirm_disbursement_item_paid<'e, E: PgExecutor<'e>>(
    executor: E,
    item_id: Uuid,
    gateway_reference: &str,
) -> Result<Option<DisbursementItem>, sqlx::Error> {
    sqlx::query_as::<_, DisbursementItem>(
        "UPDATE disbursement_items SET status='PAID', gateway_reference=$2, paid_at=NOW() \
         WHERE id=$1 AND status IN ('APPROVED','PROCESSING') \
           AND provider_code IS NOT NULL AND provider_code <> 'DEV_SIMULATED' \
         RETURNING id, batch_id, doctor_id, gross_amount, platform_fee, net_amount, gateway, gateway_reference, status, created_at, confirmed_at, payout_destination_id, currency, provider_reference_snapshot, masked_destination_snapshot, approved_by, approved_at, paid_at, reconciliation_state, statement_id, adjustments, provider_code",
    )
    .bind(item_id)
    .bind(gateway_reference)
    .fetch_optional(executor)
    .await
}

// ---- Doctor payout method (where Finance sends a scheduled disbursement) ----

pub async fn get_payout_method<'e, E: PgExecutor<'e>>(
    executor: E,
    doctor_id: DoctorProfileId,
) -> Result<Option<DoctorPayoutMethod>, sqlx::Error> {
    sqlx::query_as::<_, DoctorPayoutMethod>(
        "SELECT doctor_id, method_type, display_label, account_identifier_encrypted, account_identifier_last4, created_at, updated_at \
         FROM doctor_payout_methods WHERE doctor_id = $1",
    )
    .bind(doctor_id)
    .fetch_optional(executor)
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn upsert_payout_method<'e, E: PgExecutor<'e>>(
    executor: E,
    doctor_id: DoctorProfileId,
    method_type: PayoutMethodType,
    display_label: &str,
    account_identifier_encrypted: &[u8],
    account_identifier_last4: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO doctor_payout_methods (doctor_id, method_type, display_label, account_identifier_encrypted, account_identifier_last4, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $6) \
         ON CONFLICT (doctor_id) DO UPDATE SET \
         method_type = EXCLUDED.method_type, \
         display_label = EXCLUDED.display_label, \
         account_identifier_encrypted = EXCLUDED.account_identifier_encrypted, \
         account_identifier_last4 = EXCLUDED.account_identifier_last4, \
         updated_at = EXCLUDED.updated_at",
    )
    .bind(doctor_id)
    .bind(method_type)
    .bind(display_label)
    .bind(account_identifier_encrypted)
    .bind(account_identifier_last4)
    .bind(Utc::now())
    .execute(executor)
    .await?;
    Ok(())
}
