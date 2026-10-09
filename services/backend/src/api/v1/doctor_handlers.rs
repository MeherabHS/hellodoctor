use crate::abac::{self, Action};
use crate::auth_context::AuthContext;
use crate::domain::ids::DoctorProfileId;
use crate::error::AppError;
use crate::repository::{doctor_repo, AppState};
use crate::services::audit_service::{AuditLogEntry, AuditService};
use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct DoctorQuery {
    pub modality: Option<String>,
    pub specialty: Option<String>,
    pub search: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: T,
}

pub async fn list_doctors(
    State(state): State<AppState>,
    ctx: AuthContext,
    Query(query): Query<DoctorQuery>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::AnyAuthenticated)?;
    if let Some(ref modality) = query.modality {
        if modality != "VIDEO" && modality != "CHAT" {
            return Err(AppError::Validation(
                "modality must be VIDEO or CHAT.".into(),
            ));
        }
    }
    let list = sqlx::query_as::<_, doctor_repo::PublicDoctorProfile>("SELECT * FROM doctor_public_directory WHERE professional_verification_status='VERIFIED' AND ($1::text IS NULL OR ($1='VIDEO' AND consultation_fee_video > 0) OR ($1='CHAT' AND consultation_fee_chat > 0)) AND ($2::text IS NULL OR lower(primary_specialty)=lower($2)) AND ($3::text IS NULL OR full_name ILIKE '%' || $3 || '%' OR current_hospital ILIKE '%' || $3 || '%') ORDER BY full_name")
        .bind(query.modality).bind(query.specialty).bind(query.search).fetch_all(&state.db).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: list,
    }))
}

pub async fn get_doctor(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::AnyAuthenticated)?;
    let id = DoctorProfileId::from(id);
    let doc = doctor_repo::find_public(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Doctor profile not found.".into()))?;

    Ok(Json(ApiResponse {
        success: true,
        data: doc,
    }))
}

pub async fn get_doctor_slots(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::AnyAuthenticated)?;
    let slots = doctor_repo::list_slots_for_doctor(&state.db, DoctorProfileId::from(id)).await?;
    Ok(Json(ApiResponse {
        success: true,
        data: slots,
    }))
}

#[derive(Debug, Deserialize)]
pub struct PageQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub status: Option<String>,
}

fn page(q: &PageQuery) -> (i64, i64) {
    (
        q.limit.unwrap_or(25).clamp(1, 100),
        q.offset.unwrap_or(0).max(0),
    )
}

pub async fn get_own_profile(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::DoctorRead)?;
    let id = ctx.doctor_profile_id(&state).await?;
    let mut tx = state.begin_authed(&ctx).await?;
    let doc = doctor_repo::find_by_id(&mut *tx, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Doctor profile not found.".into()))?;
    tx.commit().await?;
    Ok(Json(ApiResponse {
        success: true,
        data: doc,
    }))
}

#[derive(Debug, Deserialize)]
pub struct DutyRequest {
    pub is_on_duty: bool,
}

#[derive(Debug, Serialize)]
pub struct DutyStatus {
    pub is_on_duty: bool,
}

pub async fn get_own_duty(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::DoctorRead)?;
    let id = ctx.doctor_profile_id(&state).await?;
    let mut tx = state.begin_authed(&ctx).await?;
    let doc = doctor_repo::find_by_id(&mut *tx, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Doctor profile not found.".into()))?;
    tx.commit().await?;
    Ok(Json(ApiResponse {
        success: true,
        data: DutyStatus {
            is_on_duty: doc.is_on_duty,
        },
    }))
}

pub async fn update_own_duty(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(body): Json<DutyRequest>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::DoctorManageOwnSchedule)?;
    let id = if body.is_on_duty {
        ctx.verified_doctor_profile_id(&state).await?
    } else {
        ctx.doctor_profile_id(&state).await?
    };
    let mut tx = state.begin_authed(&ctx).await?;
    let result = sqlx::query("UPDATE doctor_profiles SET is_on_duty = $2, updated_at = $3 WHERE id = $1 AND deleted_at IS NULL")
        .bind(id).bind(body.is_on_duty).bind(Utc::now()).execute(&mut *tx).await?;
    tx.commit().await?;
    if result.rows_affected() != 1 {
        return Err(AppError::NotFound("Doctor profile not found.".into()));
    }
    Ok(Json(ApiResponse {
        success: true,
        data: DutyStatus {
            is_on_duty: body.is_on_duty,
        },
    }))
}

pub async fn list_own_slots(
    State(state): State<AppState>,
    ctx: AuthContext,
    Query(q): Query<PageQuery>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::DoctorRead)?;
    let id = ctx.doctor_profile_id(&state).await?;
    let (limit, offset) = page(&q);
    let mut tx = state.begin_authed(&ctx).await?;
    let rows = sqlx::query("SELECT s.id, s.doctor_id, s.start_time, s.end_time, s.status::text AS status, s.created_at, a.id AS appointment_id, a.patient_id, p.full_name AS patient_display_name FROM doctor_schedule_slots s LEFT JOIN appointments a ON a.slot_id = s.id AND a.status <> 'CANCELLED' LEFT JOIN patient_profiles p ON p.id = a.patient_id WHERE s.doctor_id = $1 ORDER BY s.start_time LIMIT $2 OFFSET $3")
        .bind(id).bind(limit).bind(offset).fetch_all(&mut *tx).await?;
    tx.commit().await?;
    let data: Vec<_> = rows.into_iter().map(|r| json!({"id": r.get::<Uuid,_>("id"), "doctor_id": r.get::<Uuid,_>("doctor_id"), "start_time": r.get::<DateTime<Utc>,_>("start_time"), "end_time": r.get::<DateTime<Utc>,_>("end_time"), "status": r.get::<String,_>("status"), "created_at": r.get::<DateTime<Utc>,_>("created_at"), "appointment_id": r.get::<Option<Uuid>,_>("appointment_id"), "patient_id": r.get::<Option<Uuid>,_>("patient_id"), "patient_display_name": r.get::<Option<String>,_>("patient_display_name")})).collect();
    Ok(Json(ApiResponse {
        success: true,
        data,
    }))
}

#[derive(Debug, Deserialize)]
pub struct CreateSlotRequest {
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
}

pub async fn create_own_slot(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(body): Json<CreateSlotRequest>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::DoctorManageOwnSchedule)?;
    if body.end_time <= body.start_time {
        return Err(AppError::Validation(
            "end_time must be after start_time.".into(),
        ));
    }
    if body.start_time <= Utc::now() {
        return Err(AppError::Validation("Choose a future start time.".into()));
    }
    let id = ctx.verified_doctor_profile_id(&state).await?;
    let mut tx = state.begin_authed(&ctx).await?;
    sqlx::query("SELECT id FROM doctor_profiles WHERE id=$1 FOR UPDATE")
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
    let overlap = sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM doctor_schedule_slots WHERE doctor_id=$1 AND status <> 'CANCELLED' AND start_time < $3 AND end_time > $2)")
        .bind(id).bind(body.start_time).bind(body.end_time).fetch_one(&mut *tx).await?;
    if overlap {
        return Err(AppError::Conflict(
            "Schedule slot overlaps an existing slot.".into(),
        ));
    }
    let slot_id = Uuid::new_v4();
    sqlx::query("INSERT INTO doctor_schedule_slots (id, doctor_id, start_time, end_time, status, created_at) VALUES ($1,$2,$3,$4,'AVAILABLE',$5)")
        .bind(slot_id).bind(id).bind(body.start_time).bind(body.end_time).bind(Utc::now()).execute(&mut *tx).await?;
    AuditService::log_in_tx(
        &mut *tx,
        AuditLogEntry::new(
            Some(ctx.user_id),
            ctx.role,
            "SLOT_CREATED",
            "SCHEDULE_SLOT",
            Some(slot_id),
            "SUCCESS",
        )
        .with_after(json!({"doctor_id": id, "start_time": body.start_time,
            "end_time": body.end_time, "status": "AVAILABLE", "source": "doctor_schedule"})),
    )
    .await?;
    tx.commit().await?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(ApiResponse {
            success: true,
            data: json!({"id": slot_id, "doctor_id": id, "start_time": body.start_time, "end_time": body.end_time, "status": "AVAILABLE"}),
        }),
    ))
}

pub async fn cancel_own_slot(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(slot_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::DoctorManageOwnSchedule)?;
    let id = ctx.doctor_profile_id(&state).await?;
    let mut tx = state.begin_authed(&ctx).await?;
    let result = sqlx::query("UPDATE doctor_schedule_slots SET status='CANCELLED' WHERE id=$1 AND doctor_id=$2 AND status IN ('AVAILABLE','BLOCKED')")
        .bind(slot_id).bind(id).execute(&mut *tx).await?;
    tx.commit().await?;
    if result.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "Only an own, unbooked available or blocked slot can be cancelled.".into(),
        ));
    }
    Ok(Json(ApiResponse {
        success: true,
        data: json!({"slot_id": slot_id, "status": "CANCELLED"}),
    }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OwnSlotAvailability {
    Available,
    Blocked,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateOwnSlotRequest {
    pub status: OwnSlotAvailability,
}

pub async fn update_own_slot(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(slot_id): Path<Uuid>,
    Json(body): Json<UpdateOwnSlotRequest>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::DoctorManageOwnSchedule)?;
    let id = ctx.verified_doctor_profile_id(&state).await?;
    let blocked = matches!(body.status, OwnSlotAvailability::Blocked);
    let mut tx = state.begin_authed(&ctx).await?;
    let changed = doctor_repo::set_own_slot_blocked(&mut *tx, slot_id, id, blocked).await?;
    if !changed {
        return Err(AppError::Conflict(
            "This slot cannot change availability. Refresh your schedule.".into(),
        ));
    }
    tx.commit().await?;
    Ok(Json(ApiResponse {
        success: true,
        data: json!({"slot_id": slot_id, "status": if blocked {"BLOCKED"} else {"AVAILABLE"}}),
    }))
}

pub async fn list_own_appointments(
    State(state): State<AppState>,
    ctx: AuthContext,
    Query(q): Query<PageQuery>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::DoctorRead)?;
    let id = ctx.verified_doctor_profile_id(&state).await?;
    let (limit, offset) = page(&q);
    let mut tx = state.begin_authed(&ctx).await?;
    // Default Queue ordering (approved product rule, not just a Flutter sort):
    // eligible UPCOMING consultations (CONFIRMED/WAITING/IN_CONSULTATION) always
    // come first, nearest-start-time first; everything else (COMPLETED,
    // CANCELLED, EXPIRED, PREMATURE_TERMINATION, DISPUTED, REFUNDED) follows,
    // newest-first, and can never displace an upcoming item regardless of its
    // own start_time. `a.id` is the deterministic tie-break for two
    // consultations sharing the same `start_time`. Applied unconditionally
    // (not just when `$4` is NULL) — with a single status filter active this
    // same CASE collapses to a plain ASC or DESC ordering for that one status,
    // so there's no need to branch the query shape.
    let rows = sqlx::query("SELECT a.id, a.appointment_number, a.patient_id, a.doctor_id, a.slot_id, a.modality::text AS modality, a.status::text AS status, a.consultation_fee, a.clinical_outcome, a.completed_at, a.created_at, a.updated_at, s.start_time, s.end_time, s.start_time - INTERVAL '15 minutes' AS history_access_from, CASE WHEN a.status='COMPLETED' THEN COALESCE(cs.ended_at,a.completed_at) + INTERVAL '24 hours' END AS history_access_until, doctor_history_context_allowed(a.id,$5) AS history_access_active, CASE WHEN a.status='COMPLETED' AND (COALESCE(cs.ended_at,a.completed_at) IS NULL OR CURRENT_TIMESTAMP >= COALESCE(cs.ended_at,a.completed_at) + INTERVAL '24 hours') THEN 'DONE' ELSE a.status::text END AS doctor_lifecycle, p.full_name AS patient_display_name FROM appointments a JOIN doctor_schedule_slots s ON s.id=a.slot_id JOIN patient_profiles p ON p.id=a.patient_id LEFT JOIN consultation_sessions cs ON cs.appointment_id=a.id WHERE a.doctor_id=$1 AND ($4::text IS NULL OR a.status::text=$4) ORDER BY CASE WHEN a.status::text IN ('CONFIRMED','WAITING','IN_CONSULTATION') THEN 0 ELSE 1 END ASC, CASE WHEN a.status::text IN ('CONFIRMED','WAITING','IN_CONSULTATION') THEN s.start_time END ASC, CASE WHEN a.status::text NOT IN ('CONFIRMED','WAITING','IN_CONSULTATION') THEN s.start_time END DESC, a.id ASC LIMIT $2 OFFSET $3")
        .bind(id).bind(limit).bind(offset).bind(q.status).bind(ctx.user_id).fetch_all(&mut *tx).await?;
    tx.commit().await?;
    let data: Vec<_> = rows.into_iter().map(|r| json!({"id": r.get::<Uuid,_>("id"), "appointment_number": r.get::<String,_>("appointment_number"), "patient_id": r.get::<Uuid,_>("patient_id"), "patient_display_name": r.get::<String,_>("patient_display_name"), "modality": r.get::<String,_>("modality"), "status": r.get::<String,_>("status"), "doctor_lifecycle": r.get::<String,_>("doctor_lifecycle"), "history_access_from": r.get::<DateTime<Utc>,_>("history_access_from"), "history_access_until": r.get::<Option<DateTime<Utc>>,_>("history_access_until"), "history_access_active": r.get::<bool,_>("history_access_active"), "consultation_fee": r.get::<bigdecimal::BigDecimal,_>("consultation_fee"), "clinical_outcome": r.get::<Option<String>,_>("clinical_outcome"), "completed_at": r.get::<Option<DateTime<Utc>>,_>("completed_at"), "start_time": r.get::<DateTime<Utc>,_>("start_time"), "end_time": r.get::<DateTime<Utc>,_>("end_time")})).collect();
    Ok(Json(ApiResponse {
        success: true,
        data,
    }))
}

pub async fn list_own_chats(
    State(state): State<AppState>,
    ctx: AuthContext,
    Query(q): Query<PageQuery>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::DoctorRead)?;
    let id = ctx.verified_doctor_profile_id(&state).await?;
    let (limit, offset) = page(&q);
    let mut tx = state.begin_authed(&ctx).await?;
    let rows = sqlx::query("SELECT c.id, c.appointment_id, c.patient_id, c.doctor_id, c.expires_at, c.is_locked, c.created_at, p.full_name AS patient_display_name, MAX(m.sent_at) AS last_message_at FROM chat_conversations c JOIN patient_profiles p ON p.id=c.patient_id LEFT JOIN chat_messages m ON m.conversation_id=c.id WHERE c.doctor_id=$1 GROUP BY c.id, p.full_name ORDER BY COALESCE(MAX(m.sent_at), c.created_at) DESC LIMIT $2 OFFSET $3")
        .bind(id).bind(limit).bind(offset).fetch_all(&mut *tx).await?;
    tx.commit().await?;
    let data: Vec<_> = rows.into_iter().map(|r| json!({"id": r.get::<Uuid,_>("id"), "appointment_id": r.get::<Uuid,_>("appointment_id"), "patient_id": r.get::<Uuid,_>("patient_id"), "patient_display_name": r.get::<String,_>("patient_display_name"), "expires_at": r.get::<DateTime<Utc>,_>("expires_at"), "is_locked": r.get::<bool,_>("is_locked"), "created_at": r.get::<DateTime<Utc>,_>("created_at"), "last_message_at": r.get::<Option<DateTime<Utc>>,_>("last_message_at")})).collect();
    Ok(Json(ApiResponse {
        success: true,
        data,
    }))
}

pub async fn list_patient_history(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(current_appointment_id): Path<Uuid>,
    Query(q): Query<PageQuery>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::DoctorRead)?;
    let (limit, offset) = page(&q);
    let mut tx = state.begin_authed(&ctx).await?;
    let allowed = sqlx::query_scalar::<_, bool>("SELECT doctor_history_context_allowed($1,$2)")
        .bind(current_appointment_id)
        .bind(ctx.user_id)
        .fetch_one(&mut *tx)
        .await?;
    if !allowed {
        return Err(AppError::NotFound(
            "Authorized current appointment not found.".into(),
        ));
    }
    let rows = sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT * FROM doctor_patient_history($1,$2,$3,$4)",
    )
    .bind(current_appointment_id)
    .bind(ctx.user_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    AuditService::log(
        &state,
        &ctx,
        AuditLogEntry::new(
            Some(ctx.user_id),
            ctx.role,
            "PATIENT_HISTORY_VIEWED",
            "PATIENT_HISTORY",
            Some(current_appointment_id),
            "SUCCESS",
        ),
    )
    .await?;
    Ok(Json(ApiResponse {
        success: true,
        data: rows,
    }))
}

pub async fn get_patient_history_detail(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path((current_appointment_id, historical_consultation_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::DoctorRead)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let row = sqlx::query_scalar::<_, Option<serde_json::Value>>(
        "SELECT doctor_patient_history_detail($1,$2,$3)",
    )
    .bind(current_appointment_id)
    .bind(ctx.user_id)
    .bind(historical_consultation_id)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    let data = row.ok_or_else(|| {
        AppError::NotFound("Historical consultation not found or not authorized.".into())
    })?;
    AuditService::log(
        &state,
        &ctx,
        AuditLogEntry::new(
            Some(ctx.user_id),
            ctx.role,
            "PATIENT_HISTORY_EPISODE_VIEWED",
            "PATIENT_HISTORY",
            Some(historical_consultation_id),
            "SUCCESS",
        ),
    )
    .await?;
    Ok(Json(ApiResponse {
        success: true,
        data,
    }))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnswerRequest {
    pub answer_text: String,
}

pub async fn list_open_questions(
    State(state): State<AppState>,
    ctx: AuthContext,
    Query(q): Query<PageQuery>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::DoctorQna)?;
    let (limit, offset) = page(&q);
    let status = q.status.unwrap_or_else(|| "OPEN".into()).to_uppercase();
    if !matches!(status.as_str(), "ALL" | "OPEN" | "ANSWERED") {
        return Err(AppError::Validation(
            "Question status must be ALL, OPEN or ANSWERED.".into(),
        ));
    }
    let mut tx = state.begin_authed(&ctx).await?;
    let rows = sqlx::query("SELECT q.id, q.question_text, q.category, q.status, q.created_at, q.updated_at, a.answer_text, a.created_at AS answered_at, d.full_name AS answering_doctor FROM health_questions q LEFT JOIN health_answers a ON a.question_id=q.id LEFT JOIN doctor_public_directory d ON d.id=a.doctor_id WHERE ($3='ALL' AND q.status IN ('OPEN','ANSWERED')) OR q.status=$3 ORDER BY q.created_at DESC LIMIT $1 OFFSET $2")
        .bind(limit).bind(offset).bind(status).fetch_all(&mut *tx).await?;
    tx.commit().await?;
    let data: Vec<_> = rows.into_iter().map(|r| json!({"id": r.get::<Uuid,_>("id"), "question_text": r.get::<String,_>("question_text"), "category": r.get::<Option<String>,_>("category"), "status": r.get::<String,_>("status"), "created_at": r.get::<DateTime<Utc>,_>("created_at"), "updated_at": r.get::<DateTime<Utc>,_>("updated_at"), "answer_text":r.get::<Option<String>,_>("answer_text"), "answered_at":r.get::<Option<DateTime<Utc>>,_>("answered_at"), "answering_doctor":r.get::<Option<String>,_>("answering_doctor")})).collect();
    Ok(Json(ApiResponse {
        success: true,
        data,
    }))
}

pub async fn answer_question(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(question_id): Path<Uuid>,
    Json(body): Json<AnswerRequest>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::DoctorQna)?;
    if body.answer_text.trim().is_empty() || body.answer_text.len() > 4000 {
        return Err(AppError::Validation(
            "answer_text must contain 1-4000 characters.".into(),
        ));
    }
    let doctor_id = ctx.verified_doctor_profile_id(&state).await?;
    let mut tx = state.begin_authed(&ctx).await?;
    let inserted = sqlx::query("INSERT INTO health_answers (id, question_id, doctor_id, answer_text, created_at, updated_at) SELECT $1, id, $2, $3, $4, $4 FROM health_questions WHERE id=$5 AND status='OPEN' ON CONFLICT (question_id) DO NOTHING")
        .bind(Uuid::new_v4()).bind(doctor_id).bind(body.answer_text.trim()).bind(Utc::now()).bind(question_id).execute(&mut *tx).await?;
    if inserted.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "Question is unavailable or already answered.".into(),
        ));
    }
    sqlx::query("UPDATE health_questions SET status='ANSWERED', updated_at=$2 WHERE id=$1 AND status='OPEN'")
        .bind(question_id).bind(Utc::now()).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(ApiResponse {
            success: true,
            data: json!({"question_id": question_id, "status": "ANSWERED"}),
        }),
    ))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DoctorProfileUpdate {
    pub full_name: Option<String>,
}

pub async fn update_own_profile(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(body): Json<DoctorProfileUpdate>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::DoctorManageProfile)?;
    if let Some(ref name) = body.full_name {
        if name.trim().is_empty() || name.chars().count() > 150 {
            return Err(AppError::Validation(
                "full_name must contain 1-150 characters.".into(),
            ));
        }
    }
    let id = ctx.doctor_profile_id(&state).await?;
    let mut tx = state.begin_authed(&ctx).await?;
    let result = sqlx::query("UPDATE doctor_profiles SET full_name=COALESCE($2,full_name), updated_at=$3 WHERE id=$1 AND deleted_at IS NULL")
        .bind(id).bind(body.full_name).bind(Utc::now()).execute(&mut *tx).await?;
    tx.commit().await?;
    if result.rows_affected() != 1 {
        return Err(AppError::NotFound("Doctor profile not found.".into()));
    }
    Ok(Json(ApiResponse {
        success: true,
        data: json!({"updated": true}),
    }))
}

pub async fn consultation_summary(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(appointment_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::DoctorRead)?;
    let doctor_id = ctx.verified_doctor_profile_id(&state).await?;
    let mut tx = state.begin_authed(&ctx).await?;
    let allowed: bool = sqlx::query_scalar("SELECT doctor_history_context_allowed($1,$2)")
        .bind(appointment_id)
        .bind(ctx.user_id)
        .fetch_one(&mut *tx)
        .await?;
    if !allowed {
        return Err(AppError::Forbidden(
            "Patient history access is unavailable outside the consultation access window.".into(),
        ));
    }
    let row = sqlx::query("SELECT a.id, a.patient_id, a.status::text AS status, a.clinical_outcome, a.completed_at, i.chief_complaint, i.clinical_notes, cs.id AS consultation_id, cs.started_at, cs.ended_at FROM appointments a LEFT JOIN appointment_clinical_intake i ON i.appointment_id=a.id LEFT JOIN consultation_sessions cs ON cs.appointment_id=a.id WHERE a.id=$1 AND a.doctor_id=$2")
        .bind(appointment_id).bind(doctor_id).fetch_optional(&mut *tx).await?;
    tx.commit().await?;
    let r = row.ok_or_else(|| AppError::NotFound("Consultation not found.".into()))?;
    let data = json!({
        "appointment_id": r.get::<Uuid, _>("id"),
        "patient_id": r.get::<Uuid, _>("patient_id"),
        "status": r.get::<String, _>("status"),
        "clinical_outcome": r.get::<Option<String>, _>("clinical_outcome"),
        "completed_at": r.get::<Option<DateTime<Utc>>, _>("completed_at"),
        "chief_complaint": r.get::<Option<String>, _>("chief_complaint"),
        "clinical_notes": r.get::<Option<String>, _>("clinical_notes"),
        "consultation_id": r.get::<Option<Uuid>, _>("consultation_id"),
        "started_at": r.get::<Option<DateTime<Utc>>, _>("started_at"),
        "ended_at": r.get::<Option<DateTime<Utc>>, _>("ended_at")
    });
    Ok(Json(ApiResponse {
        success: true,
        data,
    }))
}

pub async fn wallet_ledger(
    State(state): State<AppState>,
    ctx: AuthContext,
    Query(q): Query<PageQuery>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ViewOwnWallet)?;
    let doctor_id = ctx.doctor_profile_id(&state).await?;
    let (limit, offset) = page(&q);
    let mut tx = state.begin_authed(&ctx).await?;
    let rows = sqlx::query("SELECT t.id, t.transaction_number, t.appointment_id, t.gateway::text AS gateway, t.gross_amount, t.platform_fee_amount, t.net_amount, t.payment_status::text AS payment_status, t.created_at, t.settled_at, p.full_name AS patient_display_name FROM transactions t JOIN appointments a ON a.id=t.appointment_id JOIN patient_profiles p ON p.id=a.patient_id WHERE a.doctor_id=$1 ORDER BY t.created_at DESC LIMIT $2 OFFSET $3")
        .bind(doctor_id).bind(limit).bind(offset).fetch_all(&mut *tx).await?;
    tx.commit().await?;
    let data: Vec<_> = rows.into_iter().map(|r| json!({"id": r.get::<Uuid,_>("id"), "transaction_number": r.get::<String,_>("transaction_number"), "appointment_id": r.get::<Uuid,_>("appointment_id"), "patient_display_name": r.get::<String,_>("patient_display_name"), "gateway": r.get::<String,_>("gateway"), "gross_amount": r.get::<bigdecimal::BigDecimal,_>("gross_amount"), "platform_fee_amount": r.get::<bigdecimal::BigDecimal,_>("platform_fee_amount"), "net_amount": r.get::<bigdecimal::BigDecimal,_>("net_amount"), "payment_status": r.get::<String,_>("payment_status"), "created_at": r.get::<DateTime<Utc>,_>("created_at"), "settled_at": r.get::<Option<DateTime<Utc>>,_>("settled_at")})).collect();
    Ok(Json(ApiResponse {
        success: true,
        data,
    }))
}
