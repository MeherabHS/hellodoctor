use crate::domain::models::*;
use crate::error::AppError;
use crate::repository::AppState;
use chrono::Utc;
use uuid::Uuid;

pub struct GrievanceService;

impl GrievanceService {
    /// Patient files formal medical grievance (Star-Rating-Free)
    pub fn submit_grievance(
        state: &AppState,
        patient_id: Uuid,
        consultation_id: Uuid,
        target_type: GrievanceTarget,
        category: String,
        claim_summary: String,
    ) -> Result<GrievanceReport, AppError> {
        let grv_id = Uuid::new_v4();
        let grv_num = format!("GRV-{}-{}", Utc::now().format("%Y%m%d"), &grv_id.to_string()[0..4].to_uppercase());

        let report = GrievanceReport {
            id: grv_id,
            grievance_number: grv_num,
            patient_id,
            consultation_id,
            target_type,
            category,
            claim_summary,
            status: GrievanceStatus::PendingReview,
            created_at: Utc::now(),
        };

        state.grievance_reports.write().insert(grv_id, report.clone());
        Ok(report)
    }

    /// Admin Medical Board Adjudicates: Disburse Refund
    pub fn adjudicate_refund(
        state: &AppState,
        grievance_id: Uuid,
        admin_id: Uuid,
        audit_notes: String,
    ) -> Result<GrievanceReport, AppError> {
        let mut reports = state.grievance_reports.write();
        let report = reports.get_mut(&grievance_id).ok_or_else(|| {
            AppError::NotFound("Grievance report not found.".into())
        })?;

        report.status = GrievanceStatus::Refunded;

        // Record board adjudication
        state.grievance_adjudications.write().insert(
            grievance_id,
            GrievanceAdjudication {
                id: Uuid::new_v4(),
                grievance_id,
                adjudicated_by_admin_id: admin_id,
                board_remedy: "Payment Refund Disbursed to Patient MFS Account".into(),
                action_type: "REFUND".into(),
                audit_notes,
                adjudicated_at: Utc::now(),
            },
        );

        // Update transaction to REFUNDED_TO_PATIENT
        let mut txns = state.transactions.write();
        if let Some(txn) = txns.values_mut().find(|t| t.appointment_id == report.consultation_id) {
            txn.payment_status = PaymentStatus::RefundedToPatient;
        }

        Ok(report.clone())
    }

    /// Admin Medical Board Adjudicates: Issue Compliance Warning
    pub fn adjudicate_warning(
        state: &AppState,
        grievance_id: Uuid,
        admin_id: Uuid,
        audit_notes: String,
    ) -> Result<GrievanceReport, AppError> {
        let mut reports = state.grievance_reports.write();
        let report = reports.get_mut(&grievance_id).ok_or_else(|| {
            AppError::NotFound("Grievance report not found.".into())
        })?;

        report.status = GrievanceStatus::Warned;

        state.grievance_adjudications.write().insert(
            grievance_id,
            GrievanceAdjudication {
                id: Uuid::new_v4(),
                grievance_id,
                adjudicated_by_admin_id: admin_id,
                board_remedy: "Internal Platform Compliance Warning Logged in Doctor Dossier".into(),
                action_type: "WARNING".into(),
                audit_notes,
                adjudicated_at: Utc::now(),
            },
        );

        Ok(report.clone())
    }
}
