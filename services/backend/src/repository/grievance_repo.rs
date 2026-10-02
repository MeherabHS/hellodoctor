use crate::domain::models::{GrievanceAdjudication, GrievanceReport, GrievanceStatus};
use sqlx::PgExecutor;
use uuid::Uuid;

pub async fn insert<'e, E: PgExecutor<'e>>(executor: E, report: &GrievanceReport) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO grievance_reports (id, grievance_number, patient_id, consultation_id, target_type, category, claim_summary, status, created_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)",
    )
    .bind(report.id)
    .bind(&report.grievance_number)
    .bind(report.patient_id)
    .bind(report.consultation_id)
    .bind(report.target_type)
    .bind(&report.category)
    .bind(&report.claim_summary)
    .bind(report.status)
    .bind(report.created_at)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn find_by_id<'e, E: PgExecutor<'e>>(executor: E, id: Uuid) -> Result<Option<GrievanceReport>, sqlx::Error> {
    sqlx::query_as::<_, GrievanceReport>(
        "SELECT id, grievance_number, patient_id, consultation_id, target_type, category, claim_summary, status, created_at FROM grievance_reports WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(executor)
    .await
}

pub async fn list_all<'e, E: PgExecutor<'e>>(executor: E) -> Result<Vec<GrievanceReport>, sqlx::Error> {
    sqlx::query_as::<_, GrievanceReport>(
        "SELECT id, grievance_number, patient_id, consultation_id, target_type, category, claim_summary, status, created_at FROM grievance_reports ORDER BY created_at DESC",
    )
    .fetch_all(executor)
    .await
}

pub async fn set_status<'e, E: PgExecutor<'e>>(executor: E, id: Uuid, status: GrievanceStatus) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE grievance_reports SET status = $2 WHERE id = $1")
        .bind(id)
        .bind(status)
        .execute(executor)
        .await?;
    Ok(())
}

pub async fn insert_adjudication<'e, E: PgExecutor<'e>>(executor: E, adj: &GrievanceAdjudication) -> Result<(), sqlx::Error> {
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
