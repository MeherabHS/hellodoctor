use crate::auth_context::AuthContext;
use crate::crypto;
use crate::domain::ids::{PatientProfileId, UserId};
use crate::domain::models::*;
use crate::domain::validation;
use crate::error::AppError;
use crate::ip_jail;
use crate::repository::{
    doctor_repo, legal_repo, notification_repo, otp_repo, patient_auth_repo, patient_repo,
    session_repo, user_repo, AppState,
};
use chrono::{Datelike, Duration, Utc};
use rand::RngCore;
use serde::Serialize;
use uuid::Uuid;

const OTP_TTL_SECONDS: i64 = 120;
const OTP_MAX_ATTEMPTS: i16 = 3;
const REFRESH_TOKEN_TTL_DAYS: i64 = 7;
const MFA_CHALLENGE_TTL_SECONDS: i64 = 300;

#[path = "auth_abuse.rs"]
pub mod abuse;
#[path = "admin_totp.rs"]
pub mod admin_totp;
#[path = "device_proof.rs"]
pub mod device_proof;

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
    pub device_bound: bool,
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
        if user_repo::find_by_phone(&state.auth_db, phone)
            .await?
            .is_some_and(|u| u.password_hash.is_some())
        {
            return Err(AppError::Unauthorized(
                "Use email, password and email verification to sign in.".into(),
            ));
        }
        if state.config.environment != "development" || !state.config.dev_auth_otp_enabled {
            return Err(AppError::Conflict(
                "SMS delivery is not configured. No verification code was dispatched.".into(),
            ));
        }

        abuse::otp_delivery(state, phone).await?;
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
        accepted_documents: &[crate::api::v1::auth_handlers::AcceptedDocument],
    ) -> Result<LoginResponseDto, AppError> {
        Self::verify_otp_and_login_with_device(state, phone, otp_code, accepted_documents, None)
            .await
    }

    pub async fn verify_otp_and_login_with_device(
        state: &AppState,
        phone: &str,
        otp_code: &str,
        accepted_documents: &[crate::api::v1::auth_handlers::AcceptedDocument],
        device_key: Option<String>,
    ) -> Result<LoginResponseDto, AppError> {
        if user_repo::find_by_phone(&state.auth_db, phone)
            .await?
            .is_some_and(|u| u.password_hash.is_some())
        {
            return Err(AppError::Unauthorized(
                "Use email, password and email verification to sign in.".into(),
            ));
        }
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
        let (user, new_patient) = match user_repo::find_by_phone(&mut *tx, phone).await? {
            Some(u) => (u, false),
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
                (user_repo::create_patient_user(&mut *tx, phone).await?, true)
            }
        };

        let patient_consent = if new_patient {
            let current_documents = legal_repo::list_current(&mut *tx).await?;
            let patient_consent = current_documents
                .iter()
                .find(|doc| doc.document_type.as_str() == "PATIENT_DATA_SHARING_CONSENT")
                .ok_or_else(|| {
                    AppError::internal("Patient consent document is not configured.".to_string())
                })?;
            if !accepted_documents.iter().any(|doc| {
                doc.document_type == patient_consent.document_type.as_str()
                    && doc.version == patient_consent.version
            }) {
                return Err(AppError::Validation(
                "The current Patient data-sharing consent must be accepted before registration."
                    .into(),
            ));
            }

            Some(patient_consent.clone())
        } else {
            None
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
        if !patient_repo::exists_for_user_id(&mut *tx, user.id).await? {
            let now = Utc::now();
            let profile = PatientProfile {
                id: PatientProfileId::new(),
                user_id: Some(user.id),
                full_name: "Patient User".into(),
                date_of_birth: chrono::NaiveDate::from_ymd_opt(1995, 1, 1).unwrap(),
                gender: Gender::Other,
                blood_group: None,
                weight_kg: None,
                emergency_contact_phone: None,
                emergency_contact_relation: None,
                guardian_patient_id: None,
                relationship: None,
                created_at: now,
                updated_at: now,
            };
            patient_repo::insert(&mut *tx, &profile).await?;
        }
        if let Some(patient_consent) = patient_consent {
            let now = Utc::now();
            let acceptance = LegalAcceptance {
                id: Uuid::new_v4(),
                user_id: user.id,
                document_type: patient_consent.document_type,
                document_version: patient_consent.version.clone(),
                document_hash: patient_consent.document_hash.clone(),
                accepted_at: now,
                request_id: None,
                session_id: None,
                ip_hash: None,
                app_version: None,
                platform: None,
                locale: None,
                created_at: now,
            };
            legal_repo::insert(&mut *tx, &acceptance).await?;
        }
        tx.commit().await?;

        Self::issue_login_response_with_device(state, &user, device_key).await
    }

    pub async fn request_patient_email_otp(
        state: &AppState,
        email: &str,
        password: &str,
        phone: Option<&str>,
        register: bool,
        profile: Option<&patient_auth_repo::PatientRegistrationProfile>,
    ) -> Result<(String, i64, Option<String>), AppError> {
        validation::validate_email(email)?;
        abuse::target_attempt(
            state,
            if register {
                "patient-register"
            } else {
                "patient"
            },
            email,
        )
        .await?;
        if register {
            validation::validate_password_strength(password)?;
            let profile = profile
                .ok_or_else(|| AppError::Validation("Registration profile is required.".into()))?;
            let today = Utc::now().date_naive();
            let age = today.year()
                - profile.date_of_birth.year()
                - i32::from(today.ordinal() < profile.date_of_birth.ordinal());
            if profile.full_name.trim().chars().count() < 2
                || profile.full_name.trim().chars().count() > 120
                || profile.full_name.chars().any(char::is_control)
            {
                return Err(AppError::Validation(
                    "Enter your full name (2 to 120 characters).".into(),
                ));
            }
            if !(18..=120).contains(&age) || profile.date_of_birth > today {
                return Err(AppError::Validation("Self-registration requires age 18 to 120. Use an adult guardian's family profile for a child.".into()));
            }
            let active_count = user_repo::count_active(&state.auth_db).await?;
            if active_count as u32 >= state.config.max_active_users {
                return Err(AppError::CapacityLimitReached(
                    "Pilot capacity reached. Please try again later.".into(),
                ));
            }
        } else if password.is_empty() || password.len() > 1024 {
            return Err(AppError::Unauthorized("Invalid email or password.".into()));
        }
        if let Some(phone) = phone {
            validation::validate_phone_number(phone)?;
        }
        let email = email.trim().to_lowercase();
        let existing = user_repo::find_by_email(&state.auth_db, &email).await?;
        let existing_id = existing.as_ref().map(|u| u.id);
        if register {
            if existing.is_some() {
                return Err(AppError::Conflict(
                    "Registration details are already in use.".into(),
                ));
            }
            if let Some(phone) = phone {
                if user_repo::find_by_phone(&state.auth_db, phone)
                    .await?
                    .is_some()
                {
                    return Err(AppError::Conflict(
                        "Registration details are already in use.".into(),
                    ));
                }
            }
        } else {
            // Each fast-exit path below runs a dummy Argon2id verification
            // before returning, so "account doesn't exist / wrong role /
            // inactive / no password set" costs comparable CPU time to a
            // genuine wrong-password check instead of returning instantly —
            // closing the timing gap that would otherwise let a caller infer
            // account existence/state from response latency alone.
            match existing.filter(|u| u.role == UserRole::Patient && u.is_active) {
                Some(user) => match user.password_hash.as_deref() {
                    Some(hash) => {
                        if !crypto::verify_password_async(password.to_owned(), hash.to_owned())
                            .await?
                        {
                            return Err(AppError::Unauthorized(
                                "Invalid email or password.".into(),
                            ));
                        }
                    }
                    None => {
                        crypto::verify_password_dummy_async().await?;
                        return Err(AppError::Unauthorized("Invalid email or password.".into()));
                    }
                },
                None => {
                    crypto::verify_password_dummy_async().await?;
                    return Err(AppError::Unauthorized("Invalid email or password.".into()));
                }
            }
        }
        if state.config.environment != "development" && state.config.azure_email_endpoint.is_none()
        {
            return Err(AppError::Conflict(
                "Azure email OTP delivery is not configured.".into(),
            ));
        }
        abuse::otp_delivery(state, &email).await?;
        let code = crypto::generate_numeric_code(6);
        let token = Uuid::new_v4().to_string();
        let password_hash = if register {
            Some(crypto::hash_password_async(password.to_owned()).await?)
        } else {
            None
        };
        patient_auth_repo::insert(
            &state.auth_db,
            &token,
            &email,
            phone,
            password_hash.as_deref(),
            existing_id,
            if register { "REGISTER" } else { "LOGIN" },
            &crypto::hash_code(&code),
            Utc::now() + Duration::seconds(MFA_CHALLENGE_TTL_SECONDS),
            profile,
        )
        .await?;
        if let Err(error) =
            crate::email_delivery::send_email_otp(&state.config, &email, &code).await
        {
            patient_auth_repo::delete(&state.auth_db, &token).await?;
            return Err(error);
        }
        let debug_code = (state.config.environment == "development"
            && state.config.dev_auth_otp_enabled)
            .then_some(code);
        Ok((token, MFA_CHALLENGE_TTL_SECONDS, debug_code))
    }

    pub async fn verify_patient_email_otp(
        state: &AppState,
        token: &str,
        otp_code: &str,
        accepted_documents: &[crate::api::v1::auth_handlers::AcceptedDocument],
    ) -> Result<LoginResponseDto, AppError> {
        Self::verify_patient_email_otp_with_device(state, token, otp_code, accepted_documents, None)
            .await
    }

    pub async fn verify_patient_email_otp_with_device(
        state: &AppState,
        token: &str,
        otp_code: &str,
        accepted_documents: &[crate::api::v1::auth_handlers::AcceptedDocument],
        device_key: Option<String>,
    ) -> Result<LoginResponseDto, AppError> {
        let mut tx = state.auth_db.begin().await?;
        let challenge = patient_auth_repo::find_for_update(&mut tx, token)
            .await?
            .ok_or_else(|| {
                AppError::Unauthorized("Invalid or expired verification session.".into())
            })?;
        let documents = if challenge.flow == "REGISTER" {
            let documents = legal_repo::list_current(&mut *tx).await?;
            let types = [
                "PATIENT_DATA_SHARING_CONSENT",
                "PATIENT_TERMS",
                "PATIENT_PRIVACY_NOTICE",
                "PATIENT_DATA_RIGHTS",
            ];
            let required: Vec<_> = documents
                .into_iter()
                .filter(|d| types.contains(&d.document_type.as_str()))
                .collect();
            if required.len() != types.len() {
                return Err(AppError::internal(
                    "Patient legal documents are not configured.".into(),
                ));
            }
            if required.iter().any(|doc| {
                !accepted_documents.iter().any(|a| {
                    a.document_type == doc.document_type.as_str() && a.version == doc.version
                })
            }) {
                return Err(AppError::Validation("Read and acknowledge the current Patient terms, privacy policy, data rights and care-data consent before registration.".into()));
            }
            required
        } else {
            Vec::new()
        };
        if Utc::now() >= challenge.expires_at {
            patient_auth_repo::delete(&mut *tx, token).await?;
            tx.commit().await?;
            return Err(AppError::Unauthorized(
                "Verification session expired. Please try again.".into(),
            ));
        }
        if challenge.attempts >= OTP_MAX_ATTEMPTS {
            return Err(AppError::RateLimitExceeded(
                "Too many incorrect attempts. Please request a new code.".into(),
            ));
        }
        if !crypto::verify_code(otp_code, &challenge.code_hash) {
            patient_auth_repo::increment_attempts(&mut tx, token).await?;
            tx.commit().await?;
            return Err(AppError::Unauthorized(
                "Incorrect verification code.".into(),
            ));
        }
        patient_auth_repo::delete(&mut *tx, token).await?;
        let (user, new_patient) = if challenge.flow == "REGISTER" {
            let phone = challenge.phone_number.as_deref().ok_or_else(|| {
                AppError::Validation("A phone number is required for registration.".into())
            })?;
            let password_hash = challenge.password_hash.as_deref().ok_or_else(|| {
                AppError::internal("Registration challenge is incomplete.".into())
            })?;
            let user = user_repo::create_patient_user_with_credentials(
                &mut *tx,
                &challenge.email,
                phone,
                password_hash,
            )
            .await?;
            (user, true)
        } else {
            let user_id = challenge
                .user_id
                .ok_or_else(|| AppError::internal("Login challenge is incomplete.".into()))?;
            let user = user_repo::find_by_id(&mut *tx, user_id)
                .await?
                .ok_or_else(|| AppError::Unauthorized("Patient account unavailable.".into()))?;
            (user, false)
        };
        if user.role != UserRole::Patient || !user.is_active {
            return Err(AppError::Unauthorized(
                "Patient account unavailable.".into(),
            ));
        }
        sqlx::query("SELECT set_config('app.current_user_id',$1,true),set_config('app.current_role','PATIENT',true),set_config('app.current_doctor_id','',true),set_config('app.current_session_id','',true),set_config('app.current_request_id','',true)").bind(user.id.to_string()).execute(&mut *tx).await?;
        sqlx::query("SELECT set_config('app.current_patient_id',COALESCE((SELECT id::text FROM patient_profiles WHERE user_id=$1),''),true)").bind(user.id).execute(&mut *tx).await?;
        if !patient_repo::exists_for_user_id(&mut *tx, user.id).await? {
            let details: patient_auth_repo::PatientRegistrationProfile =
                serde_json::from_value(challenge.registration_profile.ok_or_else(|| {
                    AppError::Validation(
                        "Registration details expired. Please register again.".into(),
                    )
                })?)
                .map_err(|_| AppError::internal("Invalid registration profile.".into()))?;
            let now = Utc::now();
            let profile = PatientProfile {
                id: PatientProfileId::new(),
                user_id: Some(user.id),
                full_name: details.full_name.trim().to_owned(),
                date_of_birth: details.date_of_birth,
                gender: details.gender,
                blood_group: None,
                weight_kg: None,
                emergency_contact_phone: None,
                emergency_contact_relation: None,
                guardian_patient_id: None,
                relationship: None,
                created_at: now,
                updated_at: now,
            };
            patient_repo::insert(&mut *tx, &profile).await?;
        }
        if new_patient {
            for consent in documents {
                let now = Utc::now();
                legal_repo::insert(
                    &mut *tx,
                    &LegalAcceptance {
                        id: Uuid::new_v4(),
                        user_id: user.id,
                        document_type: consent.document_type,
                        document_version: consent.version.clone(),
                        document_hash: consent.document_hash.clone(),
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
        }
        tx.commit().await?;
        Self::issue_login_response_with_device(state, &user, device_key).await
    }

    /// Doctor step 1: real Argon2id password verification, issues a second-factor challenge.
    /// In `development`, the plaintext second-factor code is also returned (see `request_patient_otp`).
    pub async fn doctor_login_step1(
        state: &AppState,
        email: &str,
        password: &str,
    ) -> Result<(String, Option<String>), AppError> {
        let invalid_credentials = || AppError::Unauthorized("Invalid email or password.".into());
        validation::validate_email(email)?;
        if password.is_empty() || password.len() > 1024 {
            return Err(invalid_credentials());
        }
        abuse::target_attempt(state, "doctor", email).await?;

        // Each fast-exit path runs a dummy Argon2id verification before
        // returning — see `request_patient_email_otp`'s matching comment.
        let user = match user_repo::find_by_email(&state.auth_db, email).await? {
            Some(u) if u.role == UserRole::Doctor && u.is_active => u,
            _ => {
                crypto::verify_password_dummy_async().await?;
                return Err(invalid_credentials());
            }
        };
        // Confirms a doctor_profiles row actually exists for this user (and
        // not e.g. a deleted/orphaned account) — the profile itself isn't
        // otherwise needed here; downstream steps resolve doctor context
        // from the user id in the issued claims.
        if doctor_repo::find_by_user_id(&state.auth_db, user.id)
            .await?
            .is_none()
        {
            crypto::verify_password_dummy_async().await?;
            return Err(invalid_credentials());
        }
        let Some(hash) = user.password_hash.as_deref() else {
            crypto::verify_password_dummy_async().await?;
            return Err(invalid_credentials());
        };

        if !crypto::verify_password_async(password.to_owned(), hash.to_owned()).await? {
            return Err(invalid_credentials());
        }

        if state.config.environment != "development" && state.config.azure_email_endpoint.is_none()
        {
            return Err(AppError::Conflict(
                "Email verification is temporarily unavailable.".into(),
            ));
        }

        abuse::otp_delivery(state, email).await?;
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

        if let Err(error) = crate::email_delivery::send_email_otp(&state.config, email, &code).await
        {
            session_repo::delete_pending_challenge(&state.auth_db, &token).await?;
            return Err(error);
        }

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
        Self::doctor_verify_otp_with_device(state, ip, session_token, otp_code, None).await
    }

    pub async fn doctor_verify_otp_with_device(
        state: &AppState,
        ip: &str,
        session_token: &str,
        otp_code: &str,
        device_key: Option<String>,
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
        let user = user_repo::find_by_id(&mut *tx, challenge.user_id)
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

        Self::issue_login_response_with_device(state, &user, device_key).await
    }

    /// Admin password verification issues an emailed OTP challenge. Completing
    /// it additionally requires the enrolled authenticator TOTP in deployed
    /// environments; email possession alone never completes Admin MFA.
    pub async fn admin_login_step1(
        state: &AppState,
        email: &str,
        password: &str,
    ) -> Result<(String, Option<String>), AppError> {
        let invalid_credentials = || AppError::Unauthorized("Invalid credentials.".into());
        validation::validate_email(email)?;
        if password.is_empty() || password.len() > 1024 {
            return Err(invalid_credentials());
        }
        abuse::target_attempt(state, "admin", email).await?;

        // Defense-in-depth: explicitly require the ADMIN role here rather
        // than relying solely on downstream JWT/ABAC authorization to reject
        // a non-admin session later. Folded into the same fast-exit dummy-hash
        // path as a missing/wrong-role account so this check creates no
        // additional timing or error-shape oracle versus a bad password.
        let user = match user_repo::find_by_email(&state.auth_db, email).await? {
            Some(u) if u.role == UserRole::Admin && u.is_active => u,
            _ => {
                crypto::verify_password_dummy_async().await?;
                return Err(invalid_credentials());
            }
        };
        let Some(hash) = user.password_hash.as_deref() else {
            crypto::verify_password_dummy_async().await?;
            return Err(invalid_credentials());
        };

        if !crypto::verify_password_async(password.to_owned(), hash.to_owned()).await? {
            return Err(invalid_credentials());
        }
        let Some(recipient) = user.email.clone() else {
            return Err(AppError::internal(
                "Admin account has no email on file to send a verification code to.".into(),
            ));
        };

        if state.config.environment != "development" && state.config.azure_email_endpoint.is_none()
        {
            return Err(AppError::Conflict(
                "Azure email OTP delivery is not configured.".into(),
            ));
        }

        abuse::otp_delivery(state, &recipient).await?;
        let code = crypto::generate_numeric_code(6);
        let token = Uuid::new_v4().to_string();
        let expires_at = Utc::now() + Duration::seconds(MFA_CHALLENGE_TTL_SECONDS);
        session_repo::insert_pending_challenge(
            &state.auth_db,
            &token,
            user.id,
            &crypto::hash_code(&code),
            expires_at,
        )
        .await?;
        if let Err(error) =
            crate::email_delivery::send_email_otp(&state.config, &recipient, &code).await
        {
            session_repo::delete_pending_challenge(&state.auth_db, &token).await?;
            return Err(error);
        }

        let debug_code = (state.config.environment == "development"
            && state.config.dev_auth_otp_enabled)
            .then_some(code);
        Ok((token, debug_code))
    }

    /// Admin login step 2: verify the emailed code for the pending challenge
    /// session. Mirrors `doctor_verify_otp`'s brute-force protection exactly
    /// (per-challenge `attempt_count`/`max_attempts` under a row lock, plus
    /// the same account-level jail `admin_login_step1`'s caller uses).
    pub async fn admin_verify_otp(
        state: &AppState,
        ip: &str,
        session_token: &str,
        otp_code: &str,
    ) -> Result<LoginResponseDto, AppError> {
        Self::admin_verify_otp_with_totp(state, ip, session_token, otp_code, None).await
    }

    pub async fn admin_verify_otp_with_totp(
        state: &AppState,
        ip: &str,
        session_token: &str,
        otp_code: &str,
        totp_code: Option<&str>,
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

        let user = user_repo::find_by_id(&mut *tx, challenge.user_id)
            .await?
            .ok_or_else(|| {
                AppError::internal("User record for a pending MFA challenge is missing.".into())
            })?;
        if !user.is_active || user.role != UserRole::Admin {
            session_repo::delete_pending_challenge(&mut *tx, session_token).await?;
            tx.commit().await?;
            return Err(AppError::Unauthorized(
                "This login session is no longer available.".into(),
            ));
        }
        let account_key = format!(
            "admin:{}",
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

        if let Err(error) = admin_totp::verify(state, &mut tx, user.id, totp_code).await {
            if matches!(error, AppError::Unauthorized(_)) {
                session_repo::increment_challenge_attempts(&mut tx, session_token).await?;
                tx.commit().await?;
                ip_jail::record_failure(&state.redis, ip, &account_key).await?;
            }
            return Err(error);
        }
        session_repo::delete_pending_challenge(&mut *tx, session_token).await?;
        tx.commit().await?;
        ip_jail::record_success(&state.redis, &account_key).await?;

        Self::issue_login_response(state, &user).await
    }

    /// Consumes one refresh identity and inserts its successor atomically.
    /// All generations share a family lock with reuse detection and logout.
    pub async fn refresh_session(
        state: &AppState,
        presented_refresh_token: &str,
    ) -> Result<(String, String), AppError> {
        Self::refresh_session_with_proof(state, presented_refresh_token, None).await
    }

    pub async fn refresh_session_with_proof(
        state: &AppState,
        presented_refresh_token: &str,
        request: Option<(
            &axum::http::HeaderMap,
            &axum::http::Method,
            &axum::http::Uri,
        )>,
    ) -> Result<(String, String), AppError> {
        if presented_refresh_token.len() > 256 {
            return Err(AppError::Unauthorized("Refresh token is invalid.".into()));
        }
        let presented_hash = crypto::hash_code(presented_refresh_token);
        let identity = session_repo::find_by_token_hash(&state.auth_db, &presented_hash)
            .await?
            .ok_or_else(|| {
                AppError::Unauthorized(
                    "Refresh token is invalid, expired, or has been revoked.".into(),
                )
            })?;
        // A stolen credential cannot rotate OR revoke a bound family through
        // reuse detection without first proving possession of its device key.
        if let Some(key) = device_proof::family_thumbprint(state, identity.token_family_id).await? {
            let (headers, method, uri) = request
                .ok_or_else(|| AppError::Unauthorized("Session proof is required.".into()))?;
            device_proof::verify(
                state,
                headers,
                method,
                uri,
                presented_refresh_token,
                Some(&key),
            )
            .await?;
        }
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
            tracing::warn!(event = "SESSION_REFRESH_REUSE_DETECTED", family_id = %session.token_family_id, "Refresh family revoked");
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
        tracing::info!(event = "SESSION_REFRESHED", session_id = %successor.id, "Session refreshed");
        Ok((access_token, new_refresh_raw))
    }

    /// A consumed ancestor still identifies its family. Serialize logout with
    /// every generation's rotation so no live successor can escape revocation.
    /// Protected requests check the authoritative family on every request, so
    /// logout also invalidates every access JWT from this family immediately.
    pub async fn logout(state: &AppState, presented_refresh_token: &str) -> Result<(), AppError> {
        Self::logout_with_proof(state, presented_refresh_token, None).await
    }

    pub async fn logout_with_proof(
        state: &AppState,
        presented_refresh_token: &str,
        request: Option<(
            &axum::http::HeaderMap,
            &axum::http::Method,
            &axum::http::Uri,
        )>,
    ) -> Result<(), AppError> {
        if presented_refresh_token.len() > 256 {
            return Ok(());
        }
        let presented_hash = crypto::hash_code(presented_refresh_token);
        let Some(identity) =
            session_repo::find_by_token_hash(&state.auth_db, &presented_hash).await?
        else {
            return Ok(());
        };
        if let Some(key) = device_proof::family_thumbprint(state, identity.token_family_id).await? {
            let (headers, method, uri) = request
                .ok_or_else(|| AppError::Unauthorized("Session proof is required.".into()))?;
            device_proof::verify(
                state,
                headers,
                method,
                uri,
                presented_refresh_token,
                Some(&key),
            )
            .await?;
        }
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
            sqlx::query("SELECT set_config('app.current_user_id',$1,true),set_config('app.current_role',COALESCE((SELECT role::text FROM users WHERE id=$2),''),true),set_config('app.current_patient_id','',true),set_config('app.current_doctor_id','',true),set_config('app.current_session_id',$3,true),set_config('app.current_request_id',$4,true)")
                .bind(user_id.to_string()).bind(user_id).bind(identity.id.to_string()).bind(crate::api::middleware::request_context::request_id()).execute(&mut *tx).await?;
            notification_repo::device_tokens::disable_all_for_user(&mut *tx, user_id).await?;
            let role: String = sqlx::query_scalar("SELECT role::text FROM users WHERE id=$1")
                .bind(user_id)
                .fetch_one(&mut *tx)
                .await?;
            let role = UserRole::parse(&role)
                .ok_or_else(|| AppError::internal("Invalid account role".into()))?;
            crate::services::audit_service::AuditService::log_in_tx(
                &mut *tx,
                crate::services::audit_service::AuditLogEntry::new(
                    Some(user_id),
                    role,
                    "SESSION_REVOKED",
                    "AUTH_SESSION",
                    Some(identity.id),
                    "SUCCESS",
                ),
            )
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    async fn issue_login_response(
        state: &AppState,
        user: &User,
    ) -> Result<LoginResponseDto, AppError> {
        Self::issue_login_response_with_device(state, user, None).await
    }

    async fn issue_login_response_with_device(
        state: &AppState,
        user: &User,
        device_key: Option<String>,
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
        let access_token = crypto::create_access_token(
            user.id.into_inner(),
            user.role.as_str(),
            session.token_family_id,
            &state.config.jwt_ed25519_private_key_b64,
        )?;
        let mut tx = state.auth_db.begin().await?;
        session_repo::insert_session(&mut *tx, &session).await?;
        if let Some(key) = &device_key {
            sqlx::query("INSERT INTO auth_session_device_keys(token_family_id,key_thumbprint) VALUES($1,$2)")
                .bind(session.token_family_id).bind(key).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        tracing::info!(event = "SESSION_CREATED", session_id = %session.id, user_id = %user.id, device_bound = device_key.is_some(), "Session created");

        Ok(LoginResponseDto {
            device_bound: device_key.is_some(),
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
