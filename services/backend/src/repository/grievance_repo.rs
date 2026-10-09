use crate::domain::models::{
    FinancialRefundRequest, GrievanceAdjudication, GrievanceReport, GrievanceStatus,
};
use sqlx::PgExecutor;
use uuid::Uuid;

/// `grievance_number` is deliberately not supplied by the caller: the
/// column's own `DEFAULT` (migration 20261028000001) draws it from
/// `grievance_number_seq`, a Postgres sequence that is atomic under
/// concurrency by construction — two transactions can never receive the
/// same value, unlike the previous truncated-UUID scheme this replaced.
/// Returns the generated number so the caller can report it back to the
/// patient without a second round trip.
pub async fn insert<'e, E: PgExecutor<'e>>(
    executor: E,
    report: &GrievanceReport,
) -> Result<String, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO grievance_reports (id, patient_id, consultation_id, target_type, category, claim_summary, status, created_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8) RETURNING grievance_number",
    )
    .bind(report.id)
    .bind(report.patient_id)
    .bind(report.consultation_id)
    .bind(report.target_type)
    .bind(&report.category)
    .bind(&report.claim_summary)
    .bind(report.status)
    .bind(report.created_at)
    .fetch_one(executor)
    .await
}

pub async fn find_by_id<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
) -> Result<Option<GrievanceReport>, sqlx::Error> {
    sqlx::query_as::<_, GrievanceReport>(
        "SELECT id, grievance_number, patient_id, consultation_id, target_type, category, claim_summary, status, created_at FROM grievance_reports WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(executor)
    .await
}

/// Scoped lookup used by the admin consultation-timeline endpoint — a single
/// session's grievances, not the whole table (see `list_all` below, which
/// stays whole-table-fetch for the existing admin grievance-queue screen's
/// own N+1-avoidance reasons).
pub async fn find_by_consultation_id<'e, E: PgExecutor<'e>>(
    executor: E,
    consultation_id: Uuid,
) -> Result<Vec<GrievanceReport>, sqlx::Error> {
    sqlx::query_as::<_, GrievanceReport>(
        "SELECT id, grievance_number, patient_id, consultation_id, target_type, category, claim_summary, status, created_at \
         FROM grievance_reports WHERE consultation_id = $1 ORDER BY created_at DESC",
    )
    .bind(consultation_id)
    .fetch_all(executor)
    .await
}

pub async fn list_all<'e, E: PgExecutor<'e>>(
    executor: E,
) -> Result<Vec<GrievanceReport>, sqlx::Error> {
    sqlx::query_as::<_, GrievanceReport>(
        "SELECT id, grievance_number, patient_id, consultation_id, target_type, category, claim_summary, status, created_at FROM grievance_reports ORDER BY created_at DESC",
    )
    .fetch_all(executor)
    .await
}

/// Conditional on the grievance still being `PENDING_REVIEW` — the fix for a
/// real bug: nothing previously stopped a grievance from being adjudicated
/// twice, or re-decided to a different outcome after already being settled.
/// Returns true if this call actually transitioned it; the caller must treat
/// `false` as "already adjudicated" and reject rather than proceeding to
/// insert a second `grievance_adjudications` row / repeat any financial
/// effect (see `GrievanceService::adjudicate_*`, the only callers).
pub async fn set_status_if_pending<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    status: GrievanceStatus,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE grievance_reports SET status = $2 WHERE id = $1 AND status = 'PENDING_REVIEW'",
    )
    .bind(id)
    .bind(status)
    .execute(executor)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// All refund requests, newest-approved-first — gives Finance a queue instead
/// of the table being invisible after approval (GAP-C03).
pub async fn list_refund_requests<'e, E: PgExecutor<'e>>(
    executor: E,
) -> Result<Vec<FinancialRefundRequest>, sqlx::Error> {
    sqlx::query_as::<_, FinancialRefundRequest>(
        "SELECT id, grievance_id, transaction_id, approved_by, approved_at, currency, amount, previous_payment_status, status, provider_reference, refunded_at \
         FROM financial_refund_requests ORDER BY approved_at DESC",
    )
    .fetch_all(executor)
    .await
}

pub async fn find_refund_request<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
) -> Result<Option<FinancialRefundRequest>, sqlx::Error> {
    sqlx::query_as::<_, FinancialRefundRequest>(
        "SELECT id, grievance_id, transaction_id, approved_by, approved_at, currency, amount, previous_payment_status, status, provider_reference, refunded_at \
         FROM financial_refund_requests WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(executor)
    .await
}

/// Conditional on `from_status`, mirroring `set_status_if_pending` above —
/// prevents a double-transition (e.g. two concurrent "mark REFUNDED" calls)
/// from silently racing. `provider_reference`/`refunded_at` are required by
/// the table's own `CHECK` when the target status is `REFUNDED`; the caller
/// is responsible for passing them only in that case (enforced again here at
/// the DB layer regardless).
pub async fn transition_refund_request<'e, E: PgExecutor<'e>>(
    executor: E,
    id: Uuid,
    from_status: &str,
    to_status: &str,
    provider_reference: Option<&str>,
) -> Result<bool, sqlx::Error> {
    let refunded_at = if to_status == "REFUNDED" {
        Some(chrono::Utc::now())
    } else {
        None
    };
    let result = sqlx::query(
        "UPDATE financial_refund_requests SET status = $3, provider_reference = COALESCE($4, provider_reference), refunded_at = COALESCE($5, refunded_at) WHERE id = $1 AND status = $2",
    )
    .bind(id)
    .bind(from_status)
    .bind(to_status)
    .bind(provider_reference)
    .bind(refunded_at)
    .execute(executor)
    .await?;
    Ok(result.rows_affected() == 1)
}

pub async fn insert_adjudication<'e, E: PgExecutor<'e>>(
    executor: E,
    adj: &GrievanceAdjudication,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO grievance_adjudications (id, grievance_id, adjudicated_by_admin_id, board_remedy, action_type, audit_notes, adjudicated_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7)",
    )
    .bind(adj.id)
    .bind(adj.grievance_id)
    .bind(adj.adjudicated_by_admin_id)
    .bind(&adj.board_remedy)
    .bind(&adj.action_type)
    .bind(&adj.audit_notes)
    .bind(adj.adjudicated_at)
    .execute(executor)
    .await?;
    Ok(())
}
