//! Q&A answer audit (product owner, 2026-11-14; docs/admin-ui/ADMIN_REDESIGN_PLAN.md).
//!
//! An Admin lists doctors' answers to patient questions, marks an answer
//! APPROVED or FLAGGED with a note, and sees every earlier review. A FLAGGED
//! answer is hidden from patients (`patient_handlers::list_answered_questions`)
//! until it is approved again. The question's patient author is never read
//! here: only the question text, the doctor and dates.
use crate::abac::{self, Action};
use crate::auth_context::AuthContext;
use crate::error::AppError;
use crate::repository::AppState;
use crate::services::audit_service::{AuditLogEntry, AuditService};
use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize)]
struct ApiResponse<T> {
    success: bool,
    data: T,
}

#[derive(Debug, Deserialize)]
pub struct AnswerListQuery {
    /// UNREVIEWED, APPROVED, FLAGGED or ALL (default).
    pub status: Option<String>,
    pub search: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewRequest {
    /// APPROVED or FLAGGED.
    pub decision: String,
    pub note: String,
}

fn status_filter(raw: Option<&str>) -> Result<&'static str, AppError> {
    match raw.unwrap_or("ALL") {
        "ALL" => Ok("ALL"),
        "UNREVIEWED" => Ok("UNREVIEWED"),
        "APPROVED" => Ok("APPROVED"),
        "FLAGGED" => Ok("FLAGGED"),
        _ => Err(AppError::Validation(
            "status must be UNREVIEWED, APPROVED, FLAGGED or ALL.".into(),
        )),
    }
}

/// GET /api/v1/admin/qna/answers
pub async fn list_answers(
    State(state): State<AppState>,
    ctx: AuthContext,
    Query(q): Query<AnswerListQuery>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ReviewQnaAnswers)?;
    let status = status_filter(q.status.as_deref())?;
    let limit = q.limit.unwrap_or(20).clamp(1, 50);
    let offset = q.offset.unwrap_or(0).max(0);
    let search = q.search.as_deref().map(str::trim).unwrap_or("");
    if search.chars().count() > 160 {
        return Err(AppError::Validation(
            "Search is limited to 160 characters.".into(),
        ));
    }
    let pattern = format!(
        "%{}%",
        search
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    );
    let mut tx = state.begin_authed(&ctx).await?;
    let rows: Vec<(serde_json::Value,)> = sqlx::query_as(
        "SELECT jsonb_build_object(\
           'answer_id',a.id,'question_id',q.id,'question_text',q.question_text,'category',q.category,\
           'asked_at',q.created_at,'answer_text',a.answer_text,'answered_at',a.created_at,\
           'doctor_id',a.doctor_id,'doctor_name',d.full_name,'doctor_specialty',d.primary_specialty,\
           'audit_status',a.audit_status,'audit_updated_at',a.audit_updated_at,\
           'last_review',(SELECT jsonb_build_object('decision',r.decision,'note',r.note,'reviewed_at',r.reviewed_at,'reviewed_by',r.reviewed_by) \
              FROM health_answer_reviews r WHERE r.answer_id=a.id ORDER BY r.reviewed_at DESC, r.id DESC LIMIT 1)) \
         FROM health_answers a JOIN health_questions q ON q.id=a.question_id \
         LEFT JOIN doctor_profiles d ON d.id=a.doctor_id \
         WHERE ($1='ALL' OR a.audit_status=$1) \
           AND ($2='%%' OR q.question_text ILIKE $2 OR a.answer_text ILIKE $2 OR d.full_name ILIKE $2) \
         ORDER BY a.created_at DESC, a.id LIMIT $3 OFFSET $4",
    )
    .bind(status)
    .bind(&pattern)
    .bind(limit)
    .bind(offset)
    .fetch_all(&mut *tx)
    .await?;
    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM health_answers a JOIN health_questions q ON q.id=a.question_id \
         LEFT JOIN doctor_profiles d ON d.id=a.doctor_id \
         WHERE ($1='ALL' OR a.audit_status=$1) \
           AND ($2='%%' OR q.question_text ILIKE $2 OR a.answer_text ILIKE $2 OR d.full_name ILIKE $2)",
    )
    .bind(status)
    .bind(&pattern)
    .fetch_one(&mut *tx)
    .await?;
    let counts: Vec<(String, i64)> =
        sqlx::query_as("SELECT audit_status, COUNT(*) FROM health_answers GROUP BY audit_status")
            .fetch_all(&mut *tx)
            .await?;
    tx.commit().await?;
    let count = |s: &str| {
        counts
            .iter()
            .find(|(k, _)| k == s)
            .map(|(_, n)| *n)
            .unwrap_or(0)
    };
    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "items": rows.into_iter().map(|(v,)| v).collect::<Vec<_>>(),
            "total": total,
            "limit": limit,
            "offset": offset,
            "counts": {
                "UNREVIEWED": count("UNREVIEWED"),
                "APPROVED": count("APPROVED"),
                "FLAGGED": count("FLAGGED"),
                "ALL": counts.iter().map(|(_, n)| n).sum::<i64>(),
            },
        }),
    }))
}

/// GET /api/v1/admin/qna/answers/:id/reviews — every review, newest first.
pub async fn list_reviews(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ReviewQnaAnswers)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let rows: Vec<(serde_json::Value,)> = sqlx::query_as(
        "SELECT jsonb_build_object('id',id,'decision',decision,'note',note,'reviewed_by',reviewed_by,'reviewed_at',reviewed_at) \
         FROM health_answer_reviews WHERE answer_id=$1 ORDER BY reviewed_at DESC, id DESC",
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(ApiResponse {
        success: true,
        data: rows.into_iter().map(|(v,)| v).collect::<Vec<_>>(),
    }))
}

/// POST /api/v1/admin/qna/answers/:id/reviews — APPROVED or FLAGGED with a note.
pub async fn review_answer(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<ReviewRequest>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ReviewQnaAnswers)?;
    let decision = match payload.decision.as_str() {
        "APPROVED" => "APPROVED",
        "FLAGGED" => "FLAGGED",
        _ => {
            return Err(AppError::Validation(
                "decision must be APPROVED or FLAGGED.".into(),
            ))
        }
    };
    let note =
        crate::domain::validation::validate_admin_text_field(&payload.note, "Review note", 1000)?;
    if note.is_empty() {
        return Err(AppError::Validation("A review note is required.".into()));
    }
    let mut tx = state.begin_authed(&ctx).await?;
    let previous: Option<String> =
        sqlx::query_scalar("SELECT audit_status FROM health_answers WHERE id=$1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
    let previous = previous.ok_or_else(|| AppError::NotFound("Answer not found.".into()))?;
    sqlx::query("UPDATE health_answers SET audit_status=$2, audit_updated_at=now() WHERE id=$1")
        .bind(id)
        .bind(decision)
        .execute(&mut *tx)
        .await?;
    let review_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO health_answer_reviews (id, answer_id, decision, note, reviewed_by) VALUES ($1,$2,$3,$4,$5)",
    )
    .bind(review_id)
    .bind(id)
    .bind(decision)
    .bind(&note)
    .bind(ctx.user_id)
    .execute(&mut *tx)
    .await?;
    AuditService::log_in_tx(
        &mut *tx,
        AuditLogEntry::new(
            Some(ctx.user_id),
            ctx.role,
            if decision == "FLAGGED" {
                "QNA_ANSWER_FLAGGED"
            } else {
                "QNA_ANSWER_APPROVED"
            },
            "HEALTH_ANSWER",
            Some(id),
            "SUCCESS",
        )
        .with_before(serde_json::json!({ "audit_status": previous }))
        .with_after(serde_json::json!({ "audit_status": decision })),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "answer_id": id,
            "review_id": review_id,
            "audit_status": decision,
            "hidden_from_patients": decision == "FLAGGED",
        }),
    }))
}
