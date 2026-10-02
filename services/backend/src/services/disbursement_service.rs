use crate::domain::models::*;
use crate::error::AppError;
use crate::repository::AppState;
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
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
    pub fn get_doctor_wallet_summary(
        state: &AppState,
        doctor_id: Uuid,
    ) -> Result<DoctorWalletSummaryDto, AppError> {
        let docs = state.doctor_profiles.read();
        let doc = docs.get(&doctor_id).ok_or_else(|| {
            AppError::NotFound("Doctor profile not found.".into())
        })?;

        let wallets = state.doctor_wallets.read();
        let wallet = wallets.get(&doctor_id).cloned().unwrap_or(DoctorWallet {
            doctor_id,
            lifetime_gross: BigDecimal::from(0),
            lifetime_fee_withheld: BigDecimal::from(0),
            lifetime_net: BigDecimal::from(0),
            pending_disbursement: BigDecimal::from(0),
            last_disbursement_at: None,
            last_disbursement_amount: BigDecimal::from(0),
            updated_at: Utc::now(),
        });

        Ok(DoctorWalletSummaryDto {
            doctor_name: doc.full_name.clone(),
            license_number: doc.license_number.clone(),
            total_gross: wallet.lifetime_gross,
            platform_charge_percent: 20.0,
            withheld_fee: wallet.lifetime_fee_withheld,
            final_net: wallet.lifetime_net,
            pending_disbursement: wallet.pending_disbursement,
            basis_note: "Total calculation is based including the platform charge 20%.".into(),
        })
    }

    /// Finance Admin initiates monthly batch disbursement
    pub fn initiate_monthly_disbursement(
        state: &AppState,
        admin_id: Uuid,
        period_start: NaiveDate,
        period_end: NaiveDate,
    ) -> Result<DisbursementBatch, AppError> {
        let batch_id = Uuid::new_v4();
        let batch_num = format!("DISB-{}-{}", Utc::now().format("%Y%m"), &batch_id.to_string()[0..4].to_uppercase());

        let mut total_gross = BigDecimal::from(0);
        let mut total_fee = BigDecimal::from(0);
        let mut total_net = BigDecimal::from(0);
        let mut total_doctors = 0;

        let mut wallets = state.doctor_wallets.write();
        let mut items = Vec::new();

        for (doc_id, wallet) in wallets.iter_mut() {
            if wallet.pending_disbursement > BigDecimal::from(0) {
                total_doctors += 1;
                let net = wallet.pending_disbursement.clone();
                let gross = &net / bigdecimal::BigDecimal::from_str("0.80").unwrap();
                let fee = &gross - &net;

                total_gross += &gross;
                total_fee += &fee;
                total_net += &net;

                wallet.last_disbursement_at = Some(Utc::now());
                wallet.last_disbursement_amount = net.clone();
                wallet.pending_disbursement = BigDecimal::from(0);

                items.push(DisbursementItem {
                    id: Uuid::new_v4(),
                    batch_id,
                    doctor_id: *doc_id,
                    gross_amount: gross,
                    platform_fee: fee,
                    net_amount: net,
                    gateway: Gateway::Bkash,
                    gateway_reference: Some(format!("DISB-REF-{}", Uuid::new_v4().to_string()[0..8].to_uppercase())),
                    status: "CONFIRMED".into(),
                    created_at: Utc::now(),
                    confirmed_at: Some(Utc::now()),
                });
            }
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

        state.disbursement_batches.write().insert(batch_id, batch.clone());
        state.disbursement_items.write().insert(batch_id, items);

        Ok(batch)
    }
}
