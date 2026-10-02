use crate::domain::models::*;
use crate::error::AppError;
use crate::repository::{doctor_repo, transaction_repo, wallet_repo, AppState};
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, Utc};
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
    pub basis_note: String,
}

pub struct DisbursementService;

impl DisbursementService {
    /// Retrieve doctor wallet summary with 20% platform charge explicitly debarred
    pub async fn get_doctor_wallet_summary(state: &AppState, doctor_id: Uuid) -> Result<DoctorWalletSummaryDto, AppError> {
        let doc = doctor_repo::find_by_id(&state.db, doctor_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Doctor profile not found.".into()))?;

        let wallet = wallet_repo::get_or_default(&state.db, doctor_id).await?;

        Ok(DoctorWalletSummaryDto {
            doctor_name: doc.full_name,
            license_number: doc.license_number,
            total_gross: wallet.lifetime_gross,
            platform_charge_percent: 20.0,
            withheld_fee: wallet.lifetime_fee_withheld,
            final_net: wallet.lifetime_net,
            pending_disbursement: wallet.pending_disbursement,
            basis_note: "Total calculation is based including the platform charge 20%.".into(),
        })
    }

    /// Finance Admin initiates monthly batch disbursement. Batch/item totals are summed
    /// directly from the settled-but-undisbursed transaction rows for each doctor — never
    /// re-derived by division — so they always satisfy the DB's `chk_fee_math` invariant.
    pub async fn initiate_monthly_disbursement(
        state: &AppState,
        admin_id: Uuid,
        period_start: NaiveDate,
        period_end: NaiveDate,
    ) -> Result<DisbursementBatch, AppError> {
        let mut tx = state.db.begin().await?;

        let batch_id = Uuid::new_v4();
        let batch_num = format!("DISB-{}-{}", Utc::now().format("%Y%m"), batch_id.to_string()[0..4].to_uppercase());

        let mut total_gross = BigDecimal::from(0);
        let mut total_fee = BigDecimal::from(0);
        let mut total_net = BigDecimal::from(0);
        let mut total_doctors = 0;

        let wallets = wallet_repo::list_all(&mut *tx).await?;
        let mut items = Vec::new();

        for wallet in wallets.into_iter().filter(|w| w.pending_disbursement > 0) {
            let settled_txns = transaction_repo::list_settled_undisbursed_for_doctor(&mut *tx, wallet.doctor_id).await?;
            if settled_txns.is_empty() {
                continue;
            }

            let mut doctor_gross = BigDecimal::from(0);
            let mut doctor_fee = BigDecimal::from(0);
            let mut doctor_net = BigDecimal::from(0);

            for txn in &settled_txns {
                doctor_gross += &txn.gross_amount;
                doctor_fee += &txn.platform_fee_amount;
                doctor_net += &txn.net_amount;
                transaction_repo::mark_status(&mut *tx, txn.id, PaymentStatus::Disbursed).await?;
            }

            total_doctors += 1;
            total_gross += &doctor_gross;
            total_fee += &doctor_fee;
            total_net += &doctor_net;

            wallet_repo::mark_disbursed(&mut *tx, wallet.doctor_id, &doctor_net).await?;

            items.push(DisbursementItem {
                id: Uuid::new_v4(),
                batch_id,
                doctor_id: wallet.doctor_id,
                gross_amount: doctor_gross,
                platform_fee: doctor_fee,
                net_amount: doctor_net,
                gateway: Gateway::Bkash,
                gateway_reference: Some(format!("DISB-REF-{}", Uuid::new_v4().to_string()[0..8].to_uppercase())),
                status: "CONFIRMED".into(),
                created_at: Utc::now(),
                confirmed_at: Some(Utc::now()),
            });
        }

        let batch = DisbursementBatch {
            id: batch_id,
            batch_number: batch_num,
            initiated_by: admin_id,
            period_start,
            period_end,
            total_doctors,
            total_gross,
            total_platform_fee: total_fee,
            total_net_disbursed: total_net,
            status: "COMPLETED".into(),
            created_at: Utc::now(),
            completed_at: Some(Utc::now()),
        };
        // Batch row must exist before its items (FK), so insert it after totals are known but
        // before the item rows that reference it.
        wallet_repo::insert_batch(&mut *tx, &batch).await?;
        for item in &items {
            wallet_repo::insert_item(&mut *tx, item).await?;
        }

        tx.commit().await?;

        Ok(batch)
    }
}
