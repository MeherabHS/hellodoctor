//! Peer Navigator endpoints, Phase 1 (docs/peer-navigator/PEER_NAVIGATOR_PLAN.md):
//! sign-up and sign-in (Care app), the peer's own profile and support slots,
//! the approved-peer list patients choose from, and Admin approval.

use crate::abac::{self, Action};
use crate::auth_context::AuthContext;
use crate::domain::ids::UserId;
use crate::domain::models::{User, UserRole};
use crate::domain::validation;
use crate::error::AppError;
use crate::repository::{peer_repo, user_repo, AppState, DbRole};
use crate::services::audit_service::{AuditLogEntry, AuditService};
use crate::services::auth_service::AuthService;
use crate::{crypto, ip_jail, rate_limit};
use axum::{
    extract::{ConnectInfo, OriginalUri, Path, Query, State},
    http::{HeaderMap, Method, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use serde_json::json;
use std::net::SocketAddr;
use uuid::Uuid;

/// Peer support slots are 10 minutes, like the doctor schedule.
pub const PEER_SLOT_MINUTES: i64 = 10;

fn ok<T: serde::Serialize>(data: T) -> Json<serde_json::Value> {
    Json(json!({ "success": true, "data": data }))
}

fn text_field(value: &str, min: usize, max: usize, label: &str) -> Result<String, AppError> {
    let clean = value.trim();
    let count = clean.chars().count();
    // Control characters and bidirectional overrides ("Trojan source"
    // spoofing) are refused, as in `validation::validate_text_field`.
    let unsafe_char =
        |c: char| c.is_control() || matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}');
    if count < min || count > max || clean.chars().any(unsafe_char) {
        return Err(AppError::Validation(format!(
            "{label} must contain {min}-{max} characters."
        )));
    }
    Ok(clean.to_owned())
}

fn optional_text(value: Option<&str>, max: usize, label: &str) -> Result<Option<String>, AppError> {
    match value.map(str::trim).filter(|v| !v.is_empty()) {
        None => Ok(None),
        Some(v) => text_field(v, 1, max, label).map(Some),
    }
}

// ---------------------------------------------------------------------------
// Sign-up and sign-in (Care app)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct PeerRegistrationRequest {
    pub full_name: String,
    pub email: String,
    pub phone_number: String,
    pub password: String,
    pub county: String,
    pub organization: Option<String>,
    /// The three current Peer documents (`PEER_TERMS`, `PEER_PRIVACY_NOTICE`,
    /// `PEER_CONDUCT_SAFEGUARDING`), each with its current version.
    #[serde(default)]
    pub accepted_documents: Vec<crate::api::v1::auth_handlers::AcceptedDocument>,
}

/// Every Peer must accept all three current Peer documents at sign-up.
pub const PEER_DOCUMENT_TYPES: [&str; 3] = [
    "PEER_TERMS",
    "PEER_PRIVACY_NOTICE",
    "PEER_CONDUCT_SAFEGUARDING",
];

impl std::fmt::Debug for PeerRegistrationRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PeerRegistrationRequest")
            .field("email", &self.email)
            .field("password", &"[REDACTED]")
            .finish()
    }
}

/// `POST /auth/peer/register` — creates a PEER account whose profile waits
/// for Admin approval. Patients cannot see or book the peer until then.
pub async fn register(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(payload): Json<PeerRegistrationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let ip = addr.ip().to_string();
    rate_limit::check_and_increment(
        &state.redis,
        &format!("ratelimit:peer-registration:{ip}"),
        5,
        900,
    )
    .await?;
    let full_name = text_field(&payload.full_name, 2, 120, "Full name")?;
    let county = text_field(&payload.county, 2, 60, "County")?;
    let organization = optional_text(payload.organization.as_deref(), 150, "Organization")?;
    let phone = validation::normalize_kenya_phone_number(&payload.phone_number)?;
    validation::validate_email(&payload.email)?;
    validation::validate_password_strength(&payload.password)?;
    let email = validation::normalize_email(&payload.email);

    let duplicate = user_repo::find_by_email(&state.auth_db, &email)
        .await?
        .is_some()
        || user_repo::find_by_phone(&state.auth_db, &phone)
            .await?
            .is_some();
    if duplicate {
        return Err(AppError::Conflict(
            "Registration details are already in use.".into(),
        ));
    }
    // The current version of each Peer document must be accepted; the hash is
    // taken from the server's own registry, never from the request.
    let current_documents: Vec<_> = crate::repository::legal_repo::list_current(&state.auth_db)
        .await?
        .into_iter()
        .filter(|d| PEER_DOCUMENT_TYPES.contains(&d.document_type.as_str()))
        .collect();
    if current_documents.len() != PEER_DOCUMENT_TYPES.len() {
        return Err(AppError::internal(
            "Peer Navigator documents are not configured.".into(),
        ));
    }
    let all_accepted = current_documents.iter().all(|doc| {
        payload
            .accepted_documents
            .iter()
            .any(|a| a.document_type == doc.document_type.as_str() && a.version == doc.version)
    });
    if !all_accepted {
        return Err(AppError::Validation(
            "Read and accept the current Peer Navigator Terms, Privacy Notice and Confidentiality, Conduct & Safeguarding Agreement.".into(),
        ));
    }
    if user_repo::count_active(&state.auth_db).await? as u32 >= state.config.max_active_users {
        return Err(AppError::CapacityLimitReached(
            "AfyaMkononi is at its current controlled pilot capacity.".into(),
        ));
    }

    let user_id = UserId::new();
    let now = Utc::now();
    let password_hash = crypto::hash_password_async(payload.password.clone()).await?;
    let user = User {
        id: user_id,
        phone_number: phone,
        email: Some(email),
        password_hash: Some(password_hash),
        role: UserRole::Peer,
        preferred_language: "en".into(),
        is_active: true,
        is_verified: false,
        created_at: now,
        updated_at: now,
        deleted_at: None,
    };
    let mut tx = state
        .begin_authed_as(
            &AuthContext {
                user_id,
                role: UserRole::Peer,
                permissions: vec![],
                session_id: None,
                request_id: None,
            },
            DbRole::AuthWorker,
        )
        .await?;
    user_repo::insert_user(&mut *tx, &user).await?;
    let peer_id = Uuid::new_v4();
    peer_repo::insert(
        &mut *tx,
        peer_id,
        user_id,
        &full_name,
        &county,
        organization.as_deref(),
        now,
    )
    .await?;
    for doc in &current_documents {
        crate::repository::legal_repo::insert(
            &mut *tx,
            &crate::domain::models::LegalAcceptance {
                id: Uuid::new_v4(),
                user_id,
                document_type: doc.document_type,
                document_version: doc.version.clone(),
                document_hash: doc.document_hash.clone(),
                accepted_at: now,
                request_id: None,
                session_id: None,
                ip_hash: None,
                app_version: None,
                platform: None,
                locale: None,
                created_at: now,
            },
        )
        .await?;
    }
    // Tells Admins a new application is waiting (Admin notifications).
    // County only: no name, contact details or documents in the audit row.
    AuditService::log_in_tx(
        &mut *tx,
        AuditLogEntry::new(
            Some(user_id),
            UserRole::Peer,
            "PEER_NAVIGATOR_CREATED",
            "PEER_NAVIGATOR",
            Some(peer_id),
            "SUCCESS",
        )
        .with_after(json!({ "county": county, "approval_status": "PENDING" })),
    )
    .await?;
    // "Application received" email, queued on this transaction like the
    // doctor one and rendered by the dispatch worker after commit.
    let outbox_id = Uuid::new_v4();
    crate::repository::email_outbox_repo::enqueue_transactional(
        &mut *tx,
        outbox_id,
        "PEER_APPLICATION_RECEIVED",
        user.email.as_deref().unwrap_or_default(),
        &format!("peer-application-received:{peer_id}"),
        "PEER_NAVIGATOR",
        peer_id,
        crate::repository::email_outbox_repo::transactional_email_expiry(),
    )
    .await?;
    tx.commit().await?;
    {
        let state = state.clone();
        tokio::spawn(async move {
            crate::services::email_dispatch_worker::dispatch_now(&state, outbox_id).await;
        });
    }
    Ok((
        StatusCode::CREATED,
        ok(json!({ "peer_id": peer_id, "approval_status": "PENDING" })),
    ))
}

#[derive(Deserialize)]
pub struct PeerLoginRequest {
    pub email: String,
    pub password: String,
}

/// `POST /auth/peer/login` — password step; an emailed code follows.
pub async fn login(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(payload): Json<PeerLoginRequest>,
) -> Result<impl IntoResponse, AppError> {
    let ip = addr.ip().to_string();
    let email = validation::normalize_email(&payload.email);
    let account_key = format!("peer:{email}");
    if ip_jail::is_account_jailed(&state.redis, &account_key).await? {
        return Err(AppError::Forbidden(
            "Too many failed login attempts for this account. Try again later.".into(),
        ));
    }
    if ip_jail::is_ip_jailed(&state.redis, &ip).await? {
        return Err(AppError::Forbidden(
            "Too many failed login attempts from this network. Try again later.".into(),
        ));
    }
    rate_limit::check_login_attempt_limit(&state.redis, &ip).await?;
    match AuthService::peer_login_step1(&state, &email, &payload.password).await {
        Ok((session_token, debug_code)) => {
            ip_jail::record_success(&state.redis, &account_key).await?;
            Ok(ok(json!({
                "session_token": session_token,
                "requires_otp": true,
                "debug_otp_code": debug_code
            })))
        }
        Err(e) => {
            if crate::services::auth_service::abuse::credential_failure(&e) {
                ip_jail::record_failure(&state.redis, &ip, &account_key).await?;
            }
            Err(e)
        }
    }
}

#[derive(Deserialize)]
pub struct PeerVerifyOtpRequest {
    pub session_token: String,
    pub otp_code: String,
}

/// `POST /auth/peer/verify-otp` — emailed-code step; issues the session.
pub async fn verify_otp(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    method: Method,
    OriginalUri(uri): OriginalUri,
    Json(payload): Json<PeerVerifyOtpRequest>,
) -> Result<impl IntoResponse, AppError> {
    let device_key = crate::services::auth_service::device_proof::establishment_proof(
        &state,
        &headers,
        &method,
        &uri,
        &payload.session_token,
    )
    .await?;
    let ip = addr.ip().to_string();
    let response = AuthService::peer_verify_otp_with_device(
        &state,
        &ip,
        &payload.session_token,
        &payload.otp_code,
        device_key,
    )
    .await?;
    Ok(ok(response))
}

// ---------------------------------------------------------------------------
// The peer's own profile and slots
// ---------------------------------------------------------------------------

async fn own_profile(
    state: &AppState,
    ctx: &AuthContext,
) -> Result<peer_repo::PeerProfile, AppError> {
    let mut tx = state.begin_authed(ctx).await?;
    let profile = peer_repo::find_by_user_id(&mut *tx, ctx.user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Peer profile not found.".into()))?;
    tx.commit().await?;
    Ok(profile)
}

/// `GET /peer/profile` — includes the approval status, so the Care app can
/// show the "waiting for approval" gate.
pub async fn get_profile(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::PeerSelf)?;
    Ok(ok(own_profile(&state, &ctx).await?))
}

#[derive(Deserialize)]
pub struct PeerProfileUpdate {
    pub full_name: Option<String>,
    pub county: Option<String>,
    pub organization: Option<String>,
    pub bio: Option<String>,
}

/// `PATCH /peer/profile` — own display details. Approval is Admin-only.
pub async fn update_profile(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(body): Json<PeerProfileUpdate>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::PeerSelf)?;
    let full_name = body
        .full_name
        .as_deref()
        .map(|v| text_field(v, 2, 120, "Full name"))
        .transpose()?;
    let county = body
        .county
        .as_deref()
        .map(|v| text_field(v, 2, 60, "County"))
        .transpose()?;
    // An empty string clears the optional fields.
    let organization = match body.organization.as_deref() {
        None => None,
        Some(v) if v.trim().is_empty() => Some(String::new()),
        Some(v) => Some(text_field(v, 1, 150, "Organization")?),
    };
    let bio = match body.bio.as_deref() {
        None => None,
        Some(v) if v.trim().is_empty() => Some(String::new()),
        Some(v) => Some(text_field(v, 1, 500, "About you")?),
    };
    let profile = own_profile(&state, &ctx).await?;
    let mut tx = state.begin_authed(&ctx).await?;
    peer_repo::update_own(
        &mut *tx,
        profile.id,
        full_name.as_deref(),
        county.as_deref(),
        organization.as_deref(),
        bio.as_deref(),
    )
    .await?;
    let updated = peer_repo::find_by_id(&mut *tx, profile.id)
        .await?
        .ok_or_else(|| AppError::NotFound("Peer profile not found.".into()))?;
    tx.commit().await?;
    Ok(ok(updated))
}

#[derive(Deserialize)]
pub struct SlotRange {
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
}

/// `GET /peer/slots?from&to` — own slots (default: today onwards, 14 days).
pub async fn list_slots(
    State(state): State<AppState>,
    ctx: AuthContext,
    Query(range): Query<SlotRange>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::PeerSelf)?;
    let from = range
        .from
        .unwrap_or_else(|| Utc::now() - Duration::hours(12));
    let to = range.to.unwrap_or(from + Duration::days(14));
    if to <= from || to - from > Duration::days(62) {
        return Err(AppError::Validation(
            "Choose a range of up to 62 days.".into(),
        ));
    }
    let profile = own_profile(&state, &ctx).await?;
    let mut tx = state.begin_authed(&ctx).await?;
    let slots = peer_repo::list_own_slots(&mut *tx, profile.id, from, to).await?;
    tx.commit().await?;
    Ok(ok(slots))
}

#[derive(Deserialize)]
pub struct CreateSlot {
    pub start_time: DateTime<Utc>,
}

/// `POST /peer/slots` — one 10-minute support slot. Only an approved peer may
/// open slots; overlapping and past slots are refused.
pub async fn create_slot(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(body): Json<CreateSlot>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::PeerSelf)?;
    let profile = own_profile(&state, &ctx).await?;
    if profile.approval_status != "APPROVED" {
        return Err(AppError::Forbidden(
            "Your Peer Navigator account must be approved before you can add slots.".into(),
        ));
    }
    if body.start_time <= Utc::now() {
        return Err(AppError::Validation("Choose a future start time.".into()));
    }
    if body.start_time > Utc::now() + Duration::days(62) {
        return Err(AppError::Validation(
            "Slots can be added up to 62 days ahead.".into(),
        ));
    }
    let end = body.start_time + Duration::minutes(PEER_SLOT_MINUTES);
    let mut tx = state.begin_authed(&ctx).await?;
    if peer_repo::slot_overlaps(&mut *tx, profile.id, body.start_time, end).await? {
        return Err(AppError::Conflict(
            "This time overlaps one of your existing slots.".into(),
        ));
    }
    let id = Uuid::new_v4();
    peer_repo::insert_slot(&mut *tx, id, profile.id, body.start_time, end).await?;
    tx.commit().await?;
    Ok((
        StatusCode::CREATED,
        ok(json!({
            "id": id, "peer_id": profile.id, "start_time": body.start_time,
            "end_time": end, "status": "AVAILABLE"
        })),
    ))
}

/// `DELETE /peer/slots/:slot_id` — removes an open slot. A slot a patient has
/// already requested cannot be removed here.
pub async fn cancel_slot(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(slot_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::PeerSelf)?;
    let profile = own_profile(&state, &ctx).await?;
    let mut tx = state.begin_authed(&ctx).await?;
    if !peer_repo::cancel_open_slot(&mut *tx, profile.id, slot_id).await? {
        return Err(AppError::Conflict(
            "Only an open slot that nobody has requested can be removed.".into(),
        ));
    }
    tx.commit().await?;
    Ok(ok(json!({ "id": slot_id, "status": "CANCELLED" })))
}

// ---------------------------------------------------------------------------
// Patients choosing a peer
// ---------------------------------------------------------------------------

/// `GET /peers` — approved peers with their next open slot.
pub async fn directory(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::PatientRead)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let peers = peer_repo::directory(&mut *tx).await?;
    tx.commit().await?;
    Ok(ok(peers))
}

/// `GET /peers/:peer_id/slots` — that peer's open, future slots.
pub async fn peer_open_slots(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(peer_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::PatientRead)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let slots = peer_repo::open_slots(&mut *tx, peer_id).await?;
    tx.commit().await?;
    Ok(ok(slots))
}

// ---------------------------------------------------------------------------
// Admin approval
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct AdminPeerQuery {
    pub status: Option<String>,
}

/// `GET /admin/peers?status=PENDING` — Peer Navigators for review.
pub async fn admin_list(
    State(state): State<AppState>,
    ctx: AuthContext,
    Query(q): Query<AdminPeerQuery>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ManagePeerNavigators)?;
    let status = match q.status.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        None => None,
        Some(s @ ("PENDING" | "APPROVED" | "REJECTED" | "SUSPENDED")) => Some(s.to_owned()),
        Some(_) => return Err(AppError::Validation("Unknown peer status filter.".into())),
    };
    let mut tx = state.begin_authed(&ctx).await?;
    let peers = peer_repo::admin_list(&mut *tx, status.as_deref()).await?;
    tx.commit().await?;
    Ok(ok(peers))
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct PeerDirectoryStats {
    peer_id: Uuid,
    requests: i64,
    sessions_completed: i64,
    sessions_upcoming: i64,
    patients_supported: i64,
    chat_sessions: i64,
    call_sessions: i64,
    messages: i64,
    last_session_at: Option<DateTime<Utc>>,
}

/// `GET /admin/peer-directory` — every Peer Navigator with activity counts.
/// Counts only: no topics, no chat content.
pub async fn admin_directory(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ManagePeerNavigators)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let peers = peer_repo::admin_list(&mut *tx, None).await?;
    let stats: Vec<PeerDirectoryStats> = sqlx::query_as("SELECT * FROM fn_admin_peer_directory()")
        .fetch_all(&mut *tx)
        .await?;
    tx.commit().await?;
    let rows: Vec<serde_json::Value> = peers
        .into_iter()
        .map(|peer| {
            let mut row = serde_json::to_value(&peer).unwrap_or_else(|_| json!({}));
            let id = row.get("id").and_then(|v| v.as_str()).map(str::to_owned);
            let stat = stats
                .iter()
                .find(|s| Some(s.peer_id.to_string()) == id)
                .map(|s| serde_json::to_value(s).unwrap_or_default())
                .unwrap_or_else(|| json!({}));
            if let (Some(obj), Some(extra)) = (row.as_object_mut(), stat.as_object()) {
                for (k, v) in extra {
                    if k != "peer_id" {
                        obj.insert(k.clone(), v.clone());
                    }
                }
            }
            row
        })
        .collect();
    Ok(ok(rows))
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct PeerHistoryRow {
    request_id: Uuid,
    patient_id: Uuid,
    patient_name: String,
    support_type: String,
    status: String,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
    requested_at: DateTime<Utc>,
    decided_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    cancelled_at: Option<DateTime<Utc>>,
    messages: i64,
    call_started_at: Option<DateTime<Utc>>,
    call_ended_at: Option<DateTime<Utc>>,
}

/// `GET /admin/peers/:peer_id/history` — the peer's sessions with the
/// patients they supported, plus their account status history. Never the
/// topic a patient wrote or any message. Viewing is audited.
pub async fn admin_history(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(peer_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ManagePeerNavigators)?;
    let mut tx = state.begin_authed(&ctx).await?;
    peer_repo::find_by_id(&mut *tx, peer_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Peer Navigator not found.".into()))?;
    let sessions: Vec<PeerHistoryRow> = sqlx::query_as("SELECT * FROM fn_admin_peer_history($1)")
        .bind(peer_id)
        .fetch_all(&mut *tx)
        .await?;
    let status_history: Vec<(String, DateTime<Utc>, Option<String>)> = sqlx::query_as(
        "SELECT action, created_at, actor_role::text FROM audit_events          WHERE resource_type = 'PEER_NAVIGATOR' AND resource_id = $1            AND action LIKE 'PEER_NAVIGATOR_%' AND action <> 'PEER_NAVIGATOR_HISTORY_VIEWED'          ORDER BY created_at",
    )
    .bind(peer_id)
    .fetch_all(&mut *tx)
    .await?;
    AuditService::log_in_tx(
        &mut *tx,
        AuditLogEntry::new(
            Some(ctx.user_id),
            ctx.role,
            "PEER_NAVIGATOR_HISTORY_VIEWED",
            "PEER_NAVIGATOR",
            Some(peer_id),
            "SUCCESS",
        ),
    )
    .await?;
    tx.commit().await?;
    Ok(ok(json!({
        "sessions": sessions,
        "status_history": status_history
            .into_iter()
            .map(|(action, at, actor)| json!({
                "event": action.trim_start_matches("PEER_NAVIGATOR_"),
                "at": at,
                "by": actor,
            }))
            .collect::<Vec<_>>(),
    })))
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct PatientPeerSessionRow {
    request_id: Uuid,
    peer_id: Uuid,
    peer_name: String,
    support_type: String,
    status: String,
    start_time: DateTime<Utc>,
    requested_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
    messages: i64,
}

/// `GET /admin/patients/:id/peer-sessions` — the patient's Peer Navigator
/// sessions for their Admin record (the record view itself is audited by
/// `POST /admin/audit/views`). Never the topic or any message.
pub async fn admin_patient_sessions(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(patient_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ManagePeerNavigators)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let rows: Vec<PatientPeerSessionRow> =
        sqlx::query_as("SELECT * FROM fn_admin_patient_peer_sessions($1)")
            .bind(patient_id)
            .fetch_all(&mut *tx)
            .await?;
    tx.commit().await?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
pub struct AdminPeerDecision {
    pub note: Option<String>,
}

/// `POST /admin/peers/:peer_id/:action` with action `approve`, `reject`,
/// `suspend` or `reinstate`. Rejecting or suspending needs a note.
pub async fn admin_decide(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path((peer_id, action)): Path<(Uuid, String)>,
    Json(body): Json<AdminPeerDecision>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ManagePeerNavigators)?;
    let (target, allowed_from, needs_note): (&str, &[&str], bool) = match action.as_str() {
        "approve" => ("APPROVED", &["PENDING", "REJECTED"], false),
        "reject" => ("REJECTED", &["PENDING"], true),
        "suspend" => ("SUSPENDED", &["APPROVED"], true),
        "reinstate" => ("APPROVED", &["SUSPENDED"], false),
        _ => return Err(AppError::NotFound("Unknown peer action.".into())),
    };
    let note = optional_text(body.note.as_deref(), 500, "Note")?;
    if needs_note && note.is_none() {
        return Err(AppError::Validation(
            "Add a short note explaining this decision.".into(),
        ));
    }
    let mut tx = state.begin_authed(&ctx).await?;
    let current = peer_repo::find_by_id(&mut *tx, peer_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Peer Navigator not found.".into()))?;
    if !allowed_from.contains(&current.approval_status.as_str()) {
        return Err(AppError::Conflict(format!(
            "A {} Peer Navigator cannot be {}.",
            current.approval_status.to_lowercase(),
            match action.as_str() {
                "approve" | "reinstate" => "approved",
                "reject" => "rejected",
                _ => "suspended",
            }
        )));
    }
    peer_repo::set_approval(&mut *tx, peer_id, target, note.as_deref(), ctx.user_id).await?;
    // The decision email (never the note). `find_email_by_id`: the Admin
    // role has no grant on `users.password_hash`.
    let email_type = match action.as_str() {
        "approve" => "PEER_APPROVED",
        "reject" => "PEER_REJECTED",
        "suspend" => "PEER_SUSPENDED",
        _ => "PEER_REINSTATED",
    };
    let outbox_id =
        match crate::repository::user_repo::find_email_by_id(&mut *tx, current.user_id).await? {
            Some(email) => {
                let id = Uuid::new_v4();
                crate::repository::email_outbox_repo::enqueue_transactional(
                    &mut *tx,
                    id,
                    email_type,
                    &email,
                    // One email per decision: a later decision is a new event.
                    &format!("peer-decision:{peer_id}:{}", Uuid::new_v4()),
                    "PEER_NAVIGATOR",
                    peer_id,
                    crate::repository::email_outbox_repo::transactional_email_expiry(),
                )
                .await?;
                Some(id)
            }
            None => None,
        };
    AuditService::log_in_tx(
        &mut *tx,
        AuditLogEntry::new(
            Some(ctx.user_id),
            ctx.role,
            format!("PEER_NAVIGATOR_{}", target),
            "PEER_NAVIGATOR",
            Some(peer_id),
            "SUCCESS",
        )
        .with_before(json!({ "approval_status": current.approval_status }))
        .with_after(json!({ "approval_status": target })),
    )
    .await?;
    let updated = peer_repo::find_by_id(&mut *tx, peer_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Peer Navigator not found.".into()))?;
    tx.commit().await?;
    if let Some(outbox_id) = outbox_id {
        let state = state.clone();
        tokio::spawn(async move {
            crate::services::email_dispatch_worker::dispatch_now(&state, outbox_id).await;
        });
    }
    Ok(ok(updated))
}

// ---------------------------------------------------------------------------
// Requests (Phase 2): patient requests a slot, peer accepts or declines
// ---------------------------------------------------------------------------

/// A patient may have at most this many requests waiting at once.
pub const MAX_PENDING_PEER_REQUESTS: i64 = 3;

/// "Amina Otieno" -> "Amina O." — all a peer ever sees of the patient's name.
pub fn peer_display_name(full_name: &str) -> String {
    let mut parts = full_name.split_whitespace();
    let first: String = parts.next().unwrap_or("Patient").chars().take(30).collect();
    match parts.last().and_then(|last| last.chars().next()) {
        Some(initial) => format!("{first} {}.", initial.to_uppercase()),
        None => first,
    }
}

/// An age band rather than a birth date.
pub fn peer_age_range(
    date_of_birth: chrono::NaiveDate,
    today: chrono::NaiveDate,
) -> Option<String> {
    use chrono::Datelike;
    let mut age = today.year() - date_of_birth.year();
    if (today.month(), today.day()) < (date_of_birth.month(), date_of_birth.day()) {
        age -= 1;
    }
    let band = match age {
        a if a < 10 => return None,
        10..=14 => "10-14",
        15..=19 => "15-19",
        20..=24 => "20-24",
        25..=29 => "25-29",
        30..=39 => "30-39",
        _ => "40+",
    };
    Some(band.to_owned())
}

/// In-app notification without topic text or health detail. Fallback text
/// is used when no template row exists; English and Swahili.
async fn notify_peer_event(
    state: &AppState,
    conn: &mut sqlx::PgConnection,
    recipient: UserId,
    kind: &str,
    request_id: Uuid,
) -> Result<(), AppError> {
    let language = crate::services::NotificationService::user_language(&state.system_db, recipient)
        .await
        .unwrap_or_else(|_| "en".into());
    let sw = language == "sw";
    let (key, title, body) = match kind {
        "PEER_REQUEST_NEW" => (
            "peer_request_new",
            if sw {
                "Ombi jipya la msaada"
            } else {
                "New support request"
            },
            if sw {
                "Mtu ameomba kipindi cha msaada nawe. Kifungue ukubali au ukatae."
            } else {
                "Someone has requested a support session with you. Open it to accept or decline."
            },
        ),
        "PEER_REQUEST_ACCEPTED" => (
            "peer_request_accepted",
            if sw {
                "Ombi lako limekubaliwa"
            } else {
                "Your request was accepted"
            },
            if sw {
                "Mwelekezi rika amekubali ombi lako. Unaweza kuanza kuzungumza."
            } else {
                "Your peer navigator accepted your request. You can start chatting."
            },
        ),
        "PEER_REQUEST_DECLINED" => (
            "peer_request_declined",
            if sw {
                "Ombi lako halikukubaliwa"
            } else {
                "Your request was not accepted"
            },
            if sw {
                "Mwelekezi rika hakuweza kukubali ombi hili. Tafadhali chagua muda au mwelekezi mwingine."
            } else {
                "Your peer navigator couldn't take this request. Please choose another time or peer."
            },
        ),
        _ => (
            "peer_request_cancelled",
            if sw {
                "Ombi limeghairiwa"
            } else {
                "Request cancelled"
            },
            if sw {
                "Ombi la msaada limeghairiwa na muda wako uko wazi tena."
            } else {
                "A support request was cancelled and your slot is open again."
            },
        ),
    };
    let (title, body) = crate::services::NotificationService::render(
        &mut *conn,
        key,
        if sw { "sw" } else { "en" },
        &[],
        title,
        body,
    )
    .await?;
    crate::repository::notification_repo::insert_if_new(
        &mut *conn,
        Uuid::new_v4(),
        recipient,
        kind,
        &title,
        &body,
        Some("PEER_REQUEST"),
        Some(request_id),
        None,
        &format!("{}:{request_id}", kind.to_lowercase()),
        None,
    )
    .await?;
    Ok(())
}

#[derive(Deserialize)]
pub struct CreatePeerRequest {
    pub slot_id: Uuid,
    pub support_type: String,
    pub topic: String,
}

/// `POST /peer-requests` — the patient asks for one of a peer's open slots.
/// The slot is held (REQUESTED) until the peer accepts or declines, or the
/// patient cancels.
pub async fn create_request(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(body): Json<CreatePeerRequest>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::BookAppointment)?;
    rate_limit::check_booking_limit(&state.redis, &ctx.user_id.to_string()).await?;
    let support_type = match body.support_type.as_str() {
        "CHAT" | "CALL" => body.support_type.clone(),
        _ => {
            return Err(AppError::Validation(
                "Choose Chat or Voice/Video support.".into(),
            ))
        }
    };
    let topic = text_field(&body.topic, 3, 300, "Topic")?;
    let patient_id = ctx.patient_profile_id(&state).await?;
    let mut tx = state.begin_authed(&ctx).await?;
    let profile = crate::repository::patient_repo::find_by_id(&mut *tx, patient_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Patient profile not found.".into()))?;
    if peer_repo::pending_count_for_patient(&mut *tx, ctx.user_id).await?
        >= MAX_PENDING_PEER_REQUESTS
    {
        return Err(AppError::Conflict(format!(
            "You already have {MAX_PENDING_PEER_REQUESTS} requests waiting. Wait for a reply or cancel one first."
        )));
    }
    let taken =
        || AppError::Conflict("This time is no longer available. Please choose another.".into());
    let peer_id = peer_repo::open_slot_peer(&mut tx, body.slot_id)
        .await?
        .ok_or_else(taken)?;
    let peer_user = peer_repo::approved_peer_user(&mut *tx, peer_id)
        .await?
        .ok_or_else(|| AppError::Conflict("This Peer Navigator is not available.".into()))?;
    let id = Uuid::new_v4();
    let display = peer_display_name(&profile.full_name);
    let age = peer_age_range(profile.date_of_birth, Utc::now().date_naive());
    peer_repo::insert_request(
        &mut *tx,
        id,
        patient_id,
        ctx.user_id,
        peer_id,
        peer_user,
        body.slot_id,
        &support_type,
        &topic,
        &display,
        age.as_deref(),
    )
    .await
    .map_err(
        |e| match e.as_database_error().and_then(|d| d.code()).as_deref() {
            // Another request already holds this slot.
            Some("23505") => taken(),
            _ => e.into(),
        },
    )?;
    if !peer_repo::hold_slot(&mut tx, body.slot_id).await? {
        return Err(taken());
    }
    notify_peer_event(&state, &mut tx, peer_user, "PEER_REQUEST_NEW", id).await?;
    let created = peer_repo::patient_request(&mut *tx, ctx.user_id, id)
        .await?
        .ok_or_else(|| AppError::internal("Peer request could not be read back.".into()))?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, ok(created)))
}

/// `GET /peer-requests` — the patient's own requests, newest first.
pub async fn list_my_requests(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::PatientRead)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let requests = peer_repo::patient_requests(&mut *tx, ctx.user_id).await?;
    tx.commit().await?;
    Ok(ok(requests))
}

/// `POST /peer-requests/:id/cancel` — the patient withdraws a request the
/// peer has not answered yet; the slot opens again.
pub async fn cancel_request(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::PatientRead)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let core = peer_repo::find_request_for_update(&mut *tx, id)
        .await?
        .filter(|r| r.patient_user_id == ctx.user_id)
        .ok_or_else(|| AppError::NotFound("Request not found.".into()))?;
    if core.status != "PENDING" {
        return Err(AppError::Conflict(
            "Only a request that is still waiting can be cancelled.".into(),
        ));
    }
    peer_repo::set_request_status(&mut *tx, id, "CANCELLED", None).await?;
    peer_repo::set_slot_status(&mut *tx, core.slot_id, "REQUESTED", "AVAILABLE").await?;
    notify_peer_event(
        &state,
        &mut tx,
        core.peer_user_id,
        "PEER_REQUEST_CANCELLED",
        id,
    )
    .await?;
    let updated = peer_repo::patient_request(&mut *tx, ctx.user_id, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Request not found.".into()))?;
    tx.commit().await?;
    Ok(ok(updated))
}

#[derive(Deserialize)]
pub struct PeerInboxQuery {
    pub status: Option<String>,
}

/// `GET /peer/requests?status=PENDING` — the peer's requests, waiting first.
pub async fn list_inbox(
    State(state): State<AppState>,
    ctx: AuthContext,
    Query(q): Query<PeerInboxQuery>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::PeerSelf)?;
    let status = match q.status.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        None => None,
        Some(s @ ("PENDING" | "ACCEPTED" | "DECLINED" | "CANCELLED" | "COMPLETED")) => {
            Some(s.to_owned())
        }
        Some(_) => return Err(AppError::Validation("Unknown request status.".into())),
    };
    let mut tx = state.begin_authed(&ctx).await?;
    let requests = peer_repo::peer_inbox(&mut *tx, ctx.user_id, status.as_deref()).await?;
    tx.commit().await?;
    Ok(ok(requests))
}

#[derive(Deserialize, Default)]
pub struct PeerDecision {
    pub note: Option<String>,
}

/// `POST /peer/requests/:id/accept` or `/decline` — the peer answers a
/// waiting request. Accepting books the slot; declining frees it.
pub async fn decide_request(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path((id, action)): Path<(Uuid, String)>,
    body: Option<Json<PeerDecision>>,
) -> Result<axum::response::Response, AppError> {
    abac::authorize(&ctx, Action::PeerSelf)?;
    if action == "complete" {
        return complete_request(&state, &ctx, id)
            .await
            .map(IntoResponse::into_response);
    }
    let (status, slot_to, kind) = match action.as_str() {
        "accept" => ("ACCEPTED", "BOOKED", "PEER_REQUEST_ACCEPTED"),
        "decline" => ("DECLINED", "AVAILABLE", "PEER_REQUEST_DECLINED"),
        _ => return Err(AppError::NotFound("Unknown request action.".into())),
    };
    let note = optional_text(body.as_ref().and_then(|b| b.note.as_deref()), 300, "Note")?;
    let profile = own_profile(&state, &ctx).await?;
    if profile.approval_status != "APPROVED" {
        return Err(AppError::Forbidden(
            "Your Peer Navigator account is not active.".into(),
        ));
    }
    let mut tx = state.begin_authed(&ctx).await?;
    let core = peer_repo::find_request_for_update(&mut *tx, id)
        .await?
        .filter(|r| r.peer_user_id == ctx.user_id)
        .ok_or_else(|| AppError::NotFound("Request not found.".into()))?;
    if core.status != "PENDING" {
        return Err(AppError::Conflict(
            "This request has already been answered or cancelled.".into(),
        ));
    }
    peer_repo::set_request_status(
        &mut *tx,
        id,
        status,
        if status == "DECLINED" {
            note.as_deref()
        } else {
            None
        },
    )
    .await?;
    peer_repo::set_slot_status(&mut *tx, core.slot_id, "REQUESTED", slot_to).await?;
    if status == "ACCEPTED" {
        // The chat opens on acceptance and never locks.
        sqlx::query(
            "INSERT INTO peer_conversations (request_id, patient_user_id, peer_user_id) VALUES ($1,$2,$3)",
        )
        .bind(id)
        .bind(core.patient_user_id)
        .bind(core.peer_user_id)
        .execute(&mut *tx)
        .await?;
    }
    notify_peer_event(&state, &mut tx, core.patient_user_id, kind, id).await?;
    let updated = peer_repo::peer_request(&mut *tx, ctx.user_id, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Request not found.".into()))?;
    tx.commit().await?;
    Ok(ok(updated).into_response())
}

/// "Mark session complete": an accepted request becomes COMPLETED. The chat
/// stays open (it never closes); this records that the session took place.
async fn complete_request(
    state: &AppState,
    ctx: &AuthContext,
    id: Uuid,
) -> Result<Json<serde_json::Value>, AppError> {
    let mut tx = state.begin_authed(ctx).await?;
    let core = peer_repo::find_request_for_update(&mut *tx, id)
        .await?
        .filter(|r| r.peer_user_id == ctx.user_id)
        .ok_or_else(|| AppError::NotFound("Request not found.".into()))?;
    if core.status != "ACCEPTED" {
        return Err(AppError::Conflict(
            "Only an accepted session can be marked complete.".into(),
        ));
    }
    peer_repo::set_request_status(&mut *tx, id, "COMPLETED", None).await?;
    let updated = peer_repo::peer_request(&mut *tx, ctx.user_id, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Request not found.".into()))?;
    tx.commit().await?;
    Ok(ok(updated))
}

/// `POST /auth/care/login` — one Care-app sign-in for doctors and peer
/// navigators; the account's role decides which flow runs.
pub async fn care_login(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(payload): Json<PeerLoginRequest>,
) -> Result<impl IntoResponse, AppError> {
    let ip = addr.ip().to_string();
    let email = validation::normalize_email(&payload.email);
    let account_key = format!("care:{email}");
    if ip_jail::is_account_jailed(&state.redis, &account_key).await? {
        return Err(AppError::Forbidden(
            "Too many failed login attempts for this account. Try again later.".into(),
        ));
    }
    if ip_jail::is_ip_jailed(&state.redis, &ip).await? {
        return Err(AppError::Forbidden(
            "Too many failed login attempts from this network. Try again later.".into(),
        ));
    }
    rate_limit::check_login_attempt_limit(&state.redis, &ip).await?;
    match AuthService::care_any_login_step1(&state, &email, &payload.password).await {
        Ok((session_token, debug_code)) => {
            ip_jail::record_success(&state.redis, &account_key).await?;
            Ok(ok(json!({
                "session_token": session_token,
                "requires_otp": true,
                "debug_otp_code": debug_code
            })))
        }
        Err(e) => {
            if crate::services::auth_service::abuse::credential_failure(&e) {
                ip_jail::record_failure(&state.redis, &ip, &account_key).await?;
            }
            Err(e)
        }
    }
}

/// `POST /auth/care/verify-otp` — emailed-code step for either role.
pub async fn care_verify_otp(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    method: Method,
    OriginalUri(uri): OriginalUri,
    Json(payload): Json<PeerVerifyOtpRequest>,
) -> Result<impl IntoResponse, AppError> {
    let device_key = crate::services::auth_service::device_proof::establishment_proof(
        &state,
        &headers,
        &method,
        &uri,
        &payload.session_token,
    )
    .await?;
    let ip = addr.ip().to_string();
    let response = AuthService::care_any_verify_otp_with_device(
        &state,
        &ip,
        &payload.session_token,
        &payload.otp_code,
        device_key,
    )
    .await?;
    Ok(ok(response))
}
