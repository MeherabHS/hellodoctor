use crate::domain::models::*;
use crate::error::AppError;
use crate::repository::{consultation_repo, grievance_repo, transaction_repo, AppState};
use chrono::Utc;
use uuid::Uuid;

pub struct GrievanceService;

impl GrievanceService {
    /// Patient files formal medical grievance (Star-Rating-Free). `consultation_id` must be
    /// a real `consultation_sessions.id` — the FK constraint enforces this at the DB level.
    pub async fn submit_grievance(
        state: &AppState,
        patient_id: Uuid,
        consultation_id: Uuid,
        target_type: GrievanceTarget,
        category: String,
        claim_summary: String,
    ) -> Result<GrievanceReport, AppError> {
        consultation_repo::find_by_id(&state.db, consultation_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Consultation session not found.".into()))?;

        let grv_id = Uuid::new_v4();
        let grv_num = format!("GRV-{}-{}", Utc::now().format("%Y%m%d"), grv_id.to_string()[0..4].to_uppercase());

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

        grievance_repo::insert(&state.db, &report).await?;
        Ok(report)
    }

    /// Admin Medical Board Adjudicates: Disburse Refund
    pub async fn adjudicate_refund(state: &AppState, grievance_id: Uuid, admin_id: Uuid, audit_notes: String) -> Result<GrievanceReport, AppError> {
        let mut tx = state.db.begin().await?;

        let mut report = grievance_repo::find_by_id(&mut *tx, grievance_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Grievance report not found.".into()))?;

        grievance_repo::set_status(&mut *tx, grievance_id, GrievanceStatus::Refunded).await?;
        report.status = GrievanceStatus::Refunded;

        grievance_repo::insert_adjudication(
            &mut *tx,
            &GrievanceAdjudication {
                id: Uuid::new_v4(),
                grievance_id,
                adjudicated_by_admin_id: admin_id,
                board_remedy: "Payment Refund Disbursed to Patient MFS Account".into(),
                action_type: "REFUND".into(),
                audit_notes,
                adjudicated_at: Utc::now(),
            },
        )
        .await?;

        if let Some(session) = consultation_repo::find_by_id(&mut *tx, report.consultation_id).await? {
            if let Some(txn) = transaction_repo::find_by_appointment(&mut *tx, session.appointment_id).await? {
                transaction_repo::mark_status(&mut *tx, txn.id, PaymentStatus::RefundedToPatient).await?;
            }
        }

        tx.commit().await?;
        Ok(report)
    }

    /// Admin Medical Board Adjudicates: Issue Compliance Warning
    pub async fn adjudicate_warning(state: &AppState, grievance_id: Uuid, admin_id: Uuid, audit_notes: String) -> Result<GrievanceReport, AppError> {
        let mut tx = state.db.begin().await?;

        let mut report = grievance_repo::find_by_id(&mut *tx, grievance_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Grievance report not found.".into()))?;

        grievance_repo::set_status(&mut *tx, grievance_id, GrievanceStatus::Warned).await?;
        report.status = GrievanceStatus::Warned;

        grievance_repo::insert_adjudication(
            &mut *tx,
            &GrievanceAdjudication {
                id: Uuid::new_v4(),
                grievance_id,
                adjudicated_by_admin_id: admin_id,
                board_remedy: "Internal Platform Compliance Warning Logged in Doctor Dossier".into(),
                action_type: "WARNING".into(),
                audit_notes,
                adjudicated_at: Utc::now(),
            },
        )
        .await?;

        tx.commit().await?;
        Ok(report)
    }

    /// Admin Medical Board Adjudicates: Dismiss (no action warranted)
    pub async fn adjudicate_dismiss(state: &AppState, grievance_id: Uuid, admin_id: Uuid, audit_notes: String) -> Result<GrievanceReport, AppError> {
        let mut tx = state.db.begin().await?;

        let mut report = grievance_repo::find_by_id(&mut *tx, grievance_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Grievance report not found.".into()))?;

        grievance_repo::set_status(&mut *tx, grievance_id, GrievanceStatus::Dismissed).await?;
        report.status = GrievanceStatus::Dismissed;

        grievance_repo::insert_adjudication(
            &mut *tx,
            &GrievanceAdjudication {
                id: Uuid::new_v4(),
                grievance_id,
                adjudicated_by_admin_id: admin_id,
                board_remedy: "Claim Reviewed and Dismissed — No Compliance Action Warranted".into(),
                action_type: "DISMISS".into(),
                audit_notes,
                adjudicated_at: Utc::now(),
            },
        )
        .await?;

        tx.commit().await?;
        Ok(report)
    }
}
