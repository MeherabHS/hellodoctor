use crate::auth_context::AuthContext;
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
    extract::{ConnectInfo, OriginalUri, State},
    http::{HeaderMap, Method},
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

#[derive(Deserialize)]
pub struct PatientRegistrationRequest {
    #[serde(flatten)]
    pub profile: crate::repository::patient_auth_repo::PatientRegistrationProfile,
    pub email: String,
    pub phone_number: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct PatientLoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct PatientVerifyOtpRequest {
    pub challenge_token: String,
    pub otp_code: String,
    #[serde(default)]
    pub accepted_documents: Vec<AcceptedDocument>,
}

#[derive(Deserialize)]
pub struct VerifyOtpRequest {
    pub phone_number: String,
    pub otp_code: String,
    #[serde(default)]
    pub accepted_documents: Vec<AcceptedDocument>,
}

#[derive(Deserialize)]
pub struct DoctorLoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct DoctorVerifyOtpRequest {
    pub session_token: String,
    pub otp_code: String,
}

#[derive(Deserialize)]
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

#[derive(Deserialize)]
pub struct AdminLoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct AdminVerifyOtpRequest {
    pub session_token: String,
    pub otp_code: String,
}

/// Phase 2: Secure Email OTP Password Recovery (Patient/Doctor only).
#[derive(Deserialize)]
pub struct PasswordResetRequestRequest {
    pub email: String,
}

#[derive(Deserialize)]
pub struct PasswordResetVerifyRequest {
    pub email: String,
    pub otp_code: String,
}

#[derive(Deserialize)]
pub struct PasswordResetCompleteRequest {
    pub reset_authorization: String,
    pub new_password: String,
}

/// Remediation E4 (Phase 1 finding #3): these request DTOs previously
/// derived `Debug` over a plaintext `password`, `otp_code`, or bearer-style
/// `challenge_token`/`session_token`/`refresh_token` field —
/// any stray `tracing::debug!(?payload, ...)`, panic message, or assertion
/// failure formatting one of them with `{:?}` would print the real secret.
/// Each impl below lists every field explicitly and redacts only the
/// sensitive one(s), mirroring the hand-written `AppConfig` Debug impl from
/// Remediation E3 (`config.rs`) rather than `derive(Debug)`'s default of
/// printing a new field's raw value. See `debug_redaction_tests` for the
/// regression tests against synthetic secrets.
fn redacted(value: &str) -> &'static str {
    if value.is_empty() {
        "<empty>"
    } else {
        "<redacted>"
    }
}

impl std::fmt::Debug for PatientRegistrationRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PatientRegistrationRequest")
            .field("profile", &self.profile)
            .field("email", &self.email)
            .field("phone_number", &self.phone_number)
            .field("password", &redacted(&self.password))
            .finish()
    }
}

impl std::fmt::Debug for PatientLoginRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PatientLoginRequest")
            .field("email", &self.email)
            .field("password", &redacted(&self.password))
            .finish()
    }
}

impl std::fmt::Debug for PatientVerifyOtpRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PatientVerifyOtpRequest")
            .field("challenge_token", &redacted(&self.challenge_token))
            .field("otp_code", &redacted(&self.otp_code))
            .field("accepted_documents", &self.accepted_documents)
            .finish()
    }
}

impl std::fmt::Debug for VerifyOtpRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VerifyOtpRequest")
            .field("phone_number", &self.phone_number)
            .field("otp_code", &redacted(&self.otp_code))
            .field("accepted_documents", &self.accepted_documents)
            .finish()
    }
}

impl std::fmt::Debug for DoctorLoginRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DoctorLoginRequest")
            .field("email", &self.email)
            .field("password", &redacted(&self.password))
            .finish()
    }
}

impl std::fmt::Debug for DoctorVerifyOtpRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DoctorVerifyOtpRequest")
            .field("session_token", &redacted(&self.session_token))
            .field("otp_code", &redacted(&self.otp_code))
            .finish()
    }
}

impl std::fmt::Debug for DoctorRegistrationRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DoctorRegistrationRequest")
            .field("full_name", &self.full_name)
            .field("license_number", &self.license_number)
            .field("phone_number", &self.phone_number)
            .field("email", &self.email)
            .field("password", &redacted(&self.password))
            .field("primary_specialty", &self.primary_specialty)
            .field("experience_years", &self.experience_years)
            .field("current_hospital", &self.current_hospital)
            .field("qualifications", &self.qualifications)
            .field("residential_address", &self.residential_address)
            .field("accepted_documents", &self.accepted_documents)
            .finish()
    }
}

impl std::fmt::Debug for AdminLoginRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AdminLoginRequest")
            .field("email", &self.email)
            .field("password", &redacted(&self.password))
            .finish()
    }
}

impl std::fmt::Debug for AdminVerifyOtpRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AdminVerifyOtpRequest")
            .field("session_token", &redacted(&self.session_token))
            .field("otp_code", &redacted(&self.otp_code))
            .finish()
    }
}

impl std::fmt::Debug for PasswordResetVerifyRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PasswordResetVerifyRequest")
            .field("email", &self.email)
            .field("otp_code", &redacted(&self.otp_code))
            .finish()
    }
}

impl std::fmt::Debug for PasswordResetCompleteRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PasswordResetCompleteRequest")
            .field("reset_authorization", &redacted(&self.reset_authorization))
            .field("new_password", &redacted(&self.new_password))
            .finish()
    }
}

#[derive(Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

impl std::fmt::Debug for RefreshRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RefreshRequest")
            .field("refresh_token", &redacted(&self.refresh_token))
            .finish()
    }
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
    headers: HeaderMap,
    method: Method,
    OriginalUri(uri): OriginalUri,
    Json(payload): Json<VerifyOtpRequest>,
) -> Result<impl IntoResponse, AppError> {
    let device_key = crate::services::auth_service::device_proof::establishment_proof(
        &state,
        &headers,
        &method,
        &uri,
        &payload.phone_number,
    )
    .await?;
    let login_resp = AuthService::verify_otp_and_login_with_device(
        &state,
        &payload.phone_number,
        &payload.otp_code,
        &payload.accepted_documents,
        device_key,
    )
    .await?;

    Ok(Json(ApiResponse {
        success: true,
        data: login_resp,
    }))
}

pub async fn patient_register(
    State(state): State<AppState>,
    Json(payload): Json<PatientRegistrationRequest>,
) -> Result<impl IntoResponse, AppError> {
    rate_limit::check_and_increment(
        &state.redis,
        &format!(
            "ratelimit:patient-registration:{}",
            payload.email.trim().to_lowercase()
        ),
        5,
        900,
    )
    .await?;
    let (token, expires_in, debug) = AuthService::request_patient_email_otp(
        &state,
        &payload.email,
        &payload.password,
        Some(&crate::domain::validation::normalize_kenya_phone_number(
            &payload.phone_number,
        )?),
        true,
        Some(&payload.profile),
    )
    .await?;
    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({"challenge_token": token, "expires_in": expires_in, "debug_otp_code": debug}),
    }))
}

/// Patient password-login step. Mirrors `doctor_login`/`admin_login`'s
/// two-dimensional `ip_jail` abuse control (see that function's doc comment)
/// — previously this endpoint had only the per-IP `rate_limit` counter, so an
/// attacker rotating across more than a handful of source IPs could make
/// unlimited password guesses against one named patient account. The
/// mandatory emailed-OTP second factor (`patient_verify_otp`) still gates
/// actual session issuance; this closes the account-level guessing gap on
/// the password step itself.
pub async fn patient_login(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(payload): Json<PatientLoginRequest>,
) -> Result<impl IntoResponse, AppError> {
    let ip = addr.ip().to_string();
    let normalized_email = payload.email.trim().to_lowercase();
    let account_key = format!("patient:{normalized_email}");

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

    match AuthService::request_patient_email_otp(
        &state,
        &payload.email,
        &payload.password,
        None,
        false,
        None,
    )
    .await
    {
        Ok((token, expires_in, debug)) => {
            ip_jail::record_success(&state.redis, &account_key).await?;
            Ok(Json(ApiResponse {
                success: true,
                data: serde_json::json!({"challenge_token": token, "expires_in": expires_in, "debug_otp_code": debug}),
            }))
        }
        Err(e) => {
            if crate::services::auth_service::abuse::credential_failure(&e) {
                ip_jail::record_failure(&state.redis, &ip, &account_key).await?;
            }
            Err(e)
        }
    }
}

pub async fn patient_verify_otp(
    State(state): State<AppState>,
    headers: HeaderMap,
    method: Method,
    OriginalUri(uri): OriginalUri,
    Json(payload): Json<PatientVerifyOtpRequest>,
) -> Result<impl IntoResponse, AppError> {
    let device_key = crate::services::auth_service::device_proof::establishment_proof(
        &state,
        &headers,
        &method,
        &uri,
        &payload.challenge_token,
    )
    .await?;
    let login_resp = AuthService::verify_patient_email_otp_with_device(
        &state,
        &payload.challenge_token,
        &payload.otp_code,
        &payload.accepted_documents,
        device_key,
    )
    .await?;
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
            "AfyaMkononi is at its current controlled pilot capacity.".into(),
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
    // Phase 1B: atomic creation of the email intent alongside the business
    // event, on this same transaction — see
    // `email_outbox_repo::enqueue_transactional`'s doc comment. Content is
    // rendered later, at dispatch time, from the doctor_profiles row just
    // inserted above.
    let application_received_outbox_id = Uuid::new_v4();
    let dedup_key = format!("doctor-application-received:{doctor_id}");
    crate::repository::email_outbox_repo::enqueue_transactional(
        &mut *auth_tx,
        application_received_outbox_id,
        "DOCTOR_APPLICATION_RECEIVED",
        &email,
        &dedup_key,
        "DOCTOR_PROFILE",
        doctor_id.into_inner(),
        crate::repository::email_outbox_repo::transactional_email_expiry(),
    )
    .await?;
    auth_tx.commit().await?;

    // Fire-and-forget, after commit (never awaited — must not delay this
    // already-committed registration's response). The durable fallback is
    // the periodic scheduler pass.
    {
        let state = state.clone();
        tokio::spawn(async move {
            crate::services::email_dispatch_worker::dispatch_now(
                &state,
                application_received_outbox_id,
            )
            .await;
        });
    }

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
            if crate::services::auth_service::abuse::credential_failure(&e) {
                ip_jail::record_failure(&state.redis, &ip, &account_key).await?;
            }
            Err(e)
        }
    }
}

pub async fn doctor_verify_otp(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    method: Method,
    OriginalUri(uri): OriginalUri,
    Json(payload): Json<DoctorVerifyOtpRequest>,
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
    let login_resp = AuthService::doctor_verify_otp_with_device(
        &state,
        &ip,
        &payload.session_token,
        &payload.otp_code,
        device_key,
    )
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
    let normalized_email = payload.email.trim().to_lowercase();
    let account_key = format!("admin:{normalized_email}");

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

    match AuthService::admin_login_step1(&state, &normalized_email, &payload.password).await {
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
            if crate::services::auth_service::abuse::credential_failure(&e) {
                ip_jail::record_failure(&state.redis, &ip, &account_key).await?;
            }
            Err(e)
        }
    }
}

pub async fn admin_verify_otp(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(payload): Json<AdminVerifyOtpRequest>,
) -> Result<impl IntoResponse, AppError> {
    let ip = addr.ip().to_string();
    let login_resp =
        AuthService::admin_verify_otp(&state, &ip, &payload.session_token, &payload.otp_code)
            .await?;

    Ok(Json(ApiResponse {
        success: true,
        data: login_resp,
    }))
}

pub async fn refresh_token(
    State(state): State<AppState>,
    headers: HeaderMap,
    method: Method,
    OriginalUri(uri): OriginalUri,
    Json(payload): Json<RefreshRequest>,
) -> Result<impl IntoResponse, AppError> {
    let (access_token, new_refresh_token) = AuthService::refresh_session_with_proof(
        &state,
        &payload.refresh_token,
        Some((&headers, &method, &uri)),
    )
    .await?;

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
#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

/// Authenticated password change — distinct from, and unrelated to, the
/// still-disabled forgot-password/reset flow (`PASSWORD_RESET_ENABLED`
/// fails closed at startup; see `config.rs`). This requires knowing the
/// *current* password, so none of that flow's unresolved security
/// questions (anonymous token delivery, account enumeration on a
/// forgot-password request) apply here — the caller already holds a valid
/// session for this exact account.
///
/// On success: every *other* active session for this account is revoked
/// (`session_repo::revoke_all_other_sessions_for_user`) — the account-wide
/// "sign out everywhere" primitive Phase 0's audit flagged as missing,
/// built specifically for this. The session making this request is
/// deliberately kept alive (its caller has just proven they know the new
/// password — there is nothing to protect against by also logging them
/// out). A confirmation email is sent, fire-and-forget, after commit.
pub async fn change_password(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(payload): Json<ChangePasswordRequest>,
) -> Result<impl IntoResponse, AppError> {
    rate_limit::check_and_increment(
        &state.redis,
        &format!("ratelimit:change-password:{}", ctx.user_id),
        5,
        900,
    )
    .await?;

    let mut tx = state.begin_authed_as(&ctx, DbRole::AuthWorker).await?;
    let user = crate::repository::user_repo::find_by_id(&mut *tx, ctx.user_id)
        .await?
        .ok_or_else(|| AppError::internal("Authenticated user record is missing.".into()))?;
    let Some(current_hash) = user.password_hash.clone() else {
        return Err(AppError::Conflict(
            "Password changes are not available for this account.".into(),
        ));
    };
    if !crypto::verify_password_async(payload.current_password.clone(), current_hash).await? {
        return Err(AppError::Unauthorized(
            "The current password is incorrect.".into(),
        ));
    }
    crate::domain::validation::validate_password_strength(&payload.new_password)?;
    if payload.new_password == payload.current_password {
        return Err(AppError::Validation(
            "The new password must be different from the current password.".into(),
        ));
    }

    let new_hash = crypto::hash_password_async(payload.new_password.clone()).await?;
    crate::repository::user_repo::update_password_hash(&mut *tx, ctx.user_id, &new_hash).await?;
    let session_id = ctx.session_id.ok_or_else(|| {
        AppError::internal("Authenticated request is missing a session id.".into())
    })?;
    let revoked = crate::repository::session_repo::revoke_all_other_sessions_for_user(
        &mut *tx,
        ctx.user_id,
        session_id,
    )
    .await?;
    crate::services::AuditService::log_in_tx(
        &mut *tx,
        crate::services::AuditLogEntry::new(
            Some(ctx.user_id),
            ctx.role,
            "PASSWORD_CHANGED",
            "USER",
            Some(ctx.user_id.into_inner()),
            "SUCCESS",
        )
        .with_after(serde_json::json!({ "other_sessions_revoked": revoked })),
    )
    .await?;
    tx.commit().await?;

    // Fire-and-forget, after commit — see
    // email_delivery::send_templated_email's module doc.
    if let Some(email) = user.email.clone() {
        let display_name = resolve_display_name_for_email(&state, &user).await;
        let rendered = crate::services::email_templates::render_password_changed(
            &display_name,
            &state.email_assets,
        );
        // A fresh id per call, not a stable one derived from the event: every
        // distinct password change must get its own email, never deduped
        // against a previous change to the same account.
        let dedup_key = format!("password-changed:{}", Uuid::new_v4());
        crate::email_delivery::send_templated_email(
            &state,
            &email,
            &rendered,
            crate::email_delivery::TransactionalEmailKind::PasswordChanged,
            &dedup_key,
        )
        .await;
    }

    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({ "password_changed": true, "other_sessions_revoked": revoked }),
    }))
}

/// Best-effort display name for the password-changed email, role-aware
/// since `users` itself carries no name at all. Never fails the request —
/// an unresolved name just falls back to a generic greeting
/// (`email_templates::render_password_changed`'s own "there" fallback).
async fn resolve_display_name_for_email(
    state: &AppState,
    user: &crate::domain::models::User,
) -> String {
    let full_name = match user.role {
        UserRole::Doctor => crate::repository::doctor_repo::find_by_user_id(&state.db, user.id)
            .await
            .ok()
            .flatten()
            .map(|d| d.full_name),
        UserRole::Patient => crate::repository::patient_repo::find_by_user_id(&state.db, user.id)
            .await
            .ok()
            .flatten()
            .map(|p| p.full_name),
        _ => None,
    };
    full_name
        .and_then(|name| name.split_whitespace().next().map(str::to_owned))
        .unwrap_or_default()
}

/// Phase 2 step 1 (public, unauthenticated): always returns the same
/// generic success message regardless of whether the submitted email
/// corresponds to an eligible (Patient/Doctor, active, password-bearing,
/// non-Admin) account — see `PasswordResetService::request_reset`'s doc
/// comment for the anti-enumeration rationale. Only a malformed email or a
/// rate-limit/feature-availability rejection produces a different response.
pub async fn request_password_reset(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(payload): Json<PasswordResetRequestRequest>,
) -> Result<impl IntoResponse, AppError> {
    let ip = addr.ip().to_string();
    rate_limit::check_login_attempt_limit(&state.redis, &ip).await?;
    if ip_jail::is_ip_jailed(&state.redis, &ip).await? {
        return Err(AppError::Forbidden(
            "Too many failed attempts from this network. Try again later.".into(),
        ));
    }

    let debug_otp_code =
        crate::services::PasswordResetService::request_reset(&state, &payload.email).await?;
    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "message": "If an account exists for this email, a verification code has been sent.",
            "debug_otp_code": debug_otp_code
        }),
    }))
}

/// Phase 2 step 2 (public, unauthenticated): verifies the emailed OTP code
/// and, on success, returns a fresh `reset_authorization` value — the only
/// credential `complete_password_reset` accepts. The OTP code itself can
/// never be presented again after this call succeeds (or after it is
/// consumed by too many wrong attempts).
pub async fn verify_password_reset_otp(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(payload): Json<PasswordResetVerifyRequest>,
) -> Result<impl IntoResponse, AppError> {
    let ip = addr.ip().to_string();
    let (reset_authorization, expires_in) = crate::services::PasswordResetService::verify_otp(
        &state,
        &ip,
        &payload.email,
        &payload.otp_code,
    )
    .await?;
    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "reset_authorization": reset_authorization,
            "expires_in": expires_in,
        }),
    }))
}

/// Phase 2 step 3 (public, unauthenticated): the only endpoint that
/// actually changes the password. Requires the `reset_authorization` minted
/// by `verify_password_reset_otp` — a bearer secret bound to exactly one
/// verified reset flow, consumed (deleted) on use.
pub async fn complete_password_reset(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(payload): Json<PasswordResetCompleteRequest>,
) -> Result<impl IntoResponse, AppError> {
    let ip = addr.ip().to_string();
    rate_limit::check_login_attempt_limit(&state.redis, &ip).await?;
    crate::services::PasswordResetService::complete_reset(
        &state,
        &payload.reset_authorization,
        &payload.new_password,
    )
    .await?;
    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "message": "Password updated. Please sign in with your new password."
        }),
    }))
}

pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
    method: Method,
    OriginalUri(uri): OriginalUri,
    Json(payload): Json<RefreshRequest>,
) -> Result<impl IntoResponse, AppError> {
    AuthService::logout_with_proof(
        &state,
        &payload.refresh_token,
        Some((&headers, &method, &uri)),
    )
    .await?;
    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({ "logged_out": true }),
    }))
}

#[cfg(test)]
mod remediation_e4_debug_redaction_tests {
    use super::*;

    const SYNTHETIC_PASSWORD: &str = "SynthSecretPW_7f2c9a";
    const SYNTHETIC_OTP: &str = "913627";
    const SYNTHETIC_TOKEN: &str = "synth-bearer-token-ab12cd34";

    fn assert_redacted(debug_output: &str, secret: &str) {
        assert!(
            !debug_output.contains(secret),
            "Debug output leaked a synthetic secret: {debug_output}"
        );
        assert!(
            debug_output.contains("<redacted>") || debug_output.contains("<empty>"),
            "Debug output for a secret-bearing struct should show a redaction marker: {debug_output}"
        );
    }

    #[test]
    fn patient_registration_request_redacts_password() {
        let req = PatientRegistrationRequest {
            profile: crate::repository::patient_auth_repo::PatientRegistrationProfile {
                full_name: "Jane Doe".into(),
                date_of_birth: chrono::NaiveDate::from_ymd_opt(1990, 1, 1).unwrap(),
                gender: crate::domain::models::Gender::Female,
            },
            email: "jane@example.com".into(),
            phone_number: "+254700000000".into(),
            password: SYNTHETIC_PASSWORD.into(),
        };
        assert_redacted(&format!("{req:?}"), SYNTHETIC_PASSWORD);
    }

    #[test]
    fn patient_login_request_redacts_password() {
        let req = PatientLoginRequest {
            email: "jane@example.com".into(),
            password: SYNTHETIC_PASSWORD.into(),
        };
        assert_redacted(&format!("{req:?}"), SYNTHETIC_PASSWORD);
    }

    #[test]
    fn patient_verify_otp_request_redacts_token_and_code() {
        let req = PatientVerifyOtpRequest {
            challenge_token: SYNTHETIC_TOKEN.into(),
            otp_code: SYNTHETIC_OTP.into(),
            accepted_documents: vec![],
        };
        let out = format!("{req:?}");
        assert_redacted(&out, SYNTHETIC_TOKEN);
        assert_redacted(&out, SYNTHETIC_OTP);
    }

    #[test]
    fn verify_otp_request_redacts_code() {
        let req = VerifyOtpRequest {
            phone_number: "+254700000000".into(),
            otp_code: SYNTHETIC_OTP.into(),
            accepted_documents: vec![],
        };
        assert_redacted(&format!("{req:?}"), SYNTHETIC_OTP);
    }

    #[test]
    fn doctor_login_request_redacts_password() {
        let req = DoctorLoginRequest {
            email: "doc@example.com".into(),
            password: SYNTHETIC_PASSWORD.into(),
        };
        assert_redacted(&format!("{req:?}"), SYNTHETIC_PASSWORD);
    }

    #[test]
    fn doctor_verify_otp_request_redacts_token_and_code() {
        let req = DoctorVerifyOtpRequest {
            session_token: SYNTHETIC_TOKEN.into(),
            otp_code: SYNTHETIC_OTP.into(),
        };
        let out = format!("{req:?}");
        assert_redacted(&out, SYNTHETIC_TOKEN);
        assert_redacted(&out, SYNTHETIC_OTP);
    }

    #[test]
    fn doctor_registration_request_redacts_password() {
        let req = DoctorRegistrationRequest {
            full_name: "Dr Jane".into(),
            license_number: "LIC-1".into(),
            phone_number: "+254700000000".into(),
            email: "doc@example.com".into(),
            password: SYNTHETIC_PASSWORD.into(),
            primary_specialty: "General".into(),
            experience_years: 5,
            current_hospital: "Test Hospital".into(),
            qualifications: vec!["MBChB".into()],
            residential_address: "Nairobi".into(),
            accepted_documents: vec![],
        };
        assert_redacted(&format!("{req:?}"), SYNTHETIC_PASSWORD);
    }

    #[test]
    fn admin_login_request_redacts_password() {
        let req = AdminLoginRequest {
            email: "admin@example.com".into(),
            password: SYNTHETIC_PASSWORD.into(),
        };
        assert_redacted(&format!("{req:?}"), SYNTHETIC_PASSWORD);
    }

    #[test]
    fn admin_verify_otp_request_redacts_token_and_code() {
        let req = AdminVerifyOtpRequest {
            session_token: SYNTHETIC_TOKEN.into(),
            otp_code: SYNTHETIC_OTP.into(),
        };
        let out = format!("{req:?}");
        assert_redacted(&out, SYNTHETIC_TOKEN);
        assert_redacted(&out, SYNTHETIC_OTP);
    }

    #[test]
    fn password_reset_verify_request_redacts_otp_code_only() {
        let req = PasswordResetVerifyRequest {
            email: "jane@example.com".into(),
            otp_code: SYNTHETIC_OTP.into(),
        };
        let out = format!("{req:?}");
        assert_redacted(&out, SYNTHETIC_OTP);
        assert!(out.contains("jane@example.com"));
    }

    #[test]
    fn password_reset_complete_request_redacts_authorization_and_password() {
        let req = PasswordResetCompleteRequest {
            reset_authorization: SYNTHETIC_TOKEN.into(),
            new_password: SYNTHETIC_PASSWORD.into(),
        };
        let out = format!("{req:?}");
        assert_redacted(&out, SYNTHETIC_TOKEN);
        assert_redacted(&out, SYNTHETIC_PASSWORD);
    }

    #[test]
    fn refresh_request_redacts_token() {
        let req = RefreshRequest {
            refresh_token: SYNTHETIC_TOKEN.into(),
        };
        assert_redacted(&format!("{req:?}"), SYNTHETIC_TOKEN);
    }
}
