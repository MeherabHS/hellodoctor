use crate::auth_context::AuthContext;
use crate::crypto;
use crate::domain::ids::{PatientProfileId, UserId};
use crate::domain::models::*;
use crate::domain::validation;
use crate::error::AppError;
use crate::ip_jail;
use crate::repository::{
    doctor_repo, notification_repo, otp_repo, patient_repo, session_repo, user_repo, AppState,
};
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
    pub id: UserId,
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
    pub async fn request_patient_otp(
        state: &AppState,
        phone: &str,
    ) -> Result<(String, i64, Option<String>), AppError> {
        validation::validate_phone_number(phone)?;
        if state.config.environment != "development" || !state.config.dev_auth_otp_enabled {
            return Err(AppError::Conflict(
                "SMS delivery is not configured. No verification code was dispatched.".into(),
            ));
        }

        let code = crypto::generate_numeric_code(6);
        let code_hash = crypto::hash_code(&code);
        let expires_at = Utc::now() + Duration::seconds(OTP_TTL_SECONDS);
        otp_repo::upsert(&state.auth_db, phone, &code_hash, expires_at).await?;

        // Real deployments dispatch this via an SMS gateway. Never log the code itself.
        tracing::info!("Development patient OTP generated; SMS delivery bypassed");

        let debug_code = (state.config.environment == "development"
            && state.config.dev_auth_otp_enabled)
            .then_some(code);
        Ok((Uuid::new_v4().to_string(), OTP_TTL_SECONDS, debug_code))
    }

    /// Step 2 of patient login: verify the real OTP, find-or-create the patient, issue tokens.
    pub async fn verify_otp_and_login(
        state: &AppState,
        phone: &str,
        otp_code: &str,
    ) -> Result<LoginResponseDto, AppError> {
        let mut otp_tx = state.auth_db.begin().await?;
        let record = otp_repo::find_for_update(&mut otp_tx, phone)
            .await?
            .ok_or_else(|| {
                AppError::Unauthorized(
                    "No OTP was requested for this number, or it has expired.".into(),
                )
            })?;

        if Utc::now() >= record.expires_at {
            otp_repo::delete(&mut *otp_tx, phone).await?;
            otp_tx.commit().await?;
            return Err(AppError::Unauthorized(
                "OTP has expired. Please request a new code.".into(),
            ));
        }

        if record.attempts >= OTP_MAX_ATTEMPTS {
            return Err(AppError::RateLimitExceeded(
                "Too many incorrect attempts. Please request a new OTP.".into(),
            ));
        }

        if !crypto::verify_code(otp_code, &record.code_hash) {
            otp_repo::increment_attempts(&mut *otp_tx, phone).await?;
            otp_tx.commit().await?;
            return Err(AppError::Unauthorized("Incorrect OTP code.".into()));
        }

        otp_repo::delete(&mut *otp_tx, phone).await?;
        // Commit the single-use consumption before account/profile bootstrap,
        // matching the Doctor MFA pattern. A waiting verifier sees no OTP and
        // cannot create a second account or login session with the same code.
        otp_tx.commit().await?;

        // Account and profile are one bootstrap transaction. Never leave a new
        // canonical Patient identity behind if profile creation fails.
        let mut tx = state.auth_db.begin().await?;
        let user = match user_repo::find_by_phone(&mut *tx, phone).await? {
            Some(u) => u,
            None => {
                // The admission cap only gates brand-new activation, never an
                // existing user's own login — this branch is reached only
                // for a phone number with no account yet.
                let active_count = user_repo::count_active(&mut *tx).await?;
                if active_count as u32 >= state.config.max_active_users {
                    return Err(AppError::CapacityLimitReached(
                        "HelloDoctor is at its current controlled pilot capacity. Please try again later.".into(),
                    ));
                }
                user_repo::create_patient_user(&mut *tx, phone).await?
            }
        };

        // No JWT exists yet at this point in the login flow — the OTP check just above
        // *is* the authentication event, so a pseudo-context for this now-verified user
        // is what RLS sees, not a real bearer token. Both the existence check and the
        // insert must run under that same context, or the check would always see "no
        // profile" (RLS hides the row with no GUC set) and re-insert on every login.
        if user.role != UserRole::Patient || !user.is_active {
            return Err(AppError::Unauthorized(
                "Patient account unavailable.".into(),
            ));
        }
        let self_ctx = AuthContext {
            user_id: user.id,
            role: user.role,
            permissions: crypto::default_permissions_for_role(user.role.as_str()),
            session_id: None,
            request_id: None,
        };
        // patient_profiles INSERT is auth-worker-only (self-registration happens
        // at the login/bootstrap boundary, not as an ordinary API write) — see
        // migrations/20261003000001_rls_hardening.
        sqlx::query("SELECT set_config('app.current_user_id',$1,true),set_config('app.current_role','PATIENT',true)")
            .bind(self_ctx.user_id.to_string()).execute(&mut *tx).await?;
        sqlx::query("SELECT set_config('app.current_patient_id',COALESCE((SELECT id::text FROM patient_profiles WHERE user_id=$1),''),true),set_config('app.current_doctor_id','',true),set_config('app.current_session_id','',true),set_config('app.current_request_id','',true)")
            .bind(self_ctx.user_id).execute(&mut *tx).await?;
        if patient_repo::find_by_user_id(&mut *tx, user.id)
            .await?
            .is_none()
        {
            let now = Utc::now();
            let profile = PatientProfile {
                id: PatientProfileId::new(),
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
            patient_repo::insert(&mut *tx, &profile).await?;
        }
        tx.commit().await?;

        Self::issue_login_response(state, &user).await
    }

    /// Doctor step 1: real Argon2id password verification, issues a second-factor challenge.
    /// In `development`, the plaintext second-factor code is also returned (see `request_patient_otp`).
    pub async fn doctor_login_step1(
        state: &AppState,
        email: &str,
        password: &str,
    ) -> Result<(String, Option<String>), AppError> {
        let invalid_credentials = || AppError::Unauthorized("Invalid email or password.".into());

        let user = user_repo::find_by_email(&state.auth_db, email)
            .await?
            .ok_or_else(invalid_credentials)?;
        if user.role != UserRole::Doctor || !user.is_active {
            return Err(invalid_credentials());
        }
        // Confirms a doctor_profiles row actually exists for this user (and
        // not e.g. a deleted/orphaned account) — the profile itself isn't
        // otherwise needed here; downstream steps resolve doctor context
        // from the user id in the issued claims.
        doctor_repo::find_by_user_id(&state.auth_db, user.id)
            .await?
            .ok_or_else(invalid_credentials)?;
        let hash = user
            .password_hash
            .as_deref()
            .ok_or_else(invalid_credentials)?;

        if !crypto::verify_password_async(password.to_owned(), hash.to_owned()).await? {
            return Err(invalid_credentials());
        }

        if state.config.environment != "development" || !state.config.dev_auth_otp_enabled {
            return Err(AppError::Conflict(
                "SMS delivery is not configured. No verification code was dispatched.".into(),
            ));
        }

        let code = crypto::generate_numeric_code(6);
        let code_hash = crypto::hash_code(&code);
        let token = Uuid::new_v4().to_string();
        let expires_at = Utc::now() + Duration::seconds(MFA_CHALLENGE_TTL_SECONDS);
        session_repo::insert_pending_challenge(
            &state.auth_db,
            &token,
            user.id,
            &code_hash,
            expires_at,
        )
        .await?;

        tracing::info!("Development Doctor MFA code generated; SMS delivery bypassed");

        let debug_code = (state.config.environment == "development"
            && state.config.dev_auth_otp_enabled)
            .then_some(code);
        Ok((token, debug_code))
    }

    /// Doctor step 2: verify the real second-factor code for the pending challenge session.
    ///
    /// Brute-force protection (previously entirely absent on this endpoint —
    /// only `doctor_login`'s password step was jailed/rate-limited, leaving
    /// the actual second factor guessable without limit): a per-challenge
    /// `attempt_count`/`max_attempts` pair, checked and incremented
    /// atomically via `SELECT ... FOR UPDATE` so concurrent guesses against
    /// the same challenge can't race past the limit, plus the *same*
    /// account-level jail `doctor_login` already uses (keyed on license
    /// number, carrier-NAT-safe) — a run of wrong MFA codes locks the whole
    /// account, not just this one challenge, exactly like a run of wrong
    /// passwords does.
    pub async fn doctor_verify_otp(
        state: &AppState,
        ip: &str,
        session_token: &str,
        otp_code: &str,
    ) -> Result<LoginResponseDto, AppError> {
        if ip_jail::is_ip_jailed(&state.redis, ip).await? {
            return Err(AppError::Forbidden(
                "Too many failed login attempts from this network. Try again later.".into(),
            ));
        }

        let mut tx = state.auth_db.begin().await?;

        let challenge = session_repo::find_pending_challenge_for_update(&mut tx, session_token)
            .await?
            .ok_or_else(|| {
                AppError::Unauthorized("Invalid or expired verification session.".into())
            })?;

        // Fetched once and reused below for the final login response too —
        // must be the *same* key `doctor_login`'s password step jails
        // (email-based), or a repeated-MFA-failure jail would never block a
        // fresh login attempt on the same account (confirmed by
        // test_doctor_mfa_brute_force_is_blocked).
        let user = user_repo::find_by_id(&state.auth_db, challenge.user_id)
            .await?
            .ok_or_else(|| {
                AppError::internal("User record for a pending MFA challenge is missing.".into())
            })?;
        if !user.is_active || user.role != UserRole::Doctor {
            session_repo::delete_pending_challenge(&mut *tx, session_token).await?;
            tx.commit().await?;
            return Err(AppError::Unauthorized(
                "This login session is no longer available.".into(),
            ));
        }
        let account_key = format!(
            "doctor:{}",
            user.email
                .as_deref()
                .unwrap_or(&challenge.user_id.to_string())
        );

        if ip_jail::is_account_jailed(&state.redis, &account_key).await? {
            return Err(AppError::Forbidden(
                "Too many failed login attempts for this account. Try again later.".into(),
            ));
        }

        if Utc::now() > challenge.expires_at {
            session_repo::delete_pending_challenge(&mut *tx, session_token).await?;
            tx.commit().await?;
            return Err(AppError::Unauthorized(
                "Verification session expired. Please log in again.".into(),
            ));
        }

        if challenge.attempt_count >= challenge.max_attempts {
            session_repo::delete_pending_challenge(&mut *tx, session_token).await?;
            tx.commit().await?;
            return Err(AppError::Unauthorized(
                "Too many incorrect attempts for this verification session. Please log in again."
                    .into(),
            ));
        }

        if !crypto::verify_code(otp_code, &challenge.code_hash) {
            session_repo::increment_challenge_attempts(&mut tx, session_token).await?;
            tx.commit().await?;
            ip_jail::record_failure(&state.redis, ip, &account_key).await?;
            return Err(AppError::Unauthorized(
                "Incorrect verification code.".into(),
            ));
        }

        session_repo::delete_pending_challenge(&mut *tx, session_token).await?;
        tx.commit().await?;
        ip_jail::record_success(&state.redis, &account_key).await?;

        Self::issue_login_response(state, &user).await
    }

    /// Admin login: real Argon2id password + real TOTP second factor.
    pub async fn admin_login(
        state: &AppState,
        email: &str,
        password: &str,
        totp_code: &str,
    ) -> Result<LoginResponseDto, AppError> {
        let invalid_credentials = || AppError::Unauthorized("Invalid credentials.".into());

        let user = user_repo::find_by_email(&state.auth_db, email)
            .await?
            .ok_or_else(invalid_credentials)?;
        let hash = user
            .password_hash
            .as_deref()
            .ok_or_else(invalid_credentials)?;

        if !crypto::verify_password_async(password.to_owned(), hash.to_owned()).await? {
            return Err(invalid_credentials());
        }

        let mfa = session_repo::find_mfa_credential(&state.auth_db, user.id, MfaMethod::Totp)
            .await?
            .ok_or_else(|| {
                AppError::Unauthorized("TOTP is not configured for this account.".into())
            })?;

        let secret = crypto::decrypt_totp_secret(
            &mfa.totp_secret_encrypted,
            &state.config.totp_encryption_key_b64,
        )?;
        if !crypto::verify_totp(&secret, totp_code)? {
            return Err(AppError::Unauthorized("Invalid TOTP code.".into()));
        }

        session_repo::touch_mfa_verified(&state.auth_db, mfa.id, Utc::now()).await?;

        Self::issue_login_response(state, &user).await
    }

    /// Consumes one refresh identity and inserts its successor atomically.
    /// All generations share a family lock with reuse detection and logout.
    pub async fn refresh_session(
        state: &AppState,
        presented_refresh_token: &str,
    ) -> Result<(String, String), AppError> {
        let presented_hash = crypto::hash_code(presented_refresh_token);
        let identity = session_repo::find_by_token_hash(&state.auth_db, &presented_hash)
            .await?
            .ok_or_else(|| {
                AppError::Unauthorized(
                    "Refresh token is invalid, expired, or has been revoked.".into(),
                )
            })?;
        let mut tx = state.auth_db.begin().await?;
        session_repo::lock_family(&mut tx, identity.token_family_id).await?;
        // Re-read after locking: a rotation/reuse/logout may have committed
        // while we waited. Consumed hashes are never overwritten.
        let session = session_repo::find_by_token_hash(&mut *tx, &presented_hash)
            .await?
            .ok_or_else(|| AppError::Unauthorized("Refresh token is invalid.".into()))?;
        if session.revoked_at.is_some() {
            return Err(AppError::Unauthorized(
                "Refresh token has been revoked.".into(),
            ));
        }
        if session.rotated_at.is_some() {
            session_repo::revoke_family(&mut *tx, session.token_family_id).await?;
            tx.commit().await?;
            return Err(AppError::Unauthorized(
                "Refresh token reuse detected; all sessions in this family have been revoked."
                    .into(),
            ));
        }
        let now = Utc::now();
        if now >= session.expires_at {
            return Err(AppError::Unauthorized("Refresh token has expired.".into()));
        }
        let user = user_repo::find_by_id(&mut *tx, session.user_id)
            .await?
            .ok_or_else(|| AppError::Unauthorized("This account is no longer available.".into()))?;
        if !user.is_active || user.deleted_at.is_some() {
            session_repo::revoke_family(&mut *tx, session.token_family_id).await?;
            tx.commit().await?;
            return Err(AppError::Unauthorized(
                "This account is no longer available.".into(),
            ));
        }
        let new_refresh_raw = generate_opaque_token();
        let access_token = crypto::create_access_token(
            user.id.into_inner(),
            user.role.as_str(),
            session.token_family_id,
            &state.config.jwt_ed25519_private_key_b64,
        )?;
        if !session_repo::consume_refresh_token(&mut tx, session.id, now).await? {
            return Err(AppError::Unauthorized(
                "Refresh token is no longer usable.".into(),
            ));
        }
        let successor = AuthSession {
            id: Uuid::new_v4(),
            refresh_token_hash: crypto::hash_code(&new_refresh_raw),
            created_at: now,
            last_used_at: now,
            rotated_at: None,
            revoked_at: None,
            // Preserve the family expiry: consumed identities outlive every
            // usable descendant and remain available for reuse detection.
            ..session
        };
        session_repo::insert_session(&mut *tx, &successor).await?;
        tx.commit().await?;
        Ok((access_token, new_refresh_raw))
    }

    /// A consumed ancestor still identifies its family. Serialize logout with
    /// every generation's rotation so no live successor can escape revocation.
    /// Protected requests check the authoritative family on every request, so
    /// logout also invalidates every access JWT from this family immediately.
    pub async fn logout(state: &AppState, presented_refresh_token: &str) -> Result<(), AppError> {
        let presented_hash = crypto::hash_code(presented_refresh_token);
        let Some(identity) =
            session_repo::find_by_token_hash(&state.auth_db, &presented_hash).await?
        else {
            return Ok(());
        };
        let mut tx = state.auth_db.begin().await?;
        session_repo::lock_family(&mut tx, identity.token_family_id).await?;
        let user_id = session_repo::find_by_token_hash(&mut *tx, &presented_hash)
            .await?
            .filter(|session| session.revoked_at.is_none())
            .map(|session| session.user_id);
        session_repo::revoke_family(&mut *tx, identity.token_family_id).await?;
        if let Some(user_id) = user_id {
            // Refresh-token possession resolved the canonical subject. Keep
            // logout and device disabling atomic under this narrow auth role;
            // ordinary API connections need no cross-account token exception.
            sqlx::query("SELECT set_config('app.current_user_id',$1,true),set_config('app.current_role',COALESCE((SELECT role::text FROM users WHERE id=$2),''),true),set_config('app.current_patient_id','',true),set_config('app.current_doctor_id','',true),set_config('app.current_session_id','',true),set_config('app.current_request_id','',true)")
                .bind(user_id.to_string()).bind(user_id).execute(&mut *tx).await?;
            notification_repo::device_tokens::disable_all_for_user(&mut *tx, user_id).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    async fn issue_login_response(
        state: &AppState,
        user: &User,
    ) -> Result<LoginResponseDto, AppError> {
        if !state.config.is_development_or_test()
            && crate::seed::is_fixture_user(user.id.into_inner())
        {
            return Err(AppError::Unauthorized(
                "Development fixtures are unavailable.".into(),
            ));
        }
        if !user.is_active || user.deleted_at.is_some() {
            return Err(AppError::Unauthorized(
                "This account is no longer available.".into(),
            ));
        }
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
        session_repo::insert_session(&state.auth_db, &session).await?;

        let access_token = crypto::create_access_token(
            user.id.into_inner(),
            user.role.as_str(),
            session.token_family_id,
            &state.config.jwt_ed25519_private_key_b64,
        )?;

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
