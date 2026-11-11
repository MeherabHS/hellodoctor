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
    // FINANCIAL_MODULE_MODE=deferred: no fee is ever charged, so an unpriced
    // (fee = 0) doctor must still be discoverable and bookable for VIDEO —
    // the backend's deferred booking path (`book_appointment_deferred`)
    // already accepts fee = 0. CHAT keeps its fee-based offering signal
    // because the chat booking flow is payment-specific.
    let fee_not_required = state.config.financial_module_deferred();
    let list = sqlx::query_as::<_, doctor_repo::PublicDoctorProfile>("SELECT * FROM doctor_public_directory WHERE professional_verification_status='VERIFIED' AND ($1::text IS NULL OR ($1='VIDEO' AND ($4::bool OR consultation_fee_video > 0)) OR ($1='CHAT' AND consultation_fee_chat > 0)) AND ($2::text IS NULL OR lower(primary_specialty)=lower($2)) AND ($3::text IS NULL OR full_name ILIKE '%' || $3 || '%' OR current_hospital ILIKE '%' || $3 || '%') ORDER BY full_name")
        .bind(query.modality).bind(query.specialty).bind(query.search).bind(fee_not_required).fetch_all(&state.db).await?;

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
    pub day: Option<chrono::NaiveDate>,
    pub view: Option<String>,
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
    let rows = sqlx::query("SELECT (s.status='AVAILABLE' AND s.start_time > clock_timestamp()) AS bookable, s.id, s.doctor_id, s.start_time, s.end_time, s.status::text AS status, s.created_at, a.id AS appointment_id, a.patient_id, COALESCE(p.full_name,doctor_encounter_patient_name(a.id)) AS patient_display_name FROM doctor_schedule_slots s LEFT JOIN appointments a ON a.slot_id = s.id AND a.status <> 'CANCELLED' LEFT JOIN patient_profiles p ON p.id = a.patient_id WHERE s.doctor_id = $1 AND ($4::DATE IS NULL OR (s.start_time >= ($4::DATE::TIMESTAMP AT TIME ZONE 'Africa/Nairobi') AND s.start_time < (($4::DATE+1)::TIMESTAMP AT TIME ZONE 'Africa/Nairobi'))) ORDER BY s.start_time,s.id LIMIT $2 OFFSET $3")
        .bind(id).bind(limit).bind(offset).bind(q.day).fetch_all(&mut *tx).await?;
    tx.commit().await?;
    let data: Vec<_> = rows.into_iter().map(|r| json!({"bookable":r.get::<bool,_>("bookable"), "id": r.get::<Uuid,_>("id"), "doctor_id": r.get::<Uuid,_>("doctor_id"), "start_time": r.get::<DateTime<Utc>,_>("start_time"), "end_time": r.get::<DateTime<Utc>,_>("end_time"), "status": r.get::<String,_>("status"), "created_at": r.get::<DateTime<Utc>,_>("created_at"), "appointment_id": r.get::<Option<Uuid>,_>("appointment_id"), "patient_id": r.get::<Option<Uuid>,_>("patient_id"), "patient_display_name": r.get::<Option<String>,_>("patient_display_name")})).collect();
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
    // PostgreSQL evaluates current work independently of device clocks. The
    // complete list remains available for history; view=upcoming selects only
    // actionable work. Active clinical chats retain their real server window.
    if q.view
        .as_deref()
        .is_some_and(|v| !matches!(v, "all" | "upcoming" | "history" | "schedule"))
    {
        return Err(AppError::Validation("Unknown consultation view.".into()));
    }
    // Milestone 8 Phase 3 performance remediation (2026-11-02): this used to
    // compute doctor_history_context_allowed(a.id,...) -- a non-trivial
    // per-row function call (its own joins across appointments,
    // doctor_profiles, users, doctor_schedule_slots, consultation_sessions)
    // -- and LEFT JOIN patient_profiles (itself RLS-protected via
    // fn_doctor_treats_patient, a richer verification/active-account/
    // consultation-state check that is NOT simplified here) for every row
    // matching a.doctor_id, BEFORE the final WHERE/ORDER/LIMIT narrowed that
    // down to at most `limit` rows. For a doctor with ~1,600 appointments
    // (realistic after months of real use; exactly what the seeded
    // six-service benchmark dataset represents), that meant ~1,600 calls to
    // both of those expensive checks to ultimately return 20 rows. Measured
    // live via EXPLAIN (ANALYZE, BUFFERS) against the real hellodoctor_api
    // role and real RLS context: 3,196ms before this change. Restructured so
    // `encounters` computes only what's needed to FILTER/SORT/PAGINATE
    // (never patient_display_name or history_access_active, neither of which
    // affects which rows are selected or their order), then the outer query
    // joins patient_profiles and calls doctor_history_context_allowed only
    // for the at-most-`limit` rows that survive pagination -- same two
    // checks, same semantics, just evaluated for ~20 rows instead of ~1,600.
    // Combined with this migration's companion RLS policy performance fix
    // (20261204000001_doctor_rls_identity_lookup_performance, which
    // addresses a separate, unrelated per-row cost in the
    // appointments/chat_conversations/consultation_sessions RLS policies
    // themselves), measured at 838ms in an isolated clone database -- a 3.8x
    // reduction -- with row-set equality confirmed identical to the original
    // query for every tested doctor, patient, ADMIN, and unauthenticated
    // scenario. See docs/MILESTONE_8_SIX_SERVICE_PERFORMANCE_REMEDIATION.md
    // Phase 3 for the full evidence; neither fn_doctor_treats_patient nor
    // doctor_history_context_allowed's own internal logic was touched.
    let rows = sqlx::query(r#"
      WITH encounters AS (
        SELECT a.id,a.appointment_number,a.patient_id,a.doctor_id,a.slot_id,
          a.modality::text AS modality,a.status::text AS status,a.consultation_fee,
          a.payment_policy::text AS payment_policy,
          a.clinical_outcome,a.completed_at,a.created_at,a.updated_at,
          s.start_time,s.end_time,c.id AS conversation_id,c.is_locked AS chat_locked,c.expires_at AS chat_expires_at,
          cs.status::text AS consultation_session_status,cs.recovery_state AS consultation_recovery_state,
          cs.empty_room_deadline,cs.patient_present,cs.doctor_present,
          s.start_time-INTERVAL '15 minutes' AS history_access_from,
          CASE WHEN a.status='COMPLETED' THEN COALESCE(cs.ended_at,a.completed_at)+INTERVAL '24 hours' END AS history_access_until,
          CASE WHEN a.status='COMPLETED' AND (COALESCE(cs.ended_at,a.completed_at) IS NULL OR clock_timestamp()>=COALESCE(cs.ended_at,a.completed_at)+INTERVAL '24 hours') THEN 'DONE' ELSE a.status::text END AS doctor_lifecycle,
          (a.status='IN_CONSULTATION' OR (
            a.status IN ('CONFIRMED','WAITING')
            AND (s.start_time AT TIME ZONE 'Africa/Nairobi')::date=(clock_timestamp() AT TIME ZONE 'Africa/Nairobi')::date
            AND ((a.modality='VIDEO' AND s.end_time>clock_timestamp()) OR
              (a.modality='CHAT' AND COALESCE(NOT c.is_locked AND c.expires_at>clock_timestamp(),false)))
          )) AS queue_actionable
        FROM appointments a JOIN doctor_schedule_slots s ON s.id=a.slot_id
        LEFT JOIN consultation_sessions cs ON cs.appointment_id=a.id
        LEFT JOIN chat_conversations c ON c.appointment_id=a.id
        WHERE a.doctor_id=$1 AND ($4::text IS NULL OR a.status::text=$4)
      ),
      paginated AS (
        SELECT * FROM encounters WHERE
          ($6::text<>'upcoming' OR queue_actionable)
          AND ($6::text<>'schedule' OR (status IN ('CONFIRMED','WAITING') AND
            (start_time AT TIME ZONE 'Africa/Nairobi')::date>(clock_timestamp() AT TIME ZONE 'Africa/Nairobi')::date))
          AND ($6::text<>'history' OR status IN ('COMPLETED','CANCELLED','REFUNDED','EXPIRED','PREMATURE_TERMINATION','DISPUTED') OR
            (NOT queue_actionable AND (end_time<=clock_timestamp() OR (modality='CHAT' AND (chat_locked OR chat_expires_at<=clock_timestamp())))))
        ORDER BY CASE WHEN status='IN_CONSULTATION' THEN 0 WHEN queue_actionable THEN 1
            WHEN status IN ('CONFIRMED','WAITING') AND start_time>clock_timestamp() THEN 2 ELSE 3 END,
          CASE WHEN queue_actionable OR (status IN ('CONFIRMED','WAITING') AND start_time>clock_timestamp()) THEN start_time END ASC,
          CASE WHEN NOT queue_actionable AND NOT(status IN ('CONFIRMED','WAITING') AND start_time>clock_timestamp()) THEN start_time END DESC,id ASC
        LIMIT $2 OFFSET $3
      )
      SELECT paginated.*,
        doctor_history_context_allowed(paginated.id,$5) AS history_access_active,
        COALESCE(p.full_name,doctor_encounter_patient_name(paginated.id),'Patient') AS patient_display_name
      FROM paginated LEFT JOIN patient_profiles p ON p.id=paginated.patient_id
    "#)
        .bind(id).bind(limit).bind(offset).bind(q.status).bind(ctx.user_id).bind(q.view.unwrap_or_else(||"upcoming".into())).fetch_all(&mut *tx).await?;
    tx.commit().await?;
    let data: Vec<_> = rows.into_iter().map(|r| json!({"queue_actionable":r.get::<bool,_>("queue_actionable"), "id": r.get::<Uuid,_>("id"), "appointment_number": r.get::<String,_>("appointment_number"), "patient_id": r.get::<Uuid,_>("patient_id"), "patient_display_name": r.get::<String,_>("patient_display_name"), "conversation_id":r.get::<Option<Uuid>,_>("conversation_id"), "chat_locked":r.get::<Option<bool>,_>("chat_locked"), "chat_expires_at":r.get::<Option<DateTime<Utc>>,_>("chat_expires_at"), "modality": r.get::<String,_>("modality"), "status": r.get::<String,_>("status"), "doctor_lifecycle": r.get::<String,_>("doctor_lifecycle"), "history_access_from": r.get::<DateTime<Utc>,_>("history_access_from"), "history_access_until": r.get::<Option<DateTime<Utc>>,_>("history_access_until"), "history_access_active": r.get::<bool,_>("history_access_active"), "consultation_fee": r.get::<bigdecimal::BigDecimal,_>("consultation_fee"), "payment_policy": r.get::<String,_>("payment_policy"), "clinical_outcome": r.get::<Option<String>,_>("clinical_outcome"), "completed_at": r.get::<Option<DateTime<Utc>>,_>("completed_at"), "start_time": r.get::<DateTime<Utc>,_>("start_time"), "end_time": r.get::<DateTime<Utc>,_>("end_time"), "consultation_session_status": r.get::<Option<String>,_>("consultation_session_status"), "consultation_recovery_state": r.get::<Option<String>,_>("consultation_recovery_state"), "empty_room_deadline": r.get::<Option<DateTime<Utc>>,_>("empty_room_deadline"), "patient_present": r.get::<Option<bool>,_>("patient_present"), "doctor_present": r.get::<Option<bool>,_>("doctor_present")})).collect();
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
    // Milestone 8 Phase 3 performance remediation (2026-11-02): patient_display_name
    // (via patient_profiles, itself RLS-protected by fn_doctor_treats_patient
    // -- a richer verification/active-account/consultation-state check, NOT
    // simplified here) used to be joined and grouped BEFORE the LIMIT, so it
    // was evaluated once per conversation (up to ~1,300 for a doctor with a
    // realistic history) instead of once per returned row. It plays no part
    // in which conversations are selected or their order (sorted purely by
    // last-message/created-at), so it's deferred to an outer join over only
    // the already-paginated rows.
    //
    // Milestone 8 Phase 5 (2026-11-02): the MAX(m.sent_at) aggregate over
    // chat_messages flagged above as "cannot be deferred past LIMIT" turned
    // out to be the dominant remaining cost -- chat_messages has FORCE RLS,
    // so scanning it for the aggregate re-evaluated chat_messages_access's
    // correlated EXISTS back into chat_conversations once per raw message row
    // (3,591 executions for one doctor's 2,026 conversations in the seeded
    // benchmark), measured at ~930ms of the endpoint's ~1,672ms
    // (post-index-fix) execution time -- not fn_doctor_treats_patient or
    // doctor_history_context_allowed, which this same investigation confirmed
    // are NOT the bottleneck here. Fixed by denormalizing the latest message
    // timestamp onto chat_conversations.last_message_at (maintained by a
    // trigger on chat_messages INSERT -- see migration
    // 20261204000003_chat_conversation_last_message_denorm), so this query
    // never touches chat_messages at all. The appointments join (needed only
    // for display fields, not for sorting or pagination) is deferred to the
    // same already-paginated rows for the same reason patient_profiles
    // already was. Combined with 20261204000002's missing
    // chat_conversations(doctor_id) index: measured 1,849ms -> 369ms (5.0x)
    // in an isolated clone database; row-set equality and cross-doctor
    // isolation confirmed identical to the original query. See
    // docs/MILESTONE_8_PHASE5_EFFICIENCY_INVESTIGATION.md for full evidence.
    let rows = sqlx::query("
      WITH base AS (
        SELECT c.id, c.appointment_id, c.patient_id, c.doctor_id, c.expires_at, c.is_locked, c.created_at,
               c.last_message_at
        FROM chat_conversations c
        WHERE c.doctor_id=$1
        ORDER BY COALESCE(c.last_message_at, c.created_at) DESC
        LIMIT $2 OFFSET $3
      )
      SELECT base.*, a.modality::text AS modality, a.status::text AS appointment_status,
             COALESCE(p.full_name, doctor_encounter_patient_name(base.appointment_id), 'Patient') AS patient_display_name
      FROM base JOIN appointments a ON a.id = base.appointment_id
      LEFT JOIN patient_profiles p ON p.id = base.patient_id
    ")
        .bind(id).bind(limit).bind(offset).fetch_all(&mut *tx).await?;
    tx.commit().await?;
    let data: Vec<_> = rows.into_iter().map(|r| json!({"id": r.get::<Uuid,_>("id"), "appointment_id": r.get::<Uuid,_>("appointment_id"), "patient_id": r.get::<Uuid,_>("patient_id"), "patient_display_name": r.get::<String,_>("patient_display_name"), "expires_at": r.get::<DateTime<Utc>,_>("expires_at"), "is_locked": r.get::<bool,_>("is_locked"), "created_at": r.get::<DateTime<Utc>,_>("created_at"), "last_message_at": r.get::<Option<DateTime<Utc>>,_>("last_message_at"), "modality":r.get::<String,_>("modality"), "appointment_status":r.get::<String,_>("appointment_status")})).collect();
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
    let allowed = sqlx::query_scalar::<_, bool>("SELECT doctor_own_encounter_allowed($1,$2)")
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
    let allowed: bool = sqlx::query_scalar("SELECT doctor_own_encounter_allowed($1,$2)")
        .bind(appointment_id)
        .bind(ctx.user_id)
        .fetch_one(&mut *tx)
        .await?;
    if !allowed {
        return Err(AppError::Forbidden(
            "This encounter is not available to this account.".into(),
        ));
    }
    crate::api::middleware::request_context::bind_consultation(appointment_id, None);
    let row = sqlx::query("SELECT a.id, a.patient_id, a.status::text AS status, a.clinical_outcome, a.completed_at, i.chief_complaint, i.clinical_notes, cs.id AS consultation_id, cs.started_at, cs.ended_at FROM appointments a LEFT JOIN appointment_clinical_intake i ON i.appointment_id=a.id LEFT JOIN consultation_sessions cs ON cs.appointment_id=a.id WHERE a.id=$1 AND a.doctor_id=$2")
        .bind(appointment_id).bind(doctor_id).fetch_optional(&mut *tx).await?;
    let documents:Vec<serde_json::Value>=sqlx::query_scalar("SELECT jsonb_build_object('id',id,'mime_type',mime_type,'security_status',security_status,'created_at',created_at) FROM prescription_intake_documents WHERE appointment_id=$1 ORDER BY created_at,id").bind(appointment_id).fetch_all(&mut *tx).await?;
    let prescriptions:Vec<serde_json::Value>=sqlx::query_scalar("SELECT jsonb_build_object('id',id,'mime_type',mime_type,'security_status',security_status,'is_no_rx_required',is_no_rx_required,'page_number',page_number,'created_at',created_at) FROM prescriptions WHERE appointment_id=$1 ORDER BY page_number").bind(appointment_id).fetch_all(&mut *tx).await?;
    tx.commit().await?;
    let r = row.ok_or_else(|| AppError::NotFound("Consultation not found.".into()))?;
    let data = json!({
        "appointment_id": r.get::<Uuid, _>("id"),
        "documents":documents,"prescriptions":prescriptions,
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
    let rows = sqlx::query("SELECT t.id, t.transaction_number, t.appointment_id, t.gateway::text AS gateway, t.gross_amount, t.platform_fee_amount, t.net_amount, t.payment_status::text AS payment_status, t.created_at, t.settled_at, COALESCE(p.full_name,doctor_encounter_patient_name(a.id),'Patient') AS patient_display_name FROM transactions t JOIN appointments a ON a.id=t.appointment_id LEFT JOIN patient_profiles p ON p.id=a.patient_id WHERE a.doctor_id=$1 ORDER BY t.created_at DESC LIMIT $2 OFFSET $3")
        .bind(doctor_id).bind(limit).bind(offset).fetch_all(&mut *tx).await?;
    tx.commit().await?;
    let data: Vec<_> = rows.into_iter().map(|r| json!({"id": r.get::<Uuid,_>("id"), "transaction_number": r.get::<String,_>("transaction_number"), "appointment_id": r.get::<Uuid,_>("appointment_id"), "patient_display_name": r.get::<String,_>("patient_display_name"), "gateway": r.get::<String,_>("gateway"), "gross_amount": r.get::<bigdecimal::BigDecimal,_>("gross_amount"), "platform_fee_amount": r.get::<bigdecimal::BigDecimal,_>("platform_fee_amount"), "net_amount": r.get::<bigdecimal::BigDecimal,_>("net_amount"), "payment_status": r.get::<String,_>("payment_status"), "created_at": r.get::<DateTime<Utc>,_>("created_at"), "settled_at": r.get::<Option<DateTime<Utc>>,_>("settled_at")})).collect();
    Ok(Json(ApiResponse {
        success: true,
        data,
    }))
}
