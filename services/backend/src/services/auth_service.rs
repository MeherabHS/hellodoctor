use crate::crypto;
use crate::domain::models::*;
use crate::domain::validation;
use crate::error::AppError;
use crate::repository::{doctor_repo, otp_repo, patient_repo, session_repo, user_repo, AppState};
use chrono::{Duration, Utc};
use rand::RngCore;
use serde::Serialize;
use uuid::Uuid;

const OTP_TTL_SECONDS: i64 = 120;
const OTP_MAX_ATTEMPTS: i16 = 3;
const REFRESH_TOKEN_TTL_DAYS: i64 = 7;
const MFA_CHALLENGE_TTL_SECONDS: i64 = 300;

#[derive(Debug, Serialize)]
pub struct LoginUserDto {
    pub id: Uuid,
    pub role: UserRole,
    pub phone_number: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponseDto {
    pub access_token: String,
    pub refresh_token: String,
    pub user: LoginUserDto,
}

pub struct AuthService;

impl AuthService {
    /// Step 1 of patient login: generate, hash, and persist a real one-time code.
    /// In `development`, the plaintext code is also returned so test/local clients without a
    /// real SMS gateway can complete the flow; this is never populated in any other environment.
    pub async fn request_patient_otp(state: &AppState, phone: &str) -> Result<(String, i64, Option<String>), AppError> {
        validation::validate_phone_number(phone)?;

        let code = crypto::generate_numeric_code(6);
        let code_hash = crypto::hash_code(&code);
        let expires_at = Utc::now() + Duration::seconds(OTP_TTL_SECONDS);
        otp_repo::upsert(&state.db, phone, &code_hash, expires_at).await?;

        // Real deployments dispatch this via an SMS gateway. Never log the code itself.
        tracing::info!("OTP generated and dispatched for a patient login attempt");

        let debug_code = (state.config.environment == "development").then_some(code);
        Ok((Uuid::new_v4().to_string(), OTP_TTL_SECONDS, debug_code))
    }

    /// Step 2 of patient login: verify the real OTP, find-or-create the patient, issue tokens.
    pub async fn verify_otp_and_login(state: &AppState, phone: &str, otp_code: &str) -> Result<LoginResponseDto, AppError> {
        let record = otp_repo::find(&state.db, phone)
            .await?
            .ok_or_else(|| AppError::Unauthorized("No OTP was requested for this number, or it has expired.".into()))?;

        if Utc::now() > record.expires_at {
            otp_repo::delete(&state.db, phone).await?;
            return Err(AppError::Unauthorized("OTP has expired. Please request a new code.".into()));
        }

        if record.attempts >= OTP_MAX_ATTEMPTS {
            return Err(AppError::RateLimitExceeded(
                "Too many incorrect attempts. Please request a new OTP.".into(),
            ));
        }

        if !crypto::verify_code(otp_code, &record.code_hash) {
            otp_repo::increment_attempts(&state.db, phone).await?;
            return Err(AppError::Unauthorized("Incorrect OTP code.".into()));
        }

        otp_repo::delete(&state.db, phone).await?;

        let user = match user_repo::find_by_phone(&state.db, phone).await? {
            Some(u) => u,
            None => user_repo::create_patient_user(&state.db, phone).await?,
        };

        if patient_repo::find_by_user_id(&state.db, user.id).await?.is_none() {
            let now = Utc::now();
            let profile = PatientProfile {
                id: Uuid::new_v4(),
                user_id: user.id,
                full_name: "Patient User".into(),
                date_of_birth: chrono::NaiveDate::from_ymd_opt(1995, 1, 1).unwrap(),
                gender: Gender::Other,
                blood_group: None,
                weight_kg: None,
                emergency_contact_phone: None,
                emergency_contact_relation: None,
                created_at: now,
                updated_at: now,
            };
            patient_repo::insert(&state.db, &profile).await?;
        }

        Self::issue_login_response(state, &user).await
    }

    /// Doctor step 1: real Argon2id password verification, issues a second-factor challenge.
    /// In `development`, the plaintext second-factor code is also returned (see `request_patient_otp`).
    pub async fn doctor_login_step1(state: &AppState, license_number: &str, password: &str) -> Result<(String, Option<String>), AppError> {
        validation::validate_license_number(license_number)?;

        let invalid_credentials = || AppError::Unauthorized("Invalid license number or password.".into());

        let doctor = doctor_repo::find_by_license(&state.db, license_number)
            .await?
            .ok_or_else(invalid_credentials)?;
        let user = user_repo::find_by_id(&state.db, doctor.user_id)
            .await?
            .ok_or_else(invalid_credentials)?;
        let hash = user.password_hash.as_deref().ok_or_else(invalid_credentials)?;

        if !crypto::verify_password(password, hash) {
            return Err(invalid_credentials());
        }

        let code = crypto::generate_numeric_code(6);
        let code_hash = crypto::hash_code(&code);
        let token = Uuid::new_v4().to_string();
        let expires_at = Utc::now() + Duration::seconds(MFA_CHALLENGE_TTL_SECONDS);
        session_repo::insert_pending_challenge(&state.db, &token, user.id, &code_hash, expires_at).await?;

        tracing::info!("Doctor second-factor code generated and dispatched");

        let debug_code = (state.config.environment == "development").then_some(code);
        Ok((token, debug_code))
    }

    /// Doctor step 2: verify the real second-factor code for the pending challenge session.
    pub async fn doctor_verify_otp(state: &AppState, session_token: &str, otp_code: &str) -> Result<LoginResponseDto, AppError> {
        let challenge = session_repo::find_pending_challenge(&state.db, session_token)
            .await?
            .ok_or_else(|| AppError::Unauthorized("Invalid or expired verification session.".into()))?;

        if Utc::now() > challenge.expires_at {
            session_repo::delete_pending_challenge(&state.db, session_token).await?;
            return Err(AppError::Unauthorized("Verification session expired. Please log in again.".into()));
        }

        if !crypto::verify_code(otp_code, &challenge.code_hash) {
            return Err(AppError::Unauthorized("Incorrect verification code.".into()));
        }

        session_repo::delete_pending_challenge(&state.db, session_token).await?;

        let user = user_repo::find_by_id(&state.db, challenge.user_id)
            .await?
            .ok_or_else(|| AppError::Internal("User record for verified session is missing.".into()))?;

        Self::issue_login_response(state, &user).await
    }

    /// Admin login: real Argon2id password + real TOTP second factor.
    pub async fn admin_login(state: &AppState, email: &str, password: &str, totp_code: &str) -> Result<LoginResponseDto, AppError> {
        let invalid_credentials = || AppError::Unauthorized("Invalid credentials.".into());

        let user = user_repo::find_by_email(&state.db, email)
            .await?
            .ok_or_else(invalid_credentials)?;
        let hash = user.password_hash.as_deref().ok_or_else(invalid_credentials)?;

        if !crypto::verify_password(password, hash) {
            return Err(invalid_credentials());
        }

        let mfa = session_repo::find_mfa_credential(&state.db, user.id, MfaMethod::Totp)
            .await?
            .ok_or_else(|| AppError::Unauthorized("TOTP is not configured for this account.".into()))?;

        let secret = crypto::decrypt_totp_secret(&mfa.totp_secret_encrypted, &state.config.totp_encryption_key_b64)?;
        if !crypto::verify_totp(&secret, totp_code)? {
            return Err(AppError::Unauthorized("Invalid TOTP code.".into()));
        }

        session_repo::touch_mfa_verified(&state.db, mfa.id, Utc::now()).await?;

        Self::issue_login_response(state, &user).await
    }

    /// Rotates a refresh token, detecting reuse of an already-rotated token by revoking the whole family.
    pub async fn refresh_session(state: &AppState, presented_refresh_token: &str) -> Result<(String, String), AppError> {
        let presented_hash = crypto::hash_code(presented_refresh_token);

        let session = session_repo::find_active_by_token_hash(&state.db, &presented_hash)
            .await?
            .ok_or_else(|| AppError::Unauthorized("Refresh token is invalid, expired, or has been revoked.".into()))?;

        if Utc::now() > session.expires_at {
            return Err(AppError::Unauthorized("Refresh token has expired.".into()));
        }

        // Reuse detection: if this hash was already rotated away from (rotated_at set),
        // presenting it again means the token was stolen — revoke the entire family.
        if session.rotated_at.is_some() {
            session_repo::revoke_family(&state.db, session.token_family_id).await?;
            return Err(AppError::Unauthorized(
                "Refresh token reuse detected; all sessions in this family have been revoked.".into(),
            ));
        }

        let user = user_repo::find_by_id(&state.db, session.user_id)
            .await?
            .ok_or_else(|| AppError::Internal("User for this session no longer exists.".into()))?;

        let new_refresh_raw = generate_opaque_token();
        let new_refresh_hash = crypto::hash_code(&new_refresh_raw);
        session_repo::rotate(&state.db, session.id, &new_refresh_hash, Utc::now()).await?;

        let access_token = crypto::create_access_token(user.id, user.role.as_str(), &state.config.jwt_ed25519_private_key_b64)?;

        Ok((access_token, new_refresh_raw))
    }

    async fn issue_login_response(state: &AppState, user: &User) -> Result<LoginResponseDto, AppError> {
        let refresh_raw = generate_opaque_token();
        let refresh_hash = crypto::hash_code(&refresh_raw);
        let now = Utc::now();

        let session = AuthSession {
            id: Uuid::new_v4(),
            user_id: user.id,
            token_family_id: Uuid::new_v4(),
            refresh_token_hash: refresh_hash,
            device_fingerprint: None,
            ip_hash: None,
            user_agent: None,
            created_at: now,
            last_used_at: now,
            rotated_at: None,
            revoked_at: None,
            expires_at: now + Duration::days(REFRESH_TOKEN_TTL_DAYS),
        };
        session_repo::insert_session(&state.db, &session).await?;

        let access_token = crypto::create_access_token(user.id, user.role.as_str(), &state.config.jwt_ed25519_private_key_b64)?;

        Ok(LoginResponseDto {
            access_token,
            refresh_token: refresh_raw,
            user: LoginUserDto {
                id: user.id,
                role: user.role,
                phone_number: user.phone_number.clone(),
            },
        })
    }
}

fn generate_opaque_token() -> String {
    let mut bytes = [0u8; 48];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}
