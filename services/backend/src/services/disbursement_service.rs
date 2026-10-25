use crate::auth_context::AuthContext;
use crate::domain::ids::{DoctorProfileId, UserId};
use crate::domain::models::*;
use crate::error::AppError;
use crate::repository::{doctor_repo, transaction_repo, wallet_repo, AppState, DbRole};
use crate::services::audit_service::{AuditLogEntry, AuditService};
use bigdecimal::BigDecimal;
use chrono::{Datelike, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct DoctorWalletSummaryDto {
    pub doctor_name: String,
    pub license_number: String,
    pub total_gross: BigDecimal,
    pub platform_charge_percent: f64,
    pub withheld_fee: BigDecimal,
    pub final_net: BigDecimal,
    pub pending_disbursement: BigDecimal,
    pub finalized_unpaid: BigDecimal,
    pub scheduled_unpaid: BigDecimal,
    pub total_paid: BigDecimal,
    pub basis_note: String,
    pub currency: String,
    pub last_disbursement_at: Option<chrono::DateTime<Utc>>,
    pub last_disbursement_amount: Option<BigDecimal>,
}

pub struct DisbursementService;

impl DisbursementService {
    /// Retrieve doctor wallet summary with 20% platform charge explicitly debarred
    pub async fn get_doctor_wallet_summary(
        state: &AppState,
        ctx: &AuthContext,
        doctor_id: DoctorProfileId,
    ) -> Result<DoctorWalletSummaryDto, AppError> {
        let mut tx = state.begin_authed(ctx).await?;

        let doc = doctor_repo::find_by_id(&mut *tx, doctor_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Doctor profile not found.".into()))?;

        let wallet = wallet_repo::get_or_default(&mut *tx, doctor_id).await?;
        // `status <> 'CANCELLED'` (not `status = 'PAID'`): a statement already
        // allocated to a PENDING_APPROVAL/APPROVED/PROCESSING item is counted
        // in `scheduled_unpaid` below — counting it here too would double the
        // same obligation (caught by
        // test_financial_close_snapshot_adjustment_routing_and_approval after
        // wallet_repo::list_true_unpaid_totals exposed the same query shape
        // summed into one Admin KPI; see GAP-C01 in
        // docs/audit/DOCTOR_BUSINESS_LOGIC_GAP_AUDIT.md).
        let finalized_unpaid: BigDecimal = sqlx::query_scalar("SELECT COALESCE(SUM(s.final_payable),0) FROM monthly_payout_statements s WHERE s.doctor_id=$1 AND s.status='FINALIZED' AND NOT EXISTS(SELECT 1 FROM disbursement_items i WHERE i.statement_id=s.id AND i.status <> 'CANCELLED')")
            .bind(doctor_id).fetch_one(&mut *tx).await?;
        let (scheduled_unpaid,total_paid): (BigDecimal,BigDecimal) = sqlx::query_as("SELECT COALESCE(SUM(net_amount) FILTER(WHERE status IN ('PENDING_APPROVAL','APPROVED','PROCESSING','RECONCILIATION_REQUIRED')),0),COALESCE(SUM(net_amount) FILTER(WHERE status='PAID' AND gateway_reference IS NOT NULL AND paid_at IS NOT NULL AND provider_code <> 'DEV_SIMULATED'),0) FROM disbursement_items WHERE doctor_id=$1 AND currency='KES'")
            .bind(doctor_id).fetch_one(&mut *tx).await?;
        let last_disbursement = wallet_repo::last_paid_disbursement(&mut *tx, doctor_id).await?;
        tx.commit().await?;

        Ok(DoctorWalletSummaryDto {
            doctor_name: doc.full_name,
            license_number: doc.license_number,
            total_gross: wallet.lifetime_gross,
            platform_charge_percent: 20.0,
            withheld_fee: wallet.lifetime_fee_withheld,
            final_net: wallet.lifetime_net,
            // h05 remediation: doctor_wallets.pending_disbursement is a
            // write-once-incremented counter with no reachable decrement path
            // (wallet_repo::mark_disbursed has zero callers; see its own doc
            // comment and docs/audit/DOCTOR_BUSINESS_LOGIC_GAP_AUDIT.md
            // GAP-C01). finalized_unpaid+scheduled_unpaid, computed live two
            // lines above from current disbursement_items/statement status,
            // is the authoritative figure Admin's own views already use —
            // derived fresh each call, so it decrements automatically and
            // exactly once as an item moves to PAID, stays unchanged while
            // PROCESSING/APPROVED/etc., and is idempotent against a
            // duplicate payout-confirmation callback by construction.
            pending_disbursement: finalized_unpaid.clone() + scheduled_unpaid.clone(),
            finalized_unpaid,
            scheduled_unpaid,
            total_paid,
            basis_note: "Total calculation is based including the platform charge 20%.".into(),
            currency: "KES".into(),
            last_disbursement_at: last_disbursement.as_ref().map(|(at, _)| *at),
            last_disbursement_amount: last_disbursement.map(|(_, amount)| amount),
        })
    }

    /// Prepares unpaid, finalized monthly statements. Preparation never moves money.
    pub async fn initiate_monthly_disbursement(
        state: &AppState,
        ctx: &AuthContext,
        admin_id: UserId,
        period_start: NaiveDate,
        period_end: NaiveDate,
    ) -> Result<DisbursementBatch, AppError> {
        let next_month = if period_start.month() == 12 {
            NaiveDate::from_ymd_opt(period_start.year() + 1, 1, 1)
        } else {
            NaiveDate::from_ymd_opt(period_start.year(), period_start.month() + 1, 1)
        }
        .ok_or_else(|| AppError::Validation("Invalid accounting period.".into()))?;
        if period_start.day() != 1 || period_end != next_month.pred_opt().unwrap() {
            return Err(AppError::Validation(
                "Select one complete accounting month.".into(),
            ));
        }
        // Wallet settlement + disbursement batch writes are finance-worker-only
        // (see migrations/20261003000001_rls_hardening); the ordinary API role
        // has no UPDATE grant on doctor_wallets/transactions or INSERT on
        // disbursement_batches/disbursement_items.
        let mut tx = state.begin_authed_as(ctx, DbRole::FinanceWorker).await?;
        let period_id: Uuid = sqlx::query_scalar(
            "SELECT id FROM financial_periods WHERE currency='KES' AND period_year=$1 AND period_month=$2 AND status='FINALIZED'",
        ).bind(period_start.year() as i16).bind(period_start.month() as i16)
            .fetch_optional(&mut *tx).await?
            .ok_or_else(|| AppError::Conflict("Finalize the accounting period before preparing disbursement.".into()))?;

        let batch_id = Uuid::new_v4();
        let batch_num = format!(
            "DISB-{}-{}",
            Utc::now().format("%Y%m"),
            batch_id.to_string()[0..4].to_uppercase()
        );

        let mut total_gross = BigDecimal::from(0);
        let mut total_fee = BigDecimal::from(0);
        let mut total_net = BigDecimal::from(0);
        let mut total_doctors = 0;

        let statements = sqlx::query_as::<_, (Uuid, Uuid, BigDecimal, BigDecimal, BigDecimal, BigDecimal)>(
            "SELECT id, doctor_id, gross_eligible, platform_charge, adjustments, final_payable FROM monthly_payout_statements WHERE financial_period_id=$1 AND status='FINALIZED' AND final_payable > 0 ORDER BY doctor_id FOR UPDATE",
        ).bind(period_id).fetch_all(&mut *tx).await?;
        let mut items = Vec::new();

        for (statement_id, doctor_id, doctor_gross, doctor_fee, adjustments, doctor_net) in
            statements
        {
            let doctor_id = DoctorProfileId::from(doctor_id);
            let allocated: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM disbursement_items WHERE statement_id=$1 AND status <> 'CANCELLED')")
                .bind(statement_id).fetch_one(&mut *tx).await?;
            if allocated {
                continue;
            }
            // A disbursement is routed only to the doctor's active, provider-
            // bound Kenya destination. Without one, leave the obligation
            // pending for Finance review instead of claiming transactions.
            let payout_destination =
                wallet_repo::get_primary_payout_destination(&mut *tx, doctor_id).await?;
            let Some(payout_destination) = payout_destination else {
                continue;
            };
            if payout_destination.provider_code == "DEV_SIMULATED"
                && state.config.environment != "development"
            {
                continue;
            }
            let item_id = Uuid::new_v4();
            let source_ids: Vec<Uuid> = sqlx::query_scalar("SELECT transaction_id FROM monthly_statement_transactions WHERE statement_id=$1 ORDER BY transaction_id")
                .bind(statement_id).fetch_all(&mut *tx).await?;
            for transaction_id in source_ids {
                if !transaction_repo::assign_to_disbursement(&mut *tx, transaction_id, item_id)
                    .await?
                {
                    return Err(AppError::Conflict("A statement source is already allocated or no longer payable; Finance review is required.".into()));
                }
            }

            total_doctors += 1;
            total_gross += &doctor_gross;
            total_fee += &doctor_fee;
            total_net += &doctor_net;

            items.push(DisbursementItem {
                id: item_id,
                batch_id,
                doctor_id,
                gross_amount: doctor_gross,
                platform_fee: doctor_fee,
                net_amount: doctor_net,
                gateway: match payout_destination.method_type.as_str() {
                    "MPESA" => Gateway::Mpesa,
                    "BANK_CARD" => Gateway::Card,
                    "BANK_TRANSFER" => Gateway::BankTransfer,
                    _ => {
                        return Err(AppError::Validation(
                            "Unsupported Kenya payout destination.".into(),
                        ))
                    }
                },
                gateway_reference: None,
                status: "PENDING_APPROVAL".into(),
                created_at: Utc::now(),
                confirmed_at: None,
                payout_destination_id: Some(payout_destination.id),
                currency: "KES".into(),
                provider_reference_snapshot: Some(payout_destination.provider_reference),
                masked_destination_snapshot: Some(payout_destination.masked_destination),
                approved_by: None,
                approved_at: None,
                paid_at: None,
                reconciliation_state: "NOT_REQUIRED".into(),
                statement_id: Some(statement_id),
                adjustments,
                provider_code: Some(payout_destination.provider_code),
            });
        }

        if items.is_empty() {
            return Err(AppError::Conflict(
                "No unallocated finalized statements are ready for disbursement.".into(),
            ));
        }
        let batch = DisbursementBatch {
            id: batch_id,
            batch_number: batch_num,
            initiated_by: admin_id.into_inner(),
            period_start,
            period_end,
            total_doctors,
            total_gross,
            total_platform_fee: total_fee,
            total_net_disbursed: total_net,
            status: "PENDING_APPROVAL".into(),
            created_at: Utc::now(),
            completed_at: None,
        };
        // Batch row must exist before its items (FK), so insert it after totals are known but
        // before the item rows that reference it.
        wallet_repo::insert_batch(&mut *tx, &batch).await?;
        for item in &items {
            wallet_repo::insert_item(&mut *tx, item).await?;
        }

        // Written in the same FinanceWorker transaction as the batch/item
        // rows above (needs `hellodoctor_finance_worker`'s new INSERT grant
        // on `audit_events`, migrations/20261004000005) — not a separate
        // post-commit write, so the batch and its audit record commit or
        // roll back together. See docs/CHECKPOINT.md §10 (Pass 2).
        AuditService::log_in_tx(
            &mut *tx,
            AuditLogEntry::new(
                Some(admin_id),
                ctx.role,
                "DISBURSEMENT_INITIATED",
                "DISBURSEMENT_BATCH",
                Some(batch.id),
                "SUCCESS",
            )
            .with_after(serde_json::json!({
                "batch_number": batch.batch_number,
                "period_start": batch.period_start,
                "period_end": batch.period_end,
                "total_doctors": batch.total_doctors,
                "total_gross": batch.total_gross.to_string(),
                "total_platform_fee": batch.total_platform_fee.to_string(),
                "total_net_disbursed": batch.total_net_disbursed.to_string(),
            })),
        )
        .await?;

        tx.commit().await?;

        Ok(batch)
    }

    /// Admin/Finance confirmation that a disbursement item was genuinely paid
    /// by the provider — the write path `PAYOUT_PAID` notifications assume
    /// exists (the DB's `disbursement_paid_evidence` CHECK and
    /// `protect_disbursement_snapshot` trigger already enforce the
    /// `gateway_reference`/`paid_at`/non-`DEV_SIMULATED` invariants for any
    /// `PAID` row), but which no code path in this codebase ever actually
    /// performed — payout execution itself remains
    /// `DEFERRED_EXTERNAL_PAYMENT_INTEGRATION` (no live Mpesa/bank-transfer
    /// provider call exists anywhere). `gateway_reference` is supplied by the
    /// caller because it is Finance's out-of-band confirmation from the real
    /// provider statement, never fabricated here.
    ///
    /// Fires `PAYOUT_PAID` in the same transaction as the status transition
    /// — mirroring how `EARNING_CREDITED` is fired from the same transaction
    /// as its authoritative wallet credit in `consultation_service.rs` — so a
    /// rolled-back confirmation can never leave a phantom notification, and a
    /// retried call against an item that already left `APPROVED`/
    /// `PROCESSING` (including one already `PAID`) is a no-op here: the
    /// repository UPDATE affects zero rows, so this returns `Conflict`
    /// without ever reaching the notification call a second time.
    pub async fn confirm_disbursement_item_paid(
        state: &AppState,
        ctx: &AuthContext,
        admin_id: UserId,
        item_id: Uuid,
        gateway_reference: &str,
    ) -> Result<DisbursementItem, AppError> {
        let mut tx = state.begin_authed_as(ctx, DbRole::FinanceWorker).await?;

        let item =
            wallet_repo::confirm_disbursement_item_paid(&mut *tx, item_id, gateway_reference)
                .await?
                .ok_or_else(|| {
                    AppError::Conflict(
                        "Disbursement item is not eligible for payout confirmation: it must be \
                     APPROVED/PROCESSING with a real, non-DEV_SIMULATED payout destination."
                            .into(),
                    )
                })?;

        let doctor = doctor_repo::find_by_id(&mut *tx, item.doctor_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Doctor profile not found.".into()))?;

        crate::services::NotificationService::notify_payout_paid(
            &mut tx,
            doctor.user_id,
            item.id,
            &item.net_amount,
            &item.currency,
        )
        .await?;

        AuditService::log_in_tx(
            &mut *tx,
            AuditLogEntry::new(
                Some(admin_id),
                ctx.role,
                "DISBURSEMENT_PAID_CONFIRMED",
                "DISBURSEMENT_ITEM",
                Some(item.id),
                "SUCCESS",
            )
            .with_after(serde_json::json!({
                "gateway_reference": item.gateway_reference,
                "net_amount": item.net_amount.to_string(),
                "currency": item.currency,
            })),
        )
        .await?;

        tx.commit().await?;
        Ok(item)
    }
}
