//! Patient-facing profile and appointment-list endpoints.
//!
//! Added for the Patient Flutter client build: confirmed by direct router
//! inspection that no `/patients/*` route existed before this file — OTP
//! login (`auth_service.rs::verify_otp_and_login`) auto-creates a
//! placeholder `patient_profiles` row (`full_name: "Patient User"`,
//! `date_of_birth: 1995-01-01`, `gender: Other`) with no prior way for a
//! patient to read or correct it, and no prior way for a patient to list
//! their own appointments (`/doctor/appointments` is doctor-scoped only).

use crate::abac::{self, Action};
use crate::auth_context::AuthContext;
use crate::crypto;
use crate::domain::ids::PatientProfileId;
use crate::domain::models::{Gateway, Gender, PatientProfile};
use crate::domain::validation::normalize_kenya_phone_number;
use crate::error::AppError;
use crate::rate_limit;
use crate::repository::{patient_repo, payment_methods_repo, AppState};
use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use base64::Engine as _;
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::Row;

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: T,
}

pub async fn get_own_profile(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::PatientRead)?;
    let id = ctx.patient_profile_id(&state).await?;
    let mut tx = state.begin_authed(&ctx).await?;
    let profile = patient_repo::find_by_id(&mut *tx, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Patient profile not found.".into()))?;
    tx.commit().await?;
    Ok(Json(ApiResponse {
        success: true,
        data: profile,
    }))
}

#[derive(Debug, Deserialize)]
pub struct PatientProfileUpdate {
    pub full_name: Option<String>,
    pub date_of_birth: Option<NaiveDate>,
    pub gender: Option<Gender>,
    pub blood_group: Option<String>,
    pub weight_kg: Option<BigDecimal>,
    pub emergency_contact_phone: Option<String>,
    pub emergency_contact_relation: Option<String>,
    /// Only meaningful for a dependent profile — ignored (harmlessly
    /// written, but never read by anything) if sent against the caller's
    /// own self profile via `PATCH /patients/me`.
    pub relationship: Option<String>,
}

/// Shared by `update_own_profile`, `create_dependent_profile`, and
/// `update_dependent_profile` — the same field-shape validation applies to
/// a self profile and a dependent profile alike. Returns the
/// Kenya-phone-normalized `emergency_contact_phone`, since that one field's
/// validation also transforms the value, not just checks it.
fn validate_profile_fields(
    full_name: Option<&str>,
    blood_group: Option<&str>,
    emergency_contact_relation: Option<&str>,
    emergency_contact_phone: Option<&str>,
    date_of_birth: Option<NaiveDate>,
    weight_kg: Option<&BigDecimal>,
    relationship: Option<&str>,
) -> Result<Option<String>, AppError> {
    if let Some(name) = full_name {
        if name.trim().is_empty() || name.chars().count() > 150 {
            return Err(AppError::Validation(
                "full_name must contain 1-150 characters.".into(),
            ));
        }
    }
    if let Some(bg) = blood_group {
        if bg.chars().count() > 10 {
            return Err(AppError::Validation(
                "blood_group must be at most 10 characters.".into(),
            ));
        }
    }
    if let Some(rel) = emergency_contact_relation {
        if rel.chars().count() > 50 {
            return Err(AppError::Validation(
                "emergency_contact_relation must be at most 50 characters.".into(),
            ));
        }
    }
    if let Some(rel) = relationship {
        if rel.trim().is_empty() || rel.chars().count() > 30 {
            return Err(AppError::Validation(
                "relationship must contain 1-30 characters.".into(),
            ));
        }
    }
    if let Some(dob) = date_of_birth {
        if dob > Utc::now().date_naive() {
            return Err(AppError::Validation(
                "date_of_birth cannot be in the future.".into(),
            ));
        }
    }
    if let Some(weight) = weight_kg {
        // The two bounds are only ever constructed once each, here — not a
        // hot path — so comparing against freshly-built `BigDecimal`s is
        // clearer than a `Signed`/`Zero`-trait workaround.
        #[allow(clippy::cmp_owned)]
        let out_of_range = *weight <= BigDecimal::from(0) || *weight > BigDecimal::from(500);
        if out_of_range {
            return Err(AppError::Validation(
                "weight_kg must be between 0 and 500.".into(),
            ));
        }
    }
    // Re-normalized (not just validated) so the stored value always matches
    // the canonical `+254XXXXXXXXX` shape every other phone column in this
    // schema uses, regardless of which tolerated input form the client sent.
    emergency_contact_phone
        .map(normalize_kenya_phone_number)
        .transpose()
}

pub async fn update_own_profile(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(body): Json<PatientProfileUpdate>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::PatientManageProfile)?;
    let normalized_emergency_phone = validate_profile_fields(
        body.full_name.as_deref(),
        body.blood_group.as_deref(),
        body.emergency_contact_relation.as_deref(),
        body.emergency_contact_phone.as_deref(),
        body.date_of_birth,
        body.weight_kg.as_ref(),
        body.relationship.as_deref(),
    )?;

    let id = ctx.patient_profile_id(&state).await?;
    let mut tx = state.begin_authed(&ctx).await?;
    let rows = patient_repo::update(
        &mut *tx,
        id,
        body.full_name,
        body.date_of_birth,
        body.gender,
        body.blood_group,
        body.weight_kg,
        normalized_emergency_phone,
        body.emergency_contact_relation,
        body.relationship,
        Utc::now(),
    )
    .await?;
    tx.commit().await?;
    if rows != 1 {
        return Err(AppError::NotFound("Patient profile not found.".into()));
    }
    Ok(Json(ApiResponse {
        success: true,
        data: json!({"updated": true}),
    }))
}

/// `GET /patients/me/profiles` — the caller's own self profile plus every
/// dependent they guardian, backing the Flutter profile-switcher list.
pub async fn list_profiles(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::PatientRead)?;
    let self_id = ctx.patient_profile_id(&state).await?;
    let mut tx = state.begin_authed(&ctx).await?;
    let profiles = patient_repo::list_for_guardian(&mut *tx, self_id).await?;
    tx.commit().await?;
    Ok(Json(ApiResponse {
        success: true,
        data: profiles,
    }))
}

#[derive(Debug, Deserialize)]
pub struct DependentProfileCreate {
    pub full_name: String,
    pub date_of_birth: NaiveDate,
    pub gender: Gender,
    pub relationship: String,
    pub blood_group: Option<String>,
    pub weight_kg: Option<BigDecimal>,
    pub emergency_contact_phone: Option<String>,
    pub emergency_contact_relation: Option<String>,
}

/// `POST /patients/me/profiles` — create a dependent under the caller's own
/// account. No separate "is the caller themself a dependent" application
/// check is needed here: `ctx.patient_profile_id` resolves strictly via
/// `find_by_user_id`, and a dependent row always has `user_id = NULL` (see
/// `patient_profiles_self_xor_dependent`), so `self_id` below can
/// structurally never itself be a dependent — nested dependents are
/// impossible to reach through any authenticated request, not just guarded
/// against. The DB trigger `patient_profiles_single_level_dependents` and
/// RLS policy `dependent_profile_author_guard` remain as the real backstops
/// against any non-API write path.
pub async fn create_dependent_profile(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(body): Json<DependentProfileCreate>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::PatientManageProfile)?;
    rate_limit::check_dependent_creation_limit(&state.redis, &ctx.user_id.to_string()).await?;
    let normalized_emergency_phone = validate_profile_fields(
        Some(&body.full_name),
        body.blood_group.as_deref(),
        body.emergency_contact_relation.as_deref(),
        body.emergency_contact_phone.as_deref(),
        Some(body.date_of_birth),
        body.weight_kg.as_ref(),
        Some(&body.relationship),
    )?;

    let self_id = ctx.patient_profile_id(&state).await?;
    let mut tx = state.begin_authed(&ctx).await?;
    let now = Utc::now();
    let dependent = PatientProfile {
        id: PatientProfileId::new(),
        user_id: None,
        full_name: body.full_name,
        date_of_birth: body.date_of_birth,
        gender: body.gender,
        blood_group: body.blood_group,
        weight_kg: body.weight_kg,
        emergency_contact_phone: normalized_emergency_phone,
        emergency_contact_relation: body.emergency_contact_relation,
        guardian_patient_id: Some(self_id),
        relationship: Some(body.relationship),
        created_at: now,
        updated_at: now,
    };
    patient_repo::insert(&mut *tx, &dependent).await?;
    tx.commit().await?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(ApiResponse {
            success: true,
            data: dependent,
        }),
    ))
}

/// `PATCH /patients/me/profiles/:id` — update one dependent's own fields.
/// `/patients/me` (unchanged) stays the dedicated self-profile endpoint;
/// this route is dependents only — attempting to use it on the caller's own
/// self-profile id is rejected rather than silently redirected, so the two
/// endpoints' purposes stay unambiguous.
pub async fn update_dependent_profile(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<PatientProfileId>,
    Json(body): Json<PatientProfileUpdate>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::PatientManageProfile)?;
    let self_id = ctx.patient_profile_id(&state).await?;
    if id == self_id {
        return Err(AppError::Validation(
            "Use PATCH /patients/me to update your own profile.".into(),
        ));
    }
    let normalized_emergency_phone = validate_profile_fields(
        body.full_name.as_deref(),
        body.blood_group.as_deref(),
        body.emergency_contact_relation.as_deref(),
        body.emergency_contact_phone.as_deref(),
        body.date_of_birth,
        body.weight_kg.as_ref(),
        body.relationship.as_deref(),
    )?;

    let mut tx = state.begin_authed(&ctx).await?;
    let dependent = patient_repo::find_by_id(&mut *tx, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Patient profile not found.".into()))?;
    if dependent.guardian_patient_id != Some(self_id) {
        return Err(AppError::NotFound("Patient profile not found.".into()));
    }
    let rows = patient_repo::update(
        &mut *tx,
        id,
        body.full_name,
        body.date_of_birth,
        body.gender,
        body.blood_group,
        body.weight_kg,
        normalized_emergency_phone,
        body.emergency_contact_relation,
        body.relationship,
        Utc::now(),
    )
    .await?;
    tx.commit().await?;
    if rows != 1 {
        return Err(AppError::NotFound("Patient profile not found.".into()));
    }
    Ok(Json(ApiResponse {
        success: true,
        data: json!({"updated": true}),
    }))
}

#[derive(Debug, Deserialize)]
pub struct AppointmentListQuery {
    /// Friendly filter for the Flutter client's Home/history screens —
    /// deliberately NOT the raw per-status string filter
    /// `doctor_handlers::list_own_appointments` takes, since a patient has
    /// no use case needing one exact status at a time. `"upcoming"` maps to
    /// every pre-completion/pre-terminal status; `"past"` to every
    /// completed/terminal one; omitted returns everything, upcoming-first.
    pub status: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    /// Which profile's appointments to list — the caller's own self profile
    /// when omitted, or a dependent they guardian (see
    /// `AuthContext::acting_patient_profile_id`).
    pub profile_id: Option<PatientProfileId>,
}

const UPCOMING_STATUSES: [&str; 4] = ["PENDING_PAYMENT", "CONFIRMED", "WAITING", "IN_CONSULTATION"];
const PAST_STATUSES: [&str; 6] = [
    "COMPLETED",
    "CANCELLED",
    "EXPIRED",
    "PREMATURE_TERMINATION",
    "DISPUTED",
    "REFUNDED",
];

pub async fn list_own_appointments(
    State(state): State<AppState>,
    ctx: AuthContext,
    Query(q): Query<AppointmentListQuery>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::PatientRead)?;
    let id = ctx.acting_patient_profile_id(&state, q.profile_id).await?;
    let limit = q.limit.unwrap_or(25).clamp(1, 100);
    let offset = q.offset.unwrap_or(0).max(0);
    let statuses: Option<&[&str]> = match q.status.as_deref() {
        Some("upcoming") => Some(&UPCOMING_STATUSES),
        Some("past") => Some(&PAST_STATUSES),
        Some(other) => {
            return Err(AppError::Validation(format!(
                "status must be \"upcoming\" or \"past\", got \"{other}\"."
            )))
        }
        None => None,
    };
    let status_filter: Option<Vec<&str>> = statuses.map(|s| s.to_vec());

    let mut tx = state.begin_authed(&ctx).await?;
    // Same upcoming-first / nearest-start-time-first ordering rule as
    // `doctor_handlers::list_own_appointments` (an approved product rule,
    // not just a convenient sort) — a patient's own Home/history screens
    // should show the same temporal logic a doctor's Queue does.
    //
    // Joins `doctor_public_directory` (the RLS-safe projection), NOT the
    // raw `doctor_profiles` table — confirmed directly against the RLS
    // policy (`doctor_private_read`, `migrations/20261026000001_...`),
    // which restricts `doctor_profiles` SELECT to the doctor's own row or an
    // admin role. A patient session joining `doctor_profiles` directly
    // silently loses every row (RLS, not an error) rather than failing
    // loudly — caught by this endpoint's own integration test asserting a
    // booked appointment is actually returned, not just by code review.
    let rows = sqlx::query(
        "SELECT a.id, a.appointment_number, a.doctor_id, a.modality::text AS modality, \
                a.status::text AS status, a.consultation_fee, a.clinical_outcome, \
                a.completed_at, s.start_time, s.end_time, \
                d.full_name AS doctor_display_name, d.primary_specialty, c.expires_at AS chat_expires_at, c.is_locked AS chat_locked \
         FROM appointments a \
         JOIN doctor_schedule_slots s ON s.id = a.slot_id \
         JOIN doctor_public_directory d ON d.id = a.doctor_id \
         LEFT JOIN chat_conversations c ON c.appointment_id = a.id \
         WHERE a.patient_id = $1 \
           AND ($4::text[] IS NULL OR a.status::text = ANY($4)) \
         ORDER BY \
           CASE WHEN a.status::text IN ('PENDING_PAYMENT','CONFIRMED','WAITING','IN_CONSULTATION') THEN 0 ELSE 1 END ASC, \
           CASE WHEN a.status::text IN ('PENDING_PAYMENT','CONFIRMED','WAITING','IN_CONSULTATION') THEN s.start_time END ASC, \
           CASE WHEN a.status::text NOT IN ('PENDING_PAYMENT','CONFIRMED','WAITING','IN_CONSULTATION') THEN s.start_time END DESC, \
           a.id ASC \
         LIMIT $2 OFFSET $3",
    )
    .bind(id)
    .bind(limit)
    .bind(offset)
    .bind(status_filter)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;

    let data: Vec<_> = rows
        .into_iter()
        .map(|r| {
            json!({
                "id": r.get::<uuid::Uuid, _>("id"),
                "appointment_number": r.get::<String, _>("appointment_number"),
                "doctor_id": r.get::<uuid::Uuid, _>("doctor_id"),
                "doctor_display_name": r.get::<String, _>("doctor_display_name"),
                "primary_specialty": r.get::<String, _>("primary_specialty"),
                "modality": r.get::<String, _>("modality"),
                "status": r.get::<String, _>("status"),
                "consultation_fee": r.get::<bigdecimal::BigDecimal, _>("consultation_fee"),
                "chat_expires_at": r.get::<Option<chrono::DateTime<chrono::Utc>>, _>("chat_expires_at"),
                "chat_locked": r.get::<Option<bool>, _>("chat_locked"),
                "clinical_outcome": r.get::<Option<String>, _>("clinical_outcome"),
                "completed_at": r.get::<Option<chrono::DateTime<chrono::Utc>>, _>("completed_at"),
                "start_time": r.get::<chrono::DateTime<chrono::Utc>, _>("start_time"),
                "end_time": r.get::<chrono::DateTime<chrono::Utc>, _>("end_time"),
            })
        })
        .collect();
    Ok(Json(ApiResponse {
        success: true,
        data,
    }))
}

#[derive(Debug, Deserialize)]
pub struct DocumentListQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    /// Which profile's documents to list — the caller's own self profile
    /// when omitted, or a dependent they guardian (see
    /// `AuthContext::acting_patient_profile_id`).
    pub profile_id: Option<PatientProfileId>,
}

/// `GET /patients/me/documents` — Health Vault (plan's gap #2, M7): merges
/// doctor-issued e-Prescriptions (`prescriptions`, excluding no-Rx decision
/// rows, which carry `rx_image_object_key IS NULL`) and patient-uploaded
/// intake documents (`prescription_intake_documents`) into one chronological
/// list. `prescription_intake_documents` already had a `list_*_for_patient`
/// repo query; `prescriptions` did not, so this queries it directly here —
/// the same precedent `list_own_appointments` above already set for a
/// patient-facing joined read, rather than adding a thin repo wrapper that
/// would only ever have this one caller. Neither table needed a migration.
///
/// `security_status != 'CLEAN'` rows are excluded — the same gate
/// `prescription_handlers::authorized_document` enforces at download time,
/// so nothing in this list can ever 409 when the patient actually opens it.
/// Each entry carries enough for the Flutter client to request a real
/// download URL via the already-existing `GET
/// /appointments/:id/documents/:document_id` (confirmed directly in
/// `authorized_document`: it already tries intake documents, then falls back
/// to prescriptions, by the same `document_id` path — this endpoint needed
/// no changes there).
pub async fn list_own_documents(
    State(state): State<AppState>,
    ctx: AuthContext,
    Query(q): Query<DocumentListQuery>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::PatientRead)?;
    let id = ctx.acting_patient_profile_id(&state, q.profile_id).await?;
    let limit = q.limit.unwrap_or(50).clamp(1, 100) as usize;
    let offset = q.offset.unwrap_or(0).max(0) as usize;

    let mut tx = state.begin_authed(&ctx).await?;
    let prescription_rows = sqlx::query(
        "SELECT p.id, p.appointment_id, p.rx_number, p.page_number, p.mime_type, p.created_at, \
                d.full_name AS doctor_display_name, d.primary_specialty \
         FROM prescriptions p \
         JOIN doctor_public_directory d ON d.id = p.doctor_id \
         WHERE p.patient_id = $1 AND p.security_status = 'CLEAN' \
           AND p.rx_image_object_key IS NOT NULL \
         ORDER BY p.created_at DESC",
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await?;
    let intake_rows = sqlx::query(
        "SELECT id, appointment_id, page_number, mime_type, created_at \
         FROM prescription_intake_documents \
         WHERE patient_id = $1 AND security_status = 'CLEAN' \
         ORDER BY created_at DESC",
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;

    let mut documents: Vec<(chrono::DateTime<Utc>, serde_json::Value)> = Vec::new();
    for r in prescription_rows {
        let created_at: chrono::DateTime<Utc> = r.get("created_at");
        documents.push((
            created_at,
            json!({
                "kind": "PRESCRIPTION",
                "id": r.get::<uuid::Uuid, _>("id"),
                "appointment_id": r.get::<uuid::Uuid, _>("appointment_id"),
                "rx_number": r.get::<String, _>("rx_number"),
                "page_number": r.get::<i16, _>("page_number"),
                "mime_type": r.get::<Option<String>, _>("mime_type"),
                "doctor_display_name": r.get::<String, _>("doctor_display_name"),
                "primary_specialty": r.get::<String, _>("primary_specialty"),
                "created_at": created_at,
            }),
        ));
    }
    for r in intake_rows {
        let created_at: chrono::DateTime<Utc> = r.get("created_at");
        documents.push((
            created_at,
            json!({
                "kind": "INTAKE",
                "id": r.get::<uuid::Uuid, _>("id"),
                "appointment_id": r.get::<uuid::Uuid, _>("appointment_id"),
                "page_number": r.get::<i16, _>("page_number"),
                "mime_type": r.get::<String, _>("mime_type"),
                "created_at": created_at,
            }),
        ));
    }
    documents.sort_by_key(|d| std::cmp::Reverse(d.0));
    let data: Vec<_> = documents
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|(_, v)| v)
        .collect();

    Ok(Json(ApiResponse {
        success: true,
        data,
    }))
}

#[derive(Debug, Deserialize)]
pub struct QnaListQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    /// Add only this account's own/guardian OPEN questions. Default preserves
    /// the existing public answered-feed contract and anonymous author grants.
    pub include_own_open: Option<bool>,
}

/// Public answered feed, optionally including only the caller's own/guardian
/// OPEN submissions. Anonymous author linkage remains inaccessible to the API
/// role; a context-bound ID-only function implements the optional owner read.
pub async fn list_answered_questions(
    State(state): State<AppState>,
    ctx: AuthContext,
    Query(q): Query<QnaListQuery>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::PatientRead)?;
    let limit = q.limit.unwrap_or(30).clamp(1, 100);
    let offset = q.offset.unwrap_or(0).max(0);
    let mut tx = state.begin_authed(&ctx).await?;
    let rows = sqlx::query(
        "SELECT q.id, q.question_text, q.category, q.status, q.created_at, a.answer_text, \
                a.created_at AS answered_at, d.full_name AS answering_doctor, \
                d.primary_specialty AS answering_doctor_specialty, (d.professional_verification_status='VERIFIED') AS answering_doctor_verified \
         FROM health_questions q \
         LEFT JOIN health_answers a ON a.question_id = q.id \
         LEFT JOIN doctor_public_directory d ON d.id = a.doctor_id \
         WHERE q.status = 'ANSWERED' \
            OR ($3 AND q.id IN (SELECT id FROM patient_owned_open_question_ids())) \
         ORDER BY COALESCE(a.created_at,q.created_at) DESC, q.id LIMIT $1 OFFSET $2",
    )
    .bind(limit)
    .bind(offset)
    .bind(q.include_own_open.unwrap_or(false))
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    let data: Vec<_> = rows
        .into_iter()
        .map(|r| {
            json!({
                "id": r.get::<uuid::Uuid, _>("id"),
                "question_text": r.get::<String, _>("question_text"),
                "status": r.get::<String, _>("status"),
                "category": r.get::<Option<String>, _>("category"),
                "created_at": r.get::<chrono::DateTime<Utc>, _>("created_at"),
                "answer_text": r.get::<Option<String>, _>("answer_text"),
                "answered_at": r.get::<Option<chrono::DateTime<Utc>>, _>("answered_at"),
                "answering_doctor": r.get::<Option<String>, _>("answering_doctor"),
                "answering_doctor_specialty": r.get::<Option<String>, _>("answering_doctor_specialty"),
                "answering_doctor_verified": r.get::<Option<bool>, _>("answering_doctor_verified").unwrap_or(false),
            })
        })
        .collect();
    Ok(Json(ApiResponse {
        success: true,
        data,
    }))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmitQuestionRequest {
    pub question_text: String,
    pub category: Option<String>,
    /// Ask on behalf of a dependent profile (see
    /// `AuthContext::acting_patient_profile_id`); the caller's own self
    /// profile when omitted.
    pub profile_id: Option<PatientProfileId>,
}

/// `POST /patients/me/qna/questions` — the write half of gap #6. Closed by
/// `20261101000002_patient_qna_submission`, which granted INSERT and added
/// the first-ever INSERT policy on `health_questions` (confirmed by direct
/// read of `20261005000001_doctor_contract_qna`: it granted `UPDATE` only).
/// Deliberately returns only the column-scoped-granted columns in `RETURNING`
/// — `author_patient_id` has no SELECT grant for `hellodoctor_api`
/// (`20261026000001_canonical_user_isolation`), and Postgres checks SELECT
/// privilege on every column named in a `RETURNING` list, so naming it here
/// would fail the insert outright, not just omit it from the response.
pub async fn submit_question(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(body): Json<SubmitQuestionRequest>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::SubmitHealthQuestion)?;
    rate_limit::check_question_submission_limit(&state.redis, &ctx.user_id.to_string()).await?;
    let text = body.question_text.trim();
    if text.is_empty() || text.chars().count() > 4000 {
        return Err(AppError::Validation(
            "question_text must contain 1-4000 characters.".into(),
        ));
    }
    let category = body
        .category
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty());
    if let Some(c) = category {
        if c.chars().count() > 80 {
            return Err(AppError::Validation(
                "category must be at most 80 characters.".into(),
            ));
        }
    }
    let author_id = ctx
        .acting_patient_profile_id(&state, body.profile_id)
        .await?;
    let mut tx = state.begin_authed(&ctx).await?;
    let now = Utc::now();
    let row = sqlx::query(
        "INSERT INTO health_questions (id, author_patient_id, question_text, category, status, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, 'OPEN', $5, $5) \
         RETURNING id, question_text, category, status, created_at, updated_at",
    )
    .bind(uuid::Uuid::new_v4())
    .bind(author_id)
    .bind(text)
    .bind(category)
    .bind(now)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(ApiResponse {
            success: true,
            data: json!({
                "id": row.get::<uuid::Uuid, _>("id"),
                "question_text": row.get::<String, _>("question_text"),
                "category": row.get::<Option<String>, _>("category"),
                "status": row.get::<String, _>("status"),
                "created_at": row.get::<chrono::DateTime<Utc>, _>("created_at"),
                "updated_at": row.get::<chrono::DateTime<Utc>, _>("updated_at"),
            }),
        }),
    ))
}

// ---------------------------------------------------------------------
// Patient-saved payment methods — "the body waiting for the heart" (the
// real payment-gateway API key/driver). Mirrors the already-proven
// doctor_payout_destinations shape: multiple saved methods, exactly one
// active primary, selected at payment time rather than re-entered every
// booking. See migrations/20261108000001_patient_payment_methods.up.sql.
// ---------------------------------------------------------------------

fn payment_method_json(m: &crate::domain::models::PatientPaymentMethod) -> serde_json::Value {
    json!({
        "id": m.id, "gateway": m.gateway, "provider_code": m.provider_code,
        "masked_destination": m.masked_destination, "binding_status": m.binding_status,
        "verification_status": m.verification_status,
        "is_primary": m.is_primary, "created_at": m.created_at, "updated_at": m.updated_at,
    })
}

#[derive(Debug, Deserialize)]
pub struct PaymentMethodProfileQuery {
    pub profile_id: Option<PatientProfileId>,
}

pub async fn list_own_payment_methods(
    State(state): State<AppState>,
    ctx: AuthContext,
    Query(q): Query<PaymentMethodProfileQuery>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ManagePatientPaymentMethod)?;
    let id = ctx.acting_patient_profile_id(&state, q.profile_id).await?;
    let mut tx = state.begin_authed(&ctx).await?;
    let methods = payment_methods_repo::list_patient_payment_methods(&mut *tx, id).await?;
    tx.commit().await?;
    Ok(Json(ApiResponse {
        success: true,
        data: methods.iter().map(payment_method_json).collect::<Vec<_>>(),
    }))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavePaymentMethodRequest {
    pub gateway: Gateway,
    pub account_identifier: String,
    pub profile_id: Option<PatientProfileId>,
    /// Defaults to true for a profile's very first saved method (the
    /// service layer below applies that default); only needs to be
    /// explicit when the acting profile already has at least one method
    /// and is choosing whether the new one replaces it as primary.
    #[serde(default)]
    pub make_primary: Option<bool>,
}

fn gateway_display_name(gateway: Gateway) -> &'static str {
    match gateway {
        Gateway::Mpesa => "M-Pesa",
        Gateway::Bkash => "bKash",
        Gateway::Nagad => "Nagad",
        Gateway::Card => "Card",
        Gateway::BankTransfer => "Bank Transfer",
    }
}

/// Validates and encrypts the real identifier the same way
/// `wallet_handlers::validate_payout_identifier`/`bind_payout_destination`
/// already do for a doctor's own payout reference — deliberately the same
/// shape, not a parallel one, since both are "a Kenya payment identifier a
/// real person provided."
fn validate_and_encrypt_payment_identifier(
    gateway: Gateway,
    raw: &str,
    totp_encryption_key_b64: &str,
) -> Result<(String, String), AppError> {
    let clean = raw.trim().replace([' ', '-'], "");
    match gateway {
        Gateway::Mpesa | Gateway::Bkash => {
            let normalized = normalize_kenya_phone_number(&clean)?;
            let last4: String = normalized
                .chars()
                .rev()
                .take(4)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            let masked = format!(
                "{} \u{2022}\u{2022}\u{2022}\u{2022} {}",
                gateway_display_name(gateway),
                last4
            );
            let encrypted_bytes = crypto::encrypt_secret(&normalized, totp_encryption_key_b64)?;
            Ok((
                base64::engine::general_purpose::STANDARD.encode(encrypted_bytes),
                masked,
            ))
        }
        Gateway::Card => {
            if clean.chars().count() < 12
                || clean.chars().count() > 19
                || !clean.chars().all(|c| c.is_ascii_digit())
            {
                return Err(AppError::Validation(
                    "Card number must be 12-19 digits.".into(),
                ));
            }
            let last4: String = clean
                .chars()
                .rev()
                .take(4)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            let masked = format!("Card \u{2022}\u{2022}\u{2022}\u{2022} {last4}");
            let encrypted_bytes = crypto::encrypt_secret(&clean, totp_encryption_key_b64)?;
            Ok((
                base64::engine::general_purpose::STANDARD.encode(encrypted_bytes),
                masked,
            ))
        }
        Gateway::Nagad | Gateway::BankTransfer => Err(AppError::Validation(
            "This payment method is not available for the current Kenya deployment.".into(),
        )),
    }
}

pub async fn save_payment_method(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(payload): Json<SavePaymentMethodRequest>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ManagePatientPaymentMethod)?;
    let id = ctx
        .acting_patient_profile_id(&state, payload.profile_id)
        .await?;
    let (encrypted, masked_destination) = validate_and_encrypt_payment_identifier(
        payload.gateway,
        &payload.account_identifier,
        &state.config.totp_encryption_key_b64,
    )?;
    let provider_code = match payload.gateway {
        Gateway::Mpesa => "MPESA",
        Gateway::Bkash => "BKASH",
        Gateway::Card => "CARD",
        Gateway::Nagad | Gateway::BankTransfer => unreachable!("rejected above"),
    };

    let mut tx = state.begin_authed(&ctx).await?;
    // Requires the platform to actually have a provider configured for this
    // gateway before anything is saved — see
    // services::payment_provider_verification for why this is a hard stop,
    // not a warning.
    let verification =
        crate::services::payment_provider_verification::verify_patient_payment_method(
            &mut *tx,
            payload.gateway,
        )
        .await?;
    let existing = payment_methods_repo::list_patient_payment_methods(&mut *tx, id).await?;
    let make_primary = payload.make_primary.unwrap_or(existing.is_empty());
    let method = payment_methods_repo::insert_patient_payment_method(
        &mut tx,
        id,
        payload.gateway,
        provider_code,
        &encrypted,
        &masked_destination,
        verification.verification_status,
        verification.platform_payment_method_id,
        make_primary,
    )
    .await?;
    tx.commit().await?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(ApiResponse {
            success: true,
            data: payment_method_json(&method),
        }),
    ))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetPrimaryPaymentMethodRequest {
    pub profile_id: Option<PatientProfileId>,
}

pub async fn set_primary_payment_method(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(method_id): Path<uuid::Uuid>,
    Json(payload): Json<SetPrimaryPaymentMethodRequest>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ManagePatientPaymentMethod)?;
    let id = ctx
        .acting_patient_profile_id(&state, payload.profile_id)
        .await?;
    let mut tx = state.begin_authed(&ctx).await?;
    let updated =
        payment_methods_repo::set_primary_patient_payment_method(&mut tx, id, method_id).await?;
    if !updated {
        return Err(AppError::NotFound(
            "No active payment method with that id belongs to this profile.".into(),
        ));
    }
    tx.commit().await?;
    Ok(Json(ApiResponse {
        success: true,
        data: json!({ "id": method_id, "is_primary": true }),
    }))
}

pub async fn remove_payment_method(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(method_id): Path<uuid::Uuid>,
    Query(q): Query<PaymentMethodProfileQuery>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ManagePatientPaymentMethod)?;
    let id = ctx.acting_patient_profile_id(&state, q.profile_id).await?;
    let mut tx = state.begin_authed(&ctx).await?;
    let removed =
        payment_methods_repo::deactivate_patient_payment_method(&mut *tx, id, method_id).await?;
    if !removed {
        return Err(AppError::NotFound(
            "No active payment method with that id belongs to this profile.".into(),
        ));
    }
    tx.commit().await?;
    Ok(Json(ApiResponse {
        success: true,
        data: json!({ "id": method_id, "removed": true }),
    }))
}
