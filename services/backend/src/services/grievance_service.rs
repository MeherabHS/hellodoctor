use crate::auth_context::AuthContext;
use crate::domain::ids::{PatientProfileId, UserId};
use crate::domain::models::*;
use crate::error::AppError;
use crate::repository::{
    appointment_repo, consultation_repo, grievance_repo, transaction_repo, wallet_repo, AppState,
    DbRole,
};
use crate::services::audit_service::{AuditLogEntry, AuditService};
use chrono::Utc;
use uuid::Uuid;

pub struct GrievanceService;

impl GrievanceService {
    /// Patient files formal medical grievance (Star-Rating-Free). `consultation_id` must be
    /// a real `consultation_sessions.id` — the FK constraint enforces this at the DB level.
    pub async fn submit_grievance(
        state: &AppState,
        ctx: &AuthContext,
        patient_id: PatientProfileId,
        consultation_id: Uuid,
        target_type: GrievanceTarget,
        category: String,
        claim_summary: String,
    ) -> Result<GrievanceReport, AppError> {
        let mut tx = state.begin_authed(ctx).await?;

        consultation_repo::find_by_id(&mut *tx, consultation_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Consultation session not found.".into()))?;

        let grv_id = Uuid::new_v4();

        let mut report = GrievanceReport {
            id: grv_id,
            // Placeholder: `grievance_repo::insert` ignores this and returns
            // the value Postgres's own sequence-backed DEFAULT actually
            // generated (see that function's doc comment).
            grievance_number: String::new(),
            patient_id,
            consultation_id,
            target_type,
            category,
            claim_summary,
            status: GrievanceStatus::PendingReview,
            created_at: Utc::now(),
        };

        report.grievance_number = grievance_repo::insert(&mut *tx, &report).await?;
        tx.commit().await?;
        Ok(report)
    }

    /// Admin approves a refund obligation. Approval is not provider payment.
    ///
    /// Two fixes from the Pass 2 security/correctness audit (docs/CHECKPOINT.md
    /// §10) live here: (1) the status transition is now conditional on the
    /// grievance still being `PENDING_REVIEW` — previously nothing stopped a
    /// second adjudication call from silently overwriting an already-decided
    /// outcome (or inserting a second `grievance_adjudications` row); (2) the
    /// audit-trail write now happens inside this same transaction
    /// (`AuditService::log_in_tx`) instead of a separate one opened after
    /// commit, so a failure writing the audit row rolls back the whole
    /// adjudication instead of leaving a mutation with no audit record.
    pub async fn adjudicate_refund(
        state: &AppState,
        ctx: &AuthContext,
        grievance_id: Uuid,
        admin_id: UserId,
        audit_notes: String,
    ) -> Result<GrievanceReport, AppError> {
        // Adjudication writes (all three outcomes, even non-financial ones) are
        // finance-worker-only — the only role ever granted anything on
        // grievance_adjudications, matching docs/DATABASE_SCHEMA.md's grant design.
        let mut tx = state.begin_authed_as(ctx, DbRole::FinanceWorker).await?;

        let mut report = grievance_repo::find_by_id(&mut *tx, grievance_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Grievance report not found.".into()))?;
        let old_status = report.status;

        let session = consultation_repo::find_by_id(&mut *tx, report.consultation_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Consultation not found.".into()))?;
        let txn = transaction_repo::find_by_appointment(&mut *tx, session.appointment_id)
            .await?
            .ok_or_else(|| AppError::Conflict("There is no payment to refund.".into()))?;
        // Serialize against settlement/allocation. Closed accounting needs an
        // explicit reviewed correction, never a silent rewrite of a statement.
        sqlx::query("SELECT pg_advisory_xact_lock(61012, EXTRACT(YEAR FROM $1::timestamptz)::integer * 100 + EXTRACT(MONTH FROM $1::timestamptz)::integer)")
            .bind(txn.created_at).execute(&mut *tx).await?;
        sqlx::query("SELECT id FROM transactions WHERE id=$1 FOR UPDATE")
            .bind(txn.id)
            .fetch_one(&mut *tx)
            .await?;
        let txn = transaction_repo::find_by_appointment(&mut *tx, session.appointment_id)
            .await?
            .unwrap();
        let closed: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM monthly_statement_transactions WHERE transaction_id=$1)",
        )
        .bind(txn.id)
        .fetch_one(&mut *tx)
        .await?;
        if closed
            || !matches!(
                txn.payment_status,
                PaymentStatus::PaymentHeld | PaymentStatus::SettledToDoctor
            )
        {
            return Err(AppError::Conflict(
                "This payment requires a reviewed accounting correction before refund approval."
                    .into(),
            ));
        }
        if txn.currency != "KES" {
            return Err(AppError::Conflict(
                "This deployment supports KES refunds only.".into(),
            ));
        }

        let transitioned = grievance_repo::set_status_if_pending(
            &mut *tx,
            grievance_id,
            GrievanceStatus::RefundApproved,
        )
        .await?;
        if !transitioned {
            return Err(AppError::Conflict(
                "This grievance has already been adjudicated and cannot be decided again.".into(),
            ));
        }
        report.status = GrievanceStatus::RefundApproved;

        // If this transaction had already been settled into the doctor's
        // wallet (`doctor_complete_consultation` credited it at completion
        // time), reverse exactly that credit now — otherwise the doctor's
        // lifetime earnings totals permanently overstate true earnings with
        // no corrective ledger entry (GAP-C02). A still-`PAYMENT_HELD`
        // transaction was never credited, so there is nothing to reverse.
        if txn.payment_status == PaymentStatus::SettledToDoctor {
            let appointment = appointment_repo::find_by_id(&mut *tx, session.appointment_id)
                .await?
                .ok_or_else(|| AppError::NotFound("Appointment not found.".into()))?;
            wallet_repo::debit(
                &mut *tx,
                appointment.doctor_id,
                &txn.gross_amount,
                &txn.platform_fee_amount,
                &txn.net_amount,
            )
            .await?;
        }

        grievance_repo::insert_adjudication(
            &mut *tx,
            &GrievanceAdjudication {
                id: Uuid::new_v4(),
                grievance_id,
                adjudicated_by_admin_id: admin_id.into_inner(),
                board_remedy:
                    "Refund approved; provider execution pending. No money has been returned."
                        .into(),
                action_type: "REFUND".into(),
                audit_notes,
                adjudicated_at: Utc::now(),
            },
        )
        .await?;

        sqlx::query("INSERT INTO financial_refund_requests (id,grievance_id,transaction_id,approved_by,currency,amount,previous_payment_status) VALUES ($1,$2,$3,$4,$5,$6,$7)")
            .bind(Uuid::new_v4()).bind(grievance_id).bind(txn.id).bind(admin_id).bind(&txn.currency)
            .bind(&txn.gross_amount).bind(serde_json::to_value(txn.payment_status).unwrap().as_str().unwrap())
            .execute(&mut *tx).await?;
        transaction_repo::mark_status(&mut *tx, txn.id, PaymentStatus::RefundPending).await?;

        AuditService::log_in_tx(
            &mut *tx,
            AuditLogEntry::new(
                Some(admin_id),
                ctx.role,
                "GRIEVANCE_REFUND_APPROVED",
                "GRIEVANCE_REPORT",
                Some(report.id),
                "SUCCESS",
            )
            .with_before(serde_json::json!({ "status": old_status }))
            .with_after(serde_json::json!({ "status": report.status, "refund_status": "APPROVED", "money_returned": false, "wallet_reversed": txn.payment_status == PaymentStatus::SettledToDoctor })),
        )
        .await?;

        tx.commit().await?;
        Ok(report)
    }

    /// Admin Medical Board Adjudicates: Issue Compliance Warning. See
    /// `adjudicate_refund`'s doc comment for the fixes shared across all three.
    pub async fn adjudicate_warning(
        state: &AppState,
        ctx: &AuthContext,
        grievance_id: Uuid,
        admin_id: UserId,
        audit_notes: String,
    ) -> Result<GrievanceReport, AppError> {
        let mut tx = state.begin_authed_as(ctx, DbRole::FinanceWorker).await?;

        let mut report = grievance_repo::find_by_id(&mut *tx, grievance_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Grievance report not found.".into()))?;
        let old_status = report.status;

        let transitioned =
            grievance_repo::set_status_if_pending(&mut *tx, grievance_id, GrievanceStatus::Warned)
                .await?;
        if !transitioned {
            return Err(AppError::Conflict(
                "This grievance has already been adjudicated and cannot be decided again.".into(),
            ));
        }
        report.status = GrievanceStatus::Warned;

        grievance_repo::insert_adjudication(
            &mut *tx,
            &GrievanceAdjudication {
                id: Uuid::new_v4(),
                grievance_id,
                adjudicated_by_admin_id: admin_id.into_inner(),
                board_remedy: "Internal Platform Compliance Warning Logged in Doctor Dossier"
                    .into(),
                action_type: "WARNING".into(),
                audit_notes,
                adjudicated_at: Utc::now(),
            },
        )
        .await?;

        AuditService::log_in_tx(
            &mut *tx,
            AuditLogEntry::new(
                Some(admin_id),
                ctx.role,
                "GRIEVANCE_WARN",
                "GRIEVANCE_REPORT",
                Some(report.id),
                "SUCCESS",
            )
            .with_before(serde_json::json!({ "status": old_status }))
            .with_after(serde_json::json!({ "status": report.status, "board_remedy": "Internal Platform Compliance Warning Logged in Doctor Dossier" })),
        )
        .await?;

        tx.commit().await?;
        Ok(report)
    }

    /// Admin Medical Board Adjudicates: Dismiss (no action warranted). See
    /// `adjudicate_refund`'s doc comment for the fixes shared across all three.
    pub async fn adjudicate_dismiss(
        state: &AppState,
        ctx: &AuthContext,
        grievance_id: Uuid,
        admin_id: UserId,
        audit_notes: String,
    ) -> Result<GrievanceReport, AppError> {
        let mut tx = state.begin_authed_as(ctx, DbRole::FinanceWorker).await?;

        let mut report = grievance_repo::find_by_id(&mut *tx, grievance_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Grievance report not found.".into()))?;
        let old_status = report.status;

        let transitioned = grievance_repo::set_status_if_pending(
            &mut *tx,
            grievance_id,
            GrievanceStatus::Dismissed,
        )
        .await?;
        if !transitioned {
            return Err(AppError::Conflict(
                "This grievance has already been adjudicated and cannot be decided again.".into(),
            ));
        }
        report.status = GrievanceStatus::Dismissed;

        grievance_repo::insert_adjudication(
            &mut *tx,
            &GrievanceAdjudication {
                id: Uuid::new_v4(),
                grievance_id,
                adjudicated_by_admin_id: admin_id.into_inner(),
                board_remedy: "Claim Reviewed and Dismissed — No Compliance Action Warranted"
                    .into(),
                action_type: "DISMISS".into(),
                audit_notes,
                adjudicated_at: Utc::now(),
            },
        )
        .await?;

        AuditService::log_in_tx(
            &mut *tx,
            AuditLogEntry::new(
                Some(admin_id),
                ctx.role,
                "GRIEVANCE_DISMISS",
                "GRIEVANCE_REPORT",
                Some(report.id),
                "SUCCESS",
            )
            .with_before(serde_json::json!({ "status": old_status }))
            .with_after(serde_json::json!({ "status": report.status, "board_remedy": "Claim Reviewed and Dismissed — No Compliance Action Warranted" })),
        )
        .await?;

        tx.commit().await?;
        Ok(report)
    }

    /// Finance's visibility into every refund approved so far (GAP-C03: an
    /// approved refund previously had no operator-facing surface at all once
    /// `adjudicate_refund` ran).
    pub async fn list_refund_requests(
        state: &AppState,
        ctx: &AuthContext,
    ) -> Result<Vec<FinancialRefundRequest>, AppError> {
        let mut tx = state.begin_authed_as(ctx, DbRole::FinanceWorker).await?;
        let requests = grievance_repo::list_refund_requests(&mut *tx).await?;
        tx.commit().await?;
        Ok(requests)
    }

    /// Gives a previously-approved refund the same visible, auditable
    /// lifecycle disbursements already have — `APPROVED → PROCESSING →
    /// REFUNDED`, with `FAILED`/`RECONCILIATION_REQUIRED` as escape hatches
    /// Finance can retry from. Without this, `financial_refund_requests.status`
    /// sat at `APPROVED` forever with zero operator action possible (GAP-C03).
    /// This only ever updates `status`/`provider_reference`/`refunded_at` —
    /// never the amount, currency, or which transaction/grievance it
    /// belongs to (those columns are not even granted UPDATE at the DB
    /// layer; see migrations/20261018000001_audit_corrections).
    pub async fn transition_refund_request(
        state: &AppState,
        ctx: &AuthContext,
        admin_id: UserId,
        refund_request_id: Uuid,
        to_status: &str,
        provider_reference: Option<String>,
    ) -> Result<FinancialRefundRequest, AppError> {
        const ALLOWED: [&str; 4] = [
            "PROCESSING",
            "REFUNDED",
            "FAILED",
            "RECONCILIATION_REQUIRED",
        ];
        if !ALLOWED.contains(&to_status) {
            return Err(AppError::Validation(format!(
                "status must be one of {ALLOWED:?}."
            )));
        }
        if to_status == "REFUNDED"
            && provider_reference
                .as_deref()
                .is_none_or(|r| r.trim().is_empty())
        {
            return Err(AppError::Validation(
                "A provider_reference is required to mark a refund REFUNDED.".into(),
            ));
        }

        let mut tx = state.begin_authed_as(ctx, DbRole::FinanceWorker).await?;
        let existing = grievance_repo::find_refund_request(&mut *tx, refund_request_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Refund request not found.".into()))?;
        if existing.status == "REFUNDED" {
            return Err(AppError::Conflict(
                "This refund is already marked REFUNDED and cannot be changed further.".into(),
            ));
        }
        let transitioned = grievance_repo::transition_refund_request(
            &mut *tx,
            refund_request_id,
            &existing.status,
            to_status,
            provider_reference.as_deref(),
        )
        .await?;
        if !transitioned {
            return Err(AppError::Conflict(
                "This refund request was changed by another request; reload and retry.".into(),
            ));
        }

        AuditService::log_in_tx(
            &mut *tx,
            AuditLogEntry::new(
                Some(admin_id),
                ctx.role,
                "REFUND_REQUEST_TRANSITIONED",
                "FINANCIAL_REFUND_REQUEST",
                Some(refund_request_id),
                "SUCCESS",
            )
            .with_before(serde_json::json!({ "status": existing.status }))
            .with_after(serde_json::json!({ "status": to_status, "provider_reference": provider_reference })),
        )
        .await?;

        let updated = grievance_repo::find_refund_request(&mut *tx, refund_request_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Refund request not found.".into()))?;
        tx.commit().await?;
        Ok(updated)
    }
}
