use crate::crypto;
use crate::domain::ids::{DoctorProfileId, UserId};
use crate::domain::models::{
    DoctorProfile, LegalAcceptance, ProfessionalVerificationStatus, User, UserRole,
};
use crate::error::AppError;
use crate::ip_jail;
use crate::rate_limit;
use crate::repository::{doctor_repo, legal_repo, wallet_repo, AppState, DbRole};
use crate::services::AuthService;
use axum::{
    extract::{ConnectInfo, State},
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct RegisterOtpRequest {
    pub phone_number: String,
    pub role: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct VerifyOtpRequest {
    pub phone_number: String,
    pub otp_code: String,
}

#[derive(Debug, Deserialize)]
pub struct DoctorLoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct DoctorVerifyOtpRequest {
    pub session_token: String,
    pub otp_code: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DoctorRegistrationRequest {
    pub full_name: String,
    pub license_number: String,
    pub phone_number: String,
    pub email: String,
    pub password: String,
    pub primary_specialty: String,
    pub experience_years: i16,
    pub current_hospital: String,
    pub qualifications: Vec<String>,
    pub residential_address: String,
    /// Every currently-required legal document (Doctor Terms, Privacy
    /// Notice, Professional Verification Declaration, Telemedicine
    /// Practitioner Policy) plus, optionally, Marketing Communications. See
    /// docs/ADR/ADR-011-legal-consent-ledger.md.
    pub accepted_documents: Vec<AcceptedDocument>,
}

#[derive(Debug, Deserialize)]
pub struct AcceptedDocument {
    #[serde(rename = "type")]
    pub document_type: String,
    pub version: String,
}

#[derive(Debug, Deserialize)]
pub struct AdminLoginRequest {
    pub email: String,
    pub password: String,
    pub totp_code: String,
}

#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: T,
}

pub async fn register_otp(
    State(state): State<AppState>,
    Json(payload): Json<RegisterOtpRequest>,
) -> Result<impl IntoResponse, AppError> {
    let _ = payload.role;
    rate_limit::check_otp_request_limit(&state.redis, &payload.phone_number).await?;

    let (session_token, expires_in, debug_otp_code) =
        AuthService::request_patient_otp(&state, &payload.phone_number).await?;
    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "session_token": session_token,
            "expires_in": expires_in,
            "debug_otp_code": debug_otp_code
        }),
    }))
}

pub async fn verify_otp_and_login(
    State(state): State<AppState>,
    Json(payload): Json<VerifyOtpRequest>,
) -> Result<impl IntoResponse, AppError> {
    let login_resp =
        AuthService::verify_otp_and_login(&state, &payload.phone_number, &payload.otp_code).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: login_resp,
    }))
}

/// Public Doctor onboarding. This deliberately creates an unverified account;
/// ClinicalAdmin credentialing remains the only path to verification.
pub async fn doctor_register(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(payload): Json<DoctorRegistrationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let ip = addr.ip().to_string();
    rate_limit::check_and_increment(
        &state.redis,
        &format!("ratelimit:doctor-registration:{ip}"),
        5,
        900,
    )
    .await?;

    let phone = crate::domain::validation::normalize_kenya_phone_number(&payload.phone_number)?;
    crate::domain::validation::validate_license_number(&payload.license_number)?;
    if payload.full_name.trim().is_empty() || payload.full_name.chars().count() > 120 {
        return Err(AppError::Validation(
            "Full name must contain 1-120 characters.".into(),
        ));
    }
    // Previously a separate hardcoded 12-128 character check ran here before
    // `validate_password_strength`, meaning a 12-char minimum silently won
    // over the 8-char-plus-complexity policy below (12 > 8, so the stricter
    // literal always bound first) — the opposite of the explicitly requested
    // "8 characters with a proper combination" policy. Removed in favor of
    // `validate_password_strength` alone, which is now the single source of
    // truth for this rule (and deliberately has no maximum-length cap, per
    // the same no-max-cap convention already used elsewhere in this codebase).
    crate::domain::validation::validate_password_strength(&payload.password)?;
    if payload.primary_specialty.trim().is_empty() || payload.primary_specialty.chars().count() > 80
    {
        return Err(AppError::Validation(
            "Primary specialty must contain 1-80 characters.".into(),
        ));
    }
    if !(0..=100).contains(&payload.experience_years) {
        return Err(AppError::Validation(
            "Experience years must be between 0 and 100.".into(),
        ));
    }
    if payload.current_hospital.trim().is_empty() || payload.current_hospital.chars().count() > 200
    {
        return Err(AppError::Validation(
            "Hospital must contain 1-200 characters.".into(),
        ));
    }
    if payload.residential_address.trim().is_empty()
        || payload.residential_address.chars().count() > 500
    {
        return Err(AppError::Validation(
            "Residential address must contain 1-500 characters.".into(),
        ));
    }
    if payload.qualifications.is_empty()
        || payload.qualifications.len() > 10
        || payload
            .qualifications
            .iter()
            .any(|q| q.trim().is_empty() || q.chars().count() > 150)
    {
        return Err(AppError::Validation(
            "Qualifications must contain 1-10 entries of 1-150 characters.".into(),
        ));
    }
    let accepted_pairs: Vec<(String, String)> = payload
        .accepted_documents
        .iter()
        .map(|d| (d.document_type.clone(), d.version.clone()))
        .collect();
    crate::domain::validation::validate_accepted_documents(&accepted_pairs)?;

    crate::domain::validation::validate_email(&payload.email)?;

    let license = payload.license_number.trim().to_string();
    let email = payload.email.trim().to_lowercase();

    let duplicate_license = doctor_repo::find_by_license(&state.auth_db, &license)
        .await?
        .is_some();
    let duplicate_phone = crate::repository::user_repo::find_by_phone(&state.auth_db, &phone)
        .await?
        .is_some();
    let duplicate_email = crate::repository::user_repo::find_by_email(&state.auth_db, &email)
        .await?
        .is_some();
    if duplicate_license || duplicate_phone || duplicate_email {
        return Err(AppError::Conflict(
            "Registration details are already in use.".into(),
        ));
    }

    let active_count = crate::repository::user_repo::count_active(&state.auth_db).await?;
    if active_count as u32 >= state.config.max_active_users {
        return Err(AppError::CapacityLimitReached(
            "HelloDoctor is at its current controlled pilot capacity.".into(),
        ));
    }

    // Server-independent check: every CURRENTLY required document must be
    // present with its current version. The client supplies only a type and
    // version — never a hash — and cannot bypass this by calling the API
    // directly, since this is the only path `doctor_register` is reachable
    // through.
    let current_documents = legal_repo::list_current(&state.auth_db).await?;
    let missing_required = current_documents.iter().any(|doc| {
        doc.required
            && !payload
                .accepted_documents
                .iter()
                .any(|a| a.document_type == doc.document_type.as_str() && a.version == doc.version)
    });
    let any_stale_or_unknown = payload.accepted_documents.iter().any(|a| {
        !current_documents
            .iter()
            .any(|doc| a.document_type == doc.document_type.as_str() && a.version == doc.version)
    });
    if missing_required || any_stale_or_unknown {
        return Err(AppError::Validation(
            "All current required legal and professional agreements must be accepted, and only the current version of any document may be accepted.".into(),
        ));
    }

    let user_id = UserId::new();
    let doctor_id = DoctorProfileId::new();
    let now = Utc::now();
    let password_hash = crypto::hash_password_async(payload.password.clone()).await?;
    let user = User {
        id: user_id,
        phone_number: phone.clone(),
        email: Some(email.clone()),
        password_hash: Some(password_hash),
        role: UserRole::Doctor,
        preferred_language: "en".into(),
        is_active: true,
        is_verified: false,
        created_at: now,
        updated_at: now,
        deleted_at: None,
    };

    let mut auth_tx = state
        .begin_authed_as(
            &crate::auth_context::AuthContext {
                user_id,
                role: UserRole::Doctor,
                permissions: vec![],
                session_id: None,
                request_id: None,
            },
            DbRole::AuthWorker,
        )
        .await?;
    crate::repository::user_repo::insert_user(&mut *auth_tx, &user).await?;

    let doctor = DoctorProfile {
        id: doctor_id,
        user_id,
        full_name: payload.full_name.trim().to_string(),
        license_number: license,
        license_authority: "KMPDC".into(),
        license_country: "KE".into(),
        primary_specialty: payload.primary_specialty.trim().to_string(),
        experience_years: payload.experience_years,
        current_hospital: payload.current_hospital.trim().to_string(),
        qualifications: payload.qualifications,
        consultation_fee_video: bigdecimal::BigDecimal::from(0),
        consultation_fee_chat: bigdecimal::BigDecimal::from(0),
        residential_address: payload.residential_address.trim().to_string(),
        verified_phone: phone,
        is_on_duty: false,
        is_verified: false,
        professional_verification_status: ProfessionalVerificationStatus::Pending,
        professional_verification_reviewed_at: None,
        professional_verification_reviewed_by: None,
        professional_verification_note: None,
        created_at: now,
        updated_at: now,
        deleted_at: None,
    };
    doctor_repo::insert(&mut *auth_tx, &doctor).await?;
    wallet_repo::insert_default(&mut *auth_tx, doctor_id).await?;
    // One immutable row per document the Doctor actually submitted (every
    // required one, plus MARKETING_COMMUNICATIONS only if included) — in the
    // same transaction as the account itself, so the acknowledgements and
    // the account land atomically or not at all. `document_hash` always
    // comes from the server's own current-documents lookup above, never from
    // the request, and the `guard_legal_acceptance_current` trigger
    // recomputes it again from `legal_documents` regardless.
    for doc in current_documents.iter().filter(|doc| {
        payload
            .accepted_documents
            .iter()
            .any(|a| a.document_type == doc.document_type.as_str() && a.version == doc.version)
    }) {
        let acceptance = LegalAcceptance {
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
        };
        legal_repo::insert(&mut *auth_tx, &acceptance).await?;
    }
    auth_tx.commit().await?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(ApiResponse {
            success: true,
            data: serde_json::json!({
                "doctor_id": doctor_id,
                "status": "PENDING_VERIFICATION",
                "professional_verification_status": "PENDING",
                "license_authority": "KMPDC",
                "license_country": "KE",
                "message": "Registration submitted for verification."
            }),
        }),
    ))
}

/// Doctor and admin password-based logins are the targets of brute-force
/// attack. Both check two independent abuse dimensions before the real
/// credential check runs: is this *account* (license number / email) locked
/// out from its own recent failures, and is this *IP* showing a
/// credential-stuffing-shaped pattern (failing many distinct accounts)? See
/// `ip_jail`'s module doc for why this replaces a pure per-IP jail — carrier-
/// grade NAT in Kenya means many unrelated real users can share
/// one address, so an IP-only jail could lock out everyone behind it over one
/// attacker's (or one confused legitimate user's) failures. Both checks fail
/// closed: an abuse-control-store outage blocks login rather than silently
/// disabling brute-force protection.
pub async fn doctor_login(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(payload): Json<DoctorLoginRequest>,
) -> Result<impl IntoResponse, AppError> {
    let ip = addr.ip().to_string();
    let normalized_email = payload.email.trim().to_lowercase();
    let account_key = format!("doctor:{normalized_email}");

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

    match AuthService::doctor_login_step1(&state, &normalized_email, &payload.password).await {
        Ok((session_token, debug_code)) => {
            ip_jail::record_success(&state.redis, &account_key).await?;
            Ok(Json(ApiResponse {
                success: true,
                data: serde_json::json!({
                    "session_token": session_token,
                    "requires_otp": true,
                    "debug_otp_code": debug_code
                }),
            }))
        }
        Err(e) => {
            ip_jail::record_failure(&state.redis, &ip, &account_key).await?;
            Err(e)
        }
    }
}

pub async fn doctor_verify_otp(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(payload): Json<DoctorVerifyOtpRequest>,
) -> Result<impl IntoResponse, AppError> {
    let ip = addr.ip().to_string();
    let login_resp =
        AuthService::doctor_verify_otp(&state, &ip, &payload.session_token, &payload.otp_code)
            .await?;

    Ok(Json(ApiResponse {
        success: true,
        data: login_resp,
    }))
}

pub async fn admin_login(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(payload): Json<AdminLoginRequest>,
) -> Result<impl IntoResponse, AppError> {
    let ip = addr.ip().to_string();
    let account_key = format!("admin:{}", payload.email);

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

    match AuthService::admin_login(
        &state,
        &payload.email,
        &payload.password,
        &payload.totp_code,
    )
    .await
    {
        Ok(login_resp) => {
            ip_jail::record_success(&state.redis, &account_key).await?;
            Ok(Json(ApiResponse {
                success: true,
                data: login_resp,
            }))
        }
        Err(e) => {
            ip_jail::record_failure(&state.redis, &ip, &account_key).await?;
            Err(e)
        }
    }
}

pub async fn refresh_token(
    State(state): State<AppState>,
    Json(payload): Json<RefreshRequest>,
) -> Result<impl IntoResponse, AppError> {
    let (access_token, new_refresh_token) =
        AuthService::refresh_session(&state, &payload.refresh_token).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "access_token": access_token,
            "refresh_token": new_refresh_token
        }),
    }))
}

/// Revokes the presented refresh-token family. This route is public like
/// refresh because the browser may no longer have a usable access token.
pub async fn logout(
    State(state): State<AppState>,
    Json(payload): Json<RefreshRequest>,
) -> Result<impl IntoResponse, AppError> {
    AuthService::logout(&state, &payload.refresh_token).await?;
    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({ "logged_out": true }),
    }))
}
